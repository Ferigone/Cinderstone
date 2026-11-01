//! Native creative BucketItem endpoint. World, menu and statistic changes are
//! prepared before the shared menu transaction publishes any packets.
use super::main_world_interactions::MainInteractionResult;
use super::*;
use mc121_rust::{
    bucket_interactions::{self, BucketAction, BucketKind, BucketResult, BucketWorld},
    fluid_physics::{FluidShapeCache, FluidState},
    geometry::VoxelShape,
    placement::{BlockPosition, PlacementWorld},
    world_interactions::{
        InteractionEvent, InteractionPlan, InteractionWorld, RuntimeInteractionWorld,
    },
};

struct BucketQuery<'a, 'b, F> {
    placement: RuntimeInteractionWorld<'a, F>,
    fluid_shapes: &'b mut FluidShapeCache,
    ultrawarm: bool,
    native_creative_permissions: bool,
}
impl<F: FnMut(u32, BlockPosition) -> Result<bool>> PlacementWorld for BucketQuery<'_, '_, F> {
    fn block(&mut self, p: BlockPosition) -> Result<u32> {
        self.placement.block(p)
    }
    fn min_build_height(&mut self) -> Result<i32> {
        self.placement.min_build_height()
    }
    fn max_build_height(&mut self) -> Result<i32> {
        self.placement.max_build_height()
    }
    fn has_neighbor_signal(&mut self, p: BlockPosition) -> Result<bool> {
        self.placement.has_neighbor_signal(p)
    }
    fn inside_world_border(&mut self, p: BlockPosition) -> Result<bool> {
        self.placement.inside_world_border(p)
    }
    fn raw_brightness(&mut self, p: BlockPosition, n: i32) -> Result<i32> {
        self.placement.raw_brightness(p, n)
    }
    fn can_see_sky(&mut self, p: BlockPosition) -> Result<bool> {
        self.placement.can_see_sky(p)
    }
}
impl<F: FnMut(u32, BlockPosition) -> Result<bool>> InteractionWorld for BucketQuery<'_, '_, F> {
    fn unobstructed(&mut self, state: u32, p: BlockPosition) -> Result<bool> {
        self.placement.unobstructed(state, p)
    }
}
impl<F: FnMut(u32, BlockPosition) -> Result<bool>> BucketWorld for BucketQuery<'_, '_, F> {
    fn may_interact(&mut self, _position: BlockPosition) -> Result<bool> {
        Ok(self.native_creative_permissions)
    }
    fn may_use_item_at(
        &mut self,
        _position: BlockPosition,
        _face: geometry::Direction,
    ) -> Result<bool> {
        Ok(self.native_creative_permissions)
    }
    fn ultrawarm(&mut self) -> Result<bool> {
        Ok(self.ultrawarm)
    }
    fn fluid_shape(&mut self, p: BlockPosition, state: FluidState) -> Result<VoxelShape> {
        self.placement.fluid_shape(self.fluid_shapes, p, state)
    }
}

fn stage_fixture_bucket(
    world: &WorldBackend,
    plan: &InteractionPlan,
) -> Result<Option<(FlatWorld, Vec<BlockChange>)>> {
    let WorldBackend::Fixture(current) = world else {
        return Ok(None);
    };
    for (at, expected) in &plan.read_set {
        ensure!(
            current.block(at.0, at.1, at.2) == *expected,
            "world changed during bucket planning"
        );
    }
    let mut staged = current.clone();
    let mut changes = Vec::new();
    for event in &plan.events {
        if let InteractionEvent::Write {
            position,
            old_state,
            state,
            ..
        } = event
        {
            ensure!(
                staged.block(position.0, position.1, position.2) == *old_state,
                "staged bucket state changed"
            );
            placement::placement_block_name(*state)?;
            if staged.set_block(position.0, position.1, position.2, *state) {
                changes.push(BlockChange {
                    x: position.0,
                    y: position.1,
                    z: position.2,
                    state: *state,
                });
            }
        }
    }
    Ok(Some((staged, changes)))
}

impl Server {
    pub(super) async fn use_bucket_item(
        &self,
        id: i32,
        hand: i32,
    ) -> Result<MainInteractionResult> {
        let bounds = self
            .border
            .read()
            .map_err(|_| anyhow!("world border lock poisoned"))?
            .bounds(self.epoch.elapsed().as_millis() as i64);
        let border = CollisionBorder {
            min_x: bounds.min_x,
            min_z: bounds.min_z,
            max_x: bounds.max_x,
            max_z: bounds.max_z,
        };
        let mut world = self.world.write().await;
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("player registry lock poisoned"))?;
        if self.shutting_down.load(Ordering::Acquire) {
            return Ok(MainInteractionResult::Rejected);
        }
        let Some(player) = players.registry.get(id).cloned() else {
            return Ok(MainInteractionResult::Rejected);
        };
        let container = players
            .containers
            .get(&id)
            .context("bucket player inventory missing")?
            .clone();
        let selected = container.read(|menu| menu.inventory.selected_stack(hand).cloned())??;
        let Some(kind) = BucketKind::from_item_id(selected.item_id) else {
            return Ok(MainInteractionResult::Pass);
        };
        if selected.is_empty() {
            return Ok(MainInteractionResult::Pass);
        }
        // This server's explicit fresh native creative policy owns permission
        // state. Imported source permissions and survival consumption do not
        // inherit that policy merely because their data could be read.
        if player.gamemode != 1
            || !matches!(&*world, WorldBackend::Fixture(_))
            || players.gameplay.require_supported_floor().is_err()
        {
            return Ok(MainInteractionResult::Unsupported);
        }
        let dimensions = player_physics::player_dimensions(player.metadata.pose, 1.)?;
        let eyes = geometry::player_eye_position(
            Vec3::new(player.position.x, player.position.y, player.position.z),
            dimensions.eye_height,
        );
        let end = bucket_interactions::pov_ray_end(
            eyes,
            player.position.pitch,
            player.position.yaw,
            geometry::CREATIVE_BLOCK_INTERACTION_RANGE,
        )?;
        let ultrawarm = players.world_context.dimension.ultrawarm;
        let mut query = BucketQuery {
            placement: RuntimeInteractionWorld::new(
                &mut world,
                world::MIN_Y,
                world::MAX_Y + 1,
                border,
                |_, _| bail!("BucketItem does not run entity placement obstruction"),
            )?,
            fluid_shapes: &mut players.world_interactions.fluid_shapes,
            ultrawarm,
            native_creative_permissions: true,
        };
        let bucket = match bucket_interactions::plan_bucket_use(kind, eyes, end, &mut query)? {
            BucketResult::Plan(plan) => plan,
            BucketResult::Pass => return Ok(MainInteractionResult::Pass),
            BucketResult::Rejected => return Ok(MainInteractionResult::Rejected),
            BucketResult::Unsupported(_) => return Ok(MainInteractionResult::Unsupported),
        };
        drop(query);
        let plan = bucket.interaction;
        if plan.has_required_hooks()
            || plan.events.iter().any(|event| {
                matches!(
                    event,
                    InteractionEvent::CreateBlockEntity(_)
                        | InteractionEvent::DropResources { .. }
                        | InteractionEvent::BucketEvaporation { .. }
                )
            })
        {
            return Ok(MainInteractionResult::Unsupported);
        }
        players.random_block_ticks.prepare_plan(&mut world, &plan)?;
        let source_edit = match players.prepare_native_source_edit(&world, &plan) {
            Ok(token) => token,
            Err(_) => return Ok(MainInteractionResult::Unsupported),
        };
        players.world_interactions.can_append(plan.events.len())?;
        let Some((staged_world, changes)) = stage_fixture_bucket(&world, &plan)? else {
            return Ok(MainInteractionResult::Unsupported);
        };
        let mut stats = players
            .gameplay
            .snapshot_player_stats(player.profile.uuid)?;
        let stat = mc121_rust::player_stats::stat_key("minecraft:used", kind.registry_name())
            .context("bucket use statistic missing")?;
        stats.increment(stat, 1)?;
        let output = ItemStack::new(bucket.resulting_bucket.item_id(), 1)?;
        let action = bucket.action;
        let uuid = player.profile.uuid;
        let actor_position = player.position;
        let counter_plan = plan.clone();
        let LivePlayers {
            gameplay,
            world_interactions,
            chunk_runtime,
            entity_source,
            random_block_ticks,
            ..
        } = &mut *players;
        container.transact_with_commit(
            move |menu| {
                ensure!(
                    menu.inventory.selected_stack(hand)? == &selected,
                    "bucket hand changed during transaction"
                );
                if action == BucketAction::Pickup {
                    let mut contains = false;
                    for slot in &menu.inventory.slots()[5..46] {
                        if !slot.is_empty() && slot.same_item_same_components(&output)? {
                            contains = true;
                            break;
                        }
                    }
                    if !contains {
                        let mut result = output;
                        // Creative Inventory.add deliberately discards overflow;
                        // ItemUtils does not create an overflow entity here.
                        menu.add_pickup_to_inventory(&mut result, true)?;
                    }
                }
                let packets = menu.broadcast_changes()?;
                let local = menu.take_local_slot_changes();
                let prepared = gameplay.prepare_menu_block_commit(
                    uuid,
                    &menu.inventory,
                    local,
                    stats,
                    changes.clone(),
                )?;
                Ok(((prepared, staged_world, changes, plan.events), packets))
            },
            |(prepared, staged_world, changes, events)| {
                *world = WorldBackend::Fixture(staged_world);
                let counter_result = random_block_ticks.observe_plan(&counter_plan);
                let lighting = (|| -> Result<Vec<WorldUpdate>> {
                    for change in &changes {
                        chunk_runtime.invalidate_block_lighting((change.x, change.y, change.z))?;
                    }
                    chunk_runtime.settle_lighting(&world)
                })();
                let source_result = if let Some(source) = entity_source {
                    if counter_result.is_err() {
                        source.invalidate(
                            "bucket random tick counter update failed after world commit",
                        );
                        Ok(())
                    } else {
                        match source_edit {
                            Some(token) => source.observe_owned_commit(token, &world),
                            None if !changes.is_empty() => {
                                source.invalidate(
                                    "bucket mutation has no complete native transaction",
                                );
                                Ok(())
                            }
                            None => Ok(()),
                        }
                    }
                } else {
                    Ok(())
                };
                prepared.commit();
                world_interactions.append_from_player(id, actor_position, events);
                let mut updates = changes
                    .into_iter()
                    .map(WorldUpdate::Block)
                    .collect::<Vec<_>>();
                match counter_result.and(source_result).and(lighting) {
                    Ok(light) => {
                        updates.extend(light);
                        self.publish_blocks(updates);
                        Ok(())
                    }
                    Err(error) => {
                        // World/menu/stat data already form a complete commit.
                        // The real lighting owner retains its failed work; keep
                        // ordered block publication and report the light error.
                        self.publish_blocks(updates);
                        Err(error)
                    }
                }
            },
        )??;
        Ok(MainInteractionResult::Applied)
    }
}
