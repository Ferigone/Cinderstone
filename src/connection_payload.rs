//! Java 1.21 serverbound common payload decoders.
//!
//! Both CONFIGURATION and PLAY register the same custom-payload and pong
//! codecs. Vanilla's common/game listeners ignore their decoded contents;
//! ignoring a handler does not bypass the wire codec's validation. Unknown
//! custom payloads are discarded, rather than retained as plugin messages.

use crate::{
    connection_cookies::CookieKey,
    protocol::Reader,
};
use anyhow::{ensure, Result};

/// ServerboundCustomPayloadPacket's fallback bound applies to the remaining
/// unknown payload body, after decoding its ResourceLocation identifier.
pub const MAX_SERVERBOUND_CUSTOM_PAYLOAD_BYTES: usize = 32_767;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CustomPayload {
    Brand(String),
    /// The original DiscardedPayload contains only its identifier. Its body
    /// has already been consumed by the packet decoder and is not retained.
    Discarded(CookieKey),
}

/// Decode the actual registered BrandPayload, or validate/discard an unknown
/// channel. A brand is limited by its UTF codec, not the fallback byte bound.
pub fn decode_custom_payload(payload: &[u8]) -> Result<CustomPayload> {
    let mut fields = Reader::new(payload);
    // CookieKey reuses vanilla's ResourceLocation.parse rules, including the
    // default namespace, while Reader supplies FriendlyByteBuf's UTF bound.
    let channel = CookieKey::parse(&fields.string(32_767)?)?;
    if channel.as_str() == "minecraft:brand" {
        let brand = fields.string(32_767)?;
        fields.finish()?;
        Ok(CustomPayload::Brand(brand))
    } else {
        ensure!(
            fields.remaining().len() <= MAX_SERVERBOUND_CUSTOM_PAYLOAD_BYTES,
            "custom payload exceeds Java body limit"
        );
        Ok(CustomPayload::Discarded(channel))
    }
}

/// ServerboundPongPacket uses one fixed-width signed integer. The common
/// listener has no pong handler state; a validated value can be discarded.
pub fn decode_pong(payload: &[u8]) -> Result<i32> {
    let mut fields = Reader::new(payload);
    let id = fields.i32()?;
    fields.finish()?;
    Ok(id)
}
