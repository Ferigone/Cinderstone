//! Closed single chest/barrel destruction through actual native Item owners.
//!
//! The supported creative path stages the original 27-slot contents, actual
//! level RNG and entity counter before committing the palette/BE/source edit.
//! A drained staging image is never published as a live container. Active
//! viewers, double/waterlogged chests and comparator receivers remain explicit
//! boundaries; this callback does not construct XP or creative block loot.
use super::*;
use mc121_rust::{
    container_drops, native_block_entities,
    world_interactions::{self, RuntimeInteractionWorld},
};

impl Server {
    fn correct_native_container_break(&self, id: i32, at: (i32, i32, i32), state: u32) {
        let packet = Packet {
            id: 0x09,
            payload: world::block_update_payload(at.0, at.1, at.2, state),
        };
        let _ = self.entity_events.send(PlayerEvent::WorldAudience {
            recipients: Arc::new(vec![id]),
            packets: Arc::new(vec![packet]),
        });
    }

    /// The packet host has already checked reach, build height and world border.
    /// Call under the existing world -> players exclusive lock order.
    pub(super) fn try_remove_native_container_locked(
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
        let Some(kind) = native_block_entities::container_kind(old) else {
            return Ok(Outcome::Pass);
        };
        let actor = players
            .registry
            .get(id)
            .context("container breaker missing")?
            .clone();
        if self.shutting_down.load(Ordering::Acquire) || actor.gamemode != 1 {
            self.correct_native_container_break(id, at, old);
            return Ok(Outcome::Rejected);
        }
        let held = players
            .containers
            .get(&id)
            .context("container breaker inventory missing")?
            .read(|menu| menu.inventory.selected_stack(0).cloned())??;
        if !held.is_empty() {
            let item = inventory::item_name(held.item_id)
                .context("container breaker item missing")?;
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
                self.correct_native_container_break(id, at, old);
                return Ok(Outcome::Rejected);
            }
            if item == "debug_stick" {
                self.correct_native_container_break(id, at, old);
                return Ok(Outcome::Unsupported);
            }
        }
        let unsupported_shape = match kind {
            native_block_entities::ContainerKind::Chest => {
                inventory::block_state_property(old, "type").as_deref() != Some("single")
                    || inventory::block_state_property(old, "waterlogged").as_deref()
                        != Some("false")
            }
            native_block_entities::ContainerKind::Barrel => {
                inventory::block_state_property(old, "open").as_deref() != Some("false")
            }
        };
        // Spectator menus do not necessarily appear in the opener counter, so
        // certify the actual menu owner independently of the BE's viewers.
        if unsupported_shape || players.native_containers.has_menu_at(at) {
            self.correct_native_container_break(id, at, old);
            return Ok(Outcome::Unsupported);
        }
        let replacement = world_interactions::removed_replacement(old)?;
        let snapshot = {
            let WorldBackend::Fixture(flat) = &*world else {
                self.correct_native_container_break(id, at, old);
                return Ok(Outcome::Unsupported);
            };
            match flat
                .native_block_entities()
                .prepare_container_removal(at, old, replacement)
            {
                Ok(Some(snapshot)) => snapshot,
                Ok(None) => return Ok(Outcome::Pass),
                Err(_) => {
                    self.correct_native_container_break(id, at, old);
                    return Ok(Outcome::Unsupported);
                }
            }
        };
        players.gameplay.require_supported_floor()?;
        players.require_current_native_source(world)?;

        // playerWillDestroy posts BLOCK_DESTROY before removeBlock. The native
        // empty registries have no RNG/listener side effects; certify them
        // before any actual entity factory or level-random draw.
        let radius = world_interactions::game_event_radius("minecraft:block_destroy")?;
        players.chunk_runtime.certify_current_query_region(
            world,
            &mut players.chunk_transports,
            &self.entity_events,
            now,
            ((at.0 - radius).div_euclid(16), (at.2 - radius).div_euclid(16)),
            ((at.0 + radius).div_euclid(16), (at.2 + radius).div_euclid(16)),
        )?;
        {
            let mut frame = players
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
            _ => unreachable!("native container backend was validated"),
        };
        let WorldBackend::Fixture(flat) = &mut drained else {
            unreachable!()
        };
        flat.native_block_entities_mut().drain_container_removal(&snapshot)?;
        let plan = {
            let mut query = RuntimeInteractionWorld::new(
                &mut drained,
                world::MIN_Y,
                world::MAX_Y + 1,
                border,
                |_, _| bail!("container removal does not own entity placement"),
            )?;
            world_interactions::plan_edit(at, replacement, 3, &mut query)?
        };
        let mut final_world = drained;
        let mut changes = main_world_interactions::commit_fixture_plan(&mut final_world, &plan)?
            .context("container removal requires native rollback world")?;
        // dropContentsOnDestroy invokes this after inventory drops and before
        // superclass BE removal. Here only absence of actual receivers is
        // certified; no comparator observation is fabricated or reordered.
        self.notify_native_container_changed_locked(&mut final_world, players, at)?;
        let source = players
            .prepare_required_native_container_removal_source_edit(world, at, &plan)?;
        players.world_interactions.can_append(plan.events.len())?;

        let counter_before = self.entities.load(Ordering::Acquire);
        let mut next_id = counter_before;
        let mut level_after_drops = players.item_runtime.world_random_mut().clone();
        let mut slots = snapshot.slots().to_vec();
        let entries = container_drops::drop_contents_with_factory(
            Vec3::new(f64::from(at.0), f64::from(at.1), f64::from(at.2)),
            &mut slots,
            &mut level_after_drops,
            &mut next_id,
            || random_seed::create_random().map_err(anyhow::Error::from),
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
        let break_packet = mc121_rust::item_effects::level_event_packet(2001, at, old as i32, false);
        let prepared_items = players.item_runtime.prepare_items(entries)?;
        if self
            .entities
            .compare_exchange(counter_before, next_id, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            drop(prepared_items);
            self.correct_native_container_break(id, at, old);
            return Ok(Outcome::Rejected);
        }

        // No fallible gameplay construction remains under these exclusive
        // owners. The new palette precedes onRemove's entity publications.
        // Current native world-add listeners do not read the removed BE;
        // broader callbacks require their actual owner before being enabled.
        *world = final_world;
        if !break_recipients.is_empty() {
            let _ = self.entity_events.send(PlayerEvent::WorldAudience {
                recipients: Arc::new(break_recipients),
                packets: Arc::new(vec![break_packet]),
            });
        }
        prepared_items.commit_with_world_random(level_after_drops);
        players
            .observe_native_source_edit(world, Some(source), !changes.is_empty())
            .expect("exclusive prepared container removal source");
        for change in &changes {
            if let WorldUpdate::Block(block) = change {
                players
                    .gameplay
                    .observe_block_change((block.x, block.y, block.z), block.state)
                    .expect("prepared native container block facts");
                players
                    .chunk_runtime
                    .invalidate_block_lighting((block.x, block.y, block.z))
                    .expect("prepared native container lighting owner");
            }
        }
        let lighting = players.chunk_runtime.settle_lighting(world);
        players
            .world_interactions
            .append_from_player(id, actor.position, plan.events);
        // The dedicated host already handled the actual container callback;
        // generic redstone must not execute that onRemove a second time.
        players.world_interactions.mark_redstone_primary(at, old, replacement);
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
