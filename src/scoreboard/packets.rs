//! The five scoreboard packets, as the clientbound play packets the original sends.

use super::criteria::RenderType;
use super::format::{
    write_optional_component, write_optional_format, ChatFormatting, NumberFormat,
};
use super::model::DisplaySlot;
use crate::components::text::{Component, ComponentError, Result};
use crate::protocol::Writer;

/// `ClientboundSetPlayerTeamPacket.Parameters`.
#[derive(Debug, Clone, PartialEq)]
pub struct TeamParameters {
    pub display_name: Component,
    pub options: i32,
    pub nametag_visibility: String,
    pub collision_rule: String,
    /// `None` is the `null` colour a malformed `scoreboard.dat` can leave; encoding it throws.
    pub color: Option<ChatFormatting>,
    pub prefix: Component,
    pub suffix: Component,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScoreboardPacket {
    /// `ClientboundSetObjectivePacket`; `method` is 0 (add), 1 (remove) or 2 (change).
    SetObjective {
        name: String,
        method: i32,
        display_name: Component,
        render_type: RenderType,
        number_format: Option<NumberFormat>,
    },
    /// `ClientboundSetScorePacket`.
    SetScore {
        owner: String,
        objective: String,
        score: i32,
        display: Option<Component>,
        number_format: Option<NumberFormat>,
    },
    /// `ClientboundResetScorePacket`.
    ResetScore {
        owner: String,
        objective: Option<String>,
    },
    /// `ClientboundSetDisplayObjectivePacket`; an empty name clears the slot.
    SetDisplayObjective {
        slot: DisplaySlot,
        objective: String,
    },
    /// `ClientboundSetPlayerTeamPacket`; `method` 0 add, 1 remove, 2 change, 3 join, 4 leave.
    SetPlayerTeam {
        name: String,
        method: i32,
        parameters: Option<TeamParameters>,
        players: Vec<String>,
    },
}

impl ScoreboardPacket {
    /// The packet id in the 1.21 clientbound play protocol.
    pub fn id(&self) -> i32 {
        match self {
            Self::SetObjective { .. } => 0x5E,
            Self::SetScore { .. } => 0x61,
            Self::ResetScore { .. } => 0x44,
            Self::SetDisplayObjective { .. } => 0x57,
            Self::SetPlayerTeam { .. } => 0x60,
        }
    }

    /// The packet body (no id, no length prefix).
    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut out = Writer::new();
        match self {
            Self::SetObjective {
                name,
                method,
                display_name,
                render_type,
                number_format,
            } => {
                out.string(name).i8(*method as i8);
                if *method == 0 || *method == 2 {
                    out.raw(&display_name.to_network_nbt()?);
                    out.varint(render_type.ordinal());
                    write_optional_format(&mut out, number_format.as_ref())?;
                }
            }
            Self::SetScore {
                owner,
                objective,
                score,
                display,
                number_format,
            } => {
                out.string(owner).string(objective).varint(*score);
                write_optional_component(&mut out, display.as_ref())?;
                write_optional_format(&mut out, number_format.as_ref())?;
            }
            Self::ResetScore { owner, objective } => {
                out.string(owner).bool(objective.is_some());
                if let Some(objective) = objective {
                    out.string(objective);
                }
            }
            Self::SetDisplayObjective { slot, objective } => {
                out.varint(slot.id()).string(objective);
            }
            Self::SetPlayerTeam {
                name,
                method,
                parameters,
                players,
            } => {
                out.string(name).i8(*method as i8);
                if *method == 0 || *method == 2 {
                    let parameters = parameters
                        .as_ref()
                        .expect("parameters accompany method 0 and 2");
                    out.raw(&parameters.display_name.to_network_nbt()?);
                    out.i8(parameters.options as i8);
                    out.string(&parameters.nametag_visibility)
                        .string(&parameters.collision_rule);
                    let color = parameters.color.ok_or_else(|| {
                        ComponentError::Codec(
                            "NullPointerException: team colour is null".to_owned(),
                        )
                    })?;
                    out.varint(color.ordinal());
                    out.raw(&parameters.prefix.to_network_nbt()?);
                    out.raw(&parameters.suffix.to_network_nbt()?);
                }
                if matches!(method, 0 | 3 | 4) {
                    out.varint(players.len() as i32);
                    for player in players {
                        out.string(player);
                    }
                }
            }
        }
        Ok(out.into_inner())
    }

    /// Packet id (VarInt) followed by the body: what `ProtocolInfo.codec()` writes.
    pub fn encode_frame(&self) -> Result<Vec<u8>> {
        let mut out = Writer::new();
        out.varint(self.id());
        out.raw(&self.encode()?);
        Ok(out.into_inner())
    }
}
