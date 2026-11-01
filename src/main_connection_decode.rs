//! Pure supported wire decoding before accepted listener dispatch is counted.
//!
//! This does not execute the decoded actions or add gameplay support for
//! currently ignored messages. Beacon holders use the pinned actual built-in
//! vanilla mob-effect registry export, not invented IDs or a gameplay owner.

use anyhow::{bail, ensure, Result};
use serde::Deserialize;
use std::{collections::BTreeMap, sync::OnceLock};
use mc121_rust::{
    chat_wire, command_protocol,
    connection_cookies::{CookieKey, CookieResponse},
    connection_payload,
    container::ContainerClick,
    inventory::ItemStack,
    protocol::{Packet, Reader},
    reconfiguration,
    resource_packs::ResourcePackResponse,
    server_recipe_book::{RecipeBookSeenRecipe, RecipeBookSettingsChange},
};

fn ordinal(fields: &mut Reader<'_>, variants: i32) -> Result<i32> {
    let value = fields.varint()?;
    ensure!((0..variants).contains(&value), "invalid packet enum ordinal");
    Ok(value)
}

fn resource_location(fields: &mut Reader<'_>) -> Result<()> {
    CookieKey::parse(&fields.string(32_767)?)?;
    Ok(())
}

fn client_information(payload: &[u8]) -> Result<()> {
    let mut fields = Reader::new(payload);
    fields.string(16)?;
    fields.i8()?;
    ordinal(&mut fields, 3)?;
    fields.bool()?;
    fields.u8()?;
    ordinal(&mut fields, 2)?;
    fields.bool()?;
    fields.bool()?;
    fields.finish()
}

fn floats(fields: &mut Reader<'_>, count: usize) -> Result<()> {
    for _ in 0..count { fields.f32()?; }
    Ok(())
}

fn doubles(fields: &mut Reader<'_>, count: usize) -> Result<()> {
    for _ in 0..count { fields.f64()?; }
    Ok(())
}

fn varlong(fields: &mut Reader<'_>) -> Result<()> {
    for _ in 0..10 {
        if fields.u8()? & 0x80 == 0 { return Ok(()); }
    }
    bail!("VarLong is too big")
}

fn mob_effects() -> &'static BTreeMap<i32, String> {
    static EFFECTS: OnceLock<BTreeMap<i32, String>> = OnceLock::new();
    EFFECTS.get_or_init(|| {
        #[derive(Deserialize)]
        struct Entry { id: i32, name: String }
        #[derive(Deserialize)]
        struct Export { entries: Vec<Entry> }
        let export: Export = serde_json::from_str(include_str!(
            "../fixtures/connection-mob-effects-1.21.json"
        )).expect("pinned original vanilla mob-effect registry export");
        let mut effects = BTreeMap::new();
        for entry in export.entries {
            assert!(entry.id >= 0 && entry.name.starts_with("minecraft:"));
            assert!(effects.insert(entry.id, entry.name).is_none());
        }
        effects
    })
}

fn optional_mob_effect(fields: &mut Reader<'_>) -> Result<()> {
    if fields.bool()? {
        let id = fields.varint()?;
        ensure!(mob_effects().contains_key(&id), "unknown vanilla mob-effect registry ID {id}");
    }
    Ok(())
}

/// `ServerboundKeyPacket` decodes two ordinary byte arrays, without imposing
/// RSA ciphertext length or checking padding/challenge policy in the codec.
pub(super) fn validate_login_key(payload: &[u8]) -> Result<()> {
    let mut fields = Reader::new(payload);
    for _ in 0..2 {
        let length = fields.varint()?;
        ensure!(length >= 0, "negative login key byte-array length");
        fields.bytes(length as usize)?;
    }
    fields.finish()
}

pub(super) fn validate_configuration(packet: &Packet) -> Result<()> {
    match packet.id {
        0x00 => client_information(&packet.payload),
        0x01 => CookieResponse::decode(&packet.payload).map(|_| ()),
        0x02 => connection_payload::decode_custom_payload(&packet.payload).map(|_| ()),
        0x03 => {
            ensure!(packet.payload.is_empty(), "configuration finish has trailing bytes");
            Ok(())
        }
        0x04 => {
            let mut fields = Reader::new(&packet.payload);
            fields.i64()?;
            fields.finish()
        }
        0x05 => connection_payload::decode_pong(&packet.payload).map(|_| ()),
        0x06 => ResourcePackResponse::decode(&packet.payload).map(|_| ()),
        0x07 => reconfiguration::read_selected_known_packs(&packet.payload).map(|_| ()),
        _ => bail!("unknown CONFIGURATION packet {}", packet.id),
    }
}

pub(super) fn validate_play(packet: &Packet) -> Result<()> {
    let mut fields = Reader::new(&packet.payload);
    match packet.id {
        0x00 | 0x20 | 0x2d => { fields.varint()?; }
        0x01 => { fields.varint()?; fields.i64()?; }
        // Difficulty.byId maps every unsigned byte; this is not readEnum.
        0x02 => { fields.u8()?; }
        0x03 => return chat_wire::decode_chat_ack(&packet.payload).map(|_| ()),
        0x04 => return command_protocol::decode_unsigned_command(&packet.payload).map(|_| ()),
        0x05 => return command_protocol::decode_signed_command(&packet.payload).map(|_| ()),
        0x06 => return chat_wire::decode_chat_message(&packet.payload).map(|_| ()),
        0x07 => return chat_wire::decode_chat_session_update(&packet.payload).map(|_| ()),
        0x08 => { fields.f32()?; }
        0x09 => { ordinal(&mut fields, 2)?; }
        0x0a => return client_information(&packet.payload),
        0x0b => return command_protocol::decode_suggestion_request(&packet.payload).map(|_| ()),
        0x0c => {}
        // This 1.21 record uses two VAR_INT codecs, not old unsigned bytes.
        0x0d => { fields.varint()?; fields.varint()?; }
        0x0e => return ContainerClick::decode(&packet.payload).map(|_| ()),
        0x0f => { fields.u8()?; }
        0x10 => { fields.varint()?; fields.varint()?; fields.bool()?; }
        0x11 => return CookieResponse::decode(&packet.payload).map(|_| ()),
        0x12 => return connection_payload::decode_custom_payload(&packet.payload).map(|_| ()),
        0x13 => { ordinal(&mut fields, 1)?; }
        0x14 => {
            fields.varint()?;
            let pages = fields.varint()?;
            ensure!((0..=200).contains(&pages), "book page count exceeds Java limit");
            for _ in 0..pages { fields.string(8_192)?; }
            if fields.bool()? { fields.string(128)?; }
        }
        0x15 => { fields.varint()?; fields.varint()?; }
        0x16 => {
            fields.varint()?;
            match ordinal(&mut fields, 3)? {
                0 => { ordinal(&mut fields, 2)?; }
                1 => {}
                2 => { floats(&mut fields, 3)?; ordinal(&mut fields, 2)?; }
                _ => unreachable!(),
            }
            fields.bool()?;
        }
        0x17 => { fields.i64()?; fields.varint()?; fields.bool()?; }
        0x18 | 0x21 => { fields.i64()?; }
        0x19 => { fields.bool()?; }
        0x1a => { doubles(&mut fields, 3)?; fields.bool()?; }
        0x1b => { doubles(&mut fields, 3)?; floats(&mut fields, 2)?; fields.bool()?; }
        0x1c => { floats(&mut fields, 2)?; fields.bool()?; }
        0x1d => { fields.bool()?; }
        0x1e => { doubles(&mut fields, 3)?; floats(&mut fields, 2)?; }
        0x1f => { fields.bool()?; fields.bool()?; }
        0x22 => { fields.i8()?; resource_location(&mut fields)?; fields.bool()?; }
        0x23 => { fields.u8()?; }
        0x24 => {
            ordinal(&mut fields, 7)?;
            fields.i64()?;
            // Direction.from3DDataValue wraps this unsigned byte. Sequence
            // negativity is a handler policy, not a packet decoding failure.
            fields.u8()?;
            fields.varint()?;
        }
        0x25 => { fields.varint()?; ordinal(&mut fields, 9)?; fields.varint()?; }
        0x26 => { floats(&mut fields, 2)?; fields.u8()?; }
        0x27 => return connection_payload::decode_pong(&packet.payload).map(|_| ()),
        0x28 => return RecipeBookSettingsChange::decode_exact(&packet.payload).map(|_| ()),
        0x29 => return RecipeBookSeenRecipe::decode_exact(&packet.payload).map(|_| ()),
        0x2a => { fields.string(32_767)?; }
        0x2b => return ResourcePackResponse::decode(&packet.payload).map(|_| ()),
        0x2c => {
            if ordinal(&mut fields, 2)? == 0 { resource_location(&mut fields)?; }
        }
        0x2e => {
            optional_mob_effect(&mut fields)?;
            optional_mob_effect(&mut fields)?;
        }
        0x2f => { fields.i16()?; }
        0x30 => {
            fields.i64()?;
            fields.string(32_767)?;
            ordinal(&mut fields, 3)?;
            fields.u8()?;
        }
        0x31 => { fields.varint()?; fields.string(32_767)?; fields.bool()?; }
        0x32 => {
            fields.i16()?;
            return ItemStack::decode_exact(fields.remaining()).map(|_| ());
        }
        0x33 => {
            fields.i64()?;
            for _ in 0..3 { resource_location(&mut fields)?; }
            fields.string(32_767)?;
            // Unknown joint names map to ALIGNED in the packet constructor.
            fields.string(32_767)?;
            fields.varint()?;
            fields.varint()?;
        }
        0x34 => {
            fields.i64()?;
            ordinal(&mut fields, 4)?;
            ordinal(&mut fields, 4)?;
            fields.string(32_767)?;
            // Offset/size bytes and integrity float are clamped by decoding;
            // all representable values consume the same wire fields.
            fields.bytes(6)?;
            ordinal(&mut fields, 3)?;
            ordinal(&mut fields, 4)?;
            fields.string(128)?;
            fields.f32()?;
            varlong(&mut fields)?;
            fields.u8()?;
        }
        0x35 => {
            fields.i64()?;
            fields.bool()?;
            for _ in 0..4 { fields.string(384)?; }
        }
        0x36 => { ordinal(&mut fields, 2)?; }
        0x37 => { fields.uuid()?; }
        0x38 => {
            ordinal(&mut fields, 2)?;
            fields.i64()?;
            ordinal(&mut fields, 6)?;
            floats(&mut fields, 3)?;
            fields.bool()?;
            fields.varint()?;
        }
        0x39 => {
            ordinal(&mut fields, 2)?;
            fields.varint()?;
            floats(&mut fields, 2)?;
        }
        _ => bail!("unknown PLAY packet {}", packet.id),
    }
    fields.finish()
}
