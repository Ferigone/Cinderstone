//! Immediate ServerPlayer.take synchronization of the retained AnvilMenu.
//!
//! Admission runs before ItemEntity.playerTouch can commit Inventory.add. The
//! subsequent take callback borrows that same canonical inventory; neither
//! callback substitutes a fresh menu, input preview, cursor or world owner.
use super::*;
use main_anvil::NativeAnvilRuntime;
use main_anvil_context::{require_anvil_inventory_context, require_anvil_stack_context};
use main_item_gameplay::MainGameplayFrame;
use mc121_rust::{
    container::LocalSlotChange,
    container_workstation_anvil::AnvilPhase,
    inventory::PlayerInventory,
    item_host::RuntimeGameplayHooks,
    runtime_entities::PickupPlayer,
    workstations::menu::{SyncEvent, WorkstationKind},
};

impl NativeAnvilRuntime {
    /// Actual immediate ServerPlayer.take callback, after canonical pickup and
    /// Collect, before the existing item host publishes its private menu batch.
    /// Its later source validity/close phase remains independent.
    pub(super) fn broadcast_for_inventory(
        &mut self,
        id: i32,
        player: PickupPlayer,
        inventory: &PlayerInventory,
        hooks: &mut MainGameplayFrame<'_>,
    ) -> Result<Vec<Packet>> {
        let open = self
            .menus
            .get_mut(&id)
            .context("current anvil menu absent during item pickup")?;
        ensure!(
            id == player.entity_id
                && open.binding.player_id == id
                && open.binding.player_uuid == player.uuid
                && open.binding.position == open.position
                && open.binding.window_id == open.menu.window_id()
                && open.binding.dimension == "minecraft:overworld"
                && player.alive
                && !player.spectator
                && player.vehicle_bounds.is_none(),
            "anvil pickup actual player/menu source profile changed"
        );

        // This core plan borrows this exact inventory and privately prepares
        // only the retained MenuSync. It cannot recompute a result or invoke
        // invented world, item, XP or RNG owners. All encodings/queries and
        // real empty-criteria callback checks finish before any observation.
        let plan = open.menu.prepare_inventory_broadcast(inventory)?;
        let (packets, local_changes) =
            prepare_pickup_sync(plan.events(), plan.phase(), plan.inventory())?;
        hooks.preflight_inventory_slot_changes(player, inventory, &local_changes)?;

        // Validation consumes the plan and exclusively borrows this same UI
        // through actual callbacks and adoption. No fallible publication or
        // baseline check remains after the first real listener observation.
        let validated = plan.into_validated(&mut open.menu)?;
        for change in &local_changes {
            hooks
                .inventory_slot_changed(player, change.menu_slot, inventory, &change.stack)
                .expect("preflighted actual native empty-criteria pickup listener");
        }
        let _ = validated.commit();
        Ok(packets)
    }
}

/// Admit one actual item-pickup candidate before the canonical Inventory.add
/// mutation. Root calls this only while this player's actual anvil is active,
/// under the existing operation/Level/player owner locks. A refusal must leave
/// both the item entity and inventory untouched. Recovery, when necessary,
/// uses Root's retained-current-menu selector.
///
/// Immediate ServerPlayer.take has no block-validity or reach check. Its later
/// ServerPlayer.tick owns broadcast -> stillValid -> close; a removed anvil
/// must not turn this callback into a window0 synchronization.
pub(super) fn preflight_anvil_pickup(
    players: &LivePlayers,
    id: i32,
    candidate: &ItemStack,
) -> Result<()> {
    let open = players
        .anvils
        .menus
        .get(&id)
        .context("current anvil menu absent before item pickup")?;
    let player = players
        .registry
        .get(id)
        .context("anvil pickup actor missing")?;
    ensure!(
        open.binding.player_id == id
            && open.binding.player_uuid == player.profile.uuid
            && open.binding.position == open.position
            && open.binding.window_id == open.menu.window_id(),
        "anvil pickup captured actor/menu binding changed"
    );
    ensure!(
        matches!(player.entity_data.get(9), Some(MetadataValue::Float(health))
            if health.is_finite() && *health > 0.),
        "anvil pickup requires its actual alive player"
    );
    let lifecycle = players
        .player_lifecycle
        .get(&player.profile.uuid)
        .context("anvil pickup actual lifecycle missing")?;
    main_player_motion_hooks::require_creative(player, lifecycle)?;
    ensure!(
        open.binding.dimension == lifecycle.dimension,
        "anvil pickup captured dimension changed"
    );
    let body = players
        .player_motion_runtime
        .physical(id)
        .context("anvil pickup actual native body missing")?;
    ensure!(
        body.entity_id == id
            && body.uuid == player.profile.uuid
            && !body.removed
            && body.vehicle.is_none()
            && players.native_body_ready.contains(&id),
        "anvil pickup requires its admitted unmounted native body"
    );
    players
        .gameplay
        .validate_active_player(id, player.profile.uuid)?;
    let canonical = players
        .containers
        .get(&id)
        .context("anvil pickup canonical inventory missing")?;
    canonical.read(|menu| -> Result<()> {
        require_pickup_components(&menu.inventory, &open.menu.phase(), candidate)?;
        preflight_inventory_scan(&menu.inventory)?;
        if !candidate.is_empty() {
            candidate.max_stack_size()?;
        }
        // Validate the actual active UI and its retained hidden baselines as
        // well. This disposable read-only plan performs no source callback or
        // publication; it cannot revive a closed menu or consume a revision.
        let plan = open.menu.prepare_inventory_broadcast(&menu.inventory)?;
        plan.validate(&open.menu)?;
        Ok(())
    })??;
    Ok(())
}

fn require_pickup_components(
    inventory: &PlayerInventory,
    phase: &AnvilPhase,
    candidate: &ItemStack,
) -> Result<()> {
    require_anvil_inventory_context(inventory)?;
    for stack in phase
        .inputs
        .iter()
        .chain(std::iter::once(&phase.result_preview))
        .chain(std::iter::once(&phase.carried))
        .chain(std::iter::once(candidate))
    {
        require_anvil_stack_context(stack)?;
    }
    Ok(())
}

/// Finish fallible packet/listener preparation against the actual source
/// observation. The packet offsets retain each local listener's place in the
/// private ordered publication. A slot listener is not Inventory.setChanged.
fn prepare_pickup_sync(
    events: &[SyncEvent],
    phase: &AnvilPhase,
    inventory: &PlayerInventory,
) -> Result<(Vec<Packet>, Vec<LocalSlotChange>)> {
    require_pickup_components(inventory, phase, &ItemStack::empty())?;
    let mut packets = Vec::new();
    let mut local_changes = Vec::new();
    for event in events {
        match event {
            SyncEvent::Packet(packet) => packets.push(packet.clone()),
            SyncEvent::DataListener { index, value } => ensure!(
                *index == 0 && *value == phase.level_cost,
                "anvil pickup data listener differs from its retained cost"
            ),
            SyncEvent::SlotListener {
                menu_slot,
                player_slot,
                stack,
            } => {
                require_anvil_stack_context(stack)?;
                if let Some(slot) = player_slot {
                    ensure!(
                        WorkstationKind::Anvil.player_inventory_slot(*menu_slot) == Some(*slot),
                        "anvil pickup listener does not refer to the canonical inventory"
                    );
                    ensure!(
                        matches_pickup_observation(
                            inventory
                                .slot(*slot)
                                .context("anvil pickup inventory slot absent")?,
                            stack,
                        )?,
                        "anvil pickup listener lost its actual inventory phase"
                    );
                    local_changes.push(LocalSlotChange {
                        menu_slot: *slot,
                        inventory_slot: Some(if *slot < 36 { *slot } else { *slot - 36 }),
                        stack: stack.clone(),
                        packet_offset: packets.len(),
                    });
                } else {
                    let actual = match *menu_slot {
                        0..=1 => &phase.inputs[*menu_slot],
                        2 => &phase.result_preview,
                        _ => anyhow::bail!("unknown anvil pickup backing container"),
                    };
                    ensure!(
                        matches_pickup_observation(actual, stack)?,
                        "anvil pickup top listener lost its retained preview/input phase"
                    );
                }
            }
        }
    }
    if !local_changes.is_empty() {
        // These are exactly the nonempty stacks queried by Main's actual
        // inventory-count criterion scan. Do not reinterpret menu top/cursor
        // stacks as inventory entries or charge another dirty counter.
        preflight_inventory_scan(inventory)?;
    }
    Ok((packets, local_changes))
}

fn preflight_inventory_scan(inventory: &PlayerInventory) -> Result<()> {
    for slot in (36..45)
        .chain(9..36)
        .chain((5..9).rev())
        .chain(std::iter::once(45))
    {
        let stack = inventory
            .slot(slot)
            .context("anvil pickup inventory scan slot absent")?;
        if !stack.is_empty() {
            stack.max_stack_size()?;
        }
    }
    Ok(())
}

fn matches_pickup_observation(actual: &ItemStack, observed: &ItemStack) -> Result<bool> {
    // MenuSync copies logical empties to the canonical EMPTY observation.
    // Source equality still compares logical counts and components; it does
    // not require a detached copy to preserve raw zero-count representation.
    Ok((if actual.is_empty() { 0 } else { actual.count })
        == (if observed.is_empty() {
            0
        } else {
            observed.count
        })
        && actual.same_item_same_components(observed)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use main_item_gameplay::{
        GameplayCriterion, GameplayObservation, GameplaySourceScope, MainItemGameplay,
    };
    use mc121_rust::{
        crafting::RecipeBook,
        entity::{PlayerProfile, Position},
        geometry::Aabb,
        inventory::Component,
        runtime_chunk_manager::RuntimeChunkManager,
        workstations::menu::MenuSync,
    };

    fn item(name: &str) -> ItemStack {
        ItemStack::new(RecipeBook::vanilla().item_id(name).unwrap(), 1).unwrap()
    }

    fn phase() -> AnvilPhase {
        AnvilPhase {
            inputs: [item("iron_pickaxe"), item("iron_ingot")],
            result_preview: item("iron_pickaxe"),
            carried: item("stone"),
            level_cost: 1,
            repair_material_count: 1,
        }
    }

    #[test]
    fn pickup_admission_observes_candidate_and_retained_cursor_without_mutation() -> Result<()> {
        let mut inventory = PlayerInventory::new();
        inventory.set_creative_slot(9, item("bread"))?;
        let retained = phase();
        let mut unresolved = item("mushroom_stew");
        unresolved.components.push(Component {
            type_id: 20,
            data: unresolved.effective_component_bytes(20).unwrap().to_vec(),
        });
        let before = unresolved.clone();
        let counter = inventory.get_times_changed();
        assert!(require_pickup_components(&inventory, &retained, &unresolved).is_err());
        assert_eq!(unresolved, before);
        assert_eq!(inventory.get_times_changed(), counter);
        require_pickup_components(&inventory, &retained, &item("bread"))?;
        assert_eq!(retained.carried, item("stone"));
        assert_eq!(inventory.get_times_changed(), counter);
        let mut unsupported_cursor = retained;
        unsupported_cursor.carried = unresolved;
        assert!(
            require_pickup_components(&inventory, &unsupported_cursor, &item("stone")).is_err()
        );
        let mut enchanted = item("iron_pickaxe");
        let mut fields = Writer::new();
        fields.varint(1).varint(0).varint(1).bool(true);
        enchanted.components.push(Component {
            type_id: 9,
            data: fields.into_inner(),
        });
        assert!(require_pickup_components(&inventory, &phase(), &enchanted)
            .unwrap_err()
            .is::<main_anvil_context::UnsupportedAnvilEnchantmentContext>());
        Ok(())
    }

    #[test]
    fn pickup_sync_maps_actual_inventory_and_retains_source_packet_offsets() -> Result<()> {
        let mut inventory = PlayerInventory::new();
        inventory.set_creative_slot(9, item("stone"))?;
        inventory.set_creative_slot(36, item("bread"))?;
        let retained = phase();
        let packet = |tag| Packet {
            id: 0x15,
            payload: vec![tag],
        };
        let events = vec![
            SyncEvent::SlotListener {
                menu_slot: 2,
                player_slot: None,
                stack: retained.result_preview.clone(),
            },
            SyncEvent::Packet(packet(1)),
            SyncEvent::SlotListener {
                menu_slot: 3,
                player_slot: Some(9),
                stack: item("stone"),
            },
            SyncEvent::Packet(packet(2)),
            SyncEvent::SlotListener {
                menu_slot: 30,
                player_slot: Some(36),
                stack: item("bread"),
            },
            SyncEvent::Packet(packet(3)),
            SyncEvent::DataListener {
                index: 0,
                value: retained.level_cost,
            },
        ];
        let counter = inventory.get_times_changed();
        let (packets, local) = prepare_pickup_sync(&events, &retained, &inventory)?;
        assert_eq!(
            packets.iter().map(|p| p.payload[0]).collect::<Vec<_>>(),
            [1, 2, 3]
        );
        assert_eq!(
            local
                .iter()
                .map(|c| (c.menu_slot, c.inventory_slot, c.packet_offset))
                .collect::<Vec<_>>(),
            [(9, Some(9), 1), (36, Some(0), 2)]
        );
        assert_eq!(inventory.get_times_changed(), counter);
        let foreign = [SyncEvent::SlotListener {
            menu_slot: 3,
            player_slot: Some(10),
            stack: item("stone"),
        }];
        assert!(prepare_pickup_sync(&foreign, &retained, &inventory).is_err());
        Ok(())
    }

    #[test]
    fn actual_pickup_listener_keeps_external_cursor_preview_and_dirty_owner() -> Result<()> {
        let mut registry = EntityRegistry::new();
        registry.register(PlayerState::new(
            1,
            PlayerProfile {
                uuid: [1; 16],
                name: "Picker".into(),
                properties: vec![],
            },
            Position {
                x: 0.,
                y: 64.,
                z: 0.,
                yaw: 0.,
                pitch: 0.,
                on_ground: true,
            },
        )?)?;
        let player = PickupPlayer {
            entity_id: 1,
            uuid: [1; 16],
            bounds: Aabb::player(Vec3::new(0., 64., 0.), 0.6, 1.8),
            alive: true,
            spectator: false,
            vehicle_bounds: None,
        };
        let manager = RuntimeChunkManager::new(2, 2);
        let mut gameplay = MainItemGameplay::new(GameplaySourceScope::NativeFixtureEmptyCriteria);
        gameplay.register_player(1, player.uuid)?;
        let mut inventory = PlayerInventory::new();
        let retained = phase();
        let top = [
            retained.inputs[0].clone(),
            retained.inputs[1].clone(),
            retained.result_preview.clone(),
        ];
        let mut sync = MenuSync::new(7, WorkstationKind::Anvil)?;
        sync.set_carried(retained.carried.clone());
        sync.initialize(&top, &inventory, &[retained.level_cost])?;
        // The existing pickup owner already installed this actual inventory
        // stack before its ServerPlayer.take synchronization callback.
        inventory.set_creative_slot(9, item("bread"))?;
        let counter = inventory.get_times_changed();
        let events = sync.broadcast_changes(&top, &inventory, &[retained.level_cost])?;
        let (packets, changes) = prepare_pickup_sync(&events, &retained, &inventory)?;
        {
            let mut frame = gameplay.frame(&registry, &manager, true);
            let foreign = PickupPlayer {
                uuid: [2; 16],
                ..player
            };
            assert!(frame
                .preflight_inventory_slot_changes(foreign, &inventory, &changes)
                .is_err());
            frame.preflight_inventory_slot_changes(player, &inventory, &changes)?;
            for change in &changes {
                frame.inventory_slot_changed(
                    player,
                    change.menu_slot,
                    &inventory,
                    &change.stack,
                )?;
            }
        }
        let observations = gameplay.drain_observations();
        assert!(matches!(observations.as_slice(), [
            GameplayObservation::InventoryScan { player: scan_player, counts },
            GameplayObservation::EmptyCriterion { player: criterion_player, criterion: GameplayCriterion::InventoryChanged },
        ] if *scan_player == [1; 16] && *criterion_player == [1; 16]
            && counts.occupied == 1 && counts.empty == 40 && counts.full == 0));
        assert_eq!(packets.len(), 1);
        assert_eq!(packets[0].id, 0x15);
        let mut fields = Reader::new(&packets[0].payload);
        assert_eq!(fields.i8()?, 7);
        assert_eq!(fields.varint()?, sync.state_id());
        assert_eq!(fields.i16()?, 3);
        assert_eq!(ItemStack::decode_exact(fields.remaining())?, item("bread"));
        assert_eq!(sync.carried(), &retained.carried);
        assert_eq!(&sync.snapshot(&top, &inventory)?[..3], &top);
        assert_eq!(inventory.get_times_changed(), counter);
        Ok(())
    }
}
