//! Deterministic flat fixture world for Java protocol 767 interoperability.
//!
//! This is not vanilla world generation or a vanilla lighting simulation.
//! The fixture has stone through Y=60, dirt at 61..=62, grass at 63, and air
//! above. Heightmaps describe non-air occupancy; skylight is deliberately full
//! in every section. They are sufficient for a rendering/protocol checkpoint,
//! not evidence of survival, redstone, fluid, opacity or generation parity.

use std::collections::BTreeMap;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

pub const MIN_Y: i32 = -64;
pub const MAX_Y: i32 = 319;

/// BiomeManager.obfuscateSeed: Guava hashes the seed's little-endian long and
/// interprets the first eight SHA-256 bytes as another little-endian long.
/// This is the seed sent to Java clients for biome zoom, not the world seed.
pub fn obfuscate_biome_seed(seed: i64) -> i64 {
    let digest = openssl::sha::sha256(&seed.to_le_bytes());
    i64::from_le_bytes(digest[..8].try_into().expect("SHA-256 prefix"))
}
const SECTION_COUNT: usize = 24;
const SECTION_VOLUME: usize = 4096;
const MAX_BLOCK_STATE: u32 = (1 << 15) - 1;
const MIN_WIRE_COORDINATE: i32 = -(1 << 25);
const MAX_WIRE_COORDINATE: i32 = (1 << 25) - 1;

/// One actual change relative to the deterministic fixture, in world coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockChange {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub state: u32,
}

#[derive(Debug, Clone)]
pub struct FlatWorld {
    air: u32,
    stone: u32,
    dirt: u32,
    grass: u32,
    biome: u32,
    // A local index uses X in bits 0..4, Z in 4..8 and Y+64 above bit 8.
    // Keeping diffs per chunk makes ordinary unmodified sections inexpensive.
    changes: BTreeMap<(i32, i32), BTreeMap<u32, u32>>,
    native_block_entities: crate::native_block_entities::NativeBlockEntities,
}

impl FlatWorld {
    pub fn new(air: u32, stone: u32, dirt: u32, grass: u32, biome: u32) -> Self {
        assert!(
            [air, stone, dirt, grass]
                .iter()
                .all(|&state| state <= MAX_BLOCK_STATE),
            "protocol 767 block states must fit the 15-bit global palette"
        );
        assert!(biome <= i32::MAX as u32, "biome ID must fit a VarInt");
        Self {
            air,
            stone,
            dirt,
            grass,
            biome,
            changes: BTreeMap::new(),
            native_block_entities: crate::native_block_entities::NativeBlockEntities::default(),
        }
    }

    pub fn base_states(&self) -> [u32; 4] {
        [self.air, self.stone, self.dirt, self.grass]
    }
    pub fn biome_id(&self) -> u32 {
        self.biome
    }
    pub fn changed_states_in_chunk(&self, chunk: (i32, i32)) -> impl Iterator<Item = u32> + '_ {
        self.changes
            .get(&chunk)
            .into_iter()
            .flat_map(|changes| changes.values().copied())
    }

    fn base_block(&self, y: i32) -> u32 {
        match y {
            MIN_Y..=60 => self.stone,
            61..=62 => self.dirt,
            63 => self.grass,
            _ => self.air,
        }
    }

    pub fn block(&self, x: i32, y: i32, z: i32) -> u32 {
        if !(MIN_Y..=MAX_Y).contains(&y) {
            return self.air;
        }
        let chunk = (x.div_euclid(16), z.div_euclid(16));
        let local = local_index(x.rem_euclid(16), y, z.rem_euclid(16));
        self.changes
            .get(&chunk)
            .and_then(|changes| changes.get(&local))
            .copied()
            .unwrap_or_else(|| self.base_block(y))
    }

    /// Change a representable block; return true only when its state changed.
    /// Invalid coordinates and IDs are rejected without modifying the world.
    pub fn set_block(&mut self, x: i32, y: i32, z: i32, state: u32) -> bool {
        if !valid_change(x, y, z, state) || self.block(x, y, z) == state {
            return false;
        }
        if self.native_block_entities.transition((x,y,z), self.block(x,y,z),state).is_err() {
            return false;
        }
        self.set_block_storage(x,y,z,state)
    }
    fn set_block_storage(&mut self, x:i32,y:i32,z:i32,state:u32)->bool {
        if !valid_change(x,y,z,state)||self.block(x,y,z)==state{return false;}
        let chunk = (x.div_euclid(16), z.div_euclid(16));
        let local = local_index(x.rem_euclid(16), y, z.rem_euclid(16));
        if state == self.base_block(y) {
            let remove_chunk = if let Some(changes) = self.changes.get_mut(&chunk) {
                changes.remove(&local);
                changes.is_empty()
            } else {
                false
            };
            if remove_chunk {
                self.changes.remove(&chunk);
            }
        } else {
            self.changes.entry(chunk).or_default().insert(local, state);
        }
        true
    }

    /// Canonical, deterministic changes suitable for a versioned persistence file.
    pub fn changes(&self) -> Vec<BlockChange> {
        let mut result = Vec::new();
        for (&(cx, cz), changes) in &self.changes {
            for (&index, &state) in changes {
                result.push(BlockChange {
                    x: cx * 16 + (index & 15) as i32,
                    y: (index >> 8) as i32 + MIN_Y,
                    z: cz * 16 + ((index >> 4) & 15) as i32,
                    state,
                });
            }
        }
        result
    }

    /// Validate the entire input before applying any changes.
    pub fn apply_changes(&mut self, changes: &[BlockChange]) -> Result<usize> {
        for change in changes {
            if !valid_change(change.x, change.y, change.z, change.state) {
                bail!("invalid persisted block change: {change:?}");
            }
        }
        Ok(changes
            .iter()
            .filter(|change| self.set_block_storage(change.x, change.y, change.z, change.state))
            .count())
    }
    pub fn native_block_entities(&self)->&crate::native_block_entities::NativeBlockEntities{&self.native_block_entities}
    pub fn native_block_entities_mut(&mut self)->&mut crate::native_block_entities::NativeBlockEntities{&mut self.native_block_entities}
    pub fn restore_native_block_entities(&mut self,saved:crate::native_block_entities::SavedNativeBlockEntities,_now:i64)->Result<()>{let mut owner=crate::native_block_entities::NativeBlockEntities::default();owner.restore(saved,|p|self.block(p.0,p.1,p.2))?;self.native_block_entities=owner;Ok(())}

    fn local_state(&self, changes: Option<&BTreeMap<u32, u32>>, index: u32) -> u32 {
        changes
            .and_then(|changes| changes.get(&index))
            .copied()
            .unwrap_or_else(|| self.base_block((index >> 8) as i32 + MIN_Y))
    }

    fn height_values(&self, changes: Option<&BTreeMap<u32, u32>>) -> [u16; 256] {
        let mut heights = [128_u16; 256];
        if let Some(changes) = changes {
            // Added non-air blocks may raise a column above the baseline.
            for (&index, &state) in changes {
                if state != self.air {
                    let column = (index & 255) as usize;
                    heights[column] = heights[column].max((index >> 8) as u16 + 1);
                }
            }
            // Removed top blocks may lower it. Lower candidates are checked
            // against the actual diffs, including completely emptied columns.
            for (column, height) in heights.iter_mut().enumerate() {
                while *height > 0 {
                    let index = (u32::from(*height - 1) << 8) | column as u32;
                    if self.local_state(Some(changes), index) != self.air {
                        break;
                    }
                    *height -= 1;
                }
            }
        }
        heights
    }

    /// Clientbound Level Chunk With Light payload, excluding packet ID/framing.
    /// The protocol-767 packet ID is 0x27. Sources: PrismarineJS minecraft-data
    /// data/pc/1.21.1/protocol.json (1.21 and 1.21.1 share protocol 767).
    pub fn chunk_packet(&self, x: i32, z: i32) -> Vec<u8> {
        let changes = self.changes.get(&(x, z));
        let mut sections = Vec::with_capacity(8192);
        for section in 0..SECTION_COUNT {
            let start = (section * SECTION_VOLUME) as u32;
            let end = start + SECTION_VOLUME as u32;
            let mut states = (0..SECTION_VOLUME)
                .map(|index| self.base_block(MIN_Y + (section * 16 + index / 256) as i32))
                .collect::<Vec<_>>();
            if let Some(changes) = changes {
                for (&index, &state) in changes.range(start..end) {
                    states[(index - start) as usize] = state;
                }
            }
            let count = states.iter().filter(|&&state| state != self.air).count() as i16;
            sections.extend_from_slice(&count.to_be_bytes());
            encode_block_palette(&states, &mut sections);
            // All 64 biome entries in this fixture section are identical.
            sections.push(0);
            varint(self.biome, &mut sections);
            varint(0, &mut sections);
        }

        let mut out = Vec::with_capacity(sections.len() + 55_000);
        out.extend_from_slice(&x.to_be_bytes());
        out.extend_from_slice(&z.to_be_bytes());
        encode_heightmaps(&self.height_values(changes), &mut out);
        varint(sections.len() as u32, &mut out);
        out.extend_from_slice(&sections);
        self.native_block_entities.write_chunk_entities(x,z,&mut out);

        let all_sections = (1_u64 << (SECTION_COUNT + 2)) - 1;
        bitset(Some(all_sections), &mut out); // all 26 skylight arrays present
        bitset(None, &mut out); // no non-empty block-light arrays
        bitset(None, &mut out); // no explicitly empty skylight arrays
        bitset(Some(all_sections), &mut out); // all block-light sections empty
        varint((SECTION_COUNT + 2) as u32, &mut out);
        for _ in 0..SECTION_COUNT + 2 {
            varint(2048, &mut out);
            out.extend_from_slice(&[0xff; 2048]);
        }
        varint(0, &mut out); // no block-light byte arrays
        out
    }
}

fn valid_change(x: i32, y: i32, z: i32, state: u32) -> bool {
    (MIN_WIRE_COORDINATE..=MAX_WIRE_COORDINATE).contains(&x)
        && (MIN_WIRE_COORDINATE..=MAX_WIRE_COORDINATE).contains(&z)
        && (MIN_Y..=MAX_Y).contains(&y)
        && state <= MAX_BLOCK_STATE
}

fn local_index(x: i32, y: i32, z: i32) -> u32 {
    (((y - MIN_Y) as u32) << 8) | ((z as u32) << 4) | x as u32
}

fn varint(mut value: u32, out: &mut Vec<u8>) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        out.push(if value != 0 { byte | 0x80 } else { byte });
        if value == 0 {
            break;
        }
    }
}

fn bitset(value: Option<u64>, out: &mut Vec<u8>) {
    match value {
        Some(value) => {
            varint(1, out);
            out.extend_from_slice(&value.to_be_bytes());
        }
        None => varint(0, out),
    }
}

/// Modern SimpleBitStorage does not let a value span two network longs.
fn pack(values: impl IntoIterator<Item = u32>, bits: usize) -> Vec<u64> {
    let per_long = 64 / bits;
    let mask = (1_u64 << bits) - 1;
    let mut words = Vec::new();
    for (index, value) in values.into_iter().enumerate() {
        if index % per_long == 0 {
            words.push(0);
        }
        *words.last_mut().expect("one word just allocated") |=
            (u64::from(value) & mask) << ((index % per_long) * bits);
    }
    words
}

fn encode_block_palette(states: &[u32], out: &mut Vec<u8>) {
    let mut palette = Vec::new();
    let mut lookup = BTreeMap::new();
    for &state in states {
        if let std::collections::btree_map::Entry::Vacant(entry) = lookup.entry(state) {
            entry.insert(palette.len() as u32);
            palette.push(state);
        }
    }
    if palette.len() == 1 {
        out.push(0);
        varint(palette[0], out);
        varint(0, out);
        return;
    }
    let required = (usize::BITS - (palette.len() - 1).leading_zeros()) as usize;
    let indirect = palette.len() <= 256;
    let bits = if indirect { required.max(4) } else { 15 };
    out.push(bits as u8);
    if indirect {
        varint(palette.len() as u32, out);
        for &state in &palette {
            varint(state, out);
        }
    }
    let words = pack(
        states
            .iter()
            .map(|state| if indirect { lookup[state] } else { *state }),
        bits,
    );
    varint(words.len() as u32, out);
    for word in words {
        out.extend_from_slice(&word.to_be_bytes());
    }
}

fn encode_heightmaps(heights: &[u16; 256], out: &mut Vec<u8>) {
    let words = pack(heights.iter().copied().map(u32::from), 9);
    out.push(10); // anonymous compound: no root name is encoded
    for name in ["MOTION_BLOCKING", "WORLD_SURFACE"] {
        out.push(12); // named long array
        out.extend_from_slice(&(name.len() as u16).to_be_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(&(words.len() as i32).to_be_bytes());
        for &word in &words {
            out.extend_from_slice(&word.to_be_bytes());
        }
    }
    out.push(0); // compound END
}

/// A packed network Position, preserving signed X/Z (26 bits) and Y (12 bits).
pub fn packed_position(x: i32, y: i32, z: i32) -> Result<i64> {
    if !(MIN_WIRE_COORDINATE..=MAX_WIRE_COORDINATE).contains(&x)
        || !(MIN_WIRE_COORDINATE..=MAX_WIRE_COORDINATE).contains(&z)
        || !(-2048..=2047).contains(&y)
    {
        bail!("position is outside protocol 767's packed coordinate range");
    }
    Ok(((i64::from(x) & 0x3ff_ffff) << 38)
        | ((i64::from(z) & 0x3ff_ffff) << 12)
        | (i64::from(y) & 0xfff))
}

pub fn try_block_update_payload(x: i32, y: i32, z: i32, state: u32) -> Result<Vec<u8>> {
    if state > MAX_BLOCK_STATE {
        bail!("block state is outside protocol 767's global palette");
    }
    let mut out = packed_position(x, y, z)?.to_be_bytes().to_vec();
    varint(state, &mut out);
    Ok(out)
}

/// Clientbound Block Update payload (packet 0x09), excluding ID/framing.
/// Call `try_block_update_payload` when the coordinates have not been validated.
pub fn block_update_payload(x: i32, y: i32, z: i32, state: u32) -> Vec<u8> {
    try_block_update_payload(x, y, z, state).expect("validated block update")
}

#[cfg(test)]
mod tests {
    #[test]
    fn biome_seed_hashes_match_original_java_biome_manager() {
        let vectors: Vec<[i64; 2]> =
            serde_json::from_str(include_str!("../fixtures/biome-seed-vectors.json")).unwrap();
        assert_eq!(vectors.len(), 1032);
        for [seed, expected] in vectors {
            assert_eq!(super::obfuscate_biome_seed(seed), expected, "seed {seed}");
        }
    }
    use super::*;

    fn world() -> FlatWorld {
        FlatWorld::new(0, 1, 10, 9, 1)
    }

    #[test]
    fn fixture_layers_and_vertical_bounds_are_explicit() {
        let w = world();
        assert_eq!(w.block(0, -65, 0), 0);
        assert_eq!(w.block(0, -64, 0), 1);
        assert_eq!(w.block(0, 60, 0), 1);
        assert_eq!(w.block(0, 61, 0), 10);
        assert_eq!(w.block(0, 62, 0), 10);
        assert_eq!(w.block(0, 63, 0), 9);
        assert_eq!(w.block(0, 64, 0), 0);
        assert_eq!(w.block(0, 320, 0), 0);
    }

    #[test]
    fn negative_chunks_do_not_alias_and_restoring_removes_the_diff() {
        let mut w = world();
        assert!(w.set_block(-1, 63, -17, 0));
        assert_eq!(w.block(-1, 63, -17), 0);
        assert_eq!(w.block(15, 63, -17), 9);
        assert_eq!(w.block(-1, 63, -1), 9);
        assert_eq!(
            w.changes(),
            vec![BlockChange {
                x: -1,
                y: 63,
                z: -17,
                state: 0
            }]
        );
        assert!(!w.set_block(-1, 63, -17, 0));
        assert!(w.set_block(-1, 63, -17, 9));
        assert!(w.changes().is_empty());
    }

    #[test]
    fn persistence_validation_is_atomic() {
        let mut w = world();
        let changes = [
            BlockChange {
                x: 1,
                y: 63,
                z: 2,
                state: 0,
            },
            BlockChange {
                x: 1,
                y: 320,
                z: 2,
                state: 1,
            },
        ];
        assert!(w.apply_changes(&changes).is_err());
        assert_eq!(w.block(1, 63, 2), 9);
        let valid = [BlockChange {
            x: -17,
            y: 80,
            z: -1,
            state: 10,
        }];
        assert_eq!(w.apply_changes(&valid).unwrap(), 1);
        let json = serde_json::to_string(&w.changes()).unwrap();
        let restored: Vec<BlockChange> = serde_json::from_str(&json).unwrap();
        let mut other = world();
        other.apply_changes(&restored).unwrap();
        assert_eq!(other.block(-17, 80, -1), 10);
    }

    #[test]
    fn heightmap_tracks_removed_top_and_added_blocks_at_signed_coordinates() {
        let mut w = world();
        assert_eq!(w.height_values(None), [128; 256]);
        w.set_block(-1, 63, -1, 0);
        w.set_block(-2, 100, -1, 1);
        let heights = w.height_values(w.changes.get(&(-1, -1)));
        assert_eq!(heights[255], 127);
        assert_eq!(heights[254], 165);
        assert_eq!(heights[0], 128);
    }

    #[test]
    fn nine_bit_storage_pads_instead_of_crossing_long_boundaries() {
        let words = pack([1, 2, 3, 4, 5, 6, 7, 8], 9);
        assert_eq!(words.len(), 2);
        assert_eq!(
            words[0],
            1 | (2 << 9) | (3 << 18) | (4 << 27) | (5 << 36) | (6 << 45) | (7 << 54)
        );
        assert_eq!(words[1], 8);
        assert_eq!(pack([128; 256], 9).len(), 37);
    }

    #[test]
    fn uniform_palette_has_no_palette_array_or_data_longs() {
        let mut encoded = Vec::new();
        encode_block_palette(&vec![300; 4096], &mut encoded);
        assert_eq!(encoded, [0, 0xac, 0x02, 0]);
    }

    #[test]
    fn direct_palette_is_used_above_256_distinct_states() {
        let states = (0..4096).map(|i| i % 257).collect::<Vec<u32>>();
        let mut encoded = Vec::new();
        encode_block_palette(&states, &mut encoded);
        assert_eq!(encoded[0], 15);
        // ceil(4096 / floor(64 / 15)) = 1024 longs, VarInt 0x80 0x08.
        assert_eq!(&encoded[1..3], &[0x80, 0x08]);
        assert_eq!(encoded.len(), 3 + 1024 * 8);
        assert_eq!(
            u64::from_be_bytes(encoded[3..11].try_into().unwrap()),
            (1 << 15) | (2 << 30) | (3 << 45)
        );
    }

    #[test]
    fn packed_position_round_trips_signed_boundaries() {
        for &(x, y, z) in &[
            (-1, -64, -17),
            (0, 63, 0),
            (MAX_WIRE_COORDINATE, 2047, MIN_WIRE_COORDINATE),
        ] {
            let packed = packed_position(x, y, z).unwrap();
            assert_eq!((packed >> 38) as i32, x);
            assert_eq!((packed << 26 >> 38) as i32, z);
            assert_eq!((packed << 52 >> 52) as i32, y);
        }
        assert!(packed_position(MAX_WIRE_COORDINATE + 1, 0, 0).is_err());
        assert!(packed_position(0, 2048, 0).is_err());
        assert_eq!(
            block_update_payload(-1, -1, -1, 0),
            [255, 255, 255, 255, 255, 255, 255, 255, 0]
        );
    }

    #[test]
    fn chunk_coordinates_and_anonymous_heightmap_header_are_wire_values() {
        let packet = world().chunk_packet(-2, 3);
        assert_eq!(&packet[..4], &(-2_i32).to_be_bytes());
        assert_eq!(&packet[4..8], &3_i32.to_be_bytes());
        assert_eq!(&packet[8..12], &[10, 12, 0, 15]);
        assert_eq!(&packet[12..27], b"MOTION_BLOCKING");
        assert_eq!(&packet[27..31], &37_i32.to_be_bytes());
    }
}
