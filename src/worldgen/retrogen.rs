//! Original BelowZeroRetrogen payload and operations on the actual live target.
//! No helper publishes readiness, advances status, resets random state or
//! substitutes a missing target/registry owner.
use super::target::{GenerationTargetHost, HeightRange};
pub use crate::chunk_readiness::ChunkStatus;
use crate::{density::DensityPoint, nbt::Tag};
use anyhow::{anyhow, bail, ensure, Context, Result};
use std::sync::Arc;

pub const UPGRADE_MIN_Y: i32 = -64;
pub const UPGRADE_HEIGHT: i32 = 64;

/// Same source chunk-status identity shared with the readiness owner. This
/// parser reads payload data; it does not make that status successful.
pub fn parse_status(name: &str) -> Result<ChunkStatus> {
    let canonical = match name.split_once(':') {
        Some(("", path)) => format!("minecraft:{path}"),
        Some(_) => name.to_owned(),
        None => format!("minecraft:{name}"),
    };
    (0..12)
        .filter_map(ChunkStatus::from_index)
        .find(|status| status.name() == canonical)
        .ok_or_else(|| anyhow!("unknown original chunk status {name}"))
}

/// Immutable actual payload. Java BitSet retains every nonzero high word;
/// hasBedrockHoles observes those words even though column reads use 256 bits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BelowZeroRetrogen {
    target_status: ChunkStatus,
    missing_bedrock: Vec<u64>,
}
impl BelowZeroRetrogen {
    pub fn new(target_status: ChunkStatus, mut missing_bedrock: Vec<u64>) -> Result<Self> {
        ensure!(
            target_status != ChunkStatus::EMPTY,
            "target_status cannot be empty"
        );
        while missing_bedrock.last() == Some(&0) {
            missing_bedrock.pop();
        }
        Ok(Self {
            target_status,
            missing_bedrock,
        })
    }
    pub fn target_status(&self) -> ChunkStatus {
        self.target_status
    }
    pub fn bedrock_words(&self) -> &[u64] {
        &self.missing_bedrock
    }
    pub fn has_bedrock_holes(&self) -> bool {
        !self.missing_bedrock.is_empty()
    }
    pub fn has_bedrock_hole(&self, x: i32, z: i32) -> bool {
        let bit = ((z & 15) * 16 + (x & 15)) as usize;
        self.missing_bedrock
            .get(bit >> 6)
            .is_some_and(|word| word & (1_u64 << (bit & 63)) != 0)
    }
    pub const fn generation_height(&self) -> HeightRange {
        HeightRange {
            min_y: UPGRADE_MIN_Y,
            height: UPGRADE_HEIGHT,
        }
    }

    /// Original CODEC under NbtOps: required nonempty target, lenient optional
    /// LONG_STREAM mask. A malformed optional mask is absent, as in the source.
    pub fn decode_codec(tag: &Tag) -> Result<Self> {
        ensure!(
            matches!(tag, Tag::Compound(_)),
            "retrogen codec requires a compound"
        );
        let status = target_name(tag)?;
        let words = tag
            .get("missing_bedrock")
            .and_then(codec_words)
            .unwrap_or_default();
        Self::new(parse_status(status)?, words)
    }

    /// Original direct CompoundTag reader differs from CODEC: EMPTY means no
    /// retrogen, and getLongArray ignores a mask with another tag type. Invalid
    /// or unknown target identities are unavailable supported data, not EMPTY.
    pub fn read_nbt(tag: &Tag) -> Result<Option<Self>> {
        ensure!(
            matches!(tag, Tag::Compound(_)),
            "retrogen direct read requires a compound"
        );
        let status = parse_status(target_name(tag)?)?;
        if status == ChunkStatus::EMPTY {
            return Ok(None);
        }
        let words = match tag.get("missing_bedrock") {
            Some(Tag::LongArray(values)) => values.iter().map(|value| *value as u64).collect(),
            _ => Vec::new(),
        };
        Ok(Some(Self::new(status, words)?))
    }

    /// Source CODEC omits empty masks and emits BitSet.toLongArray words.
    pub fn encode_codec(&self) -> Tag {
        let mut entries = vec![(
            "target_status".into(),
            Tag::String(self.target_status.name().into()),
        )];
        if self.has_bedrock_holes() {
            entries.push((
                "missing_bedrock".into(),
                Tag::LongArray(
                    self.missing_bedrock
                        .iter()
                        .map(|word| *word as i64)
                        .collect(),
                ),
            ));
        }
        Tag::Compound(entries)
    }
}

fn target_name(tag: &Tag) -> Result<&str> {
    match tag.get("target_status") {
        Some(Tag::String(value)) => value
            .as_str()
            .context("retrogen target_status is not a resource identifier"),
        _ => bail!("retrogen target_status is unavailable or not a string"),
    }
}
fn numeric_word(tag: &Tag) -> Option<u64> {
    Some(match tag {
        Tag::Byte(v) => i64::from(*v) as u64,
        Tag::Short(v) => i64::from(*v) as u64,
        Tag::Int(v) => i64::from(*v) as u64,
        Tag::Long(v) => *v as u64,
        Tag::Float(v) => (*v as i64) as u64,
        Tag::Double(v) => (*v as i64) as u64,
        _ => return None,
    })
}
fn codec_words(tag: &Tag) -> Option<Vec<u64>> {
    match tag {
        Tag::LongArray(values) => Some(values.iter().map(|v| *v as u64).collect()),
        Tag::IntArray(values) => Some(values.iter().map(|v| i64::from(*v) as u64).collect()),
        Tag::ByteArray(values) => Some(values.iter().map(|v| i64::from(*v) as u64).collect()),
        Tag::List {
            values,
            element_type,
        } => values
            .iter()
            .map(|value| {
                let value = if *element_type == 10 {
                    match value {
                        Tag::Compound(entries) if entries.len() == 1 && entries[0].0 == "" => {
                            &entries[0].1
                        }
                        _ => value,
                    }
                } else {
                    value
                };
                numeric_word(value)
            })
            .collect(),
        _ => None,
    }
}

/// The actual slot observation has no Default. Unavailable is never an absent
/// slot. The same Arc survives until its owner's real persisted-status update.
#[derive(Debug, Clone)]
pub enum RetrogenSlot {
    ObservedAbsent,
    Present(Arc<BelowZeroRetrogen>),
    Unavailable,
}
impl RetrogenSlot {
    pub fn observe(&self) -> Result<Option<Arc<BelowZeroRetrogen>>> {
        match self {
            Self::ObservedAbsent => Ok(None),
            Self::Present(value) => Ok(Some(Arc::clone(value))),
            Self::Unavailable => bail!("actual target retrogen owner is unavailable"),
        }
    }
    /// Call only after the owner actually applies setPersistedStatus. This
    /// clears the slot by the source comparison; it advances no status itself.
    pub fn persisted_status_applied(&mut self, status: ChunkStatus) -> Result<bool> {
        let clear = match self {
            Self::ObservedAbsent => false,
            Self::Present(value) => status >= value.target_status(),
            Self::Unavailable => bail!("actual target retrogen owner is unavailable"),
        };
        if clear {
            *self = Self::ObservedAbsent;
        }
        Ok(clear)
    }
}

/// BEDROCK block identity in the actual canonical state catalog, including an
/// error for an unavailable state. Source positions are LOCAL chunk X/Z.
pub fn replace_old_bedrock<T: GenerationTargetHost + ?Sized>(target: &mut T) -> Result<()> {
    let deepslate = target.default_states().deepslate;
    for z in 0..16 {
        for y in 0..=4 {
            for x in 0..16 {
                let point = DensityPoint::new(x, y, z);
                let state = target.block(point)?;
                let name = crate::inventory::block_state_name(state)
                    .context("retrogen read an unknown canonical block state")?;
                if name == "bedrock" {
                    target.proto_set_block(point, deepslate)?;
                }
            }
        }
    }
    Ok(())
}

/// Source X outer/Z inner/ascending Y. Generation height is queried from the
/// CURRENT owner, rather than assumed from this detached payload's presence.
pub fn apply_bedrock_mask<T: GenerationTargetHost + ?Sized>(
    target: &mut T,
    retrogen: &BelowZeroRetrogen,
) -> Result<()> {
    let height = target.generation_height()?;
    let max_y = height.max_y()?;
    let air = target.default_states().air;
    for x in 0..16 {
        for z in 0..16 {
            if retrogen.has_bedrock_hole(x, z) {
                for y in height.min_y..max_y {
                    target.proto_set_block(DensityPoint::new(x, y, z), air)?;
                }
            }
        }
    }
    Ok(())
}

/// ChunkStatusTasks.generateNoise.thenApply, after successful fillFromNoise and
/// before outer stage completion. No queue cleanup or slot/status advancement.
pub fn finish_noise_task<T: GenerationTargetHost + ?Sized>(target: &mut T) -> Result<()> {
    if let Some(retrogen) = target.retrogen()? {
        replace_old_bedrock(target)?;
        if retrogen.has_bedrock_holes() {
            apply_bedrock_mask(target, &retrogen)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitset_high_words_and_status_clear_preserve_source_owner() {
        let status = ChunkStatus::from_index(4).unwrap();
        let payload = Arc::new(BelowZeroRetrogen::new(status, vec![1, 0, 0, 0, 8, 0]).unwrap());
        assert_eq!(payload.bedrock_words(), &[1, 0, 0, 0, 8]);
        assert!(payload.has_bedrock_holes());
        assert!(payload.has_bedrock_hole(16, -16));
        assert!(!payload.has_bedrock_hole(15, 15));
        let mut slot = RetrogenSlot::Present(Arc::clone(&payload));
        assert!(!slot
            .persisted_status_applied(ChunkStatus::from_index(3).unwrap())
            .unwrap());
        assert!(Arc::ptr_eq(&slot.observe().unwrap().unwrap(), &payload));
        assert!(slot.persisted_status_applied(status).unwrap());
        assert!(slot.observe().unwrap().is_none());
        assert!(RetrogenSlot::Unavailable.observe().is_err());
    }

    #[test]
    fn codec_and_direct_reader_have_distinct_mask_and_empty_rules() {
        let tag = Tag::Compound(vec![
            ("target_status".into(), Tag::String("noise".into())),
            ("missing_bedrock".into(), Tag::IntArray(vec![1])),
        ]);
        assert!(BelowZeroRetrogen::decode_codec(&tag)
            .unwrap()
            .has_bedrock_holes());
        assert!(!BelowZeroRetrogen::read_nbt(&tag)
            .unwrap()
            .unwrap()
            .has_bedrock_holes());
        let empty = Tag::Compound(vec![(
            "target_status".into(),
            Tag::String("minecraft:empty".into()),
        )]);
        assert!(BelowZeroRetrogen::decode_codec(&empty).is_err());
        assert!(BelowZeroRetrogen::read_nbt(&empty).unwrap().is_none());
    }
}
