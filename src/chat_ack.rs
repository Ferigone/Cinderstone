//! Connection-owned Java 1.21 chat acknowledgement and signature-cache state.
//!
//! Source: LastSeenMessagesValidator (`xi`), MessageSignatureCache (`xm`),
//! MessageSignature.Packed (`xl$a`) and LastSeenMessages (`xg`). The listener
//! supplies only signatures actually delivered to this recipient. These owners
//! are not shared between clients or persisted as world/player data.

use std::{
    collections::{HashSet, VecDeque},
    error::Error,
    fmt,
};

pub const MESSAGE_SIGNATURE_BYTES: usize = 256;
pub const LAST_SEEN_COUNT: usize = 20;
pub const DEFAULT_SIGNATURE_CACHE_CAPACITY: usize = 128;

/// MessageSignature's constructor requires exactly 256 bytes. Equality and
/// cache identity compare the bytes, not an allocation identity or base64 text.
pub type MessageSignature = [u8; MESSAGE_SIGNATURE_BYTES];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LastSeenUpdate {
    pub offset: i32,
    /// Java BitSet's little-endian fixed 20-bit representation occupies three
    /// bytes. The four spare high bits remain observable invalid bits; callers
    /// must not mask them off before validation.
    pub acknowledged: [u8; 3],
}

impl LastSeenUpdate {
    pub const fn empty() -> Self {
        Self {
            offset: 0,
            acknowledged: [0; 3],
        }
    }

    pub fn is_acknowledged(&self, index: usize) -> bool {
        self.acknowledged
            .get(index / 8)
            .is_some_and(|byte| byte & (1 << (index % 8)) != 0)
    }

    fn bit_length(&self) -> usize {
        self.acknowledged
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, byte)| {
                (*byte != 0).then_some(index * 8 + (8 - byte.leading_zeros() as usize))
            })
            .unwrap_or(0)
    }
}

impl Default for LastSeenUpdate {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AckValidationError {
    InvalidOffset {
        offset: i32,
        available: usize,
    },
    AcknowledgedOutOfRange {
        bit_length: usize,
        last_seen_count: usize,
    },
    AcknowledgedMissing {
        index: usize,
    },
    PreviouslyAcknowledgedOmitted {
        index: usize,
    },
}

impl fmt::Display for AckValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidOffset { offset, available } => write!(
                f,
                "invalid last-seen offset {offset}; available {available}"
            ),
            Self::AcknowledgedOutOfRange {
                bit_length,
                last_seen_count,
            } => write!(
                f,
                "last-seen acknowledgement bit length {bit_length} exceeds {last_seen_count}"
            ),
            Self::AcknowledgedMissing { index } => {
                write!(f, "last-seen acknowledgement selects empty slot {index}")
            }
            Self::PreviouslyAcknowledgedOmitted { index } => write!(
                f,
                "last-seen acknowledgement omits acknowledged slot {index}"
            ),
        }
    }
}
impl Error for AckValidationError {}

#[derive(Clone, Copy)]
struct LastSeenTrackedEntry {
    signature: MessageSignature,
    pending: bool,
}

/// The server listener's actual ordered acknowledgement window.
#[derive(Clone)]
pub struct LastSeenMessagesValidator {
    last_seen_count: usize,
    tracked: Vec<Option<LastSeenTrackedEntry>>,
    last_pending_message: Option<MessageSignature>,
}

impl fmt::Debug for LastSeenMessagesValidator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LastSeenMessagesValidator")
            .field("last_seen_count", &self.last_seen_count)
            .field("tracked_messages_count", &self.tracked.len())
            .finish_non_exhaustive()
    }
}

impl Default for LastSeenMessagesValidator {
    fn default() -> Self {
        Self::new(LAST_SEEN_COUNT)
    }
}

impl LastSeenMessagesValidator {
    /// Java constructs the list with `last_seen_count` empty slots; its reported
    /// count therefore starts at 20 for the standard listener, not zero.
    pub fn new(last_seen_count: usize) -> Self {
        Self {
            last_seen_count,
            tracked: vec![None; last_seen_count],
            last_pending_message: None,
        }
    }

    /// Returns whether a new pending entry was appended. Java suppresses only
    /// consecutive byte-equal signatures, even after the older entry is removed.
    pub fn add_pending(&mut self, signature: MessageSignature) -> bool {
        if self.last_pending_message.as_ref() == Some(&signature) {
            return false;
        }
        self.tracked.push(Some(LastSeenTrackedEntry {
            signature,
            pending: true,
        }));
        self.last_pending_message = Some(signature);
        true
    }

    pub fn tracked_messages_count(&self) -> usize {
        self.tracked.len()
    }

    pub fn last_seen_count(&self) -> usize {
        self.last_seen_count
    }

    /// The offset may remove only entries beyond the retained window. It does
    /// not clear `last_pending_message` or acknowledge any surviving entry.
    pub fn apply_offset(&mut self, offset: i32) -> bool {
        let available = self.tracked.len() - self.last_seen_count;
        if offset < 0 || offset as usize > available {
            return false;
        }
        self.tracked.drain(..offset as usize);
        true
    }

    /// Apply in source order, returning the byte-identical ordered last-seen
    /// list used in the signed message body. Failure is intentionally not a
    /// rollback: Java has already applied the offset and may have changed
    /// earlier slots before a later invalid bit rejects the update.
    pub fn apply_update(
        &mut self,
        update: &LastSeenUpdate,
    ) -> Result<Vec<MessageSignature>, AckValidationError> {
        let available = self.tracked.len() - self.last_seen_count;
        if !self.apply_offset(update.offset) {
            return Err(AckValidationError::InvalidOffset {
                offset: update.offset,
                available,
            });
        }
        let bit_length = update.bit_length();
        if bit_length > self.last_seen_count {
            return Err(AckValidationError::AcknowledgedOutOfRange {
                bit_length,
                last_seen_count: self.last_seen_count,
            });
        }
        let mut acknowledged = Vec::new();
        for index in 0..self.last_seen_count {
            let entry = self.tracked[index];
            if update.is_acknowledged(index) {
                let Some(mut entry) = entry else {
                    return Err(AckValidationError::AcknowledgedMissing { index });
                };
                entry.pending = false;
                self.tracked[index] = Some(entry);
                acknowledged.push(entry.signature);
            } else {
                if entry.is_some_and(|entry| !entry.pending) {
                    return Err(AckValidationError::PreviouslyAcknowledgedOmitted { index });
                }
                self.tracked[index] = None;
            }
        }
        Ok(acknowledged)
    }
}

/// The source packed form writes `0` plus 256 bytes for Full, or `id + 1` for
/// Cached. The wire codec owns reading/writing these fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PackedMessageSignature {
    Full(MessageSignature),
    Cached(i32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheError {
    InvalidReference { id: i32, capacity: usize },
}
impl fmt::Display for CacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidReference { id, capacity } => write!(
                f,
                "chat signature cache reference {id} is outside capacity {capacity}"
            ),
        }
    }
}
impl Error for CacheError {}

/// One recipient/listener cache. Pack a message using the old state and advance
/// it only through the corresponding actual delivery's last-seen/signature data.
#[derive(Clone)]
pub struct MessageSignatureCache {
    entries: Vec<Option<MessageSignature>>,
}

impl fmt::Debug for MessageSignatureCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MessageSignatureCache")
            .field("capacity", &self.entries.len())
            .field(
                "occupied",
                &self.entries.iter().filter(|entry| entry.is_some()).count(),
            )
            .finish()
    }
}

impl Default for MessageSignatureCache {
    fn default() -> Self {
        Self::new(DEFAULT_SIGNATURE_CACHE_CAPACITY)
    }
}

impl MessageSignatureCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: vec![None; capacity],
        }
    }

    pub fn capacity(&self) -> usize {
        self.entries.len()
    }

    /// Source linear search returns the first byte-equal signature or -1.
    pub fn index_of(&self, signature: &MessageSignature) -> i32 {
        self.entries
            .iter()
            .position(|entry| entry.as_ref() == Some(signature))
            .and_then(|index| i32::try_from(index).ok())
            .unwrap_or(-1)
    }

    pub fn pack(&self, signature: &MessageSignature) -> PackedMessageSignature {
        match self.index_of(signature) {
            -1 => PackedMessageSignature::Full(*signature),
            id => PackedMessageSignature::Cached(id),
        }
    }

    /// A valid empty slot is the source Optional.empty outcome. An out-of-range
    /// reference is an explicit source array-bounds exception boundary, not an
    /// invented empty signature, missing-cache fallback or process panic.
    pub fn lookup(&self, id: i32) -> Result<Option<MessageSignature>, CacheError> {
        if id < 0 || id as usize >= self.entries.len() {
            return Err(CacheError::InvalidReference {
                id,
                capacity: self.entries.len(),
            });
        }
        Ok(self.entries[id as usize])
    }

    pub fn unpack(
        &self,
        packed: &PackedMessageSignature,
    ) -> Result<Option<MessageSignature>, CacheError> {
        match packed {
            PackedMessageSignature::Full(signature) => Ok(Some(*signature)),
            PackedMessageSignature::Cached(id) => self.lookup(*id),
        }
    }

    pub fn pack_last_seen(&self, signatures: &[MessageSignature]) -> Vec<PackedMessageSignature> {
        signatures
            .iter()
            .map(|signature| self.pack(signature))
            .collect()
    }

    pub fn unpack_last_seen(
        &self,
        packed: &[PackedMessageSignature],
    ) -> Result<Option<Vec<MessageSignature>>, CacheError> {
        let mut signatures = Vec::with_capacity(packed.len());
        for packed in packed {
            let Some(signature) = self.unpack(packed)? else {
                return Ok(None);
            };
            signatures.push(signature);
        }
        Ok(Some(signatures))
    }

    /// Source push(SignedMessageBody, nullable MessageSignature): ordered
    /// last-seen entries followed by this message's signature form the deque.
    /// Duplicates in that input are retained. A fixed initial set controls
    /// whether each displaced old slot goes to the *front* of the deque.
    pub fn push(&mut self, last_seen: &[MessageSignature], signature: Option<&MessageSignature>) {
        let mut queue = VecDeque::with_capacity(last_seen.len() + usize::from(signature.is_some()));
        queue.extend(last_seen.iter().copied());
        if let Some(signature) = signature {
            queue.push_back(*signature);
        }
        self.push_queue(queue);
    }

    /// The original package-level push(List) path uses the same deque owner.
    pub fn push_signatures(&mut self, signatures: &[MessageSignature]) {
        self.push_queue(signatures.iter().copied().collect());
    }

    fn push_queue(&mut self, mut queue: VecDeque<MessageSignature>) {
        let incoming = queue.iter().copied().collect::<HashSet<_>>();
        for entry in &mut self.entries {
            let Some(next) = queue.pop_back() else {
                break;
            };
            let previous = entry.replace(next);
            if let Some(previous) = previous {
                if !incoming.contains(&previous) {
                    queue.push_front(previous);
                }
            }
        }
    }
}

/// LastSeenMessages.updateSignature: signed big-endian i32 entry count, then
/// the ordered raw 256-byte signatures. This is not their packed wire form.
pub fn last_seen_signature_bytes(entries: &[MessageSignature]) -> anyhow::Result<Vec<u8>> {
    let count = i32::try_from(entries.len())?;
    let mut output = Vec::new();
    output.extend_from_slice(&count.to_be_bytes());
    for entry in entries {
        output.extend_from_slice(entry);
    }
    Ok(output)
}
