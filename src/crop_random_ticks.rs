//! Native 1.21 FarmBlock/CropBlock random ticks, including Beetroot's gate.
//! Source: div/dht/dft from the pinned original server, not growth-rate guesses.
//! Entity lifting and BLOCK_CHANGE dispatch in turnToDirt remain required real
//! host work; moisture/age writes never substitute for those effects.
use crate::{
    fluid_physics::{FluidKind, FluidState},
    geometry::{Aabb, Axis, Vec3, VoxelShape},
    inventory::{block_default_state, block_state_property, block_state_with_properties},
    placement,
    random::LegacyRandomSource,
    runtime_lighting::NativeRuntimeLighting,
    runtime_world::RuntimeWorld,
};
use anyhow::{bail, ensure, Context, Result};
use std::collections::BTreeMap;

pub type Position = (i32, i32, i32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CropKind {
    Wheat,
    Carrots,
    Potatoes,
    Beetroots,
}
impl CropKind {
    fn of(state: u32) -> Result<Self> {
        Ok(match placement::placement_block_name(state)? {
            "minecraft:wheat" => Self::Wheat,
            "minecraft:carrots" => Self::Carrots,
            "minecraft:potatoes" => Self::Potatoes,
            "minecraft:beetroots" => Self::Beetroots,
            _ => bail!("crop random ticks do not cover sampled state {state}"),
        })
    }
    fn name(self) -> &'static str {
        match self {
            Self::Wheat => "minecraft:wheat",
            Self::Carrots => "minecraft:carrots",
            Self::Potatoes => "minecraft:potatoes",
            Self::Beetroots => "minecraft:beetroots",
        }
    }
    pub fn max_age(self) -> u8 {
        if self == Self::Beetroots {
            3
        } else {
            7
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CropProperties {
    pub kind: CropKind,
    pub age: u8,
    pub max_age: u8,
}

fn int_property(state: u32, key: &str) -> Result<u8> {
    block_state_property(state, key)
        .with_context(|| format!("state {state} has no {key}"))?
        .parse()
        .with_context(|| format!("invalid {key} on state {state}"))
}
pub fn crop_properties(state: u32) -> Result<CropProperties> {
    let kind = CropKind::of(state)?;
    let age = int_property(state, "age")?;
    let max_age = kind.max_age();
    ensure!(age <= max_age, "sampled crop age exceeds original maximum");
    Ok(CropProperties { kind, age, max_age })
}
pub fn crop_state_for_age(state: u32, age: u8) -> Result<u32> {
    let kind = CropKind::of(state)?;
    ensure!(
        age <= kind.max_age(),
        "requested crop age exceeds original maximum"
    );
    let default = block_default_state(state).context("crop has no default state")?;
    block_state_with_properties(
        default,
        &BTreeMap::from([("age".to_owned(), age.to_string())]),
    )
}
pub fn farmland_moisture(state: u32) -> Result<u8> {
    ensure!(
        placement::placement_block_name(state)? == "minecraft:farmland",
        "sampled block is not farmland"
    );
    let moisture = int_property(state, "moisture")?;
    ensure!(moisture <= 7, "farmland moisture exceeds original maximum");
    Ok(moisture)
}

/// CropBlock calls getRawBrightness(position,0), not max-local brightness or
/// the grass callback's above-cell query. Keep its supplied sky darkening.
pub trait CropRandomTickHost {
    fn raw_brightness(
        &mut self,
        world: &mut RuntimeWorld,
        position: Position,
        sky_darken: i32,
    ) -> Result<i32>;
    /// Execute the actual flags/callbacks/counters/publications synchronously.
    fn set_block(
        &mut self,
        world: &mut RuntimeWorld,
        position: Position,
        state: u32,
        flags: u32,
        random: &mut LegacyRandomSource,
    ) -> Result<()>;
}
pub trait FarmRandomTickHost: CropRandomTickHost {
    /// Level.isRainingAt: actual weather, sky/heightmap and biome precipitation.
    fn is_raining_at(&mut self, world: &mut RuntimeWorld, position: Position) -> Result<bool>;
    /// FarmBlock.turnToDirt(null,state,level,pos): Block.pushEntitiesUp(old,DIRT)
    /// BEFORE setBlockAndUpdate(flags3), then BLOCK_CHANGE(null,new dirt state).
    /// This callback must not omit intersecting entity movement or game events.
    fn turn_farmland_to_dirt(
        &mut self,
        world: &mut RuntimeWorld,
        position: Position,
        state: u32,
        random: &mut LegacyRandomSource,
    ) -> Result<()>;
}

/// The exact ONLY_SECOND difference of farmland's15/16 collision cube and
/// plain dirt's full cube, used by Block.pushEntitiesUp before the write.
pub fn farmland_push_region(position: Position) -> Aabb {
    let (x, y, z) = (
        f64::from(position.0),
        f64::from(position.1),
        f64::from(position.2),
    );
    Aabb::new(x, y + 15_f64 / 16_f64, z, x + 1_f64, y + 1_f64, z + 1_f64)
}
/// Host queries the actual entity store in the above region, excluding the
/// source's spectators/removed bodies, then executes teleportRelative(0,d,0)
/// with this original Shapes.collide(Y,bounds.move(0,1,0),[difference],-1).
/// The required host callback owns relative teleport publication and ordering.
pub fn farmland_push_distance(position: Position, bounds: Aabb) -> f64 {
    let difference = VoxelShape::from_aabb(farmland_push_region(position));
    1_f64
        + difference.collide(
            Axis::Y,
            bounds.moved(Vec3::new(0_f64, 1_f64, 0_f64)),
            -1_f64,
        )
}

fn offset(p: Position, dx: i32, dy: i32, dz: i32) -> Position {
    (
        p.0.wrapping_add(dx),
        p.1.wrapping_add(dy),
        p.2.wrapping_add(dz),
    )
}
fn block(world: &mut RuntimeWorld, p: Position) -> Result<u32> {
    world.block(p.0, p.1, p.2)
}
fn is_farmland(state: u32) -> Result<bool> {
    Ok(placement::placement_block_name(state)? == "minecraft:farmland")
}
fn same_crop(world: &mut RuntimeWorld, at: Position, kind: CropKind) -> Result<bool> {
    Ok(placement::placement_block_name(block(world, at)?)? == kind.name())
}

/// Exact BlockPos.betweenClosed iteration: X varies fastest, then Y, then Z.
/// Both the farmland's layer and the layer above it hydrate from any water tag.
pub fn is_near_water(world: &mut RuntimeWorld, position: Position) -> Result<bool> {
    for dz in -4..=4 {
        for dy in 0..=1 {
            for dx in -4..=4 {
                if FluidState::from_block(block(world, offset(position, dx, dy, dz))?)?.kind
                    == FluidKind::Water
                {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}
pub fn should_maintain_farmland(world: &mut RuntimeWorld, position: Position) -> Result<bool> {
    let state = block(world, offset(position, 0, 1, 0))?;
    placement::placement_block_name(state)?;
    Ok(placement::in_block_tag(
        state,
        "minecraft:maintains_farmland",
    ))
}
pub fn random_tick_farmland(
    world: &mut RuntimeWorld,
    state: u32,
    position: Position,
    random: &mut LegacyRandomSource,
    host: &mut impl FarmRandomTickHost,
) -> Result<()> {
    let moisture = farmland_moisture(state)?;
    if is_near_water(world, position)? || host.is_raining_at(world, offset(position, 0, 1, 0))? {
        if moisture < 7 {
            let next = block_state_with_properties(
                state,
                &BTreeMap::from([("moisture".to_owned(), "7".to_owned())]),
            )?;
            host.set_block(world, position, next, 2, random)?;
        }
    } else if moisture > 0 {
        let next = block_state_with_properties(
            state,
            &BTreeMap::from([("moisture".to_owned(), (moisture - 1).to_string())]),
        )?;
        host.set_block(world, position, next, 2, random)?;
    } else if !should_maintain_farmland(world, position)? {
        host.turn_farmland_to_dirt(world, position, state, random)?;
    }
    Ok(())
}

/// CropBlock.getGrowthSpeed: Java float additions in X-then-Z order. Same-kind
/// rows or any same-kind diagonal halve the final factor once, never twice.
pub fn crop_growth_speed(world: &mut RuntimeWorld, state: u32, position: Position) -> Result<f32> {
    let kind = CropKind::of(state)?;
    let mut speed = 1_f32;
    for dx in -1..=1 {
        for dz in -1..=1 {
            let soil = block(world, offset(position, dx, -1, dz))?;
            let mut contribution = 0_f32;
            if is_farmland(soil)? {
                contribution = if farmland_moisture(soil)? > 0 {
                    3_f32
                } else {
                    1_f32
                };
            }
            if dx != 0 || dz != 0 {
                contribution /= 4_f32;
            }
            speed += contribution;
        }
    }
    let west_east = same_crop(world, offset(position, -1, 0, 0), kind)?
        || same_crop(world, offset(position, 1, 0, 0), kind)?;
    let north_south = same_crop(world, offset(position, 0, 0, -1), kind)?
        || same_crop(world, offset(position, 0, 0, 1), kind)?;
    if west_east && north_south {
        speed /= 2_f32;
    } else if same_crop(world, offset(position, -1, 0, -1), kind)?
        || same_crop(world, offset(position, 1, 0, -1), kind)?
        || same_crop(world, offset(position, 1, 0, 1), kind)?
        || same_crop(world, offset(position, -1, 0, 1), kind)?
    {
        speed /= 2_f32;
    }
    Ok(speed)
}
pub fn random_tick_crop(
    world: &mut RuntimeWorld,
    state: u32,
    position: Position,
    random: &mut LegacyRandomSource,
    host: &mut impl CropRandomTickHost,
) -> Result<()> {
    let kind = CropKind::of(state)?;
    // BeetrootBlock.randomTick draws before entering ANY CropBlock branches.
    if kind == CropKind::Beetroots && random.next_bounded_i32(3)? == 0 {
        return Ok(());
    }
    if host.raw_brightness(world, position, 0)? < 9 {
        return Ok(());
    }
    let properties = crop_properties(state)?;
    if properties.age < properties.max_age {
        let speed = crop_growth_speed(world, state, position)?;
        let bound = (25_f32 / speed) as i32 + 1;
        if random.next_bounded_i32(bound)? == 0 {
            host.set_block(
                world,
                position,
                crop_state_for_age(state, properties.age + 1)?,
                2,
                random,
            )?;
        }
    }
    Ok(())
}

/// Native stable-light adapter with required level commit/rain/turnToDirt
/// callbacks. All callbacks share the actual light owner and Level.random.
pub struct NativeCropRandomTickHost<'a, Commit, Rain, Turn> {
    lighting: &'a mut NativeRuntimeLighting,
    commit: Commit,
    rain: Rain,
    turn_to_dirt: Turn,
}
impl<'a, Commit, Rain, Turn> NativeCropRandomTickHost<'a, Commit, Rain, Turn> {
    pub fn new(
        lighting: &'a mut NativeRuntimeLighting,
        commit: Commit,
        rain: Rain,
        turn_to_dirt: Turn,
    ) -> Self {
        Self {
            lighting,
            commit,
            rain,
            turn_to_dirt,
        }
    }
}
impl<Commit, Rain, Turn> CropRandomTickHost for NativeCropRandomTickHost<'_, Commit, Rain, Turn>
where
    Commit: FnMut(
        &mut RuntimeWorld,
        &mut NativeRuntimeLighting,
        Position,
        u32,
        u32,
        &mut LegacyRandomSource,
    ) -> Result<()>,
{
    fn raw_brightness(
        &mut self,
        world: &mut RuntimeWorld,
        position: Position,
        sky_darken: i32,
    ) -> Result<i32> {
        ensure!(
            (0..=15).contains(&sky_darken),
            "unsupported actual sky darkening"
        );
        let RuntimeWorld::Fixture(flat) = world else {
            bail!("native crop light adapter requires its actual flat-world source")
        };
        Ok(i32::from(self.lighting.raw_brightness(
            flat,
            position,
            sky_darken as u8,
        )?))
    }
    fn set_block(
        &mut self,
        world: &mut RuntimeWorld,
        position: Position,
        state: u32,
        flags: u32,
        random: &mut LegacyRandomSource,
    ) -> Result<()> {
        (self.commit)(world, self.lighting, position, state, flags, random)
    }
}
impl<Commit, Rain, Turn> FarmRandomTickHost for NativeCropRandomTickHost<'_, Commit, Rain, Turn>
where
    Commit: FnMut(
        &mut RuntimeWorld,
        &mut NativeRuntimeLighting,
        Position,
        u32,
        u32,
        &mut LegacyRandomSource,
    ) -> Result<()>,
    Rain: FnMut(&mut RuntimeWorld, &mut NativeRuntimeLighting, Position) -> Result<bool>,
    Turn: FnMut(
        &mut RuntimeWorld,
        &mut NativeRuntimeLighting,
        Position,
        u32,
        &mut LegacyRandomSource,
    ) -> Result<()>,
{
    fn is_raining_at(&mut self, world: &mut RuntimeWorld, position: Position) -> Result<bool> {
        (self.rain)(world, self.lighting, position)
    }
    fn turn_farmland_to_dirt(
        &mut self,
        world: &mut RuntimeWorld,
        position: Position,
        state: u32,
        random: &mut LegacyRandomSource,
    ) -> Result<()> {
        (self.turn_to_dirt)(world, self.lighting, position, state, random)
    }
}
