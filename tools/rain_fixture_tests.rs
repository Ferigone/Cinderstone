use super::*;
use crate::{
    entity::Position, geometry::Vec3, rain_physics::level_is_raining_at, world::FlatWorld,
};

struct Availability(bool);
impl RainChunkAvailability for Availability {
    fn require_settled_unedited_fixture(&self) -> Result<()> {
        Ok(())
    }
    fn require_current_full(&self, _: (i32, i32)) -> Result<()> {
        ensure!(self.0, "controlled current FULL source unavailable");
        Ok(())
    }
}
fn state(y: f64) -> EntityPhysicalState {
    EntityPhysicalState::new(
        1,
        [0; 16],
        Position {
            x: 0.5,
            y,
            z: 0.5,
            yaw: 0.,
            pitch: 0.,
            on_ground: false,
        },
        Vec3::ZERO,
        0.25,
        0.25,
        0.2125,
    )
    .unwrap()
}
fn world() -> RuntimeWorld {
    RuntimeWorld::Fixture(FlatWorld::new(0, 1, 10, 9, 39))
}

#[test]
fn literal_temperature_matches_genuine_bootstrapped_plains_ieee_bits() {
    let source: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/rain-fixture-facts.json")).unwrap();
    let probes = source["plains_temperature_precipitation_probes"]
        .as_array()
        .unwrap();
    assert_eq!(probes.len(), 1542);
    let mut provider = FreshFlatPlainsRain::from_observed_empty_temperature_history();
    for (index, row) in probes.iter().enumerate() {
        let p = (
            row[0].as_i64().unwrap() as i32,
            row[1].as_i64().unwrap() as i32,
            row[2].as_i64().unwrap() as i32,
        );
        let actual = provider.temperature(p);
        assert_eq!(
            format!("{:08x}", actual.to_bits()),
            row[3].as_str().unwrap(),
            "original temperature probe{index} {p:?}"
        );
        assert_eq!(
            if actual < 0.15 { "SNOW" } else { "RAIN" },
            row[4].as_str().unwrap()
        );
    }
    assert_eq!(provider.temperatures.len(), 1024);
    assert_eq!(provider.insertion.len(), 1024);
    println!("RAIN_FIXTURE_TEMPERATURE_CONFORMANCE: {{\"ok\":true,\"original_ieee_temperatures\":1542,\"mismatches\":0}}");
}

#[test]
fn concrete_exposure_uses_measured_sky_and_second_cached_xz_body_point() {
    let mut provider = FreshFlatPlainsRain::from_observed_empty_temperature_history();
    let available = Availability(true);
    let mut frame = provider.frame(&available);
    let mut world = world();
    for y in [
        -80., -64., 0., 60., 63., 63.5, 64., 80., 81., 319., 320., 335.,
    ] {
        let body = state(y);
        let expected = body.block_position.1 >= 64 || java_floor(body.bounds.max_y) >= 64;
        assert_eq!(frame.is_in_rain(&mut world, &body).unwrap(), expected);
    }
    let mut body = state(100.);
    body.block_position = (1, 63, -1);
    body.bounds.max_y = 64.01;
    assert!(frame.is_in_rain(&mut world, &body).unwrap());
    body.bounds.max_y = 63.99;
    assert!(!frame.is_in_rain(&mut world, &body).unwrap());
}

#[test]
fn actual_weather_gate_composes_with_genuine_fixture_level_rain_results() {
    let source: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/rain-fixture-facts.json")).unwrap();
    let rows = source["actual_level_rain_cases"].as_array().unwrap();
    assert_eq!(rows.len(), 75);
    let available = Availability(true);
    let mut owner = FreshFlatPlainsRain::from_observed_empty_temperature_history();
    for (index, row) in rows.iter().enumerate() {
        let rain = bits32(row[0].as_str().unwrap());
        let weather = crate::tick::WeatherState {
            rain_level: rain,
            old_rain_level: rain,
            ..Default::default()
        };
        let mut queries = FixtureRainQueries {
            owner: &mut owner,
            chunks: &available,
        };
        let actual = weather.is_raining()
            && level_is_raining_at(&mut queries, (8, row[1].as_i64().unwrap() as i32, 8)).unwrap();
        assert_eq!(
            actual,
            row[2].as_bool().unwrap(),
            "genuine fixture original Level rain case{index}"
        );
    }
    println!("RAIN_FIXTURE_LEVEL_CONFORMANCE: {{\"ok\":true,\"original_settled_level_cases\":75,\"mismatches\":0}}");
}

#[test]
fn concrete_exposure_rejects_unknown_source_facts_edits_and_missing_current_full() {
    let mut provider = FreshFlatPlainsRain::from_observed_empty_temperature_history();
    let available = Availability(true);
    let missing = Availability(false);
    assert!(provider
        .frame(&missing)
        .is_in_rain(&mut world(), &state(64.))
        .is_err());
    struct Invalidated;
    impl RainChunkAvailability for Invalidated {
        fn require_settled_unedited_fixture(&self) -> Result<()> {
            anyhow::bail!("actual lighting witness invalidated")
        }
        fn require_current_full(&self, _: (i32, i32)) -> Result<()> {
            Ok(())
        }
    }
    assert!(provider
        .frame(&Invalidated)
        .is_in_rain(&mut world(), &state(64.))
        .is_err());
    let mut wrong_biome = RuntimeWorld::Fixture(FlatWorld::new(0, 1, 10, 9, 0));
    assert!(provider
        .frame(&available)
        .is_in_rain(&mut wrong_biome, &state(64.))
        .is_err());
    let mut wrong_base = RuntimeWorld::Fixture(FlatWorld::new(0, 2, 10, 9, 39));
    assert!(provider
        .frame(&available)
        .is_in_rain(&mut wrong_base, &state(64.))
        .is_err());
    let mut edited = world();
    let RuntimeWorld::Fixture(flat) = &mut edited else {
        unreachable!()
    };
    flat.set_block(2, 64, 3, 1);
    assert!(provider
        .frame(&available)
        .is_in_rain(&mut edited, &state(64.))
        .is_err());
    assert!(provider
        .frame(&available)
        .is_in_rain(&mut world(), &state(337.))
        .is_err());
    assert!(provider
        .frame(&available)
        .is_in_rain(&mut world(), &state(-81.))
        .is_err());
    let mut outside = state(64.);
    outside.block_position = (30_000_000, 64, 0);
    assert!(provider
        .frame(&available)
        .is_in_rain(&mut world(), &outside)
        .is_err());
}
