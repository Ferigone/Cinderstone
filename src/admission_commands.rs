//! Bounded dedicated-server administration, preserving Java 1.21 callback order.
//!
//! Target/profile resolution and every persistent/live owner belong to the host.
//! This module never manufactures an offline UUID or a replacement player.

use crate::access_lists::AccessProfile;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdminCommand {
    List {
        uuids: bool,
    },
    BanList(BanListKind),
    Ban {
        targets: String,
        reason: Option<String>,
    },
    Pardon {
        targets: String,
    },
    BanIp {
        target: String,
        reason: Option<String>,
    },
    PardonIp {
        target: String,
    },
    WhitelistOn,
    WhitelistOff,
    WhitelistList,
    WhitelistReload,
    WhitelistAdd {
        targets: String,
    },
    WhitelistRemove {
        targets: String,
    },
    Op {
        targets: String,
    },
    Deop {
        targets: String,
    },
    Kick {
        targets: String,
        reason: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BanListKind {
    All,
    Players,
    Ips,
}

#[derive(Clone, Debug)]
pub struct CommandSource {
    pub name: String,
    pub permission_level: u8,
}

#[derive(Clone, Debug)]
pub struct OnlinePlayer {
    pub profile: AccessProfile,
    /// Actual Connection peer address; no port, proxy header or fabricated IP.
    pub ip: String,
}

#[derive(Clone, Debug)]
pub struct BanListItem {
    pub display_name: String,
    pub source: String,
    pub reason: String,
}

#[derive(Clone, Debug)]
pub enum MessageArg {
    Text(String),
    Integer(i32),
    Profile(AccessProfile),
    /// The supported no-team Player display name, with actual profile identity.
    DisplayProfile(AccessProfile),
    Names(Vec<String>),
    Players {
        profiles: Vec<AccessProfile>,
        uuids: bool,
    },
    Component(Box<CommandMessage>),
}

#[derive(Clone, Debug)]
pub struct CommandMessage {
    /// Empty for a literal component; inspect `literal` before translation.
    pub key: &'static str,
    pub args: Vec<MessageArg>,
    pub literal: Option<String>,
}

impl CommandMessage {
    pub fn new(key: &'static str, args: Vec<MessageArg>) -> Self {
        Self {
            key,
            args,
            literal: None,
        }
    }
    pub fn plain_key(key: &'static str) -> Self {
        Self::new(key, Vec::new())
    }
    pub fn literal(text: impl Into<String>) -> Self {
        Self {
            key: "",
            args: Vec::new(),
            literal: Some(text.into()),
        }
    }

    pub fn json_component(&self) -> serde_json::Value {
        if let Some(text) = &self.literal {
            return serde_json::json!({"text":text});
        }
        let mut value = serde_json::json!({"translate":self.key});
        if !self.args.is_empty() {
            value["with"] =
                serde_json::Value::Array(self.args.iter().map(MessageArg::json_argument).collect());
        }
        value
    }

    pub fn network_component(&self) -> anyhow::Result<Vec<u8>> {
        encode_component_json(&self.json_component())
    }
}

fn uuid_text(uuid: &[u8; 16]) -> String {
    let hex: String = uuid.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

/// The Player.decorateDisplayNameComponent source path for the current
/// no-scoreboard-team profile. The host must not use it to stand in for an
/// unsupported custom/team display name.
pub fn default_player_display_component(profile: &AccessProfile) -> serde_json::Value {
    let id: Vec<i32> = profile
        .uuid
        .chunks_exact(4)
        .map(|b| i32::from_be_bytes([b[0], b[1], b[2], b[3]]))
        .collect();
    serde_json::json!({
        "text":profile.name,
        "insertion":profile.name,
        "clickEvent":{"action":"suggest_command","value":format!("/tell {}",profile.name)},
        "hoverEvent":{"action":"show_entity","contents":{"type":"minecraft:player","id":id,"name":{"text":profile.name}}}
    })
}

fn join_components(values: Vec<serde_json::Value>) -> serde_json::Value {
    if values.is_empty() {
        return serde_json::json!({"text":""});
    }
    if values.len() == 1 {
        return values.into_iter().next().unwrap();
    }
    let mut extra = Vec::new();
    for value in values {
        if !extra.is_empty() {
            extra.push(serde_json::json!({"text":", ","color":"gray"}));
        }
        extra.push(value);
    }
    serde_json::json!({"text":"","extra":extra})
}

impl MessageArg {
    fn json_argument(&self) -> serde_json::Value {
        match self {
            Self::Text(text) => serde_json::Value::String(text.clone()),
            Self::Integer(n) => serde_json::json!(*n),
            Self::Profile(profile) => serde_json::json!({"text":profile.name}),
            Self::DisplayProfile(profile) => default_player_display_component(profile),
            Self::Names(names) => join_components(names.iter().map(|name| serde_json::json!({"text":name})).collect()),
            Self::Players { profiles, uuids } => join_components(profiles.iter().map(|profile| {
                if *uuids { serde_json::json!({"translate":"commands.list.nameAndId","with":[{"text":profile.name},{"text":uuid_text(&profile.uuid)}]}) }
                else { default_player_display_component(profile) }
            }).collect()),
            Self::Component(component) => component.json_component(),
        }
    }
}

fn nbt_record(fields: Vec<(&str, crate::nbt::Tag)>) -> crate::nbt::Tag {
    crate::nbt::Tag::Compound(fields.into_iter().map(|(k, v)| (k.into(), v)).collect())
}

fn nbt_mixed_list(values: Vec<crate::nbt::Tag>) -> crate::nbt::Tag {
    use crate::nbt::Tag;
    if values.is_empty() {
        return Tag::List {
            element_type: 0,
            values,
        };
    }
    let first = values[0].tag_id();
    if values.iter().all(|v| v.tag_id() == first) {
        return match first {
            1 => Tag::ByteArray(
                values
                    .into_iter()
                    .map(|v| {
                        if let Tag::Byte(n) = v {
                            n
                        } else {
                            unreachable!()
                        }
                    })
                    .collect(),
            ),
            3 => Tag::IntArray(
                values
                    .into_iter()
                    .map(|v| {
                        if let Tag::Int(n) = v {
                            n
                        } else {
                            unreachable!()
                        }
                    })
                    .collect(),
            ),
            4 => Tag::LongArray(
                values
                    .into_iter()
                    .map(|v| {
                        if let Tag::Long(n) = v {
                            n
                        } else {
                            unreachable!()
                        }
                    })
                    .collect(),
            ),
            _ => Tag::List {
                element_type: first,
                values,
            },
        };
    }
    Tag::List {
        element_type: 10,
        values: values
            .into_iter()
            .map(|v| match &v {
                Tag::Compound(fields) if fields.len() != 1 || fields[0].0 != "" => v,
                _ => nbt_record(vec![("", v)]),
            })
            .collect(),
    }
}

fn argument_nbt(value: &serde_json::Value, depth: usize) -> anyhow::Result<crate::nbt::Tag> {
    use crate::nbt::Tag;
    Ok(match value {
        serde_json::Value::String(text) => Tag::String(text.clone().into()),
        serde_json::Value::Bool(value) => Tag::Byte(i8::from(*value)),
        serde_json::Value::Number(value) => {
            if let Some(number) = value.as_i64() {
                Tag::Int(i32::try_from(number)?)
            } else {
                Tag::Double(
                    value
                        .as_f64()
                        .ok_or_else(|| anyhow::anyhow!("invalid argument number"))?,
                )
            }
        }
        _ => component_nbt(value, depth + 1)?,
    })
}

fn component_nbt(value: &serde_json::Value, depth: usize) -> anyhow::Result<crate::nbt::Tag> {
    use crate::nbt::Tag;
    use anyhow::{bail, ensure, Context};
    ensure!(depth <= 32, "administration component nesting limit");
    if let Some(text) = value.as_str() {
        return Ok(Tag::String(text.into()));
    }
    let fields = value
        .as_object()
        .context("administration component requires object/string")?;
    let mut out = Vec::new();
    let mut content = false;
    for (key, value) in fields {
        let tag = match key.as_str() {
            "text" | "translate" | "fallback" | "color" | "insertion" => {
                if matches!(key.as_str(), "text" | "translate") {
                    content = true;
                }
                Tag::String(
                    value
                        .as_str()
                        .context("component text/style requires string")?
                        .into(),
                )
            }
            "bold" | "italic" | "underlined" | "strikethrough" | "obfuscated" => {
                Tag::Byte(i8::from(
                    value
                        .as_bool()
                        .context("component style requires boolean")?,
                ))
            }
            "with" => nbt_mixed_list(
                value
                    .as_array()
                    .context("with requires array")?
                    .iter()
                    .map(|v| argument_nbt(v, depth + 1))
                    .collect::<anyhow::Result<_>>()?,
            ),
            "extra" => {
                let values = value.as_array().context("extra requires array")?;
                ensure!(!values.is_empty(), "extra cannot be empty");
                nbt_mixed_list(
                    values
                        .iter()
                        .map(|v| component_nbt(v, depth + 1))
                        .collect::<anyhow::Result<_>>()?,
                )
            }
            "clickEvent" => {
                let fields = value.as_object().context("clickEvent requires object")?;
                ensure!(
                    fields.len() == 2
                        && fields.get("action").and_then(|v| v.as_str()) == Some("suggest_command"),
                    "unsupported admin clickEvent"
                );
                nbt_record(vec![
                    ("action", Tag::String("suggest_command".into())),
                    (
                        "value",
                        Tag::String(
                            fields
                                .get("value")
                                .and_then(|v| v.as_str())
                                .context("click value requires string")?
                                .into(),
                        ),
                    ),
                ])
            }
            "hoverEvent" => {
                let fields = value.as_object().context("hoverEvent requires object")?;
                ensure!(
                    fields.len() == 2
                        && fields.get("action").and_then(|v| v.as_str()) == Some("show_entity"),
                    "unsupported admin hoverEvent"
                );
                let c = fields
                    .get("contents")
                    .and_then(|v| v.as_object())
                    .context("hover contents required")?;
                ensure!(
                    c.len() == 3
                        && c.get("type").and_then(|v| v.as_str()) == Some("minecraft:player"),
                    "unsupported hover entity context"
                );
                let id = c
                    .get("id")
                    .and_then(|v| v.as_array())
                    .context("hover UUID int array required")?;
                ensure!(id.len() == 4, "hover UUID requires four integers");
                let id = id
                    .iter()
                    .map(|v| {
                        Ok(i32::try_from(
                            v.as_i64().context("hover UUID integer required")?,
                        )?)
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?;
                let contents = nbt_record(vec![
                    ("type", Tag::String("minecraft:player".into())),
                    ("id", Tag::IntArray(id)),
                    (
                        "name",
                        component_nbt(c.get("name").context("hover name required")?, depth + 1)?,
                    ),
                ]);
                nbt_record(vec![
                    ("action", Tag::String("show_entity".into())),
                    ("contents", contents),
                ])
            }
            _ => bail!("unsupported administration component field {key}"),
        };
        out.push((key.as_str().into(), tag));
    }
    ensure!(content, "component has no text or translation");
    Ok(Tag::Compound(out))
}

/// Encode only the native administration component shapes produced above and
/// their known color/italic admin wrappers. This is not a general Component codec.
pub fn encode_component_json(value: &serde_json::Value) -> anyhow::Result<Vec<u8>> {
    component_nbt(value, 0)?.encode_network_anonymous()
}

#[derive(Clone, Debug)]
pub enum CommandError {
    Message(CommandMessage),
    Syntax { cursor: usize, detail: &'static str },
    Unsupported(&'static str),
    Host(String),
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Message(m) => write!(f, "{}", m.key),
            Self::Syntax { cursor, detail } => write!(f, "{detail} at {cursor}"),
            Self::Unsupported(s) => write!(f, "unsupported administration context: {s}"),
            Self::Host(s) => f.write_str(s),
        }
    }
}
impl std::error::Error for CommandError {}

/// Synchronous actual-owner callbacks. A caller may record effects in this exact
/// invocation order and apply them on its one runtime owner after releasing locks.
/// StoredUserList save failures are logged by Java rather than rolling back: a
/// host must preserve that behavior, reporting a warning without aborting a
/// successfully applied in-memory mutation.
pub trait AdministrationHost {
    fn resolve_profiles(&mut self, target: &str) -> Result<Vec<AccessProfile>, CommandError>;
    fn resolve_players(&mut self, target: &str) -> Result<Vec<OnlinePlayer>, CommandError>;
    fn online_players(&self) -> Vec<OnlinePlayer>;
    fn max_players(&self) -> i32;
    fn ban_entries(&self, kind: BanListKind) -> Vec<BanListItem>;
    fn is_player_banned(&self, profile: &AccessProfile) -> bool;
    fn is_ip_banned(&self, ip: &str) -> bool;
    fn is_operator(&self, profile: &AccessProfile) -> bool;
    fn is_whitelisted(&self, profile: &AccessProfile) -> bool;
    fn whitelist_names(&self) -> Vec<String>;
    fn uses_whitelist(&self) -> bool;
    fn server_is_published(&self) -> bool;
    fn is_singleplayer_owner(&self, profile: &AccessProfile) -> bool;

    /// Insert and save; return the actual stored reason, including default text.
    fn ban_player(
        &mut self,
        profile: &AccessProfile,
        source: &str,
        reason: Option<&str>,
    ) -> Result<String, CommandError>;
    fn pardon_player(&mut self, profile: &AccessProfile) -> Result<(), CommandError>;
    fn ban_ip(
        &mut self,
        ip: &str,
        source: &str,
        reason: Option<&str>,
    ) -> Result<String, CommandError>;
    fn pardon_ip(&mut self, ip: &str) -> Result<(), CommandError>;
    /// Includes the real permission-level packet and command-tree refresh.
    fn set_operator(&mut self, profile: &AccessProfile, value: bool) -> Result<(), CommandError>;
    fn set_whitelisted(&mut self, profile: &AccessProfile, value: bool)
        -> Result<(), CommandError>;
    fn set_uses_whitelist(&mut self, value: bool) -> Result<(), CommandError>;
    fn reload_whitelist(&mut self) -> Result<(), CommandError>;
    /// MinecraftServer.kickUnlistedPlayers, including its distinct
    /// enforce-whitelist/raw-list predicate (not the admission whitelist test).
    fn enforce_whitelist(&mut self) -> Result<(), CommandError>;
    fn feedback(&mut self, message: CommandMessage, broadcast_to_ops: bool);
    fn disconnect(&mut self, profile: &AccessProfile, reason: CommandMessage);
}

pub fn required_permission(command: &AdminCommand) -> u8 {
    match command {
        AdminCommand::List { .. } => 0,
        _ => 3,
    }
}

impl AdminCommand {
    /// Minecraft's MessageArgument is signable even if its current content is
    /// plain text. The unsigned PLAY handler must apply enforce-secure-profile.
    pub fn has_signable_arguments(&self) -> bool {
        matches!(
            self,
            Self::Ban {
                reason: Some(_),
                ..
            } | Self::BanIp {
                reason: Some(_),
                ..
            } | Self::Kick {
                reason: Some(_),
                ..
            }
        )
    }
}

fn syntax(cursor: usize, detail: &'static str) -> CommandError {
    CommandError::Syntax { cursor, detail }
}

fn split_word(input: &str) -> (&str, &str) {
    match input.find(' ') {
        Some(i) => (&input[..i], input[i..].trim_start_matches(' ')),
        None => (input, ""),
    }
}

fn one_argument(input: &str, cursor: usize) -> Result<String, CommandError> {
    let (word, rest) = split_word(input);
    if word.is_empty() {
        return Err(syntax(cursor, "Expected argument"));
    }
    if !rest.is_empty() {
        return Err(syntax(
            cursor + word.len() + 1,
            "Unexpected trailing argument",
        ));
    }
    Ok(word.to_owned())
}

fn word_argument(input: &str, cursor: usize) -> Result<String, CommandError> {
    let word = one_argument(input, cursor)?;
    if let Some(index) = word
        .bytes()
        .position(|b| !b.is_ascii_alphanumeric() && !b"_-.+".contains(&b))
    {
        return Err(syntax(
            cursor + index,
            "Unexpected character in unquoted word",
        ));
    }
    Ok(word)
}

fn target_and_reason(input: &str, cursor: usize) -> Result<(String, Option<String>), CommandError> {
    let (target, rest) = split_word(input);
    if target.is_empty() {
        return Err(syntax(cursor, "Expected target"));
    }
    // MessageArgument may expand player selectors. Until a native message
    // resolver is supplied, never interpret a selector-bearing reason as plain
    // text and silently change its meaning.
    if rest
        .as_bytes()
        .windows(2)
        .any(|w| w[0] == b'@' && b"paresn".contains(&w[1]))
    {
        return Err(CommandError::Unsupported(
            "selectors in MessageArgument reasons",
        ));
    }
    Ok((
        target.to_owned(),
        (!rest.is_empty()).then(|| rest.to_owned()),
    ))
}

/// A deliberately bounded parser for these vanilla command literals. It accepts
/// a single optional leading slash, ASCII command separators and one host-owned
/// target token. It does not claim Brigadier cursor/diagnostic equivalence,
/// arbitrary selectors, quoted-string escapes or the complete command language.
pub fn parse_command(input: &str) -> Result<AdminCommand, CommandError> {
    let input = input.strip_prefix('/').unwrap_or(input);
    if input.chars().any(char::is_control) {
        return Err(syntax(
            0,
            "Control characters are not supported in commands",
        ));
    }
    let (root, rest) = split_word(input);
    let cursor = root.len() + 1;
    Ok(match root {
        "list" => match rest.trim_end_matches(' ') {
            "" => AdminCommand::List { uuids: false },
            "uuids" => AdminCommand::List { uuids: true },
            _ => return Err(syntax(cursor, "Expected uuids or end of command")),
        },
        "banlist" => AdminCommand::BanList(match rest.trim_end_matches(' ') {
            "" => BanListKind::All,
            "players" => BanListKind::Players,
            "ips" => BanListKind::Ips,
            _ => return Err(syntax(cursor, "Expected players, ips or end of command")),
        }),
        "ban" => {
            let (targets, reason) = target_and_reason(rest, cursor)?;
            AdminCommand::Ban { targets, reason }
        }
        "pardon" => AdminCommand::Pardon {
            targets: one_argument(rest, cursor)?,
        },
        "ban-ip" => {
            let (target, reason) = target_and_reason(rest, cursor)?;
            let target = word_argument(&target, cursor)?;
            AdminCommand::BanIp { target, reason }
        }
        "pardon-ip" => AdminCommand::PardonIp {
            target: word_argument(rest, cursor)?,
        },
        "op" => AdminCommand::Op {
            targets: one_argument(rest, cursor)?,
        },
        "deop" => AdminCommand::Deop {
            targets: one_argument(rest, cursor)?,
        },
        "kick" => {
            let (targets, reason) = target_and_reason(rest, cursor)?;
            AdminCommand::Kick { targets, reason }
        }
        "whitelist" => {
            let (verb, args) = split_word(rest);
            let simple = match verb {
                "on" => Some(AdminCommand::WhitelistOn),
                "off" => Some(AdminCommand::WhitelistOff),
                "list" => Some(AdminCommand::WhitelistList),
                "reload" => Some(AdminCommand::WhitelistReload),
                _ => None,
            };
            if let Some(command) = simple {
                if !args.is_empty() {
                    return Err(syntax(
                        cursor + verb.len() + 1,
                        "Unexpected trailing argument",
                    ));
                }
                command
            } else {
                match verb {
                    "add" => AdminCommand::WhitelistAdd {
                        targets: one_argument(args, cursor + 4)?,
                    },
                    "remove" => AdminCommand::WhitelistRemove {
                        targets: one_argument(args, cursor + 7)?,
                    },
                    _ => return Err(syntax(cursor, "Unknown whitelist subcommand")),
                }
            }
        }
        _ => {
            return Err(CommandError::Message(CommandMessage::plain_key(
                "command.unknown.command",
            )))
        }
    })
}

fn failure(key: &'static str) -> CommandError {
    CommandError::Message(CommandMessage::plain_key(key))
}

fn require_changed(count: i32, key: &'static str) -> Result<i32, CommandError> {
    if count == 0 {
        Err(failure(key))
    } else {
        Ok(count)
    }
}

/// Execute against the real host in the original per-target mutation, feedback
/// and disconnect order. Mutation callbacks include StoredUserList persistence.
pub fn execute<H: AdministrationHost>(
    host: &mut H,
    source: &CommandSource,
    command: AdminCommand,
) -> Result<i32, CommandError> {
    if source.permission_level < required_permission(&command) {
        // In Brigadier an inaccessible literal is absent from the executable
        // tree, rather than producing a newly invented administration error.
        return Err(failure("command.unknown.command"));
    }
    let is_op = matches!(&command, AdminCommand::Op { .. });
    let is_whitelist_add = matches!(&command, AdminCommand::WhitelistAdd { .. });
    match command {
        AdminCommand::List { uuids } => {
            let players = host.online_players();
            let count = players.len() as i32;
            host.feedback(
                CommandMessage::new(
                    "commands.list.players",
                    vec![
                        MessageArg::Integer(count),
                        MessageArg::Integer(host.max_players()),
                        MessageArg::Players {
                            profiles: players.into_iter().map(|p| p.profile).collect(),
                            uuids,
                        },
                    ],
                ),
                false,
            );
            Ok(count)
        }
        AdminCommand::BanList(kind) => {
            let entries = host.ban_entries(kind);
            let count = entries.len() as i32;
            if entries.is_empty() {
                host.feedback(CommandMessage::plain_key("commands.banlist.none"), false);
            } else {
                host.feedback(
                    CommandMessage::new("commands.banlist.list", vec![MessageArg::Integer(count)]),
                    false,
                );
                for entry in entries {
                    host.feedback(
                        CommandMessage::new(
                            "commands.banlist.entry",
                            vec![
                                MessageArg::Text(entry.display_name),
                                MessageArg::Text(entry.source),
                                MessageArg::Text(entry.reason),
                            ],
                        ),
                        false,
                    );
                }
            }
            Ok(count)
        }
        AdminCommand::Ban { targets, reason } => {
            let profiles = host.resolve_profiles(&targets)?;
            let mut count = 0;
            for profile in profiles {
                if host.is_player_banned(&profile) {
                    continue;
                }
                let stored_reason = host.ban_player(&profile, &source.name, reason.as_deref())?;
                count += 1;
                host.feedback(
                    CommandMessage::new(
                        "commands.ban.success",
                        vec![
                            MessageArg::Profile(profile.clone()),
                            MessageArg::Text(stored_reason),
                        ],
                    ),
                    true,
                );
                if host
                    .online_players()
                    .iter()
                    .any(|p| p.profile.uuid == profile.uuid)
                {
                    host.disconnect(
                        &profile,
                        CommandMessage::plain_key("multiplayer.disconnect.banned"),
                    );
                }
            }
            require_changed(count, "commands.ban.failed")
        }
        AdminCommand::Pardon { targets } => {
            let profiles = host.resolve_profiles(&targets)?;
            let mut count = 0;
            for profile in profiles {
                if !host.is_player_banned(&profile) {
                    continue;
                }
                host.pardon_player(&profile)?;
                count += 1;
                host.feedback(
                    CommandMessage::new(
                        "commands.pardon.success",
                        vec![MessageArg::Profile(profile)],
                    ),
                    true,
                );
            }
            require_changed(count, "commands.pardon.failed")
        }
        AdminCommand::BanIp { target, reason } => {
            let players = host.online_players();
            let ip = if target.parse::<std::net::IpAddr>().is_ok() {
                target
            } else if let Some(player) = players
                .iter()
                .find(|p| p.profile.name.eq_ignore_ascii_case(&target))
            {
                player.ip.clone()
            } else {
                return Err(failure("commands.banip.invalid"));
            };
            if host.is_ip_banned(&ip) {
                return Err(failure("commands.banip.failed"));
            }
            let affected: Vec<_> = players.into_iter().filter(|p| p.ip == ip).collect();
            let stored_reason = host.ban_ip(&ip, &source.name, reason.as_deref())?;
            host.feedback(
                CommandMessage::new(
                    "commands.banip.success",
                    vec![MessageArg::Text(ip), MessageArg::Text(stored_reason)],
                ),
                true,
            );
            if !affected.is_empty() {
                host.feedback(
                    CommandMessage::new(
                        "commands.banip.info",
                        vec![
                            MessageArg::Integer(affected.len() as i32),
                            MessageArg::Players {
                                profiles: affected.iter().map(|p| p.profile.clone()).collect(),
                                uuids: false,
                            },
                        ],
                    ),
                    true,
                );
            }
            for player in &affected {
                host.disconnect(
                    &player.profile,
                    CommandMessage::plain_key("multiplayer.disconnect.ip_banned"),
                );
            }
            // BanIpCommands returns disconnected player count, including zero
            // when it successfully bans an address with nobody currently online.
            Ok(affected.len() as i32)
        }
        AdminCommand::PardonIp { target } => {
            if target.parse::<std::net::IpAddr>().is_err() {
                return Err(failure("commands.pardonip.invalid"));
            }
            if !host.is_ip_banned(&target) {
                return Err(failure("commands.pardonip.failed"));
            }
            host.pardon_ip(&target)?;
            host.feedback(
                CommandMessage::new("commands.pardonip.success", vec![MessageArg::Text(target)]),
                true,
            );
            Ok(1)
        }
        AdminCommand::Op { targets } | AdminCommand::Deop { targets } => {
            let enable = is_op;
            let profiles = host.resolve_profiles(&targets)?;
            let first_name = profiles.first().map(|p| p.name.clone()).unwrap_or_default();
            let mut count = 0;
            for profile in profiles {
                if host.is_operator(&profile) == enable {
                    continue;
                }
                host.set_operator(&profile, enable)?;
                count += 1;
                // Both original lambda helpers capture the target collection
                // and use iterator.next(), even when a later target changed.
                host.feedback(
                    CommandMessage::new(
                        if enable {
                            "commands.op.success"
                        } else {
                            "commands.deop.success"
                        },
                        vec![MessageArg::Text(first_name.clone())],
                    ),
                    true,
                );
            }
            require_changed(
                count,
                if enable {
                    "commands.op.failed"
                } else {
                    "commands.deop.failed"
                },
            )?;
            if !enable {
                host.enforce_whitelist()?;
            }
            Ok(count)
        }
        AdminCommand::WhitelistOn => {
            if host.uses_whitelist() {
                return Err(failure("commands.whitelist.alreadyOn"));
            }
            host.set_uses_whitelist(true)?;
            host.feedback(
                CommandMessage::plain_key("commands.whitelist.enabled"),
                true,
            );
            host.enforce_whitelist()?;
            Ok(1)
        }
        AdminCommand::WhitelistOff => {
            if !host.uses_whitelist() {
                return Err(failure("commands.whitelist.alreadyOff"));
            }
            host.set_uses_whitelist(false)?;
            host.feedback(
                CommandMessage::plain_key("commands.whitelist.disabled"),
                true,
            );
            Ok(1)
        }
        AdminCommand::WhitelistList => {
            let names = host.whitelist_names();
            let count = names.len() as i32;
            let message = if names.is_empty() {
                CommandMessage::plain_key("commands.whitelist.none")
            } else {
                CommandMessage::new(
                    "commands.whitelist.list",
                    vec![
                        MessageArg::Integer(count),
                        MessageArg::Text(names.join(", ")),
                    ],
                )
            };
            host.feedback(message, false);
            Ok(count)
        }
        AdminCommand::WhitelistReload => {
            host.reload_whitelist()?;
            host.feedback(
                CommandMessage::plain_key("commands.whitelist.reloaded"),
                true,
            );
            host.enforce_whitelist()?;
            Ok(1)
        }
        AdminCommand::WhitelistAdd { targets } | AdminCommand::WhitelistRemove { targets } => {
            let enable = is_whitelist_add;
            let profiles = host.resolve_profiles(&targets)?;
            let mut count = 0;
            for profile in profiles {
                if host.is_whitelisted(&profile) == enable {
                    continue;
                }
                host.set_whitelisted(&profile, enable)?;
                host.feedback(
                    CommandMessage::new(
                        if enable {
                            "commands.whitelist.add.success"
                        } else {
                            "commands.whitelist.remove.success"
                        },
                        vec![MessageArg::Profile(profile)],
                    ),
                    true,
                );
                count += 1;
            }
            require_changed(
                count,
                if enable {
                    "commands.whitelist.add.failed"
                } else {
                    "commands.whitelist.remove.failed"
                },
            )?;
            if !enable {
                host.enforce_whitelist()?;
            }
            Ok(count)
        }
        AdminCommand::Kick { targets, reason } => {
            let players = host.resolve_players(&targets)?;
            if !host.server_is_published() {
                return Err(failure("commands.kick.singleplayer.failed"));
            }
            let reason = reason
                .map(CommandMessage::literal)
                .unwrap_or_else(|| CommandMessage::plain_key("multiplayer.disconnect.kicked"));
            let mut count = 0;
            for player in players {
                if host.is_singleplayer_owner(&player.profile) {
                    continue;
                }
                host.disconnect(&player.profile, reason.clone());
                host.feedback(
                    CommandMessage::new(
                        "commands.kick.success",
                        vec![
                            MessageArg::DisplayProfile(player.profile),
                            MessageArg::Component(Box::new(reason.clone())),
                        ],
                    ),
                    true,
                );
                count += 1;
            }
            require_changed(count, "commands.kick.owner.failed")
        }
    }
}
