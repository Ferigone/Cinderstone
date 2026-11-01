//! Small executable integration example, not a substitute for the final suite.
use anyhow::{bail, Result};
use fluid_simulation::*;
use mc121_rust::fluid_simulation;
pub use mc121_rust::{
    fluid_physics, geometry, inventory, lighting, placement, random, runtime_world, tick,
};
use runtime_world::{RuntimeWorld, WorldChange};

#[derive(Default)]
struct Host {
    changes: Vec<WorldChange>,
    level_events: Vec<(i32, Position, i32)>,
}
impl FluidSimulationHost for Host {
    fn set_block(
        &mut self,
        world: &mut RuntimeWorld,
        p: Position,
        state: u32,
        _flags: u32,
    ) -> Result<Vec<WorldChange>> {
        world.edit_block(p.0, p.1, p.2, state)
    }
    fn before_destroying_block(
        &mut self,
        _: &mut RuntimeWorld,
        p: Position,
        _: u32,
        _: &mut random::LegacyRandomSource,
    ) -> Result<()> {
        bail!("example has no loot host at {p:?}")
    }
    fn level_event(&mut self, e: i32, p: Position, d: i32) -> Result<()> {
        self.level_events.push((e, p, d));
        Ok(())
    }
    fn place_custom_liquid(
        &mut self,
        _: &mut RuntimeWorld,
        p: Position,
        _: u32,
        _: fluid_physics::FluidState,
    ) -> Result<Vec<WorldChange>> {
        bail!("example has no custom liquid container at {p:?}")
    }
    fn block_changes(&mut self, c: &[WorldChange]) -> Result<()> {
        self.changes.extend_from_slice(c);
        Ok(())
    }
}
fn main() -> Result<()> {
    let mut world = RuntimeWorld::Fixture(mc121_rust::world::FlatWorld::new(0, 1, 10, 9, 39));
    let water = mc121_rust::inventory::block_state_with_properties(
        80,
        &std::collections::BTreeMap::from([("level".into(), "0".into())]),
    )?;
    let mut simulation = FluidSimulation::new();
    for x in -2..=2 {
        for z in -2..=2 {
            simulation.load_chunk((x, z), Vec::new(), 0)?;
        }
    }
    let old = world.block(8, 64, 8)?;
    let changes = world.edit_block(8, 64, 8, water)?;
    let mut host = Host::default();
    host.block_changes(&changes)?;
    let mut rng = random::LegacyRandomSource::new(12345);
    let mut order = 0;
    let mut context = FluidTickContext {
        world: &mut world,
        random: &mut rng,
        sub_tick_order: &mut order,
        host: &mut host,
        facts: FluidWorldFacts {
            ultrawarm: false,
            water_source_conversion: true,
            lava_source_conversion: false,
        },
        game_time: 0,
    };
    simulation.on_block_changed(&mut context, (8, 64, 8), old, water)?;
    let mut executed = 0;
    for now in 0..=40 {
        context.game_time = now;
        executed += simulation.tick_due(&mut context, 65536, |_| true)?.executed;
    }
    let east = context.world.block(9, 64, 8)?;
    anyhow::ensure!(
        fluid_physics::FluidState::from_block(east)?.kind == fluid_physics::FluidKind::Water,
        "water did not spread"
    );
    println!(
        "{{\"ok\":true,\"fluid_ticks\":{executed},\"block_changes\":{},\"pending\":{}}}",
        context.host.changes.len(),
        simulation.pending_count()
    );
    Ok(())
}
