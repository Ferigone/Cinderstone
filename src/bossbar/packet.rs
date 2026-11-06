//! `ClientboundBossEventPacket`: the six operations, framed as the clientbound play packet.

use super::{BossBarColor, BossBarOverlay, Uuid};
use crate::components::text::{Component, ComponentError};
use crate::protocol::Writer;

/// The clientbound play packet id of `minecraft:boss_event`.
pub const PACKET_ID: i32 = 0x0A;

/// One operation of the packet, with the data it carries.
#[derive(Debug, Clone, PartialEq)]
pub enum BossOperation {
    Add {
        name: Component,
        progress: f32,
        color: BossBarColor,
        overlay: BossBarOverlay,
        darken_screen: bool,
        play_music: bool,
        create_world_fog: bool,
    },
    Remove,
    UpdateProgress(f32),
    UpdateName(Component),
    UpdateStyle {
        color: BossBarColor,
        overlay: BossBarOverlay,
    },
    UpdateProperties {
        darken_screen: bool,
        play_music: bool,
        create_world_fog: bool,
    },
}

/// `ClientboundBossEventPacket`.
#[derive(Debug, Clone, PartialEq)]
pub struct BossPacket {
    pub id: Uuid,
    pub operation: BossOperation,
}

/// `ClientboundBossEventPacket.encodeProperties`.
fn properties(darken_screen: bool, play_music: bool, create_world_fog: bool) -> u8 {
    u8::from(darken_screen) | (u8::from(play_music) << 1) | (u8::from(create_world_fog) << 2)
}

impl BossPacket {
    /// `OperationType.ordinal()`.
    fn operation_type(&self) -> i32 {
        match self.operation {
            BossOperation::Add { .. } => 0,
            BossOperation::Remove => 1,
            BossOperation::UpdateProgress(_) => 2,
            BossOperation::UpdateName(_) => 3,
            BossOperation::UpdateStyle { .. } => 4,
            BossOperation::UpdateProperties { .. } => 5,
        }
    }

    /// The packet body: the UUID, the operation type and the operation's own data.
    pub fn body(&self) -> Result<Vec<u8>, ComponentError> {
        let mut writer = Writer::new();
        writer.uuid(self.id);
        writer.varint(self.operation_type());
        match &self.operation {
            BossOperation::Add {
                name,
                progress,
                color,
                overlay,
                darken_screen,
                play_music,
                create_world_fog,
            } => {
                writer.raw(&name.to_network_nbt()?);
                writer.f32(*progress);
                writer.varint(color.ordinal());
                writer.varint(overlay.ordinal());
                writer.u8(properties(*darken_screen, *play_music, *create_world_fog));
            }
            BossOperation::Remove => {}
            BossOperation::UpdateProgress(progress) => {
                writer.f32(*progress);
            }
            BossOperation::UpdateName(name) => {
                writer.raw(&name.to_network_nbt()?);
            }
            BossOperation::UpdateStyle { color, overlay } => {
                writer.varint(color.ordinal());
                writer.varint(overlay.ordinal());
            }
            BossOperation::UpdateProperties {
                darken_screen,
                play_music,
                create_world_fog,
            } => {
                writer.u8(properties(*darken_screen, *play_music, *create_world_fog));
            }
        }
        Ok(writer.into_inner())
    }

    /// The packet as a frame: the play packet id, then the body.
    pub fn frame(&self) -> Result<Vec<u8>, ComponentError> {
        let mut writer = Writer::new();
        writer.varint(PACKET_ID);
        writer.raw(&self.body()?);
        Ok(writer.into_inner())
    }
}
