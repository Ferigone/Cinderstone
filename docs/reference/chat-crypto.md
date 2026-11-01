# Profile certificates and chat RSA verification

Target: the pinned original Minecraft Java **1.21**, protocol **767**, with
bundled **authlib 6.0.54**. This ledger records source inspection and callable
native code. It does not claim Main integration, a successful account session,
a live certificate fetch or a completed signed-chat delivery by itself.

## Primary source

The original server and mappings remain pinned by `fixtures/source-lock.json`.
The inner server SHA-256 is
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
The authlib JAR was read from the same original outer server bundle at
`META-INF/libraries/com/mojang/authlib/6.0.54/authlib-6.0.54.jar`.
Direct JDK21 `javap -c -p` inspection was used; the authentication-service URL
concatenation was checked with `javap -v`. No REA MCP execution or Java runtime
observation is inferred. Raw class files/disassembly remain outside the repo.

| Original owner | Inspected implementation | Native responsibility |
| --- | --- | --- |
| ProfilePublicKey / Data | `cna`, `cna$a` | Source certificate payload, validation and expiry helpers |
| SignatureValidator | `azb` | Streaming validation, failure-to-false behavior, absent services validator |
| RemoteChatSession / Data | `xq`, `xq$a` | Actual profile UUID certificate binding and separate session UUID |
| YggdrasilServicesKeyInfo | bundled authlib class and `$1` | Services RSA algorithm, key lists, fetch and refresh policy |
| YggdrasilAuthenticationService / YggdrasilEnvironment | bundled authlib classes | Production HTTPS services host plus `/publickeys` |

## Certificate and message signatures are different

`ProfilePublicKey.Data.signedPayload` streams these exact bytes:

1. The actual profile UUID's most-significant and least-significant longs in
   big-endian order: 16 bytes.
2. `expiresAt.toEpochMilli()` as one big-endian signed long: 8 bytes.
3. `PublicKey.getEncoded()`: canonical X.509 SubjectPublicKeyInfo RSA DER.

The session UUID is **not** part of this certificate payload. It belongs to the
signed-message link handled by the chain owner. This is the current
`signature_v2` representation, not a legacy PEM/expiry-only representation.

Authlib's `ServicesKeyInfo.signature()` uses **SHA1withRSA**, including its
`PROFILE_KEY` collection. A certificate is valid if any trusted profile-key
services key verifies it. `ProfilePublicKey.createSignatureValidator` instead
uses **SHA256withRSA** with the certified player's key for message signatures.
Both map to RSA PKCS#1 v1.5 verification through OpenSSL; ordered signature
chunks are streamed without a substitute concatenation or rehash convention.
Provider errors and malformed signatures return false rather than acceptance.

The RSA profile key is read from at most 512 wire bytes, and its services
signature from at most 4,096 wire bytes. The source services key-info reports
4,096 bits, but its parser only checks the RSA key algorithm. The native parser
does not invent a services-key modulus-size restriction from that report.

## Expiry and trust ownership

`ProfilePublicKey.createValidated` checks the services signature **without an
expiry check**. The connection listener, replacement rule and message decoder
own their separate expiry decisions. `Data.hasExpired()` uses strict
`expiresAt.isBefore(now)`, so exact equality is not expired. The source also
exposes an eight-hour grace duration for the phases that explicitly request it;
the crypto constructor does not silently apply that grace.

`ValidatedProfileKey` has private fields and can only be obtained through
`create_validated`. It stores the checked data and parsed RSA key. It contains
no signing/private key. The actual profile UUID is supplied by the native
authenticated or selected offline profile owner, not copied from an untrusted
certificate claim.

Services trust is represented by immutable `ServiceKeySet` snapshots. The
production source URL is `https://api.minecraftservices.com/publickeys`.
Authlib maps `playerCertificateKeys` to `PROFILE_KEY` and `profilePropertyKeys`
to `PROFILE_PROPERTY`. Both lists are parsed; a malformed key in either makes
the fetch fail. Missing/null collections become empty. An empty PROFILE_KEY
collection means `SignatureValidator.from` returns **null** in Java. The
listener must handle absence as its source branch specifies; it must not install
an always-valid validator or use a connecting player's own key as a trust root.

`ServiceKeySet::from_keyset_json` is for an authenticated service response or an
operator-owned trusted capture. It is not a client packet decoder. Main must
never feed a client-provided keyset to it. The native official fetch sends no
player identity, access token or other credential.

## Refresh lifecycle

Authlib starts the fetch asynchronously when the authentication service is
constructed. Its lazy consumer waits for the first **attempt** to finish, not
necessarily for success. Successful fetches replace the current snapshot;
failed fetches keep an existing snapshot.

Before the first successful response, retry delays are
`5 minutes × 2^min(failureCount++, 6)`. Once any response successfully creates a
keyset, including an empty keyset, the next delay is 24 hours. Later failures
retain the last good snapshot and continue the 24-hour cadence. The native
`ServiceKeyRefresh` exposes this state and delay; it does not spawn a hidden
task. Main owns scheduling, the active snapshot and shutdown lifetime.

## Callable API and boundaries

`src/chat_crypto.rs` provides:

- `ProfilePublicKeyData::new`, `signed_payload`, `has_expired` and
  `has_expired_with_grace`.
- `create_validated`, `ValidatedProfileKey::data`, `verify_message` and
  `verify_message_chunks`.
- `ServiceKeySet::empty`, `is_empty`, `can_validate_profile_keys`,
  `from_keyset_json`, `fetch_official` and collection counts.
- `ServiceKeyRefresh::record_attempt`, `current` and `attempted` for the real
  caller-owned asynchronous refresh lifecycle.

The native fetch uses authenticated TLS, the source five-second connection and
read timeouts, and two explicit native boundaries: redirects are rejected and
responses are capped at 1 MiB. Authlib's reader does not have that response cap.
Custom authlib environment overrides, every Jackson JSON/Base64 coercion and
exact Java-provider ASN.1 acceptance are not claimed. The native profile expiry
inputs have millisecond precision; the wire instant and certificate payload
are also milliseconds. These helpers do not establish the rest of chat session
state, secure-message chains, acknowledgments, command signing, filtering,
message publication or overall Minecraft compatibility.

Root records compiler/runtime and Main integration evidence separately.
