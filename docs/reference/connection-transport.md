# Native TCP read-inactivity ownership

Target: original Minecraft Java **1.21**, protocol **767**. This stage implements
the raw ingress and read-inactivity contract in `src/connection_transport.rs`.
The integration coordinator owns the actual Main adapter, production evidence
and checklist decisions. A callable stream and watchdog alone are not evidence
that production invokes them.

## Pinned evidence

The original artifact identity is retained in `fixtures/source-lock.json`:
outer server SHA-1 `450698d1863ab5180c25d7c804ef0fe6369dd1ba`, official mappings
SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`, inner server SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
The bundled `io.netty:netty-handler:4.1.97.Final` jar has SHA-256
`bd4771d19149cbc9c7464ed2d58035d79cdfed7adb59d38718b0d8e7a3dcc2de`.

This evidence comes from direct JDK 21 `javap -p -c` inspection of those exact
artifacts and the official mappings. REA instructions were applied, but no REA
MCP operation was available or executed. Raw disassembly and Java artifacts stay
outside the public repository. Static inspection does not establish a Java
runtime observation.

| Original source method | Observed contract |
| --- | --- |
| `ServerConnectionListener$1.initChannel`, original lines91–109 (`art$1`) | Adds `ReadTimeoutHandler(30)` as `timeout`, before legacy-query handling and packet serialization; the duration is thirty seconds. |
| Netty `IdleStateHandler.channelRead` / `channelReadComplete` | Raw inbound events mark a read in progress; completion updates monotonic `lastReadTime`. No complete Minecraft frame is required. |
| Netty `IdleStateHandler$ReaderIdleTimeoutTask.run` | Compares monotonic nanoseconds against reader-idle duration, rescheduling for the remaining duration; an in-progress event-loop read does not expire. |
| Netty `ReadTimeoutHandler.readTimedOut` | Fires `ReadTimeoutException.INSTANCE`, then closes the actual channel; its closed flag prevents repeating the callback. |
| `Connection.exceptionCaught`, original lines140–180 (`vt`) | A `TimeoutException` directly calls `disconnect` with `disconnect.timeout`; it does not take the ordinary internal-exception packet-send branch. |
| `Connection.disconnect`, original lines442–454 (`vt.a`) | Closes the actual channel and retains disconnection details. It does not construct a LOGIN, CONFIGURATION or PLAY packet. |

The ordinary non-timeout exception branch is distinct: it requests listener
disconnection details, selects the source LOGIN/common disconnect packet when
applicable, sends that packet with a completion close, and sets the channel
read-only. A double fault closes directly. This stage does not claim a complete
port of those ordinary exception and listener-crash-detail paths.

## Callable contract and actual ownership

`spawn_raw_read_transport(socket_reader)` returns:

- `RawReadPumpGuard`: the actual connection owner retains this for its whole
  lifetime. Dropping it aborts the task instead of detaching a live socket reader.
  Its `abort_handle()` lets the terminal connection owner stop raw ingress when
  the source sets the channel read-only, while a dispatched disconnect packet
  still retains its actual writer until send completion. Queued bytes do not
  become new gameplay dispatches after that terminal transition.
- `RawReadStream`: an `AsyncRead + Unpin` stream of the exact received bytes.
  The existing `CryptoRead` wraps this stream. The pump does not decrypt,
  decompress or frame bytes, so prefetched bytes retain their original identity
  across the encryption switch and later protocol transitions.
- `ReadTimeoutWatch`: one connection-wide raw-read clock. Its `wait()` future
  must run concurrently with the complete session future, including HTTP
  authentication, initial chunk work and all CONFIGURATION/PLAY transitions.

A successful nonempty raw socket read updates monotonic activity before bytes
enter the downstream queue. Writes and decoded-packet dispatch do not update
that activity. A timer wake rechecks the latest timestamp; coalesced watch
notifications cannot restore an obsolete deadline. EOF ends the timer owner.
An original I/O error ends it before the error waits behind earlier bytes in the
ordered stream. A completed timeout returns `ReadTimeout`, whose reason is
`disconnect.timeout`; Main must close its real halves and actual actor owners
without inventing a wire disconnect packet for this transport exception.

The existing framing reader must remain alive through terminal output draining.
Cancelling and replacing it on listener changes can discard a partial encrypted
frame, independently of read-idle timing. The raw pump likewise survives listener
changes and stops only when its actual connection lifetime ends.

## Native boundaries and pending proof

The bounded queue permits eight queued chunks of up to8,192bytes, plus one
consumer chunk and one producer chunk: at most81,920bytes of payload capacity.
It applies backpressure and never drops or fabricates bytes. This is an explicit
native resource boundary, not a vanilla packet limit. At saturation, a waiting
producer cannot observe further bytes still in the kernel; the port therefore
does not claim Netty event-loop/read-completion or backpressure equivalence under
saturation. Tokio raw-read completions are the native observation boundary;
Netty batches `channelRead` events before `channelReadComplete`.

The former Main packet-completion deadlines (15seconds during negotiation,
5seconds for a status ping and60seconds per later frame) do not implement this
source contract. Their removal and actual outer-lifetime wiring belong to the
coordinator. The separate native initial-FULL-chunk delivery bound is not a
transport read-idle deadline and must remain separately identified.

The production module compiled with Rust1.99.1 in a private standalone harness.
That focused native smoke passed exact byte preservation across bounded ingress,
ordered original I/O error and EOF handling, and raw-fragment inactivity with
the consumer deliberately unpolled. An incomplete VarInt byte was followed by
another incomplete byte16seconds later. The old30second activation deadline
did not fire; inactivity expired30seconds after the second raw read, at46,003ms
total. It used the actual production30second constant and monotonic timer.
The checked module SHA-256 was
`5ad74430d3f0849681a89707121c2fd37f3ca123883ab586fa98ce92470b3430`.

This is a module/stream observation, not a Main protocol-session observation.
The coordinator records Main compiler and runtime evidence after integration.
No broad conformance, graphical-client, public-network or complete vanilla
compatibility result is asserted here.
