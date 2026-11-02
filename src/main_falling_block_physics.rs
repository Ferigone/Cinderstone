//! Native falling-block movement over actual world, section and gameplay owners.
use super::*;
use super::{main_falling_block_damage::{MainFallingDamageHost, NativeFallingDamageTypes},
    main_falling_block_clip_admission::PreparedNextTickClip,
    main_falling_block_runtime::FallingBlockPublication,
    main_falling_block_powder::{self, NativeConcretePowderOwner, PowderLandingAdmission}};
use mc121_rust::{
    entity_move::{self, EntityMoveWorld, EntityMoveContext, MoverType, MovementEmission, ProfileEvent},
    entity_state::{java_floor, EntityPhysicalState},
    falling_block::{FallingBlockState, damage::{self, FallingBlockDamageActor}},
    fluid_physics::{FluidKind, FluidWorld, RuntimeFluidWorld},
    geometry::{self, EntityCollisionContext, VoxelShape},
    item_host::RuntimeGameplayHooks,
    player_physics::{CollisionQuery, WorldCollision},
    rain_physics::RainExposure,
    runtime_collision::{CollisionActor, CollisionEntity},
    runtime_item_world::{ItemWorldFacts, RainAvailability},
    runtime_entity_index::EntityKind,
};

/// Only references to the genuine nonphysical actor fields used by damage.
/// The physical reference supplied during Entity.move remains the SAME field.
struct FallingDamageFields<'a> {
    random: &'a mut LegacyRandomSource,
    block_state: &'a mut u32,
    cancel_drop: &'a mut bool,
    hurt_entities: &'a mut bool,
    amount: &'a mut f32,
    maximum: &'a mut i32,
}
struct FallingDamageBorrow<'a, 'fields> {
    physical: &'a mut EntityPhysicalState,
    fields: &'a mut FallingDamageFields<'fields>,
}
impl FallingBlockDamageActor for FallingDamageBorrow<'_, '_> {
    fn physical(&self) -> &EntityPhysicalState { self.physical }
    fn physical_mut(&mut self) -> &mut EntityPhysicalState { self.physical }
    fn random_mut(&mut self) -> &mut LegacyRandomSource { self.fields.random }
    fn block_state(&self) -> u32 { *self.fields.block_state }
    fn set_block_state(&mut self, state: u32) { *self.fields.block_state = state; }
    fn hurt_entities(&self) -> bool { *self.fields.hurt_entities }
    fn fall_damage_per_distance(&self) -> f32 { *self.fields.amount }
    fn fall_damage_max(&self) -> i32 { *self.fields.maximum }
    fn set_cancel_drop(&mut self, value: bool) { *self.fields.cancel_drop = value; }
}

struct FallingMoveWorld<'a, 'level, 'fields> {
    services: &'a mut ActualFallingBlockEditServices<'level>,
    publication: &'a mut FallingBlockPublication,
    damage_types: &'a NativeFallingDamageTypes,
    fields: FallingDamageFields<'fields>,
    actor: CollisionActor,
    colliders: Vec<CollisionEntity>,
    facts: ItemWorldFacts,
    query_region: main_chunk_runtime::CurrentChunkQueryRegion,
}
impl FallingMoveWorld<'_, '_, '_> {
    fn context(state: &EntityPhysicalState) -> EntityCollisionContext {
        EntityCollisionContext { bottom_y: state.position.y,
            descending: state.shared_flags & 2 != 0, fall_distance: state.fall_distance,
            is_falling_block: true, can_walk_on_powder_snow: false }
    }
    fn collision(&mut self) -> RuntimeCollision<'_> {
        RuntimeCollision::new(self.services.world, self.actor, &self.colliders, self.facts.border)
    }
    fn cells(bounds: Aabb, inward: f64) -> Result<((i32,i32,i32),(i32,i32,i32))> {
        ensure!([bounds.min_x,bounds.min_y,bounds.min_z,bounds.max_x,bounds.max_y,bounds.max_z]
            .iter().all(|v| v.is_finite()), "nonfinite falling block query");
        let min=(java_floor(bounds.min_x+inward),java_floor(bounds.min_y+inward),java_floor(bounds.min_z+inward));
        let max=(java_floor(bounds.max_x-inward),java_floor(bounds.max_y-inward),java_floor(bounds.max_z-inward));
        let volume=[i64::from(max.0)-i64::from(min.0)+1,i64::from(max.1)-i64::from(min.1)+1,
            i64::from(max.2)-i64::from(min.2)+1].into_iter()
            .try_fold(1_u64,|n,v|n.checked_mul(v.max(0)as u64));
        ensure!(volume.is_some_and(|n|n<=1_048_576), "falling query exceeds actual bounded region");
        Ok((min,max))
    }
    fn inherited(block:u32, name:&str, class:&str, expected:&str)->Result<()> {
        ensure!(class==expected,"unported falling {name} callback {class} for state{block}");Ok(())
    }
}
impl WorldCollision for FallingMoveWorld<'_, '_, '_> {
    fn block_collisions(&mut self,q:CollisionQuery)->Result<Vec<VoxelShape>>{self.collision().block_collisions(q)}
    fn entity_collisions(&mut self,q:CollisionQuery)->Result<Vec<VoxelShape>>{self.collision().entity_collisions(q)}
    fn border_collision(&mut self,q:CollisionQuery)->Result<Option<VoxelShape>>{self.collision().border_collision(q)}
    fn all_blocks_air(&mut self,b:Aabb)->Result<bool>{self.collision().all_blocks_air(b)}
}
impl EntityMoveWorld for FallingMoveWorld<'_, '_, '_> {
    fn profile(&mut self,event:ProfileEvent){
        let mut frame=self.services.gameplay.frame_current(self.services.registry,&self.services.chunks.manager,true,self.query_region);
        RuntimeGameplayHooks::profile(&mut frame,event);
    }
    fn game_time(&mut self)->Result<i64>{Ok(self.facts.game_time)}
    fn set_position(&mut self,s:&mut EntityPhysicalState,p:Vec3)->Result<()>{entity_move::set_base_position(s,p,self)}
    fn position_changed(&mut self,s:&mut EntityPhysicalState)->Result<()>{
        ensure!(s.entity_id==self.actor.entity_id,"falling move actor changed");
        self.actor.position=s.position_vec();
        self.publication.position_changed(s,self.services.items)
    }
    fn reset_fall_distance(&mut self,s:&mut EntityPhysicalState)->Result<()>{s.fall_distance=0.;Ok(())}
    fn set_on_ground_with_movement(&mut self,s:&mut EntityPhysicalState,on:bool,m:Vec3)->Result<()>{
        entity_move::update_supporting_block(s,on,Some(m),self)
    }
    fn block_speed_factor(&mut self,s:&EntityPhysicalState)->Result<f32>{entity_move::block_speed_factor(s,self)}
    fn maybe_back_off_from_edge(&mut self,_:&mut EntityPhysicalState,m:Vec3,_:MoverType)->Result<Vec3>{
        // FallingBlockEntity inherits Entity's literal identity override.
        Ok(m)
    }
    fn fall_damage_resetting_ray(&mut self,_:&EntityPhysicalState,from:Vec3,to:Vec3)->Result<bool>{
        geometry::traverse_blocks(from,to,|p|{
            let r=(||->Result<bool>{
                let block=self.services.world.block(p.0,p.1,p.2)?;
                let fluid=RuntimeFluidWorld{world:self.services.world}.fluid(p)?;
                let block_hit=entity_move::block_facts(block)?.fall_damage_resetting()
                    &&VoxelShape::from_aabb(Aabb::new(0.,0.,0.,1.,1.,1.)).clip(from,to,p).is_some();
                let fluid_hit=if fluid.kind==FluidKind::Water{
                    self.services.interactions.fluid_shapes.shape(&mut RuntimeFluidWorld{world:self.services.world},p,fluid)?
                        .clip(from,to,p).is_some()
                }else{false};
                Ok(block_hit||fluid_hit)
            })();
            match r{Ok(false)=>None,r=>Some(r)}
        }).unwrap_or(Ok(false))
    }
    fn horizontal_collision_minor(&mut self,_:&EntityPhysicalState,_:Vec3)->Result<bool>{Ok(false)}
    fn supporting_block(&mut self,s:&EntityPhysicalState,b:Aabb)->Result<Option<(i32,i32,i32)>>{
        self.actor.position=s.position_vec();
        let q=CollisionQuery{actor_id:s.entity_id,bounds:b,context:Self::context(s)};
        mc121_rust::entity_support::find_supporting_block(&mut self.collision(),q)
    }
    fn block_state(&mut self,p:(i32,i32,i32))->Result<u32>{self.services.world.block(p.0,p.1,p.2)}
    fn check_fall_damage(&mut self,s:&mut EntityPhysicalState,dy:f64,on:bool,block:u32,_:(i32,i32,i32))->Result<()>{
        if on{
            if s.fall_distance>0.{
                let hooks=entity_move::block_facts(block)?.callback_classes()?;
                Self::inherited(block,"fallOn",&hooks.fall_on,"dfy")?;
                ensure!(!self.facts.has_passengers,"actual falling passengers require their causeFallDamage owner");
                let distance=s.fall_distance;
                let mut actor=FallingDamageBorrow{physical:s,fields:&mut self.fields};
                let mut host=MainFallingDamageHost{items:self.services.items,registry:self.services.registry,
                    player_motion:self.services.player_motion,actor:self.publication.handle,
                    damage_types:self.damage_types};
                damage::cause_fall_damage(&mut actor,distance,&mut host)?;
                let affected=if let Some(p)=s.supporting_block{self.services.world.block(p.0,p.1,p.2)?}else{block};
                let mut frame=self.services.gameplay.frame_current(self.services.registry,&self.services.chunks.manager,true,self.query_region);
                frame.hit_ground(s,affected)?;
            }
            self.reset_fall_distance(s)?;
        }else if dy<0.{s.fall_distance-=dy as f32;}
        Ok(())
    }
    fn after_fall_on_block(&mut self,s:&mut EntityPhysicalState,block:u32)->Result<()>{
        Self::inherited(block,"afterFall",&entity_move::block_facts(block)?.callback_classes()?.after_fall,"dfy")?;
        entity_move::default_after_fall_on_block(s);Ok(())
    }
    fn step_on(&mut self,_:&mut EntityPhysicalState,b:u32,_:(i32,i32,i32))->Result<()>{
        Self::inherited(b,"stepOn",&entity_move::block_facts(b)?.callback_classes()?.step_on,"dfy")
    }
    fn movement_emission(&mut self,_:&mut EntityPhysicalState,e:MovementEmission,_:Vec3,_:Vec3,_:(i32,i32,i32),_:u32)->Result<()>{
        ensure!(e==MovementEmission::None,"FallingBlockEntity movement emission is not NONE");Ok(())
    }
    fn try_check_inside_blocks(&mut self,s:&mut EntityPhysicalState)->Result<()>{
        let(min,max)=Self::cells(s.bounds,1e-7)?;
        if max.1<self.facts.minimum_build_height||min.1>mc121_rust::world::MAX_Y{return Ok(())}
        if !(RuntimeFluidWorld{world:self.services.world}).has_chunks_at(min.0,min.2,max.0,max.2)?{return Ok(())}
        // The native dimension's actual existing FULL futures certify access;
        // this is not an invented successful hasChunksAt for arbitrary worlds.
        self.services.chunks.manager.certify_full_region((min.0.div_euclid(16),min.2.div_euclid(16)),
            (max.0.div_euclid(16),max.2.div_euclid(16)))?;
        for x in min.0..=max.0{for y in min.1..=max.1{for z in min.2..=max.2{
            if s.removed{return Ok(())}
            let block=self.services.world.block(x,y,z)?;
            let callbacks=entity_move::block_facts(block)?.callback_classes()?;
            if callbacks.entity_inside=="dfm" {
                main_falling_block_redstone::entity_inside(self.services,self.publication,s,(x,y,z))?;
            } else {
                Self::inherited(block,"entityInside",&callbacks.entity_inside,"dtb")?;
            }
            // FallingBlockEntity inherits Entity.onInsideBlock's literal
            // empty body after the actual Block.entityInside callback.
        }}}
        Ok(())
    }
    fn any_fire_or_lava(&mut self,b:Aabb)->Result<bool>{
        let(min,max)=Self::cells(b,0.)?;
        for z in min.2..=max.2{for y in min.1..=max.1{for x in min.0..=max.0{
            if entity_move::block_facts(self.services.world.block(x,y,z)?)?.fire_or_lava(){return Ok(true)}
        }}}
        Ok(false)
    }
    fn water_rain_or_bubble(&mut self,s:&EntityPhysicalState)->Result<bool>{
        if s.was_touching_water{return Ok(true)}
        match self.facts.rain{
            RainAvailability::NotRaining=>{},
            RainAvailability::ExposureUnavailable=>{
                let (lighting,manager)=self.services.chunks.native_lighting_parts()?;
                let owner=self.services.rain.as_deref_mut()
                    .context("actual falling rain exposure owner unavailable")?;
                let mut exposure=mc121_rust::native_rain::NativeFlatPlainsRainFrame::new(owner,lighting,manager);
                if exposure.is_in_rain(self.services.world,s)?{return Ok(true)}
            }
        }
        let p=s.block_position;
        Ok(entity_move::block_facts(self.services.world.block(p.0,p.1,p.2)?)?.bubble_column())
    }
    fn play_fire_extinguished_sound(&mut self,s:&mut EntityPhysicalState)->Result<()>{
        let pitch=1.6_f32+(self.fields.random.next_f32()-self.fields.random.next_f32())*0.4_f32;
        if !s.silent{
            // FallingBlockEntity inherits Entity.getSoundSource (NEUTRAL6),
            // rather than ItemEntity's AMBIENT8 override.
            let origin=s.position_vec();
            let mut fields=Writer::new();
            fields.varint(577+1).varint(6)
                .i32((origin.x*8.)as i32).i32((origin.y*8.)as i32).i32((origin.z*8.)as i32)
                .f32(0.7).f32(pitch).i64(self.services.items.next_sound_seed()?);
            let recipients=self.services.registry.snapshot().into_iter().filter(|p|
                Vec3::new(p.position.x,p.position.y,p.position.z).subtract(origin).length_squared()<256.)
                .map(|p|p.entity_id).collect::<Vec<_>>();
            let _=self.services.events.send(PlayerEvent::WorldAudience{
                recipients:Arc::new(recipients),packets:Arc::new(vec![Packet{id:0x68,payload:fields.into_inner()}])});
        }
        Ok(())
    }
}

fn move_falling_block(actor:&mut FallingBlockState,movement:Vec3,
    services:&mut ActualFallingBlockEditServices<'_>, publication:&mut FallingBlockPublication,
    damage_types:&NativeFallingDamageTypes)->Result<()>{
    actor.validate()?;
    ensure!(services.items.entity_kind(publication.handle)==Some(EntityKind::FallingBlock)
        &&actor.entity_id==publication.handle.id,"falling motion actual handle mismatch");
    ensure!(matches!(services.world,WorldBackend::Fixture(_)),"native falling imported-world motion unavailable");
    services.gameplay.require_supported_floor()?;
    let facts=services.context.item_facts(services.now,services.border,false)?;
    let query_region=services.query_region.context("falling motion has no actual current query region")?;
    ensure!(actor.width==0.98_f32&&actor.height==0.98_f32,"actual falling dimensions mismatch");
    // All current native indexed classes inherit canBeCollidedWith=false.
    // Actually walk that same index; a future new class must update this match.
    let _candidates=services.items.query_native_entities(actor.bounds.expand_towards(movement).inflate(Vec3::new(1e-7,1e-7,1e-7)),actor.entity_id,
        |_handle,kind|match kind{EntityKind::Player|EntityKind::Item|EntityKind::ExperienceOrb|EntityKind::FallingBlock=>Ok(None)})?;
    let physical=&mut actor.physical;
    let fields=FallingDamageFields{random:&mut actor.random,block_state:&mut actor.block_state,
        cancel_drop:&mut actor.cancel_drop,hurt_entities:&mut actor.hurt_entities,
        amount:&mut actor.fall_damage_per_distance,maximum:&mut actor.fall_damage_max};
    let mut world=FallingMoveWorld{actor:CollisionActor{entity_id:physical.entity_id,position:physical.position_vec()},
        services,publication,damage_types,fields,colliders:Vec::new(),facts,query_region};
    entity_move::move_entity(physical,MoverType::SelfMove,movement,
        EntityMoveContext{step_height:0.,descending:physical.shared_flags&2!=0,is_falling_block:true,
            can_walk_on_powder_snow:false,movement_emission:MovementEmission::None},&mut world)?;
    Ok(())
}

use super::main_falling_block_spawn::ActualFallingBlockEditServices;
use mc121_rust::{
    falling_block::tick::{self, BlockEntityLoadStatus, FallingBlockTickHost},
    placement::{self, BlockPosition, SurvivalResult},
};

#[derive(Deserialize)]
struct FallingCatalogue { blocks: Vec<FallingBlockFact> }
#[derive(Deserialize)]
struct FallingBlockFact {
    class: String,
    min: u32,
    max: u32,
    item: i32,
    replacement_class: String,
    block_entity_type: Option<String>,
}
fn falling_block_fact(state: u32) -> Result<&'static FallingBlockFact> {
    static CATALOGUE: std::sync::OnceLock<FallingCatalogue> = std::sync::OnceLock::new();
    let catalogue = CATALOGUE.get_or_init(|| serde_json::from_str(include_str!(
        "../fixtures/placement-1.21.json")).expect("pinned falling block catalogue"));
    placement::placement_block_name(state)?;
    catalogue.blocks.iter().find(|block| (block.min..=block.max).contains(&state))
        .context("falling state has no actual catalogue owner")
}

/// One synchronous source tick, retaining the canonical actor Box and borrowing
/// all other owners from the real Level interval. No state is detached or
/// installed into an ItemEntity proxy. The dispatcher calls this only for an
/// actual member of the captured Level EntityTickList.
pub(super) fn tick_falling_block(
    actor: &mut FallingBlockState,
    publication: &mut FallingBlockPublication,
    services: ActualFallingBlockEditServices<'_>,
    damage_types: &NativeFallingDamageTypes,
) -> Result<()> {
    tick_falling_block_inner(actor, publication, services, damage_types, None)
}

/// The coordinator supplies its once-installed actual configuration owner.
/// This additive entry does not install the owner or admit a powder class.
pub(super) fn tick_falling_block_with_powder(
    actor: &mut FallingBlockState,
    publication: &mut FallingBlockPublication,
    services: ActualFallingBlockEditServices<'_>,
    damage_types: &NativeFallingDamageTypes,
    powder_owner: &NativeConcretePowderOwner,
) -> Result<()> {
    tick_falling_block_inner(actor, publication, services, damage_types, Some(powder_owner))
}

fn tick_falling_block_inner(
    actor: &mut FallingBlockState,
    publication: &mut FallingBlockPublication,
    mut services: ActualFallingBlockEditServices<'_>,
    damage_types: &NativeFallingDamageTypes,
    powder_owner: Option<&NativeConcretePowderOwner>,
) -> Result<()> {
    let is_powder = falling_block_fact(actor.block_state)?.class == "dhk";
    if is_powder {
        powder_owner.context("powder tick lacks its installed actual configuration/Block owner")?
            .binding_for_state(actor.block_state)?
            .context("powder tick is absent from its retained actual Block association registry")?;
    }
    services.preflight_tick(actor)?;
    ensure!(services.items.entity_kind(publication.handle) == Some(EntityKind::FallingBlock)
        && actor.entity_id == publication.handle.id && !actor.removed,
        "falling tick is not the actual live registered generation");
    services.query_region.context("falling tick requires its actual current query region")?;
    services.external_entity_uuids.context("falling tick requires its actual external identity view")?;
    ensure!(actor.portal_process.is_none(), "active falling portal processing requires its actual owner");
    // Issue the actual capability receipt as the final step before the one
    // source begin-level transition. The core's first preflight consumes this
    // same actor/owner phase; it does not rescan world shapes or prime caches.
    let powder_clip = if is_powder {
        Some(super::main_falling_block_clip_admission::preflight_next_tick_clip(
            &mut services, publication, actor)?)
    } else {None};
    actor.physical.begin_level_tick();
    let mut host = MainFallingTickHost { services, publication, damage_types,
        powder_owner, powder_clip, powder_landing: None };
    let result = tick::tick(actor, &mut host);
    let complete = host.powder_landing.as_ref().map(PowderLandingAdmission::verify_complete)
        .transpose().map(|_| ());
    result.and(complete)
}

struct MainFallingTickHost<'a, 'level> {
    services: ActualFallingBlockEditServices<'level>,
    publication: &'a mut FallingBlockPublication,
    damage_types: &'a NativeFallingDamageTypes,
    powder_owner: Option<&'a NativeConcretePowderOwner>,
    powder_clip: Option<PreparedNextTickClip>,
    powder_landing: Option<PowderLandingAdmission>,
}
impl MainFallingTickHost<'_, '_> {
    fn level_event(&mut self, event: i32, position: BlockPosition) -> Result<()> {
        self.services.gameplay.require_supported_floor()?;
        let origin = Vec3::new(f64::from(position.0), f64::from(position.1), f64::from(position.2));
        let recipients = self.services.registry.snapshot().into_iter().filter(|player|
            Vec3::new(player.position.x, player.position.y, player.position.z)
                .subtract(origin).length_squared() < 64. * 64.)
            .map(|player| player.entity_id).collect::<Vec<_>>();
        // ServerLevel.levelEvent uses PlayerList's integer BlockPos origin and
        // no excluded player. It does not draw from either Level RNG stream.
        let packet = mc121_rust::item_effects::level_event_packet(event, position, 0, false);
        let _ = self.services.events.send(PlayerEvent::WorldAudience {
            recipients: Arc::new(recipients), packets: Arc::new(vec![packet]),
        });
        Ok(())
    }
}

impl FallingBlockTickHost for MainFallingTickHost<'_, '_> {
    type Block = u32;
    type BlockEntity = std::convert::Infallible;
    fn preflight_tick(&mut self, actor: &FallingBlockState) -> Result<()> {
        if falling_block_fact(actor.block_state)?.class == "dhk" {
            self.powder_owner.context("powder tick has no retained actual Block owner")?
                .binding_for_state(actor.block_state)?
                .context("powder preflight is absent from its retained actual Block registry")?;
            return self.powder_clip.take().context("powder preflight lacks its actual clip admission phase")?
                .validate_after_begin(&self.services, self.publication, actor);
        }
        self.services.preflight_tick(actor)
    }
    fn state_is_air(&mut self, state: u32) -> Result<bool> {
        Ok(mc121_rust::lighting::state_properties(state)?.is_air())
    }
    fn block_of_state(&mut self, state: u32) -> Result<u32> {
        placement::placement_block_min_state(state)
    }
    fn move_self(&mut self, actor: &mut FallingBlockState, movement: Vec3) -> Result<()> {
        move_falling_block(actor,movement,&mut self.services,self.publication,self.damage_types)
    }
    fn handle_portal(&mut self, actor: &mut FallingBlockState) -> Result<()> {
        // Entity.handlePortal in a real ServerLevel calls processPortalCooldown
        // even when its actual PortalProcessor field is null.
        if actor.portal_cooldown > 0 { actor.portal_cooldown -= 1; }
        ensure!(actor.portal_process.is_none(), "active falling portal processor is unavailable");
        Ok(())
    }
    fn is_client_side(&mut self, _actor: &FallingBlockState) -> Result<bool> {
        ensure!(matches!(self.services.world, WorldBackend::Fixture(_)),
            "falling tick requires its real native ServerLevel");
        Ok(false)
    }
    fn is_concrete_powder(&mut self, state: u32) -> Result<bool> {
        if falling_block_fact(state)?.class == "dhk" {
            self.powder_owner.context("powder identity lacks its retained actual Block owner")?
                .binding_for_state(state)?.context("powder Block is not in the retained source association registry")?;
            return Ok(true);
        }
        ensure!(mc121_rust::falling_block::block::source_class(state)?.is_some(),
            "falling subclass requires its actual concrete-powder owner");
        // Actual AnvilBlock and ColoredFallingBlock are distinct from powder.
        // Foreign/special subclasses are not guessed.
        Ok(false)
    }
    fn fluid_is_water(&mut self, position: BlockPosition) -> Result<bool> {
        let block = self.services.world.block(position.0, position.1, position.2)?;
        Ok(mc121_rust::fluid_physics::FluidState::from_block(block)?.kind == FluidKind::Water)
    }
    fn clip_concrete_water(&mut self, actor: &FallingBlockState, from: Vec3, to: Vec3)
        -> Result<Option<BlockPosition>> {
        super::main_falling_block_clip::clip_collider_source_only(
            &mut self.services, self.publication, actor, from, to)
            .map(|hit| hit.map(|hit| hit.block_position))
    }
    fn block_state(&mut self, position: BlockPosition) -> Result<u32> {
        self.services.world.block(position.0, position.1, position.2)
    }
    fn is_moving_piston(&mut self, state: u32) -> Result<bool> {
        Ok(placement::placement_block_name(state)? == "minecraft:moving_piston")
    }
    fn can_replace_for_falling(&mut self, state: u32, _position: BlockPosition) -> Result<bool> {
        ensure!(falling_block_fact(state)?.replacement_class == "dtb",
            "falling DirectionalPlaceContext needs an actual overridden replacement owner");
        // Actual inherited canBeReplaced(context) sees ItemStack.EMPTY; its
        // item-identity condition therefore short-circuits to the real flag.
        super::main_falling_block_world::state_can_be_replaced(state)
    }
    fn falling_is_free(&mut self, state: u32) -> Result<bool> {
        super::main_falling_block_world::falling_is_free(state)
    }
    fn can_survive(&mut self, state: u32, position: BlockPosition) -> Result<bool> {
        let world = &mut *self.services.world;
        let mut query = |p: BlockPosition| world.block(p.0, p.1, p.2);
        match placement::can_survive(state, position, &mut query)? {
            SurvivalResult::Supported(result) => Ok(result),
            SurvivalResult::Unsupported(owner) => bail!("falling survival owner unavailable: {owner}"),
        }
    }
    fn has_waterlogged_property(&mut self, state: u32) -> Result<bool> {
        placement::placement_block_name(state)?;
        Ok(mc121_rust::inventory::block_state_property(state, "waterlogged").is_some())
    }
    fn fluid_is_source_water(&mut self, position: BlockPosition) -> Result<bool> {
        let state = self.services.world.block(position.0, position.1, position.2)?;
        let fluid = mc121_rust::fluid_physics::FluidState::from_block(state)?;
        Ok(fluid.kind == FluidKind::Water && fluid.source)
    }
    fn with_waterlogged(&mut self, state: u32, value: bool) -> Result<u32> {
        mc121_rust::inventory::block_state_with_properties(state,
            &BTreeMap::from([("waterlogged".into(), value.to_string())]))
    }
    fn set_block(&mut self, actor: &mut FallingBlockState, position: BlockPosition,
        state: u32, flags: i32) -> Result<bool> {
        self.services.set_block(actor, position, state, flags)
    }
    fn set_block_for_landing(&mut self, actor: &mut FallingBlockState, original: &u32,
        position: BlockPosition, state: u32, captured_replaced: u32, flags: i32) -> Result<bool> {
        if falling_block_fact(*original)?.class != "dhk" {
            return self.set_block(actor, position, state, flags);
        }
        ensure!(self.powder_landing.is_none(), "powder tick attempted a second independent landing");
        let owner = self.powder_owner.context("powder landing lacks its installed actual Block owner")?;
        let admission = main_falling_block_powder::prepare_landing(&mut self.services,
            owner, actor, *original, position, state, captured_replaced, flags)?;
        self.powder_landing = Some(admission);
        self.powder_landing.as_mut().expect("retained actual powder landing")
            .commit_first(&mut self.services)
    }
    fn broadcast_block_update(&mut self, _actor: &FallingBlockState,
        position: BlockPosition, state: u32) -> Result<()> {
        let packet = Packet { id: 0x09, payload: world::try_block_update_payload(
            position.0, position.1, position.2, state)? };
        self.publication.broadcast_tracked(self.services.events, packet);
        Ok(())
    }
    fn discard(&mut self, actor: &mut FallingBlockState) -> Result<()> {
        self.publication.discard(actor, self.services.items, self.services.events)
    }
    fn on_land_if_fallable(&mut self, original: &u32, actor: &mut FallingBlockState,
        position: BlockPosition, carried: u32, replaced: u32) -> Result<()> {
        if falling_block_fact(*original)?.class == "dhk" {
            return self.powder_landing.as_mut().context("powder onLand lacks its admitted first-write receipt")?
                .on_land(&mut self.services, actor, *original, position, carried, replaced);
        }
        use mc121_rust::falling_block::block::{source_class, NativeFallingClass};
        match source_class(*original)? {
            Some(NativeFallingClass::Anvil) => {
                if !actor.silent { self.level_event(1031, position)?; }
                Ok(())
            }
            // dhg inherits the literal return-only Fallable.onLand body.
            Some(NativeFallingClass::Ordinary) => Ok(()),
            None => bail!("falling onLand requires its actual Fallable implementation"),
        }
    }
    fn on_broken_after_fall_if_fallable(&mut self, original: &u32,
        actor: &mut FallingBlockState, position: BlockPosition) -> Result<()> {
        if falling_block_fact(*original)?.class == "dhk" {
            self.powder_owner.context("powder broken callback lacks its retained actual owner")?
                .binding_for_state(*original)?.context("broken powder has no actual retained source binding")?;
            // ConcretePowderBlock inherits Fallable's literal empty body.
            return Ok(());
        }
        use mc121_rust::falling_block::block::{source_class, NativeFallingClass};
        match source_class(*original)? {
            Some(NativeFallingClass::Anvil) => {
                if !actor.silent { self.level_event(1029, position)?; }
                Ok(())
            }
            // dhg inherits the literal return-only Fallable broken callback.
            Some(NativeFallingClass::Ordinary) => Ok(()),
            None => bail!("falling broken callback requires its actual Fallable implementation"),
        }
    }
    fn entity_drops_enabled(&mut self) -> Result<bool> {
        self.services.context.rules.boolean("doEntityDrops")
            .context("actual doEntityDrops game-rule owner is unavailable")
    }
    fn spawn_at_location(&mut self, actor: &mut FallingBlockState, original: &u32) -> Result<()> {
        self.services.spawn_falling_drop(actor, *original)
    }
    fn minimum_build_height(&mut self) -> Result<i32> { Ok(self.services.context.dimension.min_y) }
    fn maximum_build_height(&mut self) -> Result<i32> {
        self.services.context.dimension.min_y.checked_add(self.services.context.dimension.height)
            .context("actual maximum build height overflow")
    }
    fn state_has_block_entity(&mut self, state: u32) -> Result<bool> {
        Ok(falling_block_fact(state)?.block_entity_type.is_some())
    }
    fn block_entity(&mut self, _position: BlockPosition) -> Result<Option<Self::BlockEntity>> {
        bail!("falling BlockData requires its actual retained block entity")
    }
    fn save_block_entity_without_metadata(&mut self, entity: &Self::BlockEntity) -> Result<Tag> {
        match *entity {}
    }
    fn block_data_key_order(&mut self, _actor: &FallingBlockState) -> Result<Vec<mc121_rust::nbt::JavaString>> {
        bail!("falling BlockData requires its actual HashMap encounter owner")
    }
    fn load_block_entity_with_components(&mut self, entity: &mut Self::BlockEntity,
        _merged: Tag) -> Result<BlockEntityLoadStatus> { match *entity {} }
    fn set_block_entity_changed(&mut self, entity: &mut Self::BlockEntity) -> Result<()> { match *entity {} }
}

impl ActualFallingBlockEditServices<'_> {
    /// Entity.spawnAtLocation(ItemLike): actual ItemEntity constructor, default
    /// delay10, then actual world-add UUID admission. The source ignores the
    /// add boolean and does not refund either a removed falling actor or a
    /// constructed but rejected item. This is not Player.drop's velocity/stat
    /// path and must not consume any Player.random draw.
    fn spawn_falling_drop(&mut self, actor: &FallingBlockState, original: u32) -> Result<()> {
        let stack = ItemStack::new(falling_block_fact(original)?.item, 1)?;
        if stack.is_empty() { return Ok(()); }
        self.preflight_tick(actor)?;
        let external = self.external_entity_uuids
            .context("falling drop requires the actual external UUID admission view")?;
        stack.encode_canonical()?;
        let first = self.entities.load(Ordering::Relaxed);
        ensure!(first > 0 && first < i32::MAX, "shared item entity counter exhausted");
        self.items.validate_entity_registration_batch(std::iter::once(first))?;
        let position = actor.position_vec();
        ensure!([position.x, position.y, position.z].iter().all(|v| v.is_finite()),
            "falling drop has nonfinite actual source position");
        // ItemEntity's delegating constructor evaluates both Level.random
        // doubles before Entity's ID/RNG/UUID initialization.
        let dx = self.items.world_random_mut().next_f64() * 0.2 - 0.1;
        let dz = self.items.world_random_mut().next_f64() * 0.2 - 0.1;
        let id = self.entities.try_update(Ordering::Relaxed, Ordering::Relaxed,
            |next| next.checked_add(1)).map_err(|_| anyhow!("shared item entity counter exhausted"))?;
        self.items.validate_entity_registration_batch(std::iter::once(id))?;
        let mut random = random_seed::create_random()?;
        let hi = (random.next_i64() as u64 & 0xffff_ffff_ffff_0fff) | 0x4000;
        let lo = (random.next_i64() as u64 & 0x3fff_ffff_ffff_ffff) | 0x8000_0000_0000_0000;
        let mut uuid = [0;16];
        uuid[..8].copy_from_slice(&hi.to_be_bytes()); uuid[8..].copy_from_slice(&lo.to_be_bytes());
        let bob = random.next_f32() * std::f32::consts::PI * 2f32;
        let yaw = random.next_f32() * 360f32;
        let mut item = ItemState::new(id, uuid, Position {
            x: position.x, y: position.y + f64::from(0f32), z: position.z,
            yaw, pitch: 0., on_ground: false,
        }, Vec3::new(dx, 0.2, dz), stack)?;
        item.bob_offset = bob;
        item.default_pickup_delay();
        let duplicate = self.registry.get_by_uuid(&uuid).is_some()
            || self.items.item_states().any(|state| !state.removed && state.uuid == uuid)
            || external.iter().any(|(id, value)| *value == uuid && !(*id == actor.entity_id && actor.removed));
        if !duplicate {
            // The existing actual native item add owns shared index, tick-list,
            // tracking, Item replication and its ordered outbox.
            self.items.insert_item(item, random)?;
        }
        Ok(())
    }
}
