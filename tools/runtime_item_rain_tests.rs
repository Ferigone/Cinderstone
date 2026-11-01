use super::*;
use crate::{
    entity::Position, inventory::ItemStack, item_entity::ItemState, random::LegacyRandomSource,
    world::FlatWorld,
};
struct Host;
impl RuntimeItemHost for Host {
    fn position_changed(&mut self, _: &EntityPhysicalState) -> Result<()> {
        bail!("no move in rain predicate probe")
    }
    fn profile(&mut self, _: ProfileEvent) {
        panic!("no movement profile")
    }
    fn effect(&mut self, _: &EntityPhysicalState, _: RuntimeItemEffect) -> Result<()> {
        bail!("no effect in rain predicate probe")
    }
    fn entity_damage(
        &mut self,
        _: &EntityPhysicalState,
        _: &ItemStack,
        _: i32,
        _: Option<i32>,
    ) -> Result<()> {
        bail!("no hurt in rain predicate probe")
    }
    fn discarded(&mut self, _: &EntityPhysicalState, _: &ItemStack, _: i32) -> Result<()> {
        bail!("no discard in rain predicate probe")
    }
}
struct Exposure {
    calls: usize,
    answer: bool,
}
impl RainExposure for Exposure {
    fn is_in_rain(&mut self, _: &mut RuntimeWorld, _: &EntityPhysicalState) -> Result<bool> {
        self.calls += 1;
        Ok(self.answer)
    }
}
fn item() -> ItemState {
    ItemState::new(
        1,
        [0; 16],
        Position {
            x: 0.5,
            y: 64.,
            z: 0.5,
            yaw: 0.,
            pitch: 0.,
            on_ground: false,
        },
        Vec3::ZERO,
        ItemStack::new(1, 1).unwrap(),
    )
    .unwrap()
}
fn facts(rain: RainAvailability) -> ItemWorldFacts {
    ItemWorldFacts {
        game_time: 1,
        minimum_build_height: MIN_Y,
        ultrawarm: false,
        border: CollisionBorder::default(),
        rain,
        has_passengers: false,
    }
}

#[test]
fn water_and_actual_non_raining_weather_short_circuit_optional_exposure() {
    let mut world = RuntimeWorld::Fixture(FlatWorld::new(0, 1, 10, 9, 39));
    let mut item = item();
    let mut random = LegacyRandomSource::new(1);
    let mut host = Host;
    let mut exposure = Exposure {
        calls: 0,
        answer: true,
    };
    item.was_touching_water = true;
    let mut provider = RuntimeItemWorld::new(
        &mut world,
        &item.physical,
        &mut random,
        &mut host,
        facts(RainAvailability::ExposureUnavailable),
        &[],
    )
    .unwrap()
    .with_rain_exposure(&mut exposure);
    assert!(provider.water_rain_or_bubble(&item.physical).unwrap());
    assert_eq!(exposure.calls, 0);
    item.was_touching_water = false;
    let mut provider = RuntimeItemWorld::new(
        &mut world,
        &item.physical,
        &mut random,
        &mut host,
        facts(RainAvailability::NotRaining),
        &[],
    )
    .unwrap()
    .with_rain_exposure(&mut exposure);
    assert!(!provider.water_rain_or_bubble(&item.physical).unwrap());
    assert_eq!(exposure.calls, 0);
}

#[test]
fn raining_requires_capability_and_preserves_rain_then_bubble_order() {
    let mut world = RuntimeWorld::Fixture(FlatWorld::new(0, 1, 10, 9, 39));
    let item = item();
    let mut random = LegacyRandomSource::new(1);
    let mut host = Host;
    assert!(RuntimeItemWorld::new(
        &mut world,
        &item.physical,
        &mut random,
        &mut host,
        facts(RainAvailability::ExposureUnavailable),
        &[]
    )
    .unwrap()
    .water_rain_or_bubble(&item.physical)
    .is_err());
    let mut exposure = Exposure {
        calls: 0,
        answer: true,
    };
    assert!(RuntimeItemWorld::new(
        &mut world,
        &item.physical,
        &mut random,
        &mut host,
        facts(RainAvailability::ExposureUnavailable),
        &[]
    )
    .unwrap()
    .with_rain_exposure(&mut exposure)
    .water_rain_or_bubble(&item.physical)
    .unwrap());
    assert_eq!(exposure.calls, 1);
    exposure.answer = false;
    assert!(!RuntimeItemWorld::new(
        &mut world,
        &item.physical,
        &mut random,
        &mut host,
        facts(RainAvailability::ExposureUnavailable),
        &[]
    )
    .unwrap()
    .with_rain_exposure(&mut exposure)
    .water_rain_or_bubble(&item.physical)
    .unwrap());
    assert_eq!(exposure.calls, 2);
    let bubble = (0..26684)
        .find(|id| entity_move::block_facts(*id).unwrap().bubble_column())
        .unwrap();
    let RuntimeWorld::Fixture(flat) = &mut world else {
        unreachable!()
    };
    flat.set_block(0, 64, 0, bubble);
    assert!(RuntimeItemWorld::new(
        &mut world,
        &item.physical,
        &mut random,
        &mut host,
        facts(RainAvailability::ExposureUnavailable),
        &[]
    )
    .unwrap()
    .with_rain_exposure(&mut exposure)
    .water_rain_or_bubble(&item.physical)
    .unwrap());
    assert_eq!(exposure.calls, 3);
}
