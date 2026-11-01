//! Native Java 1.21 stored access-list owners.
//!
//! UUID-keyed profiles, expiration timing and JSON fields follow the pinned
//! `StoredUserList`, `BanListEntry` and their dedicated-server subclasses. The
//! caller owns the clock, directory and warning/reporting policy. In particular,
//! mutating an entry and then failing to save does not undo the mutation.

use anyhow::{bail, ensure, Context, Result};
use serde_json::{json, Map, Value};
use std::{
    collections::HashMap,
    fs::File,
    io::{BufWriter, Read, Write},
    net::SocketAddr,
    path::Path,
};

pub const PLAYER_BANS_FILE: &str = "banned-players.json";
pub const IP_BANS_FILE: &str = "banned-ips.json";
pub const OPERATORS_FILE: &str = "ops.json";
pub const WHITELIST_FILE: &str = "whitelist.json";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessProfile {
    pub uuid: [u8; 16],
    pub name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BanMetadata {
    pub created_ms: i64,
    pub source: String,
    pub expires_ms: Option<i64>,
    pub reason: String,
}

impl BanMetadata {
    /// Source constructor defaults; command callers supply their actual source.
    pub fn new(now_ms: i64, source: Option<String>, reason: Option<String>) -> Self {
        Self {
            created_ms: now_ms,
            source: source.unwrap_or_else(|| "(Unknown)".into()),
            expires_ms: None,
            reason: reason.unwrap_or_else(|| "Banned by an operator.".into()),
        }
    }

    pub fn has_expired(&self, now_ms: i64) -> bool {
        self.expires_ms.is_some_and(|expires| expires < now_ms)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayerBanEntry {
    pub profile: AccessProfile,
    pub ban: BanMetadata,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IpBanEntry {
    pub ip: String,
    pub ban: BanMetadata,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperatorEntry {
    pub profile: AccessProfile,
    /// Java loads this integer without clamping it to a command permission range.
    pub level: i32,
    pub bypasses_player_limit: bool,
}

#[derive(Clone, Debug, Default)]
pub struct AccessLists {
    pub player_bans: HashMap<[u8; 16], PlayerBanEntry>,
    pub ip_bans: HashMap<String, IpBanEntry>,
    pub operators: HashMap<[u8; 16], OperatorEntry>,
    pub whitelist: HashMap<[u8; 16], AccessProfile>,
}

impl AccessLists {
    pub fn load(directory: &Path, now_ms: i64) -> Result<Self> {
        let mut lists = Self::default();
        lists.reload(directory, now_ms)?;
        Ok(lists)
    }

    /// Convenience reload. The host can call per-file methods when source
    /// command/startup ordering or independent error recovery is needed.
    pub fn reload(&mut self, directory: &Path, now_ms: i64) -> Result<()> {
        self.load_players(directory, now_ms)?;
        self.load_ips(directory, now_ms)?;
        self.load_ops(directory, now_ms)?;
        self.load_whitelist(directory, now_ms)
    }

    pub fn load_players(&mut self, directory: &Path, now_ms: i64) -> Result<()> {
        let Some(text) = read_list(directory, PLAYER_BANS_FILE)? else {
            return Ok(());
        };
        self.player_bans.clear();
        for value in list_array(text, PLAYER_BANS_FILE)? {
            let obj = object(&value)?;
            // The subclass constructor parses metadata even for a null profile.
            let profile = profile(obj)?;
            let ban = ban_metadata(obj, now_ms)?;
            if let Some(profile) = profile {
                self.player_bans
                    .insert(profile.uuid, PlayerBanEntry { profile, ban });
            }
        }
        Ok(())
    }

    pub fn load_ips(&mut self, directory: &Path, now_ms: i64) -> Result<()> {
        let Some(text) = read_list(directory, IP_BANS_FILE)? else {
            return Ok(());
        };
        self.ip_bans.clear();
        for value in list_array(text, IP_BANS_FILE)? {
            let obj = object(&value)?;
            let ip = optional_string(obj, "ip")?;
            let ban = ban_metadata(obj, now_ms)?;
            if let Some(ip) = ip {
                self.ip_bans.insert(ip.clone(), IpBanEntry { ip, ban });
            }
        }
        Ok(())
    }

    pub fn load_ops(&mut self, directory: &Path, _now_ms: i64) -> Result<()> {
        let Some(text) = read_list(directory, OPERATORS_FILE)? else {
            return Ok(());
        };
        self.operators.clear();
        for value in list_array(text, OPERATORS_FILE)? {
            let obj = object(&value)?;
            let profile = profile(obj)?;
            let level = obj.get("level").map(gson_i32).transpose()?.unwrap_or(0);
            let bypasses_player_limit = obj
                .get("bypassesPlayerLimit")
                .map(gson_bool)
                .transpose()?
                .unwrap_or(false);
            if let Some(profile) = profile {
                self.operators.insert(
                    profile.uuid,
                    OperatorEntry {
                        profile,
                        level,
                        bypasses_player_limit,
                    },
                );
            }
        }
        Ok(())
    }

    pub fn load_whitelist(&mut self, directory: &Path, _now_ms: i64) -> Result<()> {
        let Some(text) = read_list(directory, WHITELIST_FILE)? else {
            return Ok(());
        };
        self.whitelist.clear();
        for value in list_array(text, WHITELIST_FILE)? {
            if let Some(profile) = profile(object(&value)?)? {
                self.whitelist.insert(profile.uuid, profile);
            }
        }
        Ok(())
    }

    /// `contains` intentionally does not clean expired entries. Java's login
    /// handler performs a separate `contains` then `get` sequence.
    pub fn contains_player_ban(&self, uuid: [u8; 16]) -> bool {
        self.player_bans.contains_key(&uuid)
    }

    pub fn contains_ip_ban(&self, ip: &str) -> bool {
        self.ip_bans.contains_key(ip)
    }

    pub fn get_player_ban(&mut self, uuid: [u8; 16], now_ms: i64) -> Option<&PlayerBanEntry> {
        self.player_bans
            .retain(|_, entry| !entry.ban.has_expired(now_ms));
        self.player_bans.get(&uuid)
    }

    pub fn get_ip_ban(&mut self, ip: &str, now_ms: i64) -> Option<&IpBanEntry> {
        self.ip_bans
            .retain(|_, entry| !entry.ban.has_expired(now_ms));
        self.ip_bans.get(ip)
    }

    pub fn save_players(&self, directory: &Path) -> Result<()> {
        let values = self
            .player_bans
            .values()
            .map(|entry| {
                let mut value = profile_json(&entry.profile);
                write_ban_metadata(&mut value, &entry.ban)?;
                Ok(Value::Object(value))
            })
            .collect::<Result<Vec<_>>>()?;
        write_list(directory, PLAYER_BANS_FILE, values)
    }

    pub fn save_ips(&self, directory: &Path) -> Result<()> {
        let values = self
            .ip_bans
            .values()
            .map(|entry| {
                let mut value = Map::new();
                value.insert("ip".into(), Value::String(entry.ip.clone()));
                write_ban_metadata(&mut value, &entry.ban)?;
                Ok(Value::Object(value))
            })
            .collect::<Result<Vec<_>>>()?;
        write_list(directory, IP_BANS_FILE, values)
    }

    pub fn save_ops(&self, directory: &Path) -> Result<()> {
        let values = self
            .operators
            .values()
            .map(|entry| {
                let mut value = profile_json(&entry.profile);
                value.insert("level".into(), json!(entry.level));
                value.insert(
                    "bypassesPlayerLimit".into(),
                    json!(entry.bypasses_player_limit),
                );
                Value::Object(value)
            })
            .collect();
        write_list(directory, OPERATORS_FILE, values)
    }

    pub fn save_whitelist(&self, directory: &Path) -> Result<()> {
        let values = self
            .whitelist
            .values()
            .map(|profile| Value::Object(profile_json(profile)))
            .collect();
        write_list(directory, WHITELIST_FILE, values)
    }
}

/// Exact `IpBanList.getIpFromAddress` substring behavior for an InetSocketAddress.
/// It is not an IPv6 canonicalizer: vanilla truncates at the first colon.
pub fn legacy_ip_key(address: SocketAddr) -> String {
    match address.ip() {
        std::net::IpAddr::V4(ip) => ip.to_string(),
        // Inet6Address.getHostAddress prints all eight hexadecimal groups,
        // whereas Rust's Display compresses zero groups. Preserve the Java
        // substring result even for ::1 ("[0", not Rust's compressed "[").
        std::net::IpAddr::V6(ip) => format!("[{:x}", ip.segments()[0]),
    }
}

pub fn uuid_string(uuid: [u8; 16]) -> String {
    crate::player_data::uuid_string(uuid)
}

/// Java UUID.fromString accepts short hexadecimal groups as well as canonical
/// UUID text. The returned byte identity, not the input spelling, owns the key.
pub fn parse_profile_uuid(text: &str) -> Option<[u8; 16]> {
    if text.len() > 36 {
        return None;
    }
    let parts = text.split('-').collect::<Vec<_>>();
    if parts.len() != 5 {
        return None;
    }
    let mut n = [0u64; 5];
    for (dst, text) in n.iter_mut().zip(parts) {
        let text = text.strip_prefix('+').unwrap_or(text);
        if text.is_empty() || text.len() > 16 {
            return None;
        }
        *dst = u64::from_str_radix(text, 16).ok()?;
        // UUID's general parser uses Long.parseLong, not unsigned long parsing.
        if *dst > i64::MAX as u64 {
            return None;
        }
    }
    let hi = ((n[0] & 0xffff_ffff) << 32) | ((n[1] & 0xffff) << 16) | (n[2] & 0xffff);
    let lo = ((n[3] & 0xffff) << 48) | (n[4] & 0xffff_ffff_ffff);
    let mut uuid = [0u8; 16];
    uuid[..8].copy_from_slice(&hi.to_be_bytes());
    uuid[8..].copy_from_slice(&lo.to_be_bytes());
    Some(uuid)
}

fn read_list(directory: &Path, name: &str) -> Result<Option<File>> {
    let path = directory.join(name);
    match File::open(&path) {
        Ok(file) => Ok(Some(file)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("read access list {}", path.display())),
    }
}

fn list_array(mut file: File, name: &str) -> Result<Vec<Value>> {
    let mut text = String::new();
    file.read_to_string(&mut text)
        .with_context(|| format!("read {name}"))?;
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    match serde_json::from_str::<Value>(&text).with_context(|| format!("parse {name}"))? {
        Value::Array(values) => Ok(values),
        Value::Null => Ok(Vec::new()),
        _ => bail!("{name} must contain an array or null"),
    }
}

fn object(value: &Value) -> Result<&Map<String, Value>> {
    value
        .as_object()
        .context("access-list entry must be an object")
}

fn gson_string(value: &Value) -> Result<String> {
    match value {
        Value::String(text) => Ok(text.clone()),
        Value::Number(number) => Ok(number.to_string()),
        Value::Bool(boolean) => Ok(boolean.to_string()),
        Value::Array(values) if values.len() == 1 => gson_string(&values[0]),
        _ => bail!("access-list field cannot be read as a Gson string"),
    }
}

fn optional_string(obj: &Map<String, Value>, field: &str) -> Result<Option<String>> {
    obj.get(field).map(gson_string).transpose()
}

fn gson_i32(value: &Value) -> Result<i32> {
    match value {
        Value::String(text) => text
            .parse::<i32>()
            .context("invalid operator permission level"),
        Value::Number(number) => {
            if let Some(n) = number.as_i64() {
                return Ok(n as i32);
            }
            if let Some(n) = number.as_u64() {
                return Ok(n as i32);
            }
            let n = number
                .as_f64()
                .context("unsupported operator level number")?;
            ensure!(
                n.is_finite() && n.abs() <= (1u64 << 53) as f64,
                "operator level number exceeds the exact native decimal profile"
            );
            let wrapped = n.trunc().rem_euclid(4_294_967_296.0) as u32;
            Ok(wrapped as i32)
        }
        Value::Array(values) if values.len() == 1 => gson_i32(&values[0]),
        _ => bail!("access-list field cannot be read as a Gson integer"),
    }
}

fn gson_bool(value: &Value) -> Result<bool> {
    match value {
        Value::Bool(value) => Ok(*value),
        Value::String(value) => Ok(value.eq_ignore_ascii_case("true")),
        Value::Number(value) => Ok(value.to_string().eq_ignore_ascii_case("true")),
        Value::Array(values) if values.len() == 1 => gson_bool(&values[0]),
        _ => bail!("access-list field cannot be read as a Gson boolean"),
    }
}

fn profile(obj: &Map<String, Value>) -> Result<Option<AccessProfile>> {
    if !obj.contains_key("uuid") || !obj.contains_key("name") {
        return Ok(None);
    }
    let text = gson_string(&obj["uuid"])?;
    let Some(uuid) = parse_profile_uuid(&text) else {
        return Ok(None);
    };
    Ok(Some(AccessProfile {
        uuid,
        name: gson_string(&obj["name"])?,
    }))
}

fn ban_metadata(obj: &Map<String, Value>, now_ms: i64) -> Result<BanMetadata> {
    let created_ms = optional_string(obj, "created")?
        .map(|text| parse_ban_date(&text))
        .transpose()?
        .flatten()
        .unwrap_or(now_ms);
    let expires_ms = optional_string(obj, "expires")?
        .map(|text| parse_ban_date(&text))
        .transpose()?
        .flatten();
    Ok(BanMetadata {
        created_ms,
        source: optional_string(obj, "source")?.unwrap_or_else(|| "(Unknown)".into()),
        expires_ms,
        reason: optional_string(obj, "reason")?.unwrap_or_else(|| "Banned by an operator.".into()),
    })
}

fn profile_json(profile: &AccessProfile) -> Map<String, Value> {
    let mut value = Map::new();
    value.insert("uuid".into(), Value::String(uuid_string(profile.uuid)));
    value.insert("name".into(), Value::String(profile.name.clone()));
    value
}

fn write_ban_metadata(value: &mut Map<String, Value>, ban: &BanMetadata) -> Result<()> {
    value.insert(
        "created".into(),
        Value::String(format_ban_date(ban.created_ms)?),
    );
    value.insert("source".into(), Value::String(ban.source.clone()));
    value.insert(
        "expires".into(),
        Value::String(
            ban.expires_ms
                .map(format_ban_date)
                .transpose()?
                .unwrap_or_else(|| "forever".into()),
        ),
    );
    value.insert("reason".into(), Value::String(ban.reason.clone()));
    Ok(())
}

fn write_list(directory: &Path, name: &str, values: Vec<Value>) -> Result<()> {
    let path = directory.join(name);
    // Vanilla opens a UTF-8 truncating writer; directories remain host-owned.
    let file =
        File::create(&path).with_context(|| format!("open access list {}", path.display()))?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, &values)
        .with_context(|| format!("write access list {}", path.display()))?;
    writer
        .flush()
        .with_context(|| format!("flush access list {}", path.display()))
}

/// Canonical numeric-offset SimpleDateFormat input, including variable numeric
/// widths, lenient month/day/time rollover and ignored trailing input. Malformed
/// dates return None, which selects Java's created-now / expires-never defaults.
/// Noncanonical named time zones and pre-Gregorian-cutover dates are explicit
/// unsupported contexts rather than silently changing an active ban.
pub fn parse_ban_date(text: &str) -> Result<Option<i64>> {
    let Some((parts, zone)) = date_parts(text) else {
        return Ok(None);
    };
    let [year, month, day, hour, minute, second] = parts;
    if year < 1583 {
        bail!("ban date before Gregorian-calendar native profile");
    }
    let zone = zone.trim_start();
    let offset_minutes = if zone.starts_with('+') || zone.starts_with('-') {
        let bytes = zone.as_bytes();
        if bytes.len() < 5 || !bytes[1..5].iter().all(u8::is_ascii_digit) {
            return Ok(None);
        }
        let hours = (bytes[1] - b'0') as i64 * 10 + (bytes[2] - b'0') as i64;
        let minutes = (bytes[3] - b'0') as i64 * 10 + (bytes[4] - b'0') as i64;
        if hours > 23 || minutes > 59 {
            return Ok(None);
        }
        (hours * 60 + minutes) * if bytes[0] == b'-' { -1 } else { 1 }
    } else if zone.starts_with("UTC") || zone.starts_with("GMT") {
        // UTC/GMT names are accepted, but GMT+hh:mm needs a distinct parser.
        if zone
            .as_bytes()
            .get(3)
            .is_some_and(|b| *b == b'+' || *b == b'-')
        {
            bail!("GMT-offset named ban date is outside the numeric-offset profile");
        }
        0
    } else if zone.is_empty() {
        return Ok(None);
    } else {
        bail!("named-time-zone ban date is outside the numeric-offset profile");
    };
    let adjusted_year = year
        .checked_add((month - 1).div_euclid(12))
        .context("ban date year overflow")?;
    let adjusted_month = (month - 1).rem_euclid(12) + 1;
    ensure!(
        (1583..=9999).contains(&adjusted_year),
        "ban date outside supported year range"
    );
    let days = days_from_civil(adjusted_year, adjusted_month, 1)
        .checked_add(day - 1)
        .context("ban date day overflow")?;
    let seconds = days
        .checked_mul(86400)
        .and_then(|n| hour.checked_mul(3600).and_then(|h| n.checked_add(h)))
        .and_then(|n| minute.checked_mul(60).and_then(|m| n.checked_add(m)))
        .and_then(|n| n.checked_add(second))
        .and_then(|n| n.checked_sub(offset_minutes * 60))
        .and_then(|n| n.checked_mul(1000))
        .context("ban date timestamp overflow")?;
    Ok(Some(seconds))
}

fn date_parts(mut text: &str) -> Option<([i64; 6], &str)> {
    let mut parts = [0i64; 6];
    let delimiters = ['-', '-', ' ', ':', ':'];
    for i in 0..6 {
        text = text.trim_start_matches([' ', '\t']);
        let bytes = text.as_bytes();
        let mut end = usize::from(bytes.first().is_some_and(|b| *b == b'-' || *b == b'+'));
        let first = end;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
        if end == first {
            return None;
        }
        parts[i] = text[..end].parse().ok()?;
        text = &text[end..];
        if i < 5 {
            text = text.strip_prefix(delimiters[i])?;
        }
    }
    text = text.strip_prefix(' ')?;
    Some((parts, text))
}

fn days_from_civil(mut year: i64, month: i64, day: i64) -> i64 {
    year -= i64::from(month <= 2);
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let shifted_month = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * shifted_month + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Source uses the process default zone; libc's configured local zone is used
/// here rather than hardcoding UTC or reusing the input offset after load.
pub fn format_ban_date(epoch_ms: i64) -> Result<String> {
    let seconds: libc::time_t = epoch_ms
        .div_euclid(1000)
        .try_into()
        .context("ban date time_t overflow")?;
    let mut tm = std::mem::MaybeUninit::<libc::tm>::uninit();
    // SAFETY: both pointers refer to live correctly sized objects. localtime_r
    // initializes tm on success, and strftime receives its initialized value.
    let result = unsafe { libc::localtime_r(&seconds, tm.as_mut_ptr()) };
    ensure!(!result.is_null(), "ban date local-time conversion failed");
    let tm = unsafe { tm.assume_init() };
    ensure!(
        (1583..=9999).contains(&(tm.tm_year + 1900)),
        "ban date outside supported year range"
    );
    let mut out = [0u8; 80];
    let len = unsafe {
        libc::strftime(
            out.as_mut_ptr().cast(),
            out.len(),
            b"%Y-%m-%d %H:%M:%S %z\0".as_ptr().cast(),
            &tm,
        )
    };
    ensure!(len != 0, "ban date formatting failed");
    String::from_utf8(out[..len].to_vec()).context("ban date timezone was not UTF-8")
}
