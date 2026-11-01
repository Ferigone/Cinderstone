//! Minecraft Java 1.21 server resource-pack packets and configuration task.
//!
//! The common listener's required-pack rule is server-wide and checks only
//! DECLINED. Configuration completion checks the response action's terminal
//! status, not its UUID. Do not impose an invented per-pack response sequence.
use crate::{
    nbt::Tag,
    protocol::{Packet, Reader, Writer},
};
use anyhow::{bail, ensure, Context, Result};

pub type PackId = [u8; 16];
pub const REQUIRED_PACK_DISCONNECT: &str = "multiplayer.requiredTexturePrompt.disconnect";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourcePackPhase {
    Configuration,
    Play,
}
impl ResourcePackPhase {
    pub fn push_packet_id(self) -> i32 {
        match self {
            Self::Configuration => 0x09,
            Self::Play => 0x46,
        }
    }
    pub fn pop_packet_id(self) -> i32 {
        match self {
            Self::Configuration => 0x08,
            Self::Play => 0x45,
        }
    }
    pub fn response_packet_id(self) -> i32 {
        match self {
            Self::Configuration => 0x06,
            Self::Play => 0x2b,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum ResourcePackAction {
    SuccessfullyLoaded = 0,
    Declined = 1,
    FailedDownload = 2,
    Accepted = 3,
    Downloaded = 4,
    InvalidUrl = 5,
    FailedReload = 6,
    Discarded = 7,
}
impl ResourcePackAction {
    pub fn from_wire(value: i32) -> Result<Self> {
        Ok(match value {
            0 => Self::SuccessfullyLoaded,
            1 => Self::Declined,
            2 => Self::FailedDownload,
            3 => Self::Accepted,
            4 => Self::Downloaded,
            5 => Self::InvalidUrl,
            6 => Self::FailedReload,
            7 => Self::Discarded,
            _ => bail!("invalid resource-pack response action {value}"),
        })
    }
    pub fn is_terminal(self) -> bool {
        !matches!(self, Self::Accepted | Self::Downloaded)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourcePackResponse {
    pub id: PackId,
    pub action: ResourcePackAction,
}
impl ResourcePackResponse {
    pub fn decode(payload: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(payload);
        let id = reader.uuid()?;
        let action = ResourcePackAction::from_wire(reader.varint()?)?;
        ensure!(reader.remaining().is_empty(), "trailing resource-pack response bytes");
        Ok(Self { id, action })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourcePackOffer {
    pub id: PackId,
    pub url: String,
    pub hash: String,
    pub required: bool,
    // Complete anonymous network-NBT Component, not a JSON wire string.
    prompt: Option<Vec<u8>>,
}
impl ResourcePackOffer {
    /// The host treats an absent/empty resource-pack URL as no configured pack.
    /// UUID fallback matches Java UUID.nameUUIDFromBytes(url.getBytes(UTF_8)).
    /// The existing Component codec explicitly rejects unsupported JSON keys.
    pub fn from_server_config(
        id: Option<PackId>,
        url: String,
        hash: String,
        required: bool,
        prompt_json: Option<String>,
    ) -> Result<Self> {
        ensure!(!url.is_empty(), "an empty resource-pack URL disables the configured offer");
        let prompt = match prompt_json.filter(|value| !value.is_empty()) {
            Some(value) => match serde_json::from_str::<serde_json::Value>(&value) {
                Ok(_) => Some(
                    crate::item_nbt::text_component_to_wire(&Tag::String(value.into()))
                        .context("resource-pack prompt component")?,
                ),
                Err(error) => {
                    // DedicatedServerProperties logs an invalid configured
                    // prompt and offers the pack without it. Distinguish JSON
                    // syntax from a valid Component our native codec does not
                    // support: the latter remains an explicit porting boundary.
                    eprintln!("{}", serde_json::json!({
                        "event": "resource_pack_prompt_invalid",
                        "level": "warn",
                        "reason": error.to_string(),
                        "prompt_omitted": true,
                    }));
                    None
                }
            },
            None => None,
        };
        Self::new(id.unwrap_or_else(|| default_pack_id(&url)), url, hash, required, prompt)
    }
    /// An invalid URL scheme or malformed SHA-1 is not silently corrected:
    /// vanilla sends their strings and receives the client's actual result.
    pub fn new(
        id: PackId,
        url: String,
        hash: String,
        required: bool,
        prompt: Option<Vec<u8>>,
    ) -> Result<Self> {
        ensure!(url.encode_utf16().count() <= 32767, "resource-pack URL exceeds wire string limit");
        ensure!(hash.encode_utf16().count() <= 40, "resource-pack hash exceeds 40 characters");
        if let Some(bytes) = &prompt {
            // Validate a supported complete Component and preserve its wire NBT.
            crate::item_nbt::text_component_from_wire(bytes)
                .context("resource-pack prompt network component")?;
        }
        Ok(Self { id, url, hash, required, prompt })
    }
    pub fn prompt(&self) -> Option<&[u8]> {
        self.prompt.as_deref()
    }
    pub fn push_packet(&self, phase: ResourcePackPhase) -> Result<Packet> {
        // Public fields may have changed after construction; retain wire bounds.
        ensure!(self.url.encode_utf16().count() <= 32767, "resource-pack URL exceeds wire string limit");
        ensure!(self.hash.encode_utf16().count() <= 40, "resource-pack hash exceeds 40 characters");
        let mut writer = Writer::new();
        writer.uuid(self.id).string(&self.url).string(&self.hash)
            .bool(self.required).bool(self.prompt.is_some());
        if let Some(prompt) = &self.prompt {
            writer.raw(prompt);
        }
        Ok(Packet { id: phase.push_packet_id(), payload: writer.into_inner() })
    }
}

pub fn default_pack_id(url: &str) -> PackId {
    let mut id = md5::compute(url.as_bytes()).0;
    id[6] = (id[6] & 0x0f) | 0x30;
    id[8] = (id[8] & 0x3f) | 0x80;
    id
}

pub fn pop_packet(phase: ResourcePackPhase, id: Option<PackId>) -> Packet {
    let mut writer = Writer::new();
    writer.bool(id.is_some());
    if let Some(id) = id {
        writer.uuid(id);
    }
    Packet { id: phase.pop_packet_id(), payload: writer.into_inner() }
}

/// Bookkeeping for a packet already offered by this connection. It does not
/// authorize/reject response UUIDs or require ACCEPTED before another action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrackedResourcePack {
    pub offer: ResourcePackOffer,
    pub last_response: Option<ResourcePackAction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourcePackResponseOutcome {
    pub response: ResourcePackResponse,
    pub configuration_finished: bool,
    pub disconnect_reason: Option<&'static str>,
}

#[derive(Clone, Debug)]
pub struct ResourcePackSession {
    server_requires_pack: bool,
    configuration_pending: Option<PackId>,
    offers: Vec<TrackedResourcePack>,
}
impl ResourcePackSession {
    pub fn new(server_requires_pack: bool) -> Self {
        Self { server_requires_pack, configuration_pending: None, offers: Vec::new() }
    }
    pub fn configuration_pending(&self) -> bool {
        self.configuration_pending.is_some()
    }
    pub fn offers(&self) -> &[TrackedResourcePack] {
        &self.offers
    }
    pub fn begin_configuration(&mut self, offer: &ResourcePackOffer) -> Result<Packet> {
        ensure!(self.configuration_pending.is_none(), "resource-pack configuration task already pending");
        let packet = self.push(ResourcePackPhase::Configuration, offer.clone())?;
        self.configuration_pending = Some(offer.id);
        Ok(packet)
    }
    pub fn push(&mut self, phase: ResourcePackPhase, offer: ResourcePackOffer) -> Result<Packet> {
        let packet = offer.push_packet(phase)?;
        if let Some(known) = self.offers.iter_mut().find(|known| known.offer.id == offer.id) {
            *known = TrackedResourcePack { offer, last_response: None };
        } else {
            self.offers.push(TrackedResourcePack { offer, last_response: None });
        }
        Ok(packet)
    }
    pub fn pop(&mut self, phase: ResourcePackPhase, id: Option<PackId>) -> Packet {
        match id {
            Some(id) => self.offers.retain(|known| known.offer.id != id),
            None => self.offers.clear(),
        }
        // Pop is a client stack operation, not completion of a configuration task.
        pop_packet(phase, id)
    }
    pub fn handle_response(
        &mut self,
        phase: ResourcePackPhase,
        payload: &[u8],
    ) -> Result<ResourcePackResponseOutcome> {
        let response = ResourcePackResponse::decode(payload)?;
        let disconnect_reason = (self.server_requires_pack
            && response.action == ResourcePackAction::Declined)
            .then_some(REQUIRED_PACK_DISCONNECT);
        let configuration_finished = phase == ResourcePackPhase::Configuration
            && response.action.is_terminal();
        if configuration_finished {
            // ServerConfigurationPacketListenerImpl finishes by task TYPE. It
            // does not compare the response UUID with the offered pack UUID.
            ensure!(self.configuration_pending.is_some(), "unexpected terminal resource-pack response without its configuration task");
            self.configuration_pending = None;
        }
        if let Some(known) = self.offers.iter_mut().find(|known| known.offer.id == response.id) {
            known.last_response = Some(response.action);
        }
        Ok(ResourcePackResponseOutcome { response, configuration_finished, disconnect_reason })
    }
}
