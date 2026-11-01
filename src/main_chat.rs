//! Native production chat owners for actual admitted players/connections.
//!
//! Main supplies the configured registry holder, current service keys, time and
//! operator state. This runtime represents the dedicated no-team, no external
//! text-filter profile; it never manufactures trust keys, a player or a session.
use std::collections::BTreeMap;

use anyhow::{ensure, Context, Result};
use mc121_rust::{
    access_lists::AccessProfile,
    admission_commands::{self, AdminCommand},
    chat_ack::MessageSignatureCache,
    chat_chain::PlayerChatMessage,
    chat_crypto::ServiceKeySet,
    chat_listener::{
        AuthorizedCommand, ChatCommandOutcome, ChatContext, ChatEffect, ChatListener,
        ChatVisibility, SignableArgument,
    },
    chat_wire::{self, BoundChatType},
    command_protocol,
    entity::{ChatSession, PlayerProfile},
    protocol::Packet,
};
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct MainChatSettings {
    pub visibility: ChatVisibility,
    pub text_filtering_enabled: bool,
}

impl MainChatSettings {
    pub fn from_client_information(visibility: i32, text_filtering_enabled: bool) -> Result<Self> {
        Ok(Self {
            visibility: ChatVisibility::from_id(visibility)
                .context("invalid actual client chat visibility")?,
            text_filtering_enabled,
        })
    }
}

/// These values must be current at the call phase, especially after a command
/// such as self-deop. Enforce is the live dedicated-server effective predicate.
#[derive(Clone, Copy, Debug)]
pub(super) struct MainChatContext {
    pub now_ms: i64,
    pub enforce_secure_profile: bool,
    pub is_operator: bool,
    pub is_singleplayer_owner: bool,
}

impl MainChatContext {
    fn for_settings(self, settings: MainChatSettings) -> ChatContext {
        ChatContext {
            now_ms: self.now_ms,
            enforce_secure_profile: self.enforce_secure_profile,
            chat_visibility: settings.visibility,
            is_operator: self.is_operator,
            is_singleplayer_owner: self.is_singleplayer_owner,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) enum MainChatEffect {
    Packet {
        recipient: i32,
        packet: Packet,
    },
    Disconnect {
        recipient: i32,
        reason: &'static str,
    },
    /// Update the real registry owner and publish INITIALIZE_CHAT to its actual
    /// audience before a following message can use this accepted session.
    SessionUpdate {
        entity_id: i32,
        session: ChatSession,
    },
    Warning {
        entity_id: i32,
        message: &'static str,
    },
    ChatLog {
        sender: [u8; 16],
        content: Value,
        name: Value,
        trusted: bool,
    },
}

#[derive(Clone, Debug, Default)]
pub(super) struct MainChatOutcome {
    /// Main applies this prefix in order even when a later native encode fails.
    pub effects: Vec<MainChatEffect>,
    pub command: Option<AuthorizedCommand>,
    pub reset_last_action: bool,
    pub charge_command_spam: bool,
    pub error: Option<String>,
}

impl MainChatOutcome {
    fn append_listener_effect(&mut self, recipient: i32, effect: ChatEffect) {
        match effect {
            ChatEffect::Disconnect(reason) => self
                .effects
                .push(MainChatEffect::Disconnect { recipient, reason }),
            ChatEffect::Warning { message } => self.effects.push(MainChatEffect::Warning {
                entity_id: recipient,
                message,
            }),
            ChatEffect::SystemMessage { translation, red } => {
                let mut component = json!({"translate": translation});
                if red {
                    component["color"] = json!("red");
                }
                self.append_system(recipient, &component);
            }
            ChatEffect::SystemLiteral { text, red } => {
                let mut component = json!({"text": text});
                if red {
                    component["color"] = json!("red");
                }
                self.append_system(recipient, &component);
            }
        }
    }

    fn append_system(&mut self, recipient: i32, component: &Value) {
        match chat_wire::encode_system_chat(component, false) {
            Ok(packet) => self
                .effects
                .push(MainChatEffect::Packet { recipient, packet }),
            Err(error) => self.error = Some(format!("native chat system component: {error:#}")),
        }
    }

    fn from_command(entity_id: i32, value: ChatCommandOutcome) -> Self {
        let mut result = Self {
            command: value.command,
            reset_last_action: value.reset_last_action,
            charge_command_spam: value.charge_command_spam,
            ..Self::default()
        };
        for effect in value.effects {
            result.append_listener_effect(entity_id, effect);
        }
        result
    }
}

struct MainChatConnection {
    profile: AccessProfile,
    settings: MainChatSettings,
    listener: ChatListener,
    /// ServerGamePacketListenerImpl.messageSignatureCache belongs to this
    /// receiving connection. Session updates do not clear or replace it.
    signature_cache: MessageSignatureCache,
}

#[derive(Default)]
pub(super) struct MainChatRuntime {
    connections: BTreeMap<i32, MainChatConnection>,
    /// Actual registration order, rather than guessing PlayerList ordering
    /// from asynchronously allocated entity IDs.
    registration_order: Vec<i32>,
}

impl MainChatRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        entity_id: i32,
        profile: &PlayerProfile,
        settings: MainChatSettings,
    ) -> Result<()> {
        ensure!(
            !self.connections.contains_key(&entity_id),
            "chat connection entity already registered"
        );
        ensure!(
            !self
                .connections
                .values()
                .any(|entry| entry.profile.uuid == profile.uuid),
            "chat profile already registered"
        );
        profile.validate()?;
        self.connections.insert(
            entity_id,
            MainChatConnection {
                profile: AccessProfile {
                    uuid: profile.uuid,
                    name: profile.name.clone(),
                },
                settings,
                listener: ChatListener::new(profile.uuid),
                signature_cache: MessageSignatureCache::default(),
            },
        );
        self.registration_order.push(entity_id);
        Ok(())
    }

    /// A fresh PLAY listener after reconfiguration receives a fresh runtime
    /// entry; no acknowledgment window or signing decoder is persisted.
    pub fn remove(&mut self, entity_id: i32) {
        self.connections.remove(&entity_id);
        self.registration_order.retain(|id| *id != entity_id);
    }

    pub fn contains(&self, entity_id: i32) -> bool {
        self.connections.contains_key(&entity_id)
    }

    pub fn update_client_information(
        &mut self,
        entity_id: i32,
        settings: MainChatSettings,
    ) -> Result<()> {
        self.connection_mut(entity_id)?.settings = settings;
        Ok(())
    }

    pub fn tick(&mut self) {
        for entry in self.connections.values_mut() {
            entry.listener.tick();
        }
    }

    /// Main may supply a losslessly observed cumulative actual listener tick
    /// span. A wall-clock estimate is not an equivalent input.
    pub fn advance_ticks(&mut self, count: u64) {
        for entry in self.connections.values_mut() {
            entry.listener.advance_ticks(count);
        }
    }

    pub fn handle_ack(&mut self, entity_id: i32, payload: &[u8]) -> Result<MainChatOutcome> {
        let offset = chat_wire::decode_chat_ack(payload)?;
        let effect = self.connection_mut(entity_id)?.listener.handle_ack(offset);
        let mut outcome = MainChatOutcome::default();
        if let Some(effect) = effect {
            outcome.append_listener_effect(entity_id, effect);
        }
        Ok(outcome)
    }

    pub fn handle_session_update(
        &mut self,
        entity_id: i32,
        payload: &[u8],
        services: Option<&ServiceKeySet>,
    ) -> Result<MainChatOutcome> {
        let incoming = chat_wire::decode_chat_session_update(payload)?;
        let session = self
            .connection_mut(entity_id)?
            .listener
            .handle_session_update(&incoming, services)?;
        let mut outcome = MainChatOutcome::default();
        for effect in session.effects {
            outcome.append_listener_effect(entity_id, effect);
        }
        if session.changed {
            outcome.effects.push(MainChatEffect::SessionUpdate {
                entity_id,
                session: session
                    .session
                    .context("accepted actual chat session absent")?,
            });
        }
        Ok(outcome)
    }

    /// Uses the actual configured minecraft:chat registry holder supplied by
    /// Main. The supported TextFilter owner is the dedicated no-op filter; it
    /// genuinely yields pass-through rather than pretending an external filter
    /// has run. Rich server decorators and scoreboard teams remain unsupported.
    pub fn handle_message(
        &mut self,
        entity_id: i32,
        payload: &[u8],
        host: MainChatContext,
        chat_type_registry_id: i32,
    ) -> Result<MainChatOutcome> {
        let incoming = chat_wire::decode_chat_message(payload)?;
        let (context, source, source_filtering, received) = {
            let entry = self.connection_mut(entity_id)?;
            let context = host.for_settings(entry.settings);
            let received = entry.listener.handle_chat(incoming, context);
            (
                context,
                entry.profile.clone(),
                entry.settings.text_filtering_enabled,
                received,
            )
        };
        let mut outcome = MainChatOutcome {
            reset_last_action: received.reset_last_action,
            ..MainChatOutcome::default()
        };
        for effect in received.effects {
            outcome.append_listener_effect(entity_id, effect);
        }
        let Some(message) = received.message else {
            return Ok(outcome);
        };
        let bound = BoundChatType {
            registry_id: chat_type_registry_id,
            name: admission_commands::default_player_display_component(&source),
            target_name: None,
        };
        self.broadcast(
            entity_id,
            source_filtering,
            message,
            bound,
            host.now_ms,
            &mut outcome,
        );
        if outcome.error.is_none() {
            if let Some(effect) = self
                .connection_mut(entity_id)?
                .listener
                .after_chat_broadcast(context)
            {
                outcome.append_listener_effect(entity_id, effect);
            }
        }
        Ok(outcome)
    }

    pub fn handle_unsigned_command(
        &mut self,
        entity_id: i32,
        command: &str,
        permission_level: i32,
        host: MainChatContext,
    ) -> Result<MainChatOutcome> {
        let has_signable = unsigned_has_signable_arguments(command, permission_level);
        let entry = self.connection_mut(entity_id)?;
        let value = entry.listener.handle_unsigned_command(
            command,
            has_signable,
            host.for_settings(entry.settings),
        );
        let mut outcome = MainChatOutcome::from_command(entity_id, value);
        reject_chat_leading_slash(entity_id, command, &mut outcome);
        Ok(outcome)
    }

    pub fn handle_signed_command(
        &mut self,
        entity_id: i32,
        payload: &[u8],
        permission_level: i32,
        host: MainChatContext,
    ) -> Result<MainChatOutcome> {
        let packet = command_protocol::decode_signed_command(payload)?;
        let arguments = known_signable_arguments(&packet.command, permission_level);
        let entry = self.connection_mut(entity_id)?;
        let context = host.for_settings(entry.settings);
        let value = match arguments {
            Ok(arguments) => entry
                .listener
                .handle_signed_command(&packet, &arguments, context),
            Err(detail) => entry
                .listener
                .reject_signed_command_unsupported(&packet, context, detail),
        };
        let mut outcome = MainChatOutcome::from_command(entity_id, value);
        reject_chat_leading_slash(entity_id, &packet.command, &mut outcome);
        Ok(outcome)
    }

    /// Root calls this after applying command errors/effects and attempting the
    /// actual authorized administration command. Supply fresh operator status.
    pub fn finish_command(
        &mut self,
        entity_id: i32,
        host: MainChatContext,
    ) -> Result<MainChatOutcome> {
        let entry = self.connection_mut(entity_id)?;
        let effect = entry
            .listener
            .after_command(host.for_settings(entry.settings));
        let mut outcome = MainChatOutcome::default();
        if let Some(effect) = effect {
            outcome.append_listener_effect(entity_id, effect);
        }
        Ok(outcome)
    }

    fn connection_mut(&mut self, entity_id: i32) -> Result<&mut MainChatConnection> {
        self.connections
            .get_mut(&entity_id)
            .context("actual admitted chat connection absent")
    }

    fn broadcast(
        &mut self,
        sender_id: i32,
        sender_filtering: bool,
        message: PlayerChatMessage,
        bound: BoundChatType,
        now_ms: i64,
        outcome: &mut MainChatOutcome,
    ) {
        outcome.effects.push(MainChatEffect::ChatLog {
            sender: message.sender(),
            content: message.decorated_content(),
            name: bound.name.clone(),
            trusted: message.has_signature() && !message.has_expired_server(now_ms),
        });
        let mut filtered_full = false;
        for recipient in self.registration_order.clone() {
            let Some(entry) = self.connections.get_mut(&recipient) else {
                outcome.error = Some("registered chat audience owner disappeared".into());
                return;
            };
            // ServerPlayer.shouldFilterMessageTo excludes the sender itself.
            let filtering = recipient != sender_id
                && (sender_filtering || entry.settings.text_filtering_enabled);
            filtered_full |= filtering && message.is_fully_filtered();
            if !entry.settings.visibility.accepts_player_chat() {
                continue;
            }
            let filtered = message.clone().filter(filtering);
            if filtered.is_fully_filtered() {
                continue;
            }
            let packet = if filtered.is_system() {
                chat_wire::encode_disguised_chat(&filtered.decorated_content(), &bound)
            } else {
                chat_wire::encode_player_chat(&filtered, &bound, &entry.signature_cache)
            };
            let packet = match packet {
                Ok(packet) => packet,
                Err(error) => {
                    outcome.error = Some(format!("native recipient chat packet: {error:#}"));
                    return;
                }
            };
            // Enqueue the actual packet before advancing this recipient's old
            // cache and pending validator. Main publishes this ordered prefix
            // before accepting another operation on the actual owner.
            outcome
                .effects
                .push(MainChatEffect::Packet { recipient, packet });
            if filtered.signature.is_some() {
                entry
                    .signature_cache
                    .push(&filtered.body.last_seen, filtered.signature.as_ref());
                if let Some(effect) = entry.listener.add_pending(&filtered) {
                    outcome.append_listener_effect(recipient, effect);
                }
            }
        }
        if filtered_full {
            if self
                .connections
                .get(&sender_id)
                .is_some_and(|entry| entry.settings.visibility != ChatVisibility::Hidden)
            {
                outcome.append_system(sender_id, &json!({"translate":"chat.filtered_full"}));
            }
        }
    }
}

fn reject_chat_leading_slash(entity_id: i32, command: &str, outcome: &mut MainChatOutcome) {
    // The client has already removed its command slash. Do not accidentally
    // reuse the console parser's convenience slash removal for a second slash.
    if command.starts_with('/') && outcome.command.take().is_some() {
        outcome.append_system(
            entity_id,
            &json!({"translate":"command.unknown.command","color":"red"}),
        );
    }
}

fn unsigned_has_signable_arguments(command: &str, permission_level: i32) -> bool {
    if command.starts_with('/') {
        return false;
    }
    match admission_commands::parse_command(command) {
        Ok(parsed) => {
            permission_level >= i32::from(admission_commands::required_permission(&parsed))
                && parsed.has_signable_arguments()
        }
        // Selector expansion is not implemented by the dispatcher and cannot
        // execute. Do not let an unproved reason range turn into an unsigned
        // signature exemption when secure profiles are enforced.
        Err(admission_commands::CommandError::Unsupported(_)) => {
            permission_level >= 3
                && matches!(command.split(' ').next(), Some("ban" | "ban-ip" | "kick"))
        }
        Err(_) => false,
    }
}

/// Only the existing permission-filtered bounded administration grammar may
/// claim a signable range. Canonical plain-message reasons are taken straight
/// from the original wire string, never from a normalized command AST.
fn known_signable_arguments(
    command: &str,
    permission_level: i32,
) -> std::result::Result<Vec<SignableArgument>, &'static str> {
    if command.starts_with('/') {
        return Ok(Vec::new());
    }
    let parsed = match admission_commands::parse_command(command) {
        Ok(command) => command,
        Err(admission_commands::CommandError::Unsupported(_)) => {
            return Err("unported selector or administration grammar")
        }
        // No successfully parsed MessageArgument means there is no proved
        // signable range. The actual command dispatcher still reports syntax.
        Err(_) => return Ok(Vec::new()),
    };
    if permission_level < i32::from(admission_commands::required_permission(&parsed)) {
        return Ok(Vec::new());
    }
    let (literal, target, reason) = match parsed {
        AdminCommand::Ban { targets, reason } => ("ban", targets, reason),
        AdminCommand::BanIp { target, reason } => ("ban-ip", target, reason),
        AdminCommand::Kick { targets, reason } => ("kick", targets, reason),
        _ => return Ok(Vec::new()),
    };
    let Some(_) = reason else {
        return Ok(Vec::new());
    };
    if target.starts_with('@') || target.starts_with('"') || target.starts_with('\'') {
        return Err("unported signed target-selector/string grammar");
    }
    let prefix = format!("{literal} {target} ");
    let raw = command
        .strip_prefix(&prefix)
        .ok_or("noncanonical signed argument spacing")?;
    if raw.is_empty() || raw.starts_with(' ') {
        return Err("unproved signed MessageArgument range");
    }
    if raw
        .as_bytes()
        .windows(2)
        .any(|pair| pair[0] == b'@' && b"paresn".contains(&pair[1]))
    {
        return Err("unported selector expansion in signed MessageArgument");
    }
    Ok(vec![SignableArgument {
        name: "reason".into(),
        value: raw.to_owned(),
    }])
}
