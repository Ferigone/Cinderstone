//! Minecraft Java Anvil region storage, including the Java LZ4Block stream.
//!
//! The region file is committed by fsync + atomic rename. External `.mcc`
//! payloads are separately committed before the region header, as individual
//! complete chunk files; this is not a transaction across several filesystem
//! names. One writer must own a region. Unknown NBT fields and unchanged raw
//! sector allocations are retained. A completely unchanged region reproduces
//! its original bytes, including unused sectors and padding.

use crate::nbt::{Limits as NbtLimits, NbtRoot};
use anyhow::{anyhow, bail, ensure, Context, Result};
use flate2::{bufread::GzDecoder, write::GzEncoder, write::ZlibEncoder};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

pub const SECTOR_BYTES: usize = 4096;
pub const HEADER_BYTES: usize = 8192;
pub const CHUNKS_PER_REGION: usize = 1024;
const LZ4_MAGIC: &[u8; 8] = b"LZ4Block";
const XXHASH_SEED: u32 = 0x9747_b28c;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ChunkCompression {
    Gzip = 1,
    Zlib = 2,
    Uncompressed = 3,
    Lz4 = 4,
}

impl TryFrom<u8> for ChunkCompression {
    type Error = anyhow::Error;
    fn try_from(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Gzip),
            2 => Ok(Self::Zlib),
            3 => Ok(Self::Uncompressed),
            4 => Ok(Self::Lz4),
            _ => bail!("unsupported Anvil compression ID {value}"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RegionLimits {
    pub max_region_bytes: usize,
    pub max_chunk_bytes: usize,
    pub max_external_bytes: usize,
    pub nbt: NbtLimits,
}

impl Default for RegionLimits {
    fn default() -> Self {
        Self {
            max_region_bytes: 256 * 1024 * 1024,
            max_chunk_bytes: 64 * 1024 * 1024,
            max_external_bytes: 256 * 1024 * 1024,
            nbt: NbtLimits::default(),
        }
    }
}

#[derive(Debug, Clone)]
struct StoredChunk {
    sectors: Vec<u8>,
    compression: ChunkCompression,
    external: bool,
    external_data: Option<Vec<u8>>,
}

impl StoredChunk {
    fn payload(&self) -> Result<&[u8]> {
        if self.external {
            return self.external_data.as_deref().ok_or_else(|| {
                anyhow!("external .mcc data requires RegionFile::read from a filesystem path")
            });
        }
        let length = u32::from_be_bytes(self.sectors[..4].try_into().unwrap()) as usize;
        Ok(&self.sectors[5..4 + length])
    }
}

#[derive(Debug, Clone)]
pub struct RegionFile {
    pub region_x: i32,
    pub region_z: i32,
    chunks: Vec<Option<StoredChunk>>,
    timestamps: [u32; CHUNKS_PER_REGION],
    original: Option<Vec<u8>>,
    dirty: bool,
    limits: RegionLimits,
    external_deletions: BTreeSet<(i32, i32)>,
}

/// Euclidean division is essential for chunks west/north of the origin.
pub fn region_coordinates(chunk_x: i32, chunk_z: i32) -> (i32, i32) {
    (chunk_x.div_euclid(32), chunk_z.div_euclid(32))
}

pub fn location_index(chunk_x: i32, chunk_z: i32) -> usize {
    chunk_x.rem_euclid(32) as usize + 32 * chunk_z.rem_euclid(32) as usize
}

pub fn coordinates_from_path(path: &Path) -> Result<(i32, i32)> {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .context("invalid region filename")?;
    let parts: Vec<_> = name.split('.').collect();
    ensure!(
        parts.len() == 4 && parts[0] == "r" && parts[3] == "mca",
        "expected r.<x>.<z>.mca"
    );
    Ok((
        parts[1].parse().context("invalid region X")?,
        parts[2].parse().context("invalid region Z")?,
    ))
}

impl RegionFile {
    pub fn empty(region_x: i32, region_z: i32) -> Result<Self> {
        Self::empty_with_limits(region_x, region_z, RegionLimits::default())
    }

    pub fn empty_with_limits(region_x: i32, region_z: i32, limits: RegionLimits) -> Result<Self> {
        for coordinate in [region_x, region_z] {
            coordinate
                .checked_mul(32)
                .and_then(|v| v.checked_add(31))
                .context("region coordinates overflow chunk coordinates")?;
        }
        ensure!(
            limits.max_region_bytes >= HEADER_BYTES,
            "region byte limit is smaller than its header"
        );
        Ok(Self {
            region_x,
            region_z,
            chunks: vec![None; CHUNKS_PER_REGION],
            timestamps: [0; CHUNKS_PER_REGION],
            original: None,
            dirty: true,
            limits,
            external_deletions: BTreeSet::new(),
        })
    }

    pub fn from_bytes(bytes: &[u8], region_x: i32, region_z: i32) -> Result<Self> {
        Self::from_bytes_with_limits(bytes, region_x, region_z, RegionLimits::default())
    }

    pub fn from_bytes_with_limits(
        bytes: &[u8],
        region_x: i32,
        region_z: i32,
        limits: RegionLimits,
    ) -> Result<Self> {
        ensure!(bytes.len() >= HEADER_BYTES, "truncated Anvil region header");
        ensure!(
            bytes.len() <= limits.max_region_bytes,
            "region exceeds configured byte limit"
        );
        let mut region = Self::empty_with_limits(region_x, region_z, limits)?;
        let mut used = vec![false; bytes.len().div_ceil(SECTOR_BYTES)];
        used[0] = true;
        used[1] = true;
        for index in 0..CHUNKS_PER_REGION {
            region.timestamps[index] = u32::from_be_bytes(
                bytes[4096 + index * 4..4100 + index * 4]
                    .try_into()
                    .unwrap(),
            );
            let offset = u32::from_be_bytes(bytes[index * 4..index * 4 + 4].try_into().unwrap());
            if offset == 0 {
                continue;
            }
            let sector = (offset >> 8) as usize;
            let count = (offset & 255) as usize;
            ensure!(
                sector >= 2 && count > 0,
                "chunk {index} has an invalid sector allocation"
            );
            let end_sector = sector
                .checked_add(count)
                .context("region sector arithmetic overflow")?;
            let end = end_sector
                .checked_mul(SECTOR_BYTES)
                .context("region byte arithmetic overflow")?;
            ensure!(
                end <= bytes.len(),
                "chunk {index} allocation extends beyond the region file"
            );
            for allocated in &mut used[sector..end_sector] {
                ensure!(!*allocated, "overlapping chunk sector allocation");
                *allocated = true;
            }
            let start = sector * SECTOR_BYTES;
            let length = u32::from_be_bytes(bytes[start..start + 4].try_into().unwrap()) as usize;
            ensure!(
                length >= 1 && length <= count * SECTOR_BYTES - 4,
                "chunk {index} has an invalid record length"
            );
            let marker = bytes[start + 4];
            let external = marker & 128 != 0;
            if external {
                ensure!(
                    length == 1,
                    "external chunk stub must contain only its compression marker"
                );
            }
            let compression = ChunkCompression::try_from(marker & 127)?;
            region.chunks[index] = Some(StoredChunk {
                sectors: bytes[start..end].to_vec(),
                compression,
                external,
                external_data: None,
            });
        }
        region.original = Some(bytes.to_vec());
        region.dirty = false;
        Ok(region)
    }

    pub fn read(path: impl AsRef<Path>) -> Result<Self> {
        Self::read_with_limits(path, RegionLimits::default())
    }

    pub fn read_with_limits(path: impl AsRef<Path>, limits: RegionLimits) -> Result<Self> {
        let path = path.as_ref();
        let coordinates = coordinates_from_path(path)?;
        let bytes = read_bounded(path, limits.max_region_bytes)?;
        let mut region =
            Self::from_bytes_with_limits(&bytes, coordinates.0, coordinates.1, limits)?;
        let directory = path.parent().unwrap_or_else(|| Path::new("."));
        let mut external_total = 0_usize;
        for index in 0..CHUNKS_PER_REGION {
            let (x, z) = region.chunk_coordinates(index);
            if let Some(chunk) = region.chunks[index].as_mut().filter(|c| c.external) {
                let external = directory.join(format!("c.{x}.{z}.mcc"));
                let data = read_bounded(&external, limits.max_chunk_bytes)
                    .with_context(|| format!("reading external chunk {}", external.display()))?;
                external_total = external_total
                    .checked_add(data.len())
                    .context("external byte arithmetic overflow")?;
                ensure!(
                    external_total <= limits.max_external_bytes,
                    "external chunk files exceed aggregate byte limit"
                );
                chunk.external_data = Some(data);
            }
        }
        Ok(region)
    }

    fn index(&self, chunk_x: i32, chunk_z: i32) -> Result<usize> {
        ensure!(
            region_coordinates(chunk_x, chunk_z) == (self.region_x, self.region_z),
            "chunk belongs to a different region"
        );
        Ok(location_index(chunk_x, chunk_z))
    }

    fn chunk_coordinates(&self, index: usize) -> (i32, i32) {
        (
            self.region_x * 32 + (index % 32) as i32,
            self.region_z * 32 + (index / 32) as i32,
        )
    }

    pub fn occupied_chunks(&self) -> Vec<(i32, i32)> {
        self.chunks
            .iter()
            .enumerate()
            .filter(|(_, c)| c.is_some())
            .map(|(index, _)| self.chunk_coordinates(index))
            .collect()
    }

    pub fn timestamp(&self, chunk_x: i32, chunk_z: i32) -> Result<u32> {
        Ok(self.timestamps[self.index(chunk_x, chunk_z)?])
    }

    pub fn compression(&self, chunk_x: i32, chunk_z: i32) -> Result<Option<ChunkCompression>> {
        Ok(self.chunks[self.index(chunk_x, chunk_z)?]
            .as_ref()
            .map(|c| c.compression))
    }

    pub fn is_external(&self, chunk_x: i32, chunk_z: i32) -> Result<bool> {
        Ok(self.chunks[self.index(chunk_x, chunk_z)?]
            .as_ref()
            .is_some_and(|c| c.external))
    }

    pub fn raw_sectors(&self, chunk_x: i32, chunk_z: i32) -> Result<Option<&[u8]>> {
        Ok(self.chunks[self.index(chunk_x, chunk_z)?]
            .as_ref()
            .map(|c| c.sectors.as_slice()))
    }

    pub fn chunk_bytes(&self, chunk_x: i32, chunk_z: i32) -> Result<Option<Vec<u8>>> {
        let Some(chunk) = self.chunks[self.index(chunk_x, chunk_z)?].as_ref() else {
            return Ok(None);
        };
        Ok(Some(decompress(
            chunk.compression,
            chunk.payload()?,
            self.limits.max_chunk_bytes,
        )?))
    }

    pub fn chunk(&self, chunk_x: i32, chunk_z: i32) -> Result<Option<NbtRoot>> {
        self.chunk_bytes(chunk_x, chunk_z)?
            .map(|data| NbtRoot::decode_with_limits(&data, self.limits.nbt))
            .transpose()
    }

    pub fn insert_chunk(
        &mut self,
        chunk_x: i32,
        chunk_z: i32,
        root: &NbtRoot,
        compression: ChunkCompression,
        timestamp: u32,
    ) -> Result<()> {
        let data = root.encode_with_limits(self.limits.nbt)?;
        self.insert_chunk_bytes(chunk_x, chunk_z, &data, compression, timestamp)
    }

    /// Insert verified named-root NBT. No unknown fields are normalized away.
    pub fn insert_chunk_bytes(
        &mut self,
        chunk_x: i32,
        chunk_z: i32,
        data: &[u8],
        compression: ChunkCompression,
        timestamp: u32,
    ) -> Result<()> {
        let index = self.index(chunk_x, chunk_z)?;
        ensure!(
            data.len() <= self.limits.max_chunk_bytes,
            "chunk exceeds configured byte limit"
        );
        NbtRoot::decode_with_limits(data, self.limits.nbt)?;
        let payload = compress(compression, data)?;
        ensure!(
            payload.len() <= self.limits.max_chunk_bytes,
            "compressed chunk exceeds configured byte limit"
        );
        let sectors = (payload.len() + 5).div_ceil(SECTOR_BYTES);
        let external = sectors > 255;
        let mut record = if external {
            vec![0; SECTOR_BYTES]
        } else {
            vec![0; sectors * SECTOR_BYTES]
        };
        let length = if external { 1 } else { payload.len() + 1 };
        record[..4].copy_from_slice(&(length as u32).to_be_bytes());
        record[4] = compression as u8 | if external { 128 } else { 0 };
        if !external {
            record[5..5 + payload.len()].copy_from_slice(&payload);
        }
        self.chunks[index] = Some(StoredChunk {
            sectors: record,
            compression,
            external,
            external_data: external.then_some(payload),
        });
        self.timestamps[index] = timestamp;
        if external {
            self.external_deletions.remove(&(chunk_x, chunk_z));
        } else {
            self.external_deletions.insert((chunk_x, chunk_z));
        }
        self.dirty = true;
        Ok(())
    }

    pub fn remove_chunk(&mut self, chunk_x: i32, chunk_z: i32) -> Result<bool> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as u32;
        self.remove_chunk_at(chunk_x, chunk_z, timestamp)
    }

    pub fn remove_chunk_at(&mut self, chunk_x: i32, chunk_z: i32, timestamp: u32) -> Result<bool> {
        let index = self.index(chunk_x, chunk_z)?;
        let removed = self.chunks[index].take().is_some();
        if removed {
            self.timestamps[index] = timestamp;
            self.external_deletions.insert((chunk_x, chunk_z));
            self.dirty = true;
        }
        Ok(removed)
    }

    /// Region bytes alone do not carry external `.mcc` payloads. Use
    /// `save_atomic` when materializing an external-chunk region on disk.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        if !self.dirty {
            if let Some(original) = &self.original {
                return Ok(original.clone());
            }
        }
        let mut out = vec![0; HEADER_BYTES];
        for index in 0..CHUNKS_PER_REGION {
            out[4096 + index * 4..4100 + index * 4]
                .copy_from_slice(&self.timestamps[index].to_be_bytes());
            if let Some(chunk) = &self.chunks[index] {
                let sector = out.len() / SECTOR_BYTES;
                let count = chunk.sectors.len() / SECTOR_BYTES;
                ensure!(
                    sector <= 0x00ff_ffff && (1..=255).contains(&count),
                    "chunk allocation cannot fit the region header"
                );
                let new_len = out
                    .len()
                    .checked_add(chunk.sectors.len())
                    .context("region size overflow")?;
                ensure!(
                    new_len <= self.limits.max_region_bytes,
                    "encoded region exceeds configured byte limit"
                );
                let entry = ((sector as u32) << 8) | count as u32;
                out[index * 4..index * 4 + 4].copy_from_slice(&entry.to_be_bytes());
                out.extend_from_slice(&chunk.sectors);
            }
        }
        Ok(out)
    }

    pub fn save_atomic(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let bytes = self.to_bytes()?;
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let mut external = Vec::new();
        let mut total = 0_usize;
        for (index, chunk) in self.chunks.iter().enumerate() {
            if let Some(chunk) = chunk.as_ref().filter(|c| c.external) {
                let data = chunk
                    .external_data
                    .as_deref()
                    .context("cannot save external stub without its .mcc payload")?;
                total = total
                    .checked_add(data.len())
                    .context("external byte arithmetic overflow")?;
                ensure!(
                    total <= self.limits.max_external_bytes,
                    "external chunk payloads exceed aggregate byte limit"
                );
                let (x, z) = self.chunk_coordinates(index);
                external.push((parent.join(format!("c.{x}.{z}.mcc")), data));
            }
        }
        fs::create_dir_all(parent)?;
        // Each external chunk is replaced atomically; every old or new region
        // stub points to a complete stream, never a partially-written file.
        for (name, data) in external {
            if name.is_file()
                && fs::metadata(&name)?.len() == data.len() as u64
                && read_bounded(&name, self.limits.max_chunk_bytes)? == data
            {
                continue;
            }
            write_atomic(&name, data)?;
        }
        write_atomic(path, &bytes)?;
        for &(x, z) in &self.external_deletions {
            match fs::remove_file(parent.join(format!("c.{x}.{z}.mcc"))) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        #[cfg(unix)]
        File::open(parent)?.sync_all()?;
        self.external_deletions.clear();
        self.original = Some(bytes);
        self.dirty = false;
        Ok(())
    }
}

fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let file = File::open(path)?;
    let length = file.metadata()?.len();
    ensure!(
        length <= limit as u64,
        "{} exceeds configured byte limit",
        path.display()
    );
    let mut bytes = Vec::with_capacity(length as usize);
    file.take(limit.saturating_add(1) as u64)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= limit,
        "{} grew beyond configured byte limit",
        path.display()
    );
    Ok(bytes)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .context("invalid output filename")?;
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temporary = parent.join(format!(".{name}.{}.{}.tmp", std::process::id(), sequence));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        if let Ok(existing) = fs::metadata(path) {
            file.set_permissions(existing.permissions())?;
        }
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)?;
        #[cfg(unix)]
        File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

pub fn compress(kind: ChunkCompression, bytes: &[u8]) -> Result<Vec<u8>> {
    match kind {
        ChunkCompression::Uncompressed => Ok(bytes.to_vec()),
        ChunkCompression::Gzip => {
            let mut encoder = GzEncoder::new(Vec::new(), flate2::Compression::default());
            encoder.write_all(bytes)?;
            Ok(encoder.finish()?)
        }
        ChunkCompression::Zlib => {
            let mut encoder = ZlibEncoder::new(Vec::new(), flate2::Compression::default());
            encoder.write_all(bytes)?;
            Ok(encoder.finish()?)
        }
        ChunkCompression::Lz4 => Ok(compress_lz4(bytes)),
    }
}

pub fn decompress(kind: ChunkCompression, bytes: &[u8], limit: usize) -> Result<Vec<u8>> {
    match kind {
        ChunkCompression::Uncompressed => {
            ensure!(
                bytes.len() <= limit,
                "chunk exceeds decompressed byte limit"
            );
            Ok(bytes.to_vec())
        }
        ChunkCompression::Gzip => {
            let mut decoder = GzDecoder::new(bytes);
            let mut out = Vec::new();
            decoder
                .by_ref()
                .take(limit.saturating_add(1) as u64)
                .read_to_end(&mut out)?;
            ensure!(
                out.len() <= limit,
                "gzip chunk exceeds decompressed byte limit"
            );
            ensure!(
                decoder.into_inner().is_empty(),
                "trailing data after gzip chunk"
            );
            Ok(out)
        }
        ChunkCompression::Zlib => decompress_zlib(bytes, limit),
        ChunkCompression::Lz4 => decompress_lz4(bytes, limit),
    }
}

fn decompress_zlib(bytes: &[u8], limit: usize) -> Result<Vec<u8>> {
    let mut decoder = flate2::Decompress::new(true);
    let mut out = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let before_in = decoder.total_in();
        let before_out = decoder.total_out();
        let status = decoder.decompress(
            &bytes[before_in as usize..],
            &mut buffer,
            flate2::FlushDecompress::None,
        )?;
        let written = (decoder.total_out() - before_out) as usize;
        ensure!(
            written <= limit.saturating_sub(out.len()),
            "zlib chunk exceeds decompressed byte limit"
        );
        out.extend_from_slice(&buffer[..written]);
        if status == flate2::Status::StreamEnd {
            ensure!(
                decoder.total_in() as usize == bytes.len(),
                "trailing data after zlib chunk"
            );
            return Ok(out);
        }
        ensure!(
            decoder.total_in() != before_in || written != 0,
            "truncated or unfinished zlib chunk"
        );
    }
}

fn compress_lz4(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for block in bytes.chunks(65536) {
        let compressed = lz4_flex::block::compress(block);
        let (token, payload) = if compressed.len() < block.len() {
            (0x26, compressed.as_slice())
        } else {
            (0x16, block)
        };
        out.extend_from_slice(LZ4_MAGIC);
        out.push(token);
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend_from_slice(&(block.len() as u32).to_le_bytes());
        out.extend_from_slice(&java_lz4_checksum(block).to_le_bytes());
        out.extend_from_slice(payload);
    }
    out.extend_from_slice(LZ4_MAGIC);
    out.push(0x16);
    out.extend_from_slice(&[0; 12]);
    out
}

fn decompress_lz4(bytes: &[u8], limit: usize) -> Result<Vec<u8>> {
    let mut position = 0_usize;
    let mut out = Vec::new();
    loop {
        ensure!(
            bytes.len().saturating_sub(position) >= 21,
            "truncated Java LZ4Block header"
        );
        let header = &bytes[position..position + 21];
        ensure!(&header[..8] == LZ4_MAGIC, "invalid Java LZ4Block magic");
        let method = header[8] & 0xf0;
        let block_limit = 1_usize << (10 + (header[8] & 15));
        ensure!(
            method == 0x10 || method == 0x20,
            "unknown Java LZ4Block method"
        );
        let compressed = u32::from_le_bytes(header[9..13].try_into().unwrap()) as usize;
        let decoded = u32::from_le_bytes(header[13..17].try_into().unwrap()) as usize;
        let checksum = u32::from_le_bytes(header[17..21].try_into().unwrap());
        position += 21;
        if compressed == 0 && decoded == 0 {
            ensure!(
                checksum == 0 && position == bytes.len(),
                "invalid Java LZ4Block end marker or trailing bytes"
            );
            return Ok(out);
        }
        ensure!(
            compressed > 0 && decoded > 0 && decoded <= block_limit,
            "invalid Java LZ4Block lengths"
        );
        ensure!(
            decoded <= limit.saturating_sub(out.len()),
            "LZ4 chunk exceeds decompressed byte limit"
        );
        ensure!(
            compressed <= bytes.len().saturating_sub(position),
            "truncated Java LZ4Block payload"
        );
        let payload = &bytes[position..position + compressed];
        position += compressed;
        let block = if method == 0x10 {
            ensure!(
                compressed == decoded,
                "raw Java LZ4Block has unequal lengths"
            );
            payload.to_vec()
        } else {
            let mut block = vec![0; decoded];
            ensure!(
                lz4_flex::block::decompress_into(payload, &mut block)? == decoded,
                "LZ4 block output length mismatch"
            );
            block
        };
        ensure!(
            java_lz4_checksum(&block) == checksum,
            "Java LZ4Block checksum mismatch"
        );
        out.extend_from_slice(&block);
    }
}

fn java_lz4_checksum(bytes: &[u8]) -> u32 {
    // lz4-java 1.8.0 StreamingXXHash32.asChecksum masks to 28 bits, despite
    // using a 32-bit little-endian checksum field. Verified in its bytecode.
    xxhash_rust::xxh32::xxh32(bytes, XXHASH_SEED) & 0x0fff_ffff
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nbt::{NbtRoot, Tag};
    use std::path::PathBuf;

    fn sample(value: i32) -> NbtRoot {
        NbtRoot {
            name: "".into(),
            tag: Tag::Compound(vec![
                ("DataVersion".into(), Tag::Int(3953)),
                ("xPos".into(), Tag::Int(value)),
                ("unknown-field".into(), Tag::Long(i64::MIN)),
            ]),
        }
    }

    fn temporary_directory() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "mc121-anvil-{}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        path
    }

    #[test]
    fn negative_coordinates_use_floor_division() {
        assert_eq!(region_coordinates(-1, -33), (-1, -2));
        assert_eq!(location_index(-1, -33), 1023);
        assert_eq!(
            coordinates_from_path(Path::new("r.-1.-2.mca")).unwrap(),
            (-1, -2)
        );
        assert!(coordinates_from_path(Path::new("wrong.mca")).is_err());
    }

    #[test]
    fn all_compression_formats_roundtrip_and_obey_bounds() {
        let data = [b'a'; 150_000];
        for kind in [
            ChunkCompression::Gzip,
            ChunkCompression::Zlib,
            ChunkCompression::Uncompressed,
            ChunkCompression::Lz4,
        ] {
            let compressed = compress(kind, &data).unwrap();
            assert_eq!(decompress(kind, &compressed, data.len()).unwrap(), data);
            assert!(decompress(kind, &compressed, data.len() - 1).is_err());
            if kind != ChunkCompression::Uncompressed {
                assert!(decompress(kind, &compressed[..compressed.len() - 1], data.len()).is_err());
            }
        }
    }

    #[test]
    fn malformed_headers_allocations_and_overlaps_are_rejected() {
        assert!(RegionFile::from_bytes(&[0; 10], 0, 0).is_err());
        let mut region = RegionFile::empty(0, 0).unwrap();
        region
            .insert_chunk(0, 0, &sample(0), ChunkCompression::Zlib, 7)
            .unwrap();
        let bytes = region.to_bytes().unwrap();
        let mut duplicate = bytes.clone();
        duplicate[4..8].copy_from_slice(&bytes[..4]);
        assert!(RegionFile::from_bytes(&duplicate, 0, 0).is_err());
        let mut bad = bytes.clone();
        bad[8192..8196].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(RegionFile::from_bytes(&bad, 0, 0).is_err());
        assert!(RegionFile::from_bytes(&bytes[..bytes.len() - 1], 0, 0).is_err());
    }

    #[test]
    fn no_op_roundtrip_and_unknown_fields_are_lossless() {
        let mut region = RegionFile::empty(-1, -1).unwrap();
        region
            .insert_chunk(-1, -1, &sample(-1), ChunkCompression::Zlib, 123456)
            .unwrap();
        region
            .insert_chunk(-2, -1, &sample(-2), ChunkCompression::Gzip, 234567)
            .unwrap();
        let bytes = region.to_bytes().unwrap();
        let decoded = RegionFile::from_bytes(&bytes, -1, -1).unwrap();
        assert_eq!(decoded.to_bytes().unwrap(), bytes);
        assert_eq!(
            decoded.chunk(-1, -1).unwrap().unwrap().encode().unwrap(),
            sample(-1).encode().unwrap()
        );
        assert_eq!(decoded.timestamp(-2, -1).unwrap(), 234567);
        assert!(decoded.chunk(0, 0).is_err());
    }

    #[test]
    fn editing_one_chunk_preserves_other_raw_sectors() {
        let mut region = RegionFile::empty(0, 0).unwrap();
        region
            .insert_chunk(1, 1, &sample(1), ChunkCompression::Zlib, 1)
            .unwrap();
        region
            .insert_chunk(3, 3, &sample(3), ChunkCompression::Gzip, 3)
            .unwrap();
        let bytes = region.to_bytes().unwrap();
        let mut changed = RegionFile::from_bytes(&bytes, 0, 0).unwrap();
        let untouched = changed.raw_sectors(3, 3).unwrap().unwrap().to_vec();
        changed
            .insert_chunk(1, 1, &sample(10), ChunkCompression::Lz4, 10)
            .unwrap();
        let reread = RegionFile::from_bytes(&changed.to_bytes().unwrap(), 0, 0).unwrap();
        assert_eq!(reread.raw_sectors(3, 3).unwrap().unwrap(), untouched);
        assert_eq!(reread.timestamp(3, 3).unwrap(), 3);
        assert_eq!(
            reread.chunk(1, 1).unwrap().unwrap().encode().unwrap(),
            sample(10).encode().unwrap()
        );
    }

    #[test]
    fn external_large_chunks_use_mcc_and_atomic_materialization() {
        let directory = temporary_directory();
        let path = directory.join("r.-1.0.mca");
        let root = NbtRoot {
            name: "".into(),
            tag: Tag::Compound(vec![("huge".into(), Tag::ByteArray(vec![3; 1_100_000]))]),
        };
        let mut region = RegionFile::empty(-1, 0).unwrap();
        region
            .insert_chunk(-1, 0, &root, ChunkCompression::Uncompressed, 8)
            .unwrap();
        assert!(region.is_external(-1, 0).unwrap());
        region.save_atomic(&path).unwrap();
        assert!(directory.join("c.-1.0.mcc").is_file());
        let mut read = RegionFile::read(&path).unwrap();
        assert_eq!(
            read.chunk(-1, 0).unwrap().unwrap().encode().unwrap(),
            root.encode().unwrap()
        );
        assert_eq!(read.to_bytes().unwrap(), fs::read(&path).unwrap());
        assert!(RegionFile::from_bytes(&read.to_bytes().unwrap(), -1, 0)
            .unwrap()
            .chunk(-1, 0)
            .is_err());
        assert!(read.remove_chunk_at(-1, 0, 42).unwrap());
        read.save_atomic(&path).unwrap();
        assert!(!directory.join("c.-1.0.mcc").exists());
        let removed = RegionFile::read(&path).unwrap();
        assert!(removed.chunk(-1, 0).unwrap().is_none());
        assert_eq!(removed.timestamp(-1, 0).unwrap(), 42);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn lz4_checksum_and_end_marker_are_verified() {
        let encoded = compress(
            ChunkCompression::Lz4,
            b"hello world hello world hello world",
        )
        .unwrap();
        let mut bad = encoded.clone();
        bad[17] ^= 1;
        assert!(decompress(ChunkCompression::Lz4, &bad, 1024).is_err());
        let mut trailing = encoded;
        trailing.push(0);
        assert!(decompress(ChunkCompression::Lz4, &trailing, 1024).is_err());
    }
}
