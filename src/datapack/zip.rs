//! A minimal reader for the one thing the pack repository needs from a `.zip`: a root entry
//! (`pack.mcmeta`). It reads the central directory, then stored or deflated data.

use flate2::read::DeflateDecoder;
use std::io::Read;

fn u16_at(bytes: &[u8], at: usize) -> Option<usize> {
    Some(usize::from(u16::from_le_bytes(
        bytes.get(at..at + 2)?.try_into().ok()?,
    )))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<usize> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?) as usize)
}

/// The contents of the entry named exactly `name`, or `None` when the archive has no such entry
/// or cannot be read (`ZipFile` would throw, which the original logs and treats as "no pack").
pub fn read_entry(archive: &[u8], name: &str) -> Option<Vec<u8>> {
    // End of central directory record: signature 0x06054b50 within the last 64 KiB + 22 bytes.
    let start = archive.len().saturating_sub(65_557);
    let eocd = (start..archive.len().checked_sub(21)?)
        .rev()
        .find(|at| archive[*at..*at + 4] == [0x50, 0x4b, 0x05, 0x06])?;
    let entries = u16_at(archive, eocd + 10)?;
    let mut cursor = u32_at(archive, eocd + 16)?;
    for _ in 0..entries {
        if archive.get(cursor..cursor + 4)? != [0x50, 0x4b, 0x01, 0x02] {
            return None;
        }
        let method = u16_at(archive, cursor + 10)?;
        let compressed = u32_at(archive, cursor + 20)?;
        let name_len = u16_at(archive, cursor + 28)?;
        let extra_len = u16_at(archive, cursor + 30)?;
        let comment_len = u16_at(archive, cursor + 32)?;
        let local = u32_at(archive, cursor + 42)?;
        let entry_name = archive.get(cursor + 46..cursor + 46 + name_len)?;
        if entry_name == name.as_bytes() {
            if archive.get(local..local + 4)? != [0x50, 0x4b, 0x03, 0x04] {
                return None;
            }
            let data = local + 30 + u16_at(archive, local + 26)? + u16_at(archive, local + 28)?;
            let raw = archive.get(data..data + compressed)?;
            return match method {
                0 => Some(raw.to_vec()),
                8 => {
                    let mut out = Vec::new();
                    DeflateDecoder::new(raw).read_to_end(&mut out).ok()?;
                    Some(out)
                }
                _ => None,
            };
        }
        cursor += 46 + name_len + extra_len + comment_len;
    }
    None
}
