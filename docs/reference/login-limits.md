# Login timeout and connection packet-rate ownership

Target: original Minecraft Java 1.21, protocol 767. Artifact identities are pinned
in `fixtures/source-lock.json`: server SHA-1
`450698d1863ab5180c25d7c804ef0fe6369dd1ba` and official mappings SHA-1
`31c77994d96f05ba25a870ada70f47f315330437`.

This stage used direct JDK 21 `javap -p -c` against the pinned inner server JAR
and the official mappings. It does not claim an REA MCP execution or Java runtime
observation. Raw disassembly remains outside the repository.

## Source contracts

| Mapped source | Observed contract |
| --- | --- |
| `ServerLoginPacketListenerImpl.tick`, source lines 77–90 (`arw.d`) | Performs VERIFYING work, then WAITING_FOR_DUPE_DISCONNECT work; finally compares the old signed `tick` value with 600 while incrementing it. The disconnect is on listener invocation 601. |
| `ServerLoginPacketListenerImpl.disconnect`, lines 99–105 | Sends `ClientboundLoginDisconnectPacket` and closes the actual connection with the same component. Slow login uses `multiplayer.disconnect.slow_login`. |
| `ServerLoginPacketListenerImpl.handleLoginAcknowledgement`, lines 251–258 | Replaces the LOGIN listener with the configuration listener, starts its configuration work and changes LOGIN state to ACCEPTED. Successful Login Start, key exchange, authentication or Login Success alone is not that boundary. |
| `ServerConnectionListener$1.initChannel`, lines 91–109 (`art$1`) | Constructs `RateKickingConnection` only for `rate-limit > 0`; otherwise constructs ordinary `Connection`. |
| `DedicatedServerProperties` (`apo`) | `rate-limit` defaults to 0. There is no added per-IP login throttle in this path. |
| `Connection.channelRead0`, lines 184–204 (`vt.a`) | Requires an initialized listener, checks its packet acceptance predicate, dispatches the packet and then increments the received counter. Its normal main-thread-rescheduling exception still reaches the increment. Rejected listener packets do not count. |
| `Connection.sendPacket`, lines 341–347 (`vt.b`) | Increments the sent counter before dispatching the actual write to the event loop. Merely queued preconnection sends have not reached this boundary. |
| `Connection.tick`, lines 398–418 (`vt.b`) | Runs the actual tickable listener before disconnect handling/flush and then tests `tickCount++ % 20 == 0`. Sampling occurs on connection tick 1, then 21, 41, and so on, retaining signed Java int wrap behavior. |
| `Connection.tickSecond`, lines 421–425 (`vt.c`) | Computes both averages as `Mth.lerp(0.75f, currentCount, previousAverage)` using float arithmetic; resets both current counters. |
| `Mth.lerp(float,float,float)`, line 631 (`ayo.i`) | Executes `start + delta * (end - start)` as distinct float subtraction, multiplication and addition. Thus the current packet count has weight 0.25 and the previous average weight 0.75. |
| `RateKickingConnection.tickSecond`, lines 22–31 (`wj.c`) | Calls the base sample first, then disconnects only when the smoothed received float is strictly greater than the configured integer limit converted to float. |
| `RateKickingConnection` send callback, line 28 | Sends the common `ClientboundDisconnectPacket` with `disconnect.exceeded_packet_rate`, makes the channel read-only, and closes it through the send callback. This is not an immediate LOGIN-specific packet substitution. |

## Native code and integration boundary

`src/login_limits.rs` contains callable state with no separate timer thread:

- `LoginTimeout::tick()` follows the source postincrement exactly.
  `advance_ticks(count)` preserves a timeout boundary when the host has a
  lossless cumulative count of actual connection ticks.
- `ObservedLoginTimeout::new(server_tick)` and `observe(server_tick)` adapt a
  wrapping cumulative native tick counter. Repeated observations do not reset
  or advance the timer. This adapter is native transport plumbing, not another
  vanilla state or a wall-clock interpretation of 600 ticks.
- `ConnectionPacketRate` owns per-connection counters across listener changes,
  optional positive-rate enforcement, float averages and source sample cadence.
  Accepted receive/send dispatch events and the actual connection tick must be
  supplied by the host. Returned actions carry the original translation key.

The root coordinator owns Main, tick publication, transport and module exports.
This module alone does not prove production integration. The existing Main watch
payload records elapsed milliseconds and watch notifications may coalesce; using
its notifications as individual ticks or converting milliseconds into guessed
20-TPS ticks would change the source timeout. A cumulative actual connection-tick
counter is required for the native observation adapter.

For LOGIN, install one timeout when the real LOGIN listener begins, retain it
through key/authentication/duplicate waits and stop only after the real
acknowledgement transition or closure. Slow-login checking belongs after that
tick's source verification/duplicate work. A host that coalesces callbacks must
apply an observed timeout before accepting a later protocol transition.

For packet-rate enforcement, counting all framed packets in a raw reader is
insufficient: listener acceptance, decoded packet ownership, send dispatch and
connection tick ordering must be real. The default property remains disabled.
This stage does not connect or claim a fabricated rate kick, read-only channel
state, per-IP throttle, concurrent-login cap, duplicate-player replacement or a
complete `authentication.5` delivery. Compiler/runtime validation and exact Main
callsites are recorded separately by the integration coordinator.
