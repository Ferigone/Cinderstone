//! Minecraft Java 1.21 packet framing and primitive wire types.
//!
//! `FrameCodec::read` is deliberately not cancellation-safe: a connection must
//! keep one dedicated reader task, rather than recreate this future in a select
//! loop after it may already have consumed part of a frame.

use anyhow::{anyhow, bail, Context, Result};
use flate2::{write::ZlibEncoder, Compression, Decompress, FlushDecompress, Status};
use std::io::Write as _;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// The outer frame length is a VarInt21, not a general five-byte VarInt.
pub const MAX_FRAME_LENGTH: usize = (1 << 21) - 1;
/// Vanilla 1.21's compression decoder/encoder permit this decoded size.
pub const MAX_UNCOMPRESSED_SIZE: usize = 8 * 1024 * 1024;
/// Retained as a public alias for callers that used the original wire limit.
pub const MAX_PACKET_SIZE: usize = MAX_FRAME_LENGTH;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packet {
    pub id: i32,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, Default)]
pub struct FrameCodec {
    pub compression: Option<i32>,
}

impl FrameCodec {
    /// Read a complete frame. EOF is clean only before its first length byte.
    pub async fn read<R: AsyncRead + Unpin>(&self, stream: &mut R) -> Result<Option<Packet>> {
        self.validate_threshold()?;
        let Some(length) = read_frame_length(stream).await? else {
            return Ok(None);
        };
        if length <= 0 {
            bail!("packet frame length must be positive");
        }
        let length = length as usize;
        if length > MAX_FRAME_LENGTH {
            bail!("packet frame exceeds {MAX_FRAME_LENGTH} bytes");
        }
        let mut frame = vec![0; length];
        stream
            .read_exact(&mut frame)
            .await
            .context("truncated packet frame")?;

        let body = if let Some(threshold) = self.compression {
            let mut header = Reader::new(&frame);
            let declared_size = header.varint()?;
            if declared_size < 0 {
                bail!("negative uncompressed packet length");
            }
            if declared_size == 0 {
                // Vanilla's CompressionDecoder passes these bytes through even
                // if they meet the threshold. Its threshold check applies only
                // to nonzero declared sizes (confirmed in vr bytecode).
                header.remaining().to_vec()
            } else {
                let expected = declared_size as usize;
                if expected < threshold as usize {
                    bail!("compressed packet is smaller than compression threshold");
                }
                if expected > MAX_UNCOMPRESSED_SIZE {
                    bail!("uncompressed packet exceeds {MAX_UNCOMPRESSED_SIZE} bytes");
                }
                decompress_exact(header.remaining(), expected)?
            }
        } else {
            frame
        };

        let mut reader = Reader::new(&body);
        let id = reader.varint().context("missing or invalid packet ID")?;
        if id < 0 {
            bail!("negative packet ID");
        }
        Ok(Some(Packet {
            id,
            payload: reader.remaining().to_vec(),
        }))
    }

    pub async fn write<W: AsyncWrite + Unpin>(
        &self,
        stream: &mut W,
        id: i32,
        payload: &[u8],
    ) -> Result<()> {
        self.validate_threshold()?;
        if id < 0 {
            bail!("negative packet ID");
        }
        // Check before copying an attacker-controlled payload into a new buffer.
        if payload.len() > MAX_UNCOMPRESSED_SIZE {
            bail!("packet payload exceeds {MAX_UNCOMPRESSED_SIZE} bytes");
        }
        let mut body = Writer::new();
        body.varint(id).raw(payload);
        let body = body.into_inner();
        if body.len() > MAX_UNCOMPRESSED_SIZE {
            bail!("packet body exceeds {MAX_UNCOMPRESSED_SIZE} bytes");
        }

        let frame = if let Some(threshold) = self.compression {
            let mut frame = Writer::new();
            if body.len() >= threshold as usize {
                let mut compressor = ZlibEncoder::new(Vec::new(), Compression::default());
                compressor.write_all(&body).context("zlib compression")?;
                let compressed = compressor.finish().context("finishing zlib stream")?;
                frame.varint(body.len() as i32).raw(&compressed);
            } else {
                frame.varint(0).raw(&body);
            }
            frame.into_inner()
        } else {
            body
        };
        if frame.len() > MAX_FRAME_LENGTH {
            bail!("encoded frame exceeds {MAX_FRAME_LENGTH} bytes");
        }
        let mut header = Writer::new();
        header.varint(frame.len() as i32);
        stream
            .write_all(&header.into_inner())
            .await
            .context("writing packet length")?;
        stream
            .write_all(&frame)
            .await
            .context("writing packet body")?;
        Ok(())
    }

    fn validate_threshold(&self) -> Result<()> {
        if self.compression.is_some_and(|threshold| threshold < 0) {
            bail!("negative compression threshold; use None to disable compression");
        }
        Ok(())
    }
}

fn decompress_exact(input: &[u8], expected: usize) -> Result<Vec<u8>> {
    // Intentional checkpoint hardening: unlike vanilla's single inflate call,
    // require a finished stream and reject compressed trailers/extra output.
    // This difference concerns malformed streams, not valid client packets.
    // One additional byte detects an expanding stream without ever allowing
    // read_to_end or a malicious declared size to allocate an unbounded buffer.
    let mut output = vec![0; expected + 1];
    let mut decoder = Decompress::new(true);
    let status = decoder
        .decompress(input, &mut output, FlushDecompress::Finish)
        .context("invalid zlib packet")?;
    if status != Status::StreamEnd {
        bail!("incomplete zlib stream or decompressed packet exceeds declared length");
    }
    if decoder.total_out() != expected as u64 {
        bail!("decompressed packet length does not match declaration");
    }
    if decoder.total_in() != input.len() as u64 {
        bail!("trailing data after zlib stream");
    }
    output.truncate(expected);
    Ok(output)
}

async fn read_frame_length<R: AsyncRead + Unpin>(stream: &mut R) -> Result<Option<i32>> {
    let mut value = 0_u32;
    for index in 0..3 {
        let mut byte = [0_u8];
        let read = stream
            .read(&mut byte)
            .await
            .context("reading frame length")?;
        if read == 0 {
            if index == 0 {
                return Ok(None);
            }
            bail!("truncated frame length VarInt");
        }
        let byte = byte[0];
        value |= u32::from(byte & 0x7f) << (7 * index);
        if byte & 0x80 == 0 {
            return Ok(Some(value as i32));
        }
    }
    bail!("frame length wider than 21-bit (three bytes)")
}

#[derive(Debug, Clone)]
pub struct Reader<'a> {
    input: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    pub fn new(input: &'a [u8]) -> Self {
        Self { input, offset: 0 }
    }

    pub fn bytes(&mut self, length: usize) -> Result<&'a [u8]> {
        let end = self
            .offset
            .checked_add(length)
            .filter(|&end| end <= self.input.len())
            .ok_or_else(|| anyhow!("truncated packet field: need {length} bytes"))?;
        let value = &self.input[self.offset..end];
        self.offset = end;
        Ok(value)
    }

    fn take<const N: usize>(&mut self) -> Result<[u8; N]> {
        self.bytes(N)?
            .try_into()
            .map_err(|_| anyhow!("invalid fixed-width packet field"))
    }

    pub fn remaining(&self) -> &'a [u8] {
        &self.input[self.offset..]
    }

    pub fn finish(&self) -> Result<()> {
        if !self.remaining().is_empty() {
            bail!("{} trailing packet bytes", self.remaining().len());
        }
        Ok(())
    }

    pub fn varint(&mut self) -> Result<i32> {
        let mut value = 0_u32;
        for index in 0..5 {
            let byte = self.u8().context("truncated VarInt")?;
            if index == 4 && byte & 0xf0 != 0 {
                bail!("VarInt overflows 32 bits");
            }
            value |= u32::from(byte & 0x7f) << (7 * index);
            if byte & 0x80 == 0 {
                return Ok(value as i32);
            }
        }
        bail!("VarInt exceeds five bytes")
    }

    /// Match Java's replacement UTF-8 decoding and UTF-16 String length limit.
    pub fn string(&mut self, max_chars: usize) -> Result<String> {
        let length = self.varint()?;
        if length < 0 {
            bail!("negative string byte length");
        }
        let length = length as usize;
        let max_bytes = max_chars
            .checked_mul(3)
            .ok_or_else(|| anyhow!("string character limit overflows"))?;
        if length > max_bytes {
            bail!("string exceeds maximum encoded byte length");
        }
        let text = decode_java_utf8(self.bytes(length)?);
        if text.encode_utf16().count() > max_chars {
            bail!("string exceeds maximum Java character length");
        }
        Ok(text)
    }

    // FriendlyByteBuf.readBoolean treats any nonzero byte as true.
    pub fn bool(&mut self) -> Result<bool> {
        Ok(self.u8()? != 0)
    }
    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.take::<1>()?[0])
    }
    pub fn i8(&mut self) -> Result<i8> {
        Ok(self.u8()? as i8)
    }
    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_be_bytes(self.take()?))
    }
    pub fn i16(&mut self) -> Result<i16> {
        Ok(i16::from_be_bytes(self.take()?))
    }
    pub fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_be_bytes(self.take()?))
    }
    pub fn i64(&mut self) -> Result<i64> {
        Ok(i64::from_be_bytes(self.take()?))
    }
    pub fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_be_bytes(self.take()?))
    }
    pub fn f64(&mut self) -> Result<f64> {
        Ok(f64::from_be_bytes(self.take()?))
    }
    pub fn uuid(&mut self) -> Result<[u8; 16]> {
        self.take()
    }
}

/// `Utf8String.read` delegates to Netty's Java UTF-8 string decoder. Java
/// replaces each malformed prefix, including one replacement for an encoded
/// surrogate; Rust's `from_utf8_lossy` instead splits that surrogate's bytes.
fn decode_java_utf8(input: &[u8]) -> String {
    if let Ok(text) = std::str::from_utf8(input) {
        return text.to_owned();
    }
    let mut text = String::with_capacity(input.len());
    let mut offset = 0;
    while offset < input.len() {
        let first = input[offset];
        if first < 0x80 {
            text.push(char::from(first));
            offset += 1;
            continue;
        }
        let (width, second_min, second_max) = match first {
            0xc2..=0xdf => (2, 0x80, 0xbf),
            0xe0 => (3, 0xa0, 0xbf),
            0xe1..=0xef => (3, 0x80, 0xbf),
            0xf0 => (4, 0x90, 0xbf),
            0xf1..=0xf3 => (4, 0x80, 0xbf),
            0xf4 => (4, 0x80, 0x8f),
            _ => {
                text.push('\u{fffd}');
                offset += 1;
                continue;
            }
        };
        if !input
            .get(offset + 1)
            .is_some_and(|&second| (second_min..=second_max).contains(&second))
        {
            text.push('\u{fffd}');
            offset += 1;
            continue;
        }
        // Consume a valid prefix as one malformed unit if the next byte is
        // absent or is not a continuation. Leave the offending byte for the
        // next iteration, preserving Java's replacement grouping.
        let mut consumed = 2;
        while consumed < width
            && input
                .get(offset + consumed)
                .is_some_and(|&byte| byte & 0xc0 == 0x80)
        {
            consumed += 1;
        }
        if consumed < width {
            text.push('\u{fffd}');
            offset += consumed;
            continue;
        }
        let mut scalar = u32::from(first & (0x7f >> width));
        for &byte in &input[offset + 1..offset + width] {
            scalar = (scalar << 6) | u32::from(byte & 0x3f);
        }
        // A complete UTF-8 encoding of a UTF-16 surrogate consumes all three
        // bytes and produces one U+FFFD in Java, rather than a Rust char.
        text.push(char::from_u32(scalar).unwrap_or('\u{fffd}'));
        offset += width;
    }
    text
}

#[derive(Debug, Clone, Default)]
pub struct Writer {
    output: Vec<u8>,
}

impl Writer {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn into_inner(self) -> Vec<u8> {
        self.output
    }

    pub fn varint(&mut self, value: i32) -> &mut Self {
        let mut value = value as u32;
        loop {
            let mut byte = (value & 0x7f) as u8;
            value >>= 7;
            if value != 0 {
                byte |= 0x80;
            }
            self.output.push(byte);
            if value == 0 {
                return self;
            }
        }
    }

    pub fn string(&mut self, value: &str) -> &mut Self {
        self.varint(value.len() as i32).raw(value.as_bytes())
    }
    pub fn bool(&mut self, value: bool) -> &mut Self {
        self.u8(u8::from(value))
    }
    pub fn u8(&mut self, value: u8) -> &mut Self {
        self.output.push(value);
        self
    }
    pub fn i8(&mut self, value: i8) -> &mut Self {
        self.u8(value as u8)
    }
    pub fn u16(&mut self, value: u16) -> &mut Self {
        self.raw(&value.to_be_bytes())
    }
    pub fn i16(&mut self, value: i16) -> &mut Self {
        self.raw(&value.to_be_bytes())
    }
    pub fn i32(&mut self, value: i32) -> &mut Self {
        self.raw(&value.to_be_bytes())
    }
    pub fn i64(&mut self, value: i64) -> &mut Self {
        self.raw(&value.to_be_bytes())
    }
    pub fn f32(&mut self, value: f32) -> &mut Self {
        self.raw(&value.to_be_bytes())
    }
    pub fn f64(&mut self, value: f64) -> &mut Self {
        self.raw(&value.to_be_bytes())
    }
    pub fn uuid(&mut self, value: [u8; 16]) -> &mut Self {
        self.raw(&value)
    }
    pub fn raw(&mut self, value: &[u8]) -> &mut Self {
        self.output.extend_from_slice(value);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(body: &[u8]) -> Vec<u8> {
        let mut wire = Writer::new();
        wire.varint(body.len() as i32).raw(body);
        wire.into_inner()
    }

    fn compressed_frame(declared_size: i32, input: &[u8], suffix: &[u8]) -> Vec<u8> {
        let mut compressor = ZlibEncoder::new(Vec::new(), Compression::default());
        compressor.write_all(input).unwrap();
        let compressed = compressor.finish().unwrap();
        let mut body = Writer::new();
        body.varint(declared_size).raw(&compressed).raw(suffix);
        frame(&body.into_inner())
    }

    #[test]
    fn signed_varint_boundaries_roundtrip() {
        for value in [
            0,
            1,
            127,
            128,
            255,
            16383,
            16384,
            i32::MAX,
            -1,
            -128,
            i32::MIN,
        ] {
            let mut writer = Writer::new();
            writer.varint(value);
            let encoded = writer.into_inner();
            let mut reader = Reader::new(&encoded);
            assert_eq!(reader.varint().unwrap(), value);
            reader.finish().unwrap();
        }
        let mut writer = Writer::new();
        writer.varint(-1);
        assert_eq!(writer.into_inner(), [0xff, 0xff, 0xff, 0xff, 0x0f]);
    }

    #[test]
    fn rejects_truncated_overflow_and_six_byte_varints() {
        for input in [
            &[0x80][..],
            &[0xff, 0xff, 0xff, 0xff, 0x10][..],
            &[0x80, 0x80, 0x80, 0x80, 0x80, 0x00][..],
        ] {
            assert!(Reader::new(input).varint().is_err());
        }
    }

    #[test]
    fn strings_enforce_utf8_and_java_utf16_limits() {
        let mut writer = Writer::new();
        writer.string("żółw");
        let encoded = writer.into_inner();
        assert_eq!(Reader::new(&encoded).string(4).unwrap(), "żółw");
        assert!(Reader::new(&encoded).string(3).is_err());

        let mut writer = Writer::new();
        writer.string("😀");
        let encoded = writer.into_inner();
        assert!(Reader::new(&encoded).string(1).is_err());
        assert_eq!(Reader::new(&encoded).string(2).unwrap(), "😀");
        // Java's Utf8String.read replaces malformed UTF-8 (docs/reference/connection-utf8.md)
        // instead of rejecting the field; the length bounds still apply.
        assert_eq!(Reader::new(&[1, 0xff]).string(1).unwrap(), "\u{fffd}");
        assert!(Reader::new(&[2, b'a']).string(2).is_err());
        assert!(Reader::new(&[0xff, 0xff, 0xff, 0xff, 0x0f])
            .string(5)
            .is_err());
    }

    #[test]
    fn fixed_width_fields_are_big_endian_and_trailing_bytes_rejected() {
        let mut writer = Writer::new();
        writer
            .bool(true)
            .u8(200)
            .i8(-3)
            .u16(25565)
            .i16(-40)
            .i32(-500)
            .i64(i64::MIN)
            .f32(1.5)
            .f64(-2.25)
            .uuid([0xab; 16]);
        let encoded = writer.into_inner();
        assert_eq!(&encoded[3..5], &[0x63, 0xdd]);
        let mut reader = Reader::new(&encoded);
        assert!(reader.bool().unwrap());
        assert_eq!(reader.u8().unwrap(), 200);
        assert_eq!(reader.i8().unwrap(), -3);
        assert_eq!(reader.u16().unwrap(), 25565);
        assert_eq!(reader.i16().unwrap(), -40);
        assert_eq!(reader.i32().unwrap(), -500);
        assert_eq!(reader.i64().unwrap(), i64::MIN);
        assert_eq!(reader.f32().unwrap(), 1.5);
        assert_eq!(reader.f64().unwrap(), -2.25);
        assert_eq!(reader.uuid().unwrap(), [0xab; 16]);
        reader.finish().unwrap();
        assert!(reader.bytes(usize::MAX).is_err());
        assert!(Reader::new(&[0]).finish().is_err());
    }

    #[tokio::test]
    async fn roundtrips_plain_and_compressed_threshold_boundaries() {
        for threshold in [None, Some(0), Some(256)] {
            for size in [0, 1, 254, 255, 256, 8192] {
                let codec = FrameCodec {
                    compression: threshold,
                };
                let payload = vec![0x41; size];
                let mut wire = Vec::new();
                codec.write(&mut wire, 0x7b, &payload).await.unwrap();
                let mut source = wire.as_slice();
                assert_eq!(
                    codec.read(&mut source).await.unwrap(),
                    Some(Packet { id: 0x7b, payload })
                );
                assert!(codec.read(&mut source).await.unwrap().is_none());
            }
        }
    }

    #[tokio::test]
    async fn rejects_partial_eof_negative_and_oversized_frames() {
        let codec = FrameCodec::default();
        let mut empty = &[][..];
        assert!(codec.read(&mut empty).await.unwrap().is_none());
        let mut oversized = Writer::new();
        oversized.varint((MAX_PACKET_SIZE + 1) as i32);
        for wire in [
            vec![0x80],
            vec![0],
            vec![3, 0, 1],
            vec![0xff, 0xff, 0xff, 0xff, 0x0f],
            vec![0xff, 0xff, 0xff, 0xff, 0x10],
            oversized.into_inner(),
            frame(&[0xff, 0xff, 0xff, 0xff, 0x0f]),
        ] {
            assert!(codec.read(&mut wire.as_slice()).await.is_err());
        }
    }

    #[tokio::test]
    async fn rejects_compression_bombs_mismatch_trailing_data_and_small_compressed_packets() {
        let codec = FrameCodec {
            compression: Some(1),
        };
        for wire in [
            compressed_frame(2, &[0, 1, 2], &[]),
            compressed_frame(8, &[0, 1], &[]),
            compressed_frame(2, &[0, 1], &[0]),
            compressed_frame(2, &vec![0; 100_000], &[]),
            compressed_frame((MAX_UNCOMPRESSED_SIZE + 1) as i32, &[0], &[]),
            frame(&[1, 0x78]),
        ] {
            assert!(codec.read(&mut wire.as_slice()).await.is_err());
        }
        let codec = FrameCodec {
            compression: Some(256),
        };
        let wire = compressed_frame(1, &[0], &[]);
        assert!(codec.read(&mut wire.as_slice()).await.is_err());
    }

    #[tokio::test]
    async fn outer_lengths_are_varint21_while_payload_varints_still_allow_five_bytes() {
        let codec = FrameCodec::default();
        // This encodes length 1 in four bytes. A generic VarInt decoder would
        // accept it, whereas Minecraft's Varint21FrameDecoder rejects it.
        let overlong = [0x81, 0x80, 0x80, 0x00, 0x00];
        assert!(codec.read(&mut overlong.as_slice()).await.is_err());
        assert!(codec
            .read(&mut [0x80, 0x80, 0x80].as_slice())
            .await
            .is_err());

        // Vanilla permits noncanonical encodings within the three-byte limit.
        let within_limit = [0x81, 0x80, 0x00, 0x00];
        assert_eq!(
            codec.read(&mut within_limit.as_slice()).await.unwrap(),
            Some(Packet {
                id: 0,
                payload: vec![]
            })
        );
        assert_eq!(
            Reader::new(&[0xff, 0xff, 0xff, 0xff, 0x0f])
                .varint()
                .unwrap(),
            -1
        );
    }

    #[tokio::test]
    async fn exact_outer_size_boundary_is_three_bytes_and_next_size_is_rejected() {
        let codec = FrameCodec::default();
        let payload = vec![0x42; MAX_FRAME_LENGTH - 1];
        let mut wire = Vec::new();
        codec.write(&mut wire, 0, &payload).await.unwrap();
        assert_eq!(&wire[..3], &[0xff, 0xff, 0x7f]);
        assert_eq!(wire.len(), MAX_FRAME_LENGTH + 3);
        let packet = codec.read(&mut wire.as_slice()).await.unwrap().unwrap();
        assert_eq!(packet.payload, payload);

        let mut rejected_wire = Vec::new();
        assert!(codec
            .write(&mut rejected_wire, 0, &vec![0; MAX_FRAME_LENGTH])
            .await
            .is_err());
        assert!(rejected_wire.is_empty());
    }

    #[tokio::test]
    async fn vanilla_accepts_uncompressed_data_length_zero_above_threshold() {
        for threshold in [0, 1, 256] {
            let codec = FrameCodec {
                compression: Some(threshold),
            };
            let mut body = Writer::new();
            let payload = vec![0x41; 300];
            body.varint(0).varint(0x7b).raw(&payload);
            let wire = frame(&body.into_inner());
            assert_eq!(
                codec.read(&mut wire.as_slice()).await.unwrap(),
                Some(Packet { id: 0x7b, payload })
            );
        }
    }

    #[tokio::test]
    async fn compressed_body_limit_is_eight_mib_not_the_outer_wire_limit() {
        let codec = FrameCodec {
            compression: Some(256),
        };
        let payload = vec![0; MAX_UNCOMPRESSED_SIZE - 1];
        let mut wire = Vec::new();
        codec.write(&mut wire, 0, &payload).await.unwrap();
        assert!(wire.len() < MAX_FRAME_LENGTH);
        let packet = codec.read(&mut wire.as_slice()).await.unwrap().unwrap();
        assert_eq!(packet.payload, payload);

        let mut rejected_wire = Vec::new();
        assert!(codec
            .write(&mut rejected_wire, 0, &vec![0; MAX_UNCOMPRESSED_SIZE])
            .await
            .is_err());
        assert!(rejected_wire.is_empty());
    }

    #[tokio::test]
    async fn rejects_invalid_write_configuration_before_writing_bytes() {
        let mut wire = Vec::new();
        assert!(FrameCodec {
            compression: Some(-1)
        }
        .write(&mut wire, 0, &[])
        .await
        .is_err());
        assert!(FrameCodec::default()
            .write(&mut wire, -1, &[])
            .await
            .is_err());
        assert!(FrameCodec::default()
            .write(&mut wire, 0, &vec![0; MAX_PACKET_SIZE])
            .await
            .is_err());
        assert!(wire.is_empty());
    }
}
