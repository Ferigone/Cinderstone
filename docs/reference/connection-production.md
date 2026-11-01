# Native connection transport integration

This bounded stage targets Java 1.21, protocol 767, using the unchanged pinned
original server and official mappings in `fixtures/source-lock.json`. The published core
commit is `894a3cc`; Main integration is
`87dda91054478098ba86eebaaf32869fa176f307`. Their Git trees are identical to
locally validated `f8c8c72` and `259e641`, respectively. Source inspection used direct JDK21
`javap` and bounded execution of original codecs/registries. REA MCP tools were
not exposed. No proprietary binaries or raw disassembly are published.

## Actual owners and call phases

- `connection()` creates one raw ingress pump, inactivity watchdog and packet
  counter owner for the accepted TCP connection. Their lifetimes cover the
  handshake, STATUS or LOGIN, CONFIGURATION, PLAY and later reconfiguration.
  `ConnectionRegistration` removes that exact counter owner on completion.
- The raw pump precedes `CryptoRead`, retaining original encrypted bytes across
  the encryption switch. Its independent socket reads refresh the actual
  monotonic inactivity deadline, including partial headers/bodies and periods
  when authentication or a handler is awaiting other work. The outer connection
  driver closes on thirty seconds of raw inactivity, without appending a wire
  disconnect packet, matching the source timeout branch.
- The actual server connection phase directly samples every registered packet
  counter once per tick. It does not replay coalesced watch ticks into a single
  received-packet bucket. Sampling and smoothing reuse `ConnectionPacketRate`.
  `rate-limit <= 0` retains counters and disables rate kicks.
- Pure phase validation runs before accepted listener dispatch is counted.
  `main_connection_decode` covers the registered PLAY body shapes and all eight
  CONFIGURATION IDs using existing typed codecs and source-derived fields.
  Beacon holder validation uses an actual original built-in registry export;
  it does not implement beacon gameplay. A decoded packet ignored by
  `shouldHandleMessage` during the PLAY-to-CONFIGURATION acknowledgment boundary
  is validated but not counted. Handled policy refusals remain accepted dispatch.
- All actual `send()` calls count send dispatch before encoding/writing. A rate
  terminal action is handled at a safe writer/input boundary: it cannot cancel
  half a frame and append a new packet to that unfinished ciphertext. It uses
  the current outbound common disconnect codec, stops raw ingress and closes
  after either successful or failed completion. Early protocols cannot encode
  that common packet and therefore close through the failed-codec path.
- The LOGIN 601-tick terminal action is retained while an already-dispatched
  write completes. It is not allowed to append SlowLogin after cancellation of
  a partially written frame. The timer still consumes actual connection ticks.
- CONFIGURATION well-formed but unsolicited/mismatched keepalive sends
  `disconnect.timeout`; malformed bodies remain decoder errors. Finishing
  configuration changes the outbound phase to PLAY before any key/admission
  await. Live PLAY custom payload/Pong bodies now run their shared codecs.
  PLAY Ping Request echoes the actual signed long in Pong Response.

## Configuration

Startup reads one immutable `server.properties` byte snapshot through the
existing Java-properties lexical parser. Its typed values use
`DedicatedConnectionSettings`: `rate-limit=0`,
`network-compression-threshold=256`, `enforce-secure-profile=true`. I/O failure
logs and supplies defaults; malformed Unicode escapes propagate. UTF-8 input
falls back to ISO-8859-1. Unrequested property values are also escape-validated.

Explicit native `--rate-limit`, `--compression-threshold` and
`--enforce-secure-profile` options override the corresponding loaded values.
The deployment entrypoint supplies them only for explicitly configured
`MC_RATE_LIMIT`, `MC_NETWORK_COMPRESSION_THRESHOLD` and
`MC_ENFORCE_SECURE_PROFILE`. A negative compression threshold sends no compression
negotiation; zero enables compression for all subsequent packet bodies. Effective
secure-chat enforcement continues to use the actual online-mode and service-key
owners. This stage does not change the hosted offline setting.

## Evidence and remaining boundaries

The Main build passed. The completed focused local runtime result is recorded in
`docs/validation/connection-flow.json`. Source ledgers:
[packet rate](connection-packet-rate.md), [raw transport](connection-transport.md),
[properties](connection-settings.md), [common payloads](connection-common-payload.md)
and [UTF-8 replacement](connection-utf8.md).

Wire-shape validation does not implement the currently ignored gameplay actions,
all item-component semantics, a general runtime registry/datapack owner or every
malformed decoder behavior. Existing typed-codec hardening limits still apply.
The raw queue has a declared native memory/backpressure bound; it is not a Netty
event-loop equivalence claim. The native 64-connection bound, 15-second write and
initial-chunk deadlines and asynchronous listener scheduling remain explicit.
Canonical property normalization is callable core, not a complete persisted
Java Properties lifecycle. Full profile cache, Gson/date/locale/selector and
Brigadier contexts remain unfinished.

This stage does not complete `authentication.5` or another aggregate checklist
delivery. Scope 1 remains 100 items, with 56 code and 51 Main integrations.
The GitHub integration now accepts issue and Git-object writes. Normal source
files were published with identical trees; commit metadata changed their IDs.
Railway master `1e72694` reached SUCCESS/native offline listening at
`2026-11-01T20:19:15.460728572Z`. Hosted evidence retains the public DNS failure,
one-replica and existing-volume boundaries. No public gameplay is claimed.
