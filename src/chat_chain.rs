//! Java 1.21 signed-message links, signature bodies and connection-owned chains.
//!
//! Trusted profile certificates and RSA verification belong to `chat_crypto`;
//! recipient-specific acknowledgments and signature caches belong to `chat_ack`.
//! A decode failure is a chat error component, not by itself a disconnect.

use crate::{chat_ack::MessageSignature, chat_crypto::ValidatedProfileKey};
use serde_json::{json, Value};
use std::fmt;

pub const MESSAGE_EXPIRES_AFTER_SERVER_MS: i64 = 5 * 60 * 1000;
pub const MESSAGE_EXPIRES_AFTER_CLIENT_MS: i64 = 7 * 60 * 1000;
pub const SYSTEM_SENDER: [u8; 16] = [0; 16];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignedMessageLink {
    pub index: i32,
    pub sender: [u8; 16],
    pub session_id: [u8; 16],
}

impl SignedMessageLink {
    pub const fn root(sender: [u8; 16], session_id: [u8; 16]) -> Self {
        Self {
            index: 0,
            sender,
            session_id,
        }
    }

    /// Source unsigned links are root links with the nil session UUID, not -1.
    pub const fn unsigned(sender: [u8; 16]) -> Self {
        Self::root(sender, [0; 16])
    }

    pub fn advance(self) -> Option<Self> {
        (self.index != i32::MAX).then(|| Self {
            index: self.index + 1,
            ..self
        })
    }

    pub fn is_descendant_of(&self, ancestor: &Self) -> bool {
        self.index > ancestor.index
            && self.sender == ancestor.sender
            && self.session_id == ancestor.session_id
    }

    /// The source streams UUID bytes before the four-byte big-endian index.
    pub fn signature_bytes(&self) -> [u8; 36] {
        let mut bytes = [0; 36];
        bytes[..16].copy_from_slice(&self.sender);
        bytes[16..32].copy_from_slice(&self.session_id);
        bytes[32..].copy_from_slice(&self.index.to_be_bytes());
        bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedMessageBody {
    pub content: String,
    /// FriendlyByteBuf.readInstant reads epoch milliseconds. Signature hashing
    /// deliberately uses floor-divided epoch seconds instead of these millis.
    pub timestamp_millis: i64,
    pub salt: i64,
    pub last_seen: Vec<MessageSignature>,
}

impl SignedMessageBody {
    pub fn unsigned(content: impl Into<String>, now_ms: i64) -> Self {
        Self {
            content: content.into(),
            timestamp_millis: now_ms,
            salt: 0,
            last_seen: Vec::new(),
        }
    }

    pub fn signature_bytes(&self) -> Vec<u8> {
        let text = self.content.as_bytes();
        let mut bytes = Vec::with_capacity(24 + text.len() + self.last_seen.len() * 256);
        bytes.extend_from_slice(&self.salt.to_be_bytes());
        bytes.extend_from_slice(&self.timestamp_millis.div_euclid(1000).to_be_bytes());
        bytes.extend_from_slice(&(text.len() as i32).to_be_bytes());
        bytes.extend_from_slice(text);
        bytes.extend_from_slice(&(self.last_seen.len() as i32).to_be_bytes());
        for signature in &self.last_seen {
            bytes.extend_from_slice(signature);
        }
        bytes
    }
}

/// The partial mask carries Java BitSet words: bit0 is the low bit of word0.
#[derive(Clone, Debug)]
pub enum FilterMask {
    PassThrough,
    FullyFiltered,
    PartiallyFiltered(Vec<u64>),
}

impl PartialEq for FilterMask {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::PassThrough, Self::PassThrough) | (Self::FullyFiltered, Self::FullyFiltered) => {
                true
            }
            (Self::PartiallyFiltered(a), Self::PartiallyFiltered(b)) => {
                // java.util.BitSet ignores trailing zero storage words.
                let a_end = a.iter().rposition(|word| *word != 0).map_or(0, |i| i + 1);
                let b_end = b.iter().rposition(|word| *word != 0).map_or(0, |i| i + 1);
                a[..a_end] == b[..b_end]
            }
            _ => false,
        }
    }
}
impl Eq for FilterMask {}

impl FilterMask {
    /// Java FilterMask.isEmpty tests the type, not whether its BitSet has bits.
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::PassThrough)
    }
    pub fn is_fully_filtered(&self) -> bool {
        matches!(self, Self::FullyFiltered)
    }
    pub fn is_filtered(&self, utf16_index: usize) -> bool {
        match self {
            Self::PassThrough => false,
            Self::FullyFiltered => true,
            Self::PartiallyFiltered(words) => words
                .get(utf16_index / 64)
                .is_some_and(|word| word & (1 << (utf16_index % 64)) != 0),
        }
    }

    /// Preserve Java char-array behavior even when masking half a surrogate
    /// pair. Converting these units to a Rust String can be lossy and is left
    /// to the actual component/string host rather than silently replacing it.
    pub fn apply_utf16(&self, text: &[u16]) -> Option<Vec<u16>> {
        if self.is_fully_filtered() {
            return None;
        }
        Some(
            text.iter()
                .enumerate()
                .map(|(i, unit)| {
                    if self.is_filtered(i) {
                        u16::from(b'#')
                    } else {
                        *unit
                    }
                })
                .collect(),
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerChatMessage {
    pub link: SignedMessageLink,
    pub signature: Option<MessageSignature>,
    pub body: SignedMessageBody,
    pub unsigned_content: Option<Value>,
    pub filter_mask: FilterMask,
}

impl PlayerChatMessage {
    pub fn unsigned(sender: [u8; 16], content: impl Into<String>, now_ms: i64) -> Self {
        Self {
            link: SignedMessageLink::unsigned(sender),
            signature: None,
            body: SignedMessageBody::unsigned(content, now_ms),
            unsigned_content: None,
            filter_mask: FilterMask::PassThrough,
        }
    }

    pub fn system(content: impl Into<String>, now_ms: i64) -> Self {
        Self::unsigned(SYSTEM_SENDER, content, now_ms)
    }

    pub fn signed_content(&self) -> &str {
        &self.body.content
    }
    pub fn sender(&self) -> [u8; 16] {
        self.link.sender
    }
    pub fn is_system(&self) -> bool {
        self.sender() == SYSTEM_SENDER
    }
    pub fn has_signature(&self) -> bool {
        self.signature.is_some()
    }
    pub fn has_signature_from(&self, sender: [u8; 16]) -> bool {
        self.has_signature() && self.sender() == sender
    }
    pub fn is_fully_filtered(&self) -> bool {
        self.filter_mask.is_fully_filtered()
    }

    pub fn decorated_content(&self) -> Value {
        self.unsigned_content
            .clone()
            .unwrap_or_else(|| json!({"text": self.body.content}))
    }

    /// These values are the bounded JSON representations of actual components.
    /// Component equality beyond a bare literal remains owned by that codec.
    pub fn with_unsigned_content(mut self, component: Value) -> Self {
        self.unsigned_content = if component == json!({"text": self.body.content})
            || component == Value::String(self.body.content.clone())
        {
            None
        } else {
            Some(component)
        };
        self
    }
    pub fn remove_unsigned_content(mut self) -> Self {
        self.unsigned_content = None;
        self
    }
    pub fn with_filter_mask(mut self, mask: FilterMask) -> Self {
        self.filter_mask = mask;
        self
    }
    pub fn filter(self, enabled: bool) -> Self {
        if enabled {
            self
        } else {
            self.with_filter_mask(FilterMask::PassThrough)
        }
    }

    /// PlayerChatMessage.removeSignature creates a fresh unsigned body and link,
    /// while retaining unsigned decoration and filtering from the old message.
    pub fn remove_signature(self, now_ms: i64) -> Self {
        Self {
            link: SignedMessageLink::unsigned(self.sender()),
            signature: None,
            body: SignedMessageBody::unsigned(self.body.content, now_ms),
            unsigned_content: self.unsigned_content,
            filter_mask: self.filter_mask,
        }
    }

    pub fn has_expired_server(&self, now_ms: i64) -> bool {
        i128::from(now_ms)
            > i128::from(self.body.timestamp_millis) + i128::from(MESSAGE_EXPIRES_AFTER_SERVER_MS)
    }
    pub fn has_expired_client(&self, now_ms: i64) -> bool {
        i128::from(now_ms)
            > i128::from(self.body.timestamp_millis) + i128::from(MESSAGE_EXPIRES_AFTER_CLIENT_MS)
    }

    pub fn signature_bytes(&self) -> Vec<u8> {
        signature_bytes(&self.link, &self.body)
    }
    pub fn verify(&self, key: &ValidatedProfileKey) -> bool {
        self.signature.as_ref().is_some_and(|signature| {
            let bytes = self.signature_bytes();
            key.verify_message_chunks(&[bytes.as_slice()], signature)
        })
    }
}

/// PlayerChatMessage.updateSignature starts with signed int version1, then link,
/// then body; these are signature-updater bytes, not protocol VarInts.
pub fn signature_bytes(link: &SignedMessageLink, body: &SignedMessageBody) -> Vec<u8> {
    let link_bytes = link.signature_bytes();
    let body_bytes = body.signature_bytes();
    let mut bytes = Vec::with_capacity(4 + link_bytes.len() + body_bytes.len());
    bytes.extend_from_slice(&1_i32.to_be_bytes());
    bytes.extend_from_slice(&link_bytes);
    bytes.extend_from_slice(&body_bytes);
    bytes
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodeError {
    MissingProfileKey,
    ChainBroken,
    ExpiredProfileKey,
    InvalidSignature,
    OutOfOrderChat,
}

impl DecodeError {
    pub const fn translation_key(self) -> &'static str {
        match self {
            Self::MissingProfileKey => "chat.disabled.missingProfileKey",
            Self::ChainBroken => "chat.disabled.chain_broken",
            Self::ExpiredProfileKey => "chat.disabled.expiredProfileKey",
            Self::InvalidSignature => "chat.disabled.invalid_signature",
            Self::OutOfOrderChat => "chat.disabled.out_of_order_chat",
        }
    }
    pub const fn should_disconnect(self) -> bool {
        false
    }
}
impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.translation_key())
    }
}
impl std::error::Error for DecodeError {}

#[derive(Clone, Debug, PartialEq)]
pub struct DecodedMessage {
    pub message: PlayerChatMessage,
    /// Source logs a warning for an otherwise valid message over five minutes
    /// old, then still advances and returns the message.
    pub expired_server_warning: bool,
}

#[derive(Clone)]
pub struct SignedMessageDecoder {
    next_link: Option<SignedMessageLink>,
    last_timestamp_millis: i64,
    key: ValidatedProfileKey,
}

impl fmt::Debug for SignedMessageDecoder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SignedMessageDecoder")
            .field("next_link", &self.next_link)
            .field("last_timestamp_millis", &self.last_timestamp_millis)
            .finish_non_exhaustive()
    }
}

impl SignedMessageDecoder {
    pub fn new(sender: [u8; 16], session_id: [u8; 16], key: ValidatedProfileKey) -> Self {
        Self {
            next_link: Some(SignedMessageLink::root(sender, session_id)),
            last_timestamp_millis: 0,
            key,
        }
    }
    pub fn next_link(&self) -> Option<SignedMessageLink> {
        self.next_link
    }
    pub fn last_timestamp_millis(&self) -> i64 {
        self.last_timestamp_millis
    }
    pub fn key(&self) -> &ValidatedProfileKey {
        &self.key
    }
    pub fn set_chain_broken(&mut self) {
        self.next_link = None;
    }

    pub fn unpack(
        &mut self,
        signature: Option<MessageSignature>,
        body: SignedMessageBody,
        now_ms: i64,
    ) -> Result<DecodedMessage, DecodeError> {
        // Preserve the source ordering and which failures mutate the chain.
        let signature = signature.ok_or(DecodeError::MissingProfileKey)?;
        if self.key.data().has_expired(now_ms) {
            return Err(DecodeError::ExpiredProfileKey);
        }
        let link = self.next_link.ok_or(DecodeError::ChainBroken)?;
        if body.timestamp_millis < self.last_timestamp_millis {
            self.set_chain_broken();
            return Err(DecodeError::OutOfOrderChat);
        }
        self.last_timestamp_millis = body.timestamp_millis;
        let message = PlayerChatMessage {
            link,
            signature: Some(signature),
            body,
            unsigned_content: None,
            filter_mask: FilterMask::PassThrough,
        };
        if !message.verify(&self.key) {
            self.set_chain_broken();
            return Err(DecodeError::InvalidSignature);
        }
        let expired_server_warning = message.has_expired_server(now_ms);
        self.next_link = link.advance();
        Ok(DecodedMessage {
            message,
            expired_server_warning,
        })
    }
}

/// The unsigned decoder discards the supplied salt, timestamp and last-seen
/// body, constructing PlayerChatMessage.unsigned from its content and actual now.
pub fn unsigned_decode(
    sender: [u8; 16],
    require_secure_profile: bool,
    body: SignedMessageBody,
    now_ms: i64,
) -> Result<DecodedMessage, DecodeError> {
    if require_secure_profile {
        return Err(DecodeError::MissingProfileKey);
    }
    Ok(DecodedMessage {
        message: PlayerChatMessage::unsigned(sender, body.content, now_ms),
        expired_server_warning: false,
    })
}

#[derive(Clone, Debug)]
pub enum MessageDecoder {
    Unsigned { sender: [u8; 16] },
    Signed(SignedMessageDecoder),
}
impl MessageDecoder {
    pub const fn unsigned(sender: [u8; 16]) -> Self {
        Self::Unsigned { sender }
    }
    pub fn signed(sender: [u8; 16], session_id: [u8; 16], key: ValidatedProfileKey) -> Self {
        Self::Signed(SignedMessageDecoder::new(sender, session_id, key))
    }
    pub fn set_chain_broken(&mut self) {
        if let Self::Signed(decoder) = self {
            decoder.set_chain_broken();
        }
    }
    pub fn unpack(
        &mut self,
        signature: Option<MessageSignature>,
        body: SignedMessageBody,
        now_ms: i64,
        require_secure_profile: bool,
    ) -> Result<DecodedMessage, DecodeError> {
        match self {
            Self::Unsigned { sender } => {
                unsigned_decode(*sender, require_secure_profile, body, now_ms)
            }
            Self::Signed(decoder) => decoder.unpack(signature, body, now_ms),
        }
    }
}

#[derive(Clone)]
pub struct RemoteChatSession {
    pub session_id: [u8; 16],
    pub key: ValidatedProfileKey,
}
impl RemoteChatSession {
    pub fn create_message_decoder(&self, sender: [u8; 16]) -> SignedMessageDecoder {
        SignedMessageDecoder::new(sender, self.session_id, self.key.clone())
    }
    pub fn has_expired(&self, now_ms: i64) -> bool {
        self.key.data().has_expired(now_ms)
    }
    pub fn create_message_validator(&self, grace_ms: i64) -> KeyBasedMessageValidator {
        KeyBasedMessageValidator {
            key: self.key.clone(),
            grace_ms,
            last_message: None,
            chain_valid: true,
        }
    }
}

/// Client-side RemoteChatSession validator, kept distinct from the server's
/// exact-next-link decoder. It accepts an identical repeated message or any
/// strictly descendant link after cryptographic/key-expiry validation.
#[derive(Clone)]
pub struct KeyBasedMessageValidator {
    key: ValidatedProfileKey,
    grace_ms: i64,
    last_message: Option<PlayerChatMessage>,
    chain_valid: bool,
}
impl KeyBasedMessageValidator {
    pub fn is_chain_valid(&self) -> bool {
        self.chain_valid
    }
    pub fn last_message(&self) -> Option<&PlayerChatMessage> {
        self.last_message.as_ref()
    }
    pub fn update_and_validate(
        &mut self,
        message: PlayerChatMessage,
        now_ms: i64,
    ) -> Option<PlayerChatMessage> {
        if self.chain_valid {
            let expired = i128::from(now_ms)
                > i128::from(self.key.data().expires_at_ms) + i128::from(self.grace_ms);
            self.chain_valid = !expired
                && message.verify(&self.key)
                && self.last_message.as_ref().is_none_or(|last| {
                    &message == last || message.link.is_descendant_of(&last.link)
                });
        }
        if !self.chain_valid {
            return None;
        }
        self.last_message = Some(message.clone());
        Some(message)
    }
}

/// Encoder state is separate from the server decoder's owner. Source advances
/// the link before invoking Signer, so an actual signing failure still consumes
/// that encoder link. No private key or signer is retained in this module.
#[derive(Clone, Debug)]
pub struct SignedMessageEncoder {
    next_link: Option<SignedMessageLink>,
}
impl SignedMessageEncoder {
    pub fn new(sender: [u8; 16], session_id: [u8; 16]) -> Self {
        Self {
            next_link: Some(SignedMessageLink::root(sender, session_id)),
        }
    }
    pub fn pack<E>(
        &mut self,
        body: &SignedMessageBody,
        mut sign: impl FnMut(&[u8]) -> Result<MessageSignature, E>,
    ) -> Result<Option<MessageSignature>, E> {
        let Some(link) = self.next_link else {
            return Ok(None);
        };
        self.next_link = link.advance();
        sign(&signature_bytes(&link, body)).map(Some)
    }
}
