//! Shared `ServerEntity.sendChanges` publication for an unmounted item entity.
//!
//! Keep one sender per authoritative item, independently of its observers.
//! Physical item-stack mutation is not automatically a metadata publication:
//! vanilla refreshes its whole pairing cache only when it drains dirty data.
//! Publications are completed packet batches; this API does not expose
//! reentrant packet-consumer callbacks during the original send operation.
use crate::{
    entity::{velocity_packet, EntityMotionTracker, MotionTrackingOptions},
    item_entity::ItemState,
    protocol::Packet,
    replication::EntitySnapshot,
};
use anyhow::{ensure, Result};

#[derive(Debug, Clone)]
pub struct ItemReplication {
    motion: EntityMotionTracker,
    cached_metadata: ItemState,
}

/// One globally ordered publication, to be delivered to the current viewers.
/// A host must not replace `packets` with a diff between the two snapshots:
/// dirty entries may return to their previous value and still require sending.
#[derive(Debug, Clone)]
pub struct ItemPublication {
    pub packets: Vec<Packet>,
    pub snapshot: EntitySnapshot<ItemState>,
    pub dirty_mask: u16,
}

impl ItemReplication {
    /// Captures the initial non-default metadata without draining constructor
    /// dirtiness, exactly as the original ServerEntity constructor does.
    pub fn new(item: &ItemState) -> Result<Self> {
        item.validate()?;
        ensure!(
            item.vehicle.is_none(),
            "mounted item tracking is not implemented"
        );
        Ok(Self {
            motion: EntityMotionTracker::with_options(
                item.entity_id,
                item.position,
                item.velocity,
                MotionTrackingOptions::ITEM,
            )?,
            cached_metadata: item.clone(),
        })
    }

    fn validate_identity(&self, item: &ItemState) -> Result<()> {
        ensure!(
            item.entity_id == self.cached_metadata.entity_id
                && item.uuid == self.cached_metadata.uuid,
            "item sender identity changed"
        );
        ensure!(
            item.vehicle.is_none(),
            "mounted item tracking is not implemented"
        );
        item.position.validate()?;
        ensure!(
            [item.velocity.x, item.velocity.y, item.velocity.z]
                .iter()
                .all(|value| value.is_finite()),
            "non-finite item velocity"
        );
        ensure!(
            item.metadata_dirty & !0x1ff == 0,
            "unknown item metadata ID"
        );
        Ok(())
    }

    /// A late viewer receives cached metadata and the shared sent movement
    /// baselines. Visibility uses the current physical position instead.
    pub fn snapshot(&self, item: &ItemState) -> Result<EntitySnapshot<ItemState>> {
        self.validate_identity(item)?;
        let mut published = item.clone();
        published.stack = self.cached_metadata.stack.clone();
        published.stack_is_default_reference = self.cached_metadata.stack_is_default_reference;
        published.shared_flags = self.cached_metadata.shared_flags;
        published.air_supply = self.cached_metadata.air_supply;
        published.custom_name = self.cached_metadata.custom_name.clone();
        published.custom_name_visible = self.cached_metadata.custom_name_visible;
        published.silent = self.cached_metadata.silent;
        published.no_gravity = self.cached_metadata.no_gravity;
        published.pose = self.cached_metadata.pose;
        published.ticks_frozen = self.cached_metadata.ticks_frozen;
        published.metadata_dirty = 0;
        published.velocity = self.motion.spawn_velocity();
        Ok(EntitySnapshot {
            entity: published,
            spawn_position: self.motion.spawn_position(),
            spawn_velocity: Some(self.motion.spawn_velocity()),
        })
    }

    pub fn cached_metadata_packet(&self) -> Result<Option<Packet>> {
        self.cached_metadata.metadata_packet()
    }

    pub fn tracking_tick_count(&self) -> i32 {
        self.motion.tracking_tick_count()
    }

    pub fn teleport_delay(&self) -> i32 {
        self.motion.teleport_delay()
    }

    /// Call once in the level's shared tracking phase, before that level tick's
    /// EntityTickList simulation, using the state left by the previous phase.
    /// Packet order is movement, dirty metadata, then optional hurt velocity.
    /// The hurt path deliberately does not change the sent-velocity baseline.
    pub fn send_changes(&mut self, item: &mut ItemState) -> Result<ItemPublication> {
        self.validate_identity(item)?;
        let dirty_mask = item.metadata_dirty;
        // Encode before mutating either state so an invalid component payload
        // cannot consume a dirty entry or advance the shared movement counter.
        let metadata = item.dirty_metadata_packet(dirty_mask)?;
        let hurt_velocity = if item.hurt_marked {
            Some(velocity_packet(item.entity_id, item.velocity)?)
        } else {
            None
        };
        let mut packets = self.motion.collect_movement_updates(
            item.position,
            item.velocity,
            dirty_mask != 0,
            item.has_impulse,
        )?;
        item.take_metadata_dirty();
        if let Some(metadata) = metadata {
            // getNonDefaultValues copies every current value, including Slot8
            // after an in-place count change, even when only another ID dirtied.
            self.cached_metadata = item.clone();
            packets.push(metadata);
        }
        // A true impulse necessarily enters the interval gate. Clearing false
        // outside that gate has the same state effect without a second clock.
        item.has_impulse = false;
        self.motion.advance_tracking_tick();
        if let Some(packet) = hurt_velocity {
            item.hurt_marked = false;
            packets.push(packet);
        }
        Ok(ItemPublication {
            packets,
            snapshot: self.snapshot(item)?,
            dirty_mask,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        entity::Position, geometry::Vec3, inventory::ItemStack, replication::ReplicatedEntity,
    };
    use base64::{engine::general_purpose::STANDARD, Engine};
    use serde_json::Value;

    fn stack(value: &Value) -> ItemStack {
        ItemStack::decode_exact(&STANDARD.decode(value.as_str().unwrap()).unwrap()).unwrap()
    }
    fn vector(value: &Value) -> Vec3 {
        Vec3::new(
            value[0].as_f64().unwrap(),
            value[1].as_f64().unwrap(),
            value[2].as_f64().unwrap(),
        )
    }
    fn position(value: &Value) -> Position {
        Position {
            x: value["x"].as_f64().unwrap(),
            y: value["y"].as_f64().unwrap(),
            z: value["z"].as_f64().unwrap(),
            yaw: value["yaw"].as_f64().unwrap() as f32,
            pitch: value["pitch"].as_f64().unwrap() as f32,
            on_ground: value["on_ground"].as_bool().unwrap(),
        }
    }
    fn assert_vector(actual: Vec3, expected: &Value, context: &str) {
        for (index, actual) in [actual.x, actual.y, actual.z].into_iter().enumerate() {
            let expected = u64::from_str_radix(expected[index].as_str().unwrap(), 16).unwrap();
            assert_eq!(actual.to_bits(), expected, "{context} axis {index}");
        }
    }
    fn assert_packets(actual: &[Packet], expected: &Value, context: &str) {
        let expected = expected.as_array().unwrap();
        assert_eq!(actual.len(), expected.len(), "{context} packet count");
        for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
            assert_eq!(
                actual.id,
                expected["id"].as_i64().unwrap() as i32,
                "{context} packet {index} ID"
            );
            assert_eq!(
                actual.payload,
                STANDARD
                    .decode(expected["payload"].as_str().unwrap())
                    .unwrap(),
                "{context} packet {index} body"
            );
        }
    }
    fn assert_state(sender: &ItemReplication, item: &ItemState, expected: &Value, context: &str) {
        let snapshot = sender.snapshot(item).unwrap();
        let spawn = snapshot.spawn_position;
        assert_vector(
            Vec3::new(spawn.x, spawn.y, spawn.z),
            &expected["spawn"]["position_bits"],
            context,
        );
        for (actual, key) in [(spawn.yaw, "yaw_bits"), (spawn.pitch, "pitch_bits")] {
            assert_eq!(
                actual.to_bits(),
                u32::from_str_radix(expected["spawn"][key].as_str().unwrap(), 16).unwrap(),
                "{context} {key}"
            );
        }
        assert_vector(
            snapshot.spawn_velocity.unwrap(),
            &expected["sent_velocity_bits"],
            context,
        );
        let actual_metadata = sender.cached_metadata_packet().unwrap();
        if expected["cached_metadata"].is_null() {
            assert!(actual_metadata.is_none(), "{context} metadata cache");
        } else {
            assert_packets(
                &[actual_metadata.unwrap()],
                &Value::Array(vec![expected["cached_metadata"].clone()]),
                context,
            );
        }
        assert_eq!(
            item.metadata_dirty,
            expected["dirty_mask"].as_u64().unwrap() as u16,
            "{context} dirty mask"
        );
        assert_eq!(
            item.metadata_dirty != 0,
            expected["dirty"].as_bool().unwrap(),
            "{context} dirty"
        );
        assert_eq!(
            item.has_impulse,
            expected["has_impulse"].as_bool().unwrap(),
            "{context} impulse"
        );
        assert_eq!(
            item.hurt_marked,
            expected["hurt_marked"].as_bool().unwrap(),
            "{context} hurt"
        );
        assert_eq!(
            sender.tracking_tick_count(),
            expected["tick_count"].as_i64().unwrap() as i32,
            "{context} tick"
        );
        assert_eq!(
            sender.teleport_delay(),
            expected["teleport_delay"].as_i64().unwrap() as i32,
            "{context} delay"
        );
        assert_vector(
            item.position_vec(),
            &expected["physical_position_bits"],
            context,
        );
        assert_vector(item.velocity, &expected["physical_velocity_bits"], context);
        assert_eq!(
            item.stack.encode(),
            STANDARD
                .decode(expected["physical_stack_wire"].as_str().unwrap())
                .unwrap(),
            "{context} physical stack"
        );
        assert_eq!(
            item.removed,
            expected["physical_removed"].as_bool().unwrap(),
            "{context} removal"
        );
        // Snapshot visibility follows physical position while its metadata and
        // velocity remain published values; pending changes cannot leak here.
        assert_eq!(
            snapshot.entity.position, item.position,
            "{context} visibility position"
        );
        assert_vector(
            snapshot.entity.velocity,
            &expected["sent_velocity_bits"],
            context,
        );
        assert_eq!(
            item.stack_is_default_reference,
            expected["physical_stack_reference_is_default"]
                .as_bool()
                .unwrap(),
            "{context} Slot8 default reference"
        );
        if let Some(pairing) = expected.get("pairing_packets") {
            let packets = snapshot
                .entity
                .spawn_packets_with_velocity(snapshot.spawn_position, snapshot.spawn_velocity)
                .unwrap();
            assert_packets(&packets, pairing, context);
        }
    }

    fn mutate(item: &mut ItemState, step: &Value) {
        if let Some(value) = step.get("position") {
            let value = position(value);
            item.set_position(Vec3::new(value.x, value.y, value.z));
            item.position.yaw = value.yaw;
            item.position.pitch = value.pitch;
            item.position.on_ground = value.on_ground;
        }
        if let Some(value) = step.get("velocity") {
            item.velocity = vector(value);
        }
        if let Some(value) = step.get("stack_wire") {
            item.set_stack(stack(value));
        }
        if step["empty_reference"].as_bool().unwrap_or(false) {
            item.set_stack_default_empty();
        }
        if let Some(value) = step.get("count") {
            item.set_stack_count_in_place(value.as_i64().unwrap() as i32);
        }
        if let Some(value) = step.get("shrink") {
            let count = item
                .stack
                .count
                .wrapping_sub(value.as_i64().unwrap() as i32);
            item.set_stack_count_in_place(count);
        }
        if let Some(value) = step.get("no_gravity") {
            item.set_no_gravity(value.as_bool().unwrap());
        }
        if let Some(value) = step.get("silent") {
            item.set_silent(value.as_bool().unwrap());
        }
        if let Some(value) = step.get("air_supply") {
            item.set_air_supply(value.as_i64().unwrap() as i32);
        }
        if let Some(value) = step.get("ticks_frozen") {
            item.set_ticks_frozen(value.as_i64().unwrap() as i32);
        }
        if let Some(value) = step.get("shared_flags") {
            let value = value.as_u64().unwrap() as u8;
            if item.shared_flags != value {
                item.shared_flags = value;
                item.metadata_dirty |= 1;
            }
        }
        if let Some(value) = step.get("impulse") {
            item.has_impulse = value.as_bool().unwrap();
        }
        if let Some(value) = step.get("hurt") {
            item.hurt_marked = value.as_bool().unwrap();
        }
        // These inputs are original private cjh.merge calls, bypassing its
        // high-level eligibility policy. Replay its authoritative stack edits;
        // this test proves the sender, not a second merge eligibility policy.
        if let Some(value) = step.get("merge_destination_wire") {
            let source = stack(value);
            let amount =
                (item.stack.max_stack_size().unwrap().min(64) - item.stack.count).min(source.count);
            let mut destination = item.stack.clone();
            destination.count += amount;
            item.set_stack(destination);
        }
        if let Some(value) = step.get("merge_source_wire") {
            let destination = stack(value);
            let amount = (destination.max_stack_size().unwrap().min(64) - destination.count)
                .min(item.stack.count);
            let count = item.stack.count - amount;
            item.set_stack_count_in_place(count);
            if item.stack.is_empty() {
                item.removed = true;
            }
        }
        if let Some(value) = step.get("pickup") {
            let capacity = value.as_i64().unwrap() as i32;
            item.try_pickup([0; 16], 19, |stack| {
                let amount = stack.count.min(capacity.max(0));
                stack.count -= amount;
                Ok(amount > 0)
            })
            .unwrap();
        }
        // set_same_reference invokes SynchedEntityData.set with its current
        // ItemStack object. Keeping this Rust value also keeps dirtiness.
    }

    #[test]
    fn actual_java_shared_item_sender_histories() {
        let fixture: Value =
            serde_json::from_str(include_str!("../fixtures/item_replication_sender.json")).unwrap();
        assert_eq!(fixture["protocol"], 767);
        assert_eq!(fixture["cases"].as_array().unwrap().len(), 94);
        let mut states = 0;
        let mut packets = 0;
        for case in fixture["cases"].as_array().unwrap() {
            let name = case["name"].as_str().unwrap();
            let initial = &case["initial"];
            let mut uuid = [0; 16];
            uuid[6] = 0x40;
            uuid[8] = 0x80;
            uuid[15] = 1;
            let mut item = ItemState::new(
                17,
                uuid,
                position(&initial["position"]),
                vector(&initial["velocity"]),
                stack(&initial["stack_wire"]),
            )
            .unwrap();
            item.set_no_gravity(initial["no_gravity"].as_bool().unwrap());
            let mut sender = ItemReplication::new(&item).unwrap();
            assert_state(&sender, &item, &case["initial_expected"], name);
            states += 1;
            for (index, step) in case["steps"].as_array().unwrap().iter().enumerate() {
                let context = format!("{name} step {index}");
                mutate(&mut item, step);
                let actual = if step["op"] == "pair" {
                    Vec::new()
                } else {
                    sender.send_changes(&mut item).unwrap().packets
                };
                assert_packets(&actual, &step["expected"]["packets"], &context);
                packets += actual.len();
                packets += step["expected"]["pairing_packets"]
                    .as_array()
                    .map_or(0, Vec::len);
                assert_state(&sender, &item, &step["expected"], &context);
                states += 1;
            }
        }
        assert_eq!(states, 5208);
        assert_eq!(packets, 5292);
        println!("actual ItemEntity ServerEntity: {states} snapshots, {packets} original packets");
    }
}
