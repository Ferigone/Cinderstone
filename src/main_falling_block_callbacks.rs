//! First active fluid callback profile for retained native falling actors.
//!
//! This reuses the actual Main fluid runtime/host. The admitted water-only
//! neighborhood cannot harden lava, ignite fire, write another block, invoke
//! loot or call an active redstone/block-entity hook. Remaining contexts error
//! before the primary write or irreversible constructor entropy.
use super::*;
use mc121_rust::fluid_physics::FluidKind;

const ADJACENT: [(i32,i32,i32);7] = [
    (0,0,0),(-1,0,0),(1,0,0),(0,-1,0),(0,1,0),(0,0,-1),(0,0,1),
];

fn at(origin:BlockPosition,delta:(i32,i32,i32))->BlockPosition {
    (origin.0.wrapping_add(delta.0),origin.1.wrapping_add(delta.1),
        origin.2.wrapping_add(delta.2))
}

pub(super) fn has_water_context(world:&mut WorldBackend,plan:&InteractionPlan)->Result<bool> {
    for event in &plan.events {
        match event {
            InteractionEvent::Write { position,old_state,state,.. } => {
                for value in [*old_state,*state] {
                    if FluidState::from_block(value)?.kind==FluidKind::Water { return Ok(true); }
                }
                for delta in ADJACENT {
                    let p=at(*position,delta);
                    if FluidState::from_block(world.block(p.0,p.1,p.2)?)?.kind==FluidKind::Water {
                        return Ok(true);
                    }
                }
            }
            InteractionEvent::ScheduleWaterTick {..} => return Ok(true),
            _ => {}
        }
    }
    Ok(false)
}

/// Source phase certificate for this bounded callback family. There is one
/// actual solid/air write, then its FallingBlock.onPlace obligation, then the
/// neighbor/shape callbacks. Other observable shape callbacks are unavailable
/// here rather than moved across the water scheduling calls.
pub(super) fn preflight_water_plan(services:&mut ActualFallingBlockEditServices<'_>,
    plan:&InteractionPlan)->Result<()> {
    services.require_powder_plan_owners(plan)?;
    validate_anvil_schedule_obligations(plan)?;
    let writes=plan.events.iter().filter_map(|event|match event {
        InteractionEvent::Write {position,old_state,state,flags}=>Some((*position,*old_state,*state,*flags)),
        _=>None,
    }).collect::<Vec<_>>();
    ensure!(writes.len()==1,"active water callback profile needs one actual falling block write");
    let (position,old,new,flags)=writes[0];
    ensure!(flags==3,"active falling water callback requires source flags3");
    ensure!(FluidState::from_block(new)?.is_empty(),
        "new liquid onPlace needs a separate literal callback-phase owner");
    let a=mc121_rust::redstone::block(old)?;
    let b=mc121_rust::redstone::block(new)?;
    ensure!((a.min==b.min||a.on_remove=="dtb")
        &&(b.on_place=="dtb"||represented_anvil_change(old,new)?),
        "water falling edit reaches active non-fluid block callbacks");
    let WorldBackend::Fixture(flat)=&*services.world else {
        bail!("water falling callback requires actual native world source")
    };
    let mut staged=WorldBackend::Fixture(flat.clone());
    main_world_interactions::commit_fixture_plan(&mut staged,plan)?
        .context("water callback proof needs native atomic planning")?;
    for value in [old,new] {
        ensure!(FluidState::from_block(value)?.kind!=FluidKind::Lava
            &&!placement::in_block_tag(value,"minecraft:fire")
            &&!mc121_rust::fire_simulation::has_block_entity(value)?,
            "lava/fire/block-entity callbacks are not supplied by the water owner");
    }
    for delta in ADJACENT {
        let p=at(position,delta);
        let value=staged.block(p.0,p.1,p.2)?;
        let fluid=FluidState::from_block(value)?;
        let block=mc121_rust::redstone::block(value)?;
        ensure!(fluid.kind!=FluidKind::Lava&&!placement::in_block_tag(value,"minecraft:fire"),
            "water callback neighborhood reaches actual lava/fire work");
        ensure!(inventory::block_state_property(value,"waterlogged").as_deref()!=Some("true"),
            "waterlogged shape callbacks need their separate ordered phase owner");
        if delta!=(0,0,0) {
            // Exact base Block/AirBlock and LiquidBlock neighbors have no
            // additional shape scheduler between the liquid shape attempts.
            // A powder target's inherited onPlace is its represented schedule
            // above. Adjacent powder still requires an interleaved shape/fluid
            // owner; do not turn wet hardening into a quiescent callback.
            // Snowy/plant/custom/Falling neighbors require their real owner.
            ensure!(matches!(block.class.as_str(),"dfy"|"dev"|"dko"),
                "mixed active shape callback cannot be moved across fluid scheduling");
            ensure!(block.neighbor=="dtb"||(block.neighbor=="dko"&&fluid.kind==FluidKind::Water),
                "water edit reaches actual active non-fluid neighborChanged");
        }
    }
    let mut neighbors=0;
    for event in &plan.events {
        match event {
            InteractionEvent::Write {..}=>{},
            InteractionEvent::ScheduleBlockTick(tick)=>{
                ensure!(tick.position==position&&tick.state==new
                    &&services.block_runtime.owns_falling_tick(tick.state,tick.delay)?,
                    "active water profile cannot reorder another actual block scheduler");
                services.block_runtime.prepare_halo(services.world,services.chunks,
                    (position.0.div_euclid(16),position.2.div_euclid(16)),services.now)?;
            }
            InteractionEvent::NeighbourNotice {position:p,source_state}=>{
                ensure!(*p==position&&*source_state==old,
                    "water edit has a foreign neighbor callback obligation");
                neighbors+=1;
            }
            _=>bail!("active water profile needs an additional actual effect/shape owner"),
        }
    }
    ensure!(neighbors==1,"water falling edit lost its actual neighbor callback phase");
    services.fluid_runtime.require_healthy()?;
    services.fluid_runtime.sync_chunks(&services.chunks.manager,services.now)?;
    services.fluid_runtime.prepare_halo(services.world,services.chunks,
        (position.0.div_euclid(16),position.2.div_euclid(16)),services.now)?;
    crate::main_fluid_simulation::facts(services.context)?;
    Ok(())
}

/// New-state onPlace is proved literally nonliquid above. Thus the full real
/// fluid kernel can run at NeighbourNotice: after FallingBlock.onPlace and
/// before any other observable shape callback. Only LiquidBlock water shape
/// schedules remain in this admitted profile, in their original kernel order.
pub(super) fn execute_water_event(services:&mut ActualFallingBlockEditServices<'_>,
    event:&InteractionEvent)->Result<()> {
    let InteractionEvent::NeighbourNotice {position,source_state} = event else {return Ok(())};
    let new=services.world.block(position.0,position.1,position.2)?;
    let world_facts=crate::main_fluid_simulation::facts(services.context)?;
    let active_items=services.items.item_count();
    let snapshots=services.registry.snapshot();
    let mut host=crate::main_fluid_simulation::MainFluidHost {
        chunks:&mut *services.chunks,gameplay:&mut *services.gameplay,
        interactions:&mut *services.interactions,entity_source:&mut *services.source,
        revision:&mut *services.world_revision,events:services.events,players:snapshots,
        active_items,border:services.border,random_ticks:Some(&mut *services.random_ticks),
    };
    services.fluid_runtime.observe_change_with_parts(services.world,
        services.items.world_random_mut(),&mut host,world_facts,services.now,
        *position,*source_state,new,3)?;
    ensure!(services.interactions.pending_count()==0,
        "water callback produced actual extra block/effect work requiring its owner");
    Ok(())
}
