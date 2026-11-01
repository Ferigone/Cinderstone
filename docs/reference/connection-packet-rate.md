# Native connection packet-rate adapter

Target: original Minecraft Java 1.21, protocol 767. The source artifact and
official mappings are pinned in `fixtures/source-lock.json`: server SHA-1
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`, inner JAR SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`, and
mappings SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`.

The existing numerical owner is `ConnectionPacketRate` in `src/login_limits.rs`.
The new `src/main_connection_rate.rs` adapter shares that owner between the
actual server connection registry, listener dispatch and output writer. It
does not introduce another numerical limiter or change the default property.
Production callsites and runtime validation belong to the integration stage;
this adapter alone is not proof of Main integration or a completed checklist row.

## Pinned source observations

This review used direct JDK 21 `javap -p -c` of the pinned original inner JAR and
the official mappings. No REA MCP tool was exposed. This is static bytecode
inspection, not a Java gameplay observation. Raw disassembly remains private.

| Original mapped method | Observed contract |
| --- | --- |
| `Connection.channelRead0`, lines 184–204 (`vt.a`) | Returns if the channel is closed; requires an initialized listener; calls its `shouldHandleMessage` predicate. Only an accepted packet reaches dispatch and the received-counter tail. |
| Same dispatch exception table | `RunningOnDifferentThreadException`, `RejectedExecutionException` and `ClassCastException` all reach the counter increment. The latter two first disconnect with `multiplayer.disconnect.server_shutdown` or `multiplayer.disconnect.invalid_packet`. An arbitrary unhandled exception does not reach that tail. |
| `Connection.sendPacket`, lines 341–347 (`vt.b`) | Increments the sent counter before checking whether event-loop scheduling is needed. A dispatch that subsequently fails encoding or writing has still counted. |
| `Connection.tick`, lines 398–418 (`vt.b`) | Flushes pending actions, ticks the actual listener, handles disconnection, flushes the channel, then checks `tickCount++ % 20 == 0`. Samples are on actual connection ticks 1, 21, 41, and so on. |
| `Connection.tickSecond`, lines 421–425 (`vt.c`) | Uses `Mth.lerp(0.75f, currentCount, previousAverage)` with separate float operations, then clears both current counters. The current sample weight is 0.25; this is not an unsmoothed packet count. |
| `RateKickingConnection.tickSecond`, lines 22–31 (`wj.c`) | Invokes the base sampler first; only a strict comparison `averageReceived > (float) configuredLimit` triggers the common `ClientboundDisconnectPacket`. Its reason is `disconnect.exceeded_packet_rate`. |
| Same method | Dispatches that common packet with `PacketSendListener.thenRun`, then invokes `setReadOnly`. It does not substitute a LOGIN disconnect packet. |
| `Connection.setReadOnly`, lines 629–632 (`vt.m`) | Sets the actual channel's `autoRead` to false. It does not change `shouldHandleMessage` or undo previously accepted dispatch. |
| `PacketSendListener$1.onSuccess/onFailure`, lines 13–20 (`wg$1.a/b`) | Both invoke the close runnable. Failure returns no replacement packet. The native host must close on either send outcome. |
| `Connection.doSendPacket` completion, lines 350–365 (`vt.c`) | Calls the corresponding success or failure method of the send listener before its separate exception-on-failure listener. |
| `LoginProtocols`, `StatusProtocols`, `ConfigurationProtocols`, `GameProtocols` (`aiv`, `aju`, `abj`, `agh`) | The common disconnect packet codec is registered in CONFIGURATION and PLAY, and is absent in LOGIN and STATUS. A rate action before a common outbound codec exists is not a valid phase-specific disconnect-reason delivery. |

The construction/property contract is unchanged from
[login-limits.md](login-limits.md): `ServerConnectionListener` creates the
specialized limiter only for positive `rate-limit`; default 0 retains ordinary
connection statistics without a rate kick. There is no per-IP throttle here.

## Exported Main adapter contract

`ConnectionRate` is a cloneable handle to one `Arc` owner. It is installed at
actual TCP acceptance and retained across HANDSHAKE, STATUS/LOGIN,
CONFIGURATION, PLAY and reconfiguration. The registry removes it on closure.

- `new(rate_limit_property)` retains the existing core's positive-limit rule.
- `accepted_packet_received()` records the accepted dispatch/scheduling tail,
  including the handled source exceptions above. Do not call it for a raw frame,
  a decoder failure or a `shouldHandleMessage == false` packet. Read-only state
  deliberately does not reject an already accepted counter event.
- `packet_send_dispatched()` records send dispatch before encoding/writing,
  including the disconnect packet itself and a later failed write.
- `tick_connection()` is called directly by the actual registry connection-tick
  phase, after listener work/flush. It samples the same numerical owner and
  publishes the first terminal action to a persisted watch value. No packet
  count is reconstructed from a watch notification or elapsed milliseconds.
- `subscribe_terminal()` lets the actual connection task retain that terminal
  action until it dispatches the reason and closes on send completion. This
  notification is native asynchronous transport plumbing, not a new limiter.
- `mark_read_only()` and `is_read_only()` expose the auto-read transition. The
  writer owner marks it after dispatching the common reason, before awaiting
  send completion. `wait_read_only()` wakes the actual reader without a lost
  notification; partial-frame ownership must remain with that reader.
- `snapshot()` returns actual Java-width counts, averages, configured limit and
  read-only state. It is observability, not another completion indicator.

Direct registry ticking is required. Replaying several coalesced cumulative
tick notifications against one current packet bucket cannot reconstruct which
packets arrived before each sample boundary. The adapter therefore has no
`observe(elapsed_milliseconds)` or coalesced-tick sampling API.

The host owns packet decoding, listener acceptance, actual socket read gating,
phase-specific outbound codec availability, output ordering and completion
cleanup. A terminal notification alone does not pause socket I/O, send a packet
or close a connection. Exact Netty event-loop scheduling is not implied by the
native `Mutex`/watch/notification plumbing.

## Focused adapter validation

The adapter compiled with the recovered Rust toolchain against the actual
native library built from base `f40d8bf`. A private native smoke passed for
shared dispatch counters, retained terminal action for a late subscriber,
explicit send-dispatch-before-read-only ordering and waking the actual read-only
waiter. It also confirmed that read-only state does not erase a counter event
for an already accepted packet. The adapter source SHA-256 was
`e4e2c92b9625a1b03eda3badb453b0d86a50584497bc25ff5a72b21e3c3f79e3`;
the private smoke binary SHA-256 was
`4b87fb954492b7ffc77f3802fdf3008dc6c084ec920da709cb479f773882ad23`.
This check did not run production Main or an original Java runtime flow.
Production integration and its actual socket flow require separate evidence.

## Predispatch wire validation

`src/main_connection_decode.rs` supplies pure `validate_play` and
`validate_configuration` boundaries for the Main owner. The PLAY packet IDs
were checked against the original `GameProtocols` serverbound builder
(`agh`, `lambda$static$0`); CONFIGURATION uses its original eight registrations.
This is packet-envelope validation, not implementation of the decoded gameplay
or administration actions.

PLAY covers the packet shapes for `0x00` through `0x39`. Beacon `0x2e` uses the
actual original built-in mob-effect registry export described below. Other
packets use existing chat, command, cookie, custom-payload, container/Slot,
recipe and resource-pack decoders, or the original packet constructor/codec
field order. CONFIGURATION covers `0x00` through `0x07`. Existing typed decoder
limitations and declared native resource bounds remain applicable. This is not
execution support for all decoded messages or a full conformance claim.

Additional direct `javap` review covered the previously uncovered packet
constructors and record `STREAM_CODEC` static initializers, `ClientInformation`
(`aqh`), the interaction action variants, and
`FriendlyByteBuf.readBlockHitResult`.
Relevant distinctions preserved by the pure validator include:

- Container-button click uses two VarInts in 1.21. Book edit allows up to 200
  pages with 8192 UTF-16 units each and an optional 128-unit title.
- Packet enum ordinals are checked at decoding. Player-action direction instead
  consumes an unsigned byte that `Direction.from3DDataValue` wraps.
- Movement floats/doubles, negative interaction sequences, selected slots and
  packet entity IDs are consumed without inventing handler policy checks.
  A valid wire packet that a handler subsequently refuses is still accepted
  dispatch for the connection counter.
- Structure-block offset/size and integrity values are consumed before their
  original constructor clamping; jigsaw joint strings accept the original
  unknown-name fallback rather than a fabricated enum restriction.

Main must call the appropriate pure validator before incrementing accepted dispatch
and before entering policy handlers that may return, continue, disconnect or
await another owner. While waiting for the PLAY configuration acknowledgment,
all supported bodies are still wire-decoded, but only that terminal
acknowledgment passes the source listener's acceptance predicate. A pure decoder
error or an ignored packet must not increment the accepted counter.

The helper compiled against the actual recovered native library. No additional
repository tests or full packet conformance run were added in this substage;
production Main and socket evidence remain separately owned.

## Beacon built-in registry and LOGIN key envelope

`fixtures/connection-mob-effects-1.21.json` contains the actual ID/name entries
exported by running the unchanged pinned original `SharedConstants.tryDetectVersion`
(`ab.a`), `Bootstrap.bootStrap` (`akt.a`) and `BuiltInRegistries.MOB_EFFECT`
(`lt.d`), then iterating that registry with `Registry.getId/getKey` (`jz.a/b`).
Its provenance includes the artifact/mapping hashes and generator source hash.
The native validator loads the exported entries as an ID map; it does not infer
IDs from names, assume a contiguous range or manufacture a runtime effect owner.
This is the pinned vanilla built-in registry, not support for a custom registry
or beacon gameplay/effect application.

Direct source inspection establishes this wire path:

1. `ServerboundSetBeaconPacket.STREAM_CODEC` (`ahu.a`) composes two optional
   `MobEffect.STREAM_CODEC` values.
2. `MobEffect.STREAM_CODEC` (`brx.b`) calls
   `ByteBufCodecs.holderRegistry(Registries.MOB_EFFECT)` (`yv.b(lu.W)`).
3. `ByteBufCodecs$23` (`yv$16`) reads a direct signed VarInt and obtains the
   holder through the actual registry's holder ID map and `IdMap.byIdOrThrow`.
   There is no inline holder form and no ID-plus-one transformation here.
4. `ByteBufCodecs.optional` (`yv$10`) reads a boolean, decodes the holder only
   when present and wraps it with `Optional.of`; any nonzero boolean byte is true.

A focused execution of the original beacon codec encoded primary holder ID 0
(`minecraft:speed`) with no secondary as hex `010000`. Decoding accepted actual
ID 0 and final actual ID 38 (`minecraft:infested`) with no trailing bytes, and
rejected IDs -1 and 39 with the original `IllegalArgumentException`. This is
bounded original registry/codec evidence, not a production beacon interaction.

`validate_login_key(payload)` separately consumes the original LOGIN `0x01`
envelope before RSA policy runs. `ServerboundKeyPacket` (`aiz`, constructor
lines 26–29) calls ordinary `FriendlyByteBuf.readByteArray` twice. That method
uses the buffer's current readable-byte count as its bound before decoding the
length; negative lengths fail allocation and truncated arrays fail the read.
The native pure validator reads two nonnegative VarInt lengths and their actual
bytes, then requires packet exhaustion. It does not impose RSA modulus length,
padding, shared-secret length or challenge equality. Empty arrays and nonmodulus
ciphertexts are valid envelopes; their later cryptographic refusal must not be
mistaken for failure to decode an accepted packet.
