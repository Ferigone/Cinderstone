//! Actual-owner bridge for the bounded native dedicated administration stage.
//!
//! The owner is synchronously locked by Main. It never retains a network writer,
//! fabricates a player/profile, or applies effects to a replacement connection.
//! Main must apply `CommandOutcome.effects` in order even when `result` is Err:
//! a later failure does not undo earlier Java command callbacks.

use anyhow::{Context, Result};
use mc121_rust::{
    access_lists::{
        self, AccessLists, AccessProfile, BanMetadata, IpBanEntry, OperatorEntry, PlayerBanEntry,
    },
    admission_commands::{
        self, AdminCommand, AdministrationHost, BanListItem, BanListKind, CommandError,
        CommandMessage, CommandSource, OnlinePlayer,
    },
    admission_policy::{self, AdmissionRejection, BanLookup, BanView},
};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    net::SocketAddr,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct AdminSettings {
    pub max_players: i32,
    pub whitelist_enabled: bool,
    pub enforce_whitelist: bool,
    pub op_permission_level: i32,
}

impl Default for AdminSettings {
    fn default() -> Self {
        Self {
            max_players: 20,
            whitelist_enabled: false,
            enforce_whitelist: false,
            op_permission_level: 4,
        }
    }
}

/// Main supplies this from the actual admitted registry and accepted TCP peer.
#[derive(Clone, Debug)]
pub struct OnlineProfile {
    pub entity_id: i32,
    pub profile: AccessProfile,
    pub address: SocketAddr,
}

#[derive(Clone, Debug)]
pub enum AdminEffect {
    PersistenceWarning {
        file: &'static str,
        message: String,
    },
    Feedback {
        message: CommandMessage,
        broadcast_to_ops: bool,
    },
    Disconnect {
        entity_id: i32,
        uuid: [u8; 16],
        reason: CommandMessage,
    },
    /// Main sends the actual player permission event and refreshed command tree.
    PermissionLevel {
        entity_id: i32,
        uuid: [u8; 16],
        level: i32,
    },
}

#[derive(Debug)]
pub struct CommandOutcome {
    pub result: std::result::Result<i32, CommandError>,
    pub effects: Vec<AdminEffect>,
}

#[derive(Debug)]
pub struct Administration {
    directory: PathBuf,
    settings: AdminSettings,
    lists: AccessLists,
    /// Names observed from an actual selected login/profile or a loaded list.
    /// This is not a replacement for the unported Mojang user-cache resolver.
    known_profiles: BTreeMap<String, AccessProfile>,
    startup_warnings: Vec<AdminEffect>,
}

impl Administration {
    pub fn load(
        directory: impl Into<PathBuf>,
        mut settings: AdminSettings,
        now_ms: i64,
    ) -> Result<Self> {
        let directory = directory.into();
        fs::create_dir_all(&directory)
            .with_context(|| format!("create administration directory {}", directory.display()))?;
        let mut startup_warnings = Vec::new();
        match property_value(&directory.join("server.properties"), "white-list") {
            Ok(Some(value)) => settings.whitelist_enabled = value.eq_ignore_ascii_case("true"),
            Ok(None) => {}
            Err(error) if error.downcast_ref::<std::io::Error>().is_some() => {
                startup_warnings.push(AdminEffect::PersistenceWarning {
                    file: "server.properties",
                    message: format!("load failed; using supplied defaults: {error:#}"),
                });
            }
            Err(error) => return Err(error),
        }
        // DedicatedPlayerList constructor order and catch boundaries. The
        // unconditional saves create/normalize the actual lists; malformed ban
        // data is not swallowed as though it were an IOException.
        let mut lists = AccessLists::default();
        startup_io_warning(
            &mut startup_warnings,
            access_lists::PLAYER_BANS_FILE,
            lists.load_players(&directory, now_ms),
        )?;
        startup_io_warning(
            &mut startup_warnings,
            access_lists::PLAYER_BANS_FILE,
            lists.save_players(&directory),
        )?;
        startup_io_warning(
            &mut startup_warnings,
            access_lists::IP_BANS_FILE,
            lists.load_ips(&directory, now_ms),
        )?;
        startup_io_warning(
            &mut startup_warnings,
            access_lists::IP_BANS_FILE,
            lists.save_ips(&directory),
        )?;
        startup_any_warning(
            &mut startup_warnings,
            access_lists::OPERATORS_FILE,
            lists.load_ops(&directory, now_ms),
        );
        startup_any_warning(
            &mut startup_warnings,
            access_lists::WHITELIST_FILE,
            lists.load_whitelist(&directory, now_ms),
        );
        startup_any_warning(
            &mut startup_warnings,
            access_lists::OPERATORS_FILE,
            lists.save_ops(&directory),
        );
        if !directory.join(access_lists::WHITELIST_FILE).try_exists()? {
            startup_any_warning(
                &mut startup_warnings,
                access_lists::WHITELIST_FILE,
                lists.save_whitelist(&directory),
            );
        }
        let mut owner = Self {
            directory,
            settings,
            lists,
            known_profiles: BTreeMap::new(),
            startup_warnings,
        };
        let profiles = owner
            .lists
            .player_bans
            .values()
            .map(|entry| entry.profile.clone())
            .chain(
                owner
                    .lists
                    .operators
                    .values()
                    .map(|entry| entry.profile.clone()),
            )
            .chain(owner.lists.whitelist.values().cloned())
            .collect::<Vec<_>>();
        for profile in profiles {
            owner.remember_profile(profile);
        }
        Ok(owner)
    }

    pub fn take_startup_warnings(&mut self) -> Vec<AdminEffect> {
        std::mem::take(&mut self.startup_warnings)
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }
    pub fn settings(&self) -> &AdminSettings {
        &self.settings
    }
    pub fn lists(&self) -> &AccessLists {
        &self.lists
    }

    /// Called only for an actual authenticated/offline-selected profile or a
    /// genuine persisted profile. An unresolved command name is never enough.
    pub fn remember_profile(&mut self, profile: AccessProfile) {
        self.known_profiles
            .retain(|_, known| known.uuid != profile.uuid);
        self.known_profiles
            .insert(profile.name.to_ascii_lowercase(), profile);
    }

    pub fn permission_level(&self, uuid: [u8; 16]) -> i32 {
        self.lists
            .operators
            .get(&uuid)
            .map_or(0, |entry| entry.level)
    }

    /// Perform real list lookups only at the gate where vanilla reaches them.
    /// In particular an early player-ban rejection does not purge the IP list.
    pub fn can_player_login(
        &mut self,
        uuid: [u8; 16],
        address: SocketAddr,
        online_players: usize,
        now_ms: i64,
    ) -> Result<Option<AdmissionRejection>> {
        let player_lookup = if self.lists.contains_player_ban(uuid) {
            match self.lists.get_player_ban(uuid, now_ms) {
                Some(entry) => BanLookup::Present(BanView {
                    reason: &entry.ban.reason,
                    expires_at_ms: entry.ban.expires_ms,
                }),
                None => BanLookup::MissingAfterContains,
            }
        } else {
            BanLookup::NotListed
        };
        if let Some(rejection) = admission_policy::check_player_ban(player_lookup)? {
            return Ok(Some(rejection));
        }
        if !admission_policy::is_white_listed(
            self.settings.whitelist_enabled,
            self.lists.operators.contains_key(&uuid),
            self.lists.whitelist.contains_key(&uuid),
        ) {
            return Ok(Some(AdmissionRejection::NotWhitelisted));
        }
        let ip = access_lists::legacy_ip_key(address);
        let ip_lookup = if self.lists.contains_ip_ban(&ip) {
            match self.lists.get_ip_ban(&ip, now_ms) {
                Some(entry) => BanLookup::Present(BanView {
                    reason: &entry.ban.reason,
                    expires_at_ms: entry.ban.expires_ms,
                }),
                None => BanLookup::MissingAfterContains,
            }
        } else {
            BanLookup::NotListed
        };
        if let Some(rejection) = admission_policy::check_ip_ban(ip_lookup)? {
            return Ok(Some(rejection));
        }
        Ok(admission_policy::check_capacity(
            online_players,
            self.settings.max_players,
            self.lists
                .operators
                .get(&uuid)
                .is_some_and(|entry| entry.bypasses_player_limit),
        )?)
    }

    /// CONFIGURATION's existing admitted UUID check precedes every list lookup.
    pub fn can_finish_configuration(
        &mut self,
        duplicate_uuid_is_admitted: bool,
        uuid: [u8; 16],
        address: SocketAddr,
        online_players: usize,
        now_ms: i64,
    ) -> Result<Option<AdmissionRejection>> {
        if duplicate_uuid_is_admitted {
            return Ok(Some(AdmissionRejection::DuplicateLogin));
        }
        self.can_player_login(uuid, address, online_players, now_ms)
    }

    pub fn execute_line(
        &mut self,
        line: &str,
        source: &CommandSource,
        source_entity_id: Option<i32>,
        online: &[OnlineProfile],
        now_ms: i64,
    ) -> CommandOutcome {
        // Brigadier removes inaccessible root literals before parsing their
        // arguments. Do not reveal a hidden administration command's argument
        // grammar merely because its bounded parser runs before execute().
        let input = line.strip_prefix('/').unwrap_or(line);
        let root = input.split(' ').next().unwrap_or("");
        let permission = match root {
            "list" => Some(0),
            "banlist" | "ban" | "pardon" | "ban-ip" | "pardon-ip" | "whitelist" | "op" | "deop"
            | "kick" => Some(3),
            _ => None,
        };
        if permission.is_some_and(|required| source.permission_level < required) {
            return CommandOutcome {
                result: Err(CommandError::Message(CommandMessage::plain_key("command.unknown.command"))),
                effects: Vec::new(),
            };
        }
        match admission_commands::parse_command(line) {
            Ok(command) => self.execute(&command, source, source_entity_id, online, now_ms),
            Err(error) => CommandOutcome {
                result: Err(error),
                effects: Vec::new(),
            },
        }
    }

    pub fn execute(
        &mut self,
        command: &AdminCommand,
        source: &CommandSource,
        source_entity_id: Option<i32>,
        online: &[OnlineProfile],
        now_ms: i64,
    ) -> CommandOutcome {
        for player in online {
            self.remember_profile(player.profile.clone());
        }
        let mut host = CommandSession {
            owner: self,
            source_entity_id,
            online,
            now_ms,
            effects: Vec::new(),
        };
        let result = admission_commands::execute(&mut host, source, command.clone());
        CommandOutcome {
            result,
            effects: host.effects,
        }
    }
}

struct CommandSession<'a> {
    owner: &'a mut Administration,
    source_entity_id: Option<i32>,
    online: &'a [OnlineProfile],
    now_ms: i64,
    effects: Vec<AdminEffect>,
}

impl CommandSession<'_> {
    fn warning(&mut self, file: &'static str, result: Result<()>) {
        if let Err(error) = result {
            self.effects.push(AdminEffect::PersistenceWarning {
                file,
                message: format!("{error:#}"),
            });
        }
    }
    fn selected_players(
        &self,
        target: &str,
    ) -> std::result::Result<Vec<&OnlineProfile>, CommandError> {
        if target == "@a" {
            return Ok(self.online.iter().collect());
        }
        if target == "@s" {
            return Ok(self
                .online
                .iter()
                .filter(|player| Some(player.entity_id) == self.source_entity_id)
                .collect());
        }
        if target.starts_with('@') {
            return Err(CommandError::Unsupported(
                "only literal player names, @a and actual-player @s are implemented",
            ));
        }
        Ok(self
            .online
            .iter()
            .filter(|player| player.profile.name.eq_ignore_ascii_case(target))
            .collect())
    }
    fn player(&self, value: &OnlineProfile) -> OnlinePlayer {
        OnlinePlayer {
            profile: value.profile.clone(),
            ip: access_lists::legacy_ip_key(value.address),
        }
    }
}

impl AdministrationHost for CommandSession<'_> {
    fn resolve_profiles(
        &mut self,
        target: &str,
    ) -> std::result::Result<Vec<AccessProfile>, CommandError> {
        if target.starts_with('@') {
            let players = self.selected_players(target)?;
            if players.is_empty() {
                return Err(CommandError::Message(CommandMessage::plain_key(
                    "argument.player.unknown",
                )));
            }
            return Ok(players
                .into_iter()
                .map(|player| player.profile.clone())
                .collect());
        }
        let profile = self
            .online
            .iter()
            .find(|player| player.profile.name.eq_ignore_ascii_case(target))
            .map(|player| player.profile.clone())
            .or_else(|| {
                self.owner
                    .known_profiles
                    .get(&target.to_ascii_lowercase())
                    .cloned()
            });
        match profile {
            Some(profile) => Ok(vec![profile]),
            None => Err(CommandError::Unsupported("profile is absent from actual online/loaded/observed owners; remote user-cache lookup is not implemented")),
        }
    }
    fn resolve_players(
        &mut self,
        target: &str,
    ) -> std::result::Result<Vec<OnlinePlayer>, CommandError> {
        let selected = self.selected_players(target)?;
        if selected.is_empty() {
            return Err(CommandError::Message(CommandMessage::plain_key(
                "argument.entity.notfound.player",
            )));
        }
        Ok(selected
            .into_iter()
            .map(|player| self.player(player))
            .collect())
    }
    fn online_players(&self) -> Vec<OnlinePlayer> {
        self.online
            .iter()
            .map(|player| self.player(player))
            .collect()
    }
    fn max_players(&self) -> i32 {
        self.owner.settings.max_players
    }
    fn ban_entries(&self, kind: BanListKind) -> Vec<BanListItem> {
        let mut entries = Vec::new();
        if matches!(kind, BanListKind::All | BanListKind::Players) {
            entries.extend(
                self.owner
                    .lists
                    .player_bans
                    .values()
                    .map(|entry| BanListItem {
                        display_name: entry.profile.name.clone(),
                        source: entry.ban.source.clone(),
                        reason: entry.ban.reason.clone(),
                    }),
            );
        }
        if matches!(kind, BanListKind::All | BanListKind::Ips) {
            entries.extend(self.owner.lists.ip_bans.values().map(|entry| BanListItem {
                display_name: entry.ip.clone(),
                source: entry.ban.source.clone(),
                reason: entry.ban.reason.clone(),
            }));
        }
        entries
    }
    fn is_player_banned(&self, profile: &AccessProfile) -> bool {
        self.owner.lists.contains_player_ban(profile.uuid)
    }
    fn is_ip_banned(&self, ip: &str) -> bool {
        self.owner.lists.contains_ip_ban(ip)
    }
    fn is_operator(&self, profile: &AccessProfile) -> bool {
        self.owner.lists.operators.contains_key(&profile.uuid)
    }
    fn is_whitelisted(&self, profile: &AccessProfile) -> bool {
        self.owner.lists.whitelist.contains_key(&profile.uuid)
    }
    fn whitelist_names(&self) -> Vec<String> {
        self.owner
            .lists
            .whitelist
            .values()
            .map(|profile| profile.name.clone())
            .collect()
    }
    fn uses_whitelist(&self) -> bool {
        self.owner.settings.whitelist_enabled
    }
    fn server_is_published(&self) -> bool {
        true
    }
    fn is_singleplayer_owner(&self, _profile: &AccessProfile) -> bool {
        false
    }

    fn ban_player(
        &mut self,
        profile: &AccessProfile,
        source: &str,
        reason: Option<&str>,
    ) -> std::result::Result<String, CommandError> {
        let ban = BanMetadata::new(
            self.now_ms,
            Some(source.to_owned()),
            reason.map(str::to_owned),
        );
        let reason = ban.reason.clone();
        self.owner.lists.player_bans.insert(
            profile.uuid,
            PlayerBanEntry {
                profile: profile.clone(),
                ban,
            },
        );
        let saved = self.owner.lists.save_players(&self.owner.directory);
        self.warning(access_lists::PLAYER_BANS_FILE, saved);
        Ok(reason)
    }
    fn pardon_player(&mut self, profile: &AccessProfile) -> std::result::Result<(), CommandError> {
        self.owner.lists.player_bans.remove(&profile.uuid);
        let saved = self.owner.lists.save_players(&self.owner.directory);
        self.warning(access_lists::PLAYER_BANS_FILE, saved);
        Ok(())
    }
    fn ban_ip(
        &mut self,
        ip: &str,
        source: &str,
        reason: Option<&str>,
    ) -> std::result::Result<String, CommandError> {
        let ban = BanMetadata::new(
            self.now_ms,
            Some(source.to_owned()),
            reason.map(str::to_owned),
        );
        let reason = ban.reason.clone();
        self.owner.lists.ip_bans.insert(
            ip.to_owned(),
            IpBanEntry {
                ip: ip.to_owned(),
                ban,
            },
        );
        let saved = self.owner.lists.save_ips(&self.owner.directory);
        self.warning(access_lists::IP_BANS_FILE, saved);
        Ok(reason)
    }
    fn pardon_ip(&mut self, ip: &str) -> std::result::Result<(), CommandError> {
        self.owner.lists.ip_bans.remove(ip);
        let saved = self.owner.lists.save_ips(&self.owner.directory);
        self.warning(access_lists::IP_BANS_FILE, saved);
        Ok(())
    }
    fn set_operator(
        &mut self,
        profile: &AccessProfile,
        value: bool,
    ) -> std::result::Result<(), CommandError> {
        if value {
            let bypasses_player_limit = self
                .owner
                .lists
                .operators
                .get(&profile.uuid)
                .is_some_and(|entry| entry.bypasses_player_limit);
            self.owner.lists.operators.insert(
                profile.uuid,
                OperatorEntry {
                    profile: profile.clone(),
                    level: self.owner.settings.op_permission_level,
                    bypasses_player_limit,
                },
            );
        } else {
            self.owner.lists.operators.remove(&profile.uuid);
        }
        let saved = self.owner.lists.save_ops(&self.owner.directory);
        self.warning(access_lists::OPERATORS_FILE, saved);
        for player in self
            .online
            .iter()
            .filter(|player| player.profile.uuid == profile.uuid)
        {
            self.effects.push(AdminEffect::PermissionLevel {
                entity_id: player.entity_id,
                uuid: player.profile.uuid,
                level: self.owner.permission_level(profile.uuid),
            });
        }
        Ok(())
    }
    fn set_whitelisted(
        &mut self,
        profile: &AccessProfile,
        value: bool,
    ) -> std::result::Result<(), CommandError> {
        if value {
            self.owner
                .lists
                .whitelist
                .insert(profile.uuid, profile.clone());
        } else {
            self.owner.lists.whitelist.remove(&profile.uuid);
        }
        let saved = self.owner.lists.save_whitelist(&self.owner.directory);
        self.warning(access_lists::WHITELIST_FILE, saved);
        Ok(())
    }
    fn set_uses_whitelist(&mut self, value: bool) -> std::result::Result<(), CommandError> {
        self.owner.settings.whitelist_enabled = value;
        let saved = append_property(
            &self.owner.directory.join("server.properties"),
            "white-list",
            if value { "true" } else { "false" },
        );
        self.warning("server.properties", saved);
        Ok(())
    }
    fn reload_whitelist(&mut self) -> std::result::Result<(), CommandError> {
        let loaded = self
            .owner
            .lists
            .load_whitelist(&self.owner.directory, self.now_ms);
        self.warning(access_lists::WHITELIST_FILE, loaded);
        for profile in self
            .owner
            .lists
            .whitelist
            .values()
            .cloned()
            .collect::<Vec<_>>()
        {
            self.owner.remember_profile(profile);
        }
        Ok(())
    }
    fn enforce_whitelist(&mut self) -> std::result::Result<(), CommandError> {
        for player in self.online {
            if admission_policy::should_kick_unlisted(
                self.owner.settings.enforce_whitelist,
                self.owner
                    .lists
                    .whitelist
                    .contains_key(&player.profile.uuid),
            ) {
                self.effects.push(AdminEffect::Disconnect {
                    entity_id: player.entity_id,
                    uuid: player.profile.uuid,
                    reason: CommandMessage::plain_key("multiplayer.disconnect.not_whitelisted"),
                });
            }
        }
        Ok(())
    }
    fn feedback(&mut self, message: CommandMessage, broadcast_to_ops: bool) {
        self.effects.push(AdminEffect::Feedback {
            message,
            broadcast_to_ops,
        });
    }
    fn disconnect(&mut self, profile: &AccessProfile, reason: CommandMessage) {
        for player in self
            .online
            .iter()
            .filter(|player| player.profile.uuid == profile.uuid)
        {
            self.effects.push(AdminEffect::Disconnect {
                entity_id: player.entity_id,
                uuid: profile.uuid,
                reason: reason.clone(),
            });
        }
    }
}

fn startup_io_warning(
    warnings: &mut Vec<AdminEffect>,
    file: &'static str,
    result: Result<()>,
) -> Result<()> {
    if let Err(error) = result {
        if error.downcast_ref::<std::io::Error>().is_none() {
            return Err(error);
        }
        warnings.push(AdminEffect::PersistenceWarning {
            file,
            message: format!("startup: {error:#}"),
        });
    }
    Ok(())
}

fn startup_any_warning(warnings: &mut Vec<AdminEffect>, file: &'static str, result: Result<()>) {
    if let Err(error) = result {
        warnings.push(AdminEffect::PersistenceWarning {
            file,
            message: format!("startup: {error:#}"),
        });
    }
}

/// Preserve unrelated server.properties bytes. Java properties use the last
/// occurrence of a key, so appending a canonical key updates only this owned
/// setting without rewriting unknown keys/comments or inventing their owners.
fn append_property(path: &Path, key: &str, value: &str) -> Result<()> {
    let previous = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => {
            return Err(error).context("read server.properties before updating white-list")
        }
    };
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .context("open server.properties for white-list update")?;
    if !previous.is_empty() {
        // A blank natural line also terminates an existing continuation; merely
        // adding a single separator could make white-list part of another key.
        file.write_all(b"\n\n")?;
    }
    writeln!(file, "{key}={value}").context("persist white-list server property")
}

/// Read the one property owned here. Settings.loadFromFile first tries strict
/// UTF-8, then ISO-8859-1; Properties retains escapes, continued lines, separators
/// and last-key-wins behavior. Other settings stay with Main.
fn property_value(path: &Path, wanted: &str) -> Result<Option<String>> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("read server.properties"),
    };
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => error.into_bytes().into_iter().map(char::from).collect(),
    };
    let mut logical = String::new();
    let mut continued = false;
    let mut found = None;
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    for physical in normalized.split('\n') {
        if continued {
            logical.push_str(physical.trim_start_matches([' ', '\t', '\u{c}']));
        } else {
            logical.push_str(physical);
        }
        let trailing = logical
            .chars()
            .rev()
            .take_while(|value| *value == '\\')
            .count();
        continued = trailing % 2 == 1;
        if continued {
            logical.pop();
            continue;
        }
        read_property_line(&logical, wanted, &mut found)?;
        logical.clear();
    }
    if !logical.is_empty() {
        read_property_line(&logical, wanted, &mut found)?;
    }
    Ok(found)
}

fn read_property_line(line: &str, wanted: &str, found: &mut Option<String>) -> Result<()> {
    let line = line.trim_start_matches([' ', '\t', '\u{c}']);
    if line.is_empty() || line.starts_with(['#', '!']) {
        return Ok(());
    }
    let mut escaped = false;
    let mut separator = line.len();
    let mut whitespace_separator = false;
    for (offset, ch) in line.char_indices() {
        if !escaped && matches!(ch, '=' | ':' | ' ' | '\t' | '\u{c}') {
            separator = offset;
            whitespace_separator = matches!(ch, ' ' | '\t' | '\u{c}');
            break;
        }
        if ch == '\\' {
            escaped = !escaped;
        } else {
            escaped = false;
        }
    }
    let key = unescape_property_units(&line[..separator])?;
    if key != wanted.encode_utf16().collect::<Vec<_>>() {
        return Ok(());
    }
    let mut value = &line[separator..];
    if whitespace_separator {
        value = value.trim_start_matches([' ', '\t', '\u{c}']);
        if value.starts_with(['=', ':']) {
            value = &value[1..];
        }
    } else if !value.is_empty() {
        value = &value[1..];
    }
    value = value.trim_start_matches([' ', '\t', '\u{c}']);
    // The only owned value is Boolean.parseBoolean: every non-true UTF-16
    // value, including an isolated surrogate, is false. Lossy display therefore
    // cannot change its truth value and does not affect preserved file bytes.
    *found = Some(String::from_utf16_lossy(&unescape_property_units(value)?));
    Ok(())
}

fn unescape_property_units(text: &str) -> Result<Vec<u16>> {
    let mut units = Vec::new();
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            units.extend(ch.encode_utf16(&mut [0; 2]).iter().copied());
            continue;
        }
        let Some(ch) = chars.next() else {
            break;
        };
        match ch {
            'u' => {
                let mut value = 0u16;
                for _ in 0..4 {
                    let next = chars
                        .next()
                        .context("incomplete Unicode escape in server.properties")?;
                    value = (value << 4)
                        | next
                            .to_digit(16)
                            .context("invalid Unicode escape in server.properties")?
                            as u16;
                }
                units.push(value);
            }
            't' => units.push(b'\t' as u16),
            'r' => units.push(b'\r' as u16),
            'n' => units.push(b'\n' as u16),
            'f' => units.push(12),
            other => units.extend(other.encode_utf16(&mut [0; 2]).iter().copied()),
        }
    }
    Ok(units)
}
