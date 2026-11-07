//! Native production chat owners for actual admitted players/connections.
//!
//! Main supplies the configured registry holder, current service keys, time and
//! operator state. This runtime represents the dedicated no external
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

    pub(super) fn append_system(&mut self, recipient: i32, component: &Value) {
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
    pub decorated_names: BTreeMap<i32, Value>,
}

impl MainChatRuntime {
    pub(super) fn accepts_system_messages(&self, entity_id: i32) -> bool {
        self.connections.get(&entity_id).is_some_and(|entry| entry.settings.visibility != ChatVisibility::Hidden)
    }

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
        self.decorated_names.remove(&entity_id);
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
    /// has run. Main supplies the current scoreboard-decorated sender name.
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
            name: self.decorated_names.get(&entity_id).cloned().unwrap_or_else(|| admission_commands::default_player_display_component(&source)),
            target_name: None,
        };
        self.broadcast(
            Some(entity_id),
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
        sender_id: Option<i32>,
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
        let audience = self.registration_order.iter().map(|id| (*id, bound.clone())).collect();
        self.deliver_audience(sender_id, sender_filtering, message, audience, outcome);
    }

    pub(super) fn send_say_message(&mut self,sender:Option<i32>,message:PlayerChatMessage,
        bound:BoundChatType,now_ms:i64)->Result<MainChatOutcome>{
        let filtering=match sender {Some(id)=>self.connection_mut(id)?.settings.text_filtering_enabled,None=>false};
        let mut outcome=MainChatOutcome::default();
        self.broadcast(sender,filtering,message,bound,now_ms,&mut outcome);Ok(outcome)
    }

    pub(super) fn send_team_message(&mut self, sender_id: i32,
        members: &std::collections::BTreeSet<i32>, message: PlayerChatMessage,
        incoming: BoundChatType, outgoing: BoundChatType) -> Result<MainChatOutcome> {
        let sender_filtering = self.connection_mut(sender_id)?.settings.text_filtering_enabled;
        let audience = self.registration_order.iter()
            .filter(|id| **id == sender_id || members.contains(id))
            .map(|id| (*id, if *id == sender_id { outgoing.clone() } else { incoming.clone() }))
            .collect();
        let mut outcome = MainChatOutcome::default();
        self.deliver_audience(Some(sender_id), sender_filtering, message, audience, &mut outcome);
        Ok(outcome)
    }

    fn deliver_audience(&mut self, sender_id: Option<i32>, sender_filtering: bool,
        message: PlayerChatMessage, audience: Vec<(i32, BoundChatType)>, outcome: &mut MainChatOutcome) {
        let mut filtered_full = false;
        for (recipient, bound) in audience {
            let Some(entry) = self.connections.get_mut(&recipient) else {
                outcome.error = Some("registered chat audience owner disappeared".into());
                return;
            };
            // ServerPlayer.shouldFilterMessageTo excludes the sender itself.
            let filtering = Some(recipient) != sender_id
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
        if let Some(sender_id) = sender_id.filter(|_|filtered_full) {
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
    if mc121_rust::commands::say_command::is_command(command) {
        return mc121_rust::commands::say_command::has_message(command,permission_level);
    }
    if super::main_team_chat::is_command(command) {
        // An unsupported selector must never become an unsigned signing exemption.
        return !matches!(super::main_team_chat::literal_argument(command), Ok(None));
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
    if mc121_rust::commands::say_command::is_command(command) {
        return match mc121_rust::commands::say_command::execute(command,permission_level.clamp(0,4) as u8) {
            Ok(value)=>Ok(vec![SignableArgument{name:mc121_rust::commands::say_command::MessageCommand::from_line(command).expect("known message command").argument().into(),value}]),
            Err(mc121_rust::commands::say_command::SayError::Syntax(_))=>Ok(Vec::new()),
            Err(_)=>Err("unported selector expansion in broadcast MessageArgument"),
        };
    }
    if super::main_team_chat::is_command(command) {
        return Ok(super::main_team_chat::literal_argument(command)?.map(|value|
            SignableArgument { name: "message".into(), value: value.to_owned() }).into_iter().collect());
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

#[cfg(test)]
mod team_signing_tests {
    use super::*;
    #[test]
    fn say_signing_preserves_original_literal_and_refuses_unsigned_secure_messages(){
        let line="say   cześć  \"world\" 😀  ";
        assert_eq!(known_signable_arguments(line,2).unwrap(),vec![SignableArgument{name:"message".into(),value:"  cześć  \"world\" 😀  ".into()}]);
        assert!(unsigned_has_signable_arguments(line,2));
        assert!(!unsigned_has_signable_arguments(line,0));
        assert!(known_signable_arguments(line,0).unwrap().is_empty());
        assert!(unsigned_has_signable_arguments("say @a",2));
        assert!(known_signable_arguments("say @a",2).is_err());
        assert!(!unsigned_has_signable_arguments("say ",2));
        let mut chat=MainChatRuntime::new();chat.register(1,&PlayerProfile::offline("Alice").unwrap(),MainChatSettings::default()).unwrap();
        let host=MainChatContext{now_ms:1,enforce_secure_profile:true,is_operator:true,is_singleplayer_owner:false};
        for command in [line,"say @a"]{let outcome=chat.handle_unsigned_command(1,command,2,host).unwrap();assert!(outcome.command.is_none());assert!(!outcome.effects.is_empty());}
    }
    #[test]
    fn nonop_emote_signs_action_and_cannot_bypass_secure_unsigned_gate(){
        let line="me   cześć  \"world\" 😀  ";
        assert_eq!(known_signable_arguments(line,0).unwrap(),vec![SignableArgument{name:"action".into(),value:"  cześć  \"world\" 😀  ".into()}]);
        assert!(unsigned_has_signable_arguments(line,0));assert!(unsigned_has_signable_arguments("me @a",0));assert!(known_signable_arguments("me @a",0).is_err());
        let mut chat=MainChatRuntime::new();chat.register(1,&PlayerProfile::offline("Alice").unwrap(),MainChatSettings::default()).unwrap();
        let host=MainChatContext{now_ms:1,enforce_secure_profile:true,is_operator:false,is_singleplayer_owner:false};
        for command in [line,"me @a"]{let outcome=chat.handle_unsigned_command(1,command,0,host).unwrap();assert!(outcome.command.is_none());assert!(!outcome.effects.is_empty());}
    }
    #[test]
    fn say_signed_delivery_keeps_actual_cache_and_visibility_owners(){
        for registry_id in [1,6] {
        let mut chat=MainChatRuntime::new();let profile=PlayerProfile::offline("Alice").unwrap();
        for (id,name) in [(1,"Alice"),(2,"Bob"),(3,"Cara")]{chat.register(id,&PlayerProfile::offline(name).unwrap(),MainChatSettings::default()).unwrap();}
        chat.connections.get_mut(&3).unwrap().settings.visibility=ChatVisibility::System;
        let baseline=chat.connections[&1].listener.tracked_messages_count();
        // Delivery boundary fixture; cryptographic authorization stays in the listener tests.
        let mut message=PlayerChatMessage::unsigned(profile.uuid,"say signed fixture",1);message.signature=Some([9;256]);
        let bound=BoundChatType{registry_id,name:json!("Alice"),target_name:None};let outcome=chat.send_say_message(Some(1),message,bound,1).unwrap();
        assert_eq!(outcome.effects.iter().filter_map(|e|if let MainChatEffect::Packet{recipient,..}=e{Some(*recipient)}else{None}).collect::<Vec<_>>(),vec![1,2]);
        for id in [1,2]{assert!(chat.connections[&id].signature_cache.index_of(&[9;256])>=0);assert_eq!(chat.connections[&id].listener.tracked_messages_count(),baseline+1);}
        assert_eq!(chat.connections[&3].signature_cache.index_of(&[9;256]),-1);assert_eq!(chat.connections[&3].listener.tracked_messages_count(),baseline);
        }
    }
    #[test]
    fn nonop_team_message_uses_original_range_and_unsupported_ranges_cannot_bypass_signing() {
        let line = "tm   cześć  team ";
        assert_eq!(known_signable_arguments(line, 0).unwrap(), vec![SignableArgument {
            name: "message".into(), value: "  cześć  team ".into(),
        }]);
        assert!(unsigned_has_signable_arguments(line, 0));
        assert!(unsigned_has_signable_arguments("teammsg @a", 0));
        assert!(known_signable_arguments("teammsg @a", 0).is_err());
        assert!(!unsigned_has_signable_arguments("teammsg", 0));
        assert!(known_signable_arguments("/tm hi", 0).unwrap().is_empty());
    }
    #[test]
    fn team_delivery_advances_only_actual_recipients_signature_owners() {
        let mut chat = MainChatRuntime::new();
        let mut profiles = Vec::new();
        for (id, name) in [(1, "Alice"), (2, "Bob"), (3, "Cara")] {
            let profile = PlayerProfile::offline(name).unwrap();
            chat.register(id, &profile, MainChatSettings::default()).unwrap();
            profiles.push(profile);
        }
        // Delivery-boundary fixture, not a claim that this synthetic signature
        // has passed the separate cryptographic authorization owner.
        let mut message = PlayerChatMessage::unsigned(profiles[0].uuid, "team fixture", 1);
        message.signature = Some([7; 256]);
        let baseline = chat.connections[&1].listener.tracked_messages_count();
        let bound = BoundChatType { registry_id: 0, name: json!({"text":"Alice"}), target_name: Some(json!({"text":"team"})) };
        let outcome = chat.send_team_message(1, &std::collections::BTreeSet::from([1, 2]), message, bound.clone(), bound).unwrap();
        let recipients = outcome.effects.iter().filter_map(|effect| match effect {
            MainChatEffect::Packet { recipient, .. } => Some(*recipient), _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(recipients, vec![1, 2]);
        assert!(!outcome.effects.iter().any(|e| matches!(e, MainChatEffect::ChatLog { .. })));
        for id in [1, 2] {
            let connection = &chat.connections[&id];
            assert!(connection.signature_cache.index_of(&[7; 256]) >= 0);
            assert_eq!(connection.listener.tracked_messages_count(), baseline + 1);
        }
        assert_eq!(chat.connections[&3].signature_cache.index_of(&[7; 256]), -1);
        assert_eq!(chat.connections[&3].listener.tracked_messages_count(), baseline);
        chat.connections.get_mut(&2).unwrap().settings.visibility = ChatVisibility::System;
        let mut message = PlayerChatMessage::unsigned(profiles[0].uuid, "hidden member", 2);
        message.signature = Some([8; 256]);
        let bound = BoundChatType { registry_id: 0, name: json!("Alice"), target_name: None };
        chat.send_team_message(1, &std::collections::BTreeSet::from([1, 2]), message, bound.clone(), bound).unwrap();
        assert_eq!(chat.connections[&2].signature_cache.index_of(&[8; 256]), -1);
        assert_eq!(chat.connections[&2].listener.tracked_messages_count(), baseline + 1);
    }
    #[test]
    fn secure_unsigned_team_commands_are_refused_even_for_unsupported_selectors() {
        let mut chat = MainChatRuntime::new();
        chat.register(1, &PlayerProfile::offline("Alice").unwrap(), MainChatSettings::default()).unwrap();
        let host = MainChatContext { now_ms: 1, enforce_secure_profile: true, is_operator: false, is_singleplayer_owner: false };
        for command in ["teammsg hello", "tm @a"] {
            let outcome = chat.handle_unsigned_command(1, command, 0, host).unwrap();
            assert!(outcome.command.is_none());
            assert!(!outcome.effects.is_empty());
        }
    }
}
