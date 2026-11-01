//! Shared authoritative physical state used by entity base ticks and movement.
//!
//! Position.on_ground is the sole ground flag. Item/other entity types keep
//! their own stack, health and AI state around this value; movement snapshots
//! must not introduce a second independently initialized physical state.
use crate::{
    entity::Position,
    geometry::{Aabb, Vec3},
};
use anyhow::{ensure, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PortalKind {
    Nether,
    End,
    EndGateway,
    Custom(u32),
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PortalProcessState {
    pub portal: PortalKind,
    pub entry_position: (i32, i32, i32),
    pub portal_time: i32,
    pub inside_portal_this_tick: bool,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EntityPhysicalState {
    pub entity_id: i32,
    pub uuid: [u8; 16],
    pub position: Position,
    pub block_position: (i32, i32, i32),
    pub velocity: Vec3,
    pub bounds: Aabb,
    pub width: f32,
    pub height: f32,
    pub eye_height: f32,
    pub no_physics: bool,
    pub no_gravity: bool,
    pub removed: bool,
    pub tick_count: i32,

    pub old_position: Vec3,
    pub old_yaw: f32,
    pub old_pitch: f32,
    pub walk_dist_old: f32,
    pub walk_dist: f32,
    pub move_dist: f32,
    pub fly_dist: f32,
    pub next_step: f32,
    pub fall_distance: f32,
    pub stuck_speed_multiplier: Vec3,
    pub horizontal_collision: bool,
    pub vertical_collision: bool,
    pub vertical_collision_below: bool,
    pub minor_horizontal_collision: bool,
    pub supporting_block: Option<(i32, i32, i32)>,
    pub on_ground_no_blocks: bool,
    pub piston_totals: [f64; 3],
    pub piston_tick: i64,

    pub first_tick: bool,
    pub boarding_cooldown: i32,
    pub vehicle: Option<i32>,
    pub portal_cooldown: i32,
    pub portal_process: Option<PortalProcessState>,
    pub invulnerable_time: i32,
    pub invulnerable: bool,
    pub remaining_fire_ticks: i32,
    pub fire_immune_ticks: i32,
    /// Refresh this type/stack-dependent fact before applying base/move logic.
    pub fire_immune: bool,
    pub ticks_frozen: i32,
    pub is_in_powder_snow: bool,
    pub was_in_powder_snow: bool,
    pub was_on_fire: bool,
    pub was_touching_water: bool,
    pub was_eye_in_water: bool,
    /// Indices 0 and 1 are water and lava, respectively.
    pub fluid_heights: [f64; 2],
    /// Bit 0 is water and bit 1 lava; arbitrary data-pack tags belong to the host.
    pub fluid_on_eyes: u8,
    pub shared_flags: u8,
    pub custom_name: Option<Vec<u8>>,
    pub custom_name_visible: bool,
    pub pose: u8,
    pub silent: bool,
    pub visual_fire: bool,
    pub is_spectator: bool,
    pub leashable: bool,
    pub air_supply: i32,
    pub has_impulse: bool,
    pub hurt_marked: bool,
    pub cached_inside_block_state: Option<u32>,
    /// SynchedEntityData dirty entries 0..7; item entities additionally use bit8.
    /// Setters retain dirtiness even when
    /// a later setter restores the old value before the shared tracker runs.
    pub metadata_dirty: u16,
}

impl EntityPhysicalState {
    pub fn new(
        entity_id: i32,
        uuid: [u8; 16],
        position: Position,
        velocity: Vec3,
        width: f32,
        height: f32,
        eye_height: f32,
    ) -> Result<Self> {
        ensure!(entity_id > 0, "entity ID must be positive");
        ensure!(
            [position.x, position.y, position.z, velocity.x, velocity.y, velocity.z]
                .iter()
                .all(|value| value.is_finite())
                && position.yaw.is_finite()
                && position.pitch.is_finite(),
            "nonfinite initial physical state"
        );
        ensure!(
            [width, height, eye_height].iter().all(|v| v.is_finite())
                && width >= 0.0
                && height >= 0.0,
            "invalid entity dimensions"
        );
        let feet = Vec3::new(position.x, position.y, position.z);
        Ok(Self {
            entity_id,
            uuid,
            position,
            block_position: (
                java_floor(position.x),
                java_floor(position.y),
                java_floor(position.z),
            ),
            velocity,
            bounds: Aabb::player(feet, width, height),
            width,
            height,
            eye_height,
            no_physics: false,
            no_gravity: false,
            removed: false,
            tick_count: 0,
            old_position: Vec3::ZERO,
            old_yaw: 0.0,
            old_pitch: 0.0,
            walk_dist_old: 0.0,
            walk_dist: 0.0,
            move_dist: 0.0,
            fly_dist: 0.0,
            next_step: 1.0,
            fall_distance: 0.0,
            stuck_speed_multiplier: Vec3::ZERO,
            horizontal_collision: false,
            vertical_collision: false,
            vertical_collision_below: false,
            minor_horizontal_collision: false,
            supporting_block: None,
            on_ground_no_blocks: false,
            piston_totals: [0.0; 3],
            piston_tick: 0,
            first_tick: true,
            boarding_cooldown: 0,
            vehicle: None,
            portal_cooldown: 0,
            portal_process: None,
            invulnerable_time: 0,
            invulnerable: false,
            remaining_fire_ticks: -1,
            fire_immune_ticks: 1,
            fire_immune: false,
            ticks_frozen: 0,
            is_in_powder_snow: false,
            was_in_powder_snow: false,
            was_on_fire: false,
            was_touching_water: false,
            was_eye_in_water: false,
            fluid_heights: [0.0; 2],
            fluid_on_eyes: 0,
            shared_flags: 0,
            custom_name: None,
            custom_name_visible: false,
            pose: 0,
            silent: false,
            visual_fire: false,
            is_spectator: false,
            leashable: false,
            air_supply: 300,
            has_impulse: false,
            hurt_marked: false,
            cached_inside_block_state: None,
            metadata_dirty: 0,
        })
    }

    pub fn position_vec(&self) -> Vec3 {
        Vec3::new(self.position.x, self.position.y, self.position.z)
    }

    /// Entity.setPos retains old signed-zero coordinate bits when all three
    /// numeric coordinates compare equal, but always rebuilds its bounding box.
    /// The return value tells the host whether setPosRaw's movement callback ran.
    pub fn set_position_raw(&mut self, position: Vec3) -> bool {
        let changed = self.position.x != position.x
            || self.position.y != position.y
            || self.position.z != position.z;
        if changed {
            self.position.x = position.x;
            self.position.y = position.y;
            self.position.z = position.z;
            let block = (
                java_floor(position.x),
                java_floor(position.y),
                java_floor(position.z),
            );
            if block != self.block_position {
                self.block_position = block;
                self.cached_inside_block_state = None;
            }
        }
        changed
    }

    pub fn refresh_bounds(&mut self) {
        self.bounds = Aabb::player(self.position_vec(), self.width, self.height);
    }

    pub fn set_position(&mut self, position: Vec3) -> bool {
        let changed = self.set_position_raw(position);
        self.refresh_bounds();
        changed
    }

    pub fn on_ground(&self) -> bool {
        self.position.on_ground
    }

    pub fn set_on_ground(&mut self, on_ground: bool) {
        self.position.on_ground = on_ground;
    }

    pub fn is_swimming(&self) -> bool {
        self.shared_flags & 0x10 != 0
    }

    pub fn set_swimming(&mut self, swimming: bool) {
        self.set_shared_flag(4, swimming);
    }

    pub fn set_shared_flag(&mut self, index: u8, set: bool) {
        let mask = 1_u8 << index;
        let value = if set {
            self.shared_flags | mask
        } else {
            self.shared_flags & !mask
        };
        if value != self.shared_flags {
            self.shared_flags = value;
            self.metadata_dirty |= 1;
        }
    }

    pub fn set_air_supply(&mut self, value: i32) {
        if self.air_supply != value {
            self.air_supply = value;
            self.metadata_dirty |= 1 << 1;
        }
    }
    pub fn set_custom_name(&mut self, value: Option<Vec<u8>>) {
        use crate::metadata::MetadataValue;
        if MetadataValue::OptionalComponent(self.custom_name.clone())
            != MetadataValue::OptionalComponent(value.clone())
        {
            self.custom_name = value;
            self.metadata_dirty |= 1 << 2;
        }
    }
    pub fn set_custom_name_visible(&mut self, value: bool) {
        if self.custom_name_visible != value {
            self.custom_name_visible = value;
            self.metadata_dirty |= 1 << 3;
        }
    }
    pub fn set_silent(&mut self, value: bool) {
        if self.silent != value {
            self.silent = value;
            self.metadata_dirty |= 1 << 4;
        }
    }
    pub fn set_no_gravity(&mut self, value: bool) {
        if self.no_gravity != value {
            self.no_gravity = value;
            self.metadata_dirty |= 1 << 5;
        }
    }
    pub fn set_pose(&mut self, value: u8) {
        if self.pose != value {
            self.pose = value;
            self.metadata_dirty |= 1 << 6;
        }
    }
    pub fn set_ticks_frozen(&mut self, value: i32) {
        if self.ticks_frozen != value {
            self.ticks_frozen = value;
            self.metadata_dirty |= 1 << 7;
        }
    }
    pub fn generic_metadata_values(&self, mask: u16) -> Vec<(u8, crate::metadata::MetadataValue)> {
        use crate::metadata::MetadataValue as V;
        [
            V::Byte(self.shared_flags),
            V::Int(self.air_supply),
            V::OptionalComponent(self.custom_name.clone()),
            V::Boolean(self.custom_name_visible),
            V::Boolean(self.silent),
            V::Boolean(self.no_gravity),
            V::Pose(i32::from(self.pose)),
            V::Int(self.ticks_frozen),
        ]
        .into_iter()
        .enumerate()
        .filter_map(|(index, value)| (mask & (1 << index) != 0).then_some((index as u8, value)))
        .collect()
    }
    pub fn generic_non_default_mask(&self) -> u16 {
        [
            self.shared_flags != 0,
            self.air_supply != 300,
            self.custom_name.is_some(),
            self.custom_name_visible,
            self.silent,
            self.no_gravity,
            self.pose != 0,
            self.ticks_frozen != 0,
        ]
        .into_iter()
        .enumerate()
        .fold(0, |mask, (index, set)| mask | ((u16::from(set)) << index))
    }
    pub fn take_generic_metadata_dirty(&mut self) -> u16 {
        let mask = self.metadata_dirty & 0xff;
        self.metadata_dirty &= !0xff;
        mask
    }
    pub fn take_metadata_dirty(&mut self) -> u16 {
        std::mem::take(&mut self.metadata_dirty)
    }

    pub fn is_on_fire(&self) -> bool {
        !self.fire_immune && self.remaining_fire_ticks > 0
    }

    pub fn is_in_lava(&self) -> bool {
        !self.first_tick && self.fluid_heights[1] > 0.0
    }

    pub fn is_sprinting(&self) -> bool {
        self.shared_flags & 8 != 0
    }

    pub fn eye_y(&self) -> f64 {
        self.position.y + f64::from(self.eye_height)
    }

    /// Level.tickNonPassenger owns this increment; Entity.baseTick does not.
    pub fn begin_level_tick(&mut self) {
        self.old_position = self.position_vec();
        self.old_yaw = self.position.yaw;
        self.old_pitch = self.position.pitch;
        self.tick_count = self.tick_count.wrapping_add(1);
    }
}

pub fn java_floor(value: f64) -> i32 {
    let narrowed = value as i32;
    if value < f64::from(narrowed) {
        narrowed.wrapping_sub(1)
    } else {
        narrowed
    }
}
