//! Packet-facing staged building operations. Main owns registration and effect
//! dispatch; pending work is retained explicitly instead of being discarded.
use super::*;
use mc121_rust::placement::PlacementWorld;
use mc121_rust::world_interactions::{
    self, InteractionEvent, InteractionPlan, InteractionResult, RuntimeInteractionWorld,
};
use std::collections::VecDeque;

#[derive(Clone, Debug)]
pub(super) struct PendingInteractionEffect {
    pub actor: i32,
    pub actor_position: Option<Position>,
    pub fluid_callback_done: bool,
    pub redstone_callback_done: bool,
    pub fire_callback_done: bool,
    pub event: InteractionEvent,
}
pub(super) struct WorldInteractionRuntime {
    pending: VecDeque<PendingInteractionEffect>,
    pub(super) fluid_shapes: mc121_rust::fluid_physics::FluidShapeCache,
}
impl Default for WorldInteractionRuntime {
    fn default() -> Self {
        Self::new(false)
    }
}
impl WorldInteractionRuntime {
    pub(super) fn new(fresh_native: bool) -> Self {
        use mc121_rust::fluid_physics::{FluidShapeCache, FluidShapeOwner};
        Self {
            pending: VecDeque::new(),
            fluid_shapes: if fresh_native {
                FluidShapeCache::from_observed_empty_owners(FluidShapeOwner::ALL)
            } else {
                FluidShapeCache::unknown()
            },
        }
    }
    pub(super) fn mark_redstone_primary(&mut self,p:(i32,i32,i32),old:u32,new:u32){for effect in self.pending.iter_mut().rev(){if matches!(effect.event,InteractionEvent::FluidBlockChanged{position,old_state,state,..}|InteractionEvent::FluidOnlyBlockChanged{position,old_state,state,..} if position==p&&old_state==old&&state==new){effect.redstone_callback_done=true;break;}}}
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
    pub fn pop_front(&mut self) -> Option<PendingInteractionEffect> {
        self.pending.pop_front()
    }
    pub fn push_front(&mut self, effect: PendingInteractionEffect) {
        self.pending.push_front(effect);
    }
    pub(super) fn can_append(&self, count: usize) -> Result<()> {
        ensure!(
            self.pending
                .len()
                .checked_add(count)
                .is_some_and(|n| n <= 65_536),
            "pending interaction work limit reached"
        );
        Ok(())
    }
    pub(super) fn append(
        &mut self,
        actor: i32,
        events: impl IntoIterator<Item = InteractionEvent>,
    ) {
        self.pending.extend(
            events
                .into_iter()
                .map(|event| match event {
                    InteractionEvent::Write {
                        position,
                        old_state,
                        state,
                        flags,
                    } => InteractionEvent::FluidBlockChanged {
                        position,
                        old_state,
                        state,
                        flags,
                    },
                    event => event,
                })
                .map(|event| PendingInteractionEffect {
                    actor,
                    actor_position: None,
                    fluid_callback_done: false,
                    redstone_callback_done: false,
                    fire_callback_done: false,
                    event,
                }),
        );
    }
    pub(super) fn append_from_player(
        &mut self,
        actor: i32,
        actor_position: Position,
        events: impl IntoIterator<Item = InteractionEvent>,
    ) {
        let start = self.pending.len();
        self.append(actor, events);
        for pending in self.pending.iter_mut().skip(start) {
            pending.actor_position = Some(actor_position);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MainInteractionResult {
    Applied,
    Rejected,
    Pass,
    Unsupported,
}

#[derive(Debug, Default)]
pub(super) struct InteractionDrain {
    pub processed: usize,
    pub pending: usize,
    pub blocked: Option<String>,
}

fn publish_audience(
    registry: &EntityRegistry,
    events: &broadcast::Sender<PlayerEvent>,
    origin: Vec3,
    radius: f64,
    excluded: Option<i32>,
    packet: Packet,
) {
    let recipients = registry
        .snapshot()
        .into_iter()
        .filter(|player| {
            let d =
                Vec3::new(player.position.x, player.position.y, player.position.z).subtract(origin);
            Some(player.entity_id) != excluded
                && (d.x * d.x + d.y * d.y) + d.z * d.z < radius * radius
        })
        .map(|player| player.entity_id)
        .collect::<Vec<_>>();
    if !recipients.is_empty() {
        let _ = events.send(PlayerEvent::WorldAudience {
            recipients: Arc::new(recipients),
            packets: Arc::new(vec![packet]),
        });
    }
}

fn block_sound_packet(
    sound_id: i32,
    category: i32,
    origin: Vec3,
    volume: f32,
    pitch: f32,
    seed: i64,
) -> Packet {
    let mut out = Writer::new();
    out.varint(sound_id + 1)
        .varint(category)
        .i32((origin.x * 8.) as i32)
        .i32((origin.y * 8.) as i32)
        .i32((origin.z * 8.) as i32)
        .f32(volume)
        .f32(pitch)
        .i64(seed);
    Packet {
        id: 0x68,
        payload: out.into_inner(),
    }
}

impl LivePlayers {
    /// Consume the shared FIFO under the level and player locks. Unsupported
    /// work remains at its original position, together with its stated reason.
    pub(super) fn drain_world_interactions(
        &mut self,
        world: &mut WorldBackend,
        now: i64,
        border: CollisionBorder,
        in_overworld: bool,
        events: &broadcast::Sender<PlayerEvent>,
        limit: usize,
    ) -> InteractionDrain {
        let mut report = InteractionDrain::default();
        for _ in 0..limit {
            let Some(mut effect) = self.world_interactions.pop_front() else {
                break;
            };
            let result = (|| -> Result<()> {
                match &effect.event {
                    InteractionEvent::FluidBlockChanged { position, old_state, state, flags } => {
                        if !effect.fluid_callback_done {
                            self.observe_fluid_change(world, *position, *old_state, *state, *flags, now, border, events)?;
                            effect.fluid_callback_done = true;
                        }
                        if !effect.fire_callback_done {self.observe_fire_change(world,*position,*old_state,*state,now,border,events)?;effect.fire_callback_done=true;}
                        if !effect.redstone_callback_done {self.observe_redstone_change(world,*position,*old_state,*state,now,border,events)?;effect.redstone_callback_done=true;}
                    },
                    InteractionEvent::FluidOnlyBlockChanged { position, old_state, state, flags } => {
                        if !effect.fluid_callback_done{self.observe_fluid_change(world,*position,*old_state,*state,*flags,now,border,events)?;effect.fluid_callback_done=true;}
                        if !effect.redstone_callback_done{self.observe_redstone_change(world,*position,*old_state,*state,now,border,events)?;effect.redstone_callback_done=true;}
                    },
                    InteractionEvent::ScheduleWaterTick { position } => self.schedule_water_tick(world, *position, now)?,
                    InteractionEvent::NeighbourNotice { position, source_state } => {
                        self.observe_redstone_neighbors(world,*position,*source_state,now,border,events)?;
                        let mut query = RuntimeInteractionWorld::new(world, world::MIN_Y, world::MAX_Y + 1, border, |_, _| bail!("neighbour proof does not implement entity placement"))?;
                        ensure!(world_interactions::neighbour_notice_nonfluid_is_quiescent(*position, *source_state, &mut query)?,
                            "neighbourChanged callback requires its active block executor at {position:?}");
                    },
                    InteractionEvent::LevelEvent { position, event, data } => {
                        let origin = Vec3::new(f64::from(position.0), f64::from(position.1), f64::from(position.2));
                        publish_audience(&self.registry, events, origin, 64., None,
                            mc121_rust::item_effects::level_event_packet(*event, *position, *data, false));
                    },
                    InteractionEvent::PlaceSound { position, sound_id, volume, pitch, exclude_actor } => {
                        let seed = self.item_runtime.next_sound_seed()?;
                        let origin = Vec3::new(f64::from(position.0) + 0.5, f64::from(position.1) + 0.5, f64::from(position.2) + 0.5);
                        publish_audience(&self.registry, events, origin, f64::from(if *volume > 1. { 16. * *volume } else { 16. }),
                            exclude_actor.then_some(effect.actor), block_sound_packet(*sound_id, 4, origin, *volume, *pitch, seed));
                    },
                    InteractionEvent::ToggleSound { position, sound_id, volume, pitch_scale, pitch_offset, exclude_actor } => {
                        self.item_runtime.sound_random_state48().context("actual level sound RNG owner unavailable")?;
                        let pitch = self.item_runtime.world_random_mut().next_f32() * *pitch_scale + *pitch_offset;
                        let seed = self.item_runtime.next_sound_seed()?;
                        let origin = Vec3::new(f64::from(position.0) + 0.5, f64::from(position.1) + 0.5, f64::from(position.2) + 0.5);
                        publish_audience(&self.registry, events, origin, f64::from(if *volume > 1. { 16. * *volume } else { 16. }),
                            exclude_actor.then_some(effect.actor), block_sound_packet(*sound_id, 4, origin, *volume, pitch, seed));
                    },
                    InteractionEvent::PlayerSound { sound_id, volume, pitch } => {
                        let position = effect.actor_position.context("bucket sound has no retained original actor position")?;
                        let origin = Vec3::new(position.x, position.y, position.z);
                        let seed = self.item_runtime.next_sound_seed()?;
                        publish_audience(&self.registry, events, origin, f64::from(if *volume > 1. { 16. * *volume } else { 16. }),
                            Some(effect.actor), block_sound_packet(*sound_id, 7, origin, *volume, *pitch, seed));
                    },
                    InteractionEvent::GameEvent { position, name } => {
                        let radius = world_interactions::game_event_radius(name)?;
                        let min = ((position.0 - radius).div_euclid(16), (position.2 - radius).div_euclid(16));
                        let max = ((position.0 + radius).div_euclid(16), (position.2 + radius).div_euclid(16));
                        self.chunk_runtime.certify_query_region(world, &mut self.chunk_transports, events, now, min, max)?;
                        let mut frame = self.gameplay.frame(&self.registry, &self.chunk_runtime.manager, in_overworld);
                        mc121_rust::game_event_context::post_to_empty_registries(&mut frame,
                            mc121_rust::game_event_context::GameEventPost {
                                position: Vec3::new(f64::from(position.0) + 0.5, f64::from(position.1) + 0.5, f64::from(position.2) + 0.5),
                                notification_radius: radius,
                            })?;
                    },
                    InteractionEvent::ScheduleBlockTick(tick) => self.schedule_native_block_tick(world,tick,now)?,
                    InteractionEvent::DropResources { position, state } => bail!("actual loot context required for state {state} at {position:?}"),
                    InteractionEvent::CreateBlockEntity(entity) => {
                        let WorldBackend::Fixture(flat)=world else{bail!("imported BE lifecycle unavailable")};
                        let expected_name = if let Some(kind) = mc121_rust::native_block_entities::furnace_kind(entity.state) {
                            flat.native_block_entities().furnace_validate_at(entity.position,entity.state)?;
                            mc121_rust::native_block_entities::furnace_name(kind)
                        } else {
                            flat.native_block_entities().validate_at(entity.position,entity.state)?;
                            mc121_rust::native_block_entities::container_kind(entity.state).context("unsupported BE owner")?.name()
                        };
                        ensure!(entity.type_name == expected_name, "BE type differs from real owner");
                    },
                    InteractionEvent::RequiredHook{position,state,hook,declaring_class} if *hook=="setPlacedBy"&&declaring_class=="dia"&&mc121_rust::redstone::owns_placed_by(*state)?=>self.redstone_set_placed_by(world,*position,*state,now,border,events)?,
                    InteractionEvent::RequiredHook { hook, declaring_class, .. } => bail!("{declaring_class}.{hook} executor required"),
                    InteractionEvent::BucketEvaporation { .. } => bail!("ultrawarm bucket evaporation requires the original Math.random particle owner"),
                    InteractionEvent::Write { .. } => bail!("uncommitted write reached the effect queue"),
                }
                Ok(())
            })();
            if let Err(error) = result {
                self.world_interactions.push_front(effect);
                report.blocked = Some(format!("{error:#}"));
                break;
            }
            report.processed += 1;
        }
        report.pending = self.world_interactions.pending_count();
        if report.pending == 0
            && self
                .entity_source
                .as_ref()
                .is_some_and(|source| source.has_pending_edits())
        {
            if let Err(error) = self.complete_native_source_edits(world) {
                report.blocked = Some(format!("native source completion: {error:#}"));
            }
        }
        report
    }
}

fn unobstructed_by_players(
    states: &[PlayerState],
    state: u32,
    at: (i32, i32, i32),
) -> Result<bool> {
    for player in states {
        if player.gamemode == 3 {
            continue;
        }
        let dimensions = player_physics::player_dimensions(player.metadata.pose, 1.)?;
        let bounds = Aabb::player(
            Vec3::new(player.position.x, player.position.y, player.position.z),
            dimensions.width,
            dimensions.height,
        );
        if static_block_intersects(state, at, bounds)
            .context("dynamic placement collision required")?
        {
            return Ok(false);
        }
    }
    Ok(true)
}

/// This transaction has an actual rollback image for the native backend.
/// Imported Anvil multi-file commit is not silently emulated by repeated edits.
pub(super) fn commit_fixture_plan(
    world: &mut WorldBackend,
    plan: &InteractionPlan,
) -> Result<Option<Vec<WorldUpdate>>> {
    let WorldBackend::Fixture(current) = world else {
        return Ok(None);
    };
    for (at, expected) in &plan.read_set {
        ensure!(
            current.block(at.0, at.1, at.2) == *expected,
            "world changed during placement planning"
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
                "staged placement state changed"
            );
            placement::placement_block_name(*state)?;
            if staged.set_block(position.0, position.1, position.2, *state) {
                changes.push(WorldUpdate::Block(BlockChange {
                    x: position.0,
                    y: position.1,
                    z: position.2,
                    state: *state,
                }));
            }
        }
    }
    *current = staged;
    Ok(Some(changes))
}

impl Server {
    /// Replace the old single-state use-item-on branch with this call. Empty
    /// hands can open doors; secondary use skips that block interaction.
    pub(super) async fn interact_world_block(
        &self,
        id: i32,
        hand: i32,
        target: (i32, i32, i32),
        direction: geometry::Direction,
        hit: Vec3,
    ) -> Result<MainInteractionResult> {
        let now=self.clock.read().map_err(|_|anyhow!("world clock poisoned"))?.game_time;
        let border = self
            .border
            .read()
            .map_err(|_| anyhow!("world border lock poisoned"))?
            .bounds(self.epoch.elapsed().as_millis() as i64);
        let border = CollisionBorder {
            min_x: border.min_x,
            min_z: border.min_z,
            max_x: border.max_x,
            max_z: border.max_z,
        };
        let mut world = self.world.write().await;
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("player registry lock poisoned"))?;
        let Some(player) = players.registry.get(id).cloned() else {
            return Ok(MainInteractionResult::Rejected);
        };
        if player.gamemode == 3 {
            return Ok(MainInteractionResult::Pass);
        }
        if !geometry::valid_use_item_hit_location(target, hit)
            || !can_reach_block(player.position, player.metadata, target)
        {
            return Ok(MainInteractionResult::Rejected);
        }
        let container = players
            .containers
            .get(&id)
            .context("player inventory missing")?
            .clone();
        let (stack, both_empty) = container.read(|menu| -> Result<_> {
            Ok((
                menu.inventory.selected_stack(hand)?.clone(),
                menu.inventory.selected_stack(0)?.is_empty()
                    && menu.inventory.selected_stack(1)?.is_empty(),
            ))
        })??;
        let snapshots = players.registry.snapshot();
        let secondary = player.metadata.flags & 0x02 != 0;
        if (!secondary||both_empty)&&players.use_redstone_block(&mut world,target,id,now,border,&self.entity_events)?{return Ok(MainInteractionResult::Applied);}
        let mut query = RuntimeInteractionWorld::new(
            &mut world,
            world::MIN_Y,
            world::MAX_Y + 1,
            border,
            |state, at| unobstructed_by_players(&snapshots, state, at),
        )?;
        if !query.inside_world_border(target)? {
            return Ok(MainInteractionResult::Rejected);
        }
        let result = if !secondary || both_empty {
            world_interactions::plan_use(
                target,
                geometry::horizontal_look_direction(player.position.yaw),
                &mut query,
            )?
        } else {
            InteractionResult::Pass
        };
        let plan = match result {
            InteractionResult::Plan(plan) => plan,
            InteractionResult::Rejected => return Ok(MainInteractionResult::Rejected),
            InteractionResult::Unsupported(_) => return Ok(MainInteractionResult::Unsupported),
            InteractionResult::Pass => {
                if stack.is_empty() {
                    return Ok(MainInteractionResult::Pass);
                }
                if player.gamemode != 1 {
                    return Ok(MainInteractionResult::Unsupported);
                }
                let Some(default) = inventory::item_default_block_state(stack.item_id) else {
                    return Ok(MainInteractionResult::Pass);
                };
                let mut clicked = PlacementContext::new(
                    target,
                    direction,
                    hit,
                    player.position.pitch,
                    player.position.yaw,
                );
                clicked.secondary_use = secondary;
                clicked.replacing_clicked = true;
                let replace = placement::can_replace_with_block(
                    query.block(target)?,
                    default,
                    &clicked,
                    true,
                )?;
                let Some(replace) = replace else {
                    return Ok(MainInteractionResult::Unsupported);
                };
                let delta = direction.step();
                let at = if replace {
                    target
                } else {
                    (
                        target.0.wrapping_add(delta.0),
                        target.1.wrapping_add(delta.1),
                        target.2.wrapping_add(delta.2),
                    )
                };
                let mut context = PlacementContext::new(
                    at,
                    direction,
                    hit,
                    player.position.pitch,
                    player.position.yaw,
                );
                context.secondary_use = secondary;
                context.replacing_clicked = replace;
                if placement::can_replace_with_block(query.block(at)?, default, &context, replace)?
                    != Some(true)
                {
                    return Ok(MainInteractionResult::Rejected);
                }
                match world_interactions::plan_place(stack.item_id, &context, &mut query)? {
                    InteractionResult::Plan(plan) => plan,
                    InteractionResult::Rejected => return Ok(MainInteractionResult::Rejected),
                    InteractionResult::Unsupported(_) => {
                        return Ok(MainInteractionResult::Unsupported)
                    }
                    InteractionResult::Pass => return Ok(MainInteractionResult::Pass),
                }
            }
        };
        self.commit_world_interaction(&mut world, &mut players, id, plan)
    }

    pub(super) async fn edit_world_block(
        &self,
        id: i32,
        at: (i32, i32, i32),
        state: u32,
    ) -> Result<MainInteractionResult> {
        let border = self
            .border
            .read()
            .map_err(|_| anyhow!("world border lock poisoned"))?
            .bounds(self.epoch.elapsed().as_millis() as i64);
        let border = CollisionBorder {
            min_x: border.min_x,
            min_z: border.min_z,
            max_x: border.max_x,
            max_z: border.max_z,
        };
        let mut world = self.world.write().await;
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("player registry lock poisoned"))?;
        let Some(player) = players.registry.get(id) else {
            return Ok(MainInteractionResult::Rejected);
        };
        if player.gamemode != 1 {
            return Ok(MainInteractionResult::Unsupported);
        }
        if !(world::MIN_Y..=world::MAX_Y).contains(&at.1) {
            return Ok(MainInteractionResult::Rejected);
        }
        if !can_reach_block(player.position, player.metadata, at) {
            return Ok(MainInteractionResult::Rejected);
        }
        let snapshots = players.registry.snapshot();
        let mut query = RuntimeInteractionWorld::new(
            &mut world,
            world::MIN_Y,
            world::MAX_Y + 1,
            border,
            |state, at| unobstructed_by_players(&snapshots, state, at),
        )?;
        if !query.inside_world_border(at)? {
            return Ok(MainInteractionResult::Rejected);
        }
        let state = if state == 0 {
            world_interactions::removed_replacement(query.block(at)?)?
        } else {
            state
        };
        let plan = world_interactions::plan_edit(at, state, 3, &mut query)?;
        self.commit_world_interaction(&mut world, &mut players, id, plan)
    }

    pub(super) fn commit_world_interaction(
        &self,
        world: &mut WorldBackend,
        players: &mut LivePlayers,
        id: i32,
        plan: InteractionPlan,
    ) -> Result<MainInteractionResult> {
        if self.shutting_down.load(Ordering::Acquire) {
            return Ok(MainInteractionResult::Rejected);
        }
        if plan.events.iter().any(|e|match e {InteractionEvent::RequiredHook{state,hook,declaring_class,..}=>!(*hook=="setPlacedBy"&&declaring_class=="dia"&&mc121_rust::redstone::owns_placed_by(*state).unwrap_or(false)),_=>false}){return Ok(MainInteractionResult::Unsupported);}
        if let WorldBackend::Fixture(flat)=world {
            for event in &plan.events {if let InteractionEvent::Write{position,old_state,state,..}=event {
                if flat.native_block_entities().preflight_transition(*position,*old_state,*state).is_err(){return Ok(MainInteractionResult::Unsupported);}
            }}
        }
        let source_edit = match players.prepare_native_source_edit(world, &plan) {
            Ok(token) => token,
            Err(_) => {
                let corrections = plan
                    .changed_positions
                    .iter()
                    .map(|p| -> Result<_> {
                        Ok(WorldUpdate::Block(BlockChange {
                            x: p.0,
                            y: p.1,
                            z: p.2,
                            state: world.block(p.0, p.1, p.2)?,
                        }))
                    })
                    .collect::<Result<Vec<_>>>()?;
                self.publish_blocks(corrections);
                return Ok(MainInteractionResult::Rejected);
            }
        };
        players.world_interactions.can_append(plan.events.len())?;
        let Some(mut changes) = commit_fixture_plan(world, &plan)? else {
            return Ok(MainInteractionResult::Unsupported);
        };
        players.observe_native_source_edit(world, source_edit, !changes.is_empty())?;
        for change in &changes {
            if let WorldUpdate::Block(b) = change {
                players
                    .gameplay
                    .observe_block_change((b.x, b.y, b.z), b.state)?;
                players
                    .chunk_runtime
                    .invalidate_block_lighting((b.x, b.y, b.z))?;
            }
        }
        let lighting = players.chunk_runtime.settle_lighting(world);
        if let Some(actor)=players.registry.get(id){
            players.world_interactions.append_from_player(id,actor.position,plan.events);
        }else{ensure!(id==-1,"interaction actor disappeared");players.world_interactions.append(id,plan.events);}

        match lighting {
            Ok(light) => {
                changes.extend(light);
                self.publish_blocks(changes);
                Ok(MainInteractionResult::Applied)
            }
            Err(error) => {
                // The accepted block/effect transaction remains complete. Its
                // actual light work stays pending and cannot acquire a current
                // certificate; preserve ordered publication of the block data.
                self.publish_blocks(changes);
                Err(error)
            }
        }
    }
}
