//! Native unmounted creative Player.tick / LivingEntity.aiStep pipeline.
//! World effects and phases are mandatory host callbacks. Active effects,
//! vehicles, elytra, climbing and powder-snow behavior remain explicit errors.
use crate::{
    entity_base,
    entity_move::{self, EntityMoveContext, EntityMoveWorld, MovementEmission, MoverType},
    geometry::{self, Vec3},
    main_player_motion::{PlayerMotion, PlayerMotionFrame, PlayerMoveWorld},
    player_body_world::PlayerBodyWorldHooks,
    runtime_collision::{CollisionBorder, CollisionEntity},
    runtime_world::RuntimeWorld,
};
use anyhow::{ensure, Result};
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Deserialize)]
struct Facts {
    friction_bits: Vec<u32>,
    inventory_tick_default: Vec<bool>,
    sprint_render_visible: Vec<bool>,
    sounds: std::collections::BTreeMap<String, i32>,
}
fn facts() -> &'static Facts {
    static F: OnceLock<Facts> = OnceLock::new();
    F.get_or_init(|| {
        serde_json::from_str(include_str!("../fixtures/player-body-facts.json"))
            .expect("original Player body facts")
    })
}
pub fn inventory_tick_is_default(id: i32) -> bool {
    usize::try_from(id)
        .ok()
        .and_then(|i| facts().inventory_tick_default.get(i))
        .copied()
        .unwrap_or(false)
}
pub fn sound_id(name: &str) -> Result<i32> {
    facts()
        .sounds
        .get(name)
        .copied()
        .ok_or_else(|| anyhow::anyhow!("unrecorded native player sound {name}"))
}
pub fn friction(state: u32) -> Result<f32> {
    facts()
        .friction_bits
        .get(state as usize)
        .map(|n| f32::from_bits(*n))
        .ok_or_else(|| anyhow::anyhow!("unknown friction state {state}"))
}
pub fn sprint_render_visible(state: u32) -> Result<bool> {
    facts()
        .sprint_render_visible
        .get(state as usize)
        .copied()
        .ok_or_else(|| anyhow::anyhow!("unknown sprint render state {state}"))
}

#[derive(Clone, Debug)]
pub struct PlayerBodyState {
    pub rot_a: f32,
    pub time_offset: f32,
    pub constructor_yaw: f32,
    pub swing_time: i32,
    pub swinging: bool,
    pub swinging_hand: u8,
    pub attack_strength_ticker: i32,
    pub no_jump_delay: i32,
    pub cooldown_tick_count: i32,
    pub jumping: bool,
    pub xxa: f32,
    pub yya: f32,
    pub zza: f32,
    pub swim_amount: f32,
    pub old_swim_amount: f32,
    pub last_main_hand: crate::inventory::ItemStack,
}
impl PlayerBodyState {
    pub fn from_constructor_math(values: [f64; 3]) -> Result<Self> {
        ensure!(
            values.iter().all(|v| (0.0..1.0).contains(v)),
            "invalid original Math.random constructor values"
        );
        Ok(Self {
            rot_a: ((values[0] + 1.) * 0.009999999776482582) as f32,
            time_offset: values[1] as f32 * 12398f32,
            constructor_yaw: (values[2] * 6.2831854820251465) as f32,
            swing_time: 0,
            swinging: false,
            swinging_hand: 0,
            attack_strength_ticker: 0,
            no_jump_delay: 0,
            cooldown_tick_count: 0,
            jumping: false,
            xxa: 0.,
            yya: 0.,
            zza: 0.,
            swim_amount: 0.,
            old_swim_amount: 0.,
            last_main_hand: crate::inventory::ItemStack::empty(),
        })
    }
    pub fn swing(&mut self, hand: u8) -> Result<bool> {
        ensure!(hand < 2, "invalid swing hand");
        if !self.swinging || self.swing_time >= 3 || self.swing_time < 0 {
            self.swing_time = -1;
            self.swinging = true;
            self.swinging_hand = hand;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub enum PlayerBodyPhase {
    ItemUse,
    PeacefulFood,
    InventoryTick,
    PushEntities,
    Pickup,
    FoodAndHealth,
    FinishPlayerTick,
}
pub trait PlayerBodyHooks: PlayerBodyWorldHooks {
    fn phase(
        &mut self,
        phase: PlayerBodyPhase,
        motion: &mut PlayerMotion,
        world: &mut RuntimeWorld,
        body: &mut PlayerBodyState,
    ) -> Result<()>;
}
fn move_relative(velocity: &mut Vec3, input: Vec3, speed: f32, yaw: f32) {
    let n = input.length_squared();
    if n < 1e-7 {
        return;
    }
    let input = if n > 1. {
        input.scale(1. / n.sqrt())
    } else {
        input
    };
    let v = input.scale(f64::from(speed));
    let angle = yaw * 0.017453292f32;
    let (s, c) = (geometry::vanilla_sin(angle), geometry::vanilla_cos(angle));
    velocity.x += v.x * f64::from(c) - v.z * f64::from(s);
    velocity.y += v.y;
    velocity.z += v.z * f64::from(c) + v.x * f64::from(s);
}
fn move_body<H: PlayerBodyHooks>(
    m: &mut PlayerMotion,
    w: &mut PlayerMoveWorld<'_, H>,
) -> Result<()> {
    let movement = m.physical.velocity;
    entity_move::move_entity(
        &mut m.physical,
        MoverType::SelfMove,
        movement,
        EntityMoveContext {
            step_height: m.attributes.step_height,
            descending: m.controls.shift,
            is_falling_block: false,
            can_walk_on_powder_snow: m.controls.can_walk_on_powder_snow,
            movement_emission: MovementEmission::All,
        },
        w,
    )?;
    Ok(())
}
fn fluid_falling(m: &PlayerMotion, v: Vec3, falling: bool) -> Vec3 {
    let g = if m.physical.no_gravity {
        0.
    } else {
        m.attributes.gravity
    };
    if g == 0. || m.controls.sprinting {
        return v;
    }
    let y = if falling && (v.y - 0.005).abs() >= 0.003 && (v.y - g / 16.).abs() < 0.003 {
        -0.003
    } else {
        v.y - g / 16.
    };
    Vec3::new(v.x, y, v.z)
}
fn liquid_step<H: PlayerBodyHooks>(
    m: &mut PlayerMotion,
    w: &mut PlayerMoveWorld<'_, H>,
    old_y: f64,
) -> Result<()> {
    if m.physical.horizontal_collision {
        let v = m.physical.velocity;
        let b = m.physical.bounds.moved(Vec3::new(
            v.x,
            v.y + 0.6000000238418579 - m.physical.position.y + old_y,
            v.z,
        ));
        if w.body_collision_free(&m.physical, b)? && w.body_no_liquid(b)? {
            m.physical.velocity.y = 0.30000001192092896;
        }
    }
    Ok(())
}
pub fn travel<H: PlayerBodyHooks>(
    m: &mut PlayerMotion,
    input: Vec3,
    w: &mut PlayerMoveWorld<'_, H>,
) -> Result<()> {
    ensure!(
        m.physical.vehicle.is_none()
            && !m.controls.fall_flying
            && !m.controls.levitating
            && !m.controls.auto_spin_attack,
        "unported special Player travel"
    );
    let was_flying = m.controls.flying;
    let previous_y = m.physical.velocity.y;
    let falling = previous_y <= 0.;
    let g = if m.physical.no_gravity {
        0.
    } else {
        m.attributes.gravity
    };
    let old_y = m.physical.position.y;
    if m.physical.was_touching_water && !was_flying {
        let drag = if m.controls.sprinting { 0.9f32 } else { 0.8f32 };
        move_relative(
            &mut m.physical.velocity,
            input,
            0.02,
            m.physical.position.yaw,
        );
        move_body(m, w)?;
        let mut v = m.physical.velocity;
        if m.physical.horizontal_collision && w.body_on_climbable(&m.physical)? {
            v.y = 0.2;
        }
        v = Vec3::new(
            v.x * f64::from(drag),
            v.y * 0.800000011920929,
            v.z * f64::from(drag),
        );
        m.physical.velocity = fluid_falling(m, v, falling);
        liquid_step(m, w, old_y)?;
    } else if m.physical.is_in_lava() && !was_flying {
        move_relative(
            &mut m.physical.velocity,
            input,
            0.02,
            m.physical.position.yaw,
        );
        move_body(m, w)?;
        if m.physical.fluid_heights[1] <= 0.4 {
            let v = m.physical.velocity;
            m.physical.velocity = fluid_falling(
                m,
                Vec3::new(v.x * 0.5, v.y * 0.800000011920929, v.z * 0.5),
                falling,
            );
        } else {
            m.physical.velocity = m.physical.velocity.scale(0.5);
        }
        m.physical.velocity.y -= g / 4.;
        liquid_step(m, w, old_y)?;
    } else {
        let p = entity_move::on_position(&m.physical, 0.500001, w)?;
        let f = friction(w.block_state(p)?)?;
        let drag = if m.physical.on_ground() {
            f * 0.91f32
        } else {
            0.91f32
        };
        let speed = if m.physical.on_ground() {
            w.host.body_movement_speed()? * (0.21600002f32 / (f * f * f))
        } else {
            w.host.body_flying_speed(m.controls.sprinting, was_flying)?
        };
        move_relative(
            &mut m.physical.velocity,
            input,
            speed,
            m.physical.position.yaw,
        );
        w.body_on_climbable(&m.physical)?;
        move_body(m, w)?;
        let v = m.physical.velocity;
        m.physical.velocity = Vec3::new(
            v.x * f64::from(drag),
            (v.y - g) * 0.9800000190734863,
            v.z * f64::from(drag),
        );
    }
    if was_flying {
        m.physical.velocity.y = previous_y * 0.6;
        w.reset_fall_distance(&mut m.physical)?;
        m.controls.fall_flying = false;
        m.physical.set_shared_flag(7, false);
    }
    Ok(())
}
pub fn tick<H: PlayerBodyHooks>(
    body: &mut PlayerBodyState,
    m: &mut PlayerMotion,
    world: &mut RuntimeWorld,
    entities: &[CollisionEntity],
    border: CollisionBorder,
    frame: PlayerMotionFrame,
    host: &mut H,
) -> Result<()> {
    ensure!(
        !m.controls.sleeping
            && !m.controls.fall_flying
            && !m.controls.auto_spin_attack
            && m.physical.vehicle.is_none()
            && !frame.dead_or_dying,
        "unsupported Player body lifecycle"
    );
    let facts = m.body_move_facts(frame);
    let mut w = PlayerMoveWorld::new(world, &m.physical, entities, border, facts, host)?;
    entity_base::base_tick(&mut m.physical, &mut w, |_, _, _, _| Ok(false))?;
    // Fresh creative invulnerability is checked by Main before this entry.
    if m.physical.fluid_on_eyes & 1 == 0 && m.physical.air_supply < 300 {
        let air = (m.physical.air_supply + 4).min(300);
        m.physical.set_air_supply(air);
    }
    w.host.phase(PlayerBodyPhase::ItemUse, m, w.world, body)?;
    body.old_swim_amount = body.swim_amount;
    body.swim_amount = if m.physical.is_swimming() {
        (body.swim_amount + 0.09).min(1.)
    } else {
        (body.swim_amount - 0.09).max(0.)
    };
    if body.swinging {
        body.swing_time += 1;
        if body.swing_time >= 6 {
            body.swing_time = 0;
            body.swinging = false;
        }
    } else {
        body.swing_time = 0;
    }
    w.host
        .phase(PlayerBodyPhase::PeacefulFood, m, w.world, body)?;
    w.host
        .phase(PlayerBodyPhase::InventoryTick, m, w.world, body)?;
    if body.no_jump_delay > 0 {
        body.no_jump_delay -= 1;
    }
    let v = m.physical.velocity;
    m.physical.velocity = Vec3::new(
        if v.x.abs() < 0.003 { 0. } else { v.x },
        if v.y.abs() < 0.003 { 0. } else { v.y },
        if v.z.abs() < 0.003 { 0. } else { v.z },
    );
    ensure!(
        !body.jumping,
        "server AI jumping requires owned input producer"
    );
    body.no_jump_delay = 0;
    body.xxa *= 0.98;
    body.zza *= 0.98;
    let input = Vec3::new(
        f64::from(body.xxa),
        f64::from(body.yya),
        f64::from(body.zza),
    );
    travel(m, input, &mut w)?;
    m.physical
        .set_ticks_frozen((m.physical.ticks_frozen - 2).max(0));
    w.host
        .phase(PlayerBodyPhase::PushEntities, m, w.world, body)?;
    w.host.phase(PlayerBodyPhase::Pickup, m, w.world, body)?;
    w.host
        .phase(PlayerBodyPhase::FoodAndHealth, m, w.world, body)?;
    body.attack_strength_ticker = body.attack_strength_ticker.wrapping_add(1);
    body.cooldown_tick_count = body.cooldown_tick_count.wrapping_add(1);
    w.host
        .phase(PlayerBodyPhase::FinishPlayerTick, m, w.world, body)?;
    if m.impulse.reset_grace_time > 0 {
        m.impulse.reset_grace_time -= 1;
    }
    Ok(())
}
