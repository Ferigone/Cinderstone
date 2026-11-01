//! Stable block/skylight propagation for an explicitly loaded, finite set of chunks.
//!
//! Light facts and every directional face-pair occlusion result are extracted from
//! the unchanged Java 1.21 classes. This module computes stable light values; it
//! does not reproduce Java's asynchronous section publication/queue scheduling.
//! Missing chunks block propagation. Callers must supply the complete dependency
//! area (including a one-chunk halo when updating a smaller region).

use anyhow::{anyhow, bail, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::OnceLock,
};

const DIRECTIONS: [(i32, i32, i32); 6] = [
    (0, -1, 0),
    (0, 1, 0),
    (0, 0, -1),
    (0, 0, 1),
    (-1, 0, 0),
    (1, 0, 0),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LightProperties {
    pub emission: u8,
    pub opacity: u8,
    /// isAir=1, blocksMotion=2, nonempty fluid=4, LeavesBlock=8.
    pub heightmap_flags: u8,
    faces: [u8; 6],
}

impl LightProperties {
    pub fn is_air(self) -> bool {
        self.heightmap_flags & 1 != 0
    }
    pub fn blocks_motion(self) -> bool {
        self.heightmap_flags & 2 != 0
    }
    pub fn has_fluid(self) -> bool {
        self.heightmap_flags & 4 != 0
    }
    pub fn is_leaves(self) -> bool {
        self.heightmap_flags & 8 != 0
    }
}

#[derive(Deserialize)]
struct FactsFile {
    minecraft: String,
    face_count: usize,
    rows: Vec<[u8; 9]>,
    states: Vec<usize>,
    occlusion: String,
}
struct Facts {
    rows: Vec<LightProperties>,
    states: Vec<usize>,
    face_count: usize,
    occlusion: Vec<u8>,
}
fn facts() -> &'static Facts {
    static FACTS: OnceLock<Facts> = OnceLock::new();
    FACTS.get_or_init(|| {
        let file: FactsFile = serde_json::from_str(include_str!("../fixtures/light-1.21.json"))
            .expect("verified built-in light facts");
        assert_eq!(file.minecraft, "1.21");
        assert_eq!(file.states.len(), 26684);
        let rows = file
            .rows
            .into_iter()
            .map(|r| LightProperties {
                emission: r[0],
                opacity: r[1],
                heightmap_flags: r[2],
                faces: r[3..].try_into().unwrap(),
            })
            .collect();
        Facts {
            rows,
            states: file.states,
            face_count: file.face_count,
            occlusion: STANDARD
                .decode(file.occlusion)
                .expect("verified occlusion matrix"),
        }
    })
}

pub fn state_properties(state: u32) -> Result<LightProperties> {
    let f = facts();
    let row = *f
        .states
        .get(state as usize)
        .ok_or_else(|| anyhow!("unknown exact-1.21 state ID {state}"))?;
    Ok(f.rows[row])
}

/// Whether Java's union of the source face and opposite destination face is full.
/// Directions use the protocol order DOWN, UP, NORTH, SOUTH, WEST, EAST.
pub fn edge_occludes(source: u32, destination: u32, direction: usize) -> Result<bool> {
    if direction >= 6 {
        bail!("invalid light propagation direction");
    }
    Ok(occludes(
        state_properties(source)?,
        state_properties(destination)?,
        direction,
    ))
}
fn occludes(a: LightProperties, b: LightProperties, direction: usize) -> bool {
    let f = facts();
    let bit = a.faces[direction] as usize * f.face_count + b.faces[direction ^ 1] as usize;
    f.occlusion[bit / 8] & (1 << (bit % 8)) != 0
}

#[derive(Clone)]
struct Column {
    states: Vec<u32>,
    block: Vec<u8>,
    sky: Vec<u8>,
    lowest_source: [i32; 256],
}

/// A finite set of whole chunk columns. Extra air sections above and below the
/// build range reproduce the light engine's section padding, including sources
/// reaching the underside of the build range around solid obstructions.
pub struct LightVolume {
    min_y: i32,
    height: i32,
    columns: BTreeMap<(i32, i32), Column>,
    dirty: bool,
    sky_enabled: bool,
    initialized: bool,
    changed_columns: BTreeSet<(i32, i32)>,
    last_rebuilt_columns: usize,
}
impl LightVolume {
    pub fn new(
        min_y: i32,
        height: i32,
        chunks: impl IntoIterator<Item = (i32, i32)>,
    ) -> Result<Self> {
        Self::new_with_sky(min_y, height, chunks, true)
    }
    /// Dimensions without skylight retain zero sky values. Callers must omit
    /// their sky-layer data on the wire, as Java uses a dummy sky listener.
    pub fn new_with_sky(
        min_y: i32,
        height: i32,
        chunks: impl IntoIterator<Item = (i32, i32)>,
        sky_enabled: bool,
    ) -> Result<Self> {
        if min_y % 16 != 0
            || height <= 0
            || height % 16 != 0
            || height > 4096
            || min_y
                .checked_add(height)
                .and_then(|v| v.checked_add(16))
                .is_none()
            || min_y.checked_sub(16).is_none()
        {
            bail!("light bounds must be aligned, positive and representable");
        }
        let chunks: BTreeSet<_> = chunks.into_iter().collect();
        if chunks.len() > 16384 {
            bail!("light volume exceeds explicit allocation budget");
        }
        let len = (height as usize + 32) * 256;
        let mut columns = BTreeMap::new();
        for key in chunks {
            if key
                .0
                .checked_mul(16)
                .and_then(|v| v.checked_add(15))
                .is_none()
                || key
                    .1
                    .checked_mul(16)
                    .and_then(|v| v.checked_add(15))
                    .is_none()
            {
                bail!("unrepresentable chunk coordinate");
            }
            columns.insert(
                key,
                Column {
                    states: vec![0; len],
                    block: vec![0; len],
                    sky: vec![0; len],
                    lowest_source: [min_y - 16; 256],
                },
            );
        }
        Ok(Self {
            min_y,
            height,
            columns,
            dirty: true,
            sky_enabled,
            initialized: false,
            changed_columns: BTreeSet::new(),
            last_rebuilt_columns: 0,
        })
    }
    pub fn sky_enabled(&self) -> bool {
        self.sky_enabled
    }
    pub fn loaded_chunks(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        self.columns.keys().copied()
    }
    fn index(&self, x: i32, y: i32, z: i32) -> Option<((i32, i32), usize)> {
        let y = y.checked_sub(self.min_y - 16)?;
        if !(0..self.height + 32).contains(&y) {
            return None;
        }
        let key = (x >> 4, z >> 4);
        self.columns.contains_key(&key).then_some((
            key,
            ((y as usize) * 16 + (z & 15) as usize) * 16 + (x & 15) as usize,
        ))
    }
    pub fn set_block(&mut self, x: i32, y: i32, z: i32, state: u32) -> Result<bool> {
        state_properties(state)?;
        if !(self.min_y..self.min_y + self.height).contains(&y) {
            bail!("block outside light volume build height");
        }
        let (key, index) = self
            .index(x, y, z)
            .ok_or_else(|| anyhow!("chunk is not loaded in light volume"))?;
        let col = self.columns.get_mut(&key).unwrap();
        let changed = col.states[index] != state;
        col.states[index] = state;
        self.dirty |= changed;
        if changed {
            self.changed_columns.insert((x, z));
        }
        Ok(changed)
    }
    pub fn block(&self, x: i32, y: i32, z: i32) -> Option<u32> {
        let (k, i) = self.index(x, y, z)?;
        Some(self.columns[&k].states[i])
    }
    /// Stable values are unavailable after edits until `relight` completes.
    pub fn light(&self, x: i32, y: i32, z: i32) -> Result<Option<(u8, u8)>> {
        if self.dirty {
            bail!("light volume has unprocessed block changes");
        }
        Ok(self.index(x, y, z).map(|(k, i)| {
            let c = &self.columns[&k];
            (c.sky[i], c.block[i])
        }))
    }
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
    /// Number of vertical block columns rebuilt by the previous update.
    pub fn last_rebuilt_columns(&self) -> usize {
        self.last_rebuilt_columns
    }
    /// Compute stable values. After initialization, changes have at most a
    /// fourteen-block horizontal light influence: level 15 sources attenuate by
    /// at least one per lateral edge. A conservative fifteen-block halo is
    /// recomputed through the entire vertical range, including changed direct
    /// sky sources. Proven unchanged light outside it supplies boundary values.
    /// Java's internal increase/decrease queue order is not reproduced.
    pub fn relight(&mut self) -> Result<()> {
        if !self.dirty {
            self.last_rebuilt_columns = 0;
            return Ok(());
        }
        let mut affected = BTreeSet::new();
        if self.initialized {
            for &(x, z) in &self.changed_columns {
                for dx in -15i32..=15 {
                    for dz in -(15 - dx.abs())..=15 - dx.abs() {
                        let Some(nx) = x.checked_add(dx) else {
                            continue;
                        };
                        let Some(nz) = z.checked_add(dz) else {
                            continue;
                        };
                        if self.columns.contains_key(&(nx >> 4, nz >> 4)) {
                            affected.insert((nx, nz));
                        }
                    }
                }
            }
        }
        let region =
            (self.initialized && affected.len() < self.columns.len() * 128).then_some(&affected);
        self.rebuild(region)?;
        self.changed_columns.clear();
        self.initialized = true;
        self.dirty = false;
        Ok(())
    }
    fn rebuild(&mut self, region: Option<&BTreeSet<(i32, i32)>>) -> Result<()> {
        let selected = |x, z| region.is_none_or(|r| r.contains(&(x, z)));
        let f = facts();
        let mut block_queue = VecDeque::new();
        let mut sky_queue = VecDeque::new();
        self.last_rebuilt_columns = 0;
        for (&(cx, cz), c) in &mut self.columns {
            if region.is_none() {
                c.block.fill(0);
                c.sky.fill(0);
                for (i, &state) in c.states.iter().enumerate() {
                    let emission = f.rows[f.states[state as usize]].emission;
                    if emission > 0 {
                        c.block[i] = emission;
                        block_queue.push_back((
                            cx * 16 + (i & 15) as i32,
                            self.min_y - 16 + (i >> 8) as i32,
                            cz * 16 + ((i >> 4) & 15) as i32,
                        ));
                    }
                }
            }
            for z in 0..16 {
                for x in 0..16 {
                    let gx = cx * 16 + x;
                    let gz = cz * 16 + z;
                    if !selected(gx, gz) {
                        continue;
                    }
                    self.last_rebuilt_columns += 1;
                    if region.is_some() {
                        for y in 0..self.height + 32 {
                            let i = ((y * 16 + z) * 16 + x) as usize;
                            c.block[i] = 0;
                            c.sky[i] = 0;
                            let p = f.rows[f.states[c.states[i] as usize]];
                            if p.emission > 0 {
                                c.block[i] = p.emission;
                                block_queue.push_back((gx, self.min_y - 16 + y, gz));
                            }
                        }
                    }
                    c.lowest_source[(z * 16 + x) as usize] = self.min_y - 16;
                    if !self.sky_enabled {
                        continue;
                    }
                    let mut above = state_properties(0)?;
                    for y in (0..self.height + 32).rev() {
                        let i = ((y * 16 + z) * 16 + x) as usize;
                        let p = f.rows[f.states[c.states[i] as usize]];
                        if p.opacity != 0 || occludes(above, p, 0) {
                            c.lowest_source[(z * 16 + x) as usize] = self.min_y - 16 + y + 1;
                            break;
                        }
                        c.sky[i] = 15;
                        above = p;
                    }
                }
            }
        }
        // Direct sources surrounded by sources cannot improve a value. Queue
        // only bottom sources and lateral source/non-source boundaries.
        if self.sky_enabled {
            for (&(cx, cz), c) in &self.columns {
                for z in 0..16i32 {
                    for x in 0..16i32 {
                        let gx = cx * 16 + x;
                        let gz = cz * 16 + z;
                        if !selected(gx, gz) {
                            continue;
                        }
                        let source_y = c.lowest_source[(z * 16 + x) as usize];
                        let mut end_y = if source_y > self.min_y - 16 {
                            source_y + 1
                        } else {
                            source_y
                        };
                        for (dx, dz) in [(0, -1), (0, 1), (-1, 0), (1, 0)] {
                            let Some(nx) = gx.checked_add(dx) else {
                                continue;
                            };
                            let Some(nz) = gz.checked_add(dz) else {
                                continue;
                            };
                            if let Some(neighbor) = self.columns.get(&(nx >> 4, nz >> 4)) {
                                end_y = end_y.max(
                                    neighbor.lowest_source[((nz & 15) * 16 + (nx & 15)) as usize],
                                );
                            }
                        }
                        for y in source_y..end_y {
                            sky_queue.push_back((gx, y, gz));
                        }
                    }
                }
            }
        }
        if let Some(region) = region {
            let mut boundary = BTreeSet::new();
            for &(x, z) in region {
                for (dx, dz) in [(0, -1), (0, 1), (-1, 0), (1, 0)] {
                    let Some(nx) = x.checked_add(dx) else {
                        continue;
                    };
                    let Some(nz) = z.checked_add(dz) else {
                        continue;
                    };
                    if !region.contains(&(nx, nz)) && self.columns.contains_key(&(nx >> 4, nz >> 4))
                    {
                        boundary.insert((nx, nz));
                    }
                }
            }
            for (x, z) in boundary {
                let c = &self.columns[&(x >> 4, z >> 4)];
                for y in 0..self.height + 32 {
                    let i = ((y * 16 + (z & 15)) * 16 + (x & 15)) as usize;
                    let pos = (x, self.min_y - 16 + y, z);
                    if c.block[i] > 1 {
                        block_queue.push_back(pos);
                    }
                    if c.sky[i] > 1 {
                        sky_queue.push_back(pos);
                    }
                }
            }
        }
        self.propagate(&mut block_queue, false, region);
        self.propagate(&mut sky_queue, true, region);
        Ok(())
    }
    fn propagate(
        &mut self,
        queue: &mut VecDeque<(i32, i32, i32)>,
        sky: bool,
        region: Option<&BTreeSet<(i32, i32)>>,
    ) {
        let f = facts();
        while let Some((x, y, z)) = queue.pop_front() {
            let (key, index) = self.index(x, y, z).unwrap();
            let c = &self.columns[&key];
            let level = if sky { c.sky[index] } else { c.block[index] };
            if level <= 1 {
                continue;
            }
            let source = f.rows[f.states[c.states[index] as usize]];
            for (direction, (dx, dy, dz)) in DIRECTIONS.into_iter().enumerate() {
                let Some(nx) = x.checked_add(dx) else {
                    continue;
                };
                let Some(ny) = y.checked_add(dy) else {
                    continue;
                };
                let Some(nz) = z.checked_add(dz) else {
                    continue;
                };
                if region.is_some_and(|r| !r.contains(&(nx, nz))) {
                    continue;
                }
                let Some((key, index)) = self.index(nx, ny, nz) else {
                    continue;
                };
                let c = self.columns.get_mut(&key).unwrap();
                let dest = f.rows[f.states[c.states[index] as usize]];
                let next = level.saturating_sub(dest.opacity.max(1));
                let old = if sky { c.sky[index] } else { c.block[index] };
                if next <= old || occludes(source, dest, direction) {
                    continue;
                }
                if sky {
                    c.sky[index] = next;
                } else {
                    c.block[index] = next;
                }
                queue.push_back((nx, ny, nz));
            }
        }
    }
    /// Return the network/Anvil nibble order: X, then Z, then Y; even index low.
    pub fn section_light(&self, cx: i32, cz: i32, section_y: i32) -> Result<(Vec<u8>, Vec<u8>)> {
        if self.dirty {
            bail!("light volume has unprocessed block changes");
        }
        let c = self
            .columns
            .get(&(cx, cz))
            .ok_or_else(|| anyhow!("chunk not loaded"))?;
        let offset = section_y
            .checked_mul(16)
            .and_then(|v| v.checked_sub(self.min_y - 16))
            .filter(|v| *v >= 0 && *v + 16 <= self.height + 32)
            .ok_or_else(|| anyhow!("section outside light volume"))? as usize
            * 256;
        let pack = |a: &[u8]| {
            a[offset..offset + 4096]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|p| p[0] | p[1] << 4)
                .collect()
        };
        Ok((pack(&c.sky), pack(&c.block)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_java_stable_light_vectors() {
        #[derive(Deserialize)]
        struct Vectors {
            scenes: Vec<Scene>,
        }
        #[derive(Deserialize)]
        struct Scene {
            #[serde(default = "default_sky_enabled")]
            sky_enabled: bool,
            min_y: i32,
            height: i32,
            chunks: Vec<(i32, i32)>,
            blocks: Vec<(i32, i32, i32, u32)>,
            updates: Vec<Vec<(i32, i32, i32, u32)>>,
            queries: Vec<(i32, i32, i32)>,
            expected: Vec<Vec<(u8, u8)>>,
        }
        fn default_sky_enabled() -> bool {
            true
        }
        let cases: Vectors =
            serde_json::from_str(include_str!("../tools/lighting_vectors.json")).unwrap();
        for (scene_index, scene) in cases.scenes.into_iter().enumerate() {
            let mut volume = LightVolume::new_with_sky(
                scene.min_y,
                scene.height,
                scene.chunks,
                scene.sky_enabled,
            )
            .unwrap();
            for (x, y, z, state) in scene.blocks {
                volume.set_block(x, y, z, state).unwrap();
            }
            for (phase, (edits, expected)) in
                scene.updates.into_iter().zip(scene.expected).enumerate()
            {
                for (x, y, z, state) in edits {
                    volume.set_block(x, y, z, state).unwrap();
                }
                volume.relight().unwrap();
                for (&(x, y, z), expected) in scene.queries.iter().zip(expected) {
                    assert_eq!(
                        volume.light(x, y, z).unwrap(),
                        Some(expected),
                        "scene{scene_index} phase{phase} at{x},{y},{z}"
                    );
                }
            }
        }
    }
    #[test]
    fn all_state_facts_are_bounded() {
        for state in 0..26684 {
            let p = state_properties(state).unwrap();
            assert!(p.emission <= 15 && p.opacity <= 15);
            assert!(p.faces.iter().all(|&v| (v as usize) < facts().face_count));
        }
        assert!(state_properties(26684).is_err());
    }
    #[test]
    fn incremental_radius_retains_unchanged_boundary_sources() {
        let mut volume = LightVolume::new(
            -16,
            32,
            (-1..=1).flat_map(|z| (-1..=1).map(move |x| (x, z))),
        )
        .unwrap();
        let source = (0..26684)
            .find(|&s| state_properties(s).unwrap().emission == 15)
            .unwrap();
        volume.set_block(25, 0, 8, source).unwrap();
        volume.relight().unwrap();
        assert_eq!(volume.last_rebuilt_columns(), 9 * 256);
        volume.set_block(8, 0, 8, source).unwrap();
        volume.relight().unwrap();
        assert_eq!(volume.last_rebuilt_columns(), 481);
        assert_eq!(volume.light(23, 0, 8).unwrap().unwrap().1, 13);
        volume.set_block(25, 0, 8, 0).unwrap();
        volume.relight().unwrap();
        assert!(volume.last_rebuilt_columns() < 481);
        assert_eq!(volume.light(10, 0, 8).unwrap().unwrap().1, 13);
        assert_eq!(volume.light(23, 0, 8).unwrap().unwrap().1, 0);
        volume.set_block(8, 0, 8, 0).unwrap();
        volume.relight().unwrap();
        assert_eq!(volume.light(10, 0, 8).unwrap().unwrap().1, 0);
    }
    #[test]
    fn stable_sources_removal_and_skylight() {
        let mut w = LightVolume::new(0, 32, [(0, 0)]).unwrap();
        w.set_block(8, 8, 8, 1).unwrap();
        w.relight().unwrap();
        assert_eq!(w.light(8, 8, 8).unwrap(), Some((0, 0)));
        assert_eq!(w.light(8, 7, 8).unwrap(), Some((14, 0)));
        let source = (0..26684)
            .find(|&s| state_properties(s).unwrap().emission == 15)
            .unwrap();
        w.set_block(8, 8, 8, source).unwrap();
        assert!(w.light(8, 8, 8).is_err());
        w.relight().unwrap();
        assert_eq!(w.light(8, 8, 8).unwrap().unwrap().1, 15);
        assert_eq!(w.light(9, 8, 8).unwrap().unwrap().1, 14);
        w.set_block(8, 8, 8, 0).unwrap();
        w.relight().unwrap();
        assert_eq!(w.light(8, 8, 8).unwrap(), Some((15, 0)));
    }
    #[test]
    fn cross_chunk_and_closed_missing_boundaries() {
        let mut w = LightVolume::new(-16, 32, [(-1, 0), (0, 0)]).unwrap();
        let source = (0..26684)
            .find(|&s| state_properties(s).unwrap().emission == 15)
            .unwrap();
        w.set_block(-1, 0, 4, source).unwrap();
        w.relight().unwrap();
        assert_eq!(w.light(0, 0, 4).unwrap().unwrap().1, 14);
        assert_eq!(w.light(-17, 0, 4).unwrap(), None);
        let (s, b) = w.section_light(-1, 0, 0).unwrap();
        assert_eq!(s.len(), 2048);
        assert_eq!(b.len(), 2048);
        assert_eq!(b[(4 * 16 + 15) / 2] >> 4, 15);
    }
}
