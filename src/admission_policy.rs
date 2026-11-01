//! Native dedicated-server login admission, from Minecraft Java 1.21.
//!
//! The host supplies actual access-list lookups and the admitted player count.
//! `PlayerList.canPlayerLogin` runs both before LOGIN success and after the
//! CONFIGURATION-finished acknowledgement. Neither pending connections nor a
//! guessed future player count belong in `online_players`.
//!
//! List lookups have observable ownership: Java's contains does not remove
//! expired entries, while get removes expired entries from that entire list.
//! Hosts should use the sequential helpers in source order and perform each
//! lookup only when its gate is reached. The immutable combined view is useful
//! once those ordered effects have already been handled.

use std::{error::Error, fmt};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use crate::nbt::Tag;

/// A ban obtained from the real access-list owner after its source-ordered get.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BanView<'a> {
    pub reason: &'a str,
    pub expires_at_ms: Option<i64>,
}

/// Result of a raw contains check followed, only when true, by the list's get.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BanLookup<'a> {
    NotListed,
    Present(BanView<'a>),
    /// The original Java method dereferences null here, for example if get
    /// removed the very expired entry which contains had just observed. Keep
    /// this exceptional outcome distinct from a successful admission.
    MissingAfterContains,
}

/// Immutable input to the dedicated admission gates, in their source order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdmissionView<'a> {
    pub player_ban: BanLookup<'a>,
    pub whitelist_enabled: bool,
    pub whitelist_member: bool,
    /// DedicatedPlayerList calls PlayerList.isOp, rather than testing an op's
    /// numerical command level. An actual level-zero op still passes here.
    pub is_operator: bool,
    pub ip_ban: BanLookup<'a>,
    /// Actual PlayerList.players size, including any duplicate still admitted.
    pub online_players: usize,
    pub max_players: i32,
    /// The actual ServerOpListEntry.bypassesPlayerLimit value. Merely being an
    /// operator is not a capacity exemption.
    pub bypasses_player_limit: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdmissionRejection {
    PlayerBanned {
        reason: String,
        expires_at_ms: Option<i64>,
    },
    NotWhitelisted,
    IpBanned {
        reason: String,
        expires_at_ms: Option<i64>,
    },
    ServerFull,
    /// CONFIGURATION completion checks an already-admitted UUID before the
    /// policy recheck; it must not disconnect/replace that player's owner here.
    DuplicateLogin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdmissionPolicyError {
    PlayerBanMissingAfterContains,
    IpBanMissingAfterContains,
}

impl fmt::Display for AdmissionPolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::PlayerBanMissingAfterContains => {
                "player ban disappeared during source contains/get admission lookup"
            }
            Self::IpBanMissingAfterContains => {
                "IP ban disappeared during source contains/get admission lookup"
            }
        })
    }
}
impl Error for AdmissionPolicyError {}

pub type AdmissionDecision = std::result::Result<Option<AdmissionRejection>, AdmissionPolicyError>;

pub fn check_player_ban(ban: BanLookup<'_>) -> AdmissionDecision {
    match ban {
        BanLookup::NotListed => Ok(None),
        BanLookup::Present(ban) => Ok(Some(AdmissionRejection::PlayerBanned {
            reason: ban.reason.to_owned(),
            expires_at_ms: ban.expires_at_ms,
        })),
        BanLookup::MissingAfterContains => Err(AdmissionPolicyError::PlayerBanMissingAfterContains),
    }
}

/// DedicatedPlayerList.isWhiteListed. Enforce-whitelist is not consulted here.
pub fn is_white_listed(enabled: bool, is_operator: bool, member: bool) -> bool {
    !enabled || is_operator || member
}

pub fn check_ip_ban(ban: BanLookup<'_>) -> AdmissionDecision {
    match ban {
        BanLookup::NotListed => Ok(None),
        BanLookup::Present(ban) => Ok(Some(AdmissionRejection::IpBanned {
            reason: ban.reason.to_owned(),
            expires_at_ms: ban.expires_at_ms,
        })),
        BanLookup::MissingAfterContains => Err(AdmissionPolicyError::IpBanMissingAfterContains),
    }
}

pub fn check_capacity(
    online_players: usize,
    max_players: i32,
    bypasses_player_limit: bool,
) -> AdmissionDecision {
    // The Java comparison is signed. A negative configured maximum rejects
    // every ordinary player; do not cast it into a large positive usize.
    let full = max_players <= 0 || online_players >= max_players as usize;
    Ok((full && !bypasses_player_limit).then_some(AdmissionRejection::ServerFull))
}

pub fn can_player_login(view: AdmissionView<'_>) -> AdmissionDecision {
    if let Some(reason) = check_player_ban(view.player_ban)? {
        return Ok(Some(reason));
    }
    if !is_white_listed(
        view.whitelist_enabled,
        view.is_operator,
        view.whitelist_member,
    ) {
        return Ok(Some(AdmissionRejection::NotWhitelisted));
    }
    if let Some(reason) = check_ip_ban(view.ip_ban)? {
        return Ok(Some(reason));
    }
    check_capacity(
        view.online_players,
        view.max_players,
        view.bypasses_player_limit,
    )
}

/// ServerConfigurationPacketListenerImpl.handleConfigurationFinished performs
/// this duplicate UUID gate before running PlayerList.canPlayerLogin again.
pub fn can_finish_configuration(
    duplicate_uuid_is_admitted: bool,
    view: AdmissionView<'_>,
) -> AdmissionDecision {
    if duplicate_uuid_is_admitted {
        return Ok(Some(AdmissionRejection::DuplicateLogin));
    }
    can_player_login(view)
}

/// MinecraftServer.kickUnlistedPlayers differs from the admission predicate:
/// it checks enforce-whitelist and direct whitelist membership only. It does
/// not consult white-list being enabled and does not exempt operators.
pub fn should_kick_unlisted(enforce_whitelist: bool, whitelist_member: bool) -> bool {
    enforce_whitelist && !whitelist_member
}

impl AdmissionRejection {
    pub fn translation_key(&self) -> &'static str {
        match self {
            Self::PlayerBanned { .. } => "multiplayer.disconnect.banned.reason",
            Self::NotWhitelisted => "multiplayer.disconnect.not_whitelisted",
            Self::IpBanned { .. } => "multiplayer.disconnect.banned_ip.reason",
            Self::ServerFull => "multiplayer.disconnect.server_full",
            Self::DuplicateLogin => "multiplayer.disconnect.duplicate_login",
        }
    }

    fn ban(&self) -> Option<(&str, Option<i64>, &'static str)> {
        match self {
            Self::PlayerBanned {
                reason,
                expires_at_ms,
            } => Some((
                reason,
                *expires_at_ms,
                "multiplayer.disconnect.banned.expiration",
            )),
            Self::IpBanned {
                reason,
                expires_at_ms,
            } => Some((
                reason,
                *expires_at_ms,
                "multiplayer.disconnect.banned_ip.expiration",
            )),
            _ => None,
        }
    }

    /// The bounded component has only translatable content, string arguments
    /// and an optional translatable sibling. Ban reasons are literal arguments,
    /// never JSON parsed as components. No newline is inserted before expiry.
    pub fn json_component(&self) -> Result<Value> {
        self.json_component_with_date_formatter(format_ban_expiration_local)
    }

    /// An integration with a different locale/timezone owner may supply its
    /// actual Java-compatible date formatter instead of assuming UTC.
    pub fn json_component_with_date_formatter(
        &self,
        mut format_date: impl FnMut(i64) -> Result<String>,
    ) -> Result<Value> {
        let mut component = json!({ "translate": self.translation_key() });
        if let Some((reason, expires_at_ms, expiration_key)) = self.ban() {
            component["with"] = json!([reason]);
            if let Some(expires_at_ms) = expires_at_ms {
                component["extra"] = json!([{
                    "translate": expiration_key,
                    "with": [format_date(expires_at_ms)?],
                }]);
            }
        }
        Ok(component)
    }

    /// Protocol1.21 CONFIGURATION/PLAY components use anonymous network NBT.
    /// This encodes the known admission-message shape directly; it does not
    /// claim to implement the general Component codec. Translatable string
    /// arguments are a homogeneous TAG_String list and siblings TAG_Compound.
    pub fn network_component(&self) -> Result<Vec<u8>> {
        self.network_component_with_date_formatter(format_ban_expiration_local)
    }

    pub fn network_component_with_date_formatter(
        &self,
        mut format_date: impl FnMut(i64) -> Result<String>,
    ) -> Result<Vec<u8>> {
        let mut entries = vec![(
            "translate".into(),
            Tag::String(self.translation_key().into()),
        )];
        if let Some((reason, expires_at_ms, expiration_key)) = self.ban() {
            entries.push((
                "with".into(),
                Tag::List {
                    element_type: 8,
                    values: vec![Tag::String(reason.into())],
                },
            ));
            if let Some(expires_at_ms) = expires_at_ms {
                entries.push((
                    "extra".into(),
                    Tag::List {
                        element_type: 10,
                        values: vec![Tag::Compound(vec![
                            ("translate".into(), Tag::String(expiration_key.into())),
                            (
                                "with".into(),
                                Tag::List {
                                    element_type: 8,
                                    values: vec![Tag::String(format_date(expires_at_ms)?.into())],
                                },
                            ),
                        ])],
                    },
                ));
            }
        }
        Tag::Compound(entries).encode_network_anonymous()
    }
}

/// Native host-local date rendering for the dedicated Linux server. The pinned
/// Java formatter is `yyyy-MM-dd 'at' HH:mm:ss z`, using the JVM's local timezone
/// and locale. libc supplies that host's local civil time and short zone name;
/// the supported runtime uses Gregorian dates and English/ASCII digits. Other
/// Java locale/calendar conventions require an explicit owner-supplied formatter.
#[cfg(unix)]
pub fn format_ban_expiration_local(epoch_millis: i64) -> Result<String> {
    use std::{ffi::CStr, mem::MaybeUninit};

    // Date's displayed second is floor(milliseconds/1000), including pre-epoch
    // dates; integer truncation toward zero would incorrectly shift them.
    let seconds = libc::time_t::try_from(epoch_millis.div_euclid(1000))
        .context("ban expiration is outside the native time_t range")?;
    let mut calendar = MaybeUninit::<libc::tm>::uninit();
    let ptr = unsafe { libc::localtime_r(&seconds, calendar.as_mut_ptr()) };
    if ptr.is_null() {
        bail!("ban expiration cannot be represented in the host local timezone");
    }
    let mut output = [0 as libc::c_char; 256];
    let used = unsafe {
        libc::strftime(
            output.as_mut_ptr(),
            output.len(),
            b"%Y-%m-%d at %H:%M:%S %Z\0".as_ptr().cast(),
            ptr,
        )
    };
    if used == 0 {
        bail!("ban expiration local-zone rendering exceeded the native buffer");
    }
    let rendered = unsafe { CStr::from_ptr(output.as_ptr()) };
    Ok(rendered
        .to_str()
        .context("host local timezone abbreviation is not UTF-8")?
        .to_owned())
}

#[cfg(not(unix))]
pub fn format_ban_expiration_local(_epoch_millis: i64) -> Result<String> {
    bail!("host-local Java ban-date rendering requires an explicit date formatter on this platform")
}
