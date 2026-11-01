//! Live native furnace owners, menus and block-entity tick composition.
//! World -> players -> shared inventory is the lock order. Output XP and recipe
//! packets precede menu publication. Combined output-XP plus item-drop actions
//! remain an explicit unsupported boundary until their shared RNG seam exists.
use super::*;
use mc121_rust::{
    container_furnace::FurnaceMenu,
    furnace::{FurnaceKind, FurnaceState},
    inventory::PlayerInventory,
    native_block_entities::{
        self, FurnaceImage, FurnaceOwnerKey, FurnaceRemovalSnapshot, FurnaceViewerRetention,
    },
    server_recipe_book::{RecipeBookHost, RecipeBookPacket, RecipeDescriptor, RecipeId},
};
use std::cell::RefCell;
type P = (i32, i32, i32);
#[derive(Clone, Debug)]
struct OpenFurnace {
    position: P,
    owner_key: FurnaceOwnerKey,
    menu: FurnaceMenu,
    spectator: bool,
}
#[derive(Default)]
pub(super) struct FurnaceRuntime {
    menus: BTreeMap<i32, OpenFurnace>,
    retired: BTreeMap<FurnaceOwnerKey, RetiredFurnace>,
}
#[derive(Clone, Debug)]
struct RetiredFurnace {
    position: P,
    state: FurnaceState,
    viewers: BTreeSet<i32>,
}
pub(super) struct PreparedFurnaceMenuRetirement {
    owner_key: FurnaceOwnerKey,
    retired: RetiredFurnace,
}
impl FurnaceRuntime {
    /// Existing menus retain the removed BE object, not a new BE at its position.
    pub(super) fn prepare_removal_retirement(
        &self,
        snapshot: &FurnaceRemovalSnapshot,
        retention: &FurnaceViewerRetention,
    ) -> Result<PreparedFurnaceMenuRetirement> {
        let owner_key = snapshot.owner_key();
        ensure!(
            retention.owner_key() == owner_key,
            "retained furnace identity differs"
        );
        ensure!(
            retention.image().position == snapshot.image().position,
            "retained furnace position differs"
        );
        ensure!(
            !self.retired.contains_key(&owner_key),
            "furnace owner already retired"
        );
        let viewers = self
            .menus
            .iter()
            .filter_map(|(&id, open)| (open.owner_key == owner_key).then_some(id))
            .collect::<Vec<_>>();
        ensure!(
            viewers.as_slice() == snapshot.viewers() && viewers.as_slice() == retention.viewers(),
            "furnace menu/viewer ownership differs"
        );
        ensure!(
            self.menus
                .values()
                .filter(|open| open.owner_key == owner_key)
                .all(|open| open.position == snapshot.image().position),
            "furnace menu position differs from actual owner"
        );
        let state = retention.image().furnace_state.clone();
        state.validate()?;
        ensure!(
            state.slots.iter().all(ItemStack::is_empty),
            "retired furnace still owns dropped contents"
        );
        Ok(PreparedFurnaceMenuRetirement {
            owner_key,
            retired: RetiredFurnace {
                position: snapshot.image().position,
                state,
                viewers: viewers.into_iter().collect(),
            },
        })
    }
    /// Called only after full preflight while the world/player locks remain held.
    pub(super) fn commit_removal_retirement(&mut self, prepared: PreparedFurnaceMenuRetirement) {
        if !prepared.retired.viewers.is_empty() {
            assert!(
                self.retired
                    .insert(prepared.owner_key, prepared.retired)
                    .is_none(),
                "exclusive prepared furnace retirement"
            );
        }
    }
    pub(super) fn viewer_ids_for_owner(&self, owner_key: FurnaceOwnerKey) -> Vec<i32> {
        self.menus
            .iter()
            .filter_map(|(&id, open)| (open.owner_key == owner_key).then_some(id))
            .collect()
    }
    pub(super) fn retired_owner_count(&self) -> usize {
        self.retired.len()
    }
    /// ServerPlayer.take synchronizes its current container immediately. This
    /// uses the captured live/retired furnace owner without moving the later
    /// level-phase stillValid/close check into the pickup callback.
    pub(super) fn broadcast_for_inventory(
        &mut self,
        world: &WorldBackend,
        id: i32,
        inventory: &PlayerInventory,
    ) -> Result<Vec<Packet>> {
        let open = self
            .menus
            .get(&id)
            .cloned()
            .context("current furnace menu absent")?;
        let engine = open_state(world, self, &open)?;
        self.menus
            .get_mut(&id)
            .expect("exclusive current furnace menu")
            .menu
            .broadcast_changes(&engine, inventory)
    }

    pub fn window_id(&self, id: i32) -> Option<u8> {
        self.menus.get(&id).map(|v| v.menu.window_id)
    }
    pub fn has_open(&self, id: i32) -> bool {
        self.menus.contains_key(&id)
    }
    pub fn pending_players(&self) -> Vec<i32> {
        self.menus.keys().copied().collect()
    }
    pub fn inventory_snapshot(&self, id: i32, inventory: &PlayerInventory) -> PlayerInventory {
        self.menus.get(&id).map_or_else(
            || inventory.clone(),
            |v| v.menu.inventory_snapshot(inventory),
        )
    }
}
fn send(events: &broadcast::Sender<PlayerEvent>, id: i32, packets: Vec<Packet>) {
    if !packets.is_empty() {
        let _ = events.send(PlayerEvent::WorldAudience {
            recipients: Arc::new(vec![id]),
            packets: Arc::new(packets),
        });
    }
}
fn still_valid(
    world: &WorldBackend,
    player: &PlayerState,
    open: &OpenFurnace,
    furnaces: &FurnaceRuntime,
) -> Result<bool> {
    if furnaces.retired.contains_key(&open.owner_key) {
        return Ok(false);
    }
    let p = open.position;
    let WorldBackend::Fixture(flat) = &*world else {
        return Ok(false);
    };
    if flat.native_block_entities().furnace_owner_key(p).ok() != Some(open.owner_key)
        || flat
            .native_block_entities()
            .furnace_validate_at(p, flat.block(p.0, p.1, p.2))
            .is_err()
    {
        return Ok(false);
    }
    let eye = player_physics::player_dimensions(player.metadata.pose, 1.)?.eye_height;
    let eyes = geometry::player_eye_position(
        Vec3::new(player.position.x, player.position.y, player.position.z),
        eye,
    );
    let range = if player.gamemode == 1 {
        geometry::CREATIVE_BLOCK_INTERACTION_RANGE
    } else {
        geometry::DEFAULT_BLOCK_INTERACTION_RANGE
    };
    Ok(geometry::can_interact_with_block(eyes, p, range, 4.))
}
fn images(world: &WorldBackend) -> Result<Vec<FurnaceImage>> {
    let WorldBackend::Fixture(flat) = world else {
        bail!("native furnace requires fixture backend")
    };
    Ok(flat.native_block_entities().furnace_images())
}
fn state(world: &WorldBackend, p: P) -> Result<FurnaceState> {
    let WorldBackend::Fixture(flat) = world else {
        bail!("native furnace requires fixture backend")
    };
    Ok(flat
        .native_block_entities()
        .furnace_entry(p)?
        .image
        .furnace_state
        .clone())
}
fn open_state(
    world: &WorldBackend,
    furnaces: &FurnaceRuntime,
    open: &OpenFurnace,
) -> Result<FurnaceState> {
    if let Some(retired) = furnaces.retired.get(&open.owner_key) {
        ensure!(
            retired.position == open.position,
            "retired furnace position differs"
        );
        return Ok(retired.state.clone());
    }
    let WorldBackend::Fixture(flat) = world else {
        bail!("native furnace backend changed")
    };
    ensure!(
        flat.native_block_entities()
            .furnace_owner_key(open.position)?
            == open.owner_key,
        "open furnace no longer references its actual owner"
    );
    state(world, open.position)
}
fn write_state(world: &mut WorldBackend, p: P, next: FurnaceState) -> Result<()> {
    let WorldBackend::Fixture(flat) = world else {
        bail!("native furnace backend changed")
    };
    flat.native_block_entities_mut().furnace_set_state(p, next)
}
fn observe(players: &mut LivePlayers, world: &WorldBackend, before: &[FurnaceImage]) -> Result<()> {
    players
        .entity_source
        .as_mut()
        .context("native furnace source absent")?
        .observe_owned_furnace_change(world, before)
}
/// Constructed only after the real MainItemGameplay registry certifies the
/// fresh native empty-criteria profile. This is not a default advancement host.
struct NativeRecipeHost {
    empty_criteria_proved: bool,
    packets: Vec<Packet>,
}
impl RecipeBookHost for NativeRecipeHost {
    fn recipe_unlocked(
        &mut self,
        _book: &mut ServerRecipeBook,
        _recipe: &RecipeDescriptor,
    ) -> Result<()> {
        ensure!(
            self.empty_criteria_proved,
            "unowned recipe-unlocked criterion"
        );
        Ok(())
    }
    fn send_packet(
        &mut self,
        _book: &mut ServerRecipeBook,
        packet: RecipeBookPacket,
    ) -> Result<()> {
        self.packets.push(packet.encode()?);
        Ok(())
    }
}
impl Server {
    pub(super) async fn use_native_furnace(&self, id: i32, hand: i32, target: P) -> Result<bool> {
        let now = self
            .clock
            .read()
            .map_err(|_| anyhow!("clock poisoned"))?
            .game_time;
        let border = self.native_container_border()?;
        let mut world = self.world.write().await;
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("players poisoned"))?;
        if self.shutting_down.load(Ordering::Acquire) {
            return Ok(false);
        }
        let Some(player) = players.registry.get(id).cloned() else {
            return Ok(false);
        };
        let Some(kind) =
            native_block_entities::furnace_kind(world.block(target.0, target.1, target.2)?)
        else {
            return Ok(false);
        };
        ensure!((0..=1).contains(&hand), "invalid furnace interaction hand");
        if !can_reach_block(player.position, player.metadata, target) {
            return Ok(true);
        }
        let shared = players
            .containers
            .get(&id)
            .context("furnace player inventory absent")?
            .clone();
        let (empty, held) = shared.read(|m| -> Result<_> {
            Ok((
                m.inventory.selected_stack(0)?.is_empty()
                    && m.inventory.selected_stack(1)?.is_empty(),
                m.inventory.selected_stack(0)?.clone(),
            ))
        })??;
        if player.metadata.flags & 2 != 0 && !empty {
            return Ok(false);
        }
        players.require_current_native_source(&world)?;
        players.gameplay.require_supported_floor()?;
        let title = {
            let WorldBackend::Fixture(flat) = &*world else {
                bail!("furnace backend unowned")
            };
            flat.native_block_entities()
                .furnace_validate_at(target, flat.block(target.0, target.1, target.2))?;
            if !flat.native_block_entities().furnace_can_unlock(
                target,
                &held,
                player.gamemode == 3,
            )? {
                return Ok(true);
            }
            flat.native_block_entities().furnace_title(target)?
        };
        if let Some(window) = players.furnaces.window_id(id) {
            self.furnace_menu_action_locked(
                &mut world,
                &mut players,
                id,
                None,
                Some(window),
                true,
                now,
                border,
            )?;
        }
        if let Some(window) = players.crafting_tables.window_id(id) {
            self.crafting_table_action_locked(
                &mut world,
                &mut players,
                id,
                None,
                Some(window),
                true,
                now,
                border,
            )?;
        }
        if let Some(window) = players.native_containers.window_id(id) {
            self.native_menu_action_locked(
                &mut world,
                &mut players,
                id,
                None,
                Some(window),
                true,
                now,
                border,
            )?;
        }
        self.close_inventory_menu_locked(&mut world, &mut players, id, now, border)?;
        let window = players.native_containers.allocate_window(id);
        let mut menu = FurnaceMenu::new(window)?;
        let engine = state(&world, target)?;
        let initial = shared.read(|m| menu.full_sync(&engine, &m.inventory))??;
        let stat_name = match kind {
            FurnaceKind::Furnace => "minecraft:interact_with_furnace",
            FurnaceKind::BlastFurnace => "minecraft:interact_with_blast_furnace",
            FurnaceKind::Smoker => "minecraft:interact_with_smoker",
        };
        let stat = mc121_rust::player_stats::stat_key("minecraft:custom", stat_name)
            .context("furnace interaction statistic absent")?;
        players
            .gameplay
            .stats_mut(player.profile.uuid)?
            .increment(stat, 1)?;
        let WorldBackend::Fixture(flat) = &mut *world else {
            unreachable!()
        };
        let owner_key = flat.native_block_entities().furnace_owner_key(target)?;
        flat.native_block_entities_mut()
            .furnace_start_open(target, id)?;
        players.furnaces.menus.insert(
            id,
            OpenFurnace {
                position: target,
                owner_key,
                menu,
                spectator: player.gamemode == 3,
            },
        );
        let mut w = Writer::new();
        w.varint(window as i32)
            .varint(FurnaceMenu::menu_type(kind))
            .raw(&title.encode_network_anonymous()?);
        let mut packets = vec![Packet {
            id: 0x33,
            payload: w.into_inner(),
        }];
        packets.extend(initial);
        send(&self.entity_events, id, packets);
        Ok(true)
    }
    pub(super) async fn click_native_furnace(
        &self,
        id: i32,
        click: &ContainerClick,
    ) -> Result<bool> {
        self.furnace_menu_action(id, Some(click), None, false).await
    }
    pub(super) async fn close_native_furnace(
        &self,
        id: i32,
        window: u8,
        send_close: bool,
    ) -> Result<bool> {
        self.furnace_menu_action(id, None, Some(window), send_close)
            .await
    }
    async fn furnace_menu_action(
        &self,
        id: i32,
        click: Option<&ContainerClick>,
        close: Option<u8>,
        send_close: bool,
    ) -> Result<bool> {
        let now = self
            .clock
            .read()
            .map_err(|_| anyhow!("clock poisoned"))?
            .game_time;
        let border = self.native_container_border()?;
        let mut world = self.world.write().await;
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("players poisoned"))?;
        self.furnace_menu_action_locked(
            &mut world,
            &mut players,
            id,
            click,
            close,
            send_close,
            now,
            border,
        )
    }
    pub(super) fn furnace_menu_action_locked(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        id: i32,
        click: Option<&ContainerClick>,
        close: Option<u8>,
        send_close: bool,
        now: i64,
        border: CollisionBorder,
    ) -> Result<bool> {
        let Some(open) = players.furnaces.menus.get(&id).cloned() else {
            return Ok(false);
        };
        // handleContainerClick ignores an ID mismatch; handleContainerClose
        // invokes doCloseContainer for the actual menu regardless of packet ID.
        if click.is_some_and(|c| c.window_id != open.menu.window_id) {
            return Ok(true);
        }
        let player = players
            .registry
            .get(id)
            .context("furnace actor absent")?
            .clone();
        // Spectator full-sync precedes stillValid in handleContainerClick.
        // An invalid non-spectator click is ignored; the Level player tick owns
        // broadcastChanges followed by the eventual closeContainer call.
        if click.is_some()
            && !open.spectator
            && !still_valid(world, &player, &open, &players.furnaces)?
        {
            return Ok(true);
        }
        players.require_current_native_source(world)?;
        players.gameplay.require_supported_floor()?;
        let shared = players
            .containers
            .get(&id)
            .context("furnace shared inventory absent")?
            .clone();
        let baseline = shared.read(|m| m.inventory.clone())?;
        let detached = players.furnaces.retired.contains_key(&open.owner_key);
        let old_state = open_state(world, &players.furnaces, &open)?;
        let before = images(world)?;
        let mut next_inventory = baseline.clone();
        let mut next_state = old_state.clone();
        let mut next_open = open.clone();
        let outcome = if close.is_some() {
            next_open.menu.close(&mut next_inventory)?
        } else if open.spectator {
            ClickOutcome {
                packets: next_open.menu.full_sync(&next_state, &next_inventory)?,
                ..Default::default()
            }
        } else {
            next_open.menu.click(
                click.context("furnace click absent")?,
                &mut next_state,
                &mut next_inventory,
                player.gamemode == 1,
            )?
        };
        let output_events = next_open.menu.take_output_events();
        ensure!(
            output_events.is_empty() || outcome.drops.iter().all(|d| d.stack.is_empty()),
            "furnace output XP plus item drop requires the combined native RNG transaction"
        );
        if !detached && next_state.slots != old_state.slots {
            self.notify_native_furnace_changed_locked(world, players, open.position)?;
        }
        let mut rewards = Vec::new();
        let mut final_reward_state = next_state.clone();
        for _event in &output_events {
            rewards.extend(final_reward_state.pending_recipe_rewards()?);
            final_reward_state.commit_recipe_rewards()?;
        }
        let xp = if output_events.is_empty() {
            None
        } else {
            Some(main_experience::prepare_furnace_experience(
                world,
                players,
                id,
                &rewards,
                &self.entities,
            )?)
        };
        let mut book = players
            .recipe_books
            .get(&player.profile.uuid)
            .context("furnace recipe book absent")?
            .clone();
        let mut recipe_host = NativeRecipeHost {
            empty_criteria_proved: true,
            packets: vec![],
        };
        let descriptors = rewards
            .iter()
            .map(|r| {
                let key = RecipeId::parse(&r.recipe_id)?;
                VanillaRecipeCatalogue::get()
                    .descriptor(&key)
                    .context("actual furnace recipe descriptor absent")
            })
            .collect::<Result<Vec<_>>>()?;
        book.add_recipes(&descriptors, &mut recipe_host)?;
        // triggerRecipeCrafted has no listeners in the certified native empty-
        // criteria registry. Each callback's actual inventory is nevertheless
        // retained by the menu, and unsupported component decoding fails now.
        for event in &output_events {
            for stack in &event.slots {
                stack.encode_canonical()?;
            }
        }
        final_reward_state.validate()?;
        let reward_clear = !output_events.is_empty();
        let stats = players
            .gameplay
            .snapshot_player_stats(player.profile.uuid)?;
        let facts = players.world_context.item_facts(now, border, false)?;
        let rain_available = matches!(
            facts.rain,
            mc121_rust::runtime_item_world::RainAvailability::NotRaining
        ) || players.rain_context.is_some();
        let drop = PlayerDrop {
            player_uuid: player.profile.uuid,
            position: player.position,
            eye_height: player_physics::player_dimensions(player.metadata.pose, 1.)?.eye_height,
            around: false,
            trace_item: false,
        };
        let close_packet = if send_close && close.is_some() {
            let mut w = Writer::new();
            w.u8(open.menu.window_id);
            Some(Packet {
                id: 0x12,
                payload: w.into_inner(),
            })
        } else {
            None
        };
        let staged = RefCell::new((next_open, next_state));
        let world_slot = RefCell::new(world);
        let packets = RefCell::new(Vec::new());
        {
            let LivePlayers {
                item_runtime,
                player_random,
                gameplay,
                chunk_runtime,
                chunk_transports,
                furnaces,
                entity_source,
                ..
            } = players;
            menu_action_with_items(
                &shared,
                DropCommitOwner {
                    runtime: item_runtime,
                    player_random: player_random
                        .get_mut(&id)
                        .context("furnace player RNG absent")?,
                    stats,
                    entity_counter: &self.entities,
                },
                drop,
                |m| {
                    ensure!(
                        m.inventory.slots() == baseline.slots()
                            && m.inventory.carried() == baseline.carried()
                            && m.inventory.selected_hotbar() == baseline.selected_hotbar(),
                        "furnace inventory changed during staged preflight"
                    );
                    m.inventory = next_inventory;
                    let mut outcome = outcome;
                    if close.is_some() {
                        // doCloseContainer transfers the old menu's shared
                        // local/remote slot baselines, without broadcasting the
                        // inventory menu now. Raw Inventory return packets are
                        // already in the removed callback's outcome.
                        staged.borrow().0.menu.transfer_inventory_remote_state(m)?;
                    } else {
                        let _ = m.broadcast_changes()?;
                    }
                    *packets.borrow_mut() = std::mem::take(&mut outcome.packets);
                    Ok(outcome)
                },
                |_| {
                    ensure!(rain_available, "furnace drop rain owner absent");
                    // A pending player-ticket update does not invalidate the
                    // actual current FULL/light owners under these locks.
                    // Closing a menu must use those owners without requiring
                    // unrelated future scheduler work to be globally idle.
                    chunk_runtime.certify_current_query_region(
                        &mut world_slot.borrow_mut(),
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
                    ).map(|_| ())
                },
                || random_seed::create_random().map_err(anyhow::Error::from),
                DropCommitCallbacks {
                    prepare: |m: &mut PlayerContainer, mut next_stats: PlayerStats| {
                        let gameplay = gameplay;
                        for event in &output_events {
                            if event.amount > 0 && !event.stack.is_empty() {
                                next_stats.increment(
                                    mc121_rust::player_stats::StatKey::new(1, event.stack.item_id)?,
                                    event.amount,
                                )?;
                            }
                        }
                        let changes = m.take_local_slot_changes();
                        gameplay.prepare_menu_commit(
                            player.profile.uuid,
                            &m.inventory,
                            changes,
                            next_stats,
                        )
                    },
                    before_spawn: || {
                        if let Some(packet) = &close_packet {
                            send(&self.entity_events, id, vec![packet.clone()]);
                        }
                        let s = staged.borrow();
                        if detached {
                            let remove_owner = {
                                let retired = furnaces
                                    .retired
                                    .get_mut(&s.0.owner_key)
                                    .expect("exclusive retained furnace owner");
                                retired.state = s.1.clone();
                                if close.is_some() {
                                    retired.viewers.remove(&id);
                                }
                                retired.viewers.is_empty()
                            };
                            if close.is_some() {
                                furnaces.menus.remove(&id);
                            } else {
                                furnaces.menus.insert(id, s.0.clone());
                            }
                            if remove_owner {
                                furnaces.retired.remove(&s.0.owner_key);
                            }
                        } else {
                            let mut w = world_slot.borrow_mut();
                            let WorldBackend::Fixture(flat) = &mut **w else {
                                unreachable!()
                            };
                            flat.native_block_entities_mut()
                                .furnace_set_state(s.0.position, s.1.clone())
                                .expect("prevalidated furnace state");
                            if close.is_some() {
                                flat.native_block_entities_mut()
                                    .furnace_stop_open(s.0.position, id)
                                    .expect("actual furnace viewer owner");
                                furnaces.menus.remove(&id);
                            } else {
                                furnaces.menus.insert(id, s.0.clone());
                            }
                            entity_source
                                .as_mut()
                                .expect("validated furnace source")
                                .observe_owned_furnace_change(&w, &before)
                                .expect("exclusive owned furnace mutation");
                        }
                    },
                    commit: |p: main_item_gameplay::PreparedGameplayMenu<'_>| p.commit(),
                },
            )?;
        }
        let (open, _) = staged.into_inner();
        let world = world_slot.into_inner();
        // The supported output path cannot also drop items, so the empty item
        // batch preserved the exact XP token's source/index and RNG baselines.
        if let Some(xp) = xp {
            xp.commit(players, &self.entity_events)
                .expect("prevalidated XP commit under exclusive world/player locks");
        }
        if reward_clear {
            players.recipe_books.insert(player.profile.uuid, book);
            send(&self.entity_events, id, recipe_host.packets);
            let before = images(world).expect("exclusive fixture furnace backend");
            write_state(world, open.position, final_reward_state)
                .expect("prevalidated recipe reward clear");
            observe(players, world, &before).expect("exclusive owned furnace reward clear");
        }
        send(&self.entity_events, id, packets.into_inner());
        Ok(true)
    }
    /// Exact ServerPlayer.tick menu phase in the shared level-entity pass:
    /// broadcast the captured container first, then test identity/range and
    /// close. A newly placed BE at the same coordinates is a different owner.
    pub(super) fn tick_native_furnace_menu_for_player_locked(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        id: i32,
        now: i64,
        border: CollisionBorder,
    ) -> Result<()> {
        let Some(open) = players.furnaces.menus.get(&id).cloned() else {
            return Ok(());
        };
        let shared = players
            .containers
            .get(&id)
            .context("furnace shared inventory absent")?
            .clone();
        let engine = open_state(world, &players.furnaces, &open)?;
        let packets = {
            let actual = players
                .furnaces
                .menus
                .get_mut(&id)
                .expect("actual furnace menu");
            shared.read(|m| actual.menu.broadcast_changes(&engine, &m.inventory))??
        };
        send(&self.entity_events, id, packets);
        let player = players
            .registry
            .get(id)
            .context("orphan furnace menu")?
            .clone();
        if !still_valid(world, &player, &open, &players.furnaces)? {
            self.furnace_menu_action_locked(
                world,
                players,
                id,
                None,
                Some(open.menu.window_id),
                true,
                now,
                border,
            )?;
        }
        Ok(())
    }
    pub(super) fn close_furnaces_for_save_locked(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        now: i64,
    ) -> Result<()> {
        let border = self.native_container_border()?;
        for id in players.furnaces.pending_players() {
            let window = players.furnaces.window_id(id).unwrap();
            self.furnace_menu_action_locked(
                world,
                players,
                id,
                None,
                Some(window),
                true,
                now,
                border,
            )?;
        }
        Ok(())
    }
    pub(super) async fn tick_native_furnaces(&self) -> Result<()> {
        let now = self
            .clock
            .read()
            .map_err(|_| anyhow!("clock poisoned"))?
            .game_time;
        let border = self.native_container_border()?;
        let mut world = self.world.write().await;
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("players poisoned"))?;
        if self.shutting_down.load(Ordering::Acquire) {
            return Ok(());
        }
        let positions = {
            let WorldBackend::Fixture(flat) = &*world else {
                return Ok(());
            };
            flat.native_block_entities().furnace_positions()
        };
        for p in positions {
            if !players
                .chunk_runtime
                .manager
                .current_block_ticking_chunk_ready((p.0.div_euclid(16), p.2.div_euclid(16)))?
            {
                continue;
            }
            players.require_current_native_source(&world)?;
            let before = images(&world)?;
            let current = state(&world, p)?;
            let mut next = current.clone();
            let tick = next.tick()?;
            if !tick.changed {
                continue;
            }
            if tick.block_entity_changed {
                self.notify_native_furnace_changed_locked(&mut world, &mut players, p)?;
            }
            let lit_plan = if let Some(lit) = tick.lit_changed {
                let old = world.block(p.0, p.1, p.2)?;
                let new = inventory::block_state_with_properties(
                    old,
                    &BTreeMap::from([("lit".into(), lit.to_string())]),
                )?;
                let mut query = mc121_rust::world_interactions::RuntimeInteractionWorld::new(
                    &mut *world,
                    world::MIN_Y,
                    world::MAX_Y + 1,
                    border,
                    |_, _| Ok(true),
                )?;
                let plan = mc121_rust::world_interactions::plan_edit(p, new, 3, &mut query)?;
                players.prepare_required_native_source_edit(&world, &plan)?;
                players.world_interactions.can_append(plan.events.len())?;
                Some(plan)
            } else {
                None
            };
            write_state(&mut world, p, next)?;
            observe(&mut players, &world, &before)?;
            if let Some(plan) = lit_plan {
                ensure!(
                    self.commit_world_interaction(&mut world, &mut players, -1, plan)?
                        == main_world_interactions::MainInteractionResult::Applied,
                    "furnace lit state rejected"
                );
                let report = players.drain_world_interactions(
                    &mut world,
                    now,
                    border,
                    true,
                    &self.entity_events,
                    4096,
                );
                ensure!(
                    report.blocked.is_none(),
                    "furnace lit callback blocked: {:?}",
                    report.blocked
                );
            }
        }
        Ok(())
    }
}
