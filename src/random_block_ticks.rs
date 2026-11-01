//! ServerLevel.tickChunk's random-block slice and original position selector.
//!
//! Level.randValue is a separate signed-int LCG, not Level.random. Weather
//! prelude callbacks and block/fluid callbacks receive the one actual level
//! random stream. Chunk encounter/shuffle order belongs to the source host.
use crate::{
    fluid_physics::FluidState, random::LegacyRandomSource, runtime_world::RuntimeWorld,
    world_rules::WorldRules,
};
use anyhow::{anyhow, bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::OnceLock};

pub type Position = (i32, i32, i32);
pub type Chunk = (i32, i32);

/// Exact Level.getBlockRandomPos, including Java signed overflow and shifts.
/// The caller retains the actual Level constructor's randValue across visits.
pub fn get_block_random_pos(rand_value: &mut i32, x: i32, y: i32, z: i32, y_mask: i32) -> Position {
    *rand_value = rand_value.wrapping_mul(3).wrapping_add(1_013_904_223);
    let bits = *rand_value >> 2;
    (
        x.wrapping_add(bits & 15),
        y.wrapping_add((bits >> 16) & y_mask),
        z.wrapping_add((bits >> 8) & 15),
    )
}
#[derive(Deserialize)]
struct BlockFacts {
    name: String,
    min: u32,
    max: u32,
    random_tick_class: String,
    precipitation_class: String,
}
#[derive(Deserialize)]
struct Facts {
    minecraft: String,
    jar_sha256: String,
    state_flags: Vec<u8>,
    fluid_random_classes: Vec<String>,
    blocks: Vec<BlockFacts>,
}
fn facts() -> &'static Facts {
    static DATA: OnceLock<Facts> = OnceLock::new();
    DATA.get_or_init(|| {
        let facts: Facts = serde_json::from_str(include_str!("../fixtures/random-tick-facts.json"))
            .expect("actual 1.21 random tick facts");
        assert_eq!(facts.minecraft, "1.21");
        assert_eq!(
            facts.jar_sha256,
            "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"
        );
        assert_eq!(facts.state_flags.len(), 26684);
        assert_eq!(facts.fluid_random_classes.len(), 26684);
        facts
    })
}
fn flags(state: u32) -> Result<u8> {
    facts()
        .state_flags
        .get(state as usize)
        .copied()
        .ok_or_else(|| anyhow!("unknown 1.21 random-tick state {state}"))
}
fn block_facts(state: u32) -> Result<&'static BlockFacts> {
    let i = facts()
        .blocks
        .partition_point(|b| b.min <= state)
        .checked_sub(1)
        .context("unknown random-tick block state")?;
    let b = &facts().blocks[i];
    ensure!(state <= b.max, "unknown random-tick block state {state}");
    Ok(b)
}
pub fn is_randomly_ticking_block(state: u32) -> Result<bool> {
    Ok(flags(state)? & 1 != 0)
}
pub fn is_randomly_ticking_fluid(state: u32) -> Result<bool> {
    Ok(flags(state)? & 2 != 0)
}
pub fn precipitation_handler_class(state: u32) -> Result<&'static str> {
    Ok(&block_facts(state)?.precipitation_class)
}

/// Util.shuffle(list, Level.random), called on the original resident holder
/// encounter order before ServerChunkCache applies per-chunk tick gates.
pub fn shuffle_tick_chunks<T>(chunks: &mut [T], random: &mut LegacyRandomSource) -> Result<()> {
    ensure!(
        chunks.len() <= i32::MAX as usize,
        "chunk list exceeds Java List range"
    );
    for length in (2..=chunks.len()).rev() {
        let other = random.next_bounded_i32(length as i32)? as usize;
        chunks.swap(length - 1, other);
    }
    Ok(())
}
/// DimensionType.timeOfDay and Level.updateSkyBrightness, preserving float
/// multiplications before widening and the vanilla cosine lookup table.
pub fn sky_darken(
    day_time: i64,
    fixed_time: Option<i64>,
    weather: &crate::tick::WeatherState,
) -> i32 {
    let time = fixed_time.unwrap_or(day_time) as f64 / 24000.0 - 0.25;
    let phase = time - (time.floor() as i64) as f64;
    let curved = 0.5 - crate::hotspot_math::sin_cos(phase * std::f64::consts::PI).1 / 2.0;
    let day = ((phase * 2.0 + curved) as f32) / 3.0_f32;
    let cosine = f64::from(crate::geometry::vanilla_cos(day * 6.2831855_f32)).clamp(-0.25, 0.25);
    let sun = 0.5 + 2.0 * cosine;
    let rain = 1.0 - f64::from(weather.rain_level_at(1.0) * 5.0_f32) / 16.0;
    let thunder = 1.0 - f64::from(weather.thunder_level_at(1.0) * 5.0_f32) / 16.0;
    ((1.0 - (sun * rain) * thunder) * 11.0) as i32
}

/// Actual LevelChunkSection signed-short fields. Constructor recount and
/// setBlockState have different fluid tests in the original 1.21 bytecode:
/// recount counts randomly-ticking fluid; writes count every nonempty fluid.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectionTickCounters {
    pub non_empty_blocks: i16,
    pub ticking_blocks: i16,
    pub ticking_fluids: i16,
}
impl SectionTickCounters {
    pub fn is_randomly_ticking(self) -> bool {
        self.ticking_blocks > 0 || self.ticking_fluids > 0
    }
    fn recount_add(&mut self, state: u32) -> Result<()> {
        let f = flags(state)?;
        if f & 8 != 0 {
            self.non_empty_blocks = self.non_empty_blocks.wrapping_add(1);
            if f & 1 != 0 {
                self.ticking_blocks = self.ticking_blocks.wrapping_add(1);
            }
        }
        if f & 4 != 0 {
            self.non_empty_blocks = self.non_empty_blocks.wrapping_add(1);
            if f & 2 != 0 {
                self.ticking_fluids = self.ticking_fluids.wrapping_add(1);
            }
        }
        Ok(())
    }
    pub fn on_block_write(&mut self, old: u32, new: u32) -> Result<()> {
        let old = flags(old)?;
        let new = flags(new)?;
        if old & 8 != 0 {
            self.non_empty_blocks = self.non_empty_blocks.wrapping_sub(1);
            if old & 1 != 0 {
                self.ticking_blocks = self.ticking_blocks.wrapping_sub(1);
            }
        }
        if old & 4 != 0 {
            self.ticking_fluids = self.ticking_fluids.wrapping_sub(1);
        }
        if new & 8 != 0 {
            self.non_empty_blocks = self.non_empty_blocks.wrapping_add(1);
            if new & 1 != 0 {
                self.ticking_blocks = self.ticking_blocks.wrapping_add(1);
            }
        }
        if new & 4 != 0 {
            self.ticking_fluids = self.ticking_fluids.wrapping_add(1);
        }
        Ok(())
    }
}
/// Retain this owner for each actual resident section. Recount only on its real
/// construction/deserialization boundary, not after every edit or FULL demotion.
#[derive(Default)]
pub struct RandomTickSectionIndex {
    sections: BTreeMap<(i32, i32, i32), SectionTickCounters>,
}
impl RandomTickSectionIndex {
    pub fn contains_chunk(&self, chunk: Chunk, min_section: i32, section_count: u32) -> bool {
        (0..section_count).all(|i| {
            self.sections
                .contains_key(&(chunk.0, min_section.wrapping_add(i as i32), chunk.1))
        })
    }
    pub fn load_recalculated_chunk(
        &mut self,
        world: &mut RuntimeWorld,
        chunk: Chunk,
        min_section: i32,
        section_count: u32,
    ) -> Result<()> {
        ensure!(
            section_count > 0 && section_count <= 256,
            "invalid random tick section count"
        );
        ensure!(
            !self.sections.keys().any(|&(x, _, z)| (x, z) == chunk),
            "random tick section owner already exists"
        );
        let x = chunk
            .0
            .checked_mul(16)
            .context("random tick chunk X overflow")?;
        let z = chunk
            .1
            .checked_mul(16)
            .context("random tick chunk Z overflow")?;
        x.checked_add(15).context("random tick block X overflow")?;
        z.checked_add(15).context("random tick block Z overflow")?;
        let mut staged = Vec::with_capacity(section_count as usize);
        for i in 0..section_count {
            let sy = min_section
                .checked_add(i as i32)
                .context("random tick section overflow")?;
            let y = sy
                .checked_mul(16)
                .context("random tick section Y overflow")?;
            y.checked_add(15).context("random tick block Y overflow")?;
            let mut counts = SectionTickCounters::default();
            for ly in 0..16 {
                for lz in 0..16 {
                    for lx in 0..16 {
                        counts.recount_add(world.block(x + lx, y + ly, z + lz)?)?;
                    }
                }
            }
            staged.push(((chunk.0, sy, chunk.1), counts));
        }
        self.sections.extend(staged);
        Ok(())
    }
    pub fn section(&self, chunk: Chunk, section_y: i32) -> Result<SectionTickCounters> {
        self.sections
            .get(&(chunk.0, section_y, chunk.1))
            .copied()
            .context("actual random tick section counters unavailable")
    }
    /// Call at the committed write boundary, with its observed old state,
    /// under the same world/section lock. Unowned sections are errors.
    pub fn observe_block_write(&mut self, position: Position, old: u32, new: u32) -> Result<()> {
        self.sections
            .get_mut(&(
                position.0.div_euclid(16),
                position.1.div_euclid(16),
                position.2.div_euclid(16),
            ))
            .context("block write requires original random-tick counter owner")?
            .on_block_write(old, new)
    }
    pub fn unload_chunk(&mut self, chunk: Chunk) -> Vec<(i32, SectionTickCounters)> {
        let keys = self
            .sections
            .keys()
            .copied()
            .filter(|&(x, _, z)| (x, z) == chunk)
            .collect::<Vec<_>>();
        keys.into_iter()
            .map(|key| (key.1, self.sections.remove(&key).unwrap()))
            .collect()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ReadyChunkSections {
    pub min_section_y: i32,
    pub section_count: u32,
}
#[derive(Clone, Copy, Debug)]
pub struct SampledBlockTick {
    pub position: Position,
    pub state: u32,
    pub block_name: &'static str,
    pub declaring_class: &'static str,
}
#[derive(Clone, Copy, Debug)]
pub struct SampledFluidTick {
    pub position: Position,
    pub sampled_block_state: u32,
    pub fluid: FluidState,
    pub declaring_class: &'static str,
}

pub use crate::spreading_grass::GrassRandomTickHost;
pub trait RandomBlockTickHost:
    GrassRandomTickHost + crate::crop_random_ticks::FarmRandomTickHost
{
    /// Caller has already selected the actual source encounter/shuffle order.
    /// Certify current BLOCK readiness, player/tick-rate gates and section bounds.
    fn ready_chunk_sections(
        &mut self,
        world: &mut RuntimeWorld,
        chunk: Chunk,
    ) -> Result<ReadyChunkSections>;
    fn section_randomly_ticking(
        &mut self,
        world: &mut RuntimeWorld,
        chunk: Chunk,
        section_y: i32,
    ) -> Result<bool>;
    fn is_raining(&mut self, world: &mut RuntimeWorld) -> Result<bool>;
    fn is_thundering(&mut self, world: &mut RuntimeWorld) -> Result<bool>;
    /// Execute findLightningTargetAround/isRainingAt/difficulty/spawns from this
    /// original LCG seed; Level.random draws inside those callbacks stay shared.
    fn thunder(
        &mut self,
        world: &mut RuntimeWorld,
        seed: Position,
        random: &mut LegacyRandomSource,
        rand_value: &mut i32,
    ) -> Result<()>;
    /// Original tickPrecipitation, including freezing/snow/precipitation effects.
    fn precipitation(
        &mut self,
        world: &mut RuntimeWorld,
        seed: Position,
        random: &mut LegacyRandomSource,
        rand_value: &mut i32,
    ) -> Result<()>;
    fn random_fluid_tick(
        &mut self,
        world: &mut RuntimeWorld,
        tick: SampledFluidTick,
        random: &mut LegacyRandomSource,
        rand_value: &mut i32,
    ) -> Result<()>;
    fn other_random_block_tick(
        &mut self,
        _world: &mut RuntimeWorld,
        tick: SampledBlockTick,
        _random: &mut LegacyRandomSource,
        _rand_value: &mut i32,
    ) -> Result<()> {
        bail!(
            "unported original randomTick {} ({}) at {:?}",
            tick.block_name,
            tick.declaring_class,
            tick.position
        )
    }
}
#[derive(Debug, Default, Clone, Copy)]
pub struct RandomTickReport {
    pub precipitation_attempts: u64,
    pub precipitation_callbacks: u64,
    pub thunder_draws: u64,
    pub thunder_callbacks: u64,
    pub checked_sections: u64,
    pub selected_sections: u64,
    pub position_samples: u64,
    pub block_callbacks: u64,
    pub fluid_callbacks: u64,
}
pub struct RandomBlockTickContext<'a, H> {
    pub world: &'a mut RuntimeWorld,
    pub random: &'a mut LegacyRandomSource,
    pub rand_value: &'a mut i32,
    pub host: &'a mut H,
}
/// Exact random tick portion of one ServerLevel.tickChunk. Profiling labels do
/// not affect world state. No chunk sorting, RNG reseeding or capped attempts.
pub fn tick_chunk<H: RandomBlockTickHost>(
    context: &mut RandomBlockTickContext<'_, H>,
    chunk: Chunk,
    random_tick_speed: i32,
) -> Result<RandomTickReport> {
    let sections = context.host.ready_chunk_sections(context.world, chunk)?;
    ensure!(
        sections.section_count > 0 && sections.section_count <= 256,
        "invalid tick chunk section layout"
    );
    let (x, z) = (chunk.0.wrapping_shl(4), chunk.1.wrapping_shl(4));
    let raining = context.host.is_raining(context.world)?;
    let mut report = RandomTickReport::default();
    if raining && context.host.is_thundering(context.world)? {
        report.thunder_draws += 1;
        if context.random.next_bounded_i32(100_000)? == 0 {
            let seed = get_block_random_pos(context.rand_value, x, 0, z, 15);
            report.position_samples += 1;
            context
                .host
                .thunder(context.world, seed, context.random, context.rand_value)?;
            report.thunder_callbacks += 1;
        }
    }
    for _ in 0..random_tick_speed {
        report.precipitation_attempts += 1;
        if context.random.next_bounded_i32(48)? == 0 {
            let seed = get_block_random_pos(context.rand_value, x, 0, z, 15);
            report.position_samples += 1;
            context
                .host
                .precipitation(context.world, seed, context.random, context.rand_value)?;
            report.precipitation_callbacks += 1;
        }
    }
    if random_tick_speed <= 0 {
        return Ok(report);
    }
    for index in 0..sections.section_count {
        let sy = sections.min_section_y.wrapping_add(index as i32);
        report.checked_sections += 1;
        // Read once at section entry. Callback writes may change the counter,
        // but the remaining attempts in this entered section still execute.
        if !context
            .host
            .section_randomly_ticking(context.world, chunk, sy)?
        {
            continue;
        }
        report.selected_sections += 1;
        for _ in 0..random_tick_speed {
            let position = get_block_random_pos(context.rand_value, x, sy.wrapping_shl(4), z, 15);
            report.position_samples += 1;
            let state = context.world.block(position.0, position.1, position.2)?;
            let state_flags = flags(state)?;
            let b = block_facts(state)?;
            if state_flags & 1 != 0 {
                report.block_callbacks += 1;
                match b.random_tick_class.as_str() {
                    // The original BlockBehaviour implementation is empty.
                    "dtb" => {}
                    "dnq" => crate::spreading_grass::random_tick_spreading_snowy_dirt(
                        context.world,
                        state,
                        position,
                        context.random,
                        context.host,
                    )?,
                    "div" => crate::crop_random_ticks::random_tick_farmland(
                        context.world,
                        state,
                        position,
                        context.random,
                        context.host,
                    )?,
                    "dht" | "dft" => crate::crop_random_ticks::random_tick_crop(
                        context.world,
                        state,
                        position,
                        context.random,
                        context.host,
                    )?,
                    // LiquidBlock.randomTick delegates to its sampled fluid,
                    // then ServerLevel may invoke that same fluid a second time.
                    "dko" => {
                        context.host.random_fluid_tick(
                            context.world,
                            sampled_fluid(state, position)?,
                            context.random,
                            context.rand_value,
                        )?;
                        report.fluid_callbacks += 1;
                    }
                    _ => context.host.other_random_block_tick(
                        context.world,
                        SampledBlockTick {
                            position,
                            state,
                            block_name: &b.name,
                            declaring_class: &b.random_tick_class,
                        },
                        context.random,
                        context.rand_value,
                    )?,
                }
            }
            if state_flags & 2 != 0 {
                // Uses the original sampled BlockState even when its block
                // callback just replaced the current world position.
                context.host.random_fluid_tick(
                    context.world,
                    sampled_fluid(state, position)?,
                    context.random,
                    context.rand_value,
                )?;
                report.fluid_callbacks += 1;
            }
        }
    }
    Ok(report)
}
pub fn tick_chunk_with_rules<H: RandomBlockTickHost>(
    context: &mut RandomBlockTickContext<'_, H>,
    chunk: Chunk,
    rules: &WorldRules,
) -> Result<RandomTickReport> {
    tick_chunk(context, chunk, rules.random_tick_speed())
}
fn sampled_fluid(state: u32, position: Position) -> Result<SampledFluidTick> {
    let class = facts()
        .fluid_random_classes
        .get(state as usize)
        .context("missing fluid random callback class")?;
    ensure!(
        !class.is_empty(),
        "sampled fluid has no random-tick callback"
    );
    Ok(SampledFluidTick {
        position,
        sampled_block_state: state,
        fluid: FluidState::from_block(state)?,
        declaring_class: class,
    })
}
