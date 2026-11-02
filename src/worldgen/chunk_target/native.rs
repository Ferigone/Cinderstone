use super::{
    GenerationTargetHost, HeightRange, NoiseHeightmap, SourceStateFacts, TargetDefaultStates,
    TargetStateFacts,
};
use crate::{
    chunk_readiness::ChunkStatus,
    density::DensityPoint,
    worldgen::retrogen::{BelowZeroRetrogen, RetrogenSlot},
};
use anyhow::{bail, ensure, Context, Result};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, OnceLock,
};

/// The actual signed-short counters, including source recount/incremental
/// differences. Importing an existing section must not normalize its counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectionCounts {
    pub non_empty_blocks: i16,
    pub ticking_blocks: i16,
    pub ticking_fluids: i16,
}

/// Owned canonical state and biome palettes for one physical section. Neither
/// this allocation nor its state palette is cloned or replaced during NOISE.
pub struct NativeSection {
    states: Box<[u32; 4096]>,
    biomes: Box<[String; 64]>,
    counts: SectionCounts,
    acquired: Arc<AtomicBool>,
}
impl NativeSection {
    pub fn from_existing_parts(
        states: Box<[u32; 4096]>,
        biomes: Box<[String; 64]>,
        counts: SectionCounts,
        facts: &dyn SourceStateFacts,
    ) -> Result<Box<Self>> {
        for state in states.iter() {
            facts.state(*state)?;
        }
        for biome in biomes.iter() {
            validate_biome(biome)?;
        }
        Ok(Box::new(Self {
            states,
            biomes,
            counts,
            acquired: Arc::new(AtomicBool::new(false)),
        }))
    }
    /// Mirrors the original section constructor that recounts an existing
    /// palette. It is distinct from importing already maintained counters.
    pub fn from_palettes(
        states: Box<[u32; 4096]>,
        biomes: Box<[String; 64]>,
        facts: &dyn SourceStateFacts,
    ) -> Result<Box<Self>> {
        let mut counts = SectionCounts {
            non_empty_blocks: 0,
            ticking_blocks: 0,
            ticking_fluids: 0,
        };
        for state in states.iter() {
            let f = facts.state(*state)?;
            if !f.is_air {
                counts.non_empty_blocks = counts.non_empty_blocks.wrapping_add(1);
                if f.randomly_ticking_block {
                    counts.ticking_blocks = counts.ticking_blocks.wrapping_add(1);
                }
            }
            if f.has_fluid {
                counts.non_empty_blocks = counts.non_empty_blocks.wrapping_add(1);
                if f.randomly_ticking_fluid {
                    counts.ticking_fluids = counts.ticking_fluids.wrapping_add(1);
                }
            }
        }
        Self::from_existing_parts(states, biomes, counts, facts)
    }
    pub fn states(&self) -> &[u32; 4096] {
        &self.states
    }
    pub fn biomes(&self) -> &[String; 64] {
        &self.biomes
    }
    pub fn counts(&self) -> SectionCounts {
        self.counts
    }
    pub fn is_acquired(&self) -> bool {
        self.acquired.load(Ordering::Acquire)
    }
    fn set(
        &mut self,
        point: DensityPoint,
        state: u32,
        facts: &dyn SourceStateFacts,
    ) -> Result<u32> {
        let index = (((point.y & 15) << 8) | ((point.z & 15) << 4) | (point.x & 15)) as usize;
        let old = self.states[index];
        let before = facts.state(old)?;
        let after = facts.state(state)?;
        self.states[index] = state;
        if !before.is_air {
            self.counts.non_empty_blocks = self.counts.non_empty_blocks.wrapping_sub(1);
            if before.randomly_ticking_block {
                self.counts.ticking_blocks = self.counts.ticking_blocks.wrapping_sub(1);
            }
        }
        if before.has_fluid {
            self.counts.ticking_fluids = self.counts.ticking_fluids.wrapping_sub(1);
        }
        if !after.is_air {
            self.counts.non_empty_blocks = self.counts.non_empty_blocks.wrapping_add(1);
            if after.randomly_ticking_block {
                self.counts.ticking_blocks = self.counts.ticking_blocks.wrapping_add(1);
            }
        }
        if after.has_fluid {
            self.counts.ticking_fluids = self.counts.ticking_fluids.wrapping_add(1);
        }
        Ok(old)
    }
}

/// Original SimpleBitStorage layout: values never straddle 64-bit words.
/// Retain packed input exactly; map existence is separate from priming.
pub struct NativeHeightmap {
    build: HeightRange,
    bits: u32,
    values_per_long: usize,
    raw: Box<[u64]>,
}
impl NativeHeightmap {
    pub fn from_raw(build: HeightRange, raw: Box<[u64]>) -> Result<Self> {
        HeightRange::new(build.min_y, build.height)?;
        let bits = 32 - (build.height as u32).leading_zeros();
        ensure!(bits > 0 && bits <= 32, "unsupported heightmap bit width");
        let values_per_long = 64 / bits as usize;
        ensure!(
            raw.len() == 256_usize.div_ceil(values_per_long),
            "heightmap packed length differs"
        );
        Ok(Self {
            build,
            bits,
            values_per_long,
            raw,
        })
    }
    pub fn unprimed(build: HeightRange) -> Result<Self> {
        HeightRange::new(build.min_y, build.height)?;
        let bits = 32 - (build.height as u32).leading_zeros();
        let count = 256_usize.div_ceil(64 / bits as usize);
        Self::from_raw(build, vec![0; count].into_boxed_slice())
    }
    pub fn raw(&self) -> &[u64] {
        &self.raw
    }
    pub fn first_available(&self, x: i32, z: i32) -> i32 {
        let index = ((z & 15) * 16 + (x & 15)) as usize;
        let shift = (index % self.values_per_long) * self.bits as usize;
        let mask = (1_u64 << self.bits) - 1;
        (((self.raw[index / self.values_per_long] >> shift) & mask) as i32)
            .wrapping_add(self.build.min_y)
    }
    fn set_height(&mut self, x: i32, z: i32, first_available: i32) -> Result<()> {
        let index = ((z & 15) * 16 + (x & 15)) as usize;
        let shift = (index % self.values_per_long) * self.bits as usize;
        let mask = (1_u64 << self.bits) - 1;
        let word = &mut self.raw[index / self.values_per_long];
        let offset = first_available.wrapping_sub(self.build.min_y);
        ensure!(
            offset >= 0 && (offset as u64) <= mask,
            "heightmap value outside original packed storage range"
        );
        let value = offset as u64;
        *word = (*word & !(mask << shift)) | (value << shift);
        Ok(())
    }
}

/// The input's existing allocations and unrelated owners are moved, not cloned.
/// A section's Y is build.min_section_y()+its index. No section may be missing.
pub struct NativeTargetParts<M = ()> {
    pub chunk: (i32, i32),
    pub build: HeightRange,
    pub sections: Box<[Box<NativeSection>]>,
    pub persisted_status: ChunkStatus,
    pub unsaved: bool,
    pub retrogen: RetrogenSlot,
    pub ocean_floor_wg: Option<NativeHeightmap>,
    pub world_surface_wg: Option<NativeHeightmap>,
    pub postprocessing: Box<[Option<Vec<i16>>]>,
    /// Observed SharedConstants.debugVoidTerrain for this actual chunk.
    /// There is deliberately no default unknown-to-false conversion.
    pub debug_void_terrain: bool,
    /// Actual caller-owned ticks, block entities, other maps, structures, etc.
    /// The supported operations neither reinterpret nor replace this owner.
    pub unrelated: M,
}

pub struct NativeProtoTarget<M = ()> {
    parts: NativeTargetParts<M>,
    facts: Arc<dyn SourceStateFacts>,
}
impl<M> NativeProtoTarget<M> {
    pub fn from_parts(
        parts: NativeTargetParts<M>,
        facts: Arc<dyn SourceStateFacts>,
    ) -> Result<Box<Self>> {
        HeightRange::new(parts.build.min_y, parts.build.height)?;
        ensure!(
            parts.build.min_y >= -2032 && parts.build.max_y()? <= 2032,
            "target build bounds exceed original dimension limits"
        );
        ensure!(
            parts.sections.len() == parts.build.section_count()?
                && parts.postprocessing.len() == parts.sections.len(),
            "target section/post array coverage differs"
        );
        ensure!(
            parts.persisted_status.index() < 6,
            "native logical Proto writes with final heightmaps or lighting are unsupported"
        );
        for map in [&parts.ocean_floor_wg, &parts.world_surface_wg]
            .into_iter()
            .flatten()
        {
            ensure!(
                map.build == parts.build,
                "heightmap belongs to different build bounds"
            );
        }
        for section in parts.sections.iter() {
            ensure!(!section.is_acquired(), "cannot import an acquired section");
            for state in section.states.iter() {
                facts.state(*state)?;
            }
            for biome in section.biomes.iter() {
                validate_biome(biome)?;
            }
        }
        for state in [
            facts.defaults().air,
            facts.defaults().void_air,
            facts.defaults().bedrock,
            facts.defaults().deepslate,
        ] {
            facts.state(state)?;
        }
        Ok(Box::new(Self { parts, facts }))
    }
    pub fn sections(&self) -> &[Box<NativeSection>] {
        &self.parts.sections
    }
    pub fn postprocessing(&self) -> &[Option<Vec<i16>>] {
        &self.parts.postprocessing
    }
    pub fn heightmap(&self, kind: NoiseHeightmap) -> Option<&NativeHeightmap> {
        self.map(kind).as_ref()
    }
    pub fn unrelated(&self) -> &M {
        &self.parts.unrelated
    }
    pub fn unrelated_mut(&mut self) -> &mut M {
        &mut self.parts.unrelated
    }
    pub fn physical_height(&self) -> Result<HeightRange> {
        self.parts.build.physical_sections()
    }
    pub fn is_unsaved(&self) -> bool {
        self.parts.unsaved
    }
    /// Explicit source persisted-status event; generation phase success never
    /// invokes this setter implicitly. Retrogen clearing follows this event.
    pub fn set_persisted_status(&mut self, status: ChunkStatus) -> Result<()> {
        ensure!(
            status.index() < 6,
            "native final-heightmap/lighting status is unsupported"
        );
        self.parts.retrogen.observe()?;
        self.parts.persisted_status = status;
        if let RetrogenSlot::Present(retrogen) = &self.parts.retrogen {
            if status >= retrogen.target_status() {
                self.parts.retrogen = RetrogenSlot::ObservedAbsent;
            }
        }
        self.parts.unsaved = true;
        Ok(())
    }
    pub fn into_parts(self: Box<Self>) -> NativeTargetParts<M> {
        self.parts
    }
    fn map(&self, kind: NoiseHeightmap) -> &Option<NativeHeightmap> {
        match kind {
            NoiseHeightmap::OceanFloorWg => &self.parts.ocean_floor_wg,
            NoiseHeightmap::WorldSurfaceWg => &self.parts.world_surface_wg,
        }
    }
    fn map_mut(&mut self, kind: NoiseHeightmap) -> &mut Option<NativeHeightmap> {
        match kind {
            NoiseHeightmap::OceanFloorWg => &mut self.parts.ocean_floor_wg,
            NoiseHeightmap::WorldSurfaceWg => &mut self.parts.world_surface_wg,
        }
    }
    fn section_index(&self, y: i32) -> Result<usize> {
        let index = y.div_euclid(16) - self.parts.build.min_section_y();
        ensure!(
            index >= 0 && (index as usize) < self.parts.sections.len(),
            "target coordinate outside physical section coverage"
        );
        Ok(index as usize)
    }
    fn opaque(&self, kind: NoiseHeightmap, state: u32) -> Result<bool> {
        let f = self.facts.state(state)?;
        Ok(match kind {
            NoiseHeightmap::OceanFloorWg => f.ocean_floor_wg_opaque,
            NoiseHeightmap::WorldSurfaceWg => f.world_surface_wg_opaque,
        })
    }
    fn prime_maps(&mut self, kinds: &[NoiseHeightmap]) -> Result<()> {
        for kind in kinds {
            self.get_or_create_wg_heightmap(*kind)?;
        }
        // primeHeightmaps starts at highest nonempty section+16, and checks
        // canonical AIR before its predicates. It leaves empty columns intact.
        let top = self
            .parts
            .sections
            .iter()
            .rposition(|s| s.counts.non_empty_blocks != 0)
            .map(|i| (self.parts.build.min_section_y() + i as i32) * 16 + 16)
            .unwrap_or(self.parts.build.min_y.wrapping_add(16));
        for x in 0..16 {
            for z in 0..16 {
                let mut pending = kinds.to_vec();
                for y in (self.parts.build.min_y..top).rev() {
                    let state = self.block(DensityPoint::new(x, y, z))?;
                    if state == self.default_states().air {
                        continue;
                    }
                    let mut next = Vec::new();
                    for kind in pending {
                        if self.opaque(kind, state)? {
                            self.map_mut(kind)
                                .as_mut()
                                .expect("map acquired")
                                .set_height(x, z, y.wrapping_add(1))?;
                        } else {
                            next.push(kind);
                        }
                    }
                    pending = next;
                    if pending.is_empty() {
                        break;
                    }
                }
            }
        }
        Ok(())
    }
}

/// Exclusive ownership of the actual section lock cells. Drop releases exactly
/// the acquired allocations even after an interrupted material callback.
pub struct NoiseSectionLease {
    locks: Vec<(usize, Arc<AtomicBool>)>,
}
impl Drop for NoiseSectionLease {
    fn drop(&mut self) {
        for (_, lock) in &self.locks {
            lock.store(false, Ordering::Release);
        }
    }
}
impl<M> GenerationTargetHost for NativeProtoTarget<M> {
    type SectionLease = NoiseSectionLease;
    fn chunk_pos(&self) -> (i32, i32) {
        self.parts.chunk
    }
    fn build_height(&self) -> HeightRange {
        self.parts.build
    }
    fn generation_height(&self) -> Result<HeightRange> {
        Ok(if self.retrogen()?.is_some() {
            HeightRange {
                min_y: -64,
                height: 64,
            }
        } else {
            self.parts.build
        })
    }
    fn persisted_status(&self) -> ChunkStatus {
        self.parts.persisted_status
    }
    fn highest_generated_status(&self) -> Result<ChunkStatus> {
        Ok(self
            .retrogen()?
            .map(|r| self.parts.persisted_status.max(r.target_status()))
            .unwrap_or(self.parts.persisted_status))
    }
    fn retrogen(&self) -> Result<Option<Arc<BelowZeroRetrogen>>> {
        match &self.parts.retrogen {
            RetrogenSlot::ObservedAbsent => Ok(None),
            RetrogenSlot::Present(r) => Ok(Some(Arc::clone(r))),
            RetrogenSlot::Unavailable => bail!("actual target retrogen owner is unavailable"),
        }
    }
    fn state_facts(&self, state: u32) -> Result<TargetStateFacts> {
        self.facts.state(state)
    }
    fn default_states(&self) -> TargetDefaultStates {
        self.facts.defaults()
    }
    fn debug_void_terrain(&self) -> Result<bool> {
        Ok(self.parts.debug_void_terrain)
    }
    fn acquire_noise_sections(&mut self, top: i32, bottom: i32) -> Result<NoiseSectionLease> {
        ensure!(top >= bottom, "invalid descending section lease span");
        let first = self.parts.build.min_section_y();
        let last = first + self.parts.sections.len() as i32 - 1;
        ensure!(
            bottom >= first && top <= last,
            "section lease outside physical coverage"
        );
        let mut lease = NoiseSectionLease { locks: Vec::new() };
        for y in (bottom..=top).rev() {
            let index = y - self.parts.build.min_section_y();
            ensure!(
                index >= 0 && (index as usize) < self.parts.sections.len(),
                "section lease outside physical coverage"
            );
            let index = index as usize;
            let lock = Arc::clone(&self.parts.sections[index].acquired);
            ensure!(
                lock.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok(),
                "section is already exclusively acquired"
            );
            lease.locks.push((index, lock));
        }
        Ok(lease)
    }
    fn get_or_create_wg_heightmap(&mut self, kind: NoiseHeightmap) -> Result<()> {
        if self.map(kind).is_none() {
            let map = NativeHeightmap::unprimed(self.parts.build)?;
            *self.map_mut(kind) = Some(map);
        }
        Ok(())
    }
    fn direct_section_set(
        &mut self,
        lease: &NoiseSectionLease,
        p: DensityPoint,
        state: u32,
    ) -> Result<u32> {
        let i = self.section_index(p.y)?;
        ensure!(
            lease.locks.iter().any(|(index, lock)| *index == i
                && Arc::ptr_eq(lock, &self.parts.sections[i].acquired)
                && lock.load(Ordering::Acquire)),
            "direct section write has no matching exclusive lease"
        );
        self.parts.sections[i].set(p, state, self.facts.as_ref())
    }
    fn update_wg_heightmap(
        &mut self,
        kind: NoiseHeightmap,
        p: DensityPoint,
        state: u32,
    ) -> Result<bool> {
        let first = self
            .map(kind)
            .as_ref()
            .context("WG heightmap was not acquired")?
            .first_available(p.x, p.z);
        if p.y <= first.wrapping_sub(2) {
            return Ok(false);
        }
        if self.opaque(kind, state)? {
            if p.y >= first {
                self.map_mut(kind)
                    .as_mut()
                    .expect("map acquired")
                    .set_height(p.x, p.z, p.y.wrapping_add(1))?;
                return Ok(true);
            }
        } else if first.wrapping_sub(1) == p.y {
            let mut y = p.y.wrapping_sub(1);
            while y >= self.parts.build.min_y {
                let below = self.block(DensityPoint::new(p.x, y, p.z))?;
                if self.opaque(kind, below)? {
                    break;
                }
                y -= 1;
            }
            let height = if y < self.parts.build.min_y {
                self.parts.build.min_y
            } else {
                y.wrapping_add(1)
            };
            self.map_mut(kind)
                .as_mut()
                .expect("map acquired")
                .set_height(p.x, p.z, height)?;
            return Ok(true);
        }
        Ok(false)
    }
    fn mark_pos_for_postprocessing(&mut self, p: DensityPoint) -> Result<()> {
        if !self.parts.build.contains(p.y) {
            return Ok(());
        }
        let i = self.section_index(p.y)?;
        let packed = ((p.x & 15) | ((p.y & 15) << 4) | ((p.z & 15) << 8)) as i16;
        self.parts.postprocessing[i]
            .get_or_insert_with(Vec::new)
            .push(packed);
        Ok(())
    }
    fn block(&self, p: DensityPoint) -> Result<u32> {
        if !self.parts.build.contains(p.y) {
            return Ok(self.default_states().void_air);
        }
        let section = &self.parts.sections[self.section_index(p.y)?];
        if section.counts.non_empty_blocks == 0 {
            return Ok(self.default_states().air);
        }
        Ok(section.states[(((p.y & 15) << 8) | ((p.z & 15) << 4) | (p.x & 15)) as usize])
    }
    fn proto_set_block(&mut self, p: DensityPoint, state: u32) -> Result<u32> {
        if !self.parts.build.contains(p.y) {
            return Ok(self.default_states().void_air);
        }
        ensure!(
            self.parts.persisted_status.index() < 6,
            "logical Proto final-heightmap/lighting context is unsupported"
        );
        self.facts.state(state)?;
        let i = self.section_index(p.y)?;
        if self.parts.sections[i].counts.non_empty_blocks == 0 && state == self.default_states().air
        {
            return Ok(state);
        }
        let lease = self.acquire_noise_sections(p.y.div_euclid(16), p.y.div_euclid(16))?;
        let old = self.direct_section_set(&lease, p, state)?;
        drop(lease);
        // ProtoChunk iterates PRE_FEATURES in EnumSet ordinal order: WS, OF.
        let kinds = [NoiseHeightmap::WorldSurfaceWg, NoiseHeightmap::OceanFloorWg];
        let missing: Vec<_> = kinds
            .iter()
            .copied()
            .filter(|k| self.map(*k).is_none())
            .collect();
        if !missing.is_empty() {
            self.prime_maps(&missing)?;
        }
        for kind in kinds {
            self.update_wg_heightmap(kind, p, state)?;
        }
        Ok(old)
    }
    fn wg_height(&mut self, kind: NoiseHeightmap, x: i32, z: i32) -> Result<i32> {
        if self.map(kind).is_none() {
            self.prime_maps(&[kind])?;
        }
        Ok(self
            .map(kind)
            .as_ref()
            .expect("map primed")
            .first_available(x, z)
            .wrapping_sub(1))
    }
    fn get_noise_biome(&self, x: i32, y: i32, z: i32) -> Result<String> {
        ensure!(
            self.highest_generated_status()?.index() >= 3,
            "BIOMES data requested before actual BIOMES status"
        );
        let min = self.parts.build.min_y >> 2;
        // ChunkAccess uses minQuart + quartCount - 1, not floor(maxY/4).
        // Mth.clamp remains Math.min(Math.max(...)) even for a zero count.
        let max = min
            .wrapping_add(self.parts.build.height >> 2)
            .wrapping_sub(1);
        let y = y.max(min).min(max);
        let i = self.section_index(y.wrapping_mul(4))?;
        Ok(
            self.parts.sections[i].biomes[(((y & 3) << 4) | ((z & 3) << 2) | (x & 3)) as usize]
                .clone(),
        )
    }
    fn biome_registry_key(&self, identity: &str) -> Result<Option<String>> {
        validate_biome(identity)?;
        Ok(Some(identity.to_owned()))
    }
    fn commit_biome_section(&mut self, section_y: i32, biomes: Box<[String; 64]>) -> Result<()> {
        for biome in biomes.iter() {
            validate_biome(biome)?;
        }
        let i = section_y - self.parts.build.min_section_y();
        ensure!(
            i >= 0 && (i as usize) < self.parts.sections.len(),
            "biome palette outside physical sections"
        );
        self.parts.sections[i as usize].biomes = biomes;
        Ok(())
    }
}

fn validate_biome(name: &str) -> Result<()> {
    static BIOMES: OnceLock<serde_json::Value> = OnceLock::new();
    let data = BIOMES.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../fixtures/world-generation-biome-climate.json"
        ))
        .expect("packaged biome identities")
    });
    ensure!(
        data["biomes"].get(name).is_some(),
        "native target requires an actual built-in biome identity: {name}"
    );
    Ok(())
}
