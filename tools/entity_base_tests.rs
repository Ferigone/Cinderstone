//! Differential world callbacks match only the explicitly injected Java oracle
//! context. These fixtures do not replace full world/environment conformance.
use super::*;
use crate::{
    entity::Position,
    item_entity::{block_physics, fluid_height, FluidKind},
    random::LegacyRandomSource,
};
use anyhow::{bail, ensure};
use serde_json::{json, Value};

fn f(value: &Value) -> f32 {
    f32::from_bits(u32::from_str_radix(value.as_str().unwrap(), 16).unwrap())
}
fn d(value: &Value) -> f64 {
    f64::from_bits(u64::from_str_radix(value.as_str().unwrap(), 16).unwrap())
}
fn fb(value: f32) -> String {
    format!("{:08x}", value.to_bits())
}
fn db(value: f64) -> String {
    format!("{:016x}", value.to_bits())
}
fn flag(q: &Value, name: &str) -> bool {
    q[name].as_bool().unwrap()
}

struct TestWorld<'a> {
    query: &'a Value,
    events: Vec<String>,
    random: LegacyRandomSource,
    sound: Option<Value>,
}
impl TestWorld<'_> {
    fn event(&mut self, name: &str) {
        self.events.push(name.into());
    }
}
impl EntityBaseWorld for TestWorld<'_> {
    fn is_client_side(&self) -> bool {
        flag(self.query, "client")
    }
    fn is_server_level(&self) -> bool {
        flag(self.query, "server_level")
    }
    fn minimum_build_height(&self) -> i32 {
        self.query["min_y"].as_i64().unwrap() as i32
    }
    fn ultrawarm(&self) -> bool {
        flag(self.query, "ultrawarm")
    }
    fn vehicle_removed(&mut self, _: i32) -> Result<bool> {
        Ok(flag(self.query, "vehicle_removed"))
    }
    fn tick_portal(&mut self, _: &mut EntityPhysicalState) -> Result<()> {
        if flag(self.query, "portal_active") && self.is_server_level() {
            self.event("portal");
        }
        Ok(())
    }
    fn can_spawn_sprint_particle(&mut self, _: &EntityPhysicalState) -> Result<bool> {
        Ok(flag(self.query, "sprint_particles"))
    }
    fn update_swimming(&mut self, _: &mut EntityPhysicalState) -> Result<()> {
        self.event("swimming");
        Ok(())
    }
    fn process_portal(&mut self, _: &mut EntityPhysicalState) -> Result<()> {
        bail!("oracle replaces the complete portal virtual hook")
    }
    fn spawn_sprint_particle(&mut self, _: &mut EntityPhysicalState) -> Result<()> {
        self.event("sprint_particle");
        Ok(())
    }
    fn boat_fluid_state(&mut self, _: i32) -> Result<Option<BoatFluidState>> {
        Ok(flag(self.query, "boat").then(|| BoatFluidState {
            under_water: flag(self.query, "boat_underwater"),
            bounds: Aabb::new(
                7.,
                d(&self.query["boat_min_y"]),
                7.,
                9.,
                d(&self.query["boat_max_y"]),
                9.,
            ),
        }))
    }
    fn update_fluid(
        &mut self,
        state: &EntityPhysicalState,
        tag: FluidTag,
        push: f64,
    ) -> Result<FluidUpdate> {
        let (name, field, expected) = match tag {
            FluidTag::Water => ("fluid_water", "water_depth", 0.014_f64),
            FluidTag::Lava => (
                "fluid_lava",
                "lava_depth",
                if self.ultrawarm() {
                    0.007_f64
                } else {
                    0.0023333333333333335
                },
            ),
        };
        ensure!(
            push.to_bits() == expected.to_bits(),
            "original fluid push coefficient"
        );
        self.event(name);
        let depth = d(&self.query[field]);
        Ok(FluidUpdate {
            touching: depth > 0.0,
            velocity: state.velocity,
            height_written: Some(depth),
        })
    }
    fn water_splash(&mut self, _: &mut EntityPhysicalState) -> Result<()> {
        self.event("water_splash");
        Ok(())
    }
    fn eye_fluid_sample(&mut self, _: f64, y: f64, _: f64) -> Result<EyeFluidSample> {
        let state = self.query["eye_fluid"].as_u64().unwrap() as u32;
        let tags = match block_physics(state)?.fluid {
            FluidKind::Water => FluidTag::Water.mask(),
            FluidKind::Lava => FluidTag::Lava.mask(),
            FluidKind::None => 0,
        };
        // Oracle Level supplies the same original FluidState above this cell.
        // Original H adds block Y and f32 fluid height BEFORE f64 promotion.
        let height = fluid_height(state, state)?;
        let surface = f64::from((y.floor() as i32) as f32 + height);
        Ok(EyeFluidSample { surface, tags })
    }
    fn block_fluid_tags(&mut self, _: (i32, i32, i32)) -> Result<u8> {
        bail!("oracle replaces the complete swimming virtual hook")
    }
    fn next_entity_float(&mut self, _: &EntityPhysicalState) -> Result<f32> {
        Ok(self.random.next_f32())
    }
    fn effect(&mut self, _: &mut EntityPhysicalState, effect: BaseEffect) -> Result<()> {
        match effect {
            BaseEffect::StopRiding { .. } => self.event("stop_riding"),
            BaseEffect::MeltFrozen { event, data, .. } => {
                ensure!(event == 1009 && data == 1, "original melting event");
                self.event("melt_frozen");
            }
            BaseEffect::LavaBurnSound { volume, pitch } => {
                self.event("lava_sound");
                self.sound = Some(json!({"volume": fb(volume), "pitch": fb(pitch)}));
            }
            BaseEffect::DiscardBelowWorld => self.event("discard_below_world"),
        }
        Ok(())
    }
    fn tick_leash(&mut self, _: &mut EntityPhysicalState) -> Result<()> {
        self.event("leash_empty");
        Ok(())
    }
}

fn physical(q: &Value) -> EntityPhysicalState {
    let position = Position {
        x: 8.,
        y: d(&q["y"]),
        z: 8.,
        yaw: f(&q["yaw"]),
        pitch: f(&q["pitch"]),
        on_ground: false,
    };
    let mut s = EntityPhysicalState::new(
        1,
        [0; 16],
        position,
        Vec3::ZERO,
        0.25,
        0.25,
        f(&q["eye_height"]),
    )
    .unwrap();
    s.boarding_cooldown = q["boarding_cooldown"].as_i64().unwrap() as i32;
    s.remaining_fire_ticks = q["fire_ticks"].as_i64().unwrap() as i32;
    s.tick_count = q["tick_count"].as_i64().unwrap() as i32;
    s.portal_cooldown = q["portal_cooldown"].as_i64().unwrap() as i32;
    s.walk_dist_old = f(&q["walk_dist_o"]);
    s.walk_dist = f(&q["walk_dist"]);
    s.old_yaw = f(&q["yaw_o"]);
    s.old_pitch = f(&q["pitch_o"]);
    s.fall_distance = f(&q["fall_distance"]);
    s.is_in_powder_snow = flag(q, "in_powder_snow");
    s.was_in_powder_snow = flag(q, "was_in_powder_snow");
    s.first_tick = flag(q, "first_tick");
    s.was_touching_water = flag(q, "touching_water");
    s.was_eye_in_water = flag(q, "was_eye_in_water");
    s.visual_fire = flag(q, "visual_fire");
    s.fire_immune = flag(q, "fire_immune");
    s.silent = flag(q, "silent");
    s.leashable = flag(q, "leashable");
    s.shared_flags = q["flags"].as_u64().unwrap() as u8;
    s.ticks_frozen = q["frozen_ticks"].as_i64().unwrap() as i32;
    s.pose = q["pose"].as_u64().unwrap_or(0) as u8;
    s.is_spectator = q["spectator"].as_bool().unwrap_or(false);
    s.removed = q["removed"].as_bool().unwrap_or(false);
    s.metadata_dirty = q["pre_dirty"].as_u64().unwrap_or(0) as u16;
    s.vehicle = flag(q, "vehicle_present").then_some(2);
    s.cached_inside_block_state = Some(1);
    s.fluid_on_eyes = q["eye_tags"]
        .as_array()
        .unwrap()
        .iter()
        .fold(0, |mask, tag| mask | if tag == "water" { 1 } else { 2 });
    s
}
fn snapshot(s: &EntityPhysicalState) -> Value {
    let mut tags = Vec::new();
    if s.fluid_on_eyes & 1 != 0 {
        tags.push("water");
    }
    if s.fluid_on_eyes & 2 != 0 {
        tags.push("lava");
    }
    json!({"boarding_cooldown":s.boarding_cooldown,"fire_ticks":s.remaining_fire_ticks,"tick_count":s.tick_count,"portal_cooldown":s.portal_cooldown,
        "walk_dist_o":fb(s.walk_dist_old),"walk_dist":fb(s.walk_dist),"yaw_o":fb(s.old_yaw),"pitch_o":fb(s.old_pitch),"fall_distance":fb(s.fall_distance),
        "in_powder_snow":s.is_in_powder_snow,"was_in_powder_snow":s.was_in_powder_snow,"first_tick":s.first_tick,"touching_water":s.was_touching_water,"was_eye_in_water":s.was_eye_in_water,
        "vehicle_present":s.vehicle.is_some(),"removed":s.removed,"cache_cleared":s.cached_inside_block_state.is_none(),"frozen_ticks":s.ticks_frozen,"flags":s.shared_flags,"eye_tags":tags,
        "fluid_depths":[db(s.fluid_heights[0]),db(s.fluid_heights[1])],"dirty_mask":s.metadata_dirty})
}

#[test]
fn actual_java_entity_base_tick_state_and_ordered_effects() {
    let vectors: Value =
        serde_json::from_str(include_str!("../fixtures/entity-base-vectors.json")).unwrap();
    for (index, case) in vectors["cases"].as_array().unwrap().iter().enumerate() {
        let q = &case["query"];
        if q["op"] == "defaults" {
            continue;
        }
        let expected = &case["expected"];
        let mut state = physical(q);
        let mut world = TestWorld {
            query: q,
            events: Vec::new(),
            random: LegacyRandomSource::new(q["seed"].as_i64().unwrap()),
            sound: None,
        };
        base_tick(&mut state, &mut world, |_, world, damage, amount| {
            let (name, expected) = match damage {
                BaseDamage::OnFire => ("hurt_fire", 1.0_f32),
                BaseDamage::Lava => ("hurt_lava", 4.0_f32),
            };
            ensure!(amount.to_bits() == expected.to_bits());
            world.event(name);
            Ok(flag(world.query, "hurt_result"))
        })
        .unwrap();
        assert_eq!(snapshot(&state), expected["state"], "state case{index}");
        assert_eq!(
            json!(world.events),
            expected["events"],
            "ordered effects case{index}"
        );
        assert_eq!(
            world.random.next_i32(),
            expected["rng_next_int"].as_i64().unwrap() as i32,
            "RNG case{index}"
        );
        assert_eq!(
            world.sound,
            expected.get("sound").cloned(),
            "sound case{index}"
        );
    }
}

struct DefaultWorld<'a>(&'a Value);
fn unavailable<T>() -> Result<T> {
    bail!("unexpected environment hook in default decision oracle")
}
impl EntityBaseWorld for DefaultWorld<'_> {
    fn is_client_side(&self) -> bool {
        flag(self.0, "client")
    }
    fn is_server_level(&self) -> bool {
        flag(self.0, "server_level")
    }
    fn minimum_build_height(&self) -> i32 {
        self.0["min_y"].as_i64().unwrap() as i32
    }
    fn ultrawarm(&self) -> bool {
        flag(self.0, "ultrawarm")
    }
    fn vehicle_removed(&mut self, _: i32) -> Result<bool> {
        unavailable()
    }
    fn process_portal(&mut self, _: &mut EntityPhysicalState) -> Result<()> {
        unavailable()
    }
    fn spawn_sprint_particle(&mut self, _: &mut EntityPhysicalState) -> Result<()> {
        unavailable()
    }
    fn boat_fluid_state(&mut self, _: i32) -> Result<Option<BoatFluidState>> {
        unavailable()
    }
    fn update_fluid(
        &mut self,
        _: &EntityPhysicalState,
        _: FluidTag,
        _: f64,
    ) -> Result<FluidUpdate> {
        unavailable()
    }
    fn water_splash(&mut self, _: &mut EntityPhysicalState) -> Result<()> {
        unavailable()
    }
    fn eye_fluid_sample(&mut self, _: f64, _: f64, _: f64) -> Result<EyeFluidSample> {
        unavailable()
    }
    fn block_fluid_tags(&mut self, _: (i32, i32, i32)) -> Result<u8> {
        Ok(
            match block_physics(self.0["eye_fluid"].as_u64().unwrap() as u32)?.fluid {
                FluidKind::Water => FluidTag::Water.mask(),
                FluidKind::Lava => FluidTag::Lava.mask(),
                FluidKind::None => 0,
            },
        )
    }
    fn next_entity_float(&mut self, _: &EntityPhysicalState) -> Result<f32> {
        unavailable()
    }
    fn effect(&mut self, _: &mut EntityPhysicalState, _: BaseEffect) -> Result<()> {
        unavailable()
    }
    fn tick_leash(&mut self, _: &mut EntityPhysicalState) -> Result<()> {
        unavailable()
    }
}

#[test]
fn actual_java_default_swimming_and_sprint_particle_decisions() {
    let vectors: Value =
        serde_json::from_str(include_str!("../fixtures/entity-base-vectors.json")).unwrap();
    let mut count = 0;
    for (index, case) in vectors["cases"].as_array().unwrap().iter().enumerate() {
        let q = &case["query"];
        if q["op"] != "defaults" {
            continue;
        }
        let mut state = physical(q);
        state.fluid_heights[1] = d(&q["lava_depth"]);
        let mut world = DefaultWorld(q);
        let sprint = world.can_spawn_sprint_particle(&state).unwrap();
        world.update_swimming(&mut state).unwrap();
        assert_eq!(
            json!({"sprint":sprint,"flags":state.shared_flags,"dirty_mask":state.metadata_dirty}),
            case["expected"],
            "default decision case{index}"
        );
        count += 1;
    }
    assert!(count > 1000, "default matrix missing");
}
