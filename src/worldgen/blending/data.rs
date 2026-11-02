//! Original BlendingData boundary columns and one-time old-chunk calculation.
//! Region availability/status/neighbor-age collection belongs to the actual
//! region owner. Missing owners must not become empty data or invented biomes.
use crate::{density::DensityPoint, inventory::block_state_name, placement::in_block_tag};
use anyhow::{anyhow, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, sync::OnceLock};

pub const NO_VALUE: f64 = f64::MAX;
pub const COLUMN_COUNT: usize = 16;
const INNER_SHA256: &str = "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01";

/// Direction8.values() encounter order used by the original region collector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Direction8 {
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
    NorthWest,
}
impl Direction8 {
    pub const ALL: [Self; 8] = [
        Self::North,
        Self::NorthEast,
        Self::East,
        Self::SouthEast,
        Self::South,
        Self::SouthWest,
        Self::West,
        Self::NorthWest,
    ];
    pub const fn offset(self) -> (i32, i32) {
        match self {
            Self::North => (0, -1),
            Self::NorthEast => (1, -1),
            Self::East => (1, 0),
            Self::SouthEast => (1, 1),
            Self::South => (0, 1),
            Self::SouthWest => (-1, 1),
            Self::West => (-1, 0),
            Self::NorthWest => (-1, -1),
        }
    }
    pub const fn step_x(self) -> i32 {
        self.offset().0
    }
    pub const fn step_z(self) -> i32 {
        self.offset().1
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Direction8Set(u8);
impl Direction8Set {
    pub const NONE: Self = Self(0);
    pub const ALL: Self = Self(u8::MAX);
    pub const fn new() -> Self {
        Self::NONE
    }
    pub const fn contains(self, direction: Direction8) -> bool {
        self.0 & (1 << direction as u8) != 0
    }
    pub fn insert(&mut self, direction: Direction8) -> bool {
        let old = self.0;
        self.0 |= 1 << direction as u8;
        old != self.0
    }
    pub fn iter(self) -> impl Iterator<Item = Direction8> {
        Direction8::ALL
            .into_iter()
            .filter(move |d| self.contains(*d))
    }
}
impl FromIterator<Direction8> for Direction8Set {
    fn from_iter<T: IntoIterator<Item = Direction8>>(values: T) -> Self {
        let mut result = Self::NONE;
        for value in values {
            result.insert(value);
        }
        result
    }
}
impl<const N: usize> From<[Direction8; N]> for Direction8Set {
    fn from(values: [Direction8; N]) -> Self {
        values.into_iter().collect()
    }
}

/// Actual ChunkAccess operations. Coordinates are LOCAL block/quart X/Z, as
/// passed by original BlendingData, including its collision-shape context.
/// None means an observed absent WG map; unavailable height/material/biome or
/// shape owners must return Err. Collision queries run only after exclusions.
pub trait OldChunkDataHost {
    type Biome: Clone;
    fn block_state(&mut self, local: DensityPoint) -> Result<u32>;
    fn world_surface_wg_first_available(
        &mut self,
        local_x: i32,
        local_z: i32,
    ) -> Result<Option<i32>>;
    fn noise_biome(
        &mut self,
        local_quart_x: i32,
        quart_y: i32,
        local_quart_z: i32,
    ) -> Result<Self::Biome>;
    fn collision_shape_is_empty(&mut self, local: DensityPoint, state: u32) -> Result<bool>;
}

/// Persisted source fields only. Calculated columns and their one-time flag
/// are deliberately absent. Nonfinite doubles remain doubles in this model;
/// a serializer unable to represent them must report its own limitation.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SerializedBlendingData {
    pub min_section: i32,
    pub max_section: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heights: Option<[f64; COLUMN_COUNT]>,
}

#[derive(Debug)]
pub struct BlendingData<B = String> {
    min_y: i32,
    height: i32,
    calculated: bool,
    heights: [f64; COLUMN_COUNT],
    densities: [Option<Vec<f64>>; COLUMN_COUNT],
    biomes: [Option<Vec<B>>; COLUMN_COUNT],
}
impl<B> BlendingData<B> {
    pub fn from_serialized(
        min_section: i32,
        max_section: i32,
        heights: Option<[f64; COLUMN_COUNT]>,
    ) -> Result<Self> {
        // Match sectionToBlockCoord and subtraction with Java integer wrapping.
        let min_y = min_section.wrapping_shl(4);
        let height = max_section.wrapping_shl(4).wrapping_sub(min_y);
        Ok(Self {
            min_y,
            height,
            calculated: false,
            heights: heights.unwrap_or([NO_VALUE; COLUMN_COUNT]),
            densities: std::array::from_fn(|_| None),
            biomes: std::array::from_fn(|_| None),
        })
    }
    /// Source JSON shape: required integer section fields; heights is a lenient
    /// optional double-list. A decoded list with the wrong length still fails.
    pub fn from_json(value: &Value) -> Result<Self> {
        let integer = |key: &str| -> Result<i32> {
            Ok(i32::try_from(value[key].as_i64().with_context(|| {
                format!("BlendingData lacks integer {key}")
            })?)?)
        };
        let heights = match value
            .get("heights")
            .and_then(|v| serde_json::from_value::<Vec<f64>>(v.clone()).ok())
        {
            None => None,
            Some(values) => Some(values.try_into().map_err(|v: Vec<f64>| {
                anyhow!("BlendingData heights length {} differs from 16", v.len())
            })?),
        };
        Self::from_serialized(integer("min_section")?, integer("max_section")?, heights)
    }
    pub fn serialized(&self) -> SerializedBlendingData {
        SerializedBlendingData {
            min_section: self.min_section(),
            max_section: self.max_section(),
            heights: self
                .heights
                .iter()
                .any(|h| *h != NO_VALUE)
                .then_some(self.heights),
        }
    }
    pub fn min_y(&self) -> i32 {
        self.min_y
    }
    pub fn height(&self) -> i32 {
        self.height
    }
    pub fn max_y_exclusive(&self) -> i32 {
        self.min_y.wrapping_add(self.height)
    }
    pub fn min_section(&self) -> i32 {
        self.min_y >> 4
    }
    pub fn max_section(&self) -> i32 {
        self.max_y_exclusive()
            .wrapping_sub(1)
            .wrapping_shr(4)
            .wrapping_add(1)
    }
    pub fn is_calculated(&self) -> bool {
        self.calculated
    }
    pub fn heights(&self) -> &[f64; COLUMN_COUNT] {
        &self.heights
    }
    pub fn density_columns(&self) -> &[Option<Vec<f64>>; COLUMN_COUNT] {
        &self.densities
    }
    pub fn biome_columns(&self) -> &[Option<Vec<B>>; COLUMN_COUNT] {
        &self.biomes
    }
    pub fn bottom_cell_y(&self) -> i32 {
        self.min_section().wrapping_mul(2)
    }
    pub fn column_min_cell_y(&self) -> i32 {
        self.bottom_cell_y().wrapping_add(1)
    }
    fn section_count(&self) -> i32 {
        self.max_section().wrapping_sub(self.min_section())
    }
    fn cell_count(&self) -> i32 {
        self.section_count().wrapping_mul(2)
    }
    fn quart_count(&self) -> i32 {
        self.section_count().wrapping_mul(4)
    }

    pub fn get_height(&self, local_quart_x: i32, _cell_y: i32, local_quart_z: i32) -> f64 {
        column_index(local_quart_x, local_quart_z).map_or(NO_VALUE, |index| self.heights[index])
    }
    pub fn get_density(&self, local_quart_x: i32, cell_y: i32, local_quart_z: i32) -> f64 {
        // The original bottom guard precedes both column and X/Z checks.
        if cell_y == self.bottom_cell_y() {
            return 0.1;
        }
        let Some(index) = column_index(local_quart_x, local_quart_z) else {
            return NO_VALUE;
        };
        let Some(values) = self.densities[index].as_ref() else {
            return NO_VALUE;
        };
        let y = cell_y.wrapping_sub(self.column_min_cell_y());
        if y < 0 || y as usize >= values.len() {
            NO_VALUE
        } else {
            values[y as usize] * 0.1
        }
    }
    pub fn iterate_heights(
        &self,
        origin_quart_x: i32,
        origin_quart_z: i32,
        mut visitor: impl FnMut(i32, i32, f64),
    ) {
        for (index, &height) in self.heights.iter().enumerate() {
            if height != NO_VALUE {
                let (x, z) = column_position(index);
                visitor(
                    origin_quart_x.wrapping_add(x),
                    origin_quart_z.wrapping_add(z),
                    height,
                );
            }
        }
    }
    pub fn iterate_densities(
        &self,
        origin_quart_x: i32,
        origin_quart_z: i32,
        min_cell_y: i32,
        max_cell_y_exclusive: i32,
        mut visitor: impl FnMut(i32, i32, i32, f64),
    ) {
        let minimum = self.column_min_cell_y();
        let first = 0.max(min_cell_y.wrapping_sub(minimum));
        let end = self
            .cell_count()
            .min(max_cell_y_exclusive.wrapping_sub(minimum));
        for (index, column) in self.densities.iter().enumerate() {
            if let Some(column) = column {
                let (x, z) = column_position(index);
                for y in first..end {
                    visitor(
                        origin_quart_x.wrapping_add(x),
                        y.wrapping_add(minimum),
                        origin_quart_z.wrapping_add(z),
                        column[y as usize] * 0.1,
                    );
                }
            }
        }
    }
    pub fn iterate_biomes(
        &self,
        origin_quart_x: i32,
        quart_y: i32,
        origin_quart_z: i32,
        mut visitor: impl FnMut(i32, i32, &B),
    ) {
        let min = self.min_y >> 2;
        if quart_y < min || quart_y >= self.max_y_exclusive() >> 2 {
            return;
        }
        let y = quart_y.wrapping_sub(min) as usize;
        for (index, column) in self.biomes.iter().enumerate() {
            if let Some(column) = column {
                let (x, z) = column_position(index);
                visitor(
                    origin_quart_x.wrapping_add(x),
                    origin_quart_z.wrapping_add(z),
                    &column[y],
                );
            }
        }
    }
}
impl<B: Clone> BlendingData<B> {
    /// One-time source calculation. Region collection must already have queried
    /// all eight actual neighbors, including on later calls to calculated data.
    /// Errors retain earlier source mutations; no rollback is performed.
    pub fn calculate_data(
        &mut self,
        host: &mut impl OldChunkDataHost<Biome = B>,
        new_generation_neighbors: Direction8Set,
    ) -> Result<()> {
        if self.calculated {
            return Ok(());
        }
        use Direction8::*;
        let sides = new_generation_neighbors;
        if sides.contains(North) || sides.contains(West) || sides.contains(NorthWest) {
            self.add_column(inside_index(0, 0), host, 0, 0)?;
        }
        if sides.contains(North) {
            for q in 1..4 {
                self.add_column(inside_index(q, 0), host, q * 4, 0)?;
            }
        }
        if sides.contains(West) {
            for q in 1..4 {
                self.add_column(inside_index(0, q), host, 0, q * 4)?;
            }
        }
        if sides.contains(East) {
            for q in 1..4 {
                self.add_column(outside_index(4, q), host, 15, q * 4)?;
            }
        }
        if sides.contains(South) {
            for q in 0..4 {
                self.add_column(outside_index(q, 4), host, q * 4, 15)?;
            }
        }
        if sides.contains(East) && sides.contains(NorthEast) {
            self.add_column(outside_index(4, 0), host, 15, 0)?;
        }
        if sides.contains(East) && sides.contains(South) && sides.contains(SouthEast) {
            self.add_column(outside_index(4, 4), host, 15, 15)?;
        }
        self.calculated = true;
        Ok(())
    }
    fn add_column(
        &mut self,
        index: usize,
        host: &mut impl OldChunkDataHost<Biome = B>,
        x: i32,
        z: i32,
    ) -> Result<()> {
        if self.heights[index] == NO_VALUE {
            self.heights[index] = self.height_at(host, x, z)? as f64;
        }
        self.densities[index] =
            Some(self.density_column(host, x, z, java_floor(self.heights[index]))?);
        self.biomes[index] = Some(self.biome_column(host, x, z)?);
        Ok(())
    }
    fn height_at(
        &self,
        host: &mut impl OldChunkDataHost<Biome = B>,
        x: i32,
        z: i32,
    ) -> Result<i32> {
        let mut y = host
            .world_surface_wg_first_available(x, z)?
            .map_or(self.max_y_exclusive(), |h| h.min(self.max_y_exclusive()));
        while y > self.min_y {
            y = y.wrapping_sub(1);
            let state = host.block_state(DensityPoint::new(x, y, z))?;
            let name = block_state_name(state)
                .context("old surface sample has unknown canonical state")?;
            if matches!(
                name,
                "podzol"
                    | "gravel"
                    | "grass_block"
                    | "stone"
                    | "coarse_dirt"
                    | "sand"
                    | "red_sand"
                    | "mycelium"
                    | "snow_block"
                    | "terracotta"
                    | "dirt"
            ) {
                return Ok(y);
            }
        }
        Ok(self.min_y)
    }
    fn read_one(
        &self,
        host: &mut impl OldChunkDataHost<Biome = B>,
        point: &mut DensityPoint,
    ) -> Result<f64> {
        point.y = point.y.wrapping_sub(1);
        Ok(if is_ground(host, *point)? { 1.0 } else { -1.0 })
    }
    fn read_seven(
        &self,
        host: &mut impl OldChunkDataHost<Biome = B>,
        point: &mut DensityPoint,
    ) -> Result<f64> {
        let mut sum = 0.0;
        for _ in 0..7 {
            sum += self.read_one(host, point)?;
        }
        Ok(sum)
    }
    fn density_column(
        &self,
        host: &mut impl OldChunkDataHost<Biome = B>,
        x: i32,
        z: i32,
        surface_height: i32,
    ) -> Result<Vec<f64>> {
        let count = usize::try_from(self.cell_count())
            .context("negative source old-generation density-column length")?;
        let mut result = Vec::new();
        result.try_reserve_exact(count)?;
        result.resize(count, -1.0);
        let mut point = DensityPoint::new(x, self.max_y_exclusive(), z);
        let mut previous = self.read_seven(host, &mut point)?;
        let mut index = (count as i64) - 2;
        while index >= 0 {
            let middle = self.read_one(host, &mut point)?;
            let below = self.read_seven(host, &mut point)?;
            result[index as usize] = (previous + middle + below) / 15.0;
            previous = below;
            index -= 1;
        }
        let index = surface_height
            .div_euclid(8)
            .wrapping_sub(self.column_min_cell_y());
        if index >= 0 && (index as i64) < (count as i64) - 1 {
            let fraction = ((surface_height as f64 + 0.5) % 8.0) / 8.0;
            let ratio = (1.0 - fraction) / fraction;
            let scale = if ratio.is_nan() {
                ratio
            } else {
                ratio.max(1.0)
            } * 0.25;
            result[index as usize + 1] = -ratio / scale;
            result[index as usize] = 1.0 / scale;
        }
        Ok(result)
    }
    fn biome_column(
        &self,
        host: &mut impl OldChunkDataHost<Biome = B>,
        x: i32,
        z: i32,
    ) -> Result<Vec<B>> {
        let count = usize::try_from(self.quart_count())
            .context("negative source old-generation biome-column length")?;
        let mut result = Vec::new();
        result.try_reserve_exact(count)?;
        for y in 0..count {
            result.push(host.noise_biome(
                x >> 2,
                (y as i32).wrapping_add(self.min_y >> 2),
                z >> 2,
            )?);
        }
        Ok(result)
    }
}

fn inside_index(x: i32, z: i32) -> usize {
    3_i32.wrapping_sub(x).wrapping_add(z) as usize
}
fn outside_index(x: i32, z: i32) -> usize {
    7_i32.wrapping_add(x).wrapping_add(4).wrapping_sub(z) as usize
}
fn column_index(x: i32, z: i32) -> Option<usize> {
    if x == 4 || z == 4 {
        Some(outside_index(x, z))
    } else if x == 0 || z == 0 {
        Some(inside_index(x, z))
    } else {
        None
    }
}
fn column_position(index: usize) -> (i32, i32) {
    let i = index as i32;
    if i < 7 {
        ((3 - i).max(0), (i - 3).max(0))
    } else {
        let j = i - 7;
        (4 - (4 - j).max(0), 4 - (j - 4).max(0))
    }
}
fn java_floor(value: f64) -> i32 {
    let integer = value as i32;
    if value < (integer as f64) {
        integer.wrapping_sub(1)
    } else {
        integer
    }
}
fn is_ground<B: Clone>(
    host: &mut impl OldChunkDataHost<Biome = B>,
    point: DensityPoint,
) -> Result<bool> {
    let state = host.block_state(point)?;
    let name = block_state_name(state).context("old density sample has unknown canonical state")?;
    if matches!(name, "air" | "cave_air" | "void_air") {
        return Ok(false);
    }
    validate_ground_tags()?;
    if in_block_tag(state, "minecraft:leaves")
        || in_block_tag(state, "minecraft:logs")
        || matches!(name, "brown_mushroom_block" | "red_mushroom_block")
    {
        return Ok(false);
    }
    Ok(!host.collision_shape_is_empty(point, state)?)
}
fn validate_ground_tags() -> Result<()> {
    #[derive(Deserialize)]
    struct Tags {
        minecraft: String,
        jar_sha256: String,
        tags: BTreeMap<String, Vec<u32>>,
    }
    static VALID: OnceLock<std::result::Result<(), String>> = OnceLock::new();
    VALID
        .get_or_init(|| {
            let check = || -> Result<()> {
                let data: Tags = serde_json::from_str(include_str!(
                    "../../../fixtures/placement-block-tags-1.21.json"
                ))?;
                ensure!(
                    data.minecraft == "1.21" && data.jar_sha256 == INNER_SHA256,
                    "old density tag source identity differs"
                );
                for tag in ["minecraft:leaves", "minecraft:logs"] {
                    let members = data
                        .tags
                        .get(tag)
                        .context("missing old density exclusion tag")?;
                    ensure!(!members.is_empty(), "empty old density exclusion tag");
                    for state in members {
                        block_state_name(*state)
                            .context("old density tag has unknown canonical state")?;
                    }
                }
                Ok(())
            };
            check().map_err(|e| format!("{e:#}"))
        })
        .as_ref()
        .map(|_| ())
        .map_err(|e| anyhow!("invalid old density tags: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::bail;
    use serde_json::json;

    struct Host {
        queries: Vec<String>,
        saved_only: bool,
        fail_biome: bool,
    }
    impl OldChunkDataHost for Host {
        type Biome = String;
        fn block_state(&mut self, p: DensityPoint) -> Result<u32> {
            self.queries.push(format!("block:{},{},{}", p.x, p.y, p.z));
            Ok(if p.y <= 11 { 1 } else { 0 })
        }
        fn world_surface_wg_first_available(&mut self, x: i32, z: i32) -> Result<Option<i32>> {
            self.queries.push(format!("height:{x},{z}"));
            if self.saved_only {
                bail!("source must retain saved height");
            }
            Ok(None)
        }
        fn noise_biome(&mut self, x: i32, y: i32, z: i32) -> Result<String> {
            self.queries.push(format!("biome:{x},{y},{z}"));
            if self.fail_biome {
                bail!("actual old biome owner unavailable");
            }
            Ok(format!("original-holder-{x}-{y}-{z}"))
        }
        fn collision_shape_is_empty(&mut self, p: DensityPoint, _state: u32) -> Result<bool> {
            self.queries.push(format!("shape:{},{},{}", p.x, p.y, p.z));
            Ok(false)
        }
    }
    fn host() -> Host {
        Host {
            queries: Vec::new(),
            saved_only: false,
            fail_biome: false,
        }
    }

    #[test]
    fn persisted_columns_keep_original_order_and_lenient_optional_boundary() -> Result<()> {
        let heights = std::array::from_fn(|i| i as f64 + 0.25);
        let data = BlendingData::<String>::from_serialized(-1, 1, Some(heights))?;
        assert!(!data.is_calculated());
        let mut samples = Vec::new();
        data.iterate_heights(-8, 12, |x, z, h| samples.push((x, z, h)));
        let positions = [
            (3, 0),
            (2, 0),
            (1, 0),
            (0, 0),
            (0, 1),
            (0, 2),
            (0, 3),
            (0, 4),
            (1, 4),
            (2, 4),
            (3, 4),
            (4, 4),
            (4, 3),
            (4, 2),
            (4, 1),
            (4, 0),
        ];
        assert_eq!(
            samples,
            positions
                .into_iter()
                .enumerate()
                .map(|(i, (x, z))| (-8 + x, 12 + z, heights[i]))
                .collect::<Vec<_>>()
        );
        assert_eq!(data.get_height(2, 100, 2), NO_VALUE);
        assert_eq!(data.get_density(2, -2, 2), 0.1); // bottom guard before interior/column test
        assert_eq!(data.get_density(2, -1, 2), NO_VALUE);
        let absent = BlendingData::<String>::from_json(
            &json!({"min_section":0,"max_section":16,"heights":"not a double list"}),
        )?;
        assert!(absent.serialized().heights.is_none());
        assert!(BlendingData::<String>::from_json(
            &json!({"min_section":0,"max_section":16,"heights":[1.0]})
        )
        .is_err());
        let mut special = [NO_VALUE; COLUMN_COUNT];
        special[2] = f64::NAN;
        assert!(
            BlendingData::<String>::from_serialized(0, 16, Some(special))?
                .serialized()
                .heights
                .unwrap()[2]
                .is_nan()
        );
        Ok(())
    }

    #[test]
    fn calculation_keeps_saved_height_and_one_time_owner_lifecycle() -> Result<()> {
        let mut data = BlendingData::from_serialized(0, 1, Some([42.5; COLUMN_COUNT]))?;
        let mut source = host();
        source.saved_only = true;
        data.calculate_data(&mut source, Direction8Set::from([Direction8::North]))?;
        assert!(data.is_calculated());
        assert_eq!(data.heights(), &[42.5; COLUMN_COUNT]);
        assert!(source.queries.iter().all(|q| !q.starts_with("height:")));
        assert_eq!(
            data.density_columns()
                .iter()
                .filter(|v| v.is_some())
                .count(),
            4
        );
        assert_eq!(
            data.biome_columns().iter().filter(|v| v.is_some()).count(),
            4
        );
        let biomes = source
            .queries
            .iter()
            .filter(|q| q.starts_with("biome:"))
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(
            &biomes[..4],
            ["biome:0,0,0", "biome:0,1,0", "biome:0,2,0", "biome:0,3,0"]
        );
        let before = source.queries.len();
        data.calculate_data(&mut source, Direction8Set::ALL)?;
        assert_eq!(source.queries.len(), before); // region's 8 age reads are outside this method
        Ok(())
    }

    #[test]
    fn failed_real_biome_dependency_retains_source_partial_mutations() -> Result<()> {
        let mut data = BlendingData::from_serialized(0, 1, None)?;
        let mut source = host();
        source.fail_biome = true;
        assert!(data
            .calculate_data(&mut source, Direction8Set::from([Direction8::North]))
            .is_err());
        assert!(!data.is_calculated());
        assert_eq!(data.get_height(0, 0, 0), 11.0);
        assert!(data.density_columns()[3].is_some());
        assert!(data.biome_columns()[3].is_none());
        assert_eq!(
            source.queries.first().map(String::as_str),
            Some("height:0,0")
        );
        assert_eq!(
            source.queries.last().map(String::as_str),
            Some("biome:0,0,0")
        );
        Ok(())
    }

    #[test]
    fn no_selected_edge_is_calculated_without_fabricating_live_columns() -> Result<()> {
        let mut data = BlendingData::from_serialized(-1, 1, None)?;
        let mut source = host();
        data.calculate_data(&mut source, Direction8Set::NONE)?;
        assert!(data.is_calculated());
        assert!(source.queries.is_empty());
        assert!(data.density_columns().iter().all(Option::is_none));
        assert!(data.biome_columns().iter().all(Option::is_none));
        assert!(data.serialized().heights.is_none());
        assert_eq!(
            Direction8::ALL.map(Direction8::offset),
            [
                (0, -1),
                (1, -1),
                (1, 0),
                (1, 1),
                (0, 1),
                (-1, 1),
                (-1, 0),
                (-1, -1)
            ]
        );
        Ok(())
    }
}
