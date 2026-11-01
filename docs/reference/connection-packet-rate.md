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
