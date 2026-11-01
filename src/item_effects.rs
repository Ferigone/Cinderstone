//! Vanilla 1.21 packets produced by the proved ItemEntity world callbacks.
//!
//! The coordinator must consume the level's actual threadSafeRandom nextLong before calling
//! this encoder. Game-event listeners and lifecycle callbacks are separate
//! synchronous host operations; these encoders do not replace them.
use crate::{
    entity_base::BaseEffect,
    entity_state::EntityPhysicalState,
    geometry::Vec3,
    protocol::{Packet, Writer},
    runtime_item_world::{ItemSound, RuntimeItemEffect},
};
use anyhow::{bail, ensure, Context, Result};

#[derive(Clone, Debug, PartialEq)]
pub struct EffectBroadcast {
    pub packet: Packet,
    /// Original broadcast coordinates, before sound-packet quantization.
    pub origin: Vec3,
    pub radius: f64,
}
impl EffectBroadcast {
    /// PlayerList.broadcast uses the actual player position, dimension
    /// identity and a strict squared-distance comparison. It does not require
    /// the source chunk to be loaded by that player's connection.
    pub fn in_range(&self, player_position: Vec3, same_dimension: bool) -> bool {
        if !same_dimension {
            return false;
        }
        let dx = self.origin.x - player_position.x;
        let dy = self.origin.y - player_position.y;
        let dz = self.origin.z - player_position.z;
        (dx * dx + dy * dy) + dz * dz < self.radius * self.radius
    }
}

/// Registry-reference sounds for the unchanged vanilla 1.21 sound registry.
/// ItemEntity's inherited getSoundSource is AMBIENT, ordinal 8.
pub fn sound_packet(
    sound: ItemSound,
    position: Vec3,
    volume: f32,
    pitch: f32,
    seed: i64,
) -> EffectBroadcast {
    let registry_id = match sound {
        ItemSound::GenericBurn => 572,
        ItemSound::GenericExtinguishFire => 577,
        ItemSound::GenericSplash => 580,
    };
    let mut out = Writer::new();
    // SoundEvent.STREAM_CODEC uses zero for direct values and id+1 for a
    // registered Holder.Reference. These three events are variable-range.
    out.varint(registry_id + 1)
        .varint(8)
        .i32((position.x * 8.) as i32)
        .i32((position.y * 8.) as i32)
        .i32((position.z * 8.) as i32)
        .f32(volume)
        .f32(pitch)
        .i64(seed);
    EffectBroadcast {
        packet: Packet {
            id: 0x68,
            payload: out.into_inner(),
        },
        origin: position,
        radius: f64::from(if volume > 1. { 16_f32 * volume } else { 16. }),
    }
}

/// Exact ClientboundLevelEventPacket payload. BlockPos.asLong masks its input
/// coordinates; it does not reject positions outside the packed bit ranges.
pub fn level_event_packet(
    event: i32,
    position: (i32, i32, i32),
    data: i32,
    global: bool,
) -> Packet {
    let (x, y, z) = position;
    let packed = ((i64::from(x) & 0x3ff_ffff) << 38)
        | ((i64::from(z) & 0x3ff_ffff) << 12)
        | (i64::from(y) & 0xfff);
    let mut out = Writer::new();
    out.i32(event).i64(packed).i32(data).bool(global);
    Packet {
        id: 0x28,
        payload: out.into_inner(),
    }
}

/// Encode the packet portion of an already-executed synchronous world effect.
/// HitGround/Splash are game events and carry no packet of their own. The host
/// still has to execute their listeners or establish the no-listener context.
pub fn encode_effect(
    state: &EntityPhysicalState,
    effect: RuntimeItemEffect,
    sound_seed: Option<i64>,
) -> Result<Option<EffectBroadcast>> {
    if let RuntimeItemEffect::Sound {
        sound,
        volume,
        pitch,
    } = effect
    {
        return Ok(Some(sound_packet(
            sound,
            state.position_vec(),
            volume,
            pitch,
            sound_seed.context("item sound requires the actual level RNG seed")?,
        )));
    }
    ensure!(
        sound_seed.is_none(),
        "non-sound item effect must not consume a sound seed"
    );
    match effect {
        RuntimeItemEffect::Base(BaseEffect::MeltFrozen {
            position,
            event,
            data,
        }) => Ok(Some(EffectBroadcast {
            packet: level_event_packet(event, position, data, false),
            origin: Vec3::new(
                f64::from(position.0),
                f64::from(position.1),
                f64::from(position.2),
            ),
            radius: 64.,
        })),
        RuntimeItemEffect::HitGround { .. }
        | RuntimeItemEffect::Splash
        | RuntimeItemEffect::Base(BaseEffect::DiscardBelowWorld) => Ok(None),
        RuntimeItemEffect::Base(BaseEffect::StopRiding { .. }) => {
            bail!("item stop-riding lifecycle effects unavailable")
        }
        RuntimeItemEffect::Base(BaseEffect::LavaBurnSound { .. }) => {
            bail!("raw lava-burn effect must be dispatched as a seeded item sound")
        }
        RuntimeItemEffect::Sound { .. } => unreachable!(),
    }
}

#[cfg(test)]
#[path = "../tools/item_effects_tests.rs"]
mod tests;
