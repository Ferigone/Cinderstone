use super::*;
use crate::{
    entity::Position, fluid_physics::FluidShapeOwner, item_entity::ItemState,
    random::LegacyRandomSource, world::FlatWorld,
};
use serde_json::Value;

struct Host;
impl RuntimeItemHost for Host {
    fn position_changed(&mut self, _: &EntityPhysicalState) -> Result<()> {
        bail!("ray must not move entity")
    }
    fn profile(&mut self, _: ProfileEvent) {}
    fn effect(&mut self, _: &EntityPhysicalState, _: RuntimeItemEffect) -> Result<()> {
        bail!("ray must not emit an effect")
    }
    fn entity_damage(
        &mut self,
        _: &EntityPhysicalState,
        _: &ItemStack,
        _: i32,
        _: Option<i32>,
    ) -> Result<()> {
        bail!("ray must not damage entity")
    }
    fn discarded(&mut self, _: &EntityPhysicalState, _: &ItemStack, _: i32) -> Result<()> {
        bail!("ray must not discard entity")
    }
}
fn set(world: &mut RuntimeWorld, p: (i32, i32, i32), block: u32) {
    let RuntimeWorld::Fixture(world) = world else {
        panic!()
    };
    world.set_block(p.0, p.1, p.2, block);
}
fn ray(world: &mut RuntimeWorld, cache: &mut FluidShapeCache) -> Result<bool> {
    let item = ItemState::new(
        17,
        [0; 16],
        Position {
            x: 0.5,
            y: 65.,
            z: 0.5,
            yaw: 0.,
            pitch: 0.,
            on_ground: false,
        },
        Vec3::ZERO,
        ItemStack::new(1, 1)?,
    )?;
    let mut random = LegacyRandomSource::new(12345);
    let mut host = Host;
    let facts = ItemWorldFacts {
        game_time: 1,
        minimum_build_height: MIN_Y,
        ultrawarm: false,
        border: CollisionBorder::default(),
        rain: RainAvailability::NotRaining,
        has_passengers: false,
    };
    let mut provider =
        RuntimeItemWorld::new(world, &item.physical, &mut random, &mut host, facts, &[])?
            .with_fluid_shape_cache(cache);
    provider.fall_damage_resetting_ray(
        &item.physical,
        Vec3::new(-0.25, 65.94, 0.5),
        Vec3::new(1.25, 65.94, 0.5),
    )
}

#[test]
fn guarded_water_ray_retains_shared_first_query_history_across_worlds_and_aliases() {
    let fixture = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/fluid-shape-vectors.json"
    ))
    .unwrap();
    let fixture: Value = serde_json::from_str(&fixture).unwrap();
    for scene in fixture["scenes"].as_array().unwrap() {
        assert!(scene["initial_cache"]
            .as_array()
            .unwrap()
            .iter()
            .all(|owner| owner["entries"].as_array().unwrap().is_empty()));
    }
    let water = fixture["aliases"]["water_source_block_state"]
        .as_u64()
        .unwrap() as u32;
    let logged = fixture["aliases"]["waterlogged_block_state"]
        .as_u64()
        .unwrap() as u32;
    let mut world = RuntimeWorld::Fixture(FlatWorld::new(0, 1, 10, 9, 39));
    set(&mut world, (0, 65, 0), water);
    assert!(ray(&mut world, &mut FluidShapeCache::unknown())
        .unwrap_err()
        .to_string()
        .contains("history unknown"));
    let mut shallow = FluidShapeCache::from_observed_empty_owners(FluidShapeOwner::ALL);
    assert!(!ray(&mut world, &mut shallow).unwrap());
    set(&mut world, (0, 66, 0), water);
    assert!(
        !ray(&mut world, &mut shallow).unwrap(),
        "cached shallow shape cannot become full from later above water"
    );
    // This is a distinct observed fresh-process history, not a reset of the
    // first cache. Above water primes the source shape to a full block.
    let mut full = FluidShapeCache::from_observed_empty_owners(FluidShapeOwner::ALL);
    assert!(ray(&mut world, &mut full).unwrap());
    set(&mut world, (0, 66, 0), 0);
    assert!(
        ray(&mut world, &mut full).unwrap(),
        "cached full shape cannot shrink from later air"
    );
    let mut other_world = RuntimeWorld::Fixture(FlatWorld::new(0, 1, 10, 9, 39));
    set(&mut other_world, (0, 65, 0), logged);
    assert!(
        ray(&mut other_world, &mut full).unwrap(),
        "waterlogged source aliases share the original fluid identity across worlds"
    );
    let flowing = (0..26684)
        .find(|block| {
            let fluid = fluid_physics::block_facts(*block).unwrap().fluid;
            fluid.kind == FluidKind::Water && !fluid.source && fluid.amount == 8
        })
        .unwrap();
    set(&mut other_world, (0, 65, 0), flowing);
    assert!(
        !ray(&mut other_world, &mut full).unwrap(),
        "flowing owner must not reuse source-owner full shape"
    );
}
