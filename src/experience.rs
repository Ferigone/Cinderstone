//! Native Java1.21 experience rewards, orb state and player XP arithmetic.
//!
//! The host supplies the real Level/entity/global-Math RNG owners and movement
//! callbacks. Awarding does not replace an orb with an immediate player grant.
use crate::{
    entity::Position,
    entity_state::EntityPhysicalState,
    geometry::{Aabb, Vec3},
    player_data::ExperienceData,
    protocol::{Packet, Writer},
    random::{LegacyRandomSource, RandomSource},
};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};

pub const EXPERIENCE_ORB_ENTITY_TYPE: i32 = 38;
pub const ORB_LIFETIME: i32 = 6000;

#[derive(Clone, Debug)]
pub struct ExperienceReward {
    pub recipe_id: String,
    pub count: i32,
    pub experience: f32,
}

/// AbstractFurnaceBlockEntity.createExperience: the multiplication and fraction
/// are f32; the conditional global Math.random sample is f64.
pub fn recipe_experience(
    count: i32,
    experience: f32,
    mut next_math_double: impl FnMut() -> Result<f64>,
) -> Result<i32> {
    ensure!(count >= 0 && experience.is_finite() && experience >= 0., "invalid recipe XP");
    let product = count as f32 * experience;
    let floor = product.floor();
    let mut amount = floor as i32;
    let fraction = product - floor;
    if fraction != 0. && next_math_double()? < f64::from(fraction) {
        amount = amount.wrapping_add(1);
    }
    Ok(amount)
}

pub fn experience_value(amount: i32) -> i32 {
    for value in [2477, 1237, 617, 307, 149, 73, 37, 17, 7, 3] {
        if amount >= value { return value; }
    }
    1
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExperienceOrbState {
    pub physical: EntityPhysicalState,
    pub age: i32,
    pub health: i32,
    pub value: i32,
    pub count: i32,
    /// Runtime reference, not persisted by vanilla's entity NBT.
    pub following_player: Option<i32>,
}
impl std::ops::Deref for ExperienceOrbState {
    type Target = EntityPhysicalState;
    fn deref(&self) -> &Self::Target { &self.physical }
}
impl std::ops::DerefMut for ExperienceOrbState {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.physical }
}
impl ExperienceOrbState {
    /// Entity UUID uses two nextLong calls; XP constructor then samples yaw,
    /// velocity-X/Y/Z through four nextDouble calls. No global Math.random.
    pub fn new(id: i32, at: Vec3, value: i32, random: &mut impl RandomSource) -> Result<Self> {
        ensure!(value > 0, "nonpositive orb value");
        let hi = (random.next_i64() as u64 & 0xffff_ffff_ffff_0fff) | 0x4000;
        let lo = (random.next_i64() as u64 & 0x3fff_ffff_ffff_ffff) | 0x8000_0000_0000_0000;
        let mut uuid = [0; 16];
        uuid[..8].copy_from_slice(&hi.to_be_bytes());
        uuid[8..].copy_from_slice(&lo.to_be_bytes());
        let yaw = (random.next_f64() * 360.) as f32;
        let vx = (random.next_f64() * f64::from(0.2f32) - f64::from(0.1f32)) * 2.;
        let vy = random.next_f64() * 0.2 * 2.;
        let vz = (random.next_f64() * f64::from(0.2f32) - f64::from(0.1f32)) * 2.;
        let physical = EntityPhysicalState::new(id, uuid, Position {
            x: at.x, y: at.y, z: at.z, yaw, pitch: 0., on_ground: false,
        }, Vec3::new(vx, vy, vz), 0.5, 0.5, 0.425f32)?;
        Ok(Self { physical, age: 0, health: 5, value, count: 1, following_player: None })
    }
    pub fn validate(&self) -> Result<()> {
        self.position.validate()?;
        ensure!(self.entity_id > 0 && self.width == 0.5 && self.height == 0.5 && self.eye_height == 0.425f32,
            "invalid native XP orb identity/dimensions");
        ensure!(self.value > 0 && self.count > 0 && self.age >= 0 && self.health > 0,
            "invalid live XP orb state");
        ensure!(self.vehicle.is_none() && !self.leashable, "unsupported mounted/leashed XP orb");
        Ok(())
    }
    pub fn can_merge_with(&self, group: i32, value: i32) -> bool {
        !self.removed && self.entity_id.wrapping_sub(group) % 40 == 0 && self.value == value
    }
    pub fn spawn_packet(&self, at: Position) -> Result<Packet> {
        self.validate()?;
        let mut w = Writer::new();
        w.varint(self.entity_id).f64(at.x).f64(at.y).f64(at.z).i16(self.value as i16);
        Ok(Packet { id: 0x02, payload: w.into_inner() })
    }
    pub fn merge_from(&mut self, other: &mut Self) {
        self.count = self.count.wrapping_add(other.count);
        self.age = self.age.min(other.age);
        other.removed = true;
    }
    pub fn hurt(&mut self, amount: f32) -> bool {
        // Actual caller has already resolved invulnerability/damage-source tags.
        self.hurt_marked = true;
        self.health = (self.health as f32 - amount) as i32;
        if self.health <= 0 { self.removed = true; }
        true
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedExperienceOrb {
    pub state: ExperienceOrbState,
    pub random_state48: u64,
}
impl SavedExperienceOrb {
    pub fn restore_random(&self) -> Result<LegacyRandomSource> {
        ensure!(self.random_state48 < 1u64 << 48, "invalid XP orb random checkpoint");
        Ok(LegacyRandomSource::new((self.random_state48 ^ 25_214_903_917) as i64))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FollowingPlayer {
    pub entity_id: i32,
    pub position: Vec3,
    pub eye_height: f32,
    pub spectator: bool,
    pub dead: bool,
}

pub trait ExperienceOrbWorld {
    /// Run the actual inherited Entity.baseTick against this physical owner.
    fn base_tick(&mut self, orb: &mut ExperienceOrbState, random: &mut LegacyRandomSource) -> Result<()>;
    fn lava_at_block(&mut self, orb: &ExperienceOrbState) -> Result<bool>;
    fn no_collision(&mut self, orb: &ExperienceOrbState) -> Result<bool>;
    fn move_towards_closest_space(&mut self, orb: &mut ExperienceOrbState, random: &mut LegacyRandomSource) -> Result<()>;
    fn nearest_player(&mut self, orb: &ExperienceOrbState, radius: f64) -> Result<Option<FollowingPlayer>>;
    fn player(&mut self, id: i32) -> Result<Option<FollowingPlayer>>;
    /// Actual entity query order; merge candidates during the scan before move.
    fn merge_neighbours(&mut self, orb: &mut ExperienceOrbState) -> Result<()>;
    fn move_orb(&mut self, orb: &mut ExperienceOrbState, random: &mut LegacyRandomSource, movement: Vec3) -> Result<()>;
    fn ground_friction(&mut self, orb: &ExperienceOrbState, random: &mut LegacyRandomSource) -> Result<f32>;
}

pub fn tick(orb: &mut ExperienceOrbState, random: &mut LegacyRandomSource, world: &mut impl ExperienceOrbWorld) -> Result<()> {
    if orb.removed { return Ok(()); }
    orb.tick_count = orb.tick_count.wrapping_add(1);
    world.base_tick(orb, random)?;
    let before = orb.position_vec();
    orb.old_position = before;
    if orb.fluid_on_eyes & 1 != 0 {
        orb.velocity = Vec3::new(orb.velocity.x * f64::from(0.99f32),
            (orb.velocity.y + f64::from(0.0005f32)).min(f64::from(0.06f32)),
            orb.velocity.z * f64::from(0.99f32));
    } else if !orb.no_gravity {
        orb.velocity.y -= 0.03;
    }
    if world.lava_at_block(orb)? {
        orb.velocity = Vec3::new(f64::from((random.next_f32() - random.next_f32()) * 0.2f32),
            f64::from(0.2f32), f64::from((random.next_f32() - random.next_f32()) * 0.2f32));
    }
    if !world.no_collision(orb)? { world.move_towards_closest_space(orb, random)?; }
    if orb.tick_count % 20 == 1 {
        let following = match orb.following_player { Some(id) => world.player(id)?, None => None };
        if following.is_none_or(|p| p.position.subtract(orb.position_vec()).length_squared() > 64.) {
            orb.following_player = world.nearest_player(orb, 8.)?.map(|p| p.entity_id);
        }
        world.merge_neighbours(orb)?;
    }
    if let Some(id) = orb.following_player {
        let following = world.player(id)?;
        if following.is_none_or(|p| p.spectator || p.dead) {
            orb.following_player = None;
        } else if let Some(p) = following {
            let offset = Vec3::new(p.position.x - orb.position.x,
                p.position.y + f64::from(p.eye_height) / 2. - orb.position.y,
                p.position.z - orb.position.z);
            let distance = offset.length_squared();
            if distance < 64. {
                let attraction = 1. - distance.sqrt() / 8.;
                // Vec3.normalize returns zero below1e-4, not NaNs at coincident points.
                let length = distance.sqrt();
                let normalized = if length < 1e-4 { Vec3::ZERO } else { offset.scale(1. / length) };
                orb.velocity = orb.velocity.plus(normalized.scale(attraction * attraction * 0.1));
            }
        }
    }
    let movement = orb.velocity;
    world.move_orb(orb, random, movement)?;
    let friction = if orb.position.on_ground { world.ground_friction(orb, random)? * 0.98f32 } else { 0.98f32 };
    orb.velocity = Vec3::new(orb.velocity.x * f64::from(friction), orb.velocity.y * 0.98,
        orb.velocity.z * f64::from(friction));
    if orb.position.on_ground { orb.velocity.y *= -0.9; }
    orb.age = orb.age.wrapping_add(1);
    if orb.age >= ORB_LIFETIME { orb.removed = true; }
    Ok(())
}

pub fn xp_needed_for_level(level: i32) -> i32 {
    if level >= 30 { 112i32.wrapping_add(level.wrapping_sub(30).wrapping_mul(9)) }
    else if level >= 15 { 37i32.wrapping_add(level.wrapping_sub(15).wrapping_mul(5)) }
    else { 7i32.wrapping_add(level.wrapping_mul(2)) }
}

/// Player.giveExperiencePoints preserves its exact f32 conversion/division
/// boundaries. Score and level-up sound callbacks remain real host effects.
pub fn give_experience_points(
    state: &mut ExperienceData,
    amount: i32,
    mut level_changed: impl FnMut(i32) -> Result<()>,
) -> Result<()> {
    ensure!(state.progress.is_finite() && state.level >= 0 && state.total >= 0, "invalid player XP owner");
    state.progress += amount as f32 / xp_needed_for_level(state.level) as f32;
    state.total = state.total.wrapping_add(amount).clamp(0, i32::MAX);
    while state.progress < 0. {
        let remainder = state.progress * xp_needed_for_level(state.level) as f32;
        if state.level > 0 {
            state.level = state.level.wrapping_sub(1);
            level_changed(state.level)?;
            state.progress = 1. + remainder / xp_needed_for_level(state.level) as f32;
        } else {
            state.level = 0; state.total = 0; state.progress = 0.;
            level_changed(0)?;
        }
    }
    while state.progress >= 1. {
        state.progress = (state.progress - 1.) * xp_needed_for_level(state.level) as f32;
        state.level = state.level.wrapping_add(1);
        level_changed(state.level)?;
        state.progress /= xp_needed_for_level(state.level) as f32;
    }
    Ok(())
}

pub fn experience_packet(state: &ExperienceData) -> Packet {
    let mut w = Writer::new();
    w.f32(state.progress).varint(state.level).varint(state.total);
    Packet { id: 0x5c, payload: w.into_inner() }
}
pub fn take_packet(orb: i32, player: i32) -> Packet {
    let mut w = Writer::new(); w.varint(orb).varint(player).varint(1);
    Packet { id: 0x6f, payload: w.into_inner() }
}
pub fn remove_packet(ids: &[i32]) -> Packet {
    let mut w = Writer::new(); w.varint(ids.len() as i32);
    for id in ids { w.varint(*id); }
    Packet { id: 0x42, payload: w.into_inner() }
}

pub fn award_bounds(at: Vec3) -> Aabb {
    Aabb::new(at.x - 0.5, at.y - 0.5, at.z - 0.5, at.x + 0.5, at.y + 0.5, at.z + 0.5)
}
