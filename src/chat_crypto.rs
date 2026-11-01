//! Native Java 1.21 profile certificates and chat RSA verification.
//!
//! Trust enters only through an operator-owned services key set obtained from
//! the official authenticated HTTPS endpoint (or its trusted captured response).
//! A client-supplied chat key is never a services trust root. This module owns
//! no private key, message chain, packet acknowledgment or listener lifecycle.

use anyhow::{ensure, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use openssl::{
    hash::MessageDigest,
    pkey::{Id, PKey, Public},
    rsa::Padding,
    sign::Verifier,
};
use reqwest::{redirect::Policy, Client};
use serde::Deserialize;
use std::{fmt, time::Duration};

pub const SERVICES_PUBLIC_KEYS_URL: &str = "https://api.minecraftservices.com/publickeys";
pub const PROFILE_KEY_EXPIRY_GRACE_MS: i64 = 8 * 60 * 60 * 1000;
pub const EXPIRED_PROFILE_PUBLIC_KEY: &str = "multiplayer.disconnect.expired_public_key";
pub const INVALID_PROFILE_PUBLIC_KEY_SIGNATURE: &str =
    "multiplayer.disconnect.invalid_public_key_signature.new";
pub const MAX_PROFILE_KEY_BYTES: usize = 512;
pub const MAX_PROFILE_KEY_SIGNATURE_BYTES: usize = 4096;

// The original authlib reader has no body-size bound. This explicit native
// transport bound is not presented as a Java codec limit.
const MAX_SERVICES_RESPONSE_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfilePublicKeyData {
    pub expires_at_ms: i64,
    /// Canonical X.509 SubjectPublicKeyInfo for an RSA public key.
    pub public_key_der: Vec<u8>,
    pub key_signature: Vec<u8>,
}

impl ProfilePublicKeyData {
    pub fn new(
        expires_at_ms: i64,
        public_key_der: Vec<u8>,
        key_signature: Vec<u8>,
    ) -> Result<Self> {
        ensure!(
            public_key_der.len() <= MAX_PROFILE_KEY_BYTES,
            "profile public key exceeds512bytes"
        );
        ensure!(
            key_signature.len() <= MAX_PROFILE_KEY_SIGNATURE_BYTES,
            "profile key signature exceeds4096bytes"
        );
        let key = parse_rsa_public_key(&public_key_der)?;
        let public_key_der = key
            .public_key_to_der()
            .context("encode canonical profile public key")?;
        Ok(Self {
            expires_at_ms,
            public_key_der,
            key_signature,
        })
    }

    /// ProfilePublicKey.Data.signedPayload: UUID, expiry, then encoded key.
    /// This is not the legacy PEM/expiry-only certificate representation.
    pub fn signed_payload(&self, profile_uuid: [u8; 16]) -> Result<Vec<u8>> {
        let canonical = parse_rsa_public_key(&self.public_key_der)?
            .public_key_to_der()
            .context("encode profile certificate key")?;
        let mut payload = Vec::with_capacity(24 + canonical.len());
        payload.extend_from_slice(&profile_uuid);
        payload.extend_from_slice(&self.expires_at_ms.to_be_bytes());
        payload.extend_from_slice(&canonical);
        Ok(payload)
    }

    /// Java Instant.isBefore is strict: equality is not expired.
    pub fn has_expired(&self, now_ms: i64) -> bool {
        self.expires_at_ms < now_ms
    }

    /// Caller owns the phase-specific grace. Certificate creation does not
    /// itself apply expiry or this grace period in the pinned source.
    pub fn has_expired_with_grace(&self, now_ms: i64, grace_ms: i64) -> bool {
        i128::from(self.expires_at_ms) + i128::from(grace_ms) < i128::from(now_ms)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileKeyValidationError {
    InvalidSignature,
}

impl ProfileKeyValidationError {
    pub const fn translation_key(self) -> &'static str {
        match self {
            Self::InvalidSignature => INVALID_PROFILE_PUBLIC_KEY_SIGNATURE,
        }
    }
}

impl fmt::Display for ProfileKeyValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.translation_key())
    }
}
impl std::error::Error for ProfileKeyValidationError {}

/// Immutable result of a real services certificate check. The private fields
/// prevent construction from an unvalidated client key.
#[derive(Clone, Debug)]
pub struct ValidatedProfileKey {
    data: ProfilePublicKeyData,
    key: PKey<Public>,
}

impl ValidatedProfileKey {
    pub fn data(&self) -> &ProfilePublicKeyData {
        &self.data
    }

    /// SignatureUpdater streams the caller's exact ordered chunks. A malformed
    /// signature or crypto-provider error is false, as SignatureValidator.from.
    pub fn verify_message_chunks(&self, chunks: &[&[u8]], signature: &[u8]) -> bool {
        verify_chunks(&self.key, MessageDigest::sha256(), chunks, signature)
    }

    pub fn verify_message(&self, payload: &[u8], signature: &[u8]) -> bool {
        self.verify_message_chunks(&[payload], signature)
    }
}

/// ProfilePublicKey.createValidated checks only the services signature. The
/// listener/chain must separately apply its source expiry and replacement rules.
pub fn create_validated(
    profile_uuid: [u8; 16],
    data: ProfilePublicKeyData,
    services: &ServiceKeySet,
) -> std::result::Result<ValidatedProfileKey, ProfileKeyValidationError> {
    let invalid = || ProfileKeyValidationError::InvalidSignature;
    let data =
        ProfilePublicKeyData::new(data.expires_at_ms, data.public_key_der, data.key_signature)
            .map_err(|_| invalid())?;
    let payload = data.signed_payload(profile_uuid).map_err(|_| invalid())?;
    if !services.verify_profile_certificate(&payload, &data.key_signature) {
        return Err(invalid());
    }
    let key = parse_rsa_public_key(&data.public_key_der).map_err(|_| invalid())?;
    Ok(ValidatedProfileKey { data, key })
}

fn parse_rsa_public_key(der: &[u8]) -> Result<PKey<Public>> {
    let key = PKey::public_key_from_der(der).context("decode X.509 RSA public key")?;
    ensure!(key.id() == Id::RSA, "chat public key must use RSA");
    Ok(key)
}

fn verify_chunks(
    key: &PKey<Public>,
    digest: MessageDigest,
    chunks: &[&[u8]],
    signature: &[u8],
) -> bool {
    let result = (|| -> std::result::Result<bool, openssl::error::ErrorStack> {
        let mut verifier = Verifier::new(digest, key)?;
        verifier.set_rsa_padding(Padding::PKCS1)?;
        for chunk in chunks {
            verifier.update(chunk)?;
        }
        verifier.verify(signature)
    })();
    result.unwrap_or(false)
}

/// Snapshot of authlib's PROFILE_KEY and PROFILE_PROPERTY collections. Fields
/// stay private; ordinary player/session packets never populate this owner.
#[derive(Clone, Debug, Default)]
pub struct ServiceKeySet {
    profile_certificate_keys: Vec<PKey<Public>>,
    profile_property_keys: Vec<PKey<Public>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct KeySetResponse {
    #[serde(default)]
    player_certificate_keys: Option<Vec<KeyData>>,
    #[serde(default)]
    profile_property_keys: Option<Vec<KeyData>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct KeyData {
    public_key: String,
}

impl ServiceKeySet {
    pub fn empty() -> Self {
        Self::default()
    }

    /// True means SignatureValidator.from(PROFILE_KEY) is absent in Java.
    /// It must not be replaced by an always-valid validator.
    pub fn is_empty(&self) -> bool {
        self.profile_certificate_keys.is_empty()
    }
    pub fn can_validate_profile_keys(&self) -> bool {
        !self.is_empty()
    }
    pub fn profile_certificate_key_count(&self) -> usize {
        self.profile_certificate_keys.len()
    }
    pub fn profile_property_key_count(&self) -> usize {
        self.profile_property_keys.len()
    }

    /// Parse an authenticated service response or an operator-owned trusted
    /// capture, never JSON supplied by a connecting player. Authlib parses both
    /// collections; a bad key in either invalidates the response as a whole.
    pub fn from_keyset_json(json: &[u8]) -> Result<Self> {
        let response: KeySetResponse =
            serde_json::from_slice(json).context("parse services public keys")?;
        fn parse_list(items: Option<Vec<KeyData>>) -> Result<Vec<PKey<Public>>> {
            items
                .unwrap_or_default()
                .into_iter()
                .map(|item| {
                    let der = STANDARD
                        .decode(item.public_key)
                        .context("decode services RSA public key base64")?;
                    parse_rsa_public_key(&der)
                })
                .collect()
        }
        Ok(Self {
            profile_certificate_keys: parse_list(response.player_certificate_keys)?,
            profile_property_keys: parse_list(response.profile_property_keys)?,
        })
    }

    /// authlib ServicesKeyInfo.signature() is SHA1withRSA for PROFILE_KEY as
    /// well as PROFILE_PROPERTY. Message signatures instead use SHA256withRSA.
    pub fn verify_profile_certificate(&self, payload: &[u8], signature: &[u8]) -> bool {
        self.profile_certificate_keys
            .iter()
            .any(|key| verify_chunks(key, MessageDigest::sha1(), &[payload], signature))
    }

    pub fn verify_profile_property(&self, payload: &[u8], signature: &[u8]) -> bool {
        self.profile_property_keys
            .iter()
            .any(|key| verify_chunks(key, MessageDigest::sha1(), &[payload], signature))
    }

    /// Native authenticated HTTPS fetch; no bearer token or player identity is
    /// sent. A failed fetch returns Err so the refresh owner can retain lastgood.
    /// MinecraftClient uses5-second connection/read timeouts. Native differences
    /// are explicit: redirects are rejected and response bytes are capped1MiB.
    pub async fn fetch_official() -> Result<Self> {
        let client = Client::builder()
            .redirect(Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .read_timeout(Duration::from_secs(5))
            .build()
            .context("configure services public-key HTTPS client")?;
        let mut response = client
            .get(SERVICES_PUBLIC_KEYS_URL)
            .send()
            .await
            .context("fetch official services public keys")?
            .error_for_status()
            .context("services public-key HTTP status")?;
        ensure!(
            response.status().is_success(),
            "unexpected services public-key response status"
        );
        if let Some(length) = response.content_length() {
            ensure!(
                length <= MAX_SERVICES_RESPONSE_BYTES as u64,
                "services public-key response exceeds native1MiB bound"
            );
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .context("read services public keys")?
        {
            ensure!(
                bytes.len().saturating_add(chunk.len()) <= MAX_SERVICES_RESPONSE_BYTES,
                "services public-key response exceeds native1MiB bound"
            );
            bytes.extend_from_slice(&chunk);
        }
        Self::from_keyset_json(&bytes)
    }
}

/// Source refresh/backoff owner, without a hidden background task. Main owns
/// when to run the actual fetch and schedule the returned next delay.
#[derive(Clone, Debug, Default)]
pub struct ServiceKeyRefresh {
    last_good: Option<ServiceKeySet>,
    failures_without_success: u32,
    attempted: bool,
}

impl ServiceKeyRefresh {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn current(&self) -> Option<&ServiceKeySet> {
        self.last_good.as_ref()
    }
    pub fn attempted(&self) -> bool {
        self.attempted
    }

    /// Some includes a successful empty keyset: it is a completed service
    /// response, not a fetch failure. Failed refreshes preserve the lastgood set.
    pub fn record_attempt(&mut self, fetched: Option<ServiceKeySet>) -> Duration {
        self.attempted = true;
        if let Some(keys) = fetched {
            self.last_good = Some(keys);
        }
        if self.last_good.is_some() {
            return Duration::from_secs(24 * 60 * 60);
        }
        let exponent = self.failures_without_success.min(6);
        self.failures_without_success = self.failures_without_success.saturating_add(1);
        Duration::from_secs(5 * 60 * (1_u64 << exponent))
    }
}
