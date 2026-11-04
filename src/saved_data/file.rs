//! The `data/<name>.dat` envelope: gzip or plain NBT in, gzip NBT out (`NbtIo`).

use super::storage::SavedDataError;
use crate::nbt::{NbtRoot, Tag};
use flate2::{read::MultiGzDecoder, write::GzEncoder, Compression};
use std::io::{Read, Write};

/// `DimensionDataStorage.isGzip`: the first two bytes are `1f 8b`.
pub fn is_gzip(bytes: &[u8]) -> bool {
    bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b
}

/// `NbtIo.read` over the decoded stream: a named compound root. The name is skipped and bytes
/// after the root are ignored, as `DataInputStream` leaves them unread.
fn parse_named_compound(bytes: &[u8]) -> Result<Tag, SavedDataError> {
    let corrupt = |what: &str| SavedDataError::Corrupt(what.to_owned());
    let (&id, rest) = bytes
        .split_first()
        .ok_or_else(|| corrupt("empty NBT document"))?;
    if id != 10 {
        return Err(corrupt("Root tag must be a named compound tag"));
    }
    let length = rest
        .get(..2)
        .ok_or_else(|| corrupt("truncated root name"))?;
    let length = usize::from(u16::from_be_bytes([length[0], length[1]]));
    let payload = rest
        .get(2 + length..)
        .ok_or_else(|| corrupt("truncated root name"))?;
    let mut anonymous = Vec::with_capacity(payload.len() + 1);
    anonymous.push(10);
    anonymous.extend_from_slice(payload);
    let (tag, _) =
        Tag::decode_anonymous_prefix(&anonymous).map_err(|error| corrupt(&error.to_string()))?;
    Ok(tag)
}

/// Reads a saved-data file the way `DimensionDataStorage.readTagFromDisk` does: gzip when the
/// magic matches, plain NBT otherwise. Like the streaming Java reader, a gzip error after the
/// NBT root is complete (for example a damaged trailer) is not noticed.
pub fn read_root(bytes: &[u8]) -> Result<Tag, SavedDataError> {
    if !is_gzip(bytes) {
        return parse_named_compound(bytes);
    }
    let mut inflated = Vec::new();
    let outcome = MultiGzDecoder::new(bytes).read_to_end(&mut inflated);
    match parse_named_compound(&inflated) {
        Ok(tag) => Ok(tag),
        Err(parse) => match outcome {
            Err(io) => Err(SavedDataError::Corrupt(io.to_string())),
            Ok(_) => Err(parse),
        },
    }
}

/// `NbtIo.writeCompressed`: a gzip stream holding the root compound with an empty name.
pub fn write_root(root: &Tag) -> Result<Vec<u8>, SavedDataError> {
    let document = NbtRoot {
        name: "".into(),
        tag: root.clone(),
    }
    .encode()
    .map_err(|error| SavedDataError::Corrupt(error.to_string()))?;
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(&document)
        .map_err(|error| SavedDataError::Io(error.to_string()))?;
    encoder
        .finish()
        .map_err(|error| SavedDataError::Io(error.to_string()))
}

pub use crate::components::java::numeric_as_int;

/// `NbtUtils.getDataVersion(tag, default)`: `DataVersion` when it is numeric, else the default.
pub fn data_version(root: &Tag, default: i32) -> i32 {
    root.get("DataVersion")
        .and_then(numeric_as_int)
        .unwrap_or(default)
}

/// `CompoundTag.getCompound`: the member when it is a compound, otherwise an empty compound.
pub fn compound_member<'a>(root: &'a Tag, name: &str) -> std::borrow::Cow<'a, Tag> {
    match root.get(name) {
        Some(tag @ Tag::Compound(_)) => std::borrow::Cow::Borrowed(tag),
        _ => std::borrow::Cow::Owned(Tag::Compound(Vec::new())),
    }
}
