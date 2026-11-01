//! Minecraft Java 1.21 online-login cryptography and session verification.
//!
//! The wire contracts were checked against the shipped `axl`, `arw`, `air`,
//! `aiz`, `aiq`, and `yv$19/$20` classes. Cryptographic primitive tests also use
//! Node/OpenSSL independently; a mocked session service is not proof that a real
//! Microsoft-authenticated account has successfully joined the server.

use aes::{
    cipher::{BlockEncrypt, KeyInit},
    Aes128,
};
use anyhow::{bail, ensure, Context, Result};
use openssl::{
    encrypt::Decrypter,
    pkey::{PKey, Private},
    rsa::{Padding, Rsa},
};
use rand::{rngs::OsRng, RngCore};
use reqwest::{redirect::Policy, Client, StatusCode, Url};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::{
    io,
    net::IpAddr,
    pin::Pin,
    task::{Context as TaskContext, Poll},
    time::Duration,
};
use subtle::ConstantTimeEq;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use zeroize::Zeroize;

use crate::protocol::{Reader, Writer};

const SESSION_URL: &str = "https://sessionserver.mojang.com/session/minecraft/hasJoined";
const MAX_PROFILE_BODY: usize = 4 * 1024 * 1024;
const WRITE_BUFFER_LIMIT: usize = 64 * 1024;

/// Printable ASCII and at most 16 UTF-16 units, matching `StringUtil.isValidPlayerName`.
/// Vanilla's helper does not separately reject the empty string.
pub fn valid_login_name(name: &str) -> bool {
    name.len() <= 16 && name.bytes().all(|byte| (33..127).contains(&byte))
}

/// A connection-local verify token. It is public wire data, not a session secret.
pub struct LoginChallenge {
    verify_token: [u8; 4],
}

impl LoginChallenge {
    pub fn verify_token(&self) -> &[u8; 4] {
        &self.verify_token
    }
}

/// No Debug/Serialize implementation: the shared AES secret must not enter logs.
pub struct SharedSecret([u8; 16]);

impl SharedSecret {
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }
    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl Drop for SharedSecret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// One server RSA keypair; each login obtains a separate unpredictable challenge.
pub struct Authenticator {
    private: PKey<Private>,
    public_der: Vec<u8>,
}

impl Authenticator {
    pub fn new() -> Result<Self> {
        // OpenSSL 3.2+ uses implicit rejection for PKCS#1 v1.5 in its default
        // provider. RustCrypto rsa 0.9 is intentionally not used: its documented
        // Marvin timing advisory remains unpatched for network decryption.
        ensure!(
            openssl::version::number() >= 0x3020_0000,
            "online authentication requires OpenSSL 3.2 or newer"
        );
        let private = PKey::from_rsa(Rsa::generate(1024).context("generate Minecraft RSA key")?)?;
        let public_der = private
            .public_key_to_der()
            .context("encode Minecraft SPKI public key")?;
        Ok(Self {
            private,
            public_der,
        })
    }

    pub fn public_key_der(&self) -> &[u8] {
        &self.public_der
    }

    pub fn challenge(&self) -> LoginChallenge {
        let mut verify_token = [0; 4];
        OsRng.fill_bytes(&mut verify_token);
        LoginChallenge { verify_token }
    }

    /// Login clientbound 0x01 payload; the server ID is deliberately empty in vanilla.
    pub fn encryption_request(&self, challenge: &LoginChallenge) -> Vec<u8> {
        let mut out = Writer::new();
        out.string("")
            .varint(self.public_der.len() as i32)
            .raw(&self.public_der)
            .varint(4)
            .raw(&challenge.verify_token)
            .bool(true);
        out.into_inner()
    }

    /// Consume the two length-prefixed RSA ciphertexts from Login serverbound 0x01.
    /// Both invalid padding and a wrong token fail; neither causes offline fallback.
    pub fn accept_response(
        &self,
        challenge: &LoginChallenge,
        payload: &[u8],
    ) -> Result<SharedSecret> {
        let mut input = Reader::new(payload);
        let encrypted_secret = rsa_field(&mut input, self.private.size())?;
        let encrypted_token = rsa_field(&mut input, self.private.size())?;
        input.finish()?;
        // Attempt both operations before returning the same generic wire-level
        // error. OpenSSL owns blinding and PKCS#1 padding/implicit rejection.
        let secret = self.decrypt(encrypted_secret);
        let token = self.decrypt(encrypted_token);
        let mut secret = secret.unwrap_or_default();
        let mut token = token.unwrap_or_default();
        let valid_secret = secret.len() == 16;
        let valid_token =
            token.len() == 4 && bool::from(token.as_slice().ct_eq(&challenge.verify_token));
        if !valid_secret || !valid_token {
            secret.zeroize();
            token.zeroize();
            bail!("invalid encryption response");
        }
        let mut bytes = [0; 16];
        bytes.copy_from_slice(&secret);
        secret.zeroize();
        token.zeroize();
        Ok(SharedSecret(bytes))
    }

    pub fn session_hash(&self, secret: &SharedSecret) -> String {
        minecraft_session_hash("", secret.as_bytes(), &self.public_der)
    }

    fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        let mut decrypter = Decrypter::new(&self.private)?;
        decrypter.set_rsa_padding(Padding::PKCS1)?;
        let mut output = vec![0; self.private.size()];
        match decrypter.decrypt(ciphertext, &mut output) {
            Ok(length) => {
                output.truncate(length);
                Ok(output)
            }
            Err(_) => {
                output.zeroize();
                bail!("invalid encryption response")
            }
        }
    }
}

fn rsa_field<'a>(input: &mut Reader<'a>, size: usize) -> Result<&'a [u8]> {
    let length = input.varint()?;
    ensure!(
        length >= 0 && length as usize == size,
        "invalid encryption response"
    );
    input.bytes(size)
}

/// Java's SHA-1 digest converted with `new BigInteger(digest).toString(16)`.
pub fn minecraft_session_hash(server_id: &str, secret: &[u8], public_der: &[u8]) -> String {
    let mut hash = Sha1::new();
    // String.getBytes(ISO_8859_1) replaces unmappable code points with '?'.
    hash.update(
        server_id
            .chars()
            .map(|ch| u8::try_from(u32::from(ch)).unwrap_or(b'?'))
            .collect::<Vec<_>>(),
    );
    hash.update(secret);
    hash.update(public_der);
    signed_digest_hex(&hash.finalize())
}

fn signed_digest_hex(bytes: &[u8]) -> String {
    let negative = bytes.first().is_some_and(|byte| byte & 0x80 != 0);
    let mut magnitude = bytes.to_vec();
    if negative {
        let mut carry = 1_u16;
        for byte in magnitude.iter_mut().rev() {
            let value = u16::from(!*byte) + carry;
            *byte = value as u8;
            carry = value >> 8;
        }
    }
    let Some(first) = magnitude.iter().position(|byte| *byte != 0) else {
        return "0".into();
    };
    let mut result = if negative {
        "-".to_owned()
    } else {
        String::new()
    };
    result.push_str(&format!("{:x}", magnitude[first]));
    for byte in &magnitude[first + 1..] {
        result.push_str(&format!("{byte:02x}"));
    }
    result
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileProperty {
    pub name: String,
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedProfile {
    pub uuid: [u8; 16],
    pub name: String,
    pub properties: Vec<ProfileProperty>,
}

impl VerifiedProfile {
    pub fn login_success_payload(&self) -> Vec<u8> {
        let mut out = Writer::new();
        out.uuid(self.uuid)
            .string(&self.name)
            .varint(self.properties.len() as i32);
        for property in &self.properties {
            out.string(&property.name)
                .string(&property.value)
                .bool(property.signature.is_some());
            if let Some(signature) = &property.signature {
                out.string(signature);
            }
        }
        out.bool(true); // ClientboundGameProfilePacket strictErrorHandling in vanilla 1.21.
        out.into_inner()
    }
}

#[derive(Deserialize)]
struct SessionProfile {
    id: String,
    name: String,
    #[serde(default)]
    properties: Vec<ProfileProperty>,
}

/// The production endpoint is fixed. Unavailable or invalid authentication always fails closed.
#[derive(Clone)]
pub struct SessionVerifier {
    client: Client,
    endpoint: Url,
}

impl SessionVerifier {
    pub fn new() -> Result<Self> {
        Self::build(Url::parse(SESSION_URL).expect("constant Mojang session URL"))
    }

    fn build(endpoint: Url) -> Result<Self> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(4))
            .timeout(Duration::from_secs(10))
            .redirect(Policy::none())
            .user_agent("mc121-rust/0.1.0")
            .build()
            .context("configure session verification HTTPS client")?;
        Ok(Self { client, endpoint })
    }

    pub async fn verify(
        &self,
        username: &str,
        server_hash: &str,
        ip: Option<IpAddr>,
    ) -> Result<VerifiedProfile> {
        ensure!(valid_login_name(username), "invalid login name");
        let magnitude = server_hash.strip_prefix('-').unwrap_or(server_hash);
        ensure!(
            !magnitude.is_empty()
                && magnitude.len() <= 40
                && magnitude.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "invalid session hash"
        );
        let mut request = self
            .client
            .get(self.endpoint.clone())
            .query(&[("username", username), ("serverId", server_hash)]);
        if let Some(ip) = ip {
            request = request.query(&[("ip", ip.to_string())]);
        }
        let mut response = request
            .send()
            .await
            .context("authentication servers unavailable")?;
        match response.status() {
            StatusCode::OK => {}
            StatusCode::NO_CONTENT | StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                bail!("failed to verify username")
            }
            status => bail!("authentication service returned HTTP {}", status.as_u16()),
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_PROFILE_BODY as u64)
        {
            bail!("authentication profile exceeds size limit");
        }
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .context("read authentication profile")?
        {
            ensure!(
                body.len().saturating_add(chunk.len()) <= MAX_PROFILE_BODY,
                "authentication profile exceeds size limit"
            );
            body.extend_from_slice(&chunk);
        }
        parse_profile(&body, username)
    }
}

fn parse_profile(body: &[u8], username: &str) -> Result<VerifiedProfile> {
    let profile: SessionProfile =
        serde_json::from_slice(body).context("invalid authentication profile JSON")?;
    ensure!(
        valid_login_name(&profile.name) && profile.name.eq_ignore_ascii_case(username),
        "authentication profile name mismatch"
    );
    let uuid = parse_uuid(&profile.id)?;
    ensure!(
        profile.properties.len() <= 16,
        "authentication profile has too many properties"
    );
    for property in &profile.properties {
        ensure!(
            property.name.encode_utf16().count() <= 64,
            "authentication property name too long"
        );
        ensure!(
            property.value.encode_utf16().count() <= 32767,
            "authentication property value too long"
        );
        ensure!(
            property
                .signature
                .as_ref()
                .is_none_or(|value| value.encode_utf16().count() <= 1024),
            "authentication property signature too long"
        );
    }
    Ok(VerifiedProfile {
        uuid,
        name: profile.name,
        properties: profile.properties,
    })
}

fn parse_uuid(input: &str) -> Result<[u8; 16]> {
    let compact = if input.len() == 36 {
        ensure!(
            [8, 13, 18, 23]
                .iter()
                .all(|&offset| input.as_bytes()[offset] == b'-'),
            "invalid profile UUID"
        );
        input.chars().filter(|ch| *ch != '-').collect::<String>()
    } else {
        input.to_owned()
    };
    ensure!(
        compact.len() == 32 && compact.is_ascii(),
        "invalid profile UUID"
    );
    let mut uuid = [0; 16];
    for (index, byte) in uuid.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&compact[index * 2..index * 2 + 2], 16)
            .context("invalid profile UUID")?;
    }
    Ok(uuid)
}

/// Continuous CFB8 feedback. One AES block is evaluated for each stream byte.
struct Cfb8 {
    aes: Aes128,
    feedback: [u8; 16],
}

impl Cfb8 {
    fn new(secret: &SharedSecret) -> Self {
        Self {
            aes: Aes128::new(secret.as_bytes().into()),
            feedback: *secret.as_bytes(),
        }
    }

    fn apply(&mut self, bytes: &mut [u8], decrypt: bool) {
        for byte in bytes {
            let mut block = self.feedback.into();
            self.aes.encrypt_block(&mut block);
            let input = *byte;
            *byte ^= block[0];
            self.feedback.copy_within(1.., 0);
            self.feedback[15] = if decrypt { input } else { *byte };
        }
    }
}

impl Drop for Cfb8 {
    fn drop(&mut self) {
        self.feedback.zeroize();
    }
}

/// A reader that starts plaintext and is enabled precisely after Encryption Response.
pub struct CryptoRead<R> {
    inner: R,
    cipher: Option<Cfb8>,
}

impl<R> CryptoRead<R> {
    pub fn plain(inner: R) -> Self {
        Self {
            inner,
            cipher: None,
        }
    }
    pub fn enable(&mut self, secret: &SharedSecret) -> Result<()> {
        ensure!(
            self.cipher.is_none(),
            "stream encryption is already enabled"
        );
        self.cipher = Some(Cfb8::new(secret));
        Ok(())
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for CryptoRead<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let start = buffer.filled().len();
        match Pin::new(&mut this.inner).poll_read(cx, buffer) {
            Poll::Ready(Ok(())) => {
                if let Some(cipher) = &mut this.cipher {
                    cipher.apply(&mut buffer.filled_mut()[start..], true);
                }
                Poll::Ready(Ok(()))
            }
            value => value,
        }
    }
}

/// Buffered encryption writer. Its cipher advances only when bytes are accepted
/// into its bounded buffer; Pending never consumes the caller's new plaintext.
pub struct CryptoWrite<W> {
    inner: W,
    cipher: Option<Cfb8>,
    pending: Vec<u8>,
    offset: usize,
}

impl<W> CryptoWrite<W> {
    pub fn plain(inner: W) -> Self {
        Self {
            inner,
            cipher: None,
            pending: Vec::new(),
            offset: 0,
        }
    }
    pub fn enable(&mut self, secret: &SharedSecret) -> Result<()> {
        ensure!(
            self.cipher.is_none() && self.pending.is_empty(),
            "stream encryption is already enabled"
        );
        self.cipher = Some(Cfb8::new(secret));
        Ok(())
    }
}

impl<W: AsyncWrite + Unpin> CryptoWrite<W> {
    fn drain(&mut self, cx: &mut TaskContext<'_>) -> Poll<io::Result<()>> {
        while self.offset < self.pending.len() {
            match Pin::new(&mut self.inner).poll_write(cx, &self.pending[self.offset..]) {
                Poll::Ready(Ok(0)) => return Poll::Ready(Err(io::ErrorKind::WriteZero.into())),
                Poll::Ready(Ok(count)) => self.offset += count,
                Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
                Poll::Pending => return Poll::Pending,
            }
        }
        self.pending.clear();
        self.offset = 0;
        Poll::Ready(Ok(()))
    }
}

impl<W: AsyncWrite + Unpin> AsyncWrite for CryptoWrite<W> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        if this.cipher.is_none() {
            return Pin::new(&mut this.inner).poll_write(cx, bytes);
        }
        match this.drain(cx) {
            Poll::Ready(Ok(())) => {}
            Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
            Poll::Pending => return Poll::Pending,
        }
        if bytes.is_empty() {
            return Poll::Ready(Ok(0));
        }
        let count = bytes.len().min(WRITE_BUFFER_LIMIT);
        this.pending.extend_from_slice(&bytes[..count]);
        this.cipher
            .as_mut()
            .expect("encryption checked")
            .apply(&mut this.pending, false);
        // Bytes are now accepted by this buffering adapter, irrespective of the
        // underlying socket's readiness. flush/shutdown and the next write drain it.
        Poll::Ready(Ok(count))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        match this.drain(cx) {
            Poll::Ready(Ok(())) => Pin::new(&mut this.inner).poll_flush(cx),
            other => other,
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        match this.drain(cx) {
            Poll::Ready(Ok(())) => Pin::new(&mut this.inner).poll_shutdown(cx),
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn java_signed_big_integer_hex_boundaries() {
        assert_eq!(signed_digest_hex(&[0; 20]), "0");
        assert_eq!(signed_digest_hex(&[255; 20]), "-1");
        let mut minimum = [0; 20];
        minimum[0] = 128;
        assert_eq!(
            signed_digest_hex(&minimum),
            "-8000000000000000000000000000000000000000"
        );
        let mut one = [0; 20];
        one[19] = 1;
        assert_eq!(signed_digest_hex(&one), "1");
        let mut positive = [255; 20];
        positive[0] = 127;
        assert_eq!(
            signed_digest_hex(&positive),
            "7fffffffffffffffffffffffffffffffffffffff"
        );
    }

    #[test]
    fn login_name_matches_printable_ascii_vanilla_rule() {
        for name in ["", "Notch", "Hello,World!", "1234567890123456"] {
            assert!(valid_login_name(name));
        }
        for name in ["has space", "tab\t", "\u{7f}", "ą", "12345678901234567"] {
            assert!(!valid_login_name(name));
        }
    }

    #[test]
    fn profile_limits_uuid_properties_and_strict_success() {
        let body = br#"{"id":"0123456789abcdef0123456789abcdef","name":"Notch","properties":[{"name":"textures","value":"fixture","signature":"signature"}]}"#;
        let profile = parse_profile(body, "notch").unwrap();
        assert_eq!(
            profile.uuid,
            [1, 35, 69, 103, 137, 171, 205, 239, 1, 35, 69, 103, 137, 171, 205, 239]
        );
        let bytes = profile.login_success_payload();
        let mut reader = Reader::new(&bytes);
        assert_eq!(reader.uuid().unwrap(), profile.uuid);
        assert_eq!(reader.string(16).unwrap(), "Notch");
        assert_eq!(reader.varint().unwrap(), 1);
        assert_eq!(reader.string(64).unwrap(), "textures");
        assert_eq!(reader.string(32767).unwrap(), "fixture");
        assert!(reader.bool().unwrap());
        assert_eq!(reader.string(1024).unwrap(), "signature");
        assert!(reader.bool().unwrap());
        reader.finish().unwrap();
        assert!(parse_profile(body, "other").is_err());
        assert!(parse_uuid("01234567-89ab-cdef-0123-456789abcdef").is_ok());
        assert!(parse_uuid("01234567-89ab_cdef-0123-456789abcdef").is_err());
        let mut invalid: serde_json::Value = serde_json::from_slice(body).unwrap();
        invalid["properties"] =
            serde_json::Value::Array(vec![serde_json::json!({"name":"x","value":"v"}); 17]);
        assert!(parse_profile(&serde_json::to_vec(&invalid).unwrap(), "Notch").is_err());
    }

    #[test]
    fn rsa_request_response_and_wrong_token_fail_closed() {
        let authenticator = Authenticator::new().unwrap();
        let challenge = authenticator.challenge();
        let request = authenticator.encryption_request(&challenge);
        let mut reader = Reader::new(&request);
        assert_eq!(reader.string(20).unwrap(), "");
        let len = reader.varint().unwrap() as usize;
        assert_eq!(reader.bytes(len).unwrap(), authenticator.public_key_der());
        assert_eq!(reader.varint().unwrap(), 4);
        assert_eq!(reader.bytes(4).unwrap(), challenge.verify_token());
        assert!(reader.bool().unwrap());
        reader.finish().unwrap();
        let key = authenticator.private.rsa().unwrap();
        let encrypt = |input: &[u8]| {
            let mut output = vec![0; key.size() as usize];
            let length = key
                .public_encrypt(input, &mut output, Padding::PKCS1)
                .unwrap();
            output.truncate(length);
            output
        };
        let secret = [42; 16];
        let encrypted_secret = encrypt(&secret);
        let encrypted_token = encrypt(challenge.verify_token());
        let mut response = Writer::new();
        response
            .varint(128)
            .raw(&encrypted_secret)
            .varint(128)
            .raw(&encrypted_token);
        let valid = response.into_inner();
        assert_eq!(
            authenticator
                .accept_response(&challenge, &valid)
                .unwrap()
                .as_bytes(),
            &secret
        );
        let wrong_token = encrypt(&[challenge.verify_token[0] ^ 255; 4]);
        let mut response = Writer::new();
        response
            .varint(128)
            .raw(&encrypted_secret)
            .varint(128)
            .raw(&wrong_token);
        assert!(authenticator
            .accept_response(&challenge, &response.into_inner())
            .is_err());
        let mut trailing = valid.clone();
        trailing.push(0);
        assert!(authenticator
            .accept_response(&challenge, &trailing)
            .is_err());
        assert!(authenticator.accept_response(&challenge, &[0, 0]).is_err());
    }

    #[tokio::test]
    async fn cfb8_survives_partial_writes_reads_and_plaintext_transition() {
        let secret = SharedSecret::from_bytes([19; 16]);
        let (a, b) = tokio::io::duplex(7);
        let mut writer = CryptoWrite::plain(a);
        let mut reader = CryptoRead::plain(b);
        writer.write_all(b"plain").await.unwrap();
        let mut prefix = [0; 5];
        reader.read_exact(&mut prefix).await.unwrap();
        assert_eq!(&prefix, b"plain");
        writer.enable(&secret).unwrap();
        reader.enable(&secret).unwrap();
        let plain: Vec<_> = (0..200_000).map(|i| (i % 251) as u8).collect();
        let expected = plain.clone();
        let writing = tokio::spawn(async move {
            for chunk in plain.chunks(131) {
                writer.write_all(chunk).await.unwrap();
            }
            writer.shutdown().await.unwrap();
        });
        let mut actual = Vec::new();
        reader.read_to_end(&mut actual).await.unwrap();
        writing.await.unwrap();
        assert_eq!(actual, expected);
    }

    async fn mock_service(
        status: &str,
        body: String,
        declared_length: Option<usize>,
    ) -> (SessionVerifier, tokio::task::JoinHandle<String>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = Url::parse(&format!(
            "http://{}/session/minecraft/hasJoined",
            listener.local_addr().unwrap()
        ))
        .unwrap();
        let status = status.to_owned();
        let serving = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut byte = [0];
            while !request.ends_with(b"\r\n\r\n") {
                socket.read_exact(&mut byte).await.unwrap();
                request.push(byte[0]);
                assert!(request.len() <= 16384);
            }
            let response=format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",declared_length.unwrap_or(body.len()));
            let _ = socket.write_all(response.as_bytes()).await;
            String::from_utf8(request).unwrap()
        });
        // This client only addresses the in-process loopback fixture. The
        // production constructor retains inherited proxies and validated TLS.
        let verifier = SessionVerifier {
            client: Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(2))
                .redirect(Policy::none())
                .build()
                .unwrap(),
            endpoint,
        };
        (verifier, serving)
    }

    #[tokio::test]
    async fn session_service_query_profile_and_fail_closed_statuses() {
        let body=r#"{"id":"0123456789abcdef0123456789abcdef","name":"Notch","properties":[{"name":"textures","value":"fixture"}]}"#.to_owned();
        let (verifier, server) = mock_service("200 OK", body, None).await;
        let profile = verifier
            .verify("NoTcH", "-1a23", Some("2001:db8::1".parse().unwrap()))
            .await
            .unwrap();
        assert_eq!(profile.name, "Notch");
        assert_eq!(profile.properties.len(), 1);
        let request = server.await.unwrap();
        assert!(request.starts_with("GET /session/minecraft/hasJoined?username=NoTcH&serverId=-1a23&ip=2001%3Adb8%3A%3A1 HTTP/1.1\r\n"));
        for status in [
            "204 No Content",
            "401 Unauthorized",
            "403 Forbidden",
            "503 Service Unavailable",
            "302 Found",
        ] {
            let (verifier, server) = mock_service(status, String::new(), None).await;
            assert!(
                verifier.verify("Notch", "1234", None).await.is_err(),
                "{status}"
            );
            server.await.unwrap();
        }
    }

    #[tokio::test]
    async fn session_service_rejects_untrusted_or_oversized_profiles() {
        for body in [
            "{}",
            "not JSON",
            r#"{"id":"bad","name":"Notch"}"#,
            r#"{"id":"0123456789abcdef0123456789abcdef","name":"Other"}"#,
        ] {
            let (verifier, server) = mock_service("200 OK", body.to_owned(), None).await;
            assert!(verifier.verify("Notch", "1234", None).await.is_err());
            server.await.unwrap();
        }
        let (verifier, server) =
            mock_service("200 OK", String::new(), Some(MAX_PROFILE_BODY + 1)).await;
        assert!(verifier.verify("Notch", "1234", None).await.is_err());
        server.await.unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = Url::parse(&format!(
            "http://{}/session/minecraft/hasJoined",
            listener.local_addr().unwrap()
        ))
        .unwrap();
        drop(listener);
        let verifier = SessionVerifier {
            client: Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(2))
                .build()
                .unwrap(),
            endpoint,
        };
        assert!(verifier.verify("Notch", "1234", None).await.is_err());
    }

    #[test]
    fn profile_string_bounds_count_java_utf16_units() {
        let mut profile = serde_json::json!({"id":"0123456789abcdef0123456789abcdef","name":"Notch",
            "properties":[{"name":"😀".repeat(32),"value":"😀".repeat(16383)+"a","signature":"😀".repeat(512)}]});
        assert!(parse_profile(&serde_json::to_vec(&profile).unwrap(), "Notch").is_ok());
        profile["properties"][0]["name"] = serde_json::json!("😀".repeat(33));
        assert!(parse_profile(&serde_json::to_vec(&profile).unwrap(), "Notch").is_err());
        profile["properties"][0]["name"] = serde_json::json!("textures");
        profile["properties"][0]["value"] = serde_json::json!("😀".repeat(16384));
        assert!(parse_profile(&serde_json::to_vec(&profile).unwrap(), "Notch").is_err());
        profile["properties"][0]["value"] = serde_json::json!("fixture");
        profile["properties"][0]["signature"] = serde_json::json!("😀".repeat(513));
        assert!(parse_profile(&serde_json::to_vec(&profile).unwrap(), "Notch").is_err());
    }
}
