//! Minecraft1.21 dropped-item state, packetdata, drop kinematics and tick rules.
//!
//! World-dependent collision/block/fluid effects are supplied explicitly by the
//! host. This module does not substitute empty-space motion for unavailable
//! world geometry. Item ownership, delays and stack mutation follow Java cjh.
use crate::{
    entity::{self, Position},
    entity_base::{self, BaseDamage, EntityBaseWorld},
    entity_state::EntityPhysicalState,
    geometry::{vanilla_cos, vanilla_sin, Aabb, Vec3},
    inventory::ItemStack,
    metadata::{self, MetadataValue},
    protocol::{Packet, Writer},
    random::{LegacyRandomSource, RandomSource},
};
use anyhow::{ensure, Result};
use std::{collections::BTreeMap, sync::Arc};

pub const ITEM_ENTITY_TYPE: i32 = 58;
pub const INFINITE_PICKUP_DELAY: i32 = 32767;
pub const INFINITE_LIFETIME: i32 = -32768;
pub const DESPAWN_AGE: i32 = 6000;
pub const ITEM_STACK_METADATA_MASK: u16 = 1 << 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FluidKind {
    None,
    Water,
    Lava,
}
#[derive(Clone, Copy, Debug)]
pub struct BlockPhysics {
    pub friction: f32,
    pub fluid: FluidKind,
    pub fluid_own_height: f32,
}
pub fn block_physics(state: u32) -> Result<BlockPhysics> {
    use std::sync::OnceLock;
    #[derive(serde::Deserialize)]
    struct File {
        states: Vec<usize>,
        rows: Vec<[u32; 3]>,
    }
    static FACTS: OnceLock<File> = OnceLock::new();
    let facts = FACTS.get_or_init(|| {
        serde_json::from_str(include_str!("../tools/item_entity_physics_facts.json"))
            .expect("verified Java itemphysics facts")
    });
    let row = facts.rows[*facts
        .states
        .get(state as usize)
        .ok_or_else(|| anyhow::anyhow!("unknown exact1.21state"))?];
    Ok(BlockPhysics {
        friction: f32::from_bits(row[0]),
        fluid: match row[1] {
            1 => FluidKind::Water,
            2 => FluidKind::Lava,
            _ => FluidKind::None,
        },
        fluid_own_height: f32::from_bits(row[2]),
    })
}
pub fn fluid_height(state: u32, above: u32) -> Result<f32> {
    let here = block_physics(state)?;
    if here.fluid == FluidKind::None {
        Ok(0.)
    } else if block_physics(above)?.fluid == here.fluid {
        Ok(1.)
    } else {
        Ok(here.fluid_own_height)
    }
}

#[derive(Clone, Debug)]
pub struct ItemState {
    pub physical: EntityPhysicalState,
    pub stack: ItemStack,
    /// Whether watched Slot8 is the canonical Java ItemStack.EMPTY reference.
    /// Logical emptiness after in-place mutation does not change this identity.
    pub stack_is_default_reference: bool,
    pub age: i32,
    pub pickup_delay: i32,
    pub health: i32,
    pub target: Option<[u8; 16]>,
    pub thrower: Option<[u8; 16]>,
    pub bob_offset: f32,
}
impl std::ops::Deref for ItemState {
    type Target = EntityPhysicalState;
    fn deref(&self) -> &Self::Target {
        &self.physical
    }
}
impl std::ops::DerefMut for ItemState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.physical
    }
}
#[derive(Clone, Copy, Debug)]
pub struct PlayerDrop {
    pub player_uuid: [u8; 16],
    pub position: Position,
    pub eye_height: f32,
    pub around: bool,
    pub trace_item: bool,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DropKinematics {
    pub position: Vec3,
    pub velocity: Vec3,
}
/// Uses the original f32 operations and Mth lookup for aim, then Java Math's
/// double trigonometry for random jitter. It consumes2or4playerfloat samples.
pub fn player_drop_kinematics(drop: PlayerDrop, random: &mut impl RandomSource) -> DropKinematics {
    let p = drop.position;
    let position = Vec3::new(
        p.x,
        p.y + f64::from(drop.eye_height) - f64::from(0.3f32),
        p.z,
    );
    let velocity = if drop.around {
        let speed = random.next_f32() * 0.5f32;
        let angle = random.next_f32() * std::f32::consts::TAU;
        Vec3::new(
            f64::from(-vanilla_sin(angle) * speed),
            f64::from(0.2f32),
            f64::from(vanilla_cos(angle) * speed),
        )
    } else {
        let pitch = p.pitch * 0.017453292f32;
        let yaw = p.yaw * 0.017453292f32;
        let pitch_sin = vanilla_sin(pitch);
        let pitch_cos = vanilla_cos(pitch);
        let yaw_sin = vanilla_sin(yaw);
        let yaw_cos = vanilla_cos(yaw);
        let angle = random.next_f32() * std::f32::consts::TAU;
        let spread = 0.02f32 * random.next_f32();
        let vy = -pitch_sin * 0.3f32 + 0.1f32 + (random.next_f32() - random.next_f32()) * 0.1f32;
        let (jitter_sin, jitter_cos) = crate::hotspot_math::sin_cos(f64::from(angle));
        Vec3::new(
            f64::from(-yaw_sin * pitch_cos * 0.3f32) + jitter_cos * f64::from(spread),
            f64::from(vy),
            f64::from(yaw_cos * pitch_cos * 0.3f32) + jitter_sin * f64::from(spread),
        )
    };
    DropKinematics { position, velocity }
}

fn random_uuid(random: &mut impl RandomSource) -> [u8; 16] {
    let hi = (random.next_i64() as u64 & 0xffff_ffff_ffff_0fff) | 0x4000;
    let lo = (random.next_i64() as u64 & 0x3fff_ffff_ffff_ffff) | 0x8000_0000_0000_0000;
    let mut uuid = [0u8; 16];
    uuid[..8].copy_from_slice(&hi.to_be_bytes());
    uuid[8..].copy_from_slice(&lo.to_be_bytes());
    uuid
}
impl ItemState {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.entity_id > 0, "item entityID mustbepositive");
        self.position.validate()?;
        ensure!(
            [self.velocity.x, self.velocity.y, self.velocity.z]
                .iter()
                .all(|v| v.is_finite()),
            "nonfiniteitem velocity"
        );
        // A watched stack can become EMPTY before the following entity tick
        // discards it. Tracking/pairing in that interval is valid in Java.
        Ok(())
    }
    pub fn new(
        entity_id: i32,
        uuid: [u8; 16],
        position: Position,
        velocity: Vec3,
        stack: ItemStack,
    ) -> Result<Self> {
        let mut physical =
            EntityPhysicalState::new(entity_id, uuid, position, velocity, 0.25, 0.25, 0.2125)?;
        physical.fire_immune = stack.effective_component_bytes(21).is_some();
        // The actual constructor calls setItem, replacing the default EMPTY
        // reference. Snapshotting nondefaults does not clear this dirty entry.
        physical.metadata_dirty |= ITEM_STACK_METADATA_MASK;
        let value = Self {
            physical,
            stack,
            stack_is_default_reference: false,
            age: 0,
            pickup_delay: 0,
            health: 5,
            target: None,
            thrower: None,
            bob_offset: 0.,
        };
        value.validate()?;
        Ok(value)
    }
    pub fn bounding_box(&self) -> Aabb {
        self.bounds
    }
    /// A new Java ItemStack object replaces the watched reference. ItemStack
    /// does not override Object.equals, so even equal contents mark Slot8 dirty.
    pub fn set_stack(&mut self, stack: ItemStack) {
        self.stack = stack;
        self.stack_is_default_reference = false;
        self.metadata_dirty |= ITEM_STACK_METADATA_MASK;
        self.fire_immune = self.stack.effective_component_bytes(21).is_some();
    }
    /// Assign the canonical watched default reference, distinct from creating
    /// a new logically empty Java stack or shrinking the current stack to zero.
    pub fn set_stack_default_empty(&mut self) {
        if !self.stack_is_default_reference {
            self.metadata_dirty |= ITEM_STACK_METADATA_MASK;
        }
        self.stack = ItemStack::empty();
        self.stack_is_default_reference = true;
        self.fire_immune = false;
    }
    /// Mutation through the existing Java stack reference does not call
    /// SynchedEntityData.set. Pickup and merge-source shrinking use this path.
    pub fn set_stack_count_in_place(&mut self, count: i32) {
        self.stack.count = count;
    }
    pub fn default_pickup_delay(&mut self) {
        self.pickup_delay = 10;
    }
    pub fn no_pickup_delay(&mut self) {
        self.pickup_delay = 0;
    }
    pub fn never_pick_up(&mut self) {
        self.pickup_delay = INFINITE_PICKUP_DELAY;
    }
    pub fn unlimited_lifetime(&mut self) {
        self.age = INFINITE_LIFETIME;
    }
    pub fn extended_lifetime(&mut self) {
        self.age = -6000;
    }
    pub fn make_fake(&mut self) {
        self.never_pick_up();
        self.age = 5999;
    }
    pub fn is_mergeable(&self) -> Result<bool> {
        Ok(!self.removed
            && self.pickup_delay != INFINITE_PICKUP_DELAY
            && self.age != INFINITE_LIFETIME
            && self.age < DESPAWN_AGE
            && self.stack.count < self.stack.max_stack_size()?)
    }
    pub fn metadata_packet(&self) -> Result<Option<Packet>> {
        let mut values = self.generic_metadata_values(self.generic_non_default_mask());
        if !self.stack_is_default_reference {
            values.push((8, MetadataValue::ItemStack(Arc::new(self.stack.clone()))));
        }
        metadata::encode_values(self.entity_id, &values)
    }
    /// Encode the globally drained dirty mask without clearing it again. The
    /// caller combines base entries 0..7 and Slot8 before publishing one batch.
    pub fn dirty_metadata_packet(&self, mask: u16) -> Result<Option<Packet>> {
        let mut values = self.generic_metadata_values(mask & 0xff);
        if mask & ITEM_STACK_METADATA_MASK != 0 {
            values.push((8, MetadataValue::ItemStack(Arc::new(self.stack.clone()))));
        }
        metadata::encode_values(self.entity_id, &values)
    }
    /// Complete pairing for this published item view. Shared trackers call the
    /// ReplicatedEntity velocity-aware method to install their sent baselines.
    pub fn spawn_packets(&self) -> Result<Vec<Packet>> {
        self.validate()?;
        let p = self.position;
        let mut out = Writer::new();
        out.varint(self.entity_id)
            .uuid(self.uuid)
            .varint(ITEM_ENTITY_TYPE)
            .f64(p.x)
            .f64(p.y)
            .f64(p.z)
            .i8(entity::angle_byte(p.pitch))
            .i8(entity::angle_byte(p.yaw))
            .i8(0)
            .varint(0);
        for value in [self.velocity.x, self.velocity.y, self.velocity.z] {
            out.i16(velocity_units(value));
        }
        let mut packets = vec![Packet {
            id: 0x01,
            payload: out.into_inner(),
        }];
        if let Some(metadata) = self.metadata_packet()? {
            packets.push(metadata);
        }
        // Non-living delta-tracked entities receive an explicit motion packet
        // after AddEntity and cached metadata, even with a zero velocity.
        packets.push(velocity_packet(self.entity_id, self.velocity)?);
        Ok(packets)
    }
    /// Run one item tick including the host's usual entity-counter increment.
    /// Environmental movement returns post-collision motion. This standalone
    /// variant has no neighboring entities; Items supplies their merge phase.
    pub fn tick(&mut self, world: &mut impl ItemWorld) -> Result<ItemTick> {
        self.tick_with_merges(world, |_| Ok(()))
    }

    /// Full standalone tick, including the level's counter/old-position step.
    /// Mixed entity-list hosts call begin_level_tick and tick_full_body instead.
    pub fn tick_full(&mut self, world: &mut impl FullItemWorld) -> Result<ItemTick> {
        if self.removed {
            return Ok(ItemTick::default());
        }
        self.begin_level_tick();
        self.tick_full_body(world, |_| Ok(()))
    }

    /// Original ItemEntity.tick body, with full generic base/movement providers.
    /// The caller has already run Level.tickNonPassenger's counter increment.
    /// The merge callback executes before age, final fluid update and despawn.
    pub fn tick_full_body(
        &mut self,
        world: &mut impl FullItemWorld,
        mut merge: impl FnMut(&mut Self) -> Result<()>,
    ) -> Result<ItemTick> {
        if self.removed {
            return Ok(ItemTick::default());
        }
        if self.stack.is_empty() {
            self.removed = true;
            return Ok(ItemTick {
                removed: true,
                ..Default::default()
            });
        }
        self.fire_immune = self.stack.effective_component_bytes(21).is_some();
        {
            let stack = &mut self.stack;
            let health = &mut self.health;
            entity_base::base_tick(&mut self.physical, world, |state, world, damage, amount| {
                world.hurt_item(state, stack, health, damage, amount)
            })?;
        }
        if self.pickup_delay > 0 && self.pickup_delay != INFINITE_PICKUP_DELAY {
            self.pickup_delay -= 1;
        }
        let previous = self.position;
        self.old_position = self.position_vec();
        let old_velocity = self.velocity;
        if self.was_touching_water && self.fluid_heights[0] > f64::from(0.1_f32) {
            self.fluid_velocity(0.99_f32);
        } else if self.is_in_lava() && self.fluid_heights[1] > f64::from(0.1_f32) {
            self.fluid_velocity(0.95_f32);
        } else if !self.no_gravity {
            // Entity.applyGravity calls Vec3.add(0,-gravity,0), whose +0
            // additions canonicalize a negative zero in X/Z.
            self.velocity = self.velocity.plus(Vec3::new(0.0, -0.04, 0.0));
        }
        if world.is_client_side() {
            self.no_physics = false;
        } else {
            let bounds = self.bounds.deflate(Vec3::new(1e-7, 1e-7, 1e-7));
            self.no_physics = !world.item_no_collision(&self.physical, bounds)?;
            if self.no_physics {
                let position = Vec3::new(
                    self.position.x,
                    (self.bounds.min_y + self.bounds.max_y) / 2.0,
                    self.position.z,
                );
                move_towards_closest_space(&mut self.physical, position, world)?;
            }
        }
        let should_move = !self.on_ground()
            || self.velocity.horizontal_distance_squared() > f64::from(1.0e-5_f32)
            || self.tick_count.wrapping_add(self.entity_id) % 4 == 0;
        if should_move {
            let velocity = self.velocity;
            world.move_item_full(
                &mut self.physical,
                &mut self.stack,
                &mut self.health,
                velocity,
            )?;
            let horizontal = if self.on_ground() {
                world.item_ground_friction(&self.physical)? * 0.98_f32
            } else {
                0.98_f32
            };
            self.velocity = self.velocity.multiply(Vec3::new(
                f64::from(horizontal),
                0.98,
                f64::from(horizontal),
            ));
            if self.on_ground() && self.velocity.y < 0.0 {
                self.velocity.y *= -0.5;
            }
        }
        let floor = crate::entity_state::java_floor;
        let changed_block = [floor(previous.x), floor(previous.y), floor(previous.z)]
            != [
                floor(self.position.x),
                floor(self.position.y),
                floor(self.position.z),
            ];
        let merge_due = self.tick_count % if changed_block { 2 } else { 40 } == 0
            && !world.is_client_side()
            && self.is_mergeable()?;
        if merge_due {
            merge(self)?;
        }
        if self.age != INFINITE_LIFETIME {
            self.age = self.age.wrapping_add(1);
        }
        let in_fluid = entity_base::update_water_and_lava(&mut self.physical, world)?;
        self.has_impulse |= in_fluid;
        if !world.is_client_side() && self.velocity.subtract(old_velocity).length_squared() > 0.01 {
            self.has_impulse = true;
        }
        if !world.is_client_side() && self.age >= DESPAWN_AGE {
            self.removed = true;
        }
        Ok(ItemTick {
            previous,
            position: self.position,
            merge_due,
            removed: self.removed,
            velocity_changed: self.has_impulse,
        })
    }
    /// A neighboring-item callback runs at Java's original merge point,
    /// before age increments and the despawn decision.
    pub fn tick_with_merges(
        &mut self,
        world: &mut impl ItemWorld,
        mut merge: impl FnMut(&mut Self) -> Result<()>,
    ) -> Result<ItemTick> {
        if self.removed {
            return Ok(ItemTick::default());
        }
        self.tick_count = self.tick_count.wrapping_add(1);
        if self.stack.is_empty() {
            self.removed = true;
            return Ok(ItemTick {
                removed: true,
                ..Default::default()
            });
        }
        if self.pickup_delay > 0 && self.pickup_delay != INFINITE_PICKUP_DELAY {
            self.pickup_delay -= 1;
        }
        let previous = self.position;
        let old_velocity = self.velocity;
        let env = world.environment(self)?;
        if env.water_height > f64::from(0.1f32) {
            self.fluid_velocity(0.99f32);
        } else if env.lava_height > f64::from(0.1f32) {
            self.fluid_velocity(0.95f32);
        } else if !self.no_gravity {
            self.velocity = self.velocity.plus(Vec3::new(0.0, -0.04, 0.0));
        }
        let should_move = !self.position.on_ground
            || self.velocity.horizontal_distance_squared() > f64::from(1.0e-5f32)
            || (self.tick_count.wrapping_add(self.entity_id)) % 4 == 0;
        if should_move {
            let moved = world.move_item(self, self.velocity)?;
            self.set_position(moved.position);
            self.position.on_ground = moved.on_ground;
            self.velocity = moved.velocity;
            let horizontal = if moved.on_ground {
                moved.ground_friction * 0.98f32
            } else {
                0.98f32
            };
            self.velocity = self.velocity.multiply(Vec3::new(
                f64::from(horizontal),
                0.98,
                f64::from(horizontal),
            ));
            if self.position.on_ground && self.velocity.y < 0. {
                self.velocity.y *= -0.5;
            }
        }
        let changed_block = [
            previous.x.floor() as i32,
            previous.y.floor() as i32,
            previous.z.floor() as i32,
        ] != [
            self.position.x.floor() as i32,
            self.position.y.floor() as i32,
            self.position.z.floor() as i32,
        ];
        let merge_due =
            self.tick_count % if changed_block { 2 } else { 40 } == 0 && self.is_mergeable()?;
        if merge_due {
            merge(self)?;
        }
        if self.age != INFINITE_LIFETIME {
            self.age = self.age.wrapping_add(1);
        }
        if self.age >= DESPAWN_AGE {
            self.removed = true;
        }
        Ok(ItemTick {
            previous,
            position: self.position,
            merge_due,
            removed: self.removed,
            velocity_changed: self.velocity.subtract(old_velocity).length_squared() > 0.01,
        })
    }
    fn fluid_velocity(&mut self, drag: f32) {
        self.velocity.x *= f64::from(drag);
        self.velocity.z *= f64::from(drag);
        if self.velocity.y < f64::from(0.06f32) {
            self.velocity.y += f64::from(0.0005f32);
        }
    }
    /// The callback executes while the caller exclusively owns the itemstate.
    /// It must have vanillaInventory.add semantics, mutating the suppliedstack.
    pub fn try_pickup(
        &mut self,
        player_uuid: [u8; 16],
        player_entity_id: i32,
        add: impl FnMut(&mut ItemStack) -> Result<bool>,
    ) -> Result<Option<Pickup>> {
        if self.removed {
            return Ok(None);
        }
        struct LegacyPickup<F>(F);
        impl<F: FnMut(&mut ItemStack) -> Result<bool>> ItemPickupHost for LegacyPickup<F> {
            fn is_client_side(&self) -> bool {
                false
            }
            fn add_to_inventory(&mut self, stack: &mut ItemStack) -> Result<bool> {
                self.0(stack)
            }
            fn collect(&mut self, _: &ItemState, _: i32, _: i32) -> Result<()> {
                Ok(())
            }
            fn discarded(&mut self, _: &ItemState) -> Result<()> {
                Ok(())
            }
            fn after(&mut self, _: &ItemState, _: Pickup, _: i32) -> Result<()> {
                Ok(())
            }
        }
        self.try_pickup_with_callbacks(player_uuid, player_entity_id, &mut LegacyPickup(add))
    }
    /// Original ItemEntity.playerTouch ordering. The caller owns its actual
    /// player-query removed/alive/spectator gates; the method itself only has
    /// the original client, delay and target gates.
    pub fn try_pickup_with_callbacks(
        &mut self,
        player_uuid: [u8; 16],
        player_entity_id: i32,
        host: &mut impl ItemPickupHost,
    ) -> Result<Option<Pickup>> {
        if host.is_client_side() {
            return Ok(None);
        }
        let original_item_id = if self.stack.is_empty() {
            0
        } else {
            self.stack.item_id
        };
        let original = if self.stack.is_empty() {
            0
        } else {
            self.stack.count
        };
        if self.pickup_delay != 0 || self.target.is_some_and(|target| target != player_uuid) {
            return Ok(None);
        }
        if !host.add_to_inventory(&mut self.stack)? {
            return Ok(None);
        }
        let remaining = if self.stack.is_empty() {
            0
        } else {
            self.stack.count
        };
        let accepted = original.wrapping_sub(remaining);
        host.collect(self, original, accepted)?;
        if self.stack.is_empty() {
            self.removed = true;
            host.discarded(self)?;
            self.set_stack_count_in_place(original);
        }
        let pickup = Pickup {
            item_entity_id: self.entity_id,
            player_entity_id,
            packet_count: original,
            accepted_count: accepted,
            removed: self.removed,
        };
        host.after(self, pickup, original_item_id)?;
        Ok(Some(pickup))
    }
}
/// Vanilla still consumes the world's2defaultvelocitydoubles before replacing
/// that velocity with playerdrop aim. The independent entityRNG generatesUUID,
/// bob phase and initialvisualyaw in their original order.
pub fn spawn_player_drop(
    entity_id: i32,
    drop: PlayerDrop,
    stack: ItemStack,
    player_random: &mut impl RandomSource,
    world_random: &mut impl RandomSource,
    entity_random: &mut impl RandomSource,
) -> Result<Option<ItemState>> {
    if stack.is_empty() {
        return Ok(None);
    }
    let _ = world_random.next_f64();
    let _ = world_random.next_f64();
    spawn_player_drop_after_world_draws(entity_id, drop, stack, player_random, entity_random)
        .map(Some)
}

/// Player.drop's constructor argument evaluation consumes both level doubles
/// before Entity's RandomSource.create samples its seed. Retain the returned
/// entity RNG after UUID/bob/yaw for later physics; never recreate its seed.
pub fn spawn_player_drop_with_factory(
    entity_id: i32,
    drop: PlayerDrop,
    stack: ItemStack,
    player_random: &mut impl RandomSource,
    world_random: &mut impl RandomSource,
    mut factory: impl FnMut() -> Result<LegacyRandomSource>,
) -> Result<Option<(ItemState, LegacyRandomSource)>> {
    if stack.is_empty() {
        return Ok(None);
    }
    let _ = world_random.next_f64();
    let _ = world_random.next_f64();
    let mut entity_random = factory()?;
    let item = spawn_player_drop_after_world_draws(
        entity_id,
        drop,
        stack,
        player_random,
        &mut entity_random,
    )?;
    Ok(Some((item, entity_random)))
}

fn spawn_player_drop_after_world_draws(
    entity_id: i32,
    drop: PlayerDrop,
    stack: ItemStack,
    player_random: &mut impl RandomSource,
    entity_random: &mut impl RandomSource,
) -> Result<ItemState> {
    let uuid = random_uuid(entity_random);
    let bob_offset = entity_random.next_f32() * std::f32::consts::PI * 2f32;
    let yaw = entity_random.next_f32() * 360f32;
    let motion = player_drop_kinematics(drop, player_random);
    let p = Position {
        x: motion.position.x,
        y: motion.position.y,
        z: motion.position.z,
        yaw,
        pitch: 0.,
        on_ground: false,
    };
    let mut item = ItemState::new(entity_id, uuid, p, motion.velocity, stack)?;
    item.bob_offset = bob_offset;
    item.pickup_delay = 40;
    if drop.trace_item {
        item.thrower = Some(drop.player_uuid);
    }
    Ok(item)
}
impl crate::replication::ReplicatedEntity for ItemState {
    fn tracking_range_chunks(&self) -> i32 {
        6
    }
    fn entity_id(&self) -> i32 {
        self.entity_id
    }
    fn position(&self) -> Position {
        self.position
    }
    fn validate(&self) -> Result<()> {
        ItemState::validate(self)
    }
    fn spawn_packets(&self, spawn_position: Position) -> Result<Vec<Packet>> {
        let mut current = self.clone();
        current.position = spawn_position;
        ItemState::spawn_packets(&current)
    }
    fn spawn_packets_with_velocity(
        &self,
        spawn_position: Position,
        spawn_velocity: Option<Vec3>,
    ) -> Result<Vec<Packet>> {
        let velocity = spawn_velocity.ok_or_else(|| {
            anyhow::anyhow!("item pairing requires the shared tracker velocity baseline")
        })?;
        let mut current = self.clone();
        current.position = spawn_position;
        current.velocity = velocity;
        ItemState::spawn_packets(&current)
    }
    fn validate_spawn_velocity(&self, spawn_velocity: Option<Vec3>) -> Result<()> {
        ensure!(
            spawn_velocity.is_some(),
            "item pairing requires the shared tracker velocity baseline"
        );
        Ok(())
    }
    fn snapshot_delta(&self, previous: &Self) -> Result<Vec<Packet>> {
        let mut values = Vec::new();
        for (current, old) in self
            .generic_metadata_values(0xff)
            .into_iter()
            .zip(previous.generic_metadata_values(0xff))
        {
            if current.1 != old.1 {
                values.push(current);
            }
        }
        if self.stack.count != previous.stack.count
            || self.stack_is_default_reference != previous.stack_is_default_reference
            || !self.stack.same_item_same_components(&previous.stack)?
        {
            values.push((8, MetadataValue::ItemStack(Arc::new(self.stack.clone()))));
        }
        let mut packets = Vec::new();
        if let Some(packet) = metadata::encode_values(self.entity_id, &values)? {
            packets.push(packet);
        }
        Ok(packets)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ItemEnvironment {
    pub water_height: f64,
    pub lava_height: f64,
}
#[derive(Clone, Copy, Debug)]
pub struct ItemMovement {
    pub position: Vec3,
    pub velocity: Vec3,
    pub on_ground: bool,
    pub ground_friction: f32,
}
pub trait ItemWorld {
    fn environment(&mut self, item: &ItemState) -> Result<ItemEnvironment>;
    fn move_item(&mut self, item: &ItemState, movement: Vec3) -> Result<ItemMovement>;
}

/// Required host operations for the full item tick. They are deliberately
/// separate from the older injected scalar tick fixture's ItemWorld contract.
pub trait FullItemWorld: EntityBaseWorld {
    fn item_no_collision(&mut self, state: &EntityPhysicalState, bounds: Aabb) -> Result<bool>;
    fn escape_block_is_full_collision(&mut self, position: (i32, i32, i32)) -> Result<bool>;
    /// The canonical stack/health stay borrowed through Entity.move callbacks.
    /// Block.entityInside can invoke ItemEntity.hurt synchronously during move.
    fn move_item_full(
        &mut self,
        state: &mut EntityPhysicalState,
        stack: &mut ItemStack,
        health: &mut i32,
        movement: Vec3,
    ) -> Result<()>;
    /// Use Entity.getOnPos(0.999999f), including the supporting-block override.
    fn item_ground_friction(&mut self, state: &EntityPhysicalState) -> Result<f32>;
    fn hurt_item(
        &mut self,
        state: &mut EntityPhysicalState,
        stack: &mut ItemStack,
        health: &mut i32,
        damage: BaseDamage,
        amount: f32,
    ) -> Result<bool>;
}

pub fn move_towards_closest_space(
    state: &mut EntityPhysicalState,
    point: Vec3,
    world: &mut impl FullItemWorld,
) -> Result<()> {
    let floor = crate::entity_state::java_floor;
    let block = (floor(point.x), floor(point.y), floor(point.z));
    let fraction = [
        point.x - f64::from(block.0),
        point.y - f64::from(block.1),
        point.z - f64::from(block.2),
    ];
    let mut axis = 1;
    let mut sign = 1_i32;
    let mut nearest = f64::MAX;
    for (step, candidate_axis, candidate_sign) in [
        ((0_i32, 0_i32, -1_i32), 2_usize, -1_i32),
        ((0, 0, 1), 2, 1),
        ((-1, 0, 0), 0, -1),
        ((1, 0, 0), 0, 1),
        ((0, 1, 0), 1, 1),
    ] {
        let neighbor = (
            block.0.wrapping_add(step.0),
            block.1.wrapping_add(step.1),
            block.2.wrapping_add(step.2),
        );
        if !world.escape_block_is_full_collision(neighbor)? {
            let distance = if candidate_sign > 0 {
                1.0 - fraction[candidate_axis]
            } else {
                fraction[candidate_axis]
            };
            if distance < nearest {
                nearest = distance;
                axis = candidate_axis;
                sign = candidate_sign;
            }
        }
    }
    let speed = world.next_entity_float(state)? * 0.2_f32 + 0.1_f32;
    let directed = f64::from(sign as f32 * speed);
    let velocity = state.velocity.scale(0.75);
    state.velocity = match axis {
        0 => Vec3::new(directed, velocity.y, velocity.z),
        1 => Vec3::new(velocity.x, directed, velocity.z),
        _ => Vec3::new(velocity.x, velocity.y, directed),
    };
    Ok(())
}
#[derive(Clone, Copy, Debug)]
pub struct ItemTick {
    pub previous: Position,
    pub position: Position,
    pub merge_due: bool,
    pub removed: bool,
    pub velocity_changed: bool,
}
impl Default for ItemTick {
    fn default() -> Self {
        Self {
            previous: Position {
                x: 0.,
                y: 0.,
                z: 0.,
                yaw: 0.,
                pitch: 0.,
                on_ground: false,
            },
            position: Position {
                x: 0.,
                y: 0.,
                z: 0.,
                yaw: 0.,
                pitch: 0.,
                on_ground: false,
            },
            merge_due: false,
            removed: false,
            velocity_changed: false,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Pickup {
    pub item_entity_id: i32,
    pub player_entity_id: i32,
    pub packet_count: i32,
    pub accepted_count: i32,
    pub removed: bool,
}
pub trait ItemPickupHost {
    fn is_client_side(&self) -> bool;
    /// Vanilla Inventory.add mutates the existing stack's count; it must retain
    /// the item's data when count reaches zero for the later count restoration.
    fn add_to_inventory(&mut self, stack: &mut ItemStack) -> Result<bool>;
    /// Player.take, before removal; packet count is the original stack count.
    fn collect(
        &mut self,
        state: &ItemState,
        original_count: i32,
        accepted_count: i32,
    ) -> Result<()>;
    /// Level callback onRemove(DISCARDED), observing empty stack and removed.
    fn discarded(&mut self, state: &ItemState) -> Result<()>;
    /// Original item-pickup statistic then Player.onItemPickup, after count
    /// restoration. original_item_id is captured before Inventory.add.
    fn after(&mut self, state: &ItemState, pickup: Pickup, original_item_id: i32) -> Result<()>;
}
impl Pickup {
    pub fn packet(self) -> Packet {
        let mut out = Writer::new();
        out.varint(self.item_entity_id)
            .varint(self.player_entity_id)
            .varint(self.packet_count);
        Packet {
            id: 0x6f,
            payload: out.into_inner(),
        }
    }
}
#[cfg(test)]
#[path = "../tools/item_pickup_tests.rs"]
mod pickup_tests;
fn velocity_units(value: f64) -> i16 {
    (value.clamp(-3.9, 3.9) * 8000.) as i32 as i16
}
pub fn velocity_packet(entity_id: i32, velocity: Vec3) -> Result<Packet> {
    ensure!(entity_id > 0, "entityIDmustbepositive");
    let mut out = Writer::new();
    out.varint(entity_id);
    for value in [velocity.x, velocity.y, velocity.z] {
        out.i16(velocity_units(value));
    }
    Ok(Packet {
        id: 0x5a,
        payload: out.into_inner(),
    })
}
pub fn stacks_mergeable(a: &ItemStack, b: &ItemStack) -> Result<bool> {
    Ok(a.count.wrapping_add(b.count) <= b.max_stack_size()? && a.same_item_same_components(b)?)
}
#[derive(Clone, Copy, Debug)]
pub struct Merge {
    pub survivor: i32,
    pub source: i32,
    pub source_removed: bool,
}
pub fn merge_pair(a: &mut ItemState, b: &mut ItemState) -> Result<Option<Merge>> {
    if !a.is_mergeable()?
        || !b.is_mergeable()?
        || a.target != b.target
        || !stacks_mergeable(&a.stack, &b.stack)?
    {
        return Ok(None);
    }
    let (destination, source) = if b.stack.count < a.stack.count {
        (a, b)
    } else {
        (b, a)
    };
    let amount = (destination.stack.max_stack_size()?.min(64) - destination.stack.count)
        .min(source.stack.count);
    let mut merged = destination.stack.clone();
    merged.count += amount;
    destination.set_stack(merged);
    source.set_stack_count_in_place(source.stack.count - amount);
    destination.pickup_delay = destination.pickup_delay.max(source.pickup_delay);
    destination.age = destination.age.min(source.age);
    if source.stack.is_empty() {
        source.removed = true;
    }
    Ok(Some(Merge {
        survivor: destination.entity_id,
        source: source.entity_id,
        source_removed: source.removed,
    }))
}
#[derive(Clone, Debug)]
pub struct ItemUpdate {
    pub previous: ItemState,
    pub current: ItemState,
    pub tick: Option<ItemTick>,
}

#[derive(Default)]
pub struct Items {
    entities: BTreeMap<i32, ItemState>,
}
impl Items {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn insert(&mut self, item: ItemState) -> Result<()> {
        item.validate()?;
        ensure!(
            !self.entities.contains_key(&item.entity_id),
            "duplicateitem entityID"
        );
        self.entities.insert(item.entity_id, item);
        Ok(())
    }
    pub fn get(&self, id: i32) -> Option<&ItemState> {
        self.entities.get(&id)
    }
    pub fn get_mut(&mut self, id: i32) -> Option<&mut ItemState> {
        self.entities.get_mut(&id)
    }
    pub fn snapshot(&self) -> Vec<ItemState> {
        self.entities
            .values()
            .filter(|i| !i.removed)
            .cloned()
            .collect()
    }
    pub fn remove(&mut self, id: i32) -> Option<ItemState> {
        self.entities.remove(&id)
    }
    /// Tick a manager in entity-ID order. Hosts with section-indexed entity
    /// storage should use tick_all_ordered to retain their exact query order.
    pub fn tick_all(&mut self, world: &mut impl ItemWorld) -> Result<Vec<ItemUpdate>> {
        self.tick_all_ordered(world, |_, entities| entities.keys().copied().collect())
    }
    /// The query callback supplies candidate IDs in the world's original
    /// spatial-query order; exact AABB and mergeability filtering stay here.
    pub fn tick_all_ordered(
        &mut self,
        world: &mut impl ItemWorld,
        mut query: impl FnMut(&ItemState, &BTreeMap<i32, ItemState>) -> Vec<i32>,
    ) -> Result<Vec<ItemUpdate>> {
        let before = self.entities.clone();
        let ids = self.entities.keys().copied().collect::<Vec<_>>();
        let mut ticks = BTreeMap::new();
        for id in ids {
            let Some(mut item) = self.entities.remove(&id) else {
                continue;
            };
            if item.removed {
                self.entities.insert(id, item);
                continue;
            }
            let result = item.tick_with_merges(world, |current| {
                let bounds = current.bounding_box().inflate(Vec3::new(0.5, 0., 0.5));
                for other in query(current, &self.entities) {
                    if !current.is_mergeable()? {
                        break;
                    }
                    if let Some(neighbor) = self.entities.get_mut(&other) {
                        if bounds.intersects(&neighbor.bounding_box()) && neighbor.is_mergeable()? {
                            merge_pair(current, neighbor)?;
                        }
                    }
                }
                Ok(())
            });
            self.entities.insert(id, item);
            ticks.insert(id, result?);
        }
        let updates = before
            .into_iter()
            .filter_map(|(id, previous)| {
                self.entities.get(&id).map(|current| ItemUpdate {
                    previous,
                    current: current.clone(),
                    tick: ticks.get(&id).copied(),
                })
            })
            .collect();
        self.entities.retain(|_, item| !item.removed);
        Ok(updates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::random::LegacyRandomSource;

    #[test]
    fn actual_java_stack_reference_dirty_masks_and_metadata_packets() {
        use base64::Engine;
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../fixtures/item-reference-vectors.json")).unwrap();
        let make = |input: &serde_json::Value, key: &str, id| {
            ItemState::new(
                id,
                [0; 16],
                Position {
                    x: 0.0,
                    y: 64.0,
                    z: 0.0,
                    yaw: 0.0,
                    pitch: 0.0,
                    on_ground: false,
                },
                Vec3::ZERO,
                ItemStack::decode_exact(
                    &base64::engine::general_purpose::STANDARD
                        .decode(input[format!("{key}_wire")].as_str().unwrap())
                        .unwrap(),
                )
                .unwrap(),
            )
            .unwrap()
        };
        let verify = |item: &mut ItemState, expected: &serde_json::Value, case, side| {
            assert_eq!(
                item.stack.count,
                expected["count"].as_i64().unwrap() as i32,
                "count case{case}{side}"
            );
            assert_eq!(
                item.removed,
                expected["removed"].as_bool().unwrap(),
                "removed case{case}{side}"
            );
            let mask = expected["dirty_mask"].as_u64().unwrap() as u16;
            assert_eq!(item.metadata_dirty, mask, "mask case{case}{side}");
            let packet = item.dirty_metadata_packet(mask).unwrap();
            if let Some(wire) = expected["dirty_wire"].as_str() {
                let packet = packet.expect("actual Java dirty packet");
                assert_eq!(packet.id, 0x58);
                assert_eq!(
                    packet.payload,
                    base64::engine::general_purpose::STANDARD
                        .decode(wire)
                        .unwrap(),
                    "wire case{case}{side}"
                );
            } else {
                assert!(packet.is_none(), "spurious dirty packet case{case}{side}");
            }
            // Draining base fields must not erase the item-specific entry.
            assert_eq!(item.take_generic_metadata_dirty(), mask & 0xff);
            assert_eq!(item.metadata_dirty, mask & !0xff);
            assert_eq!(item.take_metadata_dirty(), mask & !0xff);
        };
        let mut packets = 0;
        for (index, row) in fixture["cases"].as_array().unwrap().iter().enumerate() {
            let input = &row["input"];
            let expected = &row["expected"];
            let mut left = make(input, "left", 1);
            assert_eq!(
                left.take_metadata_dirty(),
                expected["constructor_dirty"].as_u64().unwrap() as u16
            );
            if input["op"] == "merge" {
                let mut right = make(input, "right", 2);
                right.take_metadata_dirty();
                merge_pair(&mut left, &mut right).unwrap();
                verify(&mut right, &expected["right"], index, "right");
                packets += usize::from(expected["right"]["dirty_wire"].is_string());
            } else {
                for action in input["actions"].as_array().unwrap() {
                    match action.as_str().unwrap() {
                        "same_reference" => {}
                        "equal_copy" => left.set_stack(left.stack.clone()),
                        "count_in_place" => {
                            left.set_stack_count_in_place(input["count"].as_i64().unwrap() as i32)
                        }
                        "gravity_on" => left.set_no_gravity(true),
                        "gravity_off" => left.set_no_gravity(false),
                        _ => panic!("unknown reference operation"),
                    }
                }
            }
            verify(&mut left, &expected["left"], index, "left");
            packets += usize::from(expected["left"]["dirty_wire"].is_string());
        }
        println!(
            "ITEM_REFERENCE_CONFORMANCE:{}",
            serde_json::json!({"ok":true,"cases":fixture["cases"].as_array().unwrap().len(),"actual_java_dirty_packets":packets,"differences":0,"scope":"original ItemEntity constructor and SynchedEntityData reference identity, in-place count mutation, gravity restore, merge destination/source dirtiness and exact dirty metadata packet bytes"})
        );
    }

    #[test]
    fn actual_java_item_merge_pairs() {
        use base64::Engine;
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("../tools/item_entity_merge_vectors.json")).unwrap();
        let stacks = cases["stacks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| {
                ItemStack::decode_exact(
                    &base64::engine::general_purpose::STANDARD
                        .decode(s.as_str().unwrap())
                        .unwrap(),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let make = |q: &serde_json::Value, id: i32| {
            let p = Position {
                x: 0.,
                y: 64.,
                z: 0.,
                yaw: 0.,
                pitch: 0.,
                on_ground: true,
            };
            let mut item = ItemState::new(
                id,
                [0; 16],
                p,
                Vec3::new(0., 0., 0.),
                stacks[q["stack"].as_u64().unwrap() as usize].clone(),
            )
            .unwrap();
            item.age = q["age"].as_i64().unwrap() as i32;
            item.pickup_delay = q["pickup_delay"].as_i64().unwrap() as i32;
            let uuid = |field: &str| {
                q[field].as_str().map(|name| {
                    let mut v = [0u8; 16];
                    v[6] = 0x40;
                    v[8] = 0x80;
                    v[15] = name.chars().last().unwrap().to_digit(10).unwrap() as u8;
                    v
                })
            };
            item.target = uuid("target");
            item.thrower = uuid("thrower");
            item
        };
        for (index, q) in cases["cases"].as_array().unwrap().iter().enumerate() {
            let mut left = make(&q["left"], 1);
            let mut right = make(&q["right"], 2);
            assert_eq!(
                left.is_mergeable().unwrap(),
                q["left_mergeable"].as_bool().unwrap()
            );
            assert_eq!(
                right.is_mergeable().unwrap(),
                q["right_mergeable"].as_bool().unwrap()
            );
            assert_eq!(
                stacks_mergeable(&left.stack, &right.stack).unwrap(),
                q["stacks_mergeable"].as_bool().unwrap()
            );
            merge_pair(&mut left, &mut right).unwrap();
            for (side, item) in [("left_after", left), ("right_after", right)] {
                let expected = &q[side];
                let stack = &stacks[expected["stack"].as_u64().unwrap() as usize];
                assert!(
                    item.stack.same_item_same_components(stack).unwrap(),
                    "merge{index}{side}components"
                );
                if !stack.is_empty() {
                    assert_eq!(item.stack.count, stack.count, "merge{index}{side}count");
                }
                assert_eq!(
                    item.age,
                    expected["age"].as_i64().unwrap() as i32,
                    "merge{index}{side}age"
                );
                assert_eq!(
                    item.pickup_delay,
                    expected["pickup_delay"].as_i64().unwrap() as i32
                );
                assert_eq!(item.removed, expected["removed"].as_bool().unwrap());
            }
        }
    }
    #[test]
    fn actual_java_scalar_item_tick_vectors() {
        struct World {
            water: f64,
            lava: f64,
            floor: Option<f64>,
            friction: f32,
            bounce: bool,
        }
        impl ItemWorld for World {
            fn environment(&mut self, _: &ItemState) -> Result<ItemEnvironment> {
                Ok(ItemEnvironment {
                    water_height: self.water,
                    lava_height: self.lava,
                })
            }
            fn move_item(&mut self, item: &ItemState, movement: Vec3) -> Result<ItemMovement> {
                let mut position =
                    Vec3::new(item.position.x, item.position.y, item.position.z).plus(movement);
                let mut velocity = movement;
                let mut on_ground = false;
                if let Some(floor) = self.floor {
                    if position.y < floor {
                        position.y = floor;
                        on_ground = true;
                        if !self.bounce {
                            velocity.y = 0.;
                        }
                    }
                }
                Ok(ItemMovement {
                    position,
                    velocity,
                    on_ground,
                    ground_friction: self.friction,
                })
            }
        }
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("../tools/item_entity_physics_vectors.json"))
                .unwrap();
        for (scene, q) in cases["scenes"].as_array().unwrap().iter().enumerate() {
            let vector = |field: &str| {
                Vec3::new(
                    q[field][0].as_f64().unwrap(),
                    q[field][1].as_f64().unwrap(),
                    q[field][2].as_f64().unwrap(),
                )
            };
            let p = vector("position");
            let position = Position {
                x: p.x,
                y: p.y,
                z: p.z,
                yaw: 0.,
                pitch: 0.,
                on_ground: q["on_ground"].as_bool().unwrap(),
            };
            let mut item = ItemState::new(
                q["entity_id"].as_i64().unwrap() as i32,
                [0; 16],
                position,
                vector("velocity"),
                ItemStack::new(1, q["stack"]["count"].as_i64().unwrap() as i32).unwrap(),
            )
            .unwrap();
            item.age = q["age"].as_i64().unwrap() as i32;
            item.pickup_delay = q["pickup_delay"].as_i64().unwrap() as i32;
            item.tick_count = q["tick_count"].as_i64().unwrap() as i32;
            item.no_gravity = q["no_gravity"].as_bool().unwrap();
            let friction =
                f32::from_bits(q["ground_friction_bits"].as_str().unwrap().parse().unwrap());
            assert_eq!(
                block_physics(q["floor_state"].as_u64().unwrap() as u32)
                    .unwrap()
                    .friction
                    .to_bits(),
                friction.to_bits()
            );
            let mut world = World {
                water: q["water_height"].as_f64().unwrap(),
                lava: q["lava_height"].as_f64().unwrap(),
                floor: q["floor_y"].as_f64(),
                friction,
                bounce: q["bounce"].as_bool().unwrap(),
            };
            for (tick, expected) in q["states"].as_array().unwrap().iter().enumerate() {
                let step = item.tick(&mut world).unwrap();
                for (field, values) in [
                    (
                        "position_bits",
                        [item.position.x, item.position.y, item.position.z],
                    ),
                    (
                        "velocity_bits",
                        [item.velocity.x, item.velocity.y, item.velocity.z],
                    ),
                ] {
                    for (axis, value) in values.into_iter().enumerate() {
                        assert_eq!(
                            value.to_bits(),
                            expected[field][axis]
                                .as_str()
                                .unwrap()
                                .parse::<u64>()
                                .unwrap(),
                            "scene{scene}tick{tick}{field}axis{axis}"
                        );
                    }
                }
                assert_eq!(
                    item.age,
                    expected["age"].as_i64().unwrap() as i32,
                    "age scene{scene}tick{tick}"
                );
                assert_eq!(
                    item.pickup_delay,
                    expected["pickup_delay"].as_i64().unwrap() as i32
                );
                assert_eq!(item.removed, expected["removed"].as_bool().unwrap());
                assert_eq!(
                    item.position.on_ground,
                    expected["on_ground"].as_bool().unwrap()
                );
                assert_eq!(
                    step.merge_due,
                    expected["merge_due"].as_bool().unwrap(),
                    "merge scene{scene}tick{tick}"
                );
                assert_eq!(
                    step.velocity_changed,
                    expected["velocity_changed"].as_bool().unwrap(),
                    "velocityflag scene{scene}tick{tick}"
                );
            }
        }
    }
    #[test]
    fn actual_java_player_drop_kinematics_and_rng_consumption() {
        verify_original_player_drop(false);
    }
    #[test]
    fn actual_java_player_drop_factory_after_world_draws_and_retained_rng() {
        verify_original_player_drop(true);
    }
    fn verify_original_player_drop(use_factory: bool) {
        let vectors: serde_json::Value =
            serde_json::from_str(include_str!("../tools/item_entity_drop_vectors.json")).unwrap();
        let mut factory_calls = 0;
        for (index, q) in vectors["cases"].as_array().unwrap().iter().enumerate() {
            let position = Position {
                x: q["x"].as_f64().unwrap(),
                y: q["y"].as_f64().unwrap(),
                z: q["z"].as_f64().unwrap(),
                yaw: q["yaw"].as_f64().unwrap() as f32,
                pitch: q["pitch"].as_f64().unwrap() as f32,
                on_ground: true,
            };
            let mut uuid = [0u8; 16];
            uuid[6] = 0x40;
            uuid[8] = 0x80;
            uuid[15] = 1;
            let drop = PlayerDrop {
                player_uuid: uuid,
                position,
                eye_height: q["eye_height"].as_f64().unwrap() as f32,
                around: q["around"].as_bool().unwrap(),
                trace_item: q["trace"].as_bool().unwrap(),
            };
            let stack = if q["stack"].is_null() {
                ItemStack::empty()
            } else {
                ItemStack::new(1, q["stack"]["count"].as_i64().unwrap() as i32).unwrap()
            };
            let mut player = LegacyRandomSource::new(q["seed"].as_i64().unwrap());
            let mut world = LegacyRandomSource::new(12345);
            let mut entity = if let Some(state) = q["entity_rng_state_after"].as_str() {
                let mut state = state.parse::<u64>().unwrap();
                for _ in 0..6 {
                    state =
                        state.wrapping_sub(11).wrapping_mul(246154705703781) & ((1u64 << 48) - 1);
                }
                LegacyRandomSource::new((state ^ 25214903917) as i64)
            } else {
                LegacyRandomSource::new(1)
            };
            let actual = if use_factory {
                use std::{cell::RefCell, rc::Rc};
                struct FactoryWorld(Rc<RefCell<LegacyRandomSource>>);
                impl RandomSource for FactoryWorld {
                    fn set_seed(&mut self, _: i64) {
                        panic!("unexpected world RNG operation");
                    }
                    fn fork(&mut self) -> crate::random::RandomSourceKind {
                        panic!("unexpected world RNG operation");
                    }
                    fn next_i32(&mut self) -> i32 {
                        panic!("unexpected world RNG operation");
                    }
                    fn next_i64(&mut self) -> i64 {
                        panic!("unexpected world RNG operation");
                    }
                    fn next_bounded_i32(
                        &mut self,
                        _: i32,
                    ) -> std::result::Result<i32, crate::random::InvalidBound> {
                        panic!("unexpected world RNG operation");
                    }
                    fn next_bool(&mut self) -> bool {
                        panic!("unexpected world RNG operation");
                    }
                    fn next_f32(&mut self) -> f32 {
                        panic!("unexpected world RNG operation");
                    }
                    fn next_f64(&mut self) -> f64 {
                        self.0.borrow_mut().next_f64()
                    }
                    fn consume_count(&mut self, _: i32) {
                        panic!("unexpected world RNG operation");
                    }
                    fn fork_positional(&mut self) -> crate::random::PositionalRandomFactory {
                        panic!("unexpected world RNG operation");
                    }
                }
                let state = Rc::new(RefCell::new(world));
                let mut supplied = FactoryWorld(state.clone());
                let created = spawn_player_drop_with_factory(
                    17,
                    drop,
                    stack,
                    &mut player,
                    &mut supplied,
                    || {
                        factory_calls += 1;
                        // Original Player.drop's recorded level RNG endpoint is
                        // already reached when Entity.RandomSource.create runs:
                        // both default-velocity double arguments were evaluated.
                        let mut observed = state.borrow().clone();
                        assert_eq!(
                            observed.next_i32(),
                            q["world_rng_next_int"].as_i64().unwrap() as i32,
                            "seed factory before original world arguments case{index}"
                        );
                        Ok(entity.clone())
                    },
                )
                .unwrap();
                world = state.borrow().clone();
                created.map(|(item, retained)| {
                    entity = retained;
                    item
                })
            } else {
                spawn_player_drop(17, drop, stack, &mut player, &mut world, &mut entity).unwrap()
            };
            assert_eq!(
                player.next_i32(),
                q["player_rng_next_int"].as_i64().unwrap() as i32,
                "player RNG case{index}"
            );
            assert_eq!(
                world.next_i32(),
                q["world_rng_next_int"].as_i64().unwrap() as i32,
                "world RNG case{index}"
            );
            let Some(actual) = actual else {
                assert_eq!(q["empty"], true);
                continue;
            };
            for (field, values) in [
                (
                    "position_bits",
                    [actual.position.x, actual.position.y, actual.position.z],
                ),
                (
                    "velocity_bits",
                    [actual.velocity.x, actual.velocity.y, actual.velocity.z],
                ),
            ] {
                for (axis, value) in values.into_iter().enumerate() {
                    assert_eq!(
                        value.to_bits(),
                        q[field][axis].as_str().unwrap().parse::<u64>().unwrap(),
                        "{field} case{index}axis{axis}"
                    );
                }
            }
            if let Some(state) = q["entity_rng_state_after"].as_str() {
                assert_eq!(
                    entity.state48(),
                    state.parse::<u64>().unwrap(),
                    "constructor RNG case{index}"
                );
                let uuid = q["entity_uuid"].as_str().unwrap().replace('-', "");
                let expected = (0..16)
                    .map(|i| u8::from_str_radix(&uuid[i * 2..i * 2 + 2], 16).unwrap())
                    .collect::<Vec<_>>();
                assert_eq!(
                    actual.uuid.as_slice(),
                    expected.as_slice(),
                    "UUID case{index}"
                );
                assert_eq!(
                    actual.bob_offset.to_bits(),
                    q["bob_bits"].as_str().unwrap().parse::<u32>().unwrap(),
                    "bob case{index}"
                );
                assert_eq!(
                    actual.position.yaw.to_bits(),
                    q["yaw_bits"].as_str().unwrap().parse::<u32>().unwrap(),
                    "yaw case{index}"
                );
                use base64::Engine;
                let packets = actual.spawn_packets().unwrap();
                assert_eq!(
                    packets[0].payload,
                    base64::engine::general_purpose::STANDARD
                        .decode(q["spawn_packet"].as_str().unwrap())
                        .unwrap(),
                    "AddEntity case{index}"
                );
                assert_eq!(
                    packets[1].payload,
                    base64::engine::general_purpose::STANDARD
                        .decode(q["metadata_packet"].as_str().unwrap())
                        .unwrap(),
                    "ItemSlot8 case{index}"
                );
            }
            assert_eq!(actual.pickup_delay, 40);
            assert_eq!(actual.age, 0);
            assert_eq!(actual.health, 5);
            assert_eq!(actual.thrower, drop.trace_item.then_some(uuid));
        }
        if use_factory {
            assert_eq!(
                factory_calls,
                vectors["cases"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|row| row["empty"] != true)
                    .count()
            );
            println!(
                "ITEM_DROP_FACTORY_CONFORMANCE:{}",
                serde_json::json!({"ok":true,"original_drop_cases":vectors["cases"].as_array().unwrap().len(),"factory_calls_after_original_world_rng_endpoint":factory_calls,"differences":0,"scope":"Both drop APIs against original Player.drop samples; factory after two level doubles and retained post UUID/bob/yaw stream. Factory seed source itself is separately proved by RandomSupport clock-override oracle."})
            );
        }
    }
}
