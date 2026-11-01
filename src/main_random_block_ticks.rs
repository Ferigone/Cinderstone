//! Main-owned random-tick counters and native callback composition.
use super::*;
use mc121_rust::{
    random_block_ticks::{RandomTickSectionIndex, SectionTickCounters},
    world_interactions::{InteractionEvent, InteractionPlan},
};

pub(super) struct MainRandomBlockTicks {
    pub(super) sections: RandomTickSectionIndex,
    native_source: bool,
    stopped: Option<String>,
}
impl MainRandomBlockTicks {
    pub(super) fn new(native_source: bool) -> Self {
        Self {
            sections: RandomTickSectionIndex::default(),
            native_source,
            stopped: None,
        }
    }
    pub(super) fn require_healthy(&self) -> Result<()> {
        ensure!(
            self.native_source,
            "random tick native source is unavailable"
        );
        ensure!(
            self.stopped.is_none(),
            "random tick runtime stopped: {}",
            self.stopped.as_deref().unwrap_or("")
        );
        Ok(())
    }
    pub(super) fn prepare_chunk(
        &mut self,
        world: &mut WorldBackend,
        chunk: (i32, i32),
    ) -> Result<()> {
        ensure!(
            self.native_source,
            "random tick section source is unavailable"
        );
        let WorldBackend::Fixture(flat) = world else {
            bail!("random tick counters require actual native chunk construction");
        };
        ensure!(
            flat.base_states() == [0, 1, 10, 9] && flat.biome_id() == 39,
            "random tick source is not canonical native plains"
        );
        let min = world::MIN_Y.div_euclid(16);
        let count = ((world::MAX_Y - world::MIN_Y + 1) / 16) as u32;
        if !self.sections.contains_chunk(chunk, min, count) {
            self.sections
                .load_recalculated_chunk(world, chunk, min, count)?;
        }
        Ok(())
    }
    pub(super) fn prepare_plan(
        &mut self,
        world: &mut WorldBackend,
        plan: &InteractionPlan,
    ) -> Result<()> {
        let chunks = plan
            .events
            .iter()
            .filter_map(|event| match event {
                InteractionEvent::Write { position, .. } => {
                    Some((position.0.div_euclid(16), position.2.div_euclid(16)))
                }
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        for chunk in chunks {
            self.prepare_chunk(world, chunk)?;
        }
        Ok(())
    }
    pub(super) fn observe_plan(&mut self, plan: &InteractionPlan) -> Result<()> {
        for event in &plan.events {
            if let InteractionEvent::Write {
                position,
                old_state,
                state,
                ..
            } = event
            {
                self.sections
                    .observe_block_write(*position, *old_state, *state)?;
            }
        }
        Ok(())
    }
    pub(super) fn section(&self, chunk: (i32, i32), section_y: i32) -> Result<SectionTickCounters> {
        self.sections.section(chunk, section_y)
    }
}

use super::main_fire_simulation::{MainFireHost, MainFireRuntime};
use mc121_rust::{
    crop_random_ticks::{self, CropRandomTickHost, FarmRandomTickHost},
    fire_simulation::{FireSimulationHost, FireTickContext},
    fluid_physics::FluidKind,
    fluid_simulation::{FluidSimulation, FluidSimulationHost, FluidWorldFacts},
    rain_physics::Precipitation,
    random_block_ticks::{
        self, GrassRandomTickHost, RandomBlockTickContext, RandomBlockTickHost, RandomTickReport,
        ReadyChunkSections, SampledFluidTick,
    },
};

#[derive(Default, Debug)]
pub(super) struct MainRandomTickReport {
    pub source_unavailable: Option<&'static str>,
    pub resident_chunks: usize,
    pub executed_chunks: usize,
    pub reports: Vec<((i32, i32), RandomTickReport)>,
}
struct MainRandomHost<'a> {
    fire: MainFireHost<'a>,
    fire_runtime: &'a mut MainFireRuntime,
    block_runtime: &'a mut super::main_block_ticks::MainBlockRuntime,
    fluids: &'a mut FluidSimulation,
    sub_tick_order: &'a mut i64,
    world_context: &'a WorldContext,
    fluid_facts: FluidWorldFacts,
    sky_darken: i32,
    game_time: i64,
}
impl GrassRandomTickHost for MainRandomHost<'_> {
    fn max_local_raw_brightness(
        &mut self,
        world: &mut WorldBackend,
        p: (i32, i32, i32),
    ) -> Result<i32> {
        self.raw_brightness(world, p, self.sky_darken)
    }
    fn set_block(
        &mut self,
        world: &mut WorldBackend,
        p: (i32, i32, i32),
        state: u32,
        flags: u32,
        _random: &mut LegacyRandomSource,
    ) -> Result<()> {
        let changes = self
            .fire
            .fluid
            .set_block_with_primary_callback(world, p, state, flags, false)?;
        self.fire.fluid.block_changes(&changes)
    }
}
impl CropRandomTickHost for MainRandomHost<'_> {
    fn raw_brightness(
        &mut self,
        world: &mut WorldBackend,
        p: (i32, i32, i32),
        sky_darken: i32,
    ) -> Result<i32> {
        let WorldBackend::Fixture(flat) = world else {
            bail!("native random light source is unavailable");
        };
        let light = self.fire.fluid.chunks.native_lighting_mut()?;
        light.require_current(flat)?;
        Ok(i32::from(light.raw_brightness(
            flat,
            p,
            u8::try_from(sky_darken)?,
        )?))
    }
    fn set_block(
        &mut self,
        world: &mut WorldBackend,
        p: (i32, i32, i32),
        state: u32,
        flags: u32,
        random: &mut LegacyRandomSource,
    ) -> Result<()> {
        GrassRandomTickHost::set_block(self, world, p, state, flags, random)
    }
}
impl FarmRandomTickHost for MainRandomHost<'_> {
    fn is_raining_at(&mut self, world: &mut WorldBackend, p: (i32, i32, i32)) -> Result<bool> {
        self.fire.is_raining_at(world, p)
    }
    fn turn_farmland_to_dirt(
        &mut self,
        world: &mut WorldBackend,
        p: (i32, i32, i32),
        _state: u32,
        random: &mut LegacyRandomSource,
    ) -> Result<()> {
        // The source's entity query is allowed to be empty only from these
        // actual owned native registries. Intersecting bodies require their
        // relative-teleport publication before the dirt write.
        ensure!(
            self.fire.fluid.active_items == 0,
            "farmland push requires live item-body movement integration"
        );
        let region = crop_random_ticks::farmland_push_region(p);
        for player in &self.fire.fluid.players {
            if player.gamemode == 3 {
                continue;
            }
            let d = player_physics::player_dimensions(player.metadata.pose, 1.)?;
            let bounds = Aabb::player(
                Vec3::new(player.position.x, player.position.y, player.position.z),
                d.width,
                d.height,
            );
            ensure!(
                !region.intersects(&bounds),
                "farmland push requires actual player relative teleport"
            );
        }
        self.fire.fluid.interactions.can_append(1)?;
        GrassRandomTickHost::set_block(self, world, p, 10, 3, random)?;
        self.fire.fluid.interactions.append(
            -1,
            [InteractionEvent::GameEvent {
                position: p,
                name: "minecraft:block_change",
            }],
        );
        Ok(())
    }
}
impl RandomBlockTickHost for MainRandomHost<'_> {
    fn ready_chunk_sections(
        &mut self,
        world: &mut WorldBackend,
        chunk: (i32, i32),
    ) -> Result<ReadyChunkSections> {
        ensure!(
            main_fluid_simulation::current_block_ready(self.fire.fluid.chunks, chunk)?,
            "random block tick lacks current BLOCK future/range"
        );
        self.fire.fluid.chunks.require_supported_lighting()?;
        self.fire
            .fluid
            .random_ticks
            .as_deref_mut()
            .context("random section owner missing")?
            .prepare_chunk(world, chunk)?;
        Ok(ReadyChunkSections {
            min_section_y: world::MIN_Y.div_euclid(16),
            section_count: ((world::MAX_Y - world::MIN_Y + 1) / 16) as u32,
        })
    }
    fn section_randomly_ticking(
        &mut self,
        _world: &mut WorldBackend,
        chunk: (i32, i32),
        section_y: i32,
    ) -> Result<bool> {
        Ok(self
            .fire
            .fluid
            .random_ticks
            .as_deref()
            .context("random section owner missing")?
            .section(chunk, section_y)?
            .is_randomly_ticking())
    }
    fn is_raining(&mut self, _world: &mut WorldBackend) -> Result<bool> {
        Ok(self.world_context.weather().is_raining())
    }
    fn is_thundering(&mut self, _world: &mut WorldBackend) -> Result<bool> {
        let dimension = self.world_context.dimension;
        Ok(self
            .world_context
            .weather()
            .is_thundering(dimension.has_skylight, dimension.has_ceiling))
    }
    fn thunder(
        &mut self,
        _world: &mut WorldBackend,
        seed: (i32, i32, i32),
        _random: &mut LegacyRandomSource,
        _rand_value: &mut i32,
    ) -> Result<()> {
        bail!("random thunder seed {seed:?} requires original lightning-target/entity spawning callbacks")
    }
    fn precipitation(
        &mut self,
        world: &mut WorldBackend,
        seed: (i32, i32, i32),
        _random: &mut LegacyRandomSource,
        _rand_value: &mut i32,
    ) -> Result<()> {
        let WorldBackend::Fixture(flat) = &*world else {
            bail!("native precipitation source unavailable");
        };
        ensure!(
            flat.biome_id() == 39,
            "native precipitation requires canonical plains"
        );
        let top = self
            .fire
            .fluid
            .chunks
            .native_lighting_mut()?
            .motion_blocking_height(flat, seed)?;
        let top = (seed.0, top, seed.2);
        let below = (top.0, top.1.wrapping_sub(1), top.2);
        let rain = self
            .fire
            .rain
            .as_deref_mut()
            .context("actual plains temperature cache owner missing")?;
        // Biome.shouldFreeze first tests warmEnoughToRain at the below point.
        // A genuinely cold point needs the full freeze executor, never false.
        ensure!(
            rain.precipitation_at(below) == Precipitation::Rain,
            "cold precipitation requires original freezing callbacks"
        );
        if !self.world_context.weather().is_raining() {
            return Ok(());
        }
        if self
            .world_context
            .rules
            .integer("snowAccumulationHeight")
            .context("snow gamerule missing")?
            > 0
        {
            ensure!(
                rain.precipitation_at(top) == Precipitation::Rain,
                "snow precipitation requires original snow accumulation callbacks"
            );
        }
        // Source asks precipitation at the below point after the snow branch.
        ensure!(
            rain.precipitation_at(below) == Precipitation::Rain,
            "non-rain native precipitation callback is unavailable"
        );
        let state = world.block(below.0, below.1, below.2)?;
        ensure!(
            random_block_ticks::precipitation_handler_class(state)? == "dfy",
            "nonempty block handlePrecipitation callback requires its original executor"
        );
        // Original Block.handlePrecipitation's body is empty, established by
        // the declaring method metadata. No unsupported callback is swallowed.
        Ok(())
    }
    fn random_fluid_tick(
        &mut self,
        world: &mut WorldBackend,
        tick: SampledFluidTick,
        random: &mut LegacyRandomSource,
        _rand_value: &mut i32,
    ) -> Result<()> {
        ensure!(
            tick.fluid.kind == FluidKind::Lava && tick.declaring_class == "eph",
            "sampled random fluid callback is not the ported LavaFluid"
        );
        let chunk = (
            tick.position.0.div_euclid(16),
            tick.position.2.div_euclid(16),
        );
        self.block_runtime.prepare_halo(world,self.fire.fluid.chunks,chunk,self.game_time)?;
        self.block_runtime.mark_used();
        if let Some(source)=self.fire.fluid.entity_source.as_mut(){source.observe_block_scheduler_use();}
        let facts = self.fire_runtime.facts(self.world_context)?;
        let mut context = FireTickContext {
            world,
            random,
            sub_tick_order: self.sub_tick_order,
            host: &mut self.fire,
            facts: &facts,
            game_time: self.game_time,
        };
        mc121_rust::fire_simulation::random_tick_lava_global(&mut self.block_runtime.queue,
            self.fluids,
            &mut context,
            self.fluid_facts,
            tick.position,
        )?;
        Ok(())
    }
}
fn source_chunk_eligible(
    chunks: &MainChunkRuntime,
    players: &[PlayerState],
    chunk: (i32, i32),
) -> Result<bool> {
    let Some(holder) = chunks.manager.holder(chunk) else {
        return Ok(false);
    };
    let snapshot = chunks
        .manager
        .holder_snapshot(holder.id)
        .context("random tick entity-visibility snapshot unavailable")?;
    if snapshot.output_status != Some(FullStatus::EntityTicking) {
        return Ok(false);
    }
    let center = (
        f64::from(chunk.0.wrapping_shl(4).wrapping_add(8)),
        f64::from(chunk.1.wrapping_shl(4).wrapping_add(8)),
    );
    if !players.iter().any(|p| {
        if p.gamemode == 3 {
            return false;
        }
        let dx = center.0 - p.position.x;
        let dz = center.1 - p.position.z;
        dx * dx + dz * dz < 16384.0
    }) {
        return Ok(false);
    }
    main_fluid_simulation::current_block_ready(chunks, chunk)
}
impl LivePlayers {
    pub(super) fn random_block_frame(
        &mut self,
        world: &mut WorldBackend,
        game_time: i64,
        day_time: i64,
        border: CollisionBorder,
        events: &broadcast::Sender<PlayerEvent>,
    ) -> Result<MainRandomTickReport> {
        if !self.random_block_ticks.native_source {
            return Ok(MainRandomTickReport {
                source_unavailable: Some(
                    "random tick counter source was unavailable at construction",
                ),
                ..MainRandomTickReport::default()
            });
        }
        self.random_block_ticks.require_healthy()?;
        self.gameplay.require_supported_floor()?;
        self.world_context.dimension.require_runtime_layout()?;
        ensure!(
            self.world_context.dimension.has_skylight
                && !self.world_context.dimension.has_ceiling
                && !self.world_context.dimension.ultrawarm,
            "native random ticks require the admitted overworld dimension"
        );
        let WorldBackend::Fixture(flat) = &*world else {
            bail!("random ticks cannot invent imported section counters");
        };
        self.chunk_runtime
            .native_lighting_mut()?
            .require_current(flat)?;
        // This is the actual retained native holder-id order supplied by its
        // owner. It is explicitly not a claim about Java's visible-map layout.
        let mut order = self
            .chunk_runtime
            .manager
            .native_resident_ticking_chunk_order()?;
        let snapshots = self.registry.snapshot();
        let fluid_facts = main_fluid_simulation::facts(&self.world_context)?;
        let sky_darken =
            random_block_ticks::sky_darken(day_time, None, &self.world_context.weather());
        let active_items = self.item_runtime.item_count();
        let Self {
            item_runtime,
            chunk_runtime,
            gameplay,
            world_interactions,
            entity_source,
            world_revision,
            fluid_runtime,
            fire_runtime,
            block_runtime,
            rain_context,
            world_context,
            random_block_ticks,
            level_random_position_value,
            ..
        } = self;
        let random = item_runtime.world_random_mut();
        let rand_value = level_random_position_value
            .as_mut()
            .context("actual Level.randValue owner unavailable")?;
        random_block_ticks::shuffle_tick_chunks(&mut order, random)?;
        let mut host = MainRandomHost {
            fire: MainFireHost {
                fluid: main_fluid_simulation::MainFluidHost {
                    chunks: chunk_runtime,
                    gameplay,
                    interactions: world_interactions,
                    entity_source,
                    revision: world_revision,
                    events,
                    players: snapshots.clone(),
                    active_items,
                    border,
                    random_ticks: Some(random_block_ticks),
                },
                rain: rain_context.as_mut(),
                raining: world_context.weather().is_raining(),
            },
            fire_runtime,
            block_runtime,
            fluids: &mut fluid_runtime.simulation,
            sub_tick_order: &mut fluid_runtime.sub_tick_order,
            world_context,
            fluid_facts,
            sky_darken,
            game_time,
        };
        let mut report = MainRandomTickReport {
            resident_chunks: order.len(),
            ..MainRandomTickReport::default()
        };
        for chunk in order {
            if !source_chunk_eligible(host.fire.fluid.chunks, &snapshots, chunk)? {
                continue;
            }
            let mut context = RandomBlockTickContext {
                world,
                random,
                rand_value,
                host: &mut host,
            };
            match random_block_ticks::tick_chunk(
                &mut context,
                chunk,
                world_context.rules.random_tick_speed(),
            ) {
                Ok(chunk_report) => {
                    report.executed_chunks += 1;
                    report.reports.push((chunk, chunk_report));
                }
                Err(error) => {
                    host.fire
                        .fluid
                        .random_ticks
                        .as_deref_mut()
                        .context("random section owner missing")?
                        .stopped = Some(error.to_string());
                    return Err(error);
                }
            }
        }
        Ok(report)
    }
}
impl Server {
    /// Root places this chunk/random phase after scheduled block/fluid ticks and
    /// before level entity ticks. Later drain retains every synchronous effect.
    pub(super) async fn tick_random_block_world(&self) -> Result<MainRandomTickReport> {
        let clock = *self
            .clock
            .read()
            .map_err(|_| anyhow!("world clock lock poisoned"))?;
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
        // aqu.updateSkyBrightness precedes aqu.tickTime. Root's clock has
        // already advanced in this tick, so recover that exact earlier input.
        let sky_update_day_time = if clock.daylight_cycle {
            clock.day_time.wrapping_sub(1)
        } else {
            clock.day_time
        };
        players.random_block_frame(
            &mut world,
            clock.game_time,
            sky_update_day_time,
            border,
            &self.entity_events,
        )
    }
}
