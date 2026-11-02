# Loopback services public-key source

`--loopback-service-keys PATH` selects an operator-owned local services public-key
JSON file for an offline loopback server. The option requires `--allow-offline`,
`online_mode=false` and a loopback bind IP. Online mode and public bind addresses
are rejected even when public offline access is otherwise enabled. Main checks
the effective options and the actual listener address.

```bash
./target/release/mc121-rust --allow-offline --bind 127.0.0.1 \
  --loopback-service-keys /path/to/public-services-keys.json
```

The file uses the existing `ServiceKeySet::from_keyset_json` services-response
parser. It contains public keys, never a private signing key. A valid empty set
is explicit and supplies no certificate validator:

```json
{"playerCertificateKeys":[],"profilePropertyKeys":[]}
```

Main reads and parses the initial regular file before binding a listener or
starting any key-refresh task. Missing, malformed, non-regular or oversized
input fails startup. Metadata and the bytes actually read are bounded; accepted
JSON is at most 1 MiB. No local-source error selects an HTTP source.

The selected source is fixed for the server lifetime. Its already parsed first
snapshot enters the existing `ServiceKeyRefresh`, watch channel, last-good cache
and first-attempt readiness owner. Successful empty sets are real parser
results. Subsequent refreshes reread the same local path. They retain the normal
24-hour successful refresh cadence and last-good behavior on a later local read
failure. No file attempt invokes or falls back to official HTTPS.

Before the listening event, Main emits:

```json
{"event":"chat_service_key_source","source":"loopback-file","network_refresh":false}
```

The existing `chat_service_keys_attempt_complete` and
`chat_service_keys_unavailable` events also include `source: "loopback-file"`.
An empty first result reports `profile_keys: 0` and
`next_refresh_seconds: 86400`. Selection and readiness are not certificate
validation or signed-chat success.

Omitting the option retains the official HTTPS source and its existing refresh
behavior, identified as `source: "official-https"`. Authentication, offline UUID
derivation, certificate validation, signed-message validation and the effective
secure-profile predicate are unchanged. An empty file-backed set validates no
profile certificates; unsigned offline physics flows do not establish a signed
session or trusted Mojang certificate.

This explicit native execution option is not part of vanilla's CLI. Source
review and the isolated build/runtime checks must identify their exact code and
binary; this document does not itself claim those checks passed.
