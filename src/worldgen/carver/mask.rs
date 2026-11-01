//! Source CarvingMask storage, additional filtering and packed-coordinate order.
use crate::density::DensityPoint;
use anyhow::{bail, Context, Result};

/// Actual additional mask supplied by the generation dependency owner. Its
/// query runs before the stored bit, even when that bit is already present.
pub type AdditionalCarvingMask = Box<dyn Fn(i32, i32, i32) -> Result<bool> + Send + Sync + 'static>;

/// One actual ProtoChunk/GenerationStep.Carving mask. It is not a collection of
/// successful writes: failed replacement attempts are marked by the caller.
/// Original BitSet capacity is an allocation hint, not a clipping height.
pub struct CarvingMask {
    min_y: i32,
    words: Vec<u64>,
    additional: AdditionalCarvingMask,
}
impl CarvingMask {
    /// Source fresh-mask constructor. Loaded masks use `from_array`; creating
    /// an empty mask is not a substitute for an unavailable existing owner.
    pub fn new(height: i32, min_y: i32) -> Result<Self> {
        let capacity = height
            .checked_mul(256)
            .context("carving mask capacity overflows")?;
        if capacity < 0 {
            bail!("negative carving mask capacity");
        }
        Ok(Self {
            min_y,
            words: vec![0; (capacity as usize).div_ceil(64)],
            additional: Box::new(|_, _, _| Ok(false)),
        })
    }

    /// BitSet.valueOf(long[]) copies only through its last nonzero word. The
    /// additional predicate is independent of these persisted bits.
    pub fn from_array(words: &[i64], min_y: i32) -> Self {
        let count = words
            .iter()
            .rposition(|word| *word != 0)
            .map_or(0, |index| index + 1);
        Self {
            min_y,
            words: words[..count].iter().map(|word| *word as u64).collect(),
            additional: Box::new(|_, _, _| Ok(false)),
        }
    }
    pub fn min_y(&self) -> i32 {
        self.min_y
    }
    pub fn set_additional_mask(&mut self, filter: AdditionalCarvingMask) {
        self.additional = filter;
    }
    fn index(&self, x: i32, y: i32, z: i32) -> Result<usize> {
        let index = (x & 15) | ((z & 15) << 4) | y.wrapping_sub(self.min_y).wrapping_shl(8);
        if index < 0 {
            bail!("negative carving mask bit index {index}");
        }
        Ok(index as usize)
    }
    pub fn get(&self, x: i32, y: i32, z: i32) -> Result<bool> {
        // Keep the short-circuit order: an additional true result can even
        // bypass an otherwise invalid negative BitSet index, as in the source.
        if (self.additional)(x, y, z)? {
            return Ok(true);
        }
        let index = self.index(x, y, z)?;
        Ok(self
            .words
            .get(index >> 6)
            .is_some_and(|word| word & (1_u64 << (index & 63)) != 0))
    }
    pub fn set(&mut self, x: i32, y: i32, z: i32) -> Result<()> {
        let index = self.index(x, y, z)?;
        let word = index >> 6;
        if word >= self.words.len() {
            self.words.resize(word + 1, 0);
        }
        self.words[word] |= 1_u64 << (index & 63);
        Ok(())
    }
    pub fn to_array(&self) -> Vec<i64> {
        let count = self
            .words
            .iter()
            .rposition(|word| *word != 0)
            .map_or(0, |index| index + 1);
        self.words[..count]
            .iter()
            .map(|word| *word as i64)
            .collect()
    }
    /// Original BitSet.stream order. The additional filter is neither streamed
    /// nor serialized. X/Z use the supplied chunk; mask Y is absolute minY+index.
    pub fn stream(&self, chunk: (i32, i32)) -> Vec<DensityPoint> {
        let mut points = Vec::new();
        for (word_index, word) in self.words.iter().copied().enumerate() {
            let mut word = word;
            while word != 0 {
                let bit = word.trailing_zeros() as usize;
                let index = (word_index * 64 + bit) as i32;
                points.push(DensityPoint::new(
                    chunk.0.wrapping_shl(4).wrapping_add(index & 15),
                    self.min_y.wrapping_add(index >> 8),
                    chunk.1.wrapping_shl(4).wrapping_add((index >> 4) & 15),
                ));
                word &= word - 1;
            }
        }
        points
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    #[test]
    fn additional_filter_precedes_stored_bits_and_does_not_change_persistence() -> Result<()> {
        let mut mask = CarvingMask::new(16, -64)?;
        mask.set(3, -62, 5)?;
        let calls = Arc::new(AtomicUsize::new(0));
        let witness = Arc::clone(&calls);
        mask.set_additional_mask(Box::new(move |_, _, _| {
            witness.fetch_add(1, Ordering::Relaxed);
            Ok(true)
        }));
        assert!(mask.get(3, -62, 5)?);
        assert!(mask.get(0, -65, 0)?); // additional true precedes negative index.
        assert_eq!(calls.load(Ordering::Relaxed), 2);
        let stored = mask.to_array();
        assert_eq!(stored.len(), 10);
        assert_eq!(stored[9], 1_i64 << 19); // 3 | (5<<4) | (2<<8) = 595.
        assert_eq!(mask.stream((-2, 3)), [DensityPoint::new(-29, -62, 53)]);
        let restored = CarvingMask::from_array(&[stored.as_slice(), &[0, 0]].concat(), -64);
        assert_eq!(restored.to_array(), stored);
        assert!(!restored.get(0, -64, 0)?);
        assert!(restored.get(3, -62, 5)?);
        Ok(())
    }
    #[test]
    fn stored_bit_packing_aliases_xz_and_streams_in_ascending_index_order() -> Result<()> {
        let mut mask = CarvingMask::new(0, 10)?;
        mask.set(15, 11, 15)?;
        mask.set(16, 10, -16)?;
        assert!(mask.get(0, 10, 0)?);
        assert_eq!(
            mask.stream((1, -1)),
            [
                DensityPoint::new(16, 10, -16),
                DensityPoint::new(31, 11, -1)
            ]
        );
        assert!(mask.get(0, 9, 0).is_err());
        Ok(())
    }
}
