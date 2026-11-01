//! Production composition for native scheduled fluids. Imported fluid-tick NBT,
//! block loot and custom liquid containers require their real host paths.
use super::*;
use mc121_rust::{
    chunk_lifecycle::FutureState,
    fluid_physics::FluidState,
    fluid_simulation::{
        FluidSimulation, FluidSimulationHost, FluidTickContext, FluidTickReport, FluidWorldFacts,
        ScheduledFluid,
    },
    tick::SavedTick,
    world_interactions::{self, InteractionEvent, RuntimeInteractionWorld},
};

#[derive(Default)]
pub(super) struct MainFluidRuntime {
    pub(super) simulation: FluidSimulation,
    loaded: BTreeSet<(i32, i32)>,
    saved: BTreeMap<(i32, i32), Vec<SavedTick<ScheduledFluid>>>,
    /// This is the level counter; block-tick insertion must use the same owner.
    pub sub_tick_order: i64,
    stopped: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct MainFluidCheckpoint {
    sub_tick_order: i64,
    world_random_state48: u64,
    chunks: Vec<((i32, i32), Vec<SavedTick<ScheduledFluid>>)>,
    stopped: Option<String>,
}
impl MainFluidCheckpoint {
    pub fn world_random(&self) -> Result<LegacyRandomSource> {
        ensure!(
            self.world_random_state48 < (1_u64 << 48),
            "invalid persisted fluid Level.random state"
        );
        // LegacyRandomSource::new xors its argument with the LCG multiplier.
        Ok(LegacyRandomSource::new(
            (self.world_random_state48 ^ 25_214_903_917) as i64,
        ))
    }
}
impl MainFluidRuntime {
    pub(super) fn require_healthy(&self) -> Result<()> {
        self.healthy()
    }
    /// Capture while holding the same level lock as block/world persistence.
    /// Save delay is relative to `now`, exactly as LevelChunkTicks.save writes it.
    pub fn checkpoint(&self, now: i64, world_random_state48: u64) -> MainFluidCheckpoint {
        let mut chunks = self.saved.clone();
        for chunk in &self.loaded {
            chunks.insert(
                *chunk,
                self.simulation
                    .saved_chunk_ticks(*chunk, now)
                    .expect("loaded fluid queue exists"),
            );
        }
        MainFluidCheckpoint {
            sub_tick_order: self.sub_tick_order,
            world_random_state48,
            chunks: chunks.into_iter().collect(),
            stopped: self.stopped.clone(),
        }
    }
    /// Retain delays until the actual current FULL future loads each queue.
    pub fn from_checkpoint(saved: MainFluidCheckpoint) -> Result<Self> {
        saved.world_random()?;
        let mut result = Self {
            sub_tick_order: saved.sub_tick_order,
            stopped: saved.stopped,
            ..Self::default()
        };
        for (chunk, ticks) in saved.chunks {
            ensure!(
                ticks.iter().all(|tick| tick.pos.chunk() == chunk),
                "persisted fluid tick is outside its owning chunk"
            );
            ensure!(
                result.saved.insert(chunk, ticks).is_none(),
                "duplicate persisted fluid tick chunk"
            );
        }
        Ok(result)
    }
    pub fn pending_count(&self) -> usize {
        self.simulation.pending_count() + self.saved.values().map(Vec::len).sum::<usize>()
    }
    fn healthy(&self) -> Result<()> {
        ensure!(
            self.stopped.is_none(),
            "fluid runtime stopped: {}",
            self.stopped.as_deref().unwrap_or("")
        );
        Ok(())
    }
    pub(super) fn sync_chunks(
        &mut self,
        manager: &mc121_rust::runtime_chunk_manager::RuntimeChunkManager,
        now: i64,
    ) -> Result<()> {
        for chunk in self.loaded.iter().copied().collect::<Vec<_>>() {
            if !manager.current_full_ready(chunk) {
                let ticks = self
                    .simulation
                    .unload_chunk(chunk, now)
                    .context("fluid queue disappeared on unload")?;
                self.saved.insert(chunk, ticks);
                self.loaded.remove(&chunk);
            }
        }
        for chunk in self.saved.keys().copied().collect::<Vec<_>>() {
            if manager.current_full_ready(chunk) {
                let ticks = self.saved.remove(&chunk).unwrap();
                self.simulation.load_chunk(chunk, ticks, now)?;
                self.loaded.insert(chunk);
            }
        }
        Ok(())
    }
    /// Only native flat chunks have a known empty initial scheduled-tick list.
    /// A FULL halo is needed by wall/slope queries before any fluid mutation.
    pub(super) fn prepare_halo(
        &mut self,
        world: &WorldBackend,
        chunks: &MainChunkRuntime,
        chunk: (i32, i32),
        now: i64,
    ) -> Result<()> {
        self.healthy()?;
        ensure!(
            matches!(world, WorldBackend::Fixture(_)),
            "imported fluid-tick NBT is not connected to the native scheduler"
        );
        let min = (
            chunk.0.checked_sub(1).context("fluid halo overflow")?,
            chunk.1.checked_sub(1).context("fluid halo overflow")?,
        );
        let max = (
            chunk.0.checked_add(1).context("fluid halo overflow")?,
            chunk.1.checked_add(1).context("fluid halo overflow")?,
        );
        chunks.manager.certify_full_region(min, max)?;
        for x in min.0..=max.0 {
            for z in min.1..=max.1 {
                let at = (x, z);
                if self.loaded.contains(&at) {
                    continue;
                }
                let saved = self.saved.remove(&at).unwrap_or_default();
                self.simulation.load_chunk(at, saved, now)?;
                self.loaded.insert(at);
            }
        }
        Ok(())
    }
}

pub(super) fn current_block_ready(chunks: &MainChunkRuntime, chunk: (i32, i32)) -> Result<bool> {
    if !chunks.manager.ticking_range().in_block_ticking_range(chunk) {
        return Ok(false);
    }
    let Some(holder) = chunks.manager.holder(chunk) else {
        return Ok(false);
    };
    let snapshot = chunks
        .manager
        .holder_snapshot(holder.id)
        .context("reentrant fluid block-future query")?;
    let future = snapshot.slots[1]; // Stage::Block, never published FullStatus.
    match snapshot
        .futures
        .iter()
        .find(|(id, _)| *id == future)
        .map(|(_, state)| *state)
    {
        Some(FutureState::Success) => Ok(true),
        Some(FutureState::Exceptional | FutureState::Cancelled) => {
            bail!("current fluid BLOCK future failed at {chunk:?}")
        }
        _ => Ok(false),
    }
}

pub(super) struct MainFluidHost<'a> {
    pub(super) chunks: &'a mut MainChunkRuntime,
    pub(super) gameplay: &'a mut MainItemGameplay,
    pub(super) interactions: &'a mut main_world_interactions::WorldInteractionRuntime,
    pub(super) entity_source: &'a mut Option<NativeEntitySource>,
    pub(super) revision: &'a mut u64,
    pub(super) events: &'a broadcast::Sender<PlayerEvent>,
    pub(super) players: Vec<PlayerState>,
    pub(super) active_items: usize,
    pub(super) border: CollisionBorder,
    pub(super) random_ticks: Option<&'a mut crate::main_random_block_ticks::MainRandomBlockTicks>,
}
#[derive(Clone, Copy)]
enum PrimaryCallbacks {
    FluidCore,
    Full,
    FireCore,
}
impl MainFluidHost<'_> {
    /// Generic block callbacks retain the primary write. FireSimulation passes
    /// true because it already executes fire.onPlace inline; only that primary
    /// write becomes a typed fluid-only callback. Shape writes retain full work.
    pub(super) fn set_block_with_primary_callback(
        &mut self,
        world: &mut WorldBackend,
        at: (i32, i32, i32),
        state: u32,
        flags: u32,
        primary_fire_handled: bool,
    ) -> Result<Vec<WorldUpdate>> {
        self.commit_block(
            world,
            at,
            state,
            flags,
            if primary_fire_handled {
                PrimaryCallbacks::FireCore
            } else {
                PrimaryCallbacks::Full
            },
        )
    }
    fn commit_block(
        &mut self,
        world: &mut WorldBackend,
        at: (i32, i32, i32),
        state: u32,
        flags: u32,
        primary: PrimaryCallbacks,
    ) -> Result<Vec<WorldUpdate>> {
        let mut query = RuntimeInteractionWorld::new(
            world,
            world::MIN_Y,
            world::MAX_Y + 1,
            self.border,
            |_, _| bail!("fluid state edits do not place entities"),
        )?;
        let plan = world_interactions::plan_edit(at, state, u8::try_from(flags)?, &mut query)?;
        ensure!(
            !plan.has_required_hooks(),
            "fluid edit reaches an unsupported block callback"
        );
        ensure!(
            !plan.events.iter().any(|e| matches!(
                e,
                InteractionEvent::DropResources { .. } | InteractionEvent::CreateBlockEntity(_)
            )),
            "fluid edit requires block loot or block-entity lifecycle"
        );
        let source_edit = (|| -> Result<_> {
            self.gameplay.require_supported_floor()?;
            let WorldBackend::Fixture(flat) = &*world else {
                bail!("native fluid source requires fixture");
            };
            self.chunks.native_lighting_mut()?.require_current(flat)?;
            self.entity_source
                .as_ref()
                .context("native fluid entity source unavailable")?
                .require_fluid_history(world, &self.interactions.fluid_shapes)?;
            crate::main_native_source::callbacks_can_complete(world, &plan)?;
            let token = self
                .entity_source
                .as_ref()
                .context("native fluid entity source unavailable")?
                .prepare_owned_edit(world, &plan)?;
            token.require_fluid_history(&self.interactions.fluid_shapes)?;
            Ok(token)
        })();
        let source_edit = match source_edit {
            Ok(token) => Some(token),
            Err(error) if self.active_items != 0 => return Err(error),
            Err(_) => None,
        };
        self.interactions.can_append(plan.events.len())?;
        if let Some(ticks) = &mut self.random_ticks {
            ticks.prepare_plan(world, &plan)?;
        }
        let mut changes = main_world_interactions::commit_fixture_plan(world, &plan)?
            .context("native fluid atomic mutation requires the fixture backend")?;
        if let Some(ticks) = &mut self.random_ticks {
            ticks.observe_plan(&plan)?;
        }
        if !changes.is_empty() {
            if let Some(source) = self.entity_source {
                if let Some(token) = source_edit {
                    source.observe_owned_commit(token, world)?;
                } else {
                    source.invalidate("scheduled fluid mutation has no complete native callback/cache transaction");
                }
            }
            for change in &changes {
                if let WorldUpdate::Block(b) = change {
                    self.gameplay
                        .observe_block_change((b.x, b.y, b.z), b.state)?;
                    self.chunks.invalidate_block_lighting((b.x, b.y, b.z))?;
                }
            }
        }
        changes.extend(self.chunks.settle_lighting(world)?);
        let mut primary_seen = false;
        self.interactions.append(
            -1,
            plan.events.into_iter().filter_map(|event| match event {
                InteractionEvent::Write {
                    position,
                    old_state,
                    state,
                    flags,
                } if position == at && !primary_seen => {
                    primary_seen = true;
                    match primary {
                        PrimaryCallbacks::FluidCore => None,
                        PrimaryCallbacks::Full => Some(InteractionEvent::Write {
                            position,
                            old_state,
                            state,
                            flags,
                        }),
                        PrimaryCallbacks::FireCore => {
                            Some(InteractionEvent::FluidOnlyBlockChanged {
                                position,
                                old_state,
                                state,
                                flags,
                            })
                        }
                    }
                }
                event => Some(event),
            }),
        );
        Ok(changes)
    }
}
impl FluidSimulationHost for MainFluidHost<'_> {
    fn set_block(
        &mut self,
        world: &mut WorldBackend,
        at: (i32, i32, i32),
        state: u32,
        flags: u32,
    ) -> Result<Vec<WorldUpdate>> {
        self.commit_block(world, at, state, flags, PrimaryCallbacks::FluidCore)
    }
    fn before_destroying_block(
        &mut self,
        _: &mut WorldBackend,
        at: (i32, i32, i32),
        old: u32,
        _: &mut LegacyRandomSource,
    ) -> Result<()> {
        bail!("water destruction at {at:?}, state {old}, requires the actual block loot context")
    }
    fn level_event(&mut self, event: i32, at: (i32, i32, i32), data: i32) -> Result<()> {
        let recipients = self
            .players
            .iter()
            .filter(|p| {
                let dx = p.position.x - f64::from(at.0);
                let dy = p.position.y - f64::from(at.1);
                let dz = p.position.z - f64::from(at.2);
                dx * dx + dy * dy + dz * dz < 64. * 64.
            })
            .map(|p| p.entity_id)
            .collect::<Vec<_>>();
        if !recipients.is_empty() {
            let _ = self.events.send(PlayerEvent::WorldAudience {
                recipients: Arc::new(recipients),
                packets: Arc::new(vec![mc121_rust::item_effects::level_event_packet(
                    event, at, data, false,
                )]),
            });
        }
        Ok(())
    }
    fn place_custom_liquid(
        &mut self,
        _: &mut WorldBackend,
        at: (i32, i32, i32),
        state: u32,
        _: FluidState,
    ) -> Result<Vec<WorldUpdate>> {
        bail!("custom LiquidBlockContainer callback at {at:?}, state {state}, is not yet connected")
    }
    fn block_changes(&mut self, changes: &[WorldUpdate]) -> Result<()> {
        let mut packets = Vec::with_capacity(changes.len());
        for change in changes {
            match change {
                WorldUpdate::Block(b) => packets.push(Packet {
                    id: 0x09,
                    payload: encoded(|w| {
                        w.i64(pack_position(b.x, b.y, b.z)).varint(b.state as i32);
                    }),
                }),
                WorldUpdate::Light { payload, .. } => packets.push(Packet {
                    id: 0x2a,
                    payload: payload.to_vec(),
                }),
                WorldUpdate::Acknowledge { .. } => {
                    bail!("fluid mutation unexpectedly returned an acknowledgement")
                }
            }
        }
        if !packets.is_empty() {
            *self.revision = self
                .revision
                .checked_add(1)
                .context("world revision exhausted")?;
            let _ = self.events.send(PlayerEvent::World {
                revision: *self.revision,
                packets: Arc::new(packets),
            });
        }
        Ok(())
    }
}

pub(super) fn facts(context: &WorldContext) -> Result<FluidWorldFacts> {
    Ok(FluidWorldFacts {
        ultrawarm: context.dimension.ultrawarm,
        water_source_conversion: context
            .rules
            .boolean("waterSourceConversion")
            .context("water source rule missing")?,
        lava_source_conversion: context
            .rules
            .boolean("lavaSourceConversion")
            .context("lava source rule missing")?,
    })
}

impl LivePlayers {
    fn fluid_frame(
        &mut self,
        world: &mut WorldBackend,
        game_time: i64,
        border: CollisionBorder,
        events: &broadcast::Sender<PlayerEvent>,
        change: Option<((i32, i32, i32), u32, u32, u8)>,
    ) -> Result<FluidTickReport> {
        let world_facts = facts(&self.world_context)?;
        let players = self.registry.snapshot();
        let active_items = self.item_runtime.item_count();
        let Self {
            fluid_runtime: runtime,
            chunk_runtime,
            item_runtime,
            gameplay,
            world_interactions,
            entity_source,
            world_revision,
            random_block_ticks,
            ..
        } = self;
        runtime.healthy()?;
        runtime.sync_chunks(&chunk_runtime.manager, game_time)?;
        if let Some((p, _, _, _)) = change {
            runtime.prepare_halo(
                world,
                chunk_runtime,
                (p.0.div_euclid(16), p.2.div_euclid(16)),
                game_time,
            )?;
        }
        let mut tickable = BTreeSet::new();
        if change.is_none() {
            for chunk in runtime.loaded.iter().copied().collect::<Vec<_>>() {
                if runtime.simulation.chunk_has_ticks(chunk)
                    && current_block_ready(chunk_runtime, chunk)?
                {
                    runtime.prepare_halo(world, chunk_runtime, chunk, game_time)?;
                    tickable.insert(chunk);
                }
            }
        }
        let mut host = MainFluidHost {
            chunks: chunk_runtime,
            gameplay,
            interactions: world_interactions,
            entity_source,
            revision: world_revision,
            events,
            players,
            active_items,
            border,
            random_ticks: Some(random_block_ticks),
        };
        let mut context = FluidTickContext {
            world,
            random: item_runtime.world_random_mut(),
            sub_tick_order: &mut runtime.sub_tick_order,
            host: &mut host,
            facts: world_facts,
            game_time,
        };
        let result = if let Some((p, old, new, flags)) = change {
            runtime
                .simulation
                .on_block_changed_with_flags(&mut context, p, old, new, u32::from(flags))
                .map(|()| FluidTickReport::default())
        } else {
            runtime
                .simulation
                .tick_due(&mut context, 65_536, |chunk| tickable.contains(&chunk))
        };
        if let Err(error) = &result {
            runtime.stopped = Some(format!("{error:#}"));
        }
        result
    }
    pub(super) fn observe_fluid_change(
        &mut self,
        world: &mut WorldBackend,
        position: (i32, i32, i32),
        old: u32,
        new: u32,
        flags: u8,
        now: i64,
        border: CollisionBorder,
        events: &broadcast::Sender<PlayerEvent>,
    ) -> Result<()> {
        self.fluid_frame(
            world,
            now,
            border,
            events,
            Some((position, old, new, flags)),
        )
        .map(|_| ())
    }
    pub(super) fn schedule_water_tick(
        &mut self,
        world: &mut WorldBackend,
        position: (i32, i32, i32),
        now: i64,
    ) -> Result<()> {
        let facts = facts(&self.world_context)?;
        self.fluid_runtime.prepare_halo(
            world,
            &self.chunk_runtime,
            (position.0.div_euclid(16), position.2.div_euclid(16)),
            now,
        )?;
        self.fluid_runtime.simulation.schedule(
            ScheduledFluid::Water,
            position,
            now,
            facts.delay(ScheduledFluid::Water),
            &mut self.fluid_runtime.sub_tick_order,
        )?;
        Ok(())
    }
}
impl Server {
    pub(super) async fn tick_fluid_world(&self) -> Result<FluidTickReport> {
        let game_time = self
            .clock
            .read()
            .map_err(|_| anyhow!("world clock lock poisoned"))?
            .game_time;
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
        players.fluid_frame(&mut world, game_time, border, &self.entity_events, None)
    }
}
