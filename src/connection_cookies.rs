//! Java 1.21 connection-cookie wire data and vanilla server listener behavior.
//!
//! Cookies belong to the remote client's connection/transfer lifecycle, never
//! the world or player save. Vanilla's login and common server listeners reject
//! every cookie response as an unexpected query response; they have no pending
//! cookie-query registry. Merely constructing a request must not invent one.
use crate::protocol::{Packet, Reader, Writer};
use anyhow::{ensure, Context, Result};

pub const MAX_COOKIE_PAYLOAD_SIZE: usize = 5 * 1024;
pub const MAX_COOKIE_IDENTIFIER_CHARS: usize = 32767;
pub const UNEXPECTED_COOKIE_RESPONSE_REASON: &str =
    "multiplayer.disconnect.unexpected_query_response";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CookiePhase {
    Login,
    Configuration,
    Play,
}
impl CookiePhase {
    pub const fn request_packet_id(self) -> i32 {
        match self {
            Self::Login => 0x05,
            Self::Configuration => 0x00,
            Self::Play => 0x16,
        }
    }
    pub const fn response_packet_id(self) -> i32 {
        match self {
            Self::Login => 0x04,
            Self::Configuration => 0x01,
            Self::Play => 0x11,
        }
    }
    pub const fn store_packet_id(self) -> Option<i32> {
        match self {
            Self::Login => None,
            Self::Configuration => Some(0x0a),
            Self::Play => Some(0x6b),
        }
    }
}

/// ResourceLocation.parse, including its default namespace and permitted empty
/// path. An explicit leading ':' also selects the default namespace. Encoding
/// applies the separate FriendlyByteBuf UTF length limit to its canonical form.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CookieKey(String);
impl CookieKey {
    pub fn parse(value: &str) -> Result<Self> {
        let (namespace, path) = match value.split_once(':') {
            Some(("", path)) => ("minecraft", path),
            Some(parts) => parts,
            None => ("minecraft", value),
        };
        ensure!(
            namespace
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"_.-".contains(&c)),
            "invalid cookie identifier namespace"
        );
        ensure!(
            path.bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"/._-".contains(&c)),
            "invalid cookie identifier path"
        );
        Ok(Self(format!("{namespace}:{path}")))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    fn read(reader: &mut Reader<'_>) -> Result<Self> {
        Self::parse(&reader.string(MAX_COOKIE_IDENTIFIER_CHARS)?)
    }
    fn write(&self, writer: &mut Writer) -> Result<()> {
        // Valid ResourceLocation characters are all ASCII, so byte length and
        // Java UTF-16 character length agree for this canonical identifier.
        ensure!(
            self.0.len() <= MAX_COOKIE_IDENTIFIER_CHARS,
            "cookie identifier exceeds wire string limit"
        );
        writer.string(&self.0);
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CookieRequest {
    pub key: CookieKey,
}
impl CookieRequest {
    pub fn decode(payload: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(payload);
        let key = CookieKey::read(&mut reader)?;
        reader.finish()?;
        Ok(Self { key })
    }
    pub fn packet(&self, phase: CookiePhase) -> Result<Packet> {
        let mut writer = Writer::new();
        self.key.write(&mut writer)?;
        Ok(Packet {
            id: phase.request_packet_id(),
            payload: writer.into_inner(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CookieResponse {
    pub key: CookieKey,
    /// None means no client cookie. Some(empty) is a present, empty cookie.
    pub payload: Option<Vec<u8>>,
}
impl CookieResponse {
    pub fn decode(payload: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(payload);
        let key = CookieKey::read(&mut reader)?;
        let payload = if reader.bool()? {
            Some(read_payload(&mut reader)?)
        } else {
            None
        };
        reader.finish()?;
        Ok(Self { key, payload })
    }
    pub fn packet(&self, phase: CookiePhase) -> Result<Packet> {
        let mut writer = Writer::new();
        self.key.write(&mut writer)?;
        writer.bool(self.payload.is_some());
        if let Some(payload) = &self.payload {
            write_payload(&mut writer, payload)?;
        }
        Ok(Packet {
            id: phase.response_packet_id(),
            payload: writer.into_inner(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CookieStore {
    pub key: CookieKey,
    pub payload: Vec<u8>,
}
impl CookieStore {
    pub fn decode(payload: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(payload);
        let key = CookieKey::read(&mut reader)?;
        let payload = read_payload(&mut reader)?;
        reader.finish()?;
        Ok(Self { key, payload })
    }
    pub fn packet(&self, phase: CookiePhase) -> Result<Packet> {
        let id = phase
            .store_packet_id()
            .context("store-cookie is unavailable in the login protocol")?;
        let mut writer = Writer::new();
        self.key.write(&mut writer)?;
        write_payload(&mut writer, &self.payload)?;
        Ok(Packet {
            id,
            payload: writer.into_inner(),
        })
    }
}

fn read_payload(reader: &mut Reader<'_>) -> Result<Vec<u8>> {
    let length = reader.varint()?;
    ensure!(
        (0..=MAX_COOKIE_PAYLOAD_SIZE as i32).contains(&length),
        "cookie payload exceeds 5120 bytes or has negative length"
    );
    Ok(reader.bytes(length as usize)?.to_vec())
}
fn write_payload(writer: &mut Writer, payload: &[u8]) -> Result<()> {
    ensure!(
        payload.len() <= MAX_COOKIE_PAYLOAD_SIZE,
        "cookie payload exceeds 5120 bytes"
    );
    writer.varint(payload.len() as i32).raw(payload);
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CookieAction {
    NotCookie,
    /// The host sends its current protocol's actual disconnect packet, using
    /// UNEXPECTED_COOKIE_RESPONSE_REASON, and stops this connection listener.
    DisconnectUnexpectedQuery,
}

/// One actual connection's phase and terminal cookie-listener disposition.
/// The host changes phase only at the acknowledged protocol switch. Queued
/// packets are interpreted in that actual phase, never guessed from payloads.
#[derive(Clone, Debug)]
pub struct CookieConnection {
    phase: CookiePhase,
    closed: bool,
}
impl CookieConnection {
    pub fn new(phase: CookiePhase) -> Self {
        Self {
            phase,
            closed: false,
        }
    }
    pub fn phase(&self) -> CookiePhase {
        self.phase
    }
    pub fn set_phase(&mut self, phase: CookiePhase) -> Result<()> {
        ensure!(!self.closed, "cookie connection listener is closed");
        ensure!(
            self.phase == phase
                || matches!(
                    (self.phase, phase),
                    (CookiePhase::Login, CookiePhase::Configuration)
                        | (CookiePhase::Configuration, CookiePhase::Play)
                        | (CookiePhase::Play, CookiePhase::Configuration)
                ),
            "invalid cookie connection protocol transition"
        );
        self.phase = phase;
        Ok(())
    }
    pub fn close(&mut self) {
        self.closed = true;
    }
    pub fn handle_packet(&mut self, packet: &Packet) -> Result<CookieAction> {
        if packet.id != self.phase.response_packet_id() {
            return Ok(CookieAction::NotCookie);
        }
        ensure!(!self.closed, "cookie connection listener is closed");
        // The network decoder still validates the entire response before the
        // vanilla listener unconditionally disconnects. No payload, request
        // ticket or cookie value is retained, including across reconfiguration.
        let _ = CookieResponse::decode(&packet.payload)?;
        self.closed = true;
        Ok(CookieAction::DisconnectUnexpectedQuery)
    }
    pub fn request(&self, key: CookieKey) -> Result<Packet> {
        ensure!(!self.closed, "cookie connection listener is closed");
        CookieRequest { key }.packet(self.phase)
    }
    pub fn store(&self, key: CookieKey, payload: Vec<u8>) -> Result<Packet> {
        ensure!(!self.closed, "cookie connection listener is closed");
        CookieStore { key, payload }.packet(self.phase)
    }
}
