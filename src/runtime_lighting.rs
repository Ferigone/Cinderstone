//! Stable live lighting for the actual native flat-world source.
//!
//! This composes the already ported light engine with a complete 5x5 chunk
//! dependency domain and installs only its inner 3x3 completed outputs. It
//! reproduces stable values and section packets; vanilla's threaded queue and
//! intermediate publication timing are not represented by this synchronous host.
use crate::{
    lighting::{self, LightVolume},
    nbt::Tag,
    protocol::{Reader, Writer},
    world::{BlockChange, FlatWorld, MAX_Y, MIN_Y},
};
use anyhow::{ensure, Context, Result};
use std::collections::{BTreeMap, BTreeSet};

type Chunk = (i32, i32);
type Position = (i32, i32, i32);
const MIN_SECTION: i32 = MIN_Y / 16;
const SECTION_COUNT: usize = ((MAX_Y - MIN_Y + 1) / 16) as usize;
const LIGHT_SECTIONS: usize = SECTION_COUNT + 2;
const MAX_CACHED_CHUNKS: usize = 256;

struct CachedVolume {
    center: Chunk,
    volume: LightVolume,
    source: BTreeMap<Position, u32>,
}
struct SectionLight {
    sky: Vec<u8>,
    block: Vec<u8>,
}
struct CompletedChunk {
    sections: Vec<SectionLight>,
    motion_blocking: [i32; 256],
    wire: Vec<u8>,
    revision: u64,
    dirty: bool,
}
#[derive(Clone, Debug)]
pub struct NativeLightUpdate {
    pub chunk: Chunk,
    /// Clientbound Light Update (0x2a) payload, including VarInt chunk X/Z.
    pub payload: Vec<u8>,
    pub changed: bool,
    pub revision: u64,
}
#[derive(Clone, Debug)]
pub struct NativeRelightResult {
    /// All nine completed outputs, including byte-identical previous outputs.
    pub updates: Vec<NativeLightUpdate>,
    pub rebuilt_columns: usize,
    pub source_generation: u64,
}

/// An explicit native source owner. Imported Anvil worlds have a different
/// owner and cannot acquire this certificate merely by missing their light.
/// Every public query also compares the actual FlatWorld changes, so a missed
/// mutation notification cannot return a stale cached value.
pub struct NativeRuntimeLighting {
    sky_enabled: bool,
    observed_source: Option<BTreeMap<Position, u32>>,
    pending_centers: BTreeSet<Chunk>,
    chunks: BTreeMap<Chunk, CompletedChunk>,
    revisions: BTreeMap<Chunk, u64>,
    cache: Option<CachedVolume>,
    source_generation: u64,
}
impl NativeRuntimeLighting {
    pub fn new(sky_enabled: bool) -> Self {
        Self {
            sky_enabled,
            observed_source: None,
            pending_centers: BTreeSet::new(),
            chunks: BTreeMap::new(),
            revisions: BTreeMap::new(),
            cache: None,
            source_generation: 0,
        }
    }
    pub fn sky_enabled(&self) -> bool {
        self.sky_enabled
    }
    pub fn source_generation(&self) -> u64 {
        self.source_generation
    }
    pub fn pending_centers(&self) -> impl Iterator<Item = Chunk> + '_ {
        self.pending_centers.iter().copied()
    }
    /// Main calls this at its accepted block-write boundary, before allowing
    /// another chunk/item/rain phase to use a completed-light certificate.
    pub fn invalidate(&mut self, position: Position) -> Result<()> {
        ensure!(
            (MIN_Y..=MAX_Y).contains(&position.1),
            "light edit outside build height"
        );
        self.mark_center((position.0.div_euclid(16), position.2.div_euclid(16)))
    }
    fn mark_center(&mut self, center: Chunk) -> Result<()> {
        self.pending_centers.insert(center);
        for dz in -1..=1 {
            for dx in -1..=1 {
                let chunk = offset(center, dx, dz)?;
                if let Some(cached) = self.chunks.get_mut(&chunk) {
                    cached.dirty = true;
                }
            }
        }
        Ok(())
    }
    fn snapshot(world: &FlatWorld) -> Result<BTreeMap<Position, u32>> {
        ensure!(
            world.base_states() == [0, 1, 10, 9],
            "native lighting requires the canonical native flat-world base"
        );
        for state in world.base_states() {
            lighting::state_properties(state)?;
        }
        world
            .changes()
            .into_iter()
            .map(|BlockChange { x, y, z, state }| {
                lighting::state_properties(state)?;
                Ok(((x, y, z), state))
            })
            .collect()
    }
    /// Reconcile every actual diff, including removals back to the base state.
    /// This is a source revision check, not a fabricated fresh-world witness.
    pub fn synchronize_source(&mut self, world: &FlatWorld) -> Result<()> {
        let next = Self::snapshot(world)?;
        if self.observed_source.as_ref() == Some(&next) {
            return Ok(());
        }
        let mut changed_centers = BTreeSet::new();
        if let Some(previous) = &self.observed_source {
            for (&p, state) in previous {
                if next.get(&p) != Some(state) {
                    changed_centers.insert((p.0.div_euclid(16), p.2.div_euclid(16)));
                }
            }
        }
        for (&p, state) in &next {
            if self.observed_source.as_ref().and_then(|old| old.get(&p)) != Some(state) {
                changed_centers.insert((p.0.div_euclid(16), p.2.div_euclid(16)));
            }
        }
        let generation = self
            .source_generation
            .checked_add(1)
            .context("native lighting source revision exhausted")?;
        for center in changed_centers {
            self.mark_center(center)?;
        }
        self.observed_source = Some(next);
        self.source_generation = generation;
        Ok(())
    }
    /// Current ownership state for Main's source gate. Use require_current at
    /// a boundary where the actual FlatWorld is available as an additional
    /// missed-notification check.
    pub fn require_settled(&self) -> Result<()> {
        ensure!(
            self.observed_source.is_some(),
            "native light source has not been observed"
        );
        ensure!(
            self.pending_centers.is_empty(),
            "native lighting has pending block edits"
        );
        Ok(())
    }
    pub fn require_current(&self, world: &FlatWorld) -> Result<()> {
        self.require_settled()?;
        ensure!(
            self.observed_source.as_ref() == Some(&Self::snapshot(world)?),
            "native light source changed without reconciliation"
        );
        Ok(())
    }
    /// Finish retained work in source order. A failing build leaves its pending
    /// center intact and never installs a partly computed nine-chunk result.
    pub fn process_pending(&mut self, world: &FlatWorld) -> Result<Vec<NativeLightUpdate>> {
        self.synchronize_source(world)?;
        let mut updates = Vec::new();
        while let Some(center) = self.pending_centers.first().copied() {
            updates.extend(self.relight_around(world, center.0, center.1)?.updates);
        }
        self.require_current(world)?;
        Ok(updates)
    }
    pub fn relight_around(
        &mut self,
        world: &FlatWorld,
        cx: i32,
        cz: i32,
    ) -> Result<NativeRelightResult> {
        self.synchronize_source(world)?;
        let center = (cx, cz);
        let domain = (-2..=2)
            .flat_map(|dz| (-2..=2).map(move |dx| offset(center, dx, dz)))
            .collect::<Result<Vec<_>>>()?;
        let source = self
            .observed_source
            .as_ref()
            .context("native source missing")?
            .clone();
        let cached = self.cache.take();
        let (mut volume, previous_source, rebuild) = match cached {
            Some(CachedVolume {
                center: old,
                volume,
                source,
            }) if old == center => (volume, source, false),
            _ => (
                LightVolume::new_with_sky(
                    MIN_Y,
                    MAX_Y - MIN_Y + 1,
                    domain.iter().copied(),
                    self.sky_enabled,
                )?,
                BTreeMap::new(),
                true,
            ),
        };
        if rebuild {
            for &(x, z) in &domain {
                for y in MIN_Y..=MAX_Y {
                    for lz in 0..16 {
                        for lx in 0..16 {
                            let (gx, gz) = (x * 16 + lx, z * 16 + lz);
                            let state = world.block(gx, y, gz);
                            if state != 0 {
                                volume.set_block(gx, y, gz, state)?;
                            }
                        }
                    }
                }
            }
        } else {
            let changed_positions = previous_source
                .keys()
                .chain(source.keys())
                .copied()
                .collect::<BTreeSet<_>>();
            for p in changed_positions {
                if previous_source.get(&p) != source.get(&p)
                    && volume.block(p.0, p.1, p.2).is_some()
                {
                    volume.set_block(p.0, p.1, p.2, world.block(p.0, p.1, p.2))?;
                }
            }
        }
        volume.relight()?;
        let rebuilt_columns = volume.last_rebuilt_columns();
        // This is the same section-allocation neighbourhood used by the native
        // storage host: a non-air section allocates all 26 adjacent sections.
        let mut active = BTreeSet::new();
        for &(x, z) in &domain {
            for sy in MIN_SECTION..MIN_SECTION + SECTION_COUNT as i32 {
                let mut nonempty = false;
                'section: for ly in 0..16 {
                    for lz in 0..16 {
                        for lx in 0..16 {
                            if !lighting::state_properties(
                                volume
                                    .block(x * 16 + lx, sy * 16 + ly, z * 16 + lz)
                                    .context("light domain disappeared")?,
                            )?
                            .is_air()
                            {
                                nonempty = true;
                                break 'section;
                            }
                        }
                    }
                }
                if nonempty {
                    for dy in -1..=1 {
                        for dz in -1..=1 {
                            for dx in -1..=1 {
                                active.insert((x + dx, sy + dy, z + dz));
                            }
                        }
                    }
                }
            }
        }
        let mut completed = Vec::new();
        for dz in -1..=1 {
            for dx in -1..=1 {
                let chunk = offset(center, dx, dz)?;
                let mut sections = Vec::with_capacity(LIGHT_SECTIONS);
                let mut allocated = 0_u32;
                for index in 0..LIGHT_SECTIONS {
                    let sy = MIN_SECTION - 1 + index as i32;
                    let (sky, block) = volume.section_light(chunk.0, chunk.1, sy)?;
                    if active.contains(&(chunk.0, sy, chunk.1)) {
                        allocated |= 1 << index;
                    }
                    sections.push(SectionLight { sky, block });
                }
                let wire = encode_light_data(&sections, allocated, self.sky_enabled);
                let mut motion_blocking = [MIN_Y; 256];
                for lz in 0..16 {
                    for lx in 0..16 {
                        for y in (MIN_Y..=MAX_Y).rev() {
                            let state = volume
                                .block(chunk.0 * 16 + lx, y, chunk.1 * 16 + lz)
                                .context("native heightmap domain disappeared")?;
                            let properties = lighting::state_properties(state)?;
                            if properties.blocks_motion() || properties.has_fluid() {
                                motion_blocking[(lz * 16 + lx) as usize] = y + 1;
                                break;
                            }
                        }
                    }
                }
                let old = self.chunks.get(&chunk);
                let changed = old.is_none_or(|old| old.wire != wire);
                let previous_revision = self.revisions.get(&chunk).copied().unwrap_or(0);
                let revision = if changed {
                    previous_revision
                        .checked_add(1)
                        .context("native chunk light revision exhausted")?
                } else {
                    previous_revision
                };
                let mut payload = Writer::new();
                payload.varint(chunk.0).varint(chunk.1).raw(&wire);
                completed.push((
                    chunk,
                    CompletedChunk {
                        sections,
                        motion_blocking,
                        wire,
                        revision,
                        dirty: false,
                    },
                    NativeLightUpdate {
                        chunk,
                        payload: payload.into_inner(),
                        changed,
                        revision,
                    },
                ));
            }
        }
        let mut updates = Vec::with_capacity(9);
        for (chunk, cached, update) in completed {
            self.revisions.insert(chunk, update.revision);
            self.chunks.insert(chunk, cached);
            updates.push(update);
        }
        // Only this edit center is complete. Another center may require an
        // additional outer row even when its own chunk was in these outputs.
        self.pending_centers.remove(&center);
        self.cache = Some(CachedVolume {
            center,
            volume,
            source,
        });
        while self.chunks.len() > MAX_CACHED_CHUNKS {
            let victim = self
                .chunks
                .keys()
                .find(|&&(x, z)| (x - cx).abs() > 1 || (z - cz).abs() > 1)
                .copied();
            if let Some(victim) = victim {
                self.chunks.remove(&victim);
            } else {
                break;
            }
        }
        Ok(NativeRelightResult {
            updates,
            rebuilt_columns,
            source_generation: self.source_generation,
        })
    }
    fn ensure_chunk(&mut self, world: &FlatWorld, chunk: Chunk) -> Result<()> {
        self.synchronize_source(world)?;
        if self.chunks.get(&chunk).is_none_or(|chunk| chunk.dirty) {
            self.relight_around(world, chunk.0, chunk.1)?;
        }
        Ok(())
    }
    pub fn light(&mut self, world: &FlatWorld, position: Position) -> Result<(u8, u8)> {
        if position.1 > MAX_Y + 16 {
            // No native source block exists above MAX_Y. Even a level-15
            // emitter at MAX_Y has attenuated to zero beyond this padding;
            // skylight here is the unobstructed top source in this dimension.
            Self::snapshot(world)?;
            return Ok((if self.sky_enabled { 15 } else { 0 }, 0));
        }
        ensure!(
            (MIN_Y - 16..=MAX_Y + 16).contains(&position.1),
            "light query outside computed section padding"
        );
        let chunk = (position.0.div_euclid(16), position.2.div_euclid(16));
        self.ensure_chunk(world, chunk)?;
        let index = (position.1 - (MIN_Y - 16)) as usize;
        let section = &self.chunks[&chunk].sections[index / 16];
        let cell = ((index % 16) * 16 + position.2.rem_euclid(16) as usize) * 16
            + position.0.rem_euclid(16) as usize;
        let nibble = |data: &[u8]| (data[cell / 2] >> ((cell & 1) * 4)) & 15;
        Ok((nibble(&section.sky), nibble(&section.block)))
    }
    pub fn raw_brightness(
        &mut self,
        world: &FlatWorld,
        position: Position,
        sky_darken: u8,
    ) -> Result<u8> {
        let (sky, block) = self.light(world, position)?;
        Ok(block.max(sky.saturating_sub(sky_darken)))
    }
    /// Heightmap.Types.MOTION_BLOCKING: first Y above the highest state whose
    /// original light/heightmap flags report blocksMotion or nonempty fluid.
    pub fn motion_blocking_height(&mut self, world: &FlatWorld, position: Position) -> Result<i32> {
        let chunk = (position.0.div_euclid(16), position.2.div_euclid(16));
        self.ensure_chunk(world, chunk)?;
        let index = position.2.rem_euclid(16) as usize * 16 + position.0.rem_euclid(16) as usize;
        Ok(self.chunks[&chunk].motion_blocking[index])
    }
    pub fn light_packet(&mut self, world: &FlatWorld, cx: i32, cz: i32) -> Result<Vec<u8>> {
        self.ensure_chunk(world, (cx, cz))?;
        let mut out = Writer::new();
        out.varint(cx).varint(cz).raw(&self.chunks[&(cx, cz)].wire);
        Ok(out.into_inner())
    }
    pub fn light_revision(&mut self, world: &FlatWorld, cx: i32, cz: i32) -> Result<u64> {
        self.ensure_chunk(world, (cx, cz))?;
        Ok(self.chunks[&(cx, cz)].revision)
    }
    /// Preserve the actual native block/biome/heightmap payload and replace its
    /// old full-skylight placeholder with completed light-engine section data.
    pub fn chunk_packet(&mut self, world: &FlatWorld, cx: i32, cz: i32) -> Result<Vec<u8>> {
        self.ensure_chunk(world, (cx, cz))?;
        let mut packet = world.chunk_packet(cx, cz);
        let mut input = Reader::new(&packet);
        input.i32()?;
        input.i32()?;
        let (_, used) = Tag::decode_anonymous_prefix(input.remaining())?;
        input.bytes(used)?;
        let section_len = input.varint()?;
        ensure!(section_len >= 0, "negative native section length");
        input.bytes(section_len as usize)?;
        let entities=input.varint()?;
        ensure!((0..=4096).contains(&entities),"invalid native block-entity count");
        for _ in 0..entities {
            input.u8()?;input.i16()?;input.varint()?;
            let (_,used)=Tag::decode_anonymous_prefix(input.remaining())?;
            input.bytes(used)?;
        }
        let prefix = packet.len() - input.remaining().len();
        packet.truncate(prefix);
        packet.extend_from_slice(&self.chunks[&(cx, cz)].wire);
        Ok(packet)
    }
}
fn offset(center: Chunk, dx: i32, dz: i32) -> Result<Chunk> {
    let x = center
        .0
        .checked_add(dx)
        .context("native light chunk X overflow")?;
    let z = center
        .1
        .checked_add(dz)
        .context("native light chunk Z overflow")?;
    x.checked_mul(16)
        .and_then(|v| v.checked_add(15))
        .context("native light block X overflow")?;
    z.checked_mul(16)
        .and_then(|v| v.checked_add(15))
        .context("native light block Z overflow")?;
    Ok((x, z))
}
fn encode_light_data(sections: &[SectionLight], allocated: u32, sky_enabled: bool) -> Vec<u8> {
    let mut sky_mask = 0_u32;
    let mut block_mask = 0_u32;
    let mut empty_sky = 0_u32;
    let mut empty_block = 0_u32;
    let mut sky_arrays = Vec::new();
    let mut block_arrays = Vec::new();
    for (index, section) in sections.iter().enumerate() {
        if allocated & (1 << index) == 0 {
            continue;
        }
        if sky_enabled {
            if section.sky.iter().all(|&v| v == 0) {
                empty_sky |= 1 << index;
            } else {
                sky_mask |= 1 << index;
                sky_arrays.push(&section.sky);
            }
        }
        if section.block.iter().all(|&v| v == 0) {
            empty_block |= 1 << index;
        } else {
            block_mask |= 1 << index;
            block_arrays.push(&section.block);
        }
    }
    let mut out = Writer::new();
    for bits in [sky_mask, block_mask, empty_sky, empty_block] {
        if bits == 0 {
            out.varint(0);
        } else {
            out.varint(1).i64(i64::from(bits));
        }
    }
    for arrays in [sky_arrays, block_arrays] {
        out.varint(arrays.len() as i32);
        for bytes in arrays {
            out.varint(bytes.len() as i32).raw(bytes);
        }
    }
    out.into_inner()
}
