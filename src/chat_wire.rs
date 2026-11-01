//! Minecraft Java 1.21 (protocol 767) chat packet fields and builders.
//!
//! This module decodes envelopes, not trust or chat-chain acceptance. The real
//! listener owns validation and a separate signature cache belongs to each
//! receiving connection. Builders pack against that existing cache without
//! mutating it; the transport owner advances state for the actual delivery.

use anyhow::{ensure, Context, Result};
use serde_json::Value;

use crate::{
    admission_commands::encode_component_json,
    chat_ack::{LastSeenUpdate, MessageSignature, MessageSignatureCache, PackedMessageSignature},
    chat_chain::{FilterMask, PlayerChatMessage},
    chat_crypto::ProfilePublicKeyData,
    entity::ChatSession,
    protocol::{Packet, Reader, Writer},
};

pub const SERVERBOUND_CHAT_ACK: i32 = 0x03;
pub const SERVERBOUND_CHAT_MESSAGE: i32 = 0x06;
pub const SERVERBOUND_CHAT_SESSION_UPDATE: i32 = 0x07;
pub const CLIENTBOUND_DELETE_CHAT: i32 = 0x1c;
pub const CLIENTBOUND_DISGUISED_CHAT: i32 = 0x1e;
pub const CLIENTBOUND_PLAYER_CHAT: i32 = 0x39;
pub const CLIENTBOUND_SYSTEM_CHAT: i32 = 0x6c;
pub const CHAT_MESSAGE_MAX_UTF16: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IncomingChatMessage {
    pub message: String,
    pub timestamp_millis: i64,
    pub salt: i64,
    pub signature: Option<MessageSignature>,
    pub last_seen: LastSeenUpdate,
}

pub fn decode_chat_message(payload: &[u8]) -> Result<IncomingChatMessage> {
    let mut input = Reader::new(payload);
    let message = input.string(CHAT_MESSAGE_MAX_UTF16)?;
    let timestamp_millis = input.i64()?;
    let salt = input.i64()?;
    let signature = if input.bool()? {
        Some(read_signature(&mut input)?)
    } else {
        None
    };
    let last_seen = read_last_seen_update(&mut input)?;
    input.finish()?;
    Ok(IncomingChatMessage {
        message,
        timestamp_millis,
        salt,
        signature,
        last_seen,
    })
}

/// The offset remains signed. Negative/out-of-range acknowledgement behavior
/// belongs to LastSeenMessagesValidator, not a different wire-level policy.
pub fn decode_chat_ack(payload: &[u8]) -> Result<i32> {
    let mut input = Reader::new(payload);
    let offset = input.varint()?;
    input.finish()?;
    Ok(offset)
}

/// Decode and parse the RSA SPKI key as FriendlyByteBuf.readPublicKey does.
/// This is still an untrusted certificate: no service signature or expiry has
/// been accepted. The chat listener must validate it before publishing a session.
pub fn decode_chat_session_update(payload: &[u8]) -> Result<ChatSession> {
    let mut input = Reader::new(payload);
    let uuid = input.uuid()?;
    let expires_at = input.i64()?;
    let public_key = read_byte_array(&mut input, 512, "profile public key")?;
    let key_signature = read_byte_array(&mut input, 4096, "profile key signature")?;
    input.finish()?;
    let data = ProfilePublicKeyData::new(expires_at, public_key, key_signature)
        .context("malformed chat profile public key")?;
    Ok(ChatSession {
        uuid,
        expires_at: data.expires_at_ms,
        public_key: data.public_key_der,
        key_signature: data.key_signature,
    })
}

pub fn read_last_seen_update(input: &mut Reader<'_>) -> Result<LastSeenUpdate> {
    let offset = input.varint()?;
    let mut acknowledged = [0; 3];
    acknowledged.copy_from_slice(input.bytes(3)?);
    // FixedBitSet's 20-bit decoder reads all three bytes. Do not erase the four
    // spare bits: the actual validator must be able to reject them.
    Ok(LastSeenUpdate {
        offset,
        acknowledged,
    })
}

/// A registry-backed ChatType.Bound. `registry_id` is the actual zero-based
/// holder ID from this connection's configuration registry, not its wire value.
/// Inline/unregistered ChatType values require their own decoration codec and
/// are deliberately not manufactured by this bounded builder.
#[derive(Clone, Debug, PartialEq)]
pub struct BoundChatType {
    pub registry_id: i32,
    pub name: Value,
    pub target_name: Option<Value>,
}

/// Pack using the recipient's old cache. A successful return is not evidence
/// of network delivery and does not add a pending acknowledgement or cache item.
pub fn encode_player_chat(
    message: &PlayerChatMessage,
    chat_type: &BoundChatType,
    cache: &MessageSignatureCache,
) -> Result<Packet> {
    check_chat_string(&message.body.content)?;
    ensure!(
        message.body.last_seen.len() <= 20,
        "chat body last-seen list exceeds20 entries"
    );
    let mut output = Writer::new();
    output
        .uuid(message.link.sender)
        .varint(message.link.index)
        .bool(message.signature.is_some());
    if let Some(signature) = &message.signature {
        output.raw(signature);
    }
    output
        .string(&message.body.content)
        .i64(message.body.timestamp_millis)
        .i64(message.body.salt);
    let packed = cache.pack_last_seen(&message.body.last_seen);
    output.varint(packed.len() as i32);
    for signature in &packed {
        write_packed_signature(&mut output, signature)?;
    }
    output.bool(message.unsigned_content.is_some());
    if let Some(content) = &message.unsigned_content {
        output.raw(&encode_component_json(content)?);
    }
    write_filter_mask(&mut output, &message.filter_mask)?;
    write_chat_type(&mut output, chat_type)?;
    Ok(Packet {
        id: CLIENTBOUND_PLAYER_CHAT,
        payload: output.into_inner(),
    })
}

pub fn encode_disguised_chat(message: &Value, chat_type: &BoundChatType) -> Result<Packet> {
    let mut output = Writer::new();
    output.raw(&encode_component_json(message)?);
    write_chat_type(&mut output, chat_type)?;
    Ok(Packet {
        id: CLIENTBOUND_DISGUISED_CHAT,
        payload: output.into_inner(),
    })
}

pub fn encode_system_chat(message: &Value, action_bar: bool) -> Result<Packet> {
    let mut output = Writer::new();
    output
        .raw(&encode_component_json(message)?)
        .bool(action_bar);
    Ok(Packet {
        id: CLIENTBOUND_SYSTEM_CHAT,
        payload: output.into_inner(),
    })
}

pub fn encode_delete_chat(
    signature: &MessageSignature,
    cache: &MessageSignatureCache,
) -> Result<Packet> {
    let mut output = Writer::new();
    write_packed_signature(&mut output, &cache.pack(signature))?;
    Ok(Packet {
        id: CLIENTBOUND_DELETE_CHAT,
        payload: output.into_inner(),
    })
}

pub fn read_packed_signature(input: &mut Reader<'_>) -> Result<PackedMessageSignature> {
    let id = input.varint()?.wrapping_sub(1);
    if id == -1 {
        Ok(PackedMessageSignature::Full(read_signature(input)?))
    } else {
        Ok(PackedMessageSignature::Cached(id))
    }
}

pub fn write_packed_signature(
    output: &mut Writer,
    signature: &PackedMessageSignature,
) -> Result<()> {
    match signature {
        PackedMessageSignature::Full(signature) => {
            output.varint(0).raw(signature);
        }
        PackedMessageSignature::Cached(id) => {
            ensure!(*id >= 0, "negative packed chat signature cache index");
            output.varint(
                id.checked_add(1)
                    .context("packed chat signature index overflow")?,
            );
        }
    }
    Ok(())
}

fn read_signature(input: &mut Reader<'_>) -> Result<MessageSignature> {
    let mut signature = [0; 256];
    signature.copy_from_slice(input.bytes(256)?);
    Ok(signature)
}

fn read_byte_array(input: &mut Reader<'_>, limit: usize, field: &str) -> Result<Vec<u8>> {
    let length = input.varint()?;
    ensure!(
        length >= 0 && length as usize <= limit,
        "invalid {field} length"
    );
    Ok(input.bytes(length as usize)?.to_vec())
}

fn check_chat_string(content: &str) -> Result<()> {
    ensure!(
        content.encode_utf16().count() <= CHAT_MESSAGE_MAX_UTF16,
        "chat message exceeds Java256-character bound"
    );
    ensure!(
        content.len() <= CHAT_MESSAGE_MAX_UTF16 * 3,
        "chat message exceeds encoded UTF bound"
    );
    Ok(())
}

fn write_chat_type(output: &mut Writer, chat_type: &BoundChatType) -> Result<()> {
    ensure!(
        chat_type.registry_id >= 0,
        "chat type must refer to an actual configured registry holder"
    );
    // ByteBufCodecs.holder(registry, directCodec) reserves0 for inline data.
    output
        .varint(
            chat_type
                .registry_id
                .checked_add(1)
                .context("chat type registry index overflow")?,
        )
        .raw(&encode_component_json(&chat_type.name)?)
        .bool(chat_type.target_name.is_some());
    if let Some(target) = &chat_type.target_name {
        output.raw(&encode_component_json(target)?);
    }
    Ok(())
}

fn write_filter_mask(output: &mut Writer, filter: &FilterMask) -> Result<()> {
    match filter {
        FilterMask::PassThrough => {
            output.varint(0);
        }
        FilterMask::FullyFiltered => {
            output.varint(1);
        }
        FilterMask::PartiallyFiltered(words) => {
            let length = words
                .iter()
                .rposition(|word| *word != 0)
                .map_or(0, |i| i + 1);
            output
                .varint(2)
                .varint(i32::try_from(length).context("chat filter bitset exceeds wire length")?);
            for word in &words[..length] {
                output.i64(*word as i64);
            }
        }
    }
    Ok(())
}
