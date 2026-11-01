//! Vanilla 1.21 scheduled FireBlock, BaseFireBlock placement and portal frames.
//! Weather, biome membership, loaded chunks and TNT entities are live host
//! inputs. Fire is never dispatched as a fabricated random-block tick.
use crate::{
    fluid_physics::FluidState,
    fluid_simulation::{self, FluidSimulationHost, LavaIgnitionHost, Position},
    geometry::{Axis, Direction},
    inventory::{block_state_property, block_state_with_properties},
    placement::{self, PlacementWorld, SupportType},
    random::LegacyRandomSource,
    runtime_world::{RuntimeWorld, WorldChange},
    tick::{BlockPos, ChunkTickQueue, LevelTickScheduler, SavedTick, ScheduledTick},
};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::OnceLock};

const DIRECTIONS: [Direction; 6] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];

/// Actual dimension/rule/weather values sampled while holding the level lock.
#[derive(Clone, Debug)]
pub struct FireWorldFacts {
    pub do_fire_tick: bool,
    pub is_raining: bool,
    pub difficulty_id: i32,
    /// True only for the actual OVERWORLD and NETHER dimension keys, as in
    /// BaseFireBlock.inPortalDimension; ultrawarm is a different property.
    pub portal_dimension: bool,
    pub minimum_build_height: i32,
    pub maximum_build_height: i32,
    /// Actual DimensionType.infiniburn tag key, without a leading '#'.
    pub infiniburn_tag: String,
}
impl FireWorldFacts {
    fn validate(&self) -> Result<()> {
        ensure!(
            (0..=3).contains(&self.difficulty_id),
            "invalid fire difficulty ID"
        );
        ensure!(
            self.minimum_build_height < self.maximum_build_height,
            "invalid fire build bounds"
        );
        ensure!(
            !self.infiniburn_tag.is_empty(),
            "dimension infiniburn tag unavailable"
        );
        Ok(())
    }
}

/// The embedding host executes state writes and OTHER block callbacks.
/// This module owns FireBlock/SoulFireBlock onPlace and their scheduler;
/// set_block must not invoke this same fire callback a second time.
pub trait FireSimulationHost: FluidSimulationHost {
    fn is_raining_at(&mut self, world: &mut RuntimeWorld, position: Position) -> Result<bool>;
    fn increased_fire_burnout(
        &mut self,
        world: &mut RuntimeWorld,
        position: Position,
    ) -> Result<bool>;
    /// Use actual dimension/data-pack tag membership when tags were replaced.
    fn infinitely_burning(
        &mut self,
        _world: &mut RuntimeWorld,
        _position: Position,
        state: u32,
        tag: &str,
    ) -> Result<bool> {
        placement::placement_block_name(state)?;
        Ok(placement::in_block_tag(state, tag))
    }
    fn is_loaded(&mut self, world: &mut RuntimeWorld, position: Position) -> Result<bool>;
    fn has_chunk_at(&mut self, world: &mut RuntimeWorld, position: Position) -> Result<bool>;
    /// Original TntBlock.explode after its block has burned: construct/add the
    /// actual PrimedTnt entity, sound and game event using the real RNG owner.
    fn prime_tnt(
        &mut self,
        world: &mut RuntimeWorld,
        position: Position,
        random: &mut LegacyRandomSource,
    ) -> Result<()>;
    /// A moving piston/shulker support face requires its current animation.
    /// Ordinary immutable states use the exact pinned full-support mask.
    fn sturdy_up(
        &mut self,
        _world: &mut RuntimeWorld,
        _position: Position,
        state: u32,
    ) -> Result<bool> {
        placement::block_support_face(state, Direction::Up)
            .context("animated fire support face required")
    }
}

/// ServerLevel passes its own Level.random into scheduled block ticks. Keep
/// this stream and the sub-tick counter shared with scheduled fluid callbacks.
pub struct FireTickContext<'a, H> {
    pub world: &'a mut RuntimeWorld,
    pub random: &'a mut LegacyRandomSource,
    pub sub_tick_order: &'a mut i64,
    pub host: &'a mut H,
    pub facts: &'a FireWorldFacts,
    pub game_time: i64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScheduledFire {
    #[serde(rename = "minecraft:fire")]
    Normal,
}
#[derive(Default, Debug)]
pub struct FireTickReport {
    pub executed: usize,
    pub stale: usize,
    pub changed_blocks: usize,
}
/// Scheduling loan shared by fire and the canonical global BLOCK queue.
pub trait FireScheduleAccess {
    fn schedule_fire(&mut self, position: Position, now: i64, delay: i32, order: &mut i64) -> Result<bool>;
    fn restore_fire_tick(&mut self, tick: ScheduledTick<ScheduledFire>) -> bool;
}
impl FireScheduleAccess for LevelTickScheduler<ScheduledFire> {
    fn schedule_fire(&mut self,p:Position,now:i64,delay:i32,order:&mut i64)->Result<bool>{schedule(self,p,now,delay,order)}
    fn restore_fire_tick(&mut self,t:ScheduledTick<ScheduledFire>)->bool{self.schedule(t)}
}
pub struct GlobalFireSchedule<'a> { pub queue: &'a mut crate::block_ticks::BlockTickScheduler }
impl FireScheduleAccess for GlobalFireSchedule<'_> {
    fn schedule_fire(&mut self,p:Position,now:i64,delay:i32,order:&mut i64)->Result<bool>{
        crate::block_ticks::schedule(self.queue,p,data().block_ids["cr"],now,delay,crate::tick::TickPriority::Normal,order)
    }
    fn restore_fire_tick(&mut self,t:ScheduledTick<ScheduledFire>)->bool{
        self.queue.schedule(ScheduledTick::new(crate::block_ticks::key(data().block_ids["cr"]).expect("pinned fire singleton"),t.pos,t.trigger_tick,t.priority,t.sub_tick_order))
    }
}
pub fn random_tick_lava_global<H:FireSimulationHost>(queue:&mut crate::block_ticks::BlockTickScheduler,fluids:&mut fluid_simulation::FluidSimulation,ctx:&mut FireTickContext<'_,H>,fluid_facts:fluid_simulation::FluidWorldFacts,p:Position)->Result<usize>{
    ctx.facts.validate()?;let before=*ctx.sub_tick_order;let mut unused=before;
    let mut loan=GlobalFireSchedule{queue};
    let mut bridge=LavaFireBridge{host:ctx.host,fire:&mut loan,sub_tick_order:ctx.sub_tick_order,facts:ctx.facts,game_time:ctx.game_time};
    let result=fluids.random_tick_lava(&mut fluid_simulation::FluidTickContext{world:ctx.world,random:ctx.random,sub_tick_order:&mut unused,host:&mut bridge,facts:fluid_facts,game_time:ctx.game_time},p,ctx.facts.do_fire_tick,ctx.facts.minimum_build_height,ctx.facts.maximum_build_height);
    ensure!(unused==before,"LavaFluid.randomTick unexpectedly scheduled a fluid tick");result
}
pub fn normal_fire_key()->Result<u32>{crate::block_ticks::key(data().block_ids["cr"])}
pub fn on_block_changed_global<H:FireSimulationHost>(queue:&mut crate::block_ticks::BlockTickScheduler,ctx:&mut FireTickContext<'_,H>,p:Position,old:u32,new:u32)->Result<Vec<WorldChange>>{
    ctx.facts.validate()?;
    let mut loan=GlobalFireSchedule{queue};
    let mut run=Run{scheduler:&mut loan,ctx,changes:Vec::new()};
    if let Err(e)=run.on_place(p,old,new){if !run.changes.is_empty(){run.ctx.host.block_changes(&run.changes)?;}return Err(e);}
    Ok(run.changes)
}
pub fn tick_one_global<H:FireSimulationHost>(queue:&mut crate::block_ticks::BlockTickScheduler,ctx:&mut FireTickContext<'_,H>,p:Position)->Result<FireTickReport>{
    ctx.facts.validate()?;
    let state=block(ctx.world,p)?;
    if fire_kind(state)?!=1{return Ok(FireTickReport{stale:1,..Default::default()});}
    let mut loan=GlobalFireSchedule{queue};let mut run=Run{scheduler:&mut loan,ctx,changes:Vec::new()};
    let result=run.tick(p,state);
    let changed_blocks=run.changes.iter().filter(|c|matches!(c,WorldChange::Block(_))).count();
    run.ctx.host.block_changes(&run.changes)?;result?;
    Ok(FireTickReport{executed:1,changed_blocks,..Default::default()})
}
fn place_fire_with_scheduler<H:FireSimulationHost>(scheduler:&mut dyn FireScheduleAccess,ctx:&mut FireTickContext<'_,H>,p:Position)->Result<Vec<WorldChange>>{
    ctx.facts.validate()?;let state=state_at(ctx.world,ctx.host,p,None)?;
    let mut run=Run{scheduler,ctx,changes:Vec::new()};
    if let Err(e)=run.set(p,state,3){if !run.changes.is_empty(){run.ctx.host.block_changes(&run.changes)?;}return Err(e);}
    Ok(run.changes)
}
pub struct FireSimulation {
    scheduler: LevelTickScheduler<ScheduledFire>,
}
impl Default for FireSimulation {
    fn default() -> Self {
        Self::new()
    }
}
impl FireSimulation {
    pub fn new() -> Self {
        Self {
            scheduler: LevelTickScheduler::new(),
        }
    }
    pub fn pending_count(&self) -> usize {
        self.scheduler.count()
    }
    pub fn chunk_has_ticks(&self, chunk: (i32, i32)) -> bool {
        self.scheduler.chunk(chunk).is_some_and(|q| !q.is_empty())
    }
    pub fn saved_chunk_ticks(
        &self,
        chunk: (i32, i32),
        now: i64,
    ) -> Option<Vec<SavedTick<ScheduledFire>>> {
        self.scheduler.chunk(chunk).map(|q| q.saved_ticks(now))
    }
    pub fn load_chunk(
        &mut self,
        chunk: (i32, i32),
        saved: Vec<SavedTick<ScheduledFire>>,
        now: i64,
    ) -> Result<()> {
        ensure!(
            self.scheduler.chunk(chunk).is_none(),
            "fire tick chunk already registered"
        );
        self.scheduler
            .add_chunk(chunk, ChunkTickQueue::from_saved_for_chunk(chunk, saved));
        self.scheduler.unpack_chunk(chunk, now);
        Ok(())
    }
    pub fn unload_chunk(
        &mut self,
        chunk: (i32, i32),
        now: i64,
    ) -> Option<Vec<SavedTick<ScheduledFire>>> {
        self.scheduler
            .remove_chunk(chunk)
            .map(|q| q.saved_ticks(now))
    }
    pub fn schedule(
        &mut self,
        position: Position,
        now: i64,
        delay: i32,
        order: &mut i64,
    ) -> Result<bool> {
        schedule(&mut self.scheduler, position, now, delay, order)
    }
    /// Called after an external committed transition. Return all additional
    /// fire/portal/removal edits; the caller publishes this batch exactly once.
    pub fn on_block_changed<H: FireSimulationHost>(
        &mut self,
        ctx: &mut FireTickContext<'_, H>,
        position: Position,
        old_state: u32,
        new_state: u32,
    ) -> Result<Vec<WorldChange>> {
        ctx.facts.validate()?;
        let mut run = Run {
            scheduler: &mut self.scheduler,
            ctx,
            changes: Vec::new(),
        };
        if let Err(error) = run.on_place(position, old_state, new_state) {
            if !run.changes.is_empty() {
                run.ctx.host.block_changes(&run.changes)?;
            }
            return Err(error);
        }
        Ok(run.changes)
    }
    /// BaseFireBlock.getState, followed by real setBlock/onPlace. LavaFluid's
    /// ignition caller already publishes the returned ordered changes.
    pub fn place_fire<H: FireSimulationHost>(
        &mut self,
        ctx: &mut FireTickContext<'_, H>,
        position: Position,
    ) -> Result<Vec<WorldChange>> {
        ctx.facts.validate()?;
        let state = state_at(ctx.world, ctx.host, position, None)?;
        let mut run = Run {
            scheduler: &mut self.scheduler,
            ctx,
            changes: Vec::new(),
        };
        if let Err(error) = run.set(position, state, 3) {
            if !run.changes.is_empty() {
                run.ctx.host.block_changes(&run.changes)?;
            }
            return Err(error);
        }
        Ok(run.changes)
    }
    pub fn tick_due<H: FireSimulationHost>(
        &mut self,
        ctx: &mut FireTickContext<'_, H>,
        budget: usize,
        tickable: impl FnMut((i32, i32)) -> bool,
    ) -> Result<FireTickReport> {
        ctx.facts.validate()?;
        let due = self.scheduler.drain_due(ctx.game_time, budget, tickable);
        let mut report = FireTickReport::default();
        let mut run = Run {
            scheduler: &mut self.scheduler,
            ctx,
            changes: Vec::new(),
        };
        for (index, tick) in due.iter().enumerate() {
            let p = (tick.pos.x, tick.pos.y, tick.pos.z);
            let outcome = (|| -> Result<bool> {
                let state = block(run.ctx.world, p)?;
                if fire_kind(state)? != 1 {
                    return Ok(false);
                }
                run.tick(p, state)?;
                Ok(true)
            })();
            if let Err(error) = outcome {
                for pending in &due[index + 1..] {
                    run.scheduler.restore_fire_tick(pending.clone());
                }
                // Committed source writes remain real even when a later host
                // effect cannot execute. Publish their exact ordered prefix.
                if !run.changes.is_empty() {
                    run.ctx.host.block_changes(&run.changes)?;
                }
                return Err(error.context(format!("scheduled fire tick at {p:?}")));
            }
            if outcome? {
                report.executed += 1;
            } else {
                report.stale += 1;
            }
        }
        report.changed_blocks = run
            .changes
            .iter()
            .filter(|c| matches!(c, WorldChange::Block(_)))
            .count();
        if !run.changes.is_empty() {
            run.ctx.host.block_changes(&run.changes)?;
        }
        Ok(report)
    }
    /// Execute the existing source-ported LavaFluid.randomTick with this fire
    /// owner. That callback schedules no fluid tick itself; its context's
    /// order slot is checked unchanged, while every fire callback consumes the
    /// actual shared level counter through LavaFireBridge.
    pub fn random_tick_lava<H: FireSimulationHost>(
        &mut self,
        fluids: &mut fluid_simulation::FluidSimulation,
        ctx: &mut FireTickContext<'_, H>,
        fluid_facts: fluid_simulation::FluidWorldFacts,
        position: Position,
    ) -> Result<usize> {
        ctx.facts.validate()?;
        let before = *ctx.sub_tick_order;
        let mut unused_fluid_order = before;
        let mut bridge = LavaFireBridge {
            host: ctx.host,
            fire: &mut self.scheduler,
            sub_tick_order: ctx.sub_tick_order,
            facts: ctx.facts,
            game_time: ctx.game_time,
        };
        let result = fluids.random_tick_lava(
            &mut fluid_simulation::FluidTickContext {
                world: ctx.world,
                random: ctx.random,
                sub_tick_order: &mut unused_fluid_order,
                host: &mut bridge,
                facts: fluid_facts,
                game_time: ctx.game_time,
            },
            position,
            ctx.facts.do_fire_tick,
            ctx.facts.minimum_build_height,
            ctx.facts.maximum_build_height,
        );
        ensure!(
            unused_fluid_order == before,
            "LavaFluid.randomTick unexpectedly scheduled a fluid tick"
        );
        result
    }
}

/// Adapter used directly as FluidTickContext.host. Every fluid host operation
/// delegates to its actual owner; only LavaIgnitionHost.place_fire is supplied
/// by the scheduled fire engine below. It never publishes a second batch.
pub struct LavaFireBridge<'a, H> {
    pub host: &'a mut H,
    pub fire: &'a mut dyn FireScheduleAccess,
    pub sub_tick_order: &'a mut i64,
    pub facts: &'a FireWorldFacts,
    pub game_time: i64,
}
impl<H: FireSimulationHost> FluidSimulationHost for LavaFireBridge<'_, H> {
    fn set_block(
        &mut self,
        w: &mut RuntimeWorld,
        p: Position,
        s: u32,
        f: u32,
    ) -> Result<Vec<WorldChange>> {
        self.host.set_block(w, p, s, f)
    }
    fn before_destroying_block(
        &mut self,
        w: &mut RuntimeWorld,
        p: Position,
        s: u32,
        r: &mut LegacyRandomSource,
    ) -> Result<()> {
        self.host.before_destroying_block(w, p, s, r)
    }
    fn level_event(&mut self, e: i32, p: Position, d: i32) -> Result<()> {
        self.host.level_event(e, p, d)
    }
    fn place_custom_liquid(
        &mut self,
        w: &mut RuntimeWorld,
        p: Position,
        s: u32,
        f: FluidState,
    ) -> Result<Vec<WorldChange>> {
        self.host.place_custom_liquid(w, p, s, f)
    }
    fn block_changes(&mut self, c: &[WorldChange]) -> Result<()> {
        self.host.block_changes(c)
    }
}
impl<H: FireSimulationHost> LavaIgnitionHost for LavaFireBridge<'_, H> {
    fn is_loaded(&mut self, w: &mut RuntimeWorld, p: Position) -> Result<bool> {
        self.host.is_loaded(w, p)
    }
    fn has_chunk_at(&mut self, w: &mut RuntimeWorld, p: Position) -> Result<bool> {
        self.host.has_chunk_at(w, p)
    }
    fn place_fire(
        &mut self,
        w: &mut RuntimeWorld,
        p: Position,
        r: &mut LegacyRandomSource,
    ) -> Result<Vec<WorldChange>> {
        place_fire_with_scheduler(self.fire,
            &mut FireTickContext {
                world: w,
                random: r,
                sub_tick_order: self.sub_tick_order,
                host: self.host,
                facts: self.facts,
                game_time: self.game_time,
            },
            p,
        )
    }
}

type FactRow = (i32, i32, bool, bool, u8, bool, bool, bool);
#[derive(Deserialize)]
struct Facts {
    states: Vec<usize>,
    rows: Vec<FactRow>,
    block_ids: BTreeMap<String, u32>,
    increased_burnout_biomes: Vec<String>,
    overworld_infiniburn: String,
}
fn data() -> &'static Facts {
    static DATA: OnceLock<Facts> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("../fixtures/fire-simulation-1.21.json"))
            .expect("original FireBlock odds")
    })
}
fn facts(state: u32) -> Result<FactRow> {
    data()
        .rows
        .get(
            *data()
                .states
                .get(state as usize)
                .context("unknown fire block state")?,
        )
        .copied()
        .context("unknown fire fact row")
}
pub fn burn_odds(state: u32) -> Result<i32> {
    Ok(facts(state)?.0)
}
pub fn ignite_odds(state: u32) -> Result<i32> {
    Ok(facts(state)?.1)
}
pub fn fire_kind(state: u32) -> Result<u8> {
    Ok(facts(state)?.4)
}
pub fn is_randomly_ticking(state: u32) -> Result<bool> {
    Ok(facts(state)?.5)
}
pub fn has_block_entity(state: u32) -> Result<bool> {
    Ok(facts(state)?.6)
}
/// Admit only BlockBehaviour.getDrops' actual EMPTY-key identity branch.
/// Pinned 1.21 normal/soul fire use named zero-pool tables with random_sequence,
/// so they return false: creating that sequence still needs its real owner.
/// Neither fire class overrides spawnAfterBreak (the inherited body is empty).
pub fn fire_has_empty_loot(state: u32) -> Result<bool> {
    let row = facts(state)?;
    Ok(row.4 != 0 && row.7)
}
pub fn vanilla_biome_increased_fire_burnout(name: &str) -> bool {
    data()
        .increased_burnout_biomes
        .iter()
        .any(|biome| biome == name)
}
pub fn vanilla_overworld_infiniburn_tag() -> &'static str {
    data()
        .overworld_infiniburn
        .strip_prefix('#')
        .expect("original infiniburn tag key")
}
fn block(world: &mut RuntimeWorld, p: Position) -> Result<u32> {
    world.block(p.0, p.1, p.2)
}
fn step(p: Position, d: Direction) -> Position {
    let q = d.step();
    (
        p.0.wrapping_add(q.0),
        p.1.wrapping_add(q.1),
        p.2.wrapping_add(q.2),
    )
}
fn offset(p: Position, x: i32, y: i32, z: i32) -> Position {
    (
        p.0.wrapping_add(x),
        p.1.wrapping_add(y),
        p.2.wrapping_add(z),
    )
}
fn along(p: Position, d: Direction, n: i32) -> Position {
    let q = d.step();
    offset(
        p,
        q.0.wrapping_mul(n),
        q.1.wrapping_mul(n),
        q.2.wrapping_mul(n),
    )
}
fn age(state: u32) -> Result<i32> {
    block_state_property(state, "age")
        .context("fire age property missing")?
        .parse()
        .context("invalid fire age")
}
fn set_age(state: u32, age: i32) -> Result<u32> {
    block_state_with_properties(state, &BTreeMap::from([("age".into(), age.to_string())]))
}

/// Pure state part of normal/soul fire updateShape, for the structural host's
/// overlay. Reads the same support faces and flammability odds as placement.
pub fn shape_update(state: u32, p: Position, world: &mut impl PlacementWorld) -> Result<u32> {
    match fire_kind(state)? {
        0 => Ok(state),
        2 => Ok(
            if placement::in_block_tag(
                world.block(step(p, Direction::Down))?,
                "minecraft:soul_fire_base_blocks",
            ) {
                data().block_ids["cs"]
            } else {
                0
            },
        ),
        _ => {
            if !can_survive_in(p, world)? {
                return Ok(0);
            }
            state_in(p, Some(age(state)?), world)
        }
    }
}
pub fn can_survive_in(p: Position, world: &mut impl PlacementWorld) -> Result<bool> {
    let below = step(p, Direction::Down);
    if world.support(below, Direction::Up, SupportType::Full)? {
        return Ok(true);
    }
    for d in DIRECTIONS {
        if ignite_odds(world.block(step(p, d))?)? > 0 {
            return Ok(true);
        }
    }
    Ok(false)
}
pub fn state_in(
    p: Position,
    age_value: Option<i32>,
    world: &mut impl PlacementWorld,
) -> Result<u32> {
    let below = step(p, Direction::Down);
    let lower = world.block(below)?;
    if placement::in_block_tag(lower, "minecraft:soul_fire_base_blocks") {
        return Ok(data().block_ids["cs"]);
    }
    let mut state = data().block_ids["cr"];
    if ignite_odds(lower)? == 0 && !world.support(below, Direction::Up, SupportType::Full)? {
        let mut props = BTreeMap::new();
        for d in DIRECTIONS {
            if d != Direction::Down {
                props.insert(
                    d.name().to_owned(),
                    (ignite_odds(world.block(step(p, d))?)? > 0).to_string(),
                );
            }
        }
        state = block_state_with_properties(state, &props)?;
    }
    if let Some(value) = age_value {
        state = set_age(state, value)?;
    }
    Ok(state)
}
fn state_at<H: FireSimulationHost>(
    world: &mut RuntimeWorld,
    host: &mut H,
    p: Position,
    age_value: Option<i32>,
) -> Result<u32> {
    let below = step(p, Direction::Down);
    let lower = block(world, below)?;
    if placement::in_block_tag(lower, "minecraft:soul_fire_base_blocks") {
        return Ok(data().block_ids["cs"]);
    }
    let mut state = data().block_ids["cr"];
    if ignite_odds(lower)? == 0 && !host.sturdy_up(world, below, lower)? {
        let mut props = BTreeMap::new();
        for d in DIRECTIONS {
            if d != Direction::Down {
                props.insert(
                    d.name().to_owned(),
                    (ignite_odds(block(world, step(p, d))?)? > 0).to_string(),
                );
            }
        }
        state = block_state_with_properties(state, &props)?;
    }
    if let Some(value) = age_value {
        state = set_age(state, value)?;
    }
    Ok(state)
}
fn schedule(
    scheduler: &mut LevelTickScheduler<ScheduledFire>,
    p: Position,
    now: i64,
    delay: i32,
    order: &mut i64,
) -> Result<bool> {
    let pos = BlockPos::new(p.0, p.1, p.2);
    ensure!(
        scheduler.chunk(pos.chunk()).is_some(),
        "fire tick target chunk {:?} is not registered loaded",
        pos.chunk()
    );
    let value = *order;
    *order = order.wrapping_add(1);
    Ok(scheduler.schedule(ScheduledTick::normal(
        ScheduledFire::Normal,
        pos,
        now.wrapping_add(i64::from(delay)),
        value,
    )))
}

struct Run<'a, 'b, H> {
    scheduler: &'a mut dyn FireScheduleAccess,
    ctx: &'a mut FireTickContext<'b, H>,
    changes: Vec<WorldChange>,
}
impl<H: FireSimulationHost> Run<'_, '_, H> {
    fn schedule_next(&mut self, p: Position) -> Result<()> {
        let delay = 30 + self.ctx.random.next_bounded_i32(10)?;
        self.scheduler.schedule_fire(p,self.ctx.game_time,delay,self.ctx.sub_tick_order)?;
        Ok(())
    }
    fn set(&mut self, p: Position, state: u32, flags: u32) -> Result<()> {
        // Level.setBlock refuses these coordinates without running onPlace.
        if p.1 < self.ctx.facts.minimum_build_height
            || p.1 >= self.ctx.facts.maximum_build_height
            || !(-30_000_000..30_000_000).contains(&p.0)
            || !(-30_000_000..30_000_000).contains(&p.2)
        {
            return Ok(());
        }
        let old = block(self.ctx.world, p)?;
        if old == state {
            return Ok(());
        }
        let changes = self.ctx.host.set_block(self.ctx.world, p, state, flags)?;
        let installed = block(self.ctx.world, p)?;
        ensure!(
            installed == state
                || changes
                    .iter()
                    .any(|c| matches!(c,WorldChange::Block(b) if (b.x,b.y,b.z)==p&&b.state==state)),
            "fire host did not install requested state at {p:?}"
        );
        self.changes.extend(changes);
        self.on_place(p, old, state)
    }
    fn remove(&mut self, p: Position) -> Result<()> {
        let current = block(self.ctx.world, p)?;
        let replacement = fluid_simulation::legacy(FluidState::from_block(current)?)?;
        self.set(p, replacement, 3)
    }
    fn valid_fire_location(&mut self, p: Position) -> Result<bool> {
        for d in DIRECTIONS {
            if ignite_odds(block(self.ctx.world, step(p, d))?)? > 0 {
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn can_survive(&mut self, p: Position, state: u32) -> Result<bool> {
        let below = step(p, Direction::Down);
        let lower = block(self.ctx.world, below)?;
        if fire_kind(state)? == 2 {
            return Ok(placement::in_block_tag(
                lower,
                "minecraft:soul_fire_base_blocks",
            ));
        }
        if self.ctx.host.sturdy_up(self.ctx.world, below, lower)? {
            return Ok(true);
        }
        self.valid_fire_location(p)
    }
    fn on_place(&mut self, p: Position, old: u32, new: u32) -> Result<()> {
        let kind = fire_kind(new)?;
        if kind == 0 || fire_kind(old)? == kind {
            return Ok(());
        }
        let portal = if self.ctx.facts.portal_dimension {
            find_empty_portal(self.ctx.world, p, self.ctx.facts.minimum_build_height)?
        } else {
            None
        };
        if let Some(portal) = portal {
            let state = block_state_with_properties(
                data().block_ids["ed"],
                &BTreeMap::from([(
                    "axis".into(),
                    if portal.axis == Axis::X { "x" } else { "z" }.into(),
                )]),
            )?;
            for position in portal.positions() {
                self.set(position, state, 18)?;
            }
        } else if !self.can_survive(p, new)? {
            self.remove(p)?;
        }
        // FireBlock overrides BaseFireBlock and schedules even when the base
        // callback just replaced it with a portal or removed unsupported fire.
        if kind == 1 {
            self.schedule_next(p)?;
        }
        Ok(())
    }
    fn near_rain(&mut self, p: Position) -> Result<bool> {
        for at in [
            p,
            step(p, Direction::West),
            step(p, Direction::East),
            step(p, Direction::North),
            step(p, Direction::South),
        ] {
            if self.ctx.host.is_raining_at(self.ctx.world, at)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn ignition_chance(&mut self, p: Position) -> Result<i32> {
        if !facts(block(self.ctx.world, p)?)?.2 {
            return Ok(0);
        }
        let mut chance = 0;
        for d in DIRECTIONS {
            chance = chance.max(ignite_odds(block(self.ctx.world, step(p, d))?)?);
        }
        Ok(chance)
    }
    fn burn_out(&mut self, p: Position, bound: i32, old_age: i32) -> Result<()> {
        let odds = burn_odds(block(self.ctx.world, p)?)?;
        // Even zero burn odds consume the original random.nextInt(bound).
        if self.ctx.random.next_bounded_i32(bound)? < odds {
            let old = block(self.ctx.world, p)?;
            if self.ctx.random.next_bounded_i32(old_age + 10)? < 5
                && !self.ctx.host.is_raining_at(self.ctx.world, p)?
            {
                let age = (old_age + self.ctx.random.next_bounded_i32(5)? / 4).min(15);
                let next = state_at(self.ctx.world, self.ctx.host, p, Some(age))?;
                self.set(p, next, 3)?;
            } else {
                self.remove(p)?;
            }
            if facts(old)?.3 {
                self.ctx
                    .host
                    .prime_tnt(self.ctx.world, p, self.ctx.random)?;
            }
        }
        Ok(())
    }
    fn tick(&mut self, p: Position, mut state: u32) -> Result<()> {
        self.schedule_next(p)?;
        if !self.ctx.facts.do_fire_tick {
            return Ok(());
        }
        if !self.can_survive(p, state)? {
            self.remove(p)?;
        }
        let below = step(p, Direction::Down);
        let lower = block(self.ctx.world, below)?;
        let eternal = self.ctx.host.infinitely_burning(
            self.ctx.world,
            below,
            lower,
            &self.ctx.facts.infiniburn_tag,
        )?;
        let old_age = age(state)?;
        if !eternal
            && self.ctx.facts.is_raining
            && self.near_rain(p)?
            && self.ctx.random.next_f32() < 0.2_f32 + old_age as f32 * 0.03_f32
        {
            self.remove(p)?;
            return Ok(());
        }
        let new_age = (old_age + self.ctx.random.next_bounded_i32(3)? / 2).min(15);
        if old_age != new_age {
            state = set_age(state, new_age)?;
            self.set(p, state, 4)?;
        }
        if !eternal {
            if !self.valid_fire_location(p)? {
                let lower = block(self.ctx.world, below)?;
                if !self.ctx.host.sturdy_up(self.ctx.world, below, lower)? || old_age > 3 {
                    self.remove(p)?;
                }
                return Ok(());
            }
            if old_age == 15
                && self.ctx.random.next_bounded_i32(4)? == 0
                && ignite_odds(block(self.ctx.world, below)?)? == 0
            {
                self.remove(p)?;
                return Ok(());
            }
        }
        let humid = self.ctx.host.increased_fire_burnout(self.ctx.world, p)?;
        let adjustment = if humid { -50 } else { 0 };
        for (direction, bound) in [
            (Direction::East, 300),
            (Direction::West, 300),
            (Direction::Down, 250),
            (Direction::Up, 250),
            (Direction::North, 300),
            (Direction::South, 300),
        ] {
            self.burn_out(step(p, direction), bound + adjustment, old_age)?;
        }
        for x in -1..=1 {
            for z in -1..=1 {
                for y in -1..=4 {
                    if x == 0 && y == 0 && z == 0 {
                        continue;
                    }
                    let bound = 100 + if y > 1 { (y - 1) * 100 } else { 0 };
                    let target = offset(p, x, y, z);
                    let ignite = self.ignition_chance(target)?;
                    if ignite <= 0 {
                        continue;
                    }
                    let mut chance =
                        (ignite + 40 + self.ctx.facts.difficulty_id * 7) / (old_age + 30);
                    if humid {
                        chance /= 2;
                    }
                    if chance <= 0 || self.ctx.random.next_bounded_i32(bound)? > chance {
                        continue;
                    }
                    if self.ctx.facts.is_raining && self.near_rain(target)? {
                        continue;
                    }
                    let age = (old_age + self.ctx.random.next_bounded_i32(5)? / 4).min(15);
                    let next = state_at(self.ctx.world, self.ctx.host, target, Some(age))?;
                    self.set(target, next, 3)?;
                }
            }
        }
        Ok(())
    }
}

/// Empty portal frame, matching PortalShape's 2..21 by 3..21 interior limits.
#[derive(Clone, Copy, Debug)]
pub struct EmptyPortal {
    pub axis: Axis,
    pub bottom_left: Position,
    pub width: i32,
    pub height: i32,
    right: Direction,
}
impl EmptyPortal {
    /// Original BlockPos.betweenClosed order: X fastest, then Y, then Z.
    pub fn positions(self) -> Vec<Position> {
        let end = along(
            offset(self.bottom_left, 0, self.height - 1, 0),
            self.right,
            self.width - 1,
        );
        let min = (
            self.bottom_left.0.min(end.0),
            self.bottom_left.1.min(end.1),
            self.bottom_left.2.min(end.2),
        );
        let max = (
            self.bottom_left.0.max(end.0),
            self.bottom_left.1.max(end.1),
            self.bottom_left.2.max(end.2),
        );
        let mut out = Vec::new();
        for z in min.2..=max.2 {
            for y in min.1..=max.1 {
                for x in min.0..=max.0 {
                    out.push((x, y, z));
                }
            }
        }
        out
    }
}
fn portal_empty(state: u32) -> Result<bool> {
    Ok(facts(state)?.2
        || placement::in_block_tag(state, "minecraft:fire")
        || placement::placement_block_name(state)? == "minecraft:nether_portal")
}
fn frame(state: u32) -> bool {
    state == data().block_ids["co"]
}
fn edge_distance(world: &mut RuntimeWorld, p: Position, d: Direction) -> Result<i32> {
    for distance in 0..=21 {
        let at = along(p, d, distance);
        let state = block(world, at)?;
        if !portal_empty(state)? {
            return Ok(if frame(state) { distance } else { 0 });
        }
        if !frame(block(world, step(at, Direction::Down))?) {
            break;
        }
    }
    Ok(0)
}
fn empty_portal_axis(
    world: &mut RuntimeWorld,
    mut p: Position,
    axis: Axis,
    min_y: i32,
) -> Result<Option<EmptyPortal>> {
    let right = if axis == Axis::X {
        Direction::West
    } else {
        Direction::South
    };
    let bottom_limit = min_y.max(p.1.wrapping_sub(21));
    while p.1 > bottom_limit && portal_empty(block(world, step(p, Direction::Down))?)? {
        p = step(p, Direction::Down);
    }
    let distance = edge_distance(world, p, right.opposite())? - 1;
    if distance < 0 {
        return Ok(None);
    }
    let bottom = along(p, right.opposite(), distance);
    let width = edge_distance(world, bottom, right)?;
    if !(2..=21).contains(&width) {
        return Ok(None);
    }
    let mut portal_count = 0;
    let mut height = 21;
    'height: for y in 0..21 {
        if !frame(block(world, along(offset(bottom, 0, y, 0), right, -1))?)
            || !frame(block(world, along(offset(bottom, 0, y, 0), right, width))?)
        {
            height = y;
            break;
        }
        for x in 0..width {
            let state = block(world, along(offset(bottom, 0, y, 0), right, x))?;
            if !portal_empty(state)? {
                height = y;
                break 'height;
            }
            if placement::placement_block_name(state)? == "minecraft:nether_portal" {
                portal_count += 1;
            }
        }
    }
    if !(3..=21).contains(&height) {
        return Ok(None);
    }
    for x in 0..width {
        if !frame(block(world, along(offset(bottom, 0, height, 0), right, x))?) {
            return Ok(None);
        }
    }
    if portal_count != 0 {
        return Ok(None);
    }
    Ok(Some(EmptyPortal {
        axis,
        bottom_left: bottom,
        width,
        height,
        right,
    }))
}
/// PortalShape.findEmptyPortalShape with initial X axis and the original Z
/// fallback. It rejects partly filled portals, rather than assuming emptiness.
pub fn find_empty_portal(
    world: &mut RuntimeWorld,
    p: Position,
    min_y: i32,
) -> Result<Option<EmptyPortal>> {
    if let Some(portal) = empty_portal_axis(world, p, Axis::X, min_y)? {
        return Ok(Some(portal));
    }
    empty_portal_axis(world, p, Axis::Z, min_y)
}
