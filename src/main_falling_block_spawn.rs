//! Actual native Anvil spawning and retained-actor-safe block edits.
//! The original quiescent path is retained; the separate water callback
//! profile uses actual fluid owners. Other active callbacks remain explicit.
use super::*;
use mc121_rust::{
    block_ticks,
    falling_block::FallingBlockState,
    fluid_physics::FluidState,
    placement::{self, BlockPosition},
    tick::TickPriority,
    world_interactions::{self, InteractionEvent, InteractionPlan, RuntimeInteractionWorld},
    workstations::anvil_block::{self, AnvilBlockAction, AnvilBlockHost, ShapeUpdate},
};
use super::main_falling_block_runtime::ConstructedFallingBlock;
#[path = "main_falling_block_callbacks.rs"]
mod callbacks;

/// The new onPlace is represented by the following real schedule event. The
/// old callback is literally inherited or skipped for an unchanged singleton.
pub(super) fn represented_anvil_change(old: u32, new: u32) -> Result<bool> {
    if mc121_rust::falling_block::block::source_delay(new)?.is_none() { return Ok(false); }
    let old = mc121_rust::redstone::block(old)?;
    let new = mc121_rust::redstone::block(new)?;
    ensure!(new.on_place == "diu", "FallingBlock onPlace source declaration changed");
    Ok(old.min == new.min || old.on_remove == "dtb")
}

pub(super) fn validate_anvil_schedule_obligations(plan: &InteractionPlan) -> Result<()> {
    for (index, event) in plan.events.iter().enumerate() {
        if let InteractionEvent::Write { position, state, .. } = event {
            if let Some(delay) = mc121_rust::falling_block::block::source_delay(*state)? {
                ensure!(matches!(plan.events.get(index + 1),
                    Some(InteractionEvent::ScheduleBlockTick(tick))
                    if tick.position == *position && tick.state == *state && tick.delay == delay),
                    "changed falling-block write lacks its actual ordered onPlace schedule");
            }
        }
    }
    Ok(())
}

/// The falling actor stays in its canonical Box. These are disjoint borrows
/// of every other actual owner needed by its movement and landing callbacks.
pub(super) struct ActualFallingBlockEditServices<'a> {
    pub(super) world: &'a mut WorldBackend,
    pub(super) items: &'a mut RuntimeEntities,
    pub(super) gameplay: &'a mut MainItemGameplay,
    pub(super) registry: &'a EntityRegistry,
    pub(super) player_motion: &'a main_player_motion_hooks::MainPlayerMotionRuntime,
    pub(super) chunks: &'a mut MainChunkRuntime,
    pub(super) interactions: &'a mut main_world_interactions::WorldInteractionRuntime,
    pub(super) source: &'a mut Option<NativeEntitySource>,
    pub(super) block_runtime: &'a mut main_block_ticks::MainBlockRuntime,
    pub(super) fluid_runtime: &'a mut main_fluid_simulation::MainFluidRuntime,
    pub(super) random_ticks: &'a mut main_random_block_ticks::MainRandomBlockTicks,
    pub(super) world_revision: &'a mut u64,
    pub(super) context: &'a WorldContext,
    pub(super) rain: Option<&'a mut mc121_rust::rain_physics::FreshFlatPlainsRain>,
    pub(super) changes: &'a broadcast::Sender<WorldUpdate>,
    pub(super) events: &'a broadcast::Sender<PlayerEvent>,
    pub(super) entities: &'a AtomicI32,
    /// Same-lock immutable identity view of the actual XP/Falling stores,
    /// captured before borrowing this actor's retained Box. Drop admission
    /// requires it; absence is not a fabricated empty entity population.
    pub(super) external_entity_uuids: Option<&'a [(i32,[u8;16])]>,
    /// Actual current-FULL query certificate obtained by the entity dispatch.
    /// Spawn-only block callbacks have no movement region and retain None.
    pub(super) query_region: Option<main_chunk_runtime::CurrentChunkQueryRegion>,
    pub(super) now: i64,
    pub(super) border: CollisionBorder,
}

impl ActualFallingBlockEditServices<'_> {
    fn require_source(&mut self) -> Result<()> {
        self.context.dimension.require_runtime_layout()?;
        self.gameplay.require_supported_floor()?;
        self.block_runtime.falling_damage_types()?;
        self.random_ticks.require_healthy()?;
        ensure!(self.interactions.pending_count() == 0,
            "falling edit cannot bypass earlier actual world callbacks");
        let WorldBackend::Fixture(flat) = &*self.world else {
            bail!("native falling edit cannot certify imported world callbacks")
        };
        self.chunks.native_lighting_mut()?.require_current(flat)?;
        let source = self.source.as_ref().context("actual falling native source is absent")?;
        source.require_current_world(self.world)?;
        source.require_fluid_history(self.world, &self.interactions.fluid_shapes)?;
        Ok(())
    }

    pub(super) fn preflight_tick(&mut self, actor: &FallingBlockState) -> Result<()> {
        self.require_source()?;
        actor.validate()?;
        ensure!(mc121_rust::falling_block::block::source_class(actor.block_state)?.is_some(),
            "falling actor has no actual supported block callback class");
        ensure!(actor.block_data.is_none() && actor.vehicle.is_none(),
            "falling block data/vehicle requires its actual retained owner");
        let chunk = (actor.block_position.0.div_euclid(16), actor.block_position.2.div_euclid(16));
        self.chunks.manager.certify_full_region(chunk, chunk)?;
        Ok(())
    }

    pub(super) fn preflight_block_edit(&mut self, position: BlockPosition, state: u32,
        flags: i32) -> Result<InteractionPlan> {
        self.require_source()?;
        ensure!(flags == 3, "falling Level.setBlock must retain source flags3");
        let mut query = RuntimeInteractionWorld::new(self.world, world::MIN_Y,
            world::MAX_Y + 1, self.border,
            |_, _| bail!("falling block edit does not perform entity placement"))?;
        let plan = world_interactions::plan_edit(position, state, 3, &mut query)?;
        self.validate_edit_plan(&plan)?;
        // Complete these fallible native capabilities before a caller can
        // acquire irreversible constructor entropy for the same locked edit.
        let token = self.source.as_ref().context("falling source absent")?
            .prepare_owned_edit_with_block_ticks(self.world,&plan,self.block_runtime)?;
        ensure!(!token.requires_fire_owner()?,
            "falling synchronous edit needs actual fire owner");
        token.require_fluid_history(&self.interactions.fluid_shapes)?;
        self.world_revision.checked_add(1).context("world revision exhausted")?;
        self.random_ticks.prepare_plan(self.world,&plan)?;
        Ok(plan)
    }

    fn validate_edit_plan(&mut self, plan: &InteractionPlan) -> Result<()> {
        if callbacks::has_water_context(self.world,plan)? {
            callbacks::preflight_water_plan(self,plan)
        } else {
            self.validate_quiescent_plan(plan)
        }
    }

    fn validate_quiescent_plan(&mut self, plan: &InteractionPlan) -> Result<()> {
        validate_anvil_schedule_obligations(plan)?;
        let WorldBackend::Fixture(flat) = &*self.world else { bail!("native falling fixture absent") };
        let mut staged = WorldBackend::Fixture(flat.clone());
        main_world_interactions::commit_fixture_plan(&mut staged, plan)?
            .context("falling callback proof requires the actual fixture backend")?;
        for event in &plan.events {
            match event {
                InteractionEvent::Write { position, old_state, state, .. } => {
                    let old = mc121_rust::redstone::block(*old_state)?;
                    let new = mc121_rust::redstone::block(*state)?;
                    ensure!((old.min == new.min || old.on_remove == "dtb")
                        && (new.on_place == "dtb" || represented_anvil_change(*old_state, *state)?),
                        "falling edit needs an active actual block callback owner");
                    for state in [*old_state, *state] {
                        ensure!(FluidState::from_block(state)?.is_empty()
                            && !placement::in_block_tag(state, "minecraft:fire")
                            && mc121_rust::native_block_entities::container_kind(state).is_none()
                            && mc121_rust::native_block_entities::furnace_kind(state).is_none(),
                            "falling edit needs real fluid/fire/block-entity lifecycle");
                    }
                    // Fluid onPlace and all adjacent fluid callbacks are
                    // literally quiescent only after these actual queries.
                    for delta in [(0,0,0),(-1,0,0),(1,0,0),(0,-1,0),(0,1,0),(0,0,-1),(0,0,1)] {
                        let p = (position.0.wrapping_add(delta.0), position.1.wrapping_add(delta.1),
                            position.2.wrapping_add(delta.2));
                        let value = staged.block(p.0,p.1,p.2)?;
                        ensure!(FluidState::from_block(value)?.is_empty()
                            && !placement::in_block_tag(value, "minecraft:fire"),
                            "falling edit reaches an actual fluid/fire callback");
                    }
                }
                InteractionEvent::ScheduleBlockTick(tick) => {
                    ensure!(self.block_runtime.owns_falling_tick(tick.state,tick.delay)?,
                        "falling edit scheduled a foreign actual block callback");
                    self.block_runtime.prepare_halo(self.world, self.chunks,
                        (tick.position.0.div_euclid(16),tick.position.2.div_euclid(16)),self.now)?;
                }
                InteractionEvent::NeighbourNotice { position, source_state } => {
                    // Require literal inherited neighborChanged for this
                    // synchronous profile; do not execute active redstone as
                    // an empty callback simply because its current signal is0.
                    for direction in mc121_rust::redstone::NEIGHBOR_ORDER {
                        let p = mc121_rust::redstone::step(*position,direction);
                        let state = staged.block(p.0,p.1,p.2)?;
                        ensure!(mc121_rust::redstone::block(state)?.neighbor == "dtb",
                            "falling edit needs an active neighborChanged owner");
                    }
                    let mut query = RuntimeInteractionWorld::new(&mut staged,world::MIN_Y,
                        world::MAX_Y+1,self.border,|_,_| bail!("callback proof has no entity placement"))?;
                    ensure!(world_interactions::neighbour_notice_is_quiescent(
                        *position,*source_state,&mut query)?,"unported falling neighbor callback");
                }
                _ => bail!("falling block edit needs its actual non-quiescent effect owner"),
            }
        }
        Ok(())
    }

    pub(super) fn set_block(&mut self, actor: &mut FallingBlockState,
        position: BlockPosition, state: u32, flags: i32) -> Result<bool> {
        self.preflight_tick(actor)?;
        let plan = self.preflight_block_edit(position,state,flags)?;
        self.commit_plan(plan)
    }

    pub(super) fn commit_plan(&mut self, plan: InteractionPlan) -> Result<bool> {
        self.require_source()?;
        self.validate_edit_plan(&plan)?;
        if plan.events.is_empty() { return Ok(false); }
        let token = self.source.as_ref().context("falling source absent")?
            .prepare_owned_edit_with_block_ticks(self.world,&plan,self.block_runtime)?;
        ensure!(!token.requires_fire_owner()?,
            "falling synchronous edit needs actual fire owner");
        token.require_fluid_history(&self.interactions.fluid_shapes)?;
        let next_revision = self.world_revision.checked_add(1).context("world revision exhausted")?;
        self.random_ticks.prepare_plan(self.world,&plan)?;
        let mut changes = main_world_interactions::commit_fixture_plan(self.world,&plan)?
            .context("falling actual world commit requires native fixture")?;
        self.random_ticks.observe_plan(&plan)?;
        self.source.as_mut().expect("validated falling source")
            .observe_owned_commit(token,self.world)?;
        for change in &changes {
            if let WorldUpdate::Block(block) = change {
                self.gameplay.observe_block_change((block.x,block.y,block.z),block.state)?;
                self.chunks.invalidate_block_lighting((block.x,block.y,block.z))?;
            }
        }
        // Preserve the original quiescent path and execute the separately
        // certified water callback at its real neighbor phase. Both schedulers
        // consume this SAME fluid runtime's sub-tick counter on every attempt.
        let water_callbacks = callbacks::has_water_context(self.world,&plan)?;
        for event in &plan.events {
            if water_callbacks { callbacks::execute_water_event(self,event)?; }
            if let InteractionEvent::ScheduleBlockTick(tick) = event {
                self.block_runtime.mark_used();
                self.source.as_mut().expect("validated source").observe_block_scheduler_use();
                block_ticks::schedule(&mut self.block_runtime.queue,tick.position,tick.state,
                    self.now,tick.delay,TickPriority::Normal,&mut self.fluid_runtime.sub_tick_order)?;
            }
        }
        let changed = changes.iter().any(|update| matches!(update,WorldUpdate::Block(_)));
        changes.extend(self.chunks.settle_lighting(self.world)?);
        self.source.as_mut().expect("validated falling source").complete_owned_edits(
            self.world,self.chunks.native_lighting_mut()?,&self.interactions.fluid_shapes,
            self.interactions.pending_count())?;
        *self.world_revision = next_revision;
        for update in changes { let _ = self.changes.send(update); }
        Ok(changed)
    }
}

/// The scheduled block callback can retain the full real LivePlayers borrow;
/// later entity ticks use the disjoint services above instead.
struct NativeAnvilScheduledHost<'a> {
    server: &'a Server,
    world: &'a mut WorldBackend,
    players: &'a mut LivePlayers,
    now: i64,
    border: CollisionBorder,
}
impl NativeAnvilScheduledHost<'_> {
    fn preflight_spawn(&mut self, position: BlockPosition, state: u32) -> Result<()> {
        self.players.require_current_native_source(self.world)?;
        self.players.block_runtime.falling_damage_types()?;
        let id = self.server.entities.load(Ordering::Relaxed);
        ensure!(id > 0 && id < i32::MAX,"shared falling entity counter exhausted");
        self.players.falling_blocks.preflight_registration(&self.players.item_runtime,id)?;
        let replacement = main_falling_block_world::original_fluid_replacement(state)?;
        self.services().preflight_block_edit(position,replacement,3)?;
        Ok(())
    }
    fn services(&mut self) -> ActualFallingBlockEditServices<'_> {
        let LivePlayers { item_runtime,gameplay,registry,player_motion_runtime,chunk_runtime,
            world_interactions,entity_source,block_runtime,fluid_runtime,random_block_ticks,
            world_revision,world_context,rain_context,.. } = &mut *self.players;
        ActualFallingBlockEditServices {
            world:self.world,items:item_runtime,gameplay,registry,player_motion:player_motion_runtime,
            chunks:chunk_runtime,interactions:world_interactions,source:entity_source,
            block_runtime,fluid_runtime,
            random_ticks:random_block_ticks,world_revision,context:world_context,
            rain:rain_context.as_mut(),changes:&self.server.changes,
            events:&self.server.entity_events,entities:&self.server.entities,
            external_entity_uuids:None,
            query_region:None,
            now:self.now,border:self.border,
        }
    }
}
impl AnvilBlockHost for NativeAnvilScheduledHost<'_> {
    type Block = u32;
    type BlockState = u32;
    type FallingEntity = ConstructedFallingBlock;
    fn preflight(&mut self, action:AnvilBlockAction, block:&u32,state:&u32,
        position:BlockPosition)->Result<()> {
        self.players.require_current_native_source(self.world)?;
        self.players.block_runtime.falling_damage_types()?;
        ensure!(placement::placement_block_min_state(*state)?==*block
            && placement::anvil_falling_delay(*state)?==Some(2),"foreign Anvil callback identity");
        ensure!(self.players.block_runtime.queue.chunk(block_ticks::pos(position).chunk()).is_some(),
            "actual Anvil BLOCK queue is not loaded");
        if action==AnvilBlockAction::FallingEntity { self.preflight_spawn(position,*state)?; }
        Ok(())
    }
    fn schedule_block_tick(&mut self,p:BlockPosition,b:&u32,delay:i32)->Result<()> {
        self.players.block_runtime.mark_used();
        self.players.entity_source.as_mut().context("native falling source absent")?
            .observe_block_scheduler_use();
        block_ticks::schedule(&mut self.players.block_runtime.queue,p,*b,self.now,delay,
            TickPriority::Normal,&mut self.players.fluid_runtime.sub_tick_order)?;Ok(())
    }
    fn superclass_update_shape(&mut self,update:ShapeUpdate<'_,u32>)->Result<u32> {
        placement::placement_block_name(*update.state)?;Ok(*update.state)
    }
    fn block_state(&mut self,p:BlockPosition)->Result<u32>{self.world.block(p.0,p.1,p.2)}
    fn state_is_air(&mut self,s:&u32)->Result<bool>{Ok(mc121_rust::lighting::state_properties(*s)?.is_air())}
    fn state_is_fire_tag(&mut self,s:&u32)->Result<bool>{placement::placement_block_name(*s)?;Ok(placement::in_block_tag(*s,"minecraft:fire"))}
    fn state_is_liquid(&mut self,s:&u32)->Result<bool>{main_falling_block_world::state_is_liquid(*s)}
    fn state_can_be_replaced(&mut self,s:&u32)->Result<bool>{main_falling_block_world::state_can_be_replaced(*s)}
    fn minimum_build_height(&mut self)->Result<i32>{Ok(self.players.world_context.dimension.min_y)}
    fn fall_block(&mut self,p:BlockPosition,s:&u32)->Result<ConstructedFallingBlock> {
        self.server.construct_native_falling_block_locked(self.world,self.players,p,*s,self.now,self.border)
    }
    fn set_hurts_entities(&mut self,e:&mut ConstructedFallingBlock,amount:f32,max:i32)->Result<()> {
        let state=self.players.falling_blocks.object_state_mut(e)?;
        state.hurt_entities=true;state.fall_damage_per_distance=amount;state.fall_damage_max=max;Ok(())
    }
}
impl Server {
    /// Pure capability preparation for the same actual retained native fall.
    /// It acquires no constructor RNG and reserves no entity ID.
    pub(super) fn preflight_native_falling_block_locked(&self,world:&mut WorldBackend,
        players:&mut LivePlayers,p:BlockPosition,state:u32,now:i64,border:CollisionBorder)->Result<()> {
        NativeAnvilScheduledHost { server:self,world,players,now,border }.preflight_spawn(p,state)
    }

    /// Genuine shared FallingBlockEntity.fall capability. The caller supplies
    /// the actual source block callback; this helper does not apply Anvil's
    /// later setHurtsEntities override or create a second ID/RNG/store owner.
    pub(super) fn construct_native_falling_block_locked(&self,world:&mut WorldBackend,
        players:&mut LivePlayers,p:BlockPosition,original_state:u32,now:i64,border:CollisionBorder)
        ->Result<ConstructedFallingBlock> {
        let mut host=NativeAnvilScheduledHost { server:self,world,players,now,border };
        host.preflight_spawn(p,original_state)?;
        let carried = main_falling_block_world::carried_state_for_fall(original_state)?;
        let id = host.server.entities.try_update(Ordering::Relaxed,Ordering::Relaxed,
            |next| next.checked_add(1)).map_err(|_| anyhow!("shared falling entity ID exhausted"))?;
        let random = random_seed::create_random()?;
        let state = FallingBlockState::new(id,random,carried,
            main_falling_block_world::fall_spawn_position(p))?;
        let mut object = ConstructedFallingBlock::new(state)?;
        // Original fluid replacement is queried after constructing the SAME
        // entity. The Level.setBlock boolean is deliberately ignored.
        let replacement = main_falling_block_world::original_fluid_replacement(original_state)?;
        let plan = host.services().preflight_block_edit(p,replacement,3)?;
        let _ = host.services().commit_plan(plan)?;
        let uuid = host.players.falling_blocks.object_state(&object)?.uuid;
        let duplicate = host.players.registry.get_by_uuid(&uuid).is_some()
            || host.players.item_runtime.item_states().any(|s|!s.removed&&s.uuid==uuid)
            || host.players.experience_runtime.physical_states().any(|s|!s.removed&&s.uuid==uuid)
            || host.players.falling_blocks.contains_uuid(&uuid);
        // This is the real source's UUID admission boolean, not a fabricated
        // successful add. Rejection leaves the same unregistered object alive
        // in its token and does not refund the replaced origin block.
        if !duplicate {
            host.players.falling_blocks.register_constructed(&mut host.players.item_runtime,&mut object)?;
        }
        Ok(object)
    }
    pub(super) fn tick_native_anvil_block_locked(&self,world:&mut WorldBackend,
        players:&mut LivePlayers,p:BlockPosition,state:u32,now:i64,border:CollisionBorder)->Result<()> {
        let block=placement::placement_block_min_state(state)?;
        anvil_block::scheduled_tick(&mut NativeAnvilScheduledHost {
            server:self,world,players,now,border },&block,&state,p)?;
        Ok(())
    }
}
