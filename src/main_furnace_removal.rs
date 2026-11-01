//! Owned furnace removal for the current native creative-player profile.
//!
//! Container splits and recipe XP are prepared in source order against the
//! actual level RNG, shared class index and entity counter. No drained block
//! entity image is ever published. Existing viewers retain the removed
//! container object until Java's normal later menu-validity phase closes it.
//! Unowned callbacks remain explicit boundaries.
use super::*;
use mc121_rust::{
    container_drops, native_block_entities,
    world_interactions::{self, RuntimeInteractionWorld},
};

impl Server {
    fn correct_native_furnace_break(&self, id: i32, at: (i32, i32, i32), state: u32) {
        let packet = Packet {
            id: 0x09,
            payload: world::block_update_payload(at.0, at.1, at.2, state),
        };
        let _ = self.entity_events.send(PlayerEvent::WorldAudience {
            recipients: Arc::new(vec![id]),
            packets: Arc::new(vec![packet]),
        });
    }

    /// Called with the existing world -> players lock order, after the packet
    /// host checks the actor's reach, build bounds and world border.
    pub(super) fn try_remove_native_furnace_locked(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        id: i32,
        at: (i32, i32, i32),
        now: i64,
        border: CollisionBorder,
    ) -> Result<main_world_interactions::MainInteractionResult> {
        use main_world_interactions::MainInteractionResult as Outcome;
        let old = world.block(at.0, at.1, at.2)?;
        if native_block_entities::furnace_kind(old).is_none() {
            return Ok(Outcome::Pass);
        }
        let actor = players
            .registry
            .get(id)
            .context("furnace breaker missing")?
            .clone();
        if self.shutting_down.load(Ordering::Acquire) || actor.gamemode != 1 {
            self.correct_native_furnace_break(id, at, old);
            return Ok(Outcome::Rejected);
        }
        let held = players
            .containers
            .get(&id)
            .context("furnace breaker inventory missing")?
            .read(|menu| menu.inventory.selected_stack(0).cloned())??;
        if !held.is_empty() {
            let item =
                inventory::item_name(held.item_id).context("furnace breaker item missing")?;
            let item = item.strip_prefix("minecraft:").unwrap_or(item);
            if matches!(
                item,
                "wooden_sword"
                    | "stone_sword"
                    | "iron_sword"
                    | "golden_sword"
                    | "diamond_sword"
                    | "netherite_sword"
                    | "mace"
                    | "trident"
            ) {
                // SwordItem, MaceItem and TridentItem all return !isCreative.
                self.correct_native_furnace_break(id, at, old);
                return Ok(Outcome::Rejected);
            }
            if item == "debug_stick" {
                // DebugStickItem has an actual state-cycling callback before
                // returning false. Do not substitute an ordinary block break.
                self.correct_native_furnace_break(id, at, old);
                return Ok(Outcome::Unsupported);
            }
        }
        let replacement = world_interactions::removed_replacement(old)?;
        let snapshot = {
            let WorldBackend::Fixture(flat) = &*world else {
                self.correct_native_furnace_break(id, at, old);
                return Ok(Outcome::Unsupported);
            };
            flat.native_block_entities()
                .prepare_furnace_removal(at, old, replacement)?
                .context("furnace removal did not change block identity")?
        };
        // Native viewers and live menus must identify the same actual block
        // entity. Position alone is insufficient: a new furnace at this spot
        // must never become the old menu's container.
        let viewers = players.furnaces.viewer_ids_for_owner(snapshot.owner_key());
        let retention = {
            let WorldBackend::Fixture(flat) = &*world else {
                unreachable!("native furnace backend was validated")
            };
            flat.native_block_entities()
                .prepare_furnace_viewer_retention(&snapshot, &viewers)?
        };
        let retired_menus = players
            .furnaces
            .prepare_removal_retirement(&snapshot, &retention)?;

        // playerWillDestroy posts BLOCK_DESTROY before removeBlock. Its real
        // empty native registry has no RNG/listener side effects, so certify
        // it now and retain its outgoing break packet until commit is certain.
        let radius = world_interactions::game_event_radius("minecraft:block_destroy")?;
        let min = (
            (at.0 - radius).div_euclid(16),
            (at.2 - radius).div_euclid(16),
        );
        let max = (
            (at.0 + radius).div_euclid(16),
            (at.2 + radius).div_euclid(16),
        );
        players.chunk_runtime.certify_query_region(
            world,
            &mut players.chunk_transports,
            &self.entity_events,
            now,
            min,
            max,
        )?;
        players.require_current_native_source(world)?;
        {
            let mut frame =
                players
                    .gameplay
                    .frame(&players.registry, &players.chunk_runtime.manager, true);
            mc121_rust::game_event_context::post_to_empty_registries(
                &mut frame,
                mc121_rust::game_event_context::GameEventPost {
                    position: Vec3::new(
                        f64::from(at.0) + 0.5,
                        f64::from(at.1) + 0.5,
                        f64::from(at.2) + 0.5,
                    ),
                    notification_radius: radius,
                },
            )?;
        }

        let mut drained = match &*world {
            WorldBackend::Fixture(flat) => WorldBackend::Fixture(flat.clone()),
            _ => unreachable!("native furnace backend was validated"),
        };
        let WorldBackend::Fixture(flat) = &mut drained else {
            unreachable!()
        };
        flat.native_block_entities_mut()
            .drain_furnace_removal_retaining_viewers(&snapshot, &retention)?;
        let plan = {
            let mut query = RuntimeInteractionWorld::new(
                &mut drained,
                world::MIN_Y,
                world::MAX_Y + 1,
                border,
                |_, _| bail!("furnace removal does not own entity placement"),
            )?;
            world_interactions::plan_edit(at, replacement, 3, &mut query)?
        };
        let mut final_world = drained;
        let mut changes = main_world_interactions::commit_fixture_plan(&mut final_world, &plan)?
            .context("furnace removal requires native rollback world")?;
        self.notify_native_furnace_changed_locked(&mut final_world, players, at)?;
        let source = players
            .prepare_required_native_furnace_removal_source_edit(world, at, &plan, &retention)?;
        players.world_interactions.can_append(plan.events.len())?;

        let counter_before = self.entities.load(Ordering::Acquire);
        let mut next_id = counter_before;
        let mut level_after_drops = players.item_runtime.world_random_mut().clone();
        let math_after_drops = players
            .math_random
            .as_ref()
            .context("Math.random owner missing")?
            .with_random(|random| Ok(random.clone()))?;
        let mut slots = snapshot.slots().to_vec();
        let entries = container_drops::drop_contents_with_factory(
            Vec3::new(f64::from(at.0), f64::from(at.1), f64::from(at.2)),
            &mut slots,
            &mut level_after_drops,
            &mut next_id,
            || random_seed::create_random().map_err(anyhow::Error::from),
        )?;
        let item_ids = entries
            .iter()
            .map(|(state, _)| state.entity_id)
            .collect::<Vec<_>>();
        let staged_index = players.item_runtime.prepare_index_after_items(&entries)?;
        let xp = main_experience::prepare_furnace_block_experience_after_drops(
            world,
            players,
            at,
            &snapshot.pending_recipe_rewards()?,
            &mut next_id,
            staged_index,
            &level_after_drops,
            &math_after_drops,
        )?;
        players.item_runtime.validate_entity_registration_batch(
            item_ids.iter().copied().chain(xp.registration_ids()),
        )?;

        let break_recipients = players
            .registry
            .snapshot()
            .into_iter()
            .filter(|player| {
                let dx = player.position.x - f64::from(at.0);
                let dy = player.position.y - f64::from(at.1);
                let dz = player.position.z - f64::from(at.2);
                player.entity_id != id && (dx * dx + dy * dy) + dz * dz < 64. * 64.
            })
            .map(|player| player.entity_id)
            .collect::<Vec<_>>();
        let break_packet =
            mc121_rust::item_effects::level_event_packet(2001, at, old as i32, false);
        let prepared_items = players.item_runtime.prepare_items(entries)?;
        if self
            .entities
            .compare_exchange(counter_before, next_id, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            drop(prepared_items);
            self.correct_native_furnace_break(id, at, old);
            return Ok(Outcome::Rejected);
        }

        // No fallible gameplay construction remains. The shared level/player
        // locks exclude all canonical mutation between preflight and commit.
        // setBlockState exposes the new palette before onRemove's entities.
        // addFreshEntity's current native outbox/pairing listeners do not read
        // the removed block entity; broader such callbacks remain unsupported.
        *world = final_world;
        // The real old container has had its slots consumed by dropContents.
        // Keep that object (including timers and RecipesUsed) for each old
        // menu; do not invent an immediate close or rebind by coordinates.
        players.furnaces.commit_removal_retirement(retired_menus);
        if !break_recipients.is_empty() {
            let _ = self.entity_events.send(PlayerEvent::WorldAudience {
                recipients: Arc::new(break_recipients),
                packets: Arc::new(vec![break_packet]),
            });
        }
        prepared_items.commit_with_world_random(level_after_drops);
        xp.commit(players, &self.entity_events)
            .expect("exclusive prepared container Item/XP transaction");
        players
            .observe_native_source_edit(world, Some(source), !changes.is_empty())
            .expect("exclusive prepared furnace removal source");
        for change in &changes {
            if let WorldUpdate::Block(block) = change {
                players
                    .gameplay
                    .observe_block_change((block.x, block.y, block.z), block.state)
                    .expect("prepared native furnace block facts");
                players
                    .chunk_runtime
                    .invalidate_block_lighting((block.x, block.y, block.z))
                    .expect("prepared native furnace lighting owner");
            }
        }
        let lighting = players.chunk_runtime.settle_lighting(world);
        players
            .world_interactions
            .append_from_player(id, actor.position, plan.events);
        // Actual furnace onRemove/comparator validation was executed above;
        // the generic redstone dispatcher must not claim it a second time.
        players
            .world_interactions
            .mark_redstone_primary(at, old, replacement);
        match lighting {
            Ok(light) => {
                changes.extend(light);
                self.publish_blocks(changes);
                Ok(Outcome::Applied)
            }
            Err(error) => {
                self.publish_blocks(changes);
                Err(error)
            }
        }
    }
}
