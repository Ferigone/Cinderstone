//! Connection-owned Java 1.21 server chat listener state.
//!
//! The host owns the actual server-thread queue, text filtering/decorating,
//! admitted player, and transport. This owner validates incoming acknowledgments
//! before text dispatch, keeps one real message decoder, and records effects in
//! source order. A decoded message is not itself a broadcast or command result.

use std::collections::BTreeMap;

use anyhow::Result;

use crate::{
    chat_ack::{LastSeenMessagesValidator, LastSeenUpdate},
    chat_chain::{DecodedMessage, MessageDecoder, PlayerChatMessage, SignedMessageBody},
    chat_crypto::{self, ProfilePublicKeyData, ServiceKeySet},
    chat_wire::IncomingChatMessage,
    command_protocol::SignedCommand,
    entity::ChatSession,
};

pub const CHAT_VALIDATION_FAILED: &str = "multiplayer.disconnect.chat_validation_failed";
pub const ILLEGAL_CHARACTERS: &str = "multiplayer.disconnect.illegal_characters";
pub const TOO_MANY_PENDING_CHATS: &str = "multiplayer.disconnect.too_many_pending_chats";
pub const INVALID_COMMAND_SIGNATURE: &str = "chat.disabled.invalid_command_signature";
pub const CHAT_DISABLED_OPTIONS: &str = "chat.disabled.options";
pub const CHAT_SPAM: &str = "disconnect.spam";
pub const TRACKED_MESSAGE_DISCONNECT_THRESHOLD: usize = 4096;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChatVisibility {
    #[default]
    Full,
    System,
    Hidden,
}

impl ChatVisibility {
    pub fn from_id(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::Full),
            1 => Some(Self::System),
            2 => Some(Self::Hidden),
            _ => None,
        }
    }

    /// ServerPlayer.acceptsChatMessages only permits the full setting. The
    /// SYSTEM setting nevertheless may submit chat/commands through this owner.
    pub fn accepts_player_chat(self) -> bool {
        self == Self::Full
    }
}

/// All fields must come from the current admitted player/server owners. In the
/// dedicated server enforce is property && online-mode && available trust keys.
#[derive(Clone, Copy, Debug)]
pub struct ChatContext {
    pub now_ms: i64,
    pub enforce_secure_profile: bool,
    pub chat_visibility: ChatVisibility,
    pub is_operator: bool,
    pub is_singleplayer_owner: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatEffect {
    Disconnect(&'static str),
    SystemMessage {
        translation: &'static str,
        red: bool,
    },
    /// An explicit native unsupported-context response, never a fabricated
    /// successful signature/command result or an invented vanilla translation.
    SystemLiteral {
        text: String,
        red: bool,
    },
    Warning {
        message: &'static str,
    },
}

#[derive(Clone, Debug, Default)]
pub struct ChatOutcome {
    pub message: Option<PlayerChatMessage>,
    pub effects: Vec<ChatEffect>,
    /// tryHandleChat calls ServerPlayer.resetLastActionTime before enqueuing its
    /// runnable. Hidden/illegal/invalid-ack messages do not reset that owner.
    pub reset_last_action: bool,
}

/// The value is the exact original Brigadier StringRange, before selector
/// resolution, whitespace normalization, filtering or decoration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignableArgument {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug)]
pub struct AuthorizedCommand {
    pub command: String,
    pub signed_arguments: BTreeMap<String, PlayerChatMessage>,
}

#[derive(Clone, Debug, Default)]
pub struct ChatCommandOutcome {
    pub command: Option<AuthorizedCommand>,
    pub effects: Vec<ChatEffect>,
    pub reset_last_action: bool,
    /// Source command runnables call detectRateSpam after the command attempt,
    /// including signature/parse/execution failure. The host charges after
    /// applying this outcome and attempting any authorized execution.
    pub charge_command_spam: bool,
}

#[derive(Clone, Debug, Default)]
pub struct SessionOutcome {
    pub changed: bool,
    pub session: Option<ChatSession>,
    pub effects: Vec<ChatEffect>,
}

#[derive(Clone, Debug)]
pub struct ChatListener {
    sender: [u8; 16],
    decoder: MessageDecoder,
    last_seen: LastSeenMessagesValidator,
    session: Option<ChatSession>,
    session_key_data: Option<ProfilePublicKeyData>,
    chat_spam_ticks: i32,
}

impl ChatListener {
    /// Fresh ServerGamePacketListenerImpl: unsigned decoder, 20 null tracked
    /// entries, no remote session and a zero spam counter. It is not persisted.
    pub fn new(sender: [u8; 16]) -> Self {
        Self {
            sender,
            decoder: MessageDecoder::unsigned(sender),
            last_seen: LastSeenMessagesValidator::default(),
            session: None,
            session_key_data: None,
            chat_spam_ticks: 0,
        }
    }

    pub fn sender(&self) -> [u8; 16] {
        self.sender
    }
    pub fn session(&self) -> Option<&ChatSession> {
        self.session.as_ref()
    }
    pub fn spam_ticks(&self) -> i32 {
        self.chat_spam_ticks
    }
    pub fn tracked_messages_count(&self) -> usize {
        self.last_seen.tracked_messages_count()
    }

    pub fn tick(&mut self) {
        if self.chat_spam_ticks > 0 {
            self.chat_spam_ticks -= 1;
        }
    }

    /// Only cumulative actual listener ticks may be supplied, not elapsed
    /// milliseconds or number of coalesced watch notifications.
    pub fn advance_ticks(&mut self, ticks: u64) {
        if self.chat_spam_ticks > 0 {
            self.chat_spam_ticks -= ticks.min(self.chat_spam_ticks as u64) as i32;
        }
    }

    pub fn handle_ack(&mut self, offset: i32) -> Option<ChatEffect> {
        (!self.last_seen.apply_offset(offset))
            .then_some(ChatEffect::Disconnect(CHAT_VALIDATION_FAILED))
    }

    /// Called after the actual recipient packet has been sent/enqueued and its
    /// signature cache has been pushed by that same recipient owner. Unsigned
    /// messages do not enter the acknowledgment validator at all.
    pub fn add_pending(&mut self, message: &PlayerChatMessage) -> Option<ChatEffect> {
        let signature = message.signature?;
        self.last_seen.add_pending(signature);
        (self.last_seen.tracked_messages_count() > TRACKED_MESSAGE_DISCONNECT_THRESHOLD)
            .then_some(ChatEffect::Disconnect(TOO_MANY_PENDING_CHATS))
    }

    pub fn handle_chat(
        &mut self,
        packet: IncomingChatMessage,
        context: ChatContext,
    ) -> ChatOutcome {
        let mut outcome = ChatOutcome::default();
        let last_seen = match self.apply_last_seen(&packet.last_seen) {
            Ok(value) => value,
            Err(effect) => {
                outcome.effects.push(effect);
                return outcome;
            }
        };
        if let Some(effect) = text_dispatch_gate(&packet.message, context.chat_visibility) {
            outcome.effects.push(effect);
            return outcome;
        }
        outcome.reset_last_action = true;
        let body = SignedMessageBody {
            content: packet.message,
            timestamp_millis: packet.timestamp_millis,
            salt: packet.salt,
            last_seen,
        };
        match self.decoder.unpack(
            packet.signature,
            body,
            context.now_ms,
            context.enforce_secure_profile,
        ) {
            Ok(decoded) => {
                append_decode_warning(&mut outcome.effects, &decoded);
                outcome.message = Some(decoded.message);
            }
            Err(error) => outcome.effects.push(red_system(error.translation_key())),
        }
        outcome
    }

    /// The boolean comes from an actual permission-filtered command parse. It
    /// must not be guessed merely from words contained in an unknown command.
    pub fn handle_unsigned_command(
        &mut self,
        command: &str,
        has_signable_arguments: bool,
        context: ChatContext,
    ) -> ChatCommandOutcome {
        let mut outcome = ChatCommandOutcome::default();
        if let Some(effect) = text_dispatch_gate(command, context.chat_visibility) {
            outcome.effects.push(effect);
            return outcome;
        }
        outcome.reset_last_action = true;
        outcome.charge_command_spam = true;
        if context.enforce_secure_profile && has_signable_arguments {
            outcome.effects.push(red_system(INVALID_COMMAND_SIGNATURE));
            return outcome;
        }
        outcome.command = Some(AuthorizedCommand {
            command: command.to_owned(),
            signed_arguments: BTreeMap::new(),
        });
        outcome
    }

    /// The host supplies the exact native parsed signable ranges. This does
    /// not claim support for an absent Brigadier parser or selector resolver.
    pub fn handle_signed_command(
        &mut self,
        packet: &SignedCommand,
        parsed_arguments: &[SignableArgument],
        context: ChatContext,
    ) -> ChatCommandOutcome {
        let mut outcome = ChatCommandOutcome::default();
        let update = LastSeenUpdate {
            offset: packet.last_seen_offset,
            acknowledged: packet.acknowledged,
        };
        let last_seen = match self.apply_last_seen(&update) {
            Ok(value) => value,
            Err(effect) => {
                outcome.effects.push(effect);
                return outcome;
            }
        };
        if let Some(effect) = text_dispatch_gate(&packet.command, context.chat_visibility) {
            outcome.effects.push(effect);
            return outcome;
        }
        outcome.reset_last_action = true;
        outcome.charge_command_spam = true;
        let arguments = self.collect_signed_arguments(
            packet,
            parsed_arguments,
            &last_seen,
            context,
            &mut outcome.effects,
        );
        match arguments {
            Ok(signed_arguments) => {
                outcome.command = Some(AuthorizedCommand {
                    command: packet.command.clone(),
                    signed_arguments,
                })
            }
            Err(effect) => outcome.effects.push(effect),
        }
        outcome
    }

    /// Keep the actual acknowledgment window and source text gates even when
    /// the native command grammar cannot prove a signable argument's range.
    /// Refusal here is explicitly a native limitation, not signature acceptance.
    pub fn reject_signed_command_unsupported(
        &mut self,
        packet: &SignedCommand,
        context: ChatContext,
        detail: &str,
    ) -> ChatCommandOutcome {
        let mut outcome = ChatCommandOutcome::default();
        let update = LastSeenUpdate {
            offset: packet.last_seen_offset,
            acknowledged: packet.acknowledged,
        };
        if let Err(effect) = self.apply_last_seen(&update) {
            outcome.effects.push(effect);
            return outcome;
        }
        if let Some(effect) = text_dispatch_gate(&packet.command, context.chat_visibility) {
            outcome.effects.push(effect);
            return outcome;
        }
        outcome.reset_last_action = true;
        outcome.charge_command_spam = true;
        outcome.effects.push(ChatEffect::SystemLiteral {
            text: format!("Unsupported native signed-command context: {detail}"),
            red: true,
        });
        outcome
    }

    /// A successful chat decode is not charged until filtering/decorating and
    /// the actual broadcast finish. Decode/hidden/illegal failures are not spam
    /// attempts through the source chat callback.
    pub fn after_chat_broadcast(&mut self, context: ChatContext) -> Option<ChatEffect> {
        self.detect_rate_spam(context)
    }

    /// Supply current operator/owner status after execution (for example, a
    /// successful self-deop changes the exemption before this source check).
    pub fn after_command(&mut self, context: ChatContext) -> Option<ChatEffect> {
        self.detect_rate_spam(context)
    }

    /// Codec-level malformed key errors remain ordinary decoding errors. The
    /// wire decoder and this constructor both normalize an RSA SPKI key.
    pub fn handle_session_update(
        &mut self,
        incoming: &ChatSession,
        services: Option<&ServiceKeySet>,
    ) -> Result<SessionOutcome> {
        let data = ProfilePublicKeyData::new(
            incoming.expires_at,
            incoming.public_key.clone(),
            incoming.key_signature.clone(),
        )?;
        let mut outcome = SessionOutcome::default();
        if self.session_key_data.as_ref() == Some(&data) {
            return Ok(outcome);
        }
        if self
            .session_key_data
            .as_ref()
            .is_some_and(|old| data.expires_at_ms < old.expires_at_ms)
        {
            outcome.effects.push(ChatEffect::Disconnect(
                chat_crypto::EXPIRED_PROFILE_PUBLIC_KEY,
            ));
            return Ok(outcome);
        }
        let Some(services) = services.filter(|services| !services.is_empty()) else {
            outcome.effects.push(ChatEffect::Warning {
                message: "Ignoring chat session due to missing Services public key",
            });
            return Ok(outcome);
        };
        let key = match chat_crypto::create_validated(self.sender, data.clone(), services) {
            Ok(key) => key,
            Err(error) => {
                outcome
                    .effects
                    .push(ChatEffect::Disconnect(error.translation_key()));
                return Ok(outcome);
            }
        };
        let session = ChatSession {
            uuid: incoming.uuid,
            expires_at: data.expires_at_ms,
            public_key: data.public_key_der.clone(),
            key_signature: data.key_signature.clone(),
        };
        self.decoder = MessageDecoder::signed(self.sender, session.uuid, key);
        self.session_key_data = Some(data);
        self.session = Some(session.clone());
        outcome.changed = true;
        outcome.session = Some(session);
        Ok(outcome)
    }

    fn apply_last_seen(
        &mut self,
        update: &LastSeenUpdate,
    ) -> std::result::Result<Vec<crate::chat_ack::MessageSignature>, ChatEffect> {
        self.last_seen
            .apply_update(update)
            .map_err(|_| ChatEffect::Disconnect(CHAT_VALIDATION_FAILED))
    }

    fn detect_rate_spam(&mut self, context: ChatContext) -> Option<ChatEffect> {
        self.chat_spam_ticks = self.chat_spam_ticks.wrapping_add(20);
        (self.chat_spam_ticks > 200 && !context.is_operator && !context.is_singleplayer_owner)
            .then_some(ChatEffect::Disconnect(CHAT_SPAM))
    }

    fn collect_signed_arguments(
        &mut self,
        packet: &SignedCommand,
        parsed_arguments: &[SignableArgument],
        last_seen: &[crate::chat_ack::MessageSignature],
        context: ChatContext,
        effects: &mut Vec<ChatEffect>,
    ) -> std::result::Result<BTreeMap<String, PlayerChatMessage>, ChatEffect> {
        let mut signed = BTreeMap::new();
        if packet.argument_signatures.is_empty() {
            for argument in parsed_arguments {
                let body = SignedMessageBody::unsigned(argument.value.clone(), context.now_ms);
                let decoded = self
                    .decoder
                    .unpack(None, body, context.now_ms, context.enforce_secure_profile)
                    .map_err(|error| red_system(error.translation_key()))?;
                append_decode_warning(effects, &decoded);
                signed.insert(argument.name.clone(), decoded.message);
            }
        } else {
            for signature in &packet.argument_signatures {
                let Some(argument) = parsed_arguments
                    .iter()
                    .find(|argument| argument.name == signature.name)
                else {
                    self.decoder.set_chain_broken();
                    return Err(red_system(INVALID_COMMAND_SIGNATURE));
                };
                let body = SignedMessageBody {
                    content: argument.value.clone(),
                    timestamp_millis: packet.timestamp_millis,
                    salt: packet.salt,
                    last_seen: last_seen.to_vec(),
                };
                let decoded = self
                    .decoder
                    .unpack(
                        Some(signature.signature),
                        body,
                        context.now_ms,
                        context.enforce_secure_profile,
                    )
                    .map_err(|error| red_system(error.translation_key()))?;
                append_decode_warning(effects, &decoded);
                signed.insert(argument.name.clone(), decoded.message);
            }
            if parsed_arguments
                .iter()
                .any(|argument| !signed.contains_key(&argument.name))
            {
                return Err(red_system(INVALID_COMMAND_SIGNATURE));
            }
        }
        Ok(signed)
    }
}

pub fn is_chat_message_illegal(value: &str) -> bool {
    value
        .encode_utf16()
        .any(|unit| unit == 0x00a7 || unit < 0x20 || unit == 0x7f)
}

fn text_dispatch_gate(value: &str, visibility: ChatVisibility) -> Option<ChatEffect> {
    if is_chat_message_illegal(value) {
        return Some(ChatEffect::Disconnect(ILLEGAL_CHARACTERS));
    }
    (visibility == ChatVisibility::Hidden).then(|| red_system(CHAT_DISABLED_OPTIONS))
}

fn red_system(translation: &'static str) -> ChatEffect {
    ChatEffect::SystemMessage {
        translation,
        red: true,
    }
}

fn append_decode_warning(effects: &mut Vec<ChatEffect>, decoded: &DecodedMessage) {
    if decoded.expired_server_warning {
        effects.push(ChatEffect::Warning {
            message: "Received expired chat message",
        });
    }
}
