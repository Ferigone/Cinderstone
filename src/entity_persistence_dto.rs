//! Native item snapshot values. No ItemStack/NBT payload passes through JSON
//! text conversion: component payloads remain raw bytes and have a checked
//! canonical Slot representation alongside the native object state.
use crate::{
    entity_state::EntityPhysicalState, inventory::ItemStack, item_entity::ItemState,
    random::LegacyRandomSource,
};
use anyhow::{ensure, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SavedItem {
    physical: EntityPhysicalState,
    // Native object state is retained even when count<=0 and the optional
    // Slot codec therefore omits its item/component identity entirely.
    stack: ItemStack,
    canonical_slot: String,
    stack_is_default_reference: bool,
    age: i32,
    pickup_delay: i32,
    health: i32,
    target: Option<[u8; 16]>,
    thrower: Option<[u8; 16]>,
    bob_offset: f32,
    random_state48: u64,
}

impl SavedItem {
    pub(super) fn capture(state: &ItemState, random: &LegacyRandomSource) -> Result<Self> {
        validate_state(state)?;
        let value = Self {
            physical: state.physical.clone(),
            stack: state.stack.clone(),
            canonical_slot: STANDARD.encode(state.stack.encode_canonical()?),
            stack_is_default_reference: state.stack_is_default_reference,
            age: state.age,
            pickup_delay: state.pickup_delay,
            health: state.health,
            target: state.target,
            thrower: state.thrower,
            bob_offset: state.bob_offset,
            random_state48: random.state48(),
        };
        Ok(value)
    }

    pub(super) fn entity_id(&self) -> i32 {
        self.physical.entity_id
    }

    pub(super) fn uuid(&self) -> [u8; 16] {
        self.physical.uuid
    }

    pub(super) fn restore(self) -> Result<(ItemState, LegacyRandomSource)> {
        let wire = STANDARD.decode(&self.canonical_slot)?;
        // Decode the complete stored Slot as well as validating that the raw
        // native object describes the same network value. Retain the raw
        // object, rather than replacing its empty-reference/object state.
        ItemStack::decode_exact(&wire)?;
        ensure!(
            self.stack.encode_canonical()? == wire,
            "native saved stack differs from its canonical Slot payload"
        );
        let state = ItemState {
            physical: self.physical,
            stack: self.stack,
            stack_is_default_reference: self.stack_is_default_reference,
            age: self.age,
            pickup_delay: self.pickup_delay,
            health: self.health,
            target: self.target,
            thrower: self.thrower,
            bob_offset: self.bob_offset,
        };
        validate_state(&state)?;
        Ok((state, super::restore_random(self.random_state48)?))
    }
}

fn validate_state(state: &ItemState) -> Result<()> {
    state.validate()?;
    let p = &state.physical;
    // serde_json cannot represent NaN/Inf. Reject the whole native snapshot
    // before writing instead of silently converting any runtime float to null.
    let doubles = [
        p.position.x,
        p.position.y,
        p.position.z,
        p.velocity.x,
        p.velocity.y,
        p.velocity.z,
        p.bounds.min_x,
        p.bounds.min_y,
        p.bounds.min_z,
        p.bounds.max_x,
        p.bounds.max_y,
        p.bounds.max_z,
        p.old_position.x,
        p.old_position.y,
        p.old_position.z,
        p.stuck_speed_multiplier.x,
        p.stuck_speed_multiplier.y,
        p.stuck_speed_multiplier.z,
        p.piston_totals[0],
        p.piston_totals[1],
        p.piston_totals[2],
        p.fluid_heights[0],
        p.fluid_heights[1],
    ];
    let floats = [
        p.position.yaw,
        p.position.pitch,
        p.width,
        p.height,
        p.eye_height,
        p.old_yaw,
        p.old_pitch,
        p.walk_dist_old,
        p.walk_dist,
        p.move_dist,
        p.fly_dist,
        p.next_step,
        p.fall_distance,
        state.bob_offset,
    ];
    ensure!(
        doubles.into_iter().all(f64::is_finite) && floats.into_iter().all(f32::is_finite),
        "nonfinite runtime item fields cannot be saved in native JSON"
    );
    ensure!(
        p.width >= 0.
            && p.height >= 0.
            && p.bounds.min_x <= p.bounds.max_x
            && p.bounds.min_y <= p.bounds.max_y
            && p.bounds.min_z <= p.bounds.max_z,
        "invalid native item dimensions/bounds"
    );
    ensure!(
        !state.stack_is_default_reference || state.stack.is_empty(),
        "canonical ItemStack.EMPTY reference cannot contain a nonempty stack"
    );
    ensure!(
        p.metadata_dirty & !0x1ff == 0,
        "native item metadata dirty mask has unknown fields"
    );
    // Validate outgoing metadata/NBT while the owner still has a complete
    // transaction to reject. This does not consume dirty state or RNG draws.
    state.stack.encode_canonical()?;
    if state.stack.is_empty() && state.stack.item_id != 0 {
        // OPTIONAL_STREAM_CODEC hides an emptied object's components. Check
        // those retained bytes using a nonempty view without changing the
        // native object's actual count or its reference/dirty state.
        let mut probe = state.stack.clone();
        probe.count = 1;
        probe.encode_canonical()?;
    }
    state.spawn_packets()?;
    Ok(())
}
