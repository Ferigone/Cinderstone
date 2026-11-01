//! Original 1.21 SpreadingSnowyDirtBlock (dnq) grass/mycelium random ticks.
//! World writes execute synchronously between the four RNG attempts. Light
//! queries use the actual level sky darkening, never an assumed daytime value.
use crate::{
    fluid_physics::{FluidKind, FluidState},
    inventory::{block_default_state, block_state_property, block_state_with_properties},
    lighting, placement,
    random::LegacyRandomSource,
    runtime_lighting::NativeRuntimeLighting,
    runtime_world::RuntimeWorld,
};
use anyhow::{bail, ensure, Context, Result};
use std::collections::BTreeMap;

pub type Position = (i32, i32, i32);
// The pinned 1.21 registry's plain dirt block has exactly this single state.
const DIRT: u32 = 10;
// BlockGetter.getMaxLightLevel (dcc.Q) is the original constant return 15.
const MAX_LIGHT_LEVEL: i32 = 15;

/// set_block is Level.setBlockAndUpdate: flags3, all actual block/shape/fluid
/// callbacks, section-count updates and publication before returning. It may
/// consume the same Level.random passed here; subsequent attempts retain it.
pub trait GrassRandomTickHost {
    fn max_local_raw_brightness(
        &mut self,
        world: &mut RuntimeWorld,
        position: Position,
    ) -> Result<i32>;
    fn set_block(
        &mut self,
        world: &mut RuntimeWorld,
        position: Position,
        state: u32,
        flags: u32,
        random: &mut LegacyRandomSource,
    ) -> Result<()>;
}

fn above(p: Position) -> Position {
    (p.0, p.1.wrapping_add(1), p.2)
}
fn block(world: &mut RuntimeWorld, p: Position) -> Result<u32> {
    world.block(p.0, p.1, p.2)
}
fn is_snow(state: u32) -> Result<bool> {
    Ok(placement::placement_block_name(state)? == "minecraft:snow")
}

/// LightEngine.getLightBlockInto for the original upward grass/above edge.
/// eov.a first substitutes empty shapes for !canOcclude or !useShapeForLight-
/// Occlusion, then returns16 iff mergedFaceOccludes, else above.getLightBlock.
/// The pinned light catalogue contains those exact substituted directional
/// faces and context-checked opacity values for all 26684 original states.
pub fn get_light_block_into(state: u32, upper: u32) -> Result<i32> {
    Ok(if lighting::edge_occludes(state, upper, 1)? {
        16
    } else {
        i32::from(lighting::state_properties(upper)?.opacity)
    })
}

/// dnq.canBeGrass: one snow layer overrides both fluid/light checks; a fluid
/// amount8 (including falling fluid) kills grass before checking occlusion.
pub fn can_be_grass(world: &mut RuntimeWorld, state: u32, position: Position) -> Result<bool> {
    let upper = block(world, above(position))?;
    if is_snow(upper)? && block_state_property(upper, "layers").as_deref() == Some("1") {
        return Ok(true);
    }
    if FluidState::from_block(upper)?.amount == 8 {
        return Ok(false);
    }
    Ok(get_light_block_into(state, upper)? < MAX_LIGHT_LEVEL)
}

/// dnq.canPropagate deliberately re-reads the above fluid after canBeGrass.
pub fn can_propagate(world: &mut RuntimeWorld, state: u32, position: Position) -> Result<bool> {
    Ok(can_be_grass(world, state, position)?
        && FluidState::from_block(block(world, above(position))?)?.kind != FluidKind::Water)
}

/// Invoke with the actual sampled block state, even if a preceding callback
/// changed that cell. GrassBlock and MyceliumBlock share this exact method.
pub fn random_tick_spreading_snowy_dirt(
    world: &mut RuntimeWorld,
    state: u32,
    position: Position,
    random: &mut LegacyRandomSource,
    host: &mut impl GrassRandomTickHost,
) -> Result<()> {
    ensure!(
        matches!(
            placement::placement_block_name(state)?,
            "minecraft:grass_block" | "minecraft:mycelium"
        ),
        "sampled state does not own SpreadingSnowyDirtBlock.randomTick"
    );
    if !can_be_grass(world, state, position)? {
        return host.set_block(world, position, DIRT, 3, random);
    }
    if host.max_local_raw_brightness(world, above(position))? < 9 {
        return Ok(());
    }
    let placing = block_default_state(state).context("missing spreading block default state")?;
    for _ in 0..4 {
        // JVM evaluation order is X draw3, Y draw5, Z draw3, then world reads
        // and any synchronous callbacks before the next attempt's first draw.
        let at = (
            position.0.wrapping_add(random.next_bounded_i32(3)? - 1),
            position.1.wrapping_add(random.next_bounded_i32(5)? - 3),
            position.2.wrapping_add(random.next_bounded_i32(3)? - 1),
        );
        if block(world, at)? != DIRT || !can_propagate(world, placing, at)? {
            continue;
        }
        let snowy = is_snow(block(world, above(at))?)?;
        let next = block_state_with_properties(
            placing,
            &BTreeMap::from([("snowy".to_owned(), snowy.to_string())]),
        )?;
        host.set_block(world, at, next, 3, random)?;
    }
    Ok(())
}

/// Native flat-world adapter. The required commit callback receives this same
/// lighting owner and RNG so it can execute the embedding level's real write
/// pipeline without a second light cache or independently seeded stream.
pub struct NativeGrassRandomTickHost<'a, Commit> {
    lighting: &'a mut NativeRuntimeLighting,
    sky_darken: u8,
    commit: Commit,
}
impl<'a, Commit> NativeGrassRandomTickHost<'a, Commit> {
    pub fn new(
        lighting: &'a mut NativeRuntimeLighting,
        sky_darken: u8,
        commit: Commit,
    ) -> Result<Self> {
        ensure!(sky_darken <= 15, "invalid actual level sky darkening");
        Ok(Self {
            lighting,
            sky_darken,
            commit,
        })
    }
}
impl<Commit> GrassRandomTickHost for NativeGrassRandomTickHost<'_, Commit>
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
    fn max_local_raw_brightness(&mut self, world: &mut RuntimeWorld, at: Position) -> Result<i32> {
        let RuntimeWorld::Fixture(flat) = world else {
            bail!("native grass light adapter requires its actual flat-world source");
        };
        Ok(i32::from(self.lighting.raw_brightness(
            flat,
            at,
            self.sky_darken,
        )?))
    }
    fn set_block(
        &mut self,
        world: &mut RuntimeWorld,
        at: Position,
        state: u32,
        flags: u32,
        random: &mut LegacyRandomSource,
    ) -> Result<()> {
        (self.commit)(world, self.lighting, at, state, flags, random)
    }
}
