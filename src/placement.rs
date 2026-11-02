//! Block-specific placement-state functions, pinned to vanilla 1.21 bytecode.
//! This module selects states and clicked-block replacement; world callbacks
//! (survival checks, neighbour updates, block entities and item effects) remain
//! the caller's responsibility. Unsupported functions are reported explicitly.
use crate::{
    geometry::{horizontal_look_direction, ordered_look_directions, Axis, Direction, Vec3},
    inventory::{block_state_property, block_state_with_properties},
};
use anyhow::{anyhow, bail, ensure, Result};
use serde::Deserialize;
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Deserialize)]
struct Catalog {
    minecraft: String,
    jar_sha256: String,
    blocks: Vec<Block>,
    items: Vec<ItemPlacement>,
    fluids: Vec<u8>,
    connections: Vec<u8>,
    wall_cover: Vec<u8>,
    solid_flags: Vec<u8>,
    center_support: Vec<u8>,
    rigid_support: Vec<u8>,
    collision_faces: Vec<u8>,
    collision_full_faces: Vec<u8>,
}
#[derive(Deserialize)]
struct Block {
    name: String,
    class: String,
    instrument: String,
    instrument_above: bool,
    placement_class: String,
    survival_class: String,
    may_place_class: String,
    placed_by_class: String,
    on_place_class: String,
    block_entity_type: Option<String>,
    falling_delay: Option<i32>,
    body_head_item: Option<i32>,
    hardened_state: Option<u32>,
    replacement_class: String,
    replaceable: bool,
    item: i32,
    min: u32,
    max: u32,
    default: u32,
}
#[derive(Deserialize)]
struct ItemPlacement {
    id: i32,
    class: String,
    primary_state: u32,
    wall_state: Option<u32>,
    attachment: Option<usize>,
    context_class: String,
    placement_class: String,
    place_block_class: String,
}
fn item_placement(id: i32) -> Option<&'static ItemPlacement> {
    let items = &catalog().items;
    let i = items.binary_search_by_key(&id, |item| item.id).ok()?;
    Some(&items[i])
}
fn block_named(name: &str) -> Result<u32> {
    catalog()
        .blocks
        .iter()
        .find(|b| b.name == name)
        .map(|b| b.default)
        .ok_or_else(|| anyhow!("unknown block name"))
}
fn catalog() -> &'static Catalog {
    static DATA: OnceLock<Catalog> = OnceLock::new();
    DATA.get_or_init(|| {
        let c: Catalog = serde_json::from_str(include_str!("../fixtures/placement-1.21.json"))
            .expect("placement catalog");
        assert_eq!(c.minecraft, "1.21");
        assert_eq!(
            c.jar_sha256,
            "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"
        );
        assert_eq!(c.blocks.len(), 1060);
        assert_eq!(c.fluids.len(), 26684);
        assert_eq!(c.connections.len(), 26684);
        assert_eq!(c.wall_cover.len(), 26684);
        c
    })
}
fn block(state: u32) -> Result<&'static Block> {
    let blocks = &catalog().blocks;
    let index = blocks
        .partition_point(|b| b.min <= state)
        .checked_sub(1)
        .ok_or_else(|| anyhow!("unknown block state"))?;
    let b = &blocks[index];
    ensure!(state <= b.max, "unknown block state");
    Ok(b)
}

/// Opaque identity of the actual placement catalogue already used by placement.
/// Equal-looking catalogue bytes are not a replacement for this retained owner.
#[derive(Clone, Copy)]
pub struct CanonicalPlacementCatalogIdentity {
    catalog: &'static Catalog,
}
impl PartialEq for CanonicalPlacementCatalogIdentity {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.catalog, other.catalog)
    }
}
impl Eq for CanonicalPlacementCatalogIdentity {}
impl std::fmt::Debug for CanonicalPlacementCatalogIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("CanonicalPlacementCatalogIdentity")
            .field("minecraft", &self.catalog.minecraft)
            .finish_non_exhaustive()
    }
}

/// Read-only values borrowed from one actual canonical placement row.
/// This source catalogue stores state ranges, not raw Block registry IDs.
/// `raw_registry_id` is therefore explicitly absent, never a row ordinal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CanonicalPlacementBlock<'a> {
    pub key: &'a str,
    pub minimum_state: u32,
    pub maximum_state: u32,
    pub default_state: u32,
    pub raw_registry_id: Option<i32>,
}
impl<'a> CanonicalPlacementBlock<'a> {
    fn borrowed(block: &'a Block) -> Self {
        Self {
            key: &block.name,
            minimum_state: block.min,
            maximum_state: block.max,
            default_state: block.default,
            raw_registry_id: None,
        }
    }
}

/// Borrowed access to the SAME immutable OnceLock catalogue as `block` and all
/// normal placement operations. Construction remains private; no parse, cache,
/// tag fallback, registry allocation or identity counter is introduced here.
#[derive(Clone, Copy)]
pub struct CanonicalPlacementCatalog {
    catalog: &'static Catalog,
}
impl CanonicalPlacementCatalog {
    pub fn identity(self) -> CanonicalPlacementCatalogIdentity {
        CanonicalPlacementCatalogIdentity { catalog: self.catalog }
    }
    /// Actual retained provenance. Protocol/mappings IDs are not stored by
    /// this catalogue and must be supplied by their genuine separate owner.
    pub fn minecraft_version(self) -> &'static str {
        &self.catalog.minecraft
    }
    pub fn original_inner_sha256(self) -> &'static str {
        &self.catalog.jar_sha256
    }
    pub fn state_count(self) -> usize {
        self.catalog.fluids.len()
    }
    pub fn blocks(self) -> impl ExactSizeIterator<Item = CanonicalPlacementBlock<'static>> {
        self.catalog.blocks.iter().map(CanonicalPlacementBlock::borrowed)
    }
    pub fn state_definition(self, state: u32) -> Result<CanonicalPlacementBlock<'static>> {
        let index = self.catalog.blocks.partition_point(|block| block.min <= state)
            .checked_sub(1).ok_or_else(|| anyhow!("unknown block state"))?;
        let block = &self.catalog.blocks[index];
        ensure!(state <= block.max, "unknown block state");
        Ok(CanonicalPlacementBlock::borrowed(block))
    }
}
impl std::fmt::Debug for CanonicalPlacementCatalog {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("CanonicalPlacementCatalog")
            .field("identity", &self.identity())
            .field("blocks", &self.catalog.blocks.len())
            .finish_non_exhaustive()
    }
}

pub fn canonical_placement_catalog() -> CanonicalPlacementCatalog {
    CanonicalPlacementCatalog { catalog: catalog() }
}

#[derive(Clone, Copy, Debug)]
pub struct PlacementContext {
    /// The final candidate block, after replacement/adjacent-target selection.
    pub position: (i32, i32, i32),
    pub clicked_face: Direction,
    /// Original click location in world coordinates, including when the final
    /// candidate is adjacent to the clicked block.
    pub hit_location: Vec3,
    pub horizontal_direction: Direction,
    pub nearest_looking_direction: Direction,
    pub nearest_vertical_direction: Direction,
    pub secondary_use: bool,
    pub replacing_clicked: bool,
    pub rotation: f32,
    pub ordered_directions: [Direction; 6],
}
impl PlacementContext {
    /// BlockPlaceContext.getNearestLookingDirections prioritises the opposite
    /// clicked face when the final target is adjacent rather than replaced.
    pub fn looking_directions(&self) -> [Direction; 6] {
        let mut directions = self.ordered_directions;
        if !self.replacing_clicked {
            let first = self.clicked_face.opposite();
            if let Some(i) = directions.iter().position(|d| *d == first) {
                directions[..=i].rotate_right(1);
            }
        }
        directions
    }
    pub fn new(
        position: (i32, i32, i32),
        clicked_face: Direction,
        hit_location: Vec3,
        pitch: f32,
        yaw: f32,
    ) -> Self {
        Self {
            position,
            clicked_face,
            hit_location,
            horizontal_direction: horizontal_look_direction(yaw),
            nearest_looking_direction: ordered_look_directions(pitch, yaw)[0],
            nearest_vertical_direction: if pitch < 0.0 {
                Direction::Up
            } else {
                Direction::Down
            },
            secondary_use: false,
            replacing_clicked: false,
            rotation: yaw,
            ordered_directions: ordered_look_directions(pitch, yaw),
        }
    }
}
pub type BlockPosition = (i32, i32, i32);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupportType {
    Full,
    Center,
    Rigid,
}
/// Actual world facts needed by placement/survival functions. Unprovided
/// dynamic facts fail explicitly; there is no assumed redstone/sky/border value.
pub trait PlacementWorld {
    fn block(&mut self, position: BlockPosition) -> Result<u32>;
    fn max_build_height(&mut self) -> Result<i32> {
        bail!("maximum build height fact required")
    }
    fn min_build_height(&mut self) -> Result<i32> {
        bail!("minimum build height fact required")
    }
    fn has_neighbor_signal(&mut self, _position: BlockPosition) -> Result<bool> {
        bail!("redstone neighbour signal fact required")
    }
    fn inside_world_border(&mut self, _position: BlockPosition) -> Result<bool> {
        bail!("world border fact required")
    }
    fn raw_brightness(&mut self, _position: BlockPosition, _sky_darkening: i32) -> Result<i32> {
        bail!("raw brightness fact required")
    }
    fn can_see_sky(&mut self, _position: BlockPosition) -> Result<bool> {
        bail!("sky visibility fact required")
    }
    fn support(
        &mut self,
        position: BlockPosition,
        direction: Direction,
        kind: SupportType,
    ) -> Result<bool> {
        let state = self.block(position)?;
        let b = block(state)?;
        ensure!(
            !matches!(b.class.as_str(), "dmy" | "dsu"),
            "animated block-entity support shape required"
        );
        let bits = match kind {
            SupportType::Full => catalog().connections[state as usize],
            SupportType::Center => catalog().center_support[state as usize],
            SupportType::Rigid => catalog().rigid_support[state as usize],
        };
        Ok(bits & (1 << direction as u8) != 0)
    }
    fn collision_face_nonempty(
        &mut self,
        position: BlockPosition,
        direction: Direction,
    ) -> Result<bool> {
        let state = self.block(position)?;
        let b = block(state)?;
        ensure!(
            !matches!(b.class.as_str(), "dmy" | "dsu"),
            "animated block-entity collision shape required"
        );
        Ok(catalog().collision_faces[state as usize] & (1 << direction as u8) != 0)
    }
    fn collision_face_full(
        &mut self,
        position: BlockPosition,
        direction: Direction,
    ) -> Result<bool> {
        let state = self.block(position)?;
        ensure!(
            !matches!(block(state)?.class.as_str(), "dmy" | "dsu"),
            "animated block-entity collision shape required"
        );
        Ok(catalog().collision_full_faces[state as usize] & (1 << direction as u8) != 0)
    }
    fn collision_full(&mut self, position: BlockPosition) -> Result<bool> {
        let state = self.block(position)?;
        let b = block(state)?;
        ensure!(
            !matches!(b.class.as_str(), "dmy" | "dsu"),
            "animated block-entity collision shape required"
        );
        Ok(catalog().solid_flags[state as usize] & 4 != 0)
    }
}
impl<F: FnMut(BlockPosition) -> Result<u32>> PlacementWorld for F {
    fn block(&mut self, position: BlockPosition) -> Result<u32> {
        self(position)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurvivalResult {
    Supported(bool),
    Unsupported(&'static str),
}
fn state_flag(state: u32, flag: u8) -> Result<bool> {
    Ok(catalog()
        .solid_flags
        .get(state as usize)
        .ok_or_else(|| anyhow!("unknown block state"))?
        & flag
        != 0)
}
fn support_center(
    world: &mut impl PlacementWorld,
    position: BlockPosition,
    direction: Direction,
) -> Result<bool> {
    if direction == Direction::Down
        && in_block_tag(world.block(position)?, "minecraft:unstable_bottom_center")
    {
        return Ok(false);
    }
    world.support(position, direction, SupportType::Center)
}
fn plant_soil_rule(
    b: &Block,
    soil: u32,
    below: BlockPosition,
    world: &mut impl PlacementWorld,
) -> Result<Option<bool>> {
    let name = &block(soil)?.name;
    let generic = in_block_tag(soil, "minecraft:dirt") || name == "minecraft:farmland";
    Ok(Some(match b.may_place_class.as_str() {
        "dgh" => generic,
        "dez" | "dnv" | "dht" => name == "minecraft:farmland",
        "dlf" => name == "minecraft:soul_sand",
        "dhx" => in_block_tag(soil, "minecraft:dead_bush_may_place_on"),
        "dpr" => {
            generic
                || matches!(
                    name.as_str(),
                    "minecraft:netherrack" | "minecraft:soul_sand" | "minecraft:soul_soil"
                )
        }
        "dje" => {
            generic
                || in_block_tag(soil, "minecraft:nylium")
                || matches!(name.as_str(), "minecraft:mycelium" | "minecraft:soul_soil")
        }
        "dmk" | "dld" => {
            generic || in_block_tag(soil, "minecraft:nylium") || name == "minecraft:soul_soil"
        }
        "dfa" => generic || name == "minecraft:clay",
        "dkt" => generic || name == "minecraft:clay",
        "dmx" => {
            world.support(below, Direction::Up, SupportType::Full)?
                && name != "minecraft:magma_block"
        }
        "dpc" => {
            (is_source_water(soil)
                || matches!(name.as_str(), "minecraft:ice" | "minecraft:frosted_ice"))
                && catalog().fluids[world.block(neighbour(below, Direction::Up))? as usize] & 8 == 0
        }
        "dmw" => {
            world.collision_face_nonempty(below, Direction::Up)?
                || world.support(below, Direction::Up, SupportType::Full)?
        }
        _ => return Ok(None),
    }))
}
/// Pure vanilla canSurvive predicate for implemented block families.
pub fn can_survive(
    state: u32,
    position: BlockPosition,
    world: &mut impl PlacementWorld,
) -> Result<SurvivalResult> {
    let b = block(state)?;
    let below = neighbour(position, Direction::Down);
    let answer = match b.survival_class.as_str() {
        "dtb" | "djk" => true,
        "dlq" => {
            let direction = Direction::from_name(
                &block_state_property(state, "vertical_direction")
                    .ok_or_else(|| anyhow!("dripstone direction missing"))?,
            )
            .ok_or_else(|| anyhow!("invalid dripstone direction"))?;
            dripstone_support(world, position, direction)?
        }
        "dky" => {
            let mut attached = false;
            for direction in [
                Direction::Down,
                Direction::Up,
                Direction::North,
                Direction::South,
                Direction::West,
                Direction::East,
            ] {
                if block_state_property(state, direction.name()).as_deref() != Some("true") {
                    continue;
                }
                if !multiface_support(world, position, direction)? {
                    return Ok(SurvivalResult::Supported(false));
                }
                attached = true;
            }
            attached
        }
        "dov" => {
            let mut attached = false;
            if block_state_property(state, "up").as_deref() == Some("true") {
                let above = neighbour(position, Direction::Up);
                attached = world.support(above, Direction::Up, SupportType::Full)?
                    || world.collision_face_full(above, Direction::Up)?;
            }
            for direction in [
                Direction::North,
                Direction::South,
                Direction::West,
                Direction::East,
            ] {
                if block_state_property(state, direction.name()).as_deref() == Some("true")
                    && vine_support(world, position, direction)?
                {
                    attached = true
                }
            }
            attached
        }
        "dgx" => world.support(
            neighbour(position, Direction::Up),
            Direction::Down,
            SupportType::Center,
        )?,
        "dhf" => in_block_tag(
            world.block(neighbour(
                position,
                facing(state).ok_or_else(|| anyhow!("cocoa facing required"))?,
            ))?,
            "minecraft:jungle_logs",
        ),
        "dfu" => {
            let direction = match block_state_property(state, "attachment").as_deref() {
                Some("floor") => Direction::Down,
                Some("ceiling") => Direction::Up,
                _ => facing(state).ok_or_else(|| anyhow!("bell facing required"))?,
            };
            if direction == Direction::Up {
                support_center(world, neighbour(position, direction), Direction::Down)?
            } else {
                world.support(
                    neighbour(position, direction),
                    direction.opposite(),
                    SupportType::Full,
                )?
            }
        }
        "dfd" | "dgo" | "dnu" | "dgk" => state_flag(world.block(below)?, 1)?,
        "dgp" => !state_flag(world.block(below)?, 8)?,
        "dfi" => world.support(below, Direction::Up, SupportType::Full)?,
        "dfj" | "dke" | "dpb" | "dex" | "dme" | "doq" => {
            let facing = facing(state).ok_or_else(|| anyhow!("missing support facing"))?;
            world.support(
                neighbour(position, facing.opposite()),
                facing,
                SupportType::Full,
            )?
        }
        "dow" | "doz" => {
            let f = facing(state).ok_or_else(|| anyhow!("missing support facing"))?;
            state_flag(world.block(neighbour(position, f.opposite()))?, 1)?
        }
        "dfo" | "dgn" => support_center(world, below, Direction::Up)?,
        "dfm" => {
            world.support(below, Direction::Up, SupportType::Rigid)?
                || support_center(world, below, Direction::Up)?
        }
        "dfn" | "dia" => world.support(below, Direction::Up, SupportType::Rigid)?,
        "dfb" | "dfc" => in_block_tag(world.block(below)?, "minecraft:bamboo_plantable_on"),
        "djp" => world.support(
            neighbour(position, Direction::Up),
            Direction::Down,
            SupportType::Full,
        )?,
        "dnk" => in_block_tag(world.block(below)?, "minecraft:soul_fire_base_blocks"),
        "die" => {
            if block_state_property(state, "half").as_deref() == Some("lower") {
                world.support(below, Direction::Up, SupportType::Full)?
            } else {
                block(world.block(below)?)?.min == b.min
            }
        }
        "dic" | "div" => {
            let above = world.block(neighbour(position, Direction::Up))?;
            !state_flag(above, 1)?
                || block(above)?.class == "dix"
                || (b.survival_class == "div" && block(above)?.class == "dsu")
        }
        "dis" => {
            let outward = match block_state_property(state, "face").as_deref() {
                Some("floor") => Direction::Up,
                Some("ceiling") => Direction::Down,
                _ => facing(state).ok_or_else(|| anyhow!("missing attachment facing"))?,
            };
            world.support(
                neighbour(position, outward.opposite()),
                outward,
                SupportType::Full,
            )?
        }
        "dkf" => {
            let hanging = block_state_property(state, "hanging").as_deref() == Some("true");
            support_center(
                world,
                neighbour(
                    position,
                    if hanging {
                        Direction::Up
                    } else {
                        Direction::Down
                    },
                ),
                if hanging {
                    Direction::Down
                } else {
                    Direction::Up
                },
            )?
        }
        "dnp" => {
            support_center(world, neighbour(position, Direction::Up), Direction::Down)?
                && !is_water(world.block(position)?)
        }
        "dgh" | "dmw" => {
            let soil = world.block(below)?;
            match plant_soil_rule(b, soil, below, world)? {
                Some(value) => value,
                None => return Ok(SurvivalResult::Unsupported(&b.may_place_class)),
            }
        }
        "dig" => {
            if block_state_property(state, "half").as_deref() == Some("upper") {
                let lower = world.block(below)?;
                block(lower)?.min == b.min
                    && block_state_property(lower, "half").as_deref() == Some("lower")
            } else {
                let soil = world.block(below)?;
                match plant_soil_rule(b, soil, below, world)? {
                    Some(value) => value,
                    None => return Ok(SurvivalResult::Unsupported(&b.may_place_class)),
                }
            }
        }
        "dht" => {
            world.raw_brightness(position, 0)? >= 8
                && block(world.block(below)?)?.name == "minecraft:farmland"
        }
        _ => return Ok(SurvivalResult::Unsupported(&b.survival_class)),
    };
    Ok(SurvivalResult::Supported(answer))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlacementResult {
    State(u32),
    Rejected,
    Unsupported(&'static str),
}
/// Exact original fluid type equality, distinct from all flowing-water states.
pub fn is_source_water(state: u32) -> bool {
    catalog()
        .fluids
        .get(state as usize)
        .is_some_and(|flags| flags & 1 != 0)
}
pub fn is_water(state: u32) -> bool {
    catalog()
        .fluids
        .get(state as usize)
        .is_some_and(|flags| flags & 2 != 0)
}
/// Membership in the original recursively resolved vanilla block tags.
pub fn in_block_tag(state: u32, tag: &str) -> bool {
    #[derive(Deserialize)]
    struct Tags {
        tags: BTreeMap<String, Vec<u32>>,
    }
    static DATA: OnceLock<Tags> = OnceLock::new();
    let data = DATA.get_or_init(|| {
        serde_json::from_str(include_str!("../fixtures/placement-block-tags-1.21.json"))
            .expect("block tag data")
    });
    block(state)
        .ok()
        .and_then(|b| data.tags.get(tag).map(|states| states.contains(&b.default)))
        .unwrap_or(false)
}
fn rotation_segment(angle: f32) -> u32 {
    let scaled = angle * (16.0_f32 / 360.0_f32);
    ((scaled as f64 + 0.5).floor() as i32 & 15) as u32
}
fn int_property(state: u32, name: &str) -> Result<u32> {
    block_state_property(state, name)
        .ok_or_else(|| anyhow!("missing block property {name}"))?
        .parse()
        .map_err(Into::into)
}
fn set_int(state: u32, name: &str, value: u32) -> Result<u32> {
    set(state, &[(name, &value.to_string())])
}
fn set(state: u32, values: &[(&str, &str)]) -> Result<u32> {
    block_state_with_properties(
        state,
        &values
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<BTreeMap<_, _>>(),
    )
}
fn water(state: u32, wet: bool) -> Result<u32> {
    set(
        state,
        &[("waterlogged", if wet { "true" } else { "false" })],
    )
}
fn neighbour(position: (i32, i32, i32), direction: Direction) -> (i32, i32, i32) {
    let d = direction.step();
    (
        position.0.wrapping_add(d.0),
        position.1.wrapping_add(d.1),
        position.2.wrapping_add(d.2),
    )
}
fn stair(state: u32) -> bool {
    block(state).is_ok_and(|b| b.placement_class == "dnt")
}
fn same_stair_half(a: u32, b: u32) -> bool {
    block_state_property(a, "half") == block_state_property(b, "half")
}
fn facing(state: u32) -> Option<Direction> {
    Direction::from_name(&block_state_property(state, "facing")?)
}
fn can_take_shape(
    state: u32,
    direction: Direction,
    position: (i32, i32, i32),
    get: &mut impl FnMut((i32, i32, i32)) -> Result<u32>,
) -> Result<bool> {
    let side = get(neighbour(position, direction))?;
    Ok(!stair(side) || facing(side) != facing(state) || !same_stair_half(state, side))
}
/// StairBlock.getStairsShape, shared by placement and horizontal neighbour updates.
pub fn stairs_shape(
    state: u32,
    position: (i32, i32, i32),
    mut get: impl FnMut((i32, i32, i32)) -> Result<u32>,
) -> Result<&'static str> {
    ensure!(stair(state), "not a stair state");
    let direction = facing(state).ok_or_else(|| anyhow!("invalid stair facing"))?;
    let front = get(neighbour(position, direction))?;
    if stair(front) && same_stair_half(state, front) {
        let f = facing(front).unwrap();
        if f.axis() != direction.axis() && can_take_shape(state, f.opposite(), position, &mut get)?
        {
            return Ok(if f == direction.counter_clockwise() {
                "outer_left"
            } else {
                "outer_right"
            });
        }
    }
    let back = get(neighbour(position, direction.opposite()))?;
    if stair(back) && same_stair_half(state, back) {
        let f = facing(back).unwrap();
        if f.axis() != direction.axis() && can_take_shape(state, f, position, &mut get)? {
            return Ok(if f == direction.counter_clockwise() {
                "inner_left"
            } else {
                "inner_right"
            });
        }
    }
    Ok("straight")
}

/// Original face-support facts. Animated block entities need a live-world
/// provider; their absent-entity extraction is never silently used here.
pub fn block_support_face(state: u32, direction: Direction) -> Option<bool> {
    let b = block(state).ok()?;
    if matches!(b.class.as_str(), "dmy" | "dsu") {
        return None;
    }
    Some(catalog().connections.get(state as usize)? & (1 << direction as u8) != 0)
}
fn connection_exception(state: u32) -> bool {
    catalog()
        .connections
        .get(state as usize)
        .is_none_or(|flags| flags & 64 != 0)
}
fn horizontal_connection_state(
    state: u32,
    position: (i32, i32, i32),
    get: &mut impl FnMut((i32, i32, i32)) -> Result<u32>,
) -> Result<Option<u32>> {
    let b = block(state)?;
    if !matches!(b.placement_class.as_str(), "diw" | "djz") {
        return Ok(None);
    }
    let mut props = BTreeMap::new();
    for direction in [
        Direction::North,
        Direction::South,
        Direction::West,
        Direction::East,
    ] {
        let next = get(neighbour(position, direction))?;
        let next_block = block(next)?;
        let Some(sturdy) = block_support_face(next, direction.opposite()) else {
            return Ok(None);
        };
        let solid = !connection_exception(next) && sturdy;
        let connected = if b.placement_class == "diw" {
            let same_fence = in_block_tag(next, "minecraft:fences")
                && in_block_tag(next, "minecraft:wooden_fences")
                    == in_block_tag(state, "minecraft:wooden_fences");
            let gate = next_block.placement_class == "dix"
                && facing(next).is_some_and(|f| f.axis() == direction.clockwise().axis());
            solid || same_fence || gate
        } else {
            solid || next_block.placement_class == "djz" || in_block_tag(next, "minecraft:walls")
        };
        props.insert(direction.name().to_owned(), connected.to_string());
    }
    Ok(Some(block_state_with_properties(state, &props)?))
}
fn wall_connection_state(
    state: u32,
    position: (i32, i32, i32),
    get: &mut impl FnMut((i32, i32, i32)) -> Result<u32>,
) -> Result<Option<u32>> {
    let above = get(neighbour(position, Direction::Up))?;
    if crate::geometry::static_block_collision_boxes(above).is_none() {
        return Ok(None);
    }
    let cover = catalog().wall_cover[above as usize];
    let mut sides = BTreeMap::new();
    for direction in [
        Direction::North,
        Direction::South,
        Direction::West,
        Direction::East,
    ] {
        let next = get(neighbour(position, direction))?;
        let next_block = block(next)?;
        let Some(sturdy) = block_support_face(next, direction.opposite()) else {
            return Ok(None);
        };
        let gate = next_block.placement_class == "dix"
            && facing(next).is_some_and(|f| f.axis() == direction.clockwise().axis());
        let connected = in_block_tag(next, "minecraft:walls")
            || (!connection_exception(next) && sturdy)
            || next_block.placement_class == "djz"
            || gate;
        let mask = match direction {
            Direction::North => 2,
            Direction::South => 4,
            Direction::West => 8,
            Direction::East => 16,
            _ => unreachable!(),
        };
        sides.insert(
            direction.name().to_owned(),
            if !connected {
                "none".to_owned()
            } else if cover & mask != 0 {
                "tall".to_owned()
            } else {
                "low".to_owned()
            },
        );
    }
    let n = &sides["north"];
    let s = &sides["south"];
    let w = &sides["west"];
    let e = &sides["east"];
    let above_raised = block(above)?.placement_class == "dox"
        && block_state_property(above, "up").as_deref() == Some("true");
    let all_none = [n, s, w, e].iter().all(|side| side.as_str() == "none");
    let asymmetric = (n == "none") != (s == "none") || (w == "none") != (e == "none");
    let opposite_tall = (n == "tall" && s == "tall") || (w == "tall" && e == "tall");
    let up = above_raised
        || all_none
        || asymmetric
        || (!opposite_tall
            && (in_block_tag(above, "minecraft:wall_post_override") || cover & 1 != 0));
    sides.insert("up".to_owned(), up.to_string());
    Ok(Some(block_state_with_properties(state, &sides)?))
}
/// Pure state portion of horizontal neighbour updates for fences, panes/bars
/// and stairs. Fluid tick scheduling belongs to the world's tick scheduler.
pub fn update_horizontal_connections(
    state: u32,
    position: (i32, i32, i32),
    mut get: impl FnMut((i32, i32, i32)) -> Result<u32>,
) -> Result<Option<u32>> {
    if block(state)?.placement_class == "dox" {
        return wall_connection_state(state, position, &mut get);
    }
    if stair(state) {
        return Ok(Some(set(
            state,
            &[("shape", stairs_shape(state, position, get)?)],
        )?));
    }
    horizontal_connection_state(state, position, &mut get)
}

fn replacement_in_world(
    placing: u32,
    position: BlockPosition,
    context: &PlacementContext,
    world: &mut impl PlacementWorld,
) -> Result<Option<bool>> {
    let existing = world.block(position)?;
    if block(existing)?.replacement_class == "dov" {
        let clicked = world.block(context.position)?;
        if block(clicked)?.min == block(existing)?.min {
            return Ok(Some(vine_face_count(clicked) < 5));
        }
        let old = block(existing)?;
        let new = block(placing)?;
        return Ok(Some(
            old.replaceable && (new.item == 0 || old.item != new.item),
        ));
    }
    can_replace_with_block(existing, placing, context, context.replacing_clicked)
}
fn door_hinge(context: &PlacementContext, world: &mut impl PlacementWorld) -> Result<&'static str> {
    let facing = context.horizontal_direction;
    let above = neighbour(context.position, Direction::Up);
    let left = neighbour(context.position, facing.counter_clockwise());
    let left_above = neighbour(above, facing.counter_clockwise());
    let right = neighbour(context.position, facing.clockwise());
    let right_above = neighbour(above, facing.clockwise());
    let score = -(world.collision_full(left)? as i32) - (world.collision_full(left_above)? as i32)
        + (world.collision_full(right)? as i32)
        + (world.collision_full(right_above)? as i32);
    let left_state = world.block(left)?;
    let right_state = world.block(right)?;
    let left_door = block(left_state)?.placement_class == "die"
        && block_state_property(left_state, "half").as_deref() == Some("lower");
    let right_door = block(right_state)?.placement_class == "die"
        && block_state_property(right_state, "half").as_deref() == Some("lower");
    if (left_door && !right_door) || score > 0 {
        return Ok("right");
    }
    if (right_door && !left_door) || score < 0 {
        return Ok("left");
    }
    let step = facing.step();
    let x = context.hit_location.x - context.position.0 as f64;
    let z = context.hit_location.z - context.position.2 as f64;
    Ok(
        if (step.0 < 0 && z < 0.5)
            || (step.0 > 0 && z > 0.5)
            || (step.2 < 0 && x > 0.5)
            || (step.2 > 0 && x < 0.5)
        {
            "right"
        } else {
            "left"
        },
    )
}
/// Extended original state functions requiring explicit live-world facts.
fn multiface_support(
    world: &mut impl PlacementWorld,
    position: BlockPosition,
    direction: Direction,
) -> Result<bool> {
    let at = neighbour(position, direction);
    Ok(world.support(at, direction.opposite(), SupportType::Full)?
        || world.collision_face_full(at, direction.opposite())?)
}
fn vine_support(
    world: &mut impl PlacementWorld,
    position: BlockPosition,
    direction: Direction,
) -> Result<bool> {
    if direction == Direction::Down {
        return Ok(false);
    }
    if multiface_support(world, position, direction)? {
        return Ok(true);
    }
    if direction.axis() != Axis::Y {
        let above = world.block(neighbour(position, Direction::Up))?;
        if block(above)?.placement_class == "dov"
            && block_state_property(above, direction.name()).as_deref() == Some("true")
        {
            return Ok(true);
        }
    }
    Ok(false)
}
fn dripstone_matches(state: u32, direction: Direction) -> Result<bool> {
    Ok(block(state)?.placement_class == "dlq"
        && block_state_property(state, "vertical_direction").as_deref() == Some(direction.name()))
}
fn dripstone_support(
    world: &mut impl PlacementWorld,
    position: BlockPosition,
    direction: Direction,
) -> Result<bool> {
    let back = neighbour(position, direction.opposite());
    Ok(world.support(back, direction, SupportType::Full)?
        || dripstone_matches(world.block(back)?, direction)?)
}
fn wall_hanging_sign_supported(
    state: u32,
    position: BlockPosition,
    world: &mut impl PlacementWorld,
) -> Result<bool> {
    let front = facing(state).ok_or_else(|| anyhow!("hanging sign facing required"))?;
    for side in [front.clockwise(), front.counter_clockwise()] {
        let at = neighbour(position, side);
        let next = world.block(at)?;
        if in_block_tag(next, "minecraft:wall_hanging_signs") {
            if facing(next).is_some_and(|direction| direction.axis() == front.axis()) {
                return Ok(true);
            }
        } else if world.support(at, side.opposite(), SupportType::Full)? {
            return Ok(true);
        }
    }
    Ok(false)
}
pub fn placement_state_in_world(
    placing_state: u32,
    context: &PlacementContext,
    world: &mut impl PlacementWorld,
) -> Result<PlacementResult> {
    let b = block(placing_state)?;
    let state = match b.placement_class.as_str() {
        "die" => {
            if context.position.1 >= world.max_build_height()?.wrapping_sub(1) {
                return Ok(PlacementResult::Rejected);
            }
            let above = neighbour(context.position, Direction::Up);
            match replacement_in_world(placing_state, above, context, world)? {
                Some(false) => return Ok(PlacementResult::Rejected),
                None => return Ok(PlacementResult::Unsupported("upper_block_replacement")),
                _ => {}
            }
            let powered =
                world.has_neighbor_signal(context.position)? || world.has_neighbor_signal(above)?;
            set(
                b.default,
                &[
                    ("facing", context.horizontal_direction.name()),
                    ("hinge", door_hinge(context, world)?),
                    ("powered", if powered { "true" } else { "false" }),
                    ("open", if powered { "true" } else { "false" }),
                    ("half", "lower"),
                ],
            )?
        }
        "dom" => {
            let (facing, half) =
                if !context.replacing_clicked && context.clicked_face.axis() != Axis::Y {
                    (
                        context.clicked_face,
                        if context.hit_location.y - context.position.1 as f64 > 0.5 {
                            "top"
                        } else {
                            "bottom"
                        },
                    )
                } else {
                    (
                        context.horizontal_direction.opposite(),
                        if context.clicked_face == Direction::Up {
                            "bottom"
                        } else {
                            "top"
                        },
                    )
                };
            let powered = world.has_neighbor_signal(context.position)?;
            let wet = is_source_water(world.block(context.position)?);
            water(
                set(
                    b.default,
                    &[
                        ("facing", facing.name()),
                        ("half", half),
                        ("powered", if powered { "true" } else { "false" }),
                        ("open", if powered { "true" } else { "false" }),
                    ],
                )?,
                wet,
            )?
        }
        "dix" => {
            let facing = context.horizontal_direction;
            let sides = if facing.axis() == Axis::Z {
                [Direction::East, Direction::West]
            } else {
                [Direction::North, Direction::South]
            };
            let in_wall = in_block_tag(
                world.block(neighbour(context.position, sides[0]))?,
                "minecraft:walls",
            ) || in_block_tag(
                world.block(neighbour(context.position, sides[1]))?,
                "minecraft:walls",
            );
            let powered = world.has_neighbor_signal(context.position)?;
            set(
                b.default,
                &[
                    ("facing", facing.name()),
                    ("in_wall", if in_wall { "true" } else { "false" }),
                    ("powered", if powered { "true" } else { "false" }),
                    ("open", if powered { "true" } else { "false" }),
                ],
            )?
        }
        "dfr" => {
            let head = neighbour(context.position, context.horizontal_direction);
            match replacement_in_world(placing_state, head, context, world)? {
                Some(false) => return Ok(PlacementResult::Rejected),
                None => return Ok(PlacementResult::Unsupported("bed_head_replacement")),
                _ => {}
            }
            if !world.inside_world_border(head)? {
                return Ok(PlacementResult::Rejected);
            }
            set(
                b.default,
                &[("facing", context.horizontal_direction.name())],
            )?
        }
        "dow" | "doz" | "dpa" | "dfj" | "dpb" | "dke" | "dme" | "doq" => {
            if b.placement_class == "dke" && !context.replacing_clicked {
                let back =
                    world.block(neighbour(context.position, context.clicked_face.opposite()))?;
                if block(back)?.min == b.min && facing(back) == Some(context.clicked_face) {
                    return Ok(PlacementResult::Rejected);
                }
            }
            let current = world.block(context.position)?;
            let wet = if b.placement_class == "dfj" {
                catalog().fluids[current as usize] & 4 != 0
            } else {
                is_source_water(current)
            };
            for looking in context.looking_directions() {
                if looking.axis() == Axis::Y {
                    continue;
                }
                let mut state = set(b.default, &[("facing", looking.opposite().name())])?;
                if matches!(b.placement_class.as_str(), "doz" | "dfj" | "dke") {
                    state = water(state, wet)?;
                }
                let valid = if b.placement_class == "dpa" {
                    match replacement_in_world(
                        placing_state,
                        neighbour(context.position, looking),
                        context,
                        world,
                    )? {
                        Some(replaceable) => !replaceable,
                        None => {
                            return Ok(PlacementResult::Unsupported("wall_skull_back_replacement"))
                        }
                    }
                } else {
                    match can_survive(state, context.position, world)? {
                        SurvivalResult::Supported(value) => value,
                        SurvivalResult::Unsupported(reason) => {
                            return Ok(PlacementResult::Unsupported(reason))
                        }
                    }
                };
                if valid {
                    return Ok(PlacementResult::State(state));
                }
            }
            return Ok(PlacementResult::Rejected);
        }
        "dis" => {
            for looking in context.looking_directions() {
                let state = if looking.axis() == Axis::Y {
                    set(
                        b.default,
                        &[
                            (
                                "face",
                                if looking == Direction::Up {
                                    "ceiling"
                                } else {
                                    "floor"
                                },
                            ),
                            ("facing", context.horizontal_direction.name()),
                        ],
                    )?
                } else {
                    set(
                        b.default,
                        &[("face", "wall"), ("facing", looking.opposite().name())],
                    )?
                };
                match can_survive(state, context.position, world)? {
                    SurvivalResult::Supported(true) => return Ok(PlacementResult::State(state)),
                    SurvivalResult::Unsupported(reason) => {
                        return Ok(PlacementResult::Unsupported(reason))
                    }
                    _ => {}
                }
            }
            return Ok(PlacementResult::Rejected);
        }
        "dkf" => {
            let wet = is_source_water(world.block(context.position)?);
            for looking in context.looking_directions() {
                if looking.axis() != Axis::Y {
                    continue;
                }
                let state = set(
                    b.default,
                    &[(
                        "hanging",
                        if looking == Direction::Up {
                            "true"
                        } else {
                            "false"
                        },
                    )],
                )?;
                if can_survive(state, context.position, world)? == SurvivalResult::Supported(true) {
                    return Ok(PlacementResult::State(water(state, wet)?));
                }
            }
            return Ok(PlacementResult::Rejected);
        }
        "dov" => {
            let current = world.block(context.position)?;
            let same = block(current)?.min == b.min;
            let state = if same { current } else { b.default };
            for direction in context.looking_directions() {
                if direction == Direction::Down
                    || (same
                        && block_state_property(current, direction.name()).as_deref()
                            == Some("true"))
                {
                    continue;
                }
                if vine_support(world, context.position, direction)? {
                    return Ok(PlacementResult::State(set(
                        state,
                        &[(direction.name(), "true")],
                    )?));
                }
            }
            if same {
                state
            } else {
                return Ok(PlacementResult::Rejected);
            }
        }
        "dky" => {
            let current = world.block(context.position)?;
            let same = block(current)?.min == b.min;
            for direction in context.looking_directions() {
                if block_state_property(b.default, direction.name()).is_none()
                    || (same
                        && block_state_property(current, direction.name()).as_deref()
                            == Some("true"))
                {
                    continue;
                }
                if !multiface_support(world, context.position, direction)? {
                    continue;
                }
                let mut state = if same { current } else { b.default };
                if !same && block_state_property(state, "waterlogged").is_some() {
                    state = water(state, is_source_water(current))?
                }
                return Ok(PlacementResult::State(set(
                    state,
                    &[(direction.name(), "true")],
                )?));
            }
            return Ok(PlacementResult::Rejected);
        }
        "dlq" => {
            let preferred = context.nearest_vertical_direction.opposite();
            let direction = if dripstone_support(world, context.position, preferred)? {
                preferred
            } else if dripstone_support(world, context.position, preferred.opposite())? {
                preferred.opposite()
            } else {
                return Ok(PlacementResult::Rejected);
            };
            let ahead = world.block(neighbour(context.position, direction))?;
            let thickness = if dripstone_matches(ahead, direction.opposite())? {
                if !context.secondary_use
                    || block_state_property(ahead, "thickness").as_deref() == Some("tip_merge")
                {
                    "tip_merge"
                } else {
                    "tip"
                }
            } else if !dripstone_matches(ahead, direction)? {
                "tip"
            } else if matches!(
                block_state_property(ahead, "thickness").as_deref(),
                Some("tip" | "tip_merge")
            ) {
                "frustum"
            } else if !dripstone_matches(
                world.block(neighbour(context.position, direction.opposite()))?,
                direction,
            )? {
                "base"
            } else {
                "middle"
            };
            water(
                set(
                    b.default,
                    &[
                        ("vertical_direction", direction.name()),
                        ("thickness", thickness),
                    ],
                )?,
                is_source_water(world.block(context.position)?),
            )?
        }
        "doy" => {
            let wet = is_source_water(world.block(context.position)?);
            for direction in context.looking_directions() {
                if direction.axis() == Axis::Y || direction.axis() == context.clicked_face.axis() {
                    continue;
                }
                let state = set(b.default, &[("facing", direction.opposite().name())])?;
                if wall_hanging_sign_supported(state, context.position, world)? {
                    return Ok(PlacementResult::State(water(state, wet)?));
                }
            }
            return Ok(PlacementResult::Rejected);
        }
        "dgx" => {
            let above = neighbour(context.position, Direction::Up);
            let above_state = world.block(above)?;
            let direction = horizontal_look_direction(context.rotation);
            let mut attached =
                !world.collision_face_full(above, Direction::Down)? || context.secondary_use;
            if in_block_tag(above_state, "minecraft:all_hanging_signs") && !context.secondary_use {
                if let Some(f) = facing(above_state) {
                    if f.axis() == direction.axis() {
                        attached = false
                    }
                } else if let Ok(rotation) = int_property(above_state, "rotation") {
                    let cardinal = match rotation {
                        0 => Some(Direction::South),
                        4 => Some(Direction::West),
                        8 => Some(Direction::North),
                        12 => Some(Direction::East),
                        _ => None,
                    };
                    if cardinal.is_some_and(|f| f.axis() == direction.axis()) {
                        attached = false
                    }
                }
            }
            let rotation = if attached {
                rotation_segment(context.rotation + 180.0_f32)
            } else {
                match direction.opposite() {
                    Direction::South => 0,
                    Direction::West => 4,
                    Direction::North => 8,
                    Direction::East => 12,
                    _ => unreachable!(),
                }
            };
            water(
                set_int(
                    set(
                        b.default,
                        &[("attached", if attached { "true" } else { "false" })],
                    )?,
                    "rotation",
                    rotation,
                )?,
                is_source_water(world.block(context.position)?),
            )?
        }
        "dhf" => {
            for direction in context.looking_directions() {
                if direction.axis() == Axis::Y {
                    continue;
                }
                let state = set(b.default, &[("facing", direction.name())])?;
                if can_survive(state, context.position, world)? == SurvivalResult::Supported(true) {
                    return Ok(PlacementResult::State(state));
                }
            }
            return Ok(PlacementResult::Rejected);
        }
        "dfu" => {
            let face = context.clicked_face;
            let mut state = if face.axis() == Axis::Y {
                set(
                    b.default,
                    &[
                        (
                            "attachment",
                            if face == Direction::Down {
                                "ceiling"
                            } else {
                                "floor"
                            },
                        ),
                        ("facing", context.horizontal_direction.name()),
                    ],
                )?
            } else {
                let sides = if face.axis() == Axis::X {
                    [Direction::West, Direction::East]
                } else {
                    [Direction::North, Direction::South]
                };
                let double = world.support(
                    neighbour(context.position, sides[0]),
                    sides[0].opposite(),
                    SupportType::Full,
                )? && world.support(
                    neighbour(context.position, sides[1]),
                    sides[1].opposite(),
                    SupportType::Full,
                )?;
                set(
                    b.default,
                    &[
                        (
                            "attachment",
                            if double { "double_wall" } else { "single_wall" },
                        ),
                        ("facing", face.opposite().name()),
                    ],
                )?
            };
            if can_survive(state, context.position, world)? == SurvivalResult::Supported(true) {
                return Ok(PlacementResult::State(state));
            }
            if face.axis() != Axis::Y {
                let floor = world.support(
                    neighbour(context.position, Direction::Down),
                    Direction::Up,
                    SupportType::Full,
                )?;
                state = set(
                    state,
                    &[("attachment", if floor { "floor" } else { "ceiling" })],
                )?;
                if can_survive(state, context.position, world)? == SurvivalResult::Supported(true) {
                    return Ok(PlacementResult::State(state));
                }
            }
            return Ok(PlacementResult::Rejected);
        }
        "dmc" => set(
            b.default,
            &[(
                "lit",
                if world.has_neighbor_signal(context.position)? {
                    "true"
                } else {
                    "false"
                },
            )],
        )?,
        "dhr" => {
            let front = context.nearest_looking_direction.opposite();
            let top = match front {
                Direction::Down => context.horizontal_direction.opposite(),
                Direction::Up => context.horizontal_direction,
                _ => Direction::Up,
            };
            set(
                b.default,
                &[
                    ("orientation", &format!("{}_{}", front.name(), top.name())),
                    (
                        "triggered",
                        if world.has_neighbor_signal(context.position)? {
                            "true"
                        } else {
                            "false"
                        },
                    ),
                ],
            )?
        }
        "dic" | "div" => {
            if can_survive(b.default, context.position, world)? == SurvivalResult::Supported(true) {
                b.default
            } else {
                block_named("minecraft:dirt")?
            }
        }
        "dig" | "dof" => {
            if context.position.1 >= world.max_build_height()?.wrapping_sub(1) {
                return Ok(PlacementResult::Rejected);
            }
            let above = neighbour(context.position, Direction::Up);
            match replacement_in_world(placing_state, above, context, world)? {
                Some(false) => return Ok(PlacementResult::Rejected),
                None => return Ok(PlacementResult::Unsupported("upper_block_replacement")),
                _ => {}
            }
            if b.placement_class == "dof" && catalog().fluids[world.block(above)? as usize] & 4 == 0
            {
                return Ok(PlacementResult::Rejected);
            }
            b.default
        }
        _ => return placement_state(placing_state, context, |position| world.block(position)),
    };
    Ok(PlacementResult::State(state))
}

/// Original BlockItem / StandingAndWallBlockItem state selection. The caller
/// still performs entity obstruction and item component/application effects.
pub fn placement_for_item(
    item_id: i32,
    context: &PlacementContext,
    world: &mut impl PlacementWorld,
) -> Result<PlacementResult> {
    let Some(item) = item_placement(item_id) else {
        return Ok(PlacementResult::Unsupported("not_block_item"));
    };
    if item.context_class != "cso" {
        return Ok(PlacementResult::Unsupported(&item.context_class));
    }
    if item.placement_class == "cwe" {
        let wall = item
            .wall_state
            .ok_or_else(|| anyhow!("wall variant missing"))?;
        let wall_result = placement_state_in_world(wall, context, world)?;
        let attachment = [
            Direction::Down,
            Direction::Up,
            Direction::North,
            Direction::South,
            Direction::West,
            Direction::East,
        ][item
            .attachment
            .ok_or_else(|| anyhow!("attachment direction missing"))?];
        for looking in context.looking_directions() {
            if looking == attachment.opposite() {
                continue;
            }
            let candidate = if looking == attachment {
                placement_state_in_world(item.primary_state, context, world)?
            } else {
                wall_result
            };
            match candidate {
                PlacementResult::State(state) => {
                    if item.class == "cud"
                        && block(state)?.placement_class == "doy"
                        && !wall_hanging_sign_supported(state, context.position, world)?
                    {
                        continue;
                    }
                    match can_survive(state, context.position, world)? {
                        SurvivalResult::Supported(true) => return Ok(candidate),
                        SurvivalResult::Unsupported(reason) => {
                            return Ok(PlacementResult::Unsupported(reason))
                        }
                        _ => {}
                    }
                }
                PlacementResult::Unsupported(_) => return Ok(candidate),
                PlacementResult::Rejected => {}
            }
        }
        return Ok(PlacementResult::Rejected);
    }
    let result = placement_state_in_world(item.primary_state, context, world)?;
    if let PlacementResult::State(state) = result {
        match can_survive(state, context.position, world)? {
            SurvivalResult::Supported(false) => return Ok(PlacementResult::Rejected),
            SurvivalResult::Unsupported(reason) => return Ok(PlacementResult::Unsupported(reason)),
            _ => {}
        }
    }
    Ok(result)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlannedBlockWrite {
    pub position: BlockPosition,
    pub state: u32,
    pub flags: u8,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedBlockEntity {
    pub position: BlockPosition,
    pub state: u32,
    pub type_name: &'static str,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlannedBlockTick {
    pub position: BlockPosition,
    pub state: u32,
    pub delay: i32,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeferredBlockHook {
    pub position: BlockPosition,
    pub state: u32,
    pub hook: &'static str,
    pub declaring_class: &'static str,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlannedShapeUpdate {
    pub position: BlockPosition,
    pub state: u32,
    pub flags: u8,
    pub recursion_left: i32,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlacementPlan {
    /// In original execution order: primary write followed by setPlacedBy writes.
    pub writes: Vec<PlannedBlockWrite>,
    pub block_entities: Vec<PlannedBlockEntity>,
    pub scheduled_ticks: Vec<PlannedBlockTick>,
    /// Vanilla's explicit neighbour notices in addition to flag3 world writes.
    pub extra_neighbour_notices: Vec<BlockPosition>,
    /// Extra explicit updateNeighbourShapes calls, beyond normal write flags.
    pub extra_shape_updates: Vec<PlannedShapeUpdate>,
    /// Must be executed by the world before this plan can claim full parity.
    pub deferred_hooks: Vec<DeferredBlockHook>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlacementPlanResult {
    Plan(PlacementPlan),
    Rejected,
    Unsupported(&'static str),
}
/// Prepare deterministic writes/effects; no mutation is performed during planning.
/// Validation and obstruction checks remain in the server's action transaction.
pub fn placement_plan(
    placing: u32,
    context: &PlacementContext,
    world: &mut impl PlacementWorld,
) -> Result<PlacementPlanResult> {
    let state = match placement_state_in_world(placing, context, world)? {
        PlacementResult::State(state) => state,
        PlacementResult::Rejected => return Ok(PlacementPlanResult::Rejected),
        PlacementResult::Unsupported(reason) => {
            return Ok(PlacementPlanResult::Unsupported(reason))
        }
    };
    match can_survive(state, context.position, world)? {
        SurvivalResult::Supported(false) => return Ok(PlacementPlanResult::Rejected),
        SurvivalResult::Unsupported(reason) => return Ok(PlacementPlanResult::Unsupported(reason)),
        _ => {}
    }
    let item = item_placement(block(state)?.item);
    plan_selected_state(state, item, context, world)
}
/// Select the actual item's standing/wall variant before planning writes.
pub fn placement_plan_for_item(
    item_id: i32,
    context: &PlacementContext,
    world: &mut impl PlacementWorld,
) -> Result<PlacementPlanResult> {
    let state = match placement_for_item(item_id, context, world)? {
        PlacementResult::State(state) => state,
        PlacementResult::Rejected => return Ok(PlacementPlanResult::Rejected),
        PlacementResult::Unsupported(reason) => {
            return Ok(PlacementPlanResult::Unsupported(reason))
        }
    };
    plan_selected_state(state, item_placement(item_id), context, world)
}
fn plan_selected_state(
    state: u32,
    item: Option<&'static ItemPlacement>,
    context: &PlacementContext,
    world: &mut impl PlacementWorld,
) -> Result<PlacementPlanResult> {
    let b = block(state)?;
    let primary_flags = if item.is_some_and(|i| i.place_block_class == "csn") {
        26
    } else {
        11
    };
    let mut initial_writes = Vec::new();
    if item.is_some_and(|i| i.place_block_class == "cth") {
        let above = neighbour(context.position, Direction::Up);
        initial_writes.push(PlannedBlockWrite {
            position: above,
            state: if is_water(world.block(above)?) {
                block_named("minecraft:water")?
            } else {
                0
            },
            flags: 27,
        });
    }
    initial_writes.push(PlannedBlockWrite {
        position: context.position,
        state,
        flags: primary_flags,
    });
    let mut plan = PlacementPlan {
        writes: initial_writes,
        block_entities: Vec::new(),
        scheduled_ticks: Vec::new(),
        extra_neighbour_notices: Vec::new(),
        extra_shape_updates: Vec::new(),
        deferred_hooks: Vec::new(),
    };
    match b.placed_by_class.as_str() {
        "dfy" => {}
        "die" => plan.writes.push(PlannedBlockWrite {
            position: neighbour(context.position, Direction::Up),
            state: set(state, &[("half", "upper")])?,
            flags: 3,
        }),
        "dig" => plan.writes.push(PlannedBlockWrite {
            position: neighbour(context.position, Direction::Up),
            state: set(b.default, &[("half", "upper")])?,
            flags: 3,
        }),
        "dfr" => {
            plan.writes.push(PlannedBlockWrite {
                position: neighbour(
                    context.position,
                    facing(state).ok_or_else(|| anyhow!("bed facing required"))?,
                ),
                state: set(state, &[("part", "head")])?,
                flags: 3,
            });
            plan.extra_neighbour_notices.push(context.position);
            plan.extra_shape_updates.push(PlannedShapeUpdate {
                position: context.position,
                state,
                flags: 3,
                recursion_left: 512,
            });
        }
        _ => plan.deferred_hooks.push(DeferredBlockHook {
            position: context.position,
            state,
            hook: "setPlacedBy",
            declaring_class: &b.placed_by_class,
        }),
    }
    for write in &plan.writes {
        let b = block(write.state)?;
        if let Some(type_name) = &b.block_entity_type {
            plan.block_entities.push(PlannedBlockEntity {
                position: write.position,
                state: write.state,
                type_name,
            });
        }
        if b.on_place_class == "diu" {
            plan.scheduled_ticks.push(PlannedBlockTick {
                position: write.position,
                state: write.state,
                delay: b
                    .falling_delay
                    .ok_or_else(|| anyhow!("falling block delay missing"))?,
            });
        } else if b.on_place_class != "dtb" {
            plan.deferred_hooks.push(DeferredBlockHook {
                position: write.position,
                state: write.state,
                hook: "onPlace",
                declaring_class: &b.on_place_class,
            });
        }
    }
    Ok(PlacementPlanResult::Plan(plan))
}

/// Select the exact initial state for implemented original getStateForPlacement
/// functions. `get` supplies actual states, including source fluids/neighbours.
pub fn placement_state(
    placing_state: u32,
    context: &PlacementContext,
    mut get: impl FnMut((i32, i32, i32)) -> Result<u32>,
) -> Result<PlacementResult> {
    ensure!(
        context.horizontal_direction.axis() != Axis::Y,
        "horizontal placement direction is vertical"
    );
    let b = block(placing_state)?;
    let current = get(context.position)?;
    let wet = is_source_water(current);
    let top = context.clicked_face == Direction::Down
        || (context.clicked_face != Direction::Up
            && context.hit_location.y - context.position.1 as f64 > 0.5);
    let result = match b.placement_class.as_str() {
        "dfy" => b.default,
        "dml" | "djy" => set(
            b.default,
            &[(
                "axis",
                match context.clicked_face.axis() {
                    Axis::X => "x",
                    Axis::Y => "y",
                    Axis::Z => "z",
                },
            )],
        )?,
        "dgy" => water(
            set(
                b.default,
                &[(
                    "axis",
                    match context.clicked_face.axis() {
                        Axis::X => "x",
                        Axis::Y => "y",
                        Axis::Z => "z",
                    },
                )],
            )?,
            wet,
        )?,
        "dnc" => {
            if (b.min..=b.max).contains(&current) {
                water(set(current, &[("type", "double")])?, false)?
            } else {
                water(
                    set(b.default, &[("type", if top { "top" } else { "bottom" })])?,
                    wet,
                )?
            }
        }
        "dox" => {
            let Some(state) = wall_connection_state(b.default, context.position, &mut get)? else {
                return Ok(PlacementResult::Unsupported("dynamic_neighbor_support"));
            };
            water(state, wet)?
        }
        "diw" | "djz" => {
            let Some(state) = horizontal_connection_state(b.default, context.position, &mut get)?
            else {
                return Ok(PlacementResult::Unsupported("dynamic_neighbor_support"));
            };
            water(state, wet)?
        }
        "dnt" => {
            let state = water(
                set(
                    b.default,
                    &[
                        ("facing", context.horizontal_direction.name()),
                        ("half", if top { "top" } else { "bottom" }),
                    ],
                )?,
                wet,
            )?;
            let shape = stairs_shape(state, context.position, &mut get)?;
            set(state, &[("shape", shape)])?
        }
        "det" | "dgs" | "djh" | "dkq" | "dnw" => set(
            b.default,
            &[("facing", context.horizontal_direction.opposite().name())],
        )?,
        "dip" => water(
            set(
                b.default,
                &[("facing", context.horizontal_direction.opposite().name())],
            )?,
            wet,
        )?,
        "dey" => set(
            b.default,
            &[("facing", context.horizontal_direction.clockwise().name())],
        )?,
        "dmy" => set(b.default, &[("facing", context.clicked_face.name())])?,
        "dkn" => water(
            set(b.default, &[("facing", context.clicked_face.name())])?,
            wet,
        )?,
        "dfe" | "did" | "dhh" => set(
            b.default,
            &[(
                "facing",
                context.nearest_looking_direction.opposite().name(),
            )],
        )?,
        // Observer's bytecode calls getOpposite twice, preserving nearest look.
        "dlj" => set(
            b.default,
            &[("facing", context.nearest_looking_direction.name())],
        )?,
        "dsv" => set(
            b.default,
            &[
                (
                    "facing",
                    context.nearest_looking_direction.opposite().name(),
                ),
                ("extended", "false"),
            ],
        )?,
        "dpd" | "dms" | "dmt" | "djr" => water(b.default, wet)?,
        "dfn" => water(
            set(
                b.default,
                &[(
                    "shape",
                    if context.horizontal_direction.axis() == Axis::X {
                        "east_west"
                    } else {
                        "north_south"
                    },
                )],
            )?,
            wet,
        )?,
        "dnj" => set(
            b.default,
            &[(
                "snowy",
                if in_block_tag(
                    get(neighbour(context.position, Direction::Up))?,
                    "minecraft:snow",
                ) {
                    "true"
                } else {
                    "false"
                },
            )],
        )?,
        "dki" => {
            let mut distance = 7;
            for dir in [
                Direction::Down,
                Direction::Up,
                Direction::North,
                Direction::South,
                Direction::West,
                Direction::East,
            ] {
                let next = get(neighbour(context.position, dir))?;
                let next_distance = if in_block_tag(next, "minecraft:logs") {
                    0
                } else if block(next)?.placement_class == "dki" {
                    int_property(next, "distance")?
                } else {
                    7
                };
                distance = distance.min(next_distance + 1);
                if distance == 1 {
                    break;
                }
            }
            water(
                set_int(
                    set(b.default, &[("persistent", "true")])?,
                    "distance",
                    distance,
                )?,
                wet,
            )?
        }
        "dgn" => {
            if (b.min..=b.max).contains(&current) {
                let count = int_property(current, "candles")?;
                set_int(current, "candles", if count == 4 { 1 } else { count + 1 })?
            } else {
                water(b.default, wet)?
            }
        }
        "dmw" => {
            if (b.min..=b.max).contains(&current) {
                set_int(
                    current,
                    "pickles",
                    (int_property(current, "pickles")? + 1).min(4),
                )?
            } else {
                water(b.default, wet)?
            }
        }
        "dll" => {
            if (b.min..=b.max).contains(&current) {
                set_int(
                    current,
                    "flower_amount",
                    (int_property(current, "flower_amount")? + 1).min(4),
                )?
            } else {
                set(
                    b.default,
                    &[("facing", context.horizontal_direction.opposite().name())],
                )?
            }
        }
        "dor" => {
            if (b.min..=b.max).contains(&current) {
                set_int(current, "eggs", (int_property(current, "eggs")? + 1).min(4))?
            } else {
                b.default
            }
        }
        "dhl" => water(
            b.default,
            catalog()
                .fluids
                .get(current as usize)
                .is_some_and(|f| f & 4 != 0),
        )?,
        "dfd" => set_int(
            b.default,
            "rotation",
            rotation_segment(context.rotation + 180.0_f32),
        )?,
        "dnu" => water(
            set_int(
                b.default,
                "rotation",
                rotation_segment(context.rotation + 180.0_f32),
            )?,
            wet,
        )?,
        "dnb" => set_int(b.default, "rotation", rotation_segment(context.rotation))?,
        "dff" | "dku" => water(b.default, wet)?,
        "dfi" => water(b.default, catalog().fluids[current as usize] & 4 != 0)?,
        "dhy" => water(
            set(
                b.default,
                &[
                    ("facing", context.horizontal_direction.name()),
                    ("cracked", "false"),
                ],
            )?,
            wet,
        )?,
        "dgl" => water(
            set(
                b.default,
                &[("facing", context.horizontal_direction.name())],
            )?,
            wet,
        )?,
        "dhc" => set(
            b.default,
            &[("facing", context.horizontal_direction.opposite().name())],
        )?,
        "dex" => water(
            set(b.default, &[("facing", context.clicked_face.name())])?,
            wet,
        )?,
        "dio" => {
            let back = get(neighbour(context.position, context.clicked_face.opposite()))?;
            let direction =
                if (b.min..=b.max).contains(&back) && facing(back) == Some(context.clicked_face) {
                    context.clicked_face.opposite()
                } else {
                    context.clicked_face
                };
            set(b.default, &[("facing", direction.name())])?
        }
        "dni" => {
            if (b.min..=b.max).contains(&current) {
                set_int(
                    current,
                    "layers",
                    (int_property(current, "layers")? + 1).min(8),
                )?
            } else {
                b.default
            }
        }
        "djv" => {
            let mut values = BTreeMap::new();
            for direction in [
                Direction::Down,
                Direction::Up,
                Direction::North,
                Direction::South,
                Direction::West,
                Direction::East,
            ] {
                let next = get(neighbour(context.position, direction))?;
                values.insert(
                    direction.name().to_owned(),
                    (!(b.min..=b.max).contains(&next)).to_string(),
                );
            }
            block_state_with_properties(b.default, &values)?
        }
        "dia" | "dfs" => set(
            b.default,
            &[("facing", context.horizontal_direction.opposite().name())],
        )?,
        "din" => set(
            b.default,
            &[
                ("facing", context.horizontal_direction.opposite().name()),
                ("eye", "false"),
            ],
        )?,
        "djt" => {
            let direction = context.clicked_face.opposite();
            set(
                b.default,
                &[
                    (
                        "facing",
                        if direction.axis() == Axis::Y {
                            Direction::Down
                        } else {
                            direction
                        }
                        .name(),
                    ),
                    ("enabled", "true"),
                ],
            )?
        }
        "dka" => {
            let front = context.clicked_face;
            let top = if front.axis() == Axis::Y {
                context.horizontal_direction.opposite()
            } else {
                Direction::Up
            };
            set(
                b.default,
                &[("orientation", &format!("{}_{}", front.name(), top.name()))],
            )?
        }
        "dgm" => {
            let below = get(neighbour(context.position, Direction::Down))?;
            set(
                b.default,
                &[
                    ("facing", context.horizontal_direction.name()),
                    ("waterlogged", if wet { "true" } else { "false" }),
                    ("lit", if wet { "false" } else { "true" }),
                    (
                        "signal_fire",
                        if block(below)?.name == "minecraft:hay_block" {
                            "true"
                        } else {
                            "false"
                        },
                    ),
                ],
            )?
        }
        "dhb" => {
            let mut chest_facing = |direction| -> Result<Option<Direction>> {
                let state = get(neighbour(context.position, direction))?;
                Ok(
                    if block(state)?.min == b.min
                        && block_state_property(state, "type").as_deref() == Some("single")
                    {
                        facing(state)
                    } else {
                        None
                    },
                )
            };
            let mut front = context.horizontal_direction.opposite();
            let mut kind = "single";
            if context.clicked_face.axis() != Axis::Y && context.secondary_use {
                if let Some(other) = chest_facing(context.clicked_face.opposite())? {
                    if other.axis() != context.clicked_face.axis() {
                        front = other;
                        kind = if front.counter_clockwise() == context.clicked_face.opposite() {
                            "right"
                        } else {
                            "left"
                        }
                    }
                }
            }
            if kind == "single" && !context.secondary_use {
                if chest_facing(front.clockwise())? == Some(front) {
                    kind = "left"
                } else if chest_facing(front.counter_clockwise())? == Some(front) {
                    kind = "right"
                }
            }
            water(
                set(b.default, &[("facing", front.name()), ("type", kind)])?,
                wet,
            )?
        }
        "dlh" => {
            let above = block(get(neighbour(context.position, Direction::Up))?)?;
            let below = block(get(neighbour(context.position, Direction::Down))?)?;
            let instrument = if above.instrument_above {
                above.instrument.as_str()
            } else if below.instrument_above {
                "harp"
            } else {
                below.instrument.as_str()
            };
            set(b.default, &[("instrument", instrument)])?
        }
        "dln" => b.default,
        "dop" => {
            let mut properties = BTreeMap::new();
            for direction in [
                Direction::North,
                Direction::East,
                Direction::South,
                Direction::West,
            ] {
                let next = get(neighbour(context.position, direction))?;
                let connected = block(next)?.placement_class == "dop"
                    || (block(next)?.placement_class == "doq"
                        && facing(next) == Some(direction.opposite()));
                properties.insert(direction.name().to_owned(), connected.to_string());
            }
            block_state_with_properties(b.default, &properties)?
        }
        "dhe" => {
            let mut properties = BTreeMap::new();
            for direction in [
                Direction::Down,
                Direction::Up,
                Direction::North,
                Direction::South,
                Direction::West,
                Direction::East,
            ] {
                let next = get(neighbour(context.position, direction))?;
                let neighbor = block(next)?;
                let connected = neighbor.min == b.min
                    || neighbor.name == "minecraft:chorus_flower"
                    || (direction == Direction::Down && neighbor.name == "minecraft:end_stone");
                properties.insert(direction.name().to_owned(), connected.to_string());
            }
            block_state_with_properties(b.default, &properties)?
        }
        "dfc" => {
            if catalog().fluids[current as usize] & 8 != 0 {
                return Ok(PlacementResult::Rejected);
            }
            let below = get(neighbour(context.position, Direction::Down))?;
            if !in_block_tag(below, "minecraft:bamboo_plantable_on") {
                return Ok(PlacementResult::Rejected);
            }
            if block(below)?.name == "minecraft:bamboo_sapling" {
                set_int(b.default, "age", 0)?
            } else if block(below)?.min == b.min {
                set_int(b.default, "age", u32::from(int_property(below, "age")? > 0))?
            } else {
                let above = get(neighbour(context.position, Direction::Up))?;
                if block(above)?.min == b.min {
                    set_int(b.default, "age", int_property(above, "age")?)?
                } else {
                    block_named("minecraft:bamboo_sapling")?
                }
            }
        }
        "dfv" => {
            let below = get(neighbour(context.position, Direction::Down))?;
            let front = if matches!(
                block(below)?.name.as_str(),
                "minecraft:big_dripleaf" | "minecraft:big_dripleaf_stem"
            ) {
                facing(below).ok_or_else(|| anyhow!("dripleaf facing required"))?
            } else {
                context.horizontal_direction.opposite()
            };
            water(set(b.default, &[("facing", front.name())])?, wet)?
        }
        "dmo" => {
            let below = get(neighbour(context.position, Direction::Down))?;
            let mut distance = if block(below)?.min == b.min {
                int_property(below, "distance")?
            } else if block_support_face(below, Direction::Up) == Some(true) {
                0
            } else {
                7
            };
            if distance != 0 {
                for direction in [
                    Direction::North,
                    Direction::South,
                    Direction::West,
                    Direction::East,
                ] {
                    let next = get(neighbour(context.position, direction))?;
                    if block(next)?.min == b.min {
                        distance = distance.min(int_property(next, "distance")? + 1);
                        if distance == 1 {
                            break;
                        }
                    }
                }
            }
            water(
                set_int(
                    set(
                        b.default,
                        &[(
                            "bottom",
                            if distance > 0 && block(below)?.min != b.min {
                                "true"
                            } else {
                                "false"
                            },
                        )],
                    )?,
                    "distance",
                    distance,
                )?,
                wet,
            )?
        }
        "dkt" => water(set_int(b.default, "age", 4)?, wet)?,
        "dmx" => {
            if catalog().fluids[current as usize] & 4 != 0 {
                b.default
            } else {
                return Ok(PlacementResult::Rejected);
            }
        }
        "djp" => water(b.default, wet)?,
        "dou" => set(
            b.default,
            &[("facing", context.horizontal_direction.opposite().name())],
        )?,
        "dhk" => {
            let mut harden = is_water(current);
            if !harden {
                for direction in [
                    Direction::Up,
                    Direction::North,
                    Direction::South,
                    Direction::West,
                    Direction::East,
                ] {
                    let next = get(neighbour(context.position, direction))?;
                    if is_water(next) {
                        let Some(sturdy) = block_support_face(next, direction.opposite()) else {
                            return Ok(PlacementResult::Unsupported("dynamic_neighbor_support"));
                        };
                        if !sturdy {
                            harden = true;
                            break;
                        }
                    }
                }
            }
            if harden {
                b.hardened_state
                    .ok_or_else(|| anyhow!("concrete powder target missing"))?
            } else {
                b.default
            }
        }
        _ => return Ok(PlacementResult::Unsupported(&b.placement_class)),
    };
    Ok(PlacementResult::State(result))
}

fn vine_face_count(state: u32) -> usize {
    ["up", "north", "south", "east", "west"]
        .iter()
        .filter(|face| block_state_property(state, face).as_deref() == Some("true"))
        .count()
}
fn vacant_multiface(state: u32) -> bool {
    ["up", "down", "north", "south", "east", "west"]
        .iter()
        .any(|face| block_state_property(state, face).as_deref() != Some("true"))
}
/// The nonempty block-item path through original canBeReplaced, including slab
/// merging. None explicitly marks a block-specific replacement rule not ported.
pub fn can_replace_with_block(
    existing: u32,
    placing: u32,
    context: &PlacementContext,
    replacing_clicked: bool,
) -> Result<Option<bool>> {
    let old = block(existing)?;
    let new = block(placing)?;
    match old.replacement_class.as_str() {
        "dln" => return Ok(Some(false)),
        "dmo" => return Ok(Some(old.item == new.item)),
        "djm" => {
            return Ok(Some(
                old.replaceable
                    && (new.item == 0 || old.item != new.item)
                    && old.body_head_item != Some(new.item),
            ))
        }
        "dji" | "dmv" => return Ok(Some(old.item != new.item || vacant_multiface(existing))),
        "dov" => return Ok(Some(vine_face_count(existing) < 5)),
        "dni" => {
            let layers = int_property(existing, "layers")?;
            return Ok(Some(if old.item == new.item && layers < 8 {
                !replacing_clicked || context.clicked_face == Direction::Up
            } else {
                layers == 1
            }));
        }
        _ => {}
    }
    if old.replacement_class == "dtb" {
        return Ok(Some(
            old.replaceable && (new.item == 0 || old.item != new.item),
        ));
    }
    if matches!(
        old.replacement_class.as_str(),
        "dgn" | "dmw" | "dll" | "dor"
    ) {
        let property = match old.replacement_class.as_str() {
            "dgn" => "candles",
            "dmw" => "pickles",
            "dll" => "flower_amount",
            _ => "eggs",
        };
        let stackable =
            !context.secondary_use && old.item == new.item && int_property(existing, property)? < 4;
        return Ok(Some(
            stackable || (old.replaceable && (new.item == 0 || old.item != new.item)),
        ));
    }
    if old.replacement_class == "dnc" {
        let kind =
            block_state_property(existing, "type").ok_or_else(|| anyhow!("slab has no type"))?;
        if kind == "double" || old.min != new.min {
            return Ok(Some(false));
        }
        if !replacing_clicked {
            return Ok(Some(true));
        }
        let upper_hit = context.hit_location.y - context.position.1 as f64 > 0.5;
        let face = context.clicked_face;
        return Ok(Some(if kind == "bottom" {
            face == Direction::Up || (upper_hit && face.axis() != Axis::Y)
        } else {
            face == Direction::Down || (!upper_hit && face.axis() != Axis::Y)
        }));
    }
    Ok(None)
}

pub fn placement_block_min_state(state: u32) -> Result<u32> { Ok(block(state)?.min) }
pub fn placement_block_name(state: u32) -> Result<&'static str> {
    Ok(&block(state)?.name)
}

/// The three pinned AnvilBlock instances inherit FallingBlock.onPlace and
/// updateShape, both scheduling getDelayAfterPlace()==2. This is a source fact,
/// not a claim that a world has the actual falling entity/tick/save owners.
/// Other FallingBlock subclasses retain their existing explicit obligations.
pub fn anvil_falling_delay(state: u32) -> Result<Option<i32>> {
    let b = block(state)?;
    if b.class != "dey" {
        return Ok(None);
    }
    ensure!(b.on_place_class == "diu" && b.falling_delay == Some(2),
        "Anvil callback catalogue differs from the pinned inherited source");
    Ok(b.falling_delay)
}
