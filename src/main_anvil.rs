//! Canonical AnvilMenu lifetime and actual Main owner publication.
//!
//! This module owns active UI instances, not another player inventory or world.
//! The context and dispatch/lifecycle modules supply their separately claimed
//! actual owner capabilities. Packet/drop callbacks retain their source order.
use super::*;
use crate::main_anvil_context::{
    NativeAnvilBinding, NativeAnvilEvent, NativeAnvilGameplayCertificate, NativeAnvilHost,
    NativeAnvilStage,
};
use main_item_gameplay::MainItemGameplay;
use mc121_rust::{
    container::MenuCloseContext,
    container_workstation_anvil::{AnvilMenu, AnvilMenuAction},
    inventory::PlayerInventory,
    item_entity::{spawn_player_drop_with_factory, PlayerDrop},
    nbt::JavaString,
    player_stats::{DropStatEvent, PlayerStats},
    runtime_entities::{ItemBatchPublication, PreparedItemBatch},
    workstations::menu::{SyncEvent, WorkstationKind},
};
use std::{cell::RefCell, rc::Rc};

#[derive(Debug)]
pub(super) struct OpenAnvil {
    pub(super) position: (i32, i32, i32),
    pub(super) binding: NativeAnvilBinding,
    pub(super) menu: AnvilMenu<NativeAnvilBinding>,
}
#[derive(Default)]
pub(super) struct NativeAnvilRuntime {
    pub(super) menus: BTreeMap<i32, OpenAnvil>,
}
impl NativeAnvilRuntime {
    pub(super) fn window_id(&self, id: i32) -> Option<u8> {
        self.menus.get(&id).map(|open| open.menu.window_id())
    }
    pub(super) fn has_open(&self, id: i32) -> bool {
        self.menus.contains_key(&id)
    }
    pub(super) fn pending_players(&self) -> Vec<i32> {
        self.menus.keys().copied().collect()
    }
    /// Read-only active-menu observation for existing player/item callbacks.
    /// The retained InventoryMenu still owns its separate cursor and 2x2 grid.
    pub(super) fn inventory_snapshot(
        &self,
        id: i32,
        inventory: &PlayerInventory,
    ) -> PlayerInventory {
        self.menus.get(&id).map_or_else(
            || inventory.clone(),
            |open| inventory.snapshot_with_carried(open.menu.carried()),
        )
    }
}

pub(super) enum AnvilRequest<'a> {
    Click(&'a ContainerClick),
    Rename(&'a JavaString),
    Close {
        window: u8,
        send_close: bool,
        context: MenuCloseContext,
    },
    /// ServerPlayer.tick broadcasts before its following validity/close check.
    Broadcast,
    /// Retained full-state resend, including source spectator click handling.
    Resync,
}

fn publish(events: &broadcast::Sender<PlayerEvent>, id: i32, packets: Vec<Packet>) {
    if !packets.is_empty() {
        let _ = events.send(PlayerEvent::WorldAudience {
            recipients: Arc::new(vec![id]),
            packets: Arc::new(packets),
        });
    }
}

/// Prepared against the real locked owners before the native UI is adopted.
/// Exclusive item-store/gameplay/RNG borrows prevent a competing publication.
struct PreparedAnvilCommit<'a> {
    items: PreparedItemBatch<'a>,
    player_random: &'a mut LegacyRandomSource,
    next_player_random: LegacyRandomSource,
    next_world_random: LegacyRandomSource,
    entity_counter: &'a AtomicI32,
    next_entity_id: i32,
    gameplay: &'a mut MainItemGameplay,
    events: Vec<NativeAnvilEvent>,
    final_inventory: PlayerInventory,
    stats_after_event: Vec<PlayerStats>,
    drop_ordinals: Vec<Option<usize>>,
    level_packets: BTreeMap<usize, Packet>,
    level_recipients: Arc<Vec<i32>>,
    prefix: Vec<Packet>,
}
impl PreparedAnvilCommit<'_> {
    /// The retained actual UI has already been installed before this starts.
    /// Everything below is infallible source-ordered owner publication.
    fn commit(
        self,
        id: i32,
        uuid: [u8; 16],
        events: &broadcast::Sender<PlayerEvent>,
        canonical: &mut PlayerContainer,
    ) {
        let Self {
            items,
            player_random,
            next_player_random,
            next_world_random,
            entity_counter,
            next_entity_id,
            gameplay,
            events: observations,
            final_inventory,
            stats_after_event,
            drop_ordinals,
            level_packets,
            level_recipients,
            prefix,
        } = self;
        *player_random = next_player_random;
        entity_counter.store(next_entity_id, Ordering::Relaxed);
        publish(events, id, prefix);
        let mut cursor = 0;
        let mut pending_drop: Option<usize> = None;
        items.commit_with_world_random_ordered(
            next_world_random,
            |ordinal: usize, _: &mut ItemBatchPublication<'_>| {
                if let Some(index) = pending_drop.take() {
                    // Player.drop awards after the actual entity world-add.
                    *gameplay
                        .stats_mut(uuid)
                        .expect("validated anvil statistic owner") =
                        stats_after_event[index].clone();
                    cursor = index + 1;
                }
                while cursor < observations.len() {
                    let index = cursor;
                    let observation = &observations[index];
                    canonical.inventory = observation.inventory().clone();
                    if let Some(expected) = drop_ordinals[index] {
                        assert_eq!(expected, ordinal, "validated anvil indexed drop order");
                        pending_drop = Some(index);
                        return;
                    }
                    *gameplay
                        .stats_mut(uuid)
                        .expect("validated anvil statistic owner") =
                        stats_after_event[index].clone();
                    match observation {
                        NativeAnvilEvent::Sync {
                            event:
                                SyncEvent::SlotListener {
                                    player_slot: Some(slot),
                                    stack,
                                    ..
                                },
                            ..
                        } => {
                            gameplay
                                .inventory_slot_changed(uuid, *slot, &canonical.inventory, stack)
                                .expect("validated native empty-criteria anvil listener");
                        }
                        NativeAnvilEvent::Sync {
                            event: SyncEvent::Packet(packet),
                            ..
                        } => {
                            publish(events, id, vec![packet.clone()]);
                        }
                        NativeAnvilEvent::InventoryChanged { .. } => {
                            // This observation is BEFORE the exact one native
                            // Inventory.setChanged increment, not a listener.
                            canonical.inventory.set_changed();
                        }
                        NativeAnvilEvent::LevelEvent { .. } => {
                            let packet = level_packets
                                .get(&index)
                                .expect("prepared anvil LevelEvent");
                            if !level_recipients.is_empty() {
                                let _ = events.send(PlayerEvent::WorldAudience {
                                    recipients: Arc::clone(&level_recipients),
                                    packets: Arc::new(vec![packet.clone()]),
                                });
                            }
                        }
                        // Original cqx constructor input/data listeners have
                        // no additional callbacks under the certified floor.
                        _ => {}
                    }
                    cursor += 1;
                }
            },
        );
        assert!(pending_drop.is_none() && cursor == observations.len());
        canonical.inventory = final_inventory;
    }
}

fn preflight_observations(events: &[NativeAnvilEvent]) -> Result<()> {
    for observation in events {
        let phase = observation.phase();
        for stack in phase
            .inputs
            .iter()
            .chain([&phase.result_preview, &phase.carried])
        {
            main_anvil_context::require_anvil_stack_context(stack)?;
        }
        let inventory = observation.inventory();
        main_anvil_context::require_anvil_inventory_context(inventory)?;
        // These are the exact backing queries made by the actual inventory
        // criterion scan; resolve them before the first real observation.
        for slot in (36..45)
            .chain(9..36)
            .chain((5..9).rev())
            .chain(std::iter::once(45))
        {
            let stack = inventory
                .slot(slot)
                .context("anvil phase inventory slot absent")?;
            if !stack.is_empty() {
                stack.encode_canonical()?;
                stack.max_stack_size()?;
            }
        }
        match observation {
            NativeAnvilEvent::Sync {
                event:
                    SyncEvent::SlotListener {
                        menu_slot,
                        player_slot,
                        stack,
                    },
                ..
            } => {
                stack.encode_canonical()?;
                if let Some(slot) = player_slot {
                    ensure!(
                        (9..45).contains(slot) && *menu_slot + 6 == *slot,
                        "anvil listener is not the actual canonical player backing slot"
                    );
                } else {
                    ensure!(*menu_slot < 3, "unknown anvil input/result backing slot");
                }
            }
            NativeAnvilEvent::Sync {
                event: SyncEvent::DataListener { index, .. },
                ..
            } => {
                ensure!(*index == 0, "unknown anvil source data listener");
            }
            NativeAnvilEvent::Drop { owned, .. } => {
                main_anvil_context::require_anvil_stack_context(&owned.stack)?;
            }
            _ => {}
        }
    }
    Ok(())
}

impl Server {
    pub(super) fn open_anvil_locked(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        id: i32,
        position: (i32, i32, i32),
        window: u8,
        now: i64,
        border: CollisionBorder,
    ) -> Result<()> {
        main_anvil_dispatch::require_anvil_actor(players, id)?;
        ensure!(
            !players.anvils.has_open(id),
            "actual previous anvil must close before opening"
        );
        ensure!(
            window != 0,
            "anvil requires an actual allocated external window"
        );
        let player = players
            .registry
            .get(id)
            .context("anvil opening actor missing")?;
        ensure!(
            main_anvil_dispatch::anvil_still_valid(world, player, position)?,
            "anvil opening access changed"
        );
        self.run_anvil_locked(
            world,
            players,
            id,
            None,
            Some((position, window)),
            None,
            now,
            border,
        )
    }

    pub(super) fn anvil_action_locked(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        id: i32,
        request: AnvilRequest<'_>,
        now: i64,
        border: CollisionBorder,
    ) -> Result<()> {
        let Some(open) = players.anvils.menus.get(&id) else {
            return Ok(());
        };
        if matches!(&request, AnvilRequest::Click(click) if click.window_id != open.menu.window_id())
            || matches!(&request, AnvilRequest::Close { window, .. } if *window != open.menu.window_id())
        {
            return Ok(());
        }
        if matches!(&request, AnvilRequest::Click(click) if click.slot >= 39) {
            return Ok(());
        }
        if matches!(&request, AnvilRequest::Click(_) | AnvilRequest::Rename(_)) {
            main_anvil_dispatch::require_anvil_actor(players, id)?;
            let player = players
                .registry
                .get(id)
                .context("anvil action actor missing")?;
            if !main_anvil_dispatch::anvil_still_valid(world, player, open.position)? {
                return Ok(());
            }
        }
        if let AnvilRequest::Click(click) = &request {
            main_anvil_context::require_anvil_prediction_context(click)?;
        }
        // Loan the one actual UI, never a clone. Failed native transactions
        // leave it unchanged and the outer path restores that same instance.
        let open = players
            .anvils
            .menus
            .remove(&id)
            .expect("borrowed actual anvil");
        self.run_anvil_locked(
            world,
            players,
            id,
            Some(open),
            None,
            Some(request),
            now,
            border,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn run_anvil_locked(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        id: i32,
        open: Option<OpenAnvil>,
        opening: Option<((i32, i32, i32), u8)>,
        request: Option<AnvilRequest<'_>>,
        now: i64,
        border: CollisionBorder,
    ) -> Result<()> {
        // This private holder preserves canonical ownership through every
        // preflight error, including errors before SharedContainer is entered.
        let retained = Rc::new(RefCell::new(open));
        let restore_existing = opening.is_none();
        let result = self.run_anvil_transaction(
            world,
            players,
            id,
            Rc::clone(&retained),
            opening,
            request,
            now,
            border,
        );
        if result.is_err() && restore_existing {
            if let Some(open) = retained.borrow_mut().take() {
                players.anvils.menus.insert(id, open);
            }
        }
        result
    }

    #[allow(clippy::too_many_arguments)]
    fn run_anvil_transaction(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        id: i32,
        retained: Rc<RefCell<Option<OpenAnvil>>>,
        opening: Option<((i32, i32, i32), u8)>,
        request: Option<AnvilRequest<'_>>,
        now: i64,
        border: CollisionBorder,
    ) -> Result<()> {
        players.require_current_native_source(world)?;
        let certificate = NativeAnvilGameplayCertificate::new(&players.gameplay)?;
        // Bind the exact configuration Main sends; an alternate profile is
        // unsupported, not treated as vanilla/all-features-enabled.
        let _ = main_stonecutter_recipes::NativeStonecutterRecipes::new(
            &self.fixtures,
            &players.gameplay,
        )?;
        let player = players
            .registry
            .get(id)
            .context("anvil actual actor missing")?
            .clone();
        players
            .gameplay
            .validate_active_player(id, player.profile.uuid)?;
        let lifecycle = players
            .player_lifecycle
            .get(&player.profile.uuid)
            .context("anvil lifecycle missing")?;
        let instabuild = lifecycle.abilities.instant_build;
        let experience_level = lifecycle.experience.level;
        let (position, window) = opening.unwrap_or_else(|| {
            let held = retained.borrow();
            let open = held.as_ref().expect("retained anvil");
            (open.position, open.menu.window_id())
        });
        let binding = NativeAnvilBinding {
            player_id: id,
            player_uuid: player.profile.uuid,
            position,
            window_id: window,
            dimension: lifecycle.dimension.clone(),
        };
        if let Some(open) = retained.borrow().as_ref() {
            ensure!(
                open.binding == binding,
                "anvil actual actor/window/Level binding changed"
            );
        }
        let access = main_anvil_dispatch::anvil_access(world, &player, position)?;
        let block = world.block(position.0, position.1, position.2)?;
        let shared = players
            .containers
            .get(&id)
            .context("anvil canonical inventory missing")?
            .clone();
        let facts = players.world_context.item_facts(now, border, false)?;
        let rain_available = matches!(
            facts.rain,
            mc121_rust::runtime_item_world::RainAvailability::NotRaining
        ) || players.rain_context.is_some();
        let drop_context = PlayerDrop {
            player_uuid: player.profile.uuid,
            position: player.position,
            eye_height: player_physics::player_dimensions(player.metadata.pose, 1.)?.eye_height,
            around: false,
            trace_item: false,
        };
        let mut audience = Vec::new();
        let origin = Vec3::new(position.0 as f64, position.1 as f64, position.2 as f64);
        for other in players.registry.snapshot() {
            let other_lifecycle = players
                .player_lifecycle
                .get(&other.profile.uuid)
                .context("anvil level-event audience lifecycle missing")?;
            if other_lifecycle.dimension == binding.dimension
                && Vec3::new(other.position.x, other.position.y, other.position.z)
                    .subtract(origin)
                    .length_squared()
                    < 4096.
            {
                audience.push(other.entity_id);
            }
        }
        let level_recipients = Arc::new(audience);
        let close = matches!(&request, Some(AnvilRequest::Close { .. }));
        let mut prefix = Vec::new();
        if opening.is_some() {
            let title = Tag::Compound(vec![(
                "translate".into(),
                Tag::String("container.repair".into()),
            )]);
            let mut fields = Writer::new();
            fields
                .varint(window as i32)
                .varint(WorkstationKind::Anvil.protocol_id())
                .raw(&title.encode_network_anonymous()?);
            prefix.push(Packet {
                id: 0x33,
                payload: fields.into_inner(),
            });
        } else if matches!(
            &request,
            Some(AnvilRequest::Close {
                send_close: true,
                ..
            })
        ) {
            let mut fields = Writer::new();
            fields.u8(window);
            prefix.push(Packet {
                id: 0x12,
                payload: fields.into_inner(),
            });
        }
        let LivePlayers {
            item_runtime,
            player_random,
            gameplay,
            anvils,
            chunk_runtime,
            chunk_transports,
            ..
        } = players;
        let actual_random = player_random
            .get_mut(&id)
            .context("anvil actual player RNG missing")?;
        let next_player_random = actual_random.clone();
        let ready = Rc::new(RefCell::new(None));
        let ready_prepare = Rc::clone(&ready);
        shared.transact_with_menu_commit(
            |canonical| {
                // Closing transfers only the unchanged native sync baselines.
                // Prepare it before native removal/adoption; publish it after.
                let mut next_window0 = canonical.clone();
                if close {
                    retained
                        .borrow()
                        .as_ref()
                        .expect("retained closing anvil")
                        .menu
                        .transfer_inventory_state(&mut next_window0)?;
                }
                let inventory = canonical.inventory.clone();
                let mut host = NativeAnvilHost::new(
                    binding.clone(),
                    access,
                    inventory,
                    next_player_random,
                    instabuild,
                    experience_level,
                    block,
                    certificate,
                    move |stage: NativeAnvilStage, _token, action| {
                        certificate.validate(gameplay)?;
                        gameplay.validate_active_player(id, player.profile.uuid)?;
                        ensure!(
                            stage.instabuild == instabuild
                                && stage.experience_level == experience_level
                                && stage.block == block,
                            "anvil attempted an unowned XP/ability/block mutation"
                        );
                        ensure!(
                            matches!(action, AnvilMenuAction::Close(_)) == close,
                            "anvil source publication action changed"
                        );
                        preflight_observations(&stage.events)?;
                        main_anvil_context::require_anvil_inventory_context(&stage.inventory)?;
                        let mut next_stats = gameplay.snapshot_player_stats(player.profile.uuid)?;
                        let mut stats_after_event = Vec::with_capacity(stage.events.len());
                        let mut drop_ordinals = vec![None; stage.events.len()];
                        let mut level_packets = BTreeMap::new();
                        let mut drop_order = Vec::new();
                        for (index, event) in stage.events.iter().enumerate() {
                            match event {
                                NativeAnvilEvent::Drop { owned, .. } if !owned.stack.is_empty() => {
                                    drop_ordinals[index] = Some(drop_order.len());
                                    drop_order.push(index);
                                    next_stats.award_drop(DropStatEvent {
                                        constructed: true,
                                        trace_item: owned.trace_item,
                                        returned_item_id: Some(owned.stack.item_id),
                                        original_count: owned.stack.count,
                                    })?;
                                }
                                NativeAnvilEvent::LevelEvent {
                                    event,
                                    data,
                                    position: at,
                                    ..
                                } => {
                                    ensure!(
                                        instabuild
                                            && *event == 1030
                                            && *data == 0
                                            && *at == position,
                                        "anvil requires the captured actual creative LevelEvent"
                                    );
                                    level_packets.insert(
                                        index,
                                        mc121_rust::item_effects::level_event_packet(
                                            *event, *at, *data, false,
                                        ),
                                    );
                                }
                                _ => {}
                            }
                            stats_after_event.push(next_stats.clone());
                        }
                        if !drop_order.is_empty() {
                            ensure!(rain_available, "anvil drop requires actual rain context");
                            chunk_runtime.certify_current_query_region(
                                world,
                                chunk_transports,
                                &self.entity_events,
                                now,
                                (
                                    ((player.position.x - 18.).floor() as i32).div_euclid(16),
                                    ((player.position.z - 18.).floor() as i32).div_euclid(16),
                                ),
                                (
                                    ((player.position.x + 18.).floor() as i32).div_euclid(16),
                                    ((player.position.z + 18.).floor() as i32).div_euclid(16),
                                ),
                            )?;
                        }
                        let mut observations = stage.events;
                        let mut next_player_random = stage.player_random;
                        let mut next_world_random = item_runtime.world_random_mut().clone();
                        let first_id = self.entities.load(Ordering::Relaxed);
                        let next_entity_id = first_id
                            .checked_add(i32::try_from(drop_order.len())?)
                            .context("anvil drop entity IDs exhausted")?;
                        let mut entries = Vec::with_capacity(drop_order.len());
                        for (ordinal, index) in drop_order.into_iter().enumerate() {
                            let NativeAnvilEvent::Drop { owned, .. } = &mut observations[index]
                            else {
                                unreachable!()
                            };
                            let dropped = std::mem::replace(
                                owned,
                                mc121_rust::container::DroppedStack {
                                    stack: ItemStack::empty(),
                                    around: false,
                                    trace_item: false,
                                },
                            );
                            let context = PlayerDrop {
                                around: dropped.around,
                                trace_item: dropped.trace_item,
                                ..drop_context
                            };
                            let entity_id = first_id
                                .checked_add(i32::try_from(ordinal)?)
                                .context("anvil drop ID exhausted")?;
                            let entry = spawn_player_drop_with_factory(
                                entity_id,
                                context,
                                dropped.stack,
                                &mut next_player_random,
                                &mut next_world_random,
                                &mut || random_seed::create_random().map_err(anyhow::Error::from),
                            )?
                            .context("nonempty anvil owned drop did not construct ItemEntity")?;
                            entries.push(entry);
                        }
                        certificate.validate(gameplay)?;
                        let items = item_runtime.prepare_items(entries)?;
                        next_window0.inventory = stage.inventory.clone();
                        // SharedContainer has its old copy for rollback. No live
                        // listener/event has run. UI install after Ok is infallible.
                        *canonical = next_window0;
                        *ready_prepare.borrow_mut() = Some(PreparedAnvilCommit {
                            items,
                            player_random: actual_random,
                            next_player_random,
                            next_world_random,
                            entity_counter: &self.entities,
                            next_entity_id,
                            gameplay,
                            events: observations,
                            final_inventory: stage.inventory,
                            stats_after_event,
                            drop_ordinals,
                            level_packets,
                            level_recipients,
                            prefix,
                        });
                        Ok(())
                    },
                )?;
                if opening.is_some() {
                    let menu = AnvilMenu::new(window, &host)?;
                    *retained.borrow_mut() = Some(OpenAnvil {
                        position,
                        binding,
                        menu,
                    });
                }
                let mut held = retained.borrow_mut();
                let open = held.as_mut().expect("one canonical native anvil");
                match request {
                    None => open.menu.initialize(&mut host)?,
                    Some(AnvilRequest::Broadcast) => open.menu.broadcast_changes(&mut host)?,
                    Some(AnvilRequest::Resync) => open.menu.broadcast_full_state(&mut host)?,
                    Some(AnvilRequest::Rename(name)) => {
                        open.menu
                            .set_item_name(open.menu.token(), name, &mut host)?;
                    }
                    Some(AnvilRequest::Click(click)) => {
                        open.menu.click(open.menu.token(), click, &mut host)?
                    }
                    Some(AnvilRequest::Close { context, .. }) => {
                        open.menu.close(open.menu.token(), context, &mut host)?
                    }
                }
                drop(host);
                let prepared = ready
                    .borrow_mut()
                    .take()
                    .expect("native publication prepared exactly once");
                let open = held
                    .take()
                    .expect("actual native UI retained through adoption");
                Ok(((prepared, open), vec![]))
            },
            |(prepared, open), canonical| {
                // Install the actual current menu BEFORE queuing any source packet
                // or item publication. Close owns and retires this one instance.
                if !close {
                    anvils.menus.insert(id, open);
                }
                prepared.commit(id, player.profile.uuid, &self.entity_events, canonical);
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc121_rust::container_workstation_anvil::AnvilPhase;

    fn phase() -> AnvilPhase {
        AnvilPhase {
            inputs: [ItemStack::empty(), ItemStack::empty()],
            result_preview: ItemStack::empty(),
            carried: ItemStack::empty(),
            level_cost: 0,
            repair_material_count: 0,
        }
    }
    #[test]
    fn actual_player_listener_mapping_and_source_counter_boundary() {
        let mut inventory = PlayerInventory::new();
        let before = inventory.get_times_changed();
        let observation = NativeAnvilEvent::InventoryChanged {
            phase: phase(),
            inventory: inventory.clone(),
        };
        inventory.set_changed();
        assert_eq!(observation.inventory().get_times_changed(), before);
        assert_eq!(inventory.get_times_changed(), before.wrapping_add(1));
        let event = |menu_slot, player_slot| NativeAnvilEvent::Sync {
            event: SyncEvent::SlotListener {
                menu_slot,
                player_slot: Some(player_slot),
                stack: ItemStack::empty(),
            },
            phase: phase(),
            inventory: inventory.clone(),
        };
        assert!(preflight_observations(&[observation, event(3, 9), event(38, 44)]).is_ok());
        assert!(preflight_observations(&[event(3, 10)]).is_err());
        assert!(preflight_observations(&[event(2, 8)]).is_err());
    }
    #[test]
    fn prepared_actual_owner_publication_preserves_window0_cursor_and_phase_counter() -> Result<()>
    {
        use crate::main_item_gameplay::{GameplayObservation, GameplaySourceScope};
        use mc121_rust::{crafting::RecipeBook, runtime_entities::RuntimeEntities};
        let id = 42;
        let uuid = [17; 16];
        let paper = ItemStack::new(RecipeBook::vanilla().item_id("paper").unwrap(), 3)?;
        let retained_cursor = ItemStack::new(RecipeBook::vanilla().item_id("stone").unwrap(), 2)?;
        let mut inventory = PlayerInventory::new().snapshot_with_carried(&retained_cursor);
        inventory.set_creative_slot(9, paper.clone())?;
        let initial_counter = inventory.get_times_changed();
        let before_change = inventory.clone();
        inventory.set_changed();
        let observations = vec![
            NativeAnvilEvent::InventoryChanged {
                phase: phase(),
                inventory: before_change,
            },
            NativeAnvilEvent::Sync {
                event: SyncEvent::SlotListener {
                    menu_slot: 3,
                    player_slot: Some(9),
                    stack: paper.clone(),
                },
                phase: phase(),
                inventory: inventory.clone(),
            },
        ];
        preflight_observations(&observations)?;
        let mut gameplay = MainItemGameplay::new(GameplaySourceScope::NativeFixtureEmptyCriteria);
        gameplay.register_player(id, uuid)?;
        gameplay.validate_active_player(id, uuid)?;
        let stats = gameplay.snapshot_player_stats(uuid)?;
        let mut runtime = RuntimeEntities::new(LegacyRandomSource::new(5));
        let expected_world_random = runtime.world_random_state48();
        let next_world_random = runtime.world_random_mut().clone();
        let mut player_random = LegacyRandomSource::new(6);
        let expected_player_random = player_random.state48();
        let next_player_random = player_random.clone();
        let entity_counter = AtomicI32::new(97);
        let prepared = PreparedAnvilCommit {
            items: runtime.prepare_items(vec![])?,
            player_random: &mut player_random,
            next_player_random,
            next_world_random,
            entity_counter: &entity_counter,
            next_entity_id: 97,
            gameplay: &mut gameplay,
            events: observations,
            final_inventory: inventory.clone(),
            stats_after_event: vec![stats.clone(), stats],
            drop_ordinals: vec![None, None],
            level_packets: BTreeMap::new(),
            level_recipients: Arc::new(vec![]),
            prefix: vec![],
        };
        let (events, mut receiver) = broadcast::channel(16);
        let mut canonical = PlayerContainer::from_inventory(
            PlayerInventory::new().snapshot_with_carried(&retained_cursor),
        );
        prepared.commit(id, uuid, &events, &mut canonical);
        assert_eq!(canonical.inventory.slot(9), Some(&paper));
        assert_eq!(canonical.inventory.carried(), &retained_cursor);
        assert_eq!(
            canonical.inventory.get_times_changed(),
            initial_counter.wrapping_add(1)
        );
        assert_eq!(entity_counter.load(Ordering::Relaxed), 97);
        assert_eq!(player_random.state48(), expected_player_random);
        assert_eq!(runtime.world_random_state48(), expected_world_random);
        assert!(receiver.try_recv().is_err());
        assert!(gameplay
            .drain_observations()
            .into_iter()
            .any(|observation| matches!(
                observation,GameplayObservation::InventoryScan {player,counts}
                    if player == uuid && counts.occupied == 1 && counts.full == 0
            )));
        Ok(())
    }
}
