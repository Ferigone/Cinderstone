//! Main's native scheduled fire owner, sharing fluid world writes, Level.random
//! and the level sub-tick insertion counter. Root registers phase entry points.
use super::main_fluid_simulation::{self, MainFluidHost};
use super::*;
use mc121_rust::{
    fire_simulation::{
        self, FireSimulation, FireSimulationHost, FireTickContext, FireTickReport, FireWorldFacts,
        ScheduledFire,
    },
    fluid_physics::FluidState,
    fluid_simulation::{FluidSimulationHost, Position as BlockPosition},
    native_rain::NativeFlatPlainsRainFrame,
    rain_physics::FreshFlatPlainsRain,
    tick::SavedTick,
};

#[derive(Default)]
pub(super) struct MainFireRuntime {
    pub(super) engine: FireSimulation,
    loaded: BTreeSet<(i32, i32)>,
    saved: BTreeMap<(i32, i32), Vec<SavedTick<ScheduledFire>>>,
    difficulty_id: Option<i32>,
    stopped: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct SavedFireRuntime {
    pub shared_sub_tick_order: i64,
    difficulty_id: Option<i32>,
    chunks: Vec<((i32, i32), Vec<SavedTick<ScheduledFire>>)>,
    stopped: Option<String>,
}
impl MainFireRuntime {
    /// The host supplies its actual difficulty setting. Default deliberately
    /// remains unconfigured, including for native saves predating this owner.
    pub fn new(difficulty_id: i32) -> Result<Self> {
        ensure!(
            (0..=3).contains(&difficulty_id),
            "invalid native fire difficulty"
        );
        Ok(Self {
            difficulty_id: Some(difficulty_id),
            ..Self::default()
        })
    }
    pub fn require_healthy(&self) -> Result<()> {
        ensure!(
            self.difficulty_id.is_some(),
            "fire difficulty owner is not configured"
        );
        ensure!(
            self.stopped.is_none(),
            "fire runtime stopped: {}",
            self.stopped.as_deref().unwrap_or("")
        );
        Ok(())
    }
    pub fn set_difficulty(&mut self, actual: i32) -> Result<()> {
        ensure!((0..=3).contains(&actual), "invalid actual fire difficulty");
        self.difficulty_id = Some(actual);
        Ok(())
    }
    pub(super) fn take_legacy_ticks(&mut self,now:i64)->Vec<((i32,i32),Vec<SavedTick<ScheduledFire>>)> {
        let saved=self.checkpoint(now,0).chunks;
        self.engine=FireSimulation::new();self.loaded.clear();self.saved.clear();saved
    }
    pub fn pending_count(&self) -> usize {
        self.engine.pending_count() + self.saved.values().map(Vec::len).sum::<usize>()
    }
    pub fn checkpoint(&self, now: i64, shared_subtick: i64) -> SavedFireRuntime {
        let mut chunks = self.saved.clone();
        for chunk in &self.loaded {
            chunks.insert(
                *chunk,
                self.engine
                    .saved_chunk_ticks(*chunk, now)
                    .expect("loaded fire queue exists"),
            );
        }
        SavedFireRuntime {
            shared_sub_tick_order: shared_subtick,
            difficulty_id: self.difficulty_id,
            chunks: chunks.into_iter().collect(),
            stopped: self.stopped.clone(),
        }
    }
    pub fn from_checkpoint(saved: SavedFireRuntime) -> Result<Self> {
        ensure!(
            saved.difficulty_id.is_none_or(|id| (0..=3).contains(&id)),
            "invalid persisted fire difficulty"
        );
        let mut runtime = Self {
            difficulty_id: saved.difficulty_id,
            stopped: saved.stopped,
            ..Self::default()
        };
        for (chunk, ticks) in saved.chunks {
            ensure!(
                ticks.iter().all(|t| t.pos.chunk() == chunk),
                "persisted fire tick outside owning chunk"
            );
            ensure!(
                runtime.saved.insert(chunk, ticks).is_none(),
                "duplicate persisted fire tick chunk"
            );
        }
        Ok(runtime)
    }
    pub(super) fn sync_chunks(&mut self, chunks: &MainChunkRuntime, now: i64) -> Result<()> {
        for chunk in self.loaded.iter().copied().collect::<Vec<_>>() {
            if !chunks.manager.current_full_ready(chunk) {
                let ticks = self
                    .engine
                    .unload_chunk(chunk, now)
                    .context("fire queue disappeared on unload")?;
                self.saved.insert(chunk, ticks);
                self.loaded.remove(&chunk);
            }
        }
        for chunk in self.saved.keys().copied().collect::<Vec<_>>() {
            if chunks.manager.current_full_ready(chunk) {
                let ticks = self.saved.remove(&chunk).unwrap();
                self.engine.load_chunk(chunk, ticks, now)?;
                self.loaded.insert(chunk);
            }
        }
        Ok(())
    }
    /// Fire can search a 21-block portal frame in either horizontal direction.
    /// Retain work until that actual FULL region is available; do not infer a
    /// loaded chunk from a sparse FlatWorld lookup.
    pub(super) fn prepare_halo(
        &mut self,
        world: &WorldBackend,
        chunks: &MainChunkRuntime,
        chunk: (i32, i32),
        now: i64,
    ) -> Result<()> {
        self.require_healthy()?;
        ensure!(
            matches!(world, WorldBackend::Fixture(_)),
            "imported fire tick NBT needs its own source owner"
        );
        let min = (
            chunk.0.checked_sub(2).context("fire halo overflow")?,
            chunk.1.checked_sub(2).context("fire halo overflow")?,
        );
        let max = (
            chunk.0.checked_add(2).context("fire halo overflow")?,
            chunk.1.checked_add(2).context("fire halo overflow")?,
        );
        chunks.manager.certify_full_region(min, max)?;
        for x in min.0..=max.0 {
            for z in min.1..=max.1 {
                let at = (x, z);
                if self.loaded.contains(&at) {
                    continue;
                }
                let ticks = self.saved.remove(&at).unwrap_or_default();
                self.engine.load_chunk(at, ticks, now)?;
                self.loaded.insert(at);
            }
        }
        Ok(())
    }
    pub fn supports_scheduled_state(state: u32) -> bool {
        fire_simulation::fire_kind(state).is_ok_and(|kind| kind == 1)
    }
    pub fn schedule_block_tick(
        &mut self,
        world: &WorldBackend,
        chunks: &MainChunkRuntime,
        position: BlockPosition,
        state: u32,
        delay: i32,
        now: i64,
        shared_subtick: &mut i64,
    ) -> Result<()> {
        ensure!(
            fire_simulation::fire_kind(state)? == 1,
            "scheduled block state has no fire executor"
        );
        ensure!(
            (30..=39).contains(&delay),
            "requested fire delay is outside FireBlock's original range"
        );
        self.prepare_halo(
            world,
            chunks,
            (position.0.div_euclid(16), position.2.div_euclid(16)),
            now,
        )?;
        self.engine.schedule(position, now, delay, shared_subtick)?;
        Ok(())
    }
    pub(super) fn facts(&self, context: &WorldContext) -> Result<FireWorldFacts> {
        self.require_healthy()?;
        context.dimension.require_runtime_layout()?;
        // Main's fixture admission verifies the actual minecraft:overworld
        // world key and pinned default dimension type, separately from storage.
        ensure!(
            context.dimension.has_skylight && !context.dimension.ultrawarm,
            "native fire requires the admitted overworld dimension"
        );
        Ok(FireWorldFacts {
            do_fire_tick: context
                .rules
                .boolean("doFireTick")
                .context("fire gamerule missing")?,
            is_raining: context.weather().is_raining(),
            difficulty_id: self.difficulty_id.context("fire difficulty unavailable")?,
            portal_dimension: true,
            minimum_build_height: context.dimension.min_y,
            maximum_build_height: context
                .dimension
                .min_y
                .checked_add(context.dimension.height)
                .context("fire dimension height overflow")?,
            infiniburn_tag: fire_simulation::vanilla_overworld_infiniburn_tag().to_owned(),
        })
    }
}

pub(super) struct MainFireHost<'a> {
    pub(super) fluid: MainFluidHost<'a>,
    pub(super) rain: Option<&'a mut FreshFlatPlainsRain>,
    pub(super) raining: bool,
}
impl FluidSimulationHost for MainFireHost<'_> {
    fn set_block(
        &mut self,
        w: &mut WorldBackend,
        p: BlockPosition,
        s: u32,
        f: u32,
    ) -> Result<Vec<WorldUpdate>> {
        let old = w.block(p.0, p.1, p.2)?;
        ensure!(!fire_simulation::has_block_entity(old)? && !fire_simulation::has_block_entity(s)?,
            "native fire write requires the actual block-entity removal/creation lifecycle at {p:?}");
        let mut changes = self
            .fluid
            .set_block_with_primary_callback(w, p, s, f, true)?;
        // FireBlock's age update uses flag4, without UPDATE_CLIENTS (bit2).
        // Keep actual source/counter/light observations from the shared commit,
        // but suppress only that primary block packet. Secondary shape writes
        // retain their own client-update flags and ordered publication.
        if f & 2 == 0 {
            if let Some(index) = changes.iter().position(
                |change| matches!(change,WorldUpdate::Block(b) if (b.x,b.y,b.z)==p&&b.state==s),
            ) {
                changes.remove(index);
            }
        }
        Ok(changes)
    }
    fn before_destroying_block(
        &mut self,
        w: &mut WorldBackend,
        p: BlockPosition,
        s: u32,
        r: &mut LegacyRandomSource,
    ) -> Result<()> {
        self.fluid.before_destroying_block(w, p, s, r)
    }
    fn level_event(&mut self, e: i32, p: BlockPosition, d: i32) -> Result<()> {
        self.fluid.level_event(e, p, d)
    }
    fn place_custom_liquid(
        &mut self,
        w: &mut WorldBackend,
        p: BlockPosition,
        s: u32,
        f: FluidState,
    ) -> Result<Vec<WorldUpdate>> {
        self.fluid.place_custom_liquid(w, p, s, f)
    }
    fn block_changes(&mut self, c: &[WorldUpdate]) -> Result<()> {
        self.fluid.block_changes(c)
    }
}
impl FireSimulationHost for MainFireHost<'_> {
    fn is_raining_at(&mut self, world: &mut WorldBackend, position: BlockPosition) -> Result<bool> {
        // Level.isRainingAt's first query is the actual global weather flag.
        if !self.raining {
            return Ok(false);
        }
        let rain = self
            .rain
            .as_deref_mut()
            .context("fire rain exposure owner unavailable")?;
        let (lighting, manager) = self.fluid.chunks.native_lighting_parts()?;
        NativeFlatPlainsRainFrame::new(rain, lighting, manager).is_raining_at(world, position)
    }
    fn increased_fire_burnout(
        &mut self,
        world: &mut WorldBackend,
        position: BlockPosition,
    ) -> Result<bool> {
        let WorldBackend::Fixture(flat) = world else {
            bail!("fire biome owner is unavailable for imported sources");
        };
        ensure!(
            flat.biome_id() == 39,
            "fire biome is not actual canonical plains"
        );
        let chunk = (position.0.div_euclid(16), position.2.div_euclid(16));
        self.fluid
            .chunks
            .manager
            .certify_full_region(chunk, chunk)?;
        Ok(fire_simulation::vanilla_biome_increased_fire_burnout(
            "minecraft:plains",
        ))
    }
    fn is_loaded(&mut self, _world: &mut WorldBackend, position: BlockPosition) -> Result<bool> {
        Ok((world::MIN_Y..=world::MAX_Y).contains(&position.1)
            && self
                .fluid
                .chunks
                .manager
                .current_full_ready((position.0.div_euclid(16), position.2.div_euclid(16))))
    }
    fn has_chunk_at(&mut self, _world: &mut WorldBackend, position: BlockPosition) -> Result<bool> {
        Ok(self
            .fluid
            .chunks
            .manager
            .current_full_ready((position.0.div_euclid(16), position.2.div_euclid(16))))
    }
    fn prime_tnt(
        &mut self,
        _world: &mut WorldBackend,
        position: BlockPosition,
        _random: &mut LegacyRandomSource,
    ) -> Result<()> {
        bail!("burned TNT at {position:?} requires the actual PrimedTnt entity/sound/game-event executor")
    }
}

impl LivePlayers {
    fn fire_frame(
        &mut self,
        world: &mut WorldBackend,
        game_time: i64,
        border: CollisionBorder,
        events: &broadcast::Sender<PlayerEvent>,
        change: Option<(BlockPosition, u32, u32)>,
        scheduled: Option<BlockPosition>,
    ) -> Result<FireTickReport> {
        if change.is_none() && scheduled.is_none() {
            return Ok(FireTickReport::default());
        }
        if let Some((_, old, new)) = change {
            let old_kind = fire_simulation::fire_kind(old)?;
            let new_kind = fire_simulation::fire_kind(new)?;
            if new_kind == 0 || old_kind == new_kind {
                return Ok(FireTickReport::default());
            }
        }
        self.gameplay.require_supported_floor()?;
        let facts = self.fire_runtime.facts(&self.world_context)?;
        let snapshots = self.registry.snapshot();
        let active_items = self.item_runtime.item_count();
        let Self {
            fire_runtime,
            block_runtime,
            fluid_runtime,
            chunk_runtime,
            item_runtime,
            gameplay,
            world_interactions,
            entity_source,
            world_revision,
            rain_context,
            random_block_ticks,
            ..
        } = self;
        fire_runtime.require_healthy()?;
        block_runtime.mark_used();
        if let Some(source)=entity_source.as_mut(){source.observe_block_scheduler_use();}
        fluid_runtime.require_healthy()?;
        block_runtime.sync_saved(chunk_runtime, game_time)?;
        if let Some((at, _, _)) = change {
            block_runtime.prepare_halo(
                world,
                chunk_runtime,
                (at.0.div_euclid(16), at.2.div_euclid(16)),
                game_time,
            )?;
        }
        let fluid = MainFluidHost {
            chunks: chunk_runtime,
            gameplay,
            interactions: world_interactions,
            entity_source,
            revision: world_revision,
            events,
            players: snapshots,
            active_items,
            border,
            random_ticks: Some(random_block_ticks),
        };
        let mut host = MainFireHost {
            fluid,
            rain: rain_context.as_mut(),
            raining: facts.is_raining,
        };
        let mut ctx = FireTickContext {
            world,
            random: item_runtime.world_random_mut(),
            sub_tick_order: &mut fluid_runtime.sub_tick_order,
            host: &mut host,
            facts: &facts,
            game_time,
        };
        let result = if let Some((at, old, new)) = change {
            fire_simulation::on_block_changed_global(&mut block_runtime.queue,&mut ctx, at, old, new)
                .and_then(|changes| {
                    let count = changes
                        .iter()
                        .filter(|c| matches!(c, WorldUpdate::Block(_)))
                        .count();
                    ctx.host.block_changes(&changes)?;
                    Ok(FireTickReport {
                        changed_blocks: count,
                        ..FireTickReport::default()
                    })
                })
        } else {
            fire_simulation::tick_one_global(&mut block_runtime.queue,&mut ctx,scheduled.context("scheduled fire position missing")?)
        };
        if let Err(error) = &result {
            fire_runtime.stopped = Some(format!("{error:#}"));
        }
        result
    }
    pub(super) fn tick_fire_block(&mut self,world:&mut WorldBackend,p:BlockPosition,now:i64,border:CollisionBorder,events:&broadcast::Sender<PlayerEvent>)->Result<FireTickReport>{self.fire_frame(world,now,border,events,None,Some(p))}
    pub(super) fn observe_fire_change(
        &mut self,
        world: &mut WorldBackend,
        position: BlockPosition,
        old: u32,
        new: u32,
        now: i64,
        border: CollisionBorder,
        events: &broadcast::Sender<PlayerEvent>,
    ) -> Result<()> {
        self.fire_frame(world, now, border, events, Some((position, old, new)),None)
            .map(|_| ())
    }
}
impl Server {
    pub(super) async fn tick_fire_world(&self)->Result<FireTickReport>{self.tick_block_world().await.map(|r|r.fire)}
}
