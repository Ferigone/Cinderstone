# Common custom payload and pong decoding

This stage shares the native CONFIGURATION/PLAY decoders for serverbound custom
payloads and pong replies. It does not add plugin channels or retain arbitrary
client payload bodies.

## Original source

The inputs are the pinned Minecraft Java **1.21**, protocol **767**, artifacts
from [`source-lock.json`](../../fixtures/source-lock.json): outer server JAR
SHA-1 `450698d1863ab5180c25d7c804ef0fe6369dd1ba`, inner JAR SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`,
and official mappings SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`.

The evidence is fresh direct JDK 21 `javap -p -c` inspection. No REA MCP was
available for this stage; raw binaries and disassembly stay outside the source
repository.

| Mapped class/method | Original symbol | Observed behavior |
| --- | --- | --- |
| `ServerboundCustomPayloadPacket` static codec | `aab` | Registers `BrandPayload`; every other identifier uses the fallback with a 32,767-byte bound. |
| `CustomPacketPayload` codec lookup/decode | `aaj$1.a` | Reads `ResourceLocation`, then selects the registered or fallback body codec. |
| `BrandPayload` buffer constructor | `aah(vw)` | Reads the ordinary bounded UTF string. |
| `DiscardedPayload` fallback decoder | `aak.a(int, akr, vw)` | Bounds all remaining body bytes, skips them and constructs a payload holding only its identifier. |
| `ServerboundPongPacket` buffer constructor | `aad(vw)` | Reads one signed fixed-width 32-bit integer. |
| `ServerCommonPacketListenerImpl.handlePong` / `handleCustomPayload` | `arr.a(aad)` / `arr.a(aab)` | Both handlers immediately return. |
| `PacketDecoder.decode` | `wd.decode` | Rejects unread trailing bytes after the registered packet codec returns. |

The protocol registration order in `ConfigurationProtocols` (`abj`) and
`GameProtocols` (`agh`) gives custom-payload IDs `0x02` / `0x12` and pong IDs
`0x05` / `0x27`, respectively.

## Native contract

[`connection_payload.rs`](../../src/connection_payload.rs) exports
`decode_custom_payload`, `CustomPayload`, `decode_pong` and
`MAX_SERVERBOUND_CUSTOM_PAYLOAD_BYTES`. Both live protocol phases and the
old-PLAY decode-before-ignore boundary can call these same functions.

`minecraft:brand` decodes its UTF string and rejects trailing bytes. Its bound
is the UTF codec's 32,767 Java UTF-16 units and maximum encoded-byte limit;
the fallback body's 32,767-byte bound does **not** apply to the registered brand.
Identifier parsing reuses the existing `CookieKey` implementation of
`ResourceLocation.parse`, including the default namespace. Unknown bodies are
validated and discarded without allocating another body or installing a plugin
handler. A decoded pong can be discarded: the original common listener does not
match it to a request or update keepalive latency.

This stage uses the shared `Reader::string` codec. Its malformed UTF-8 behavior
is the responsibility of that primitive; the original `Utf8String` (`wp`)
delegates to Netty's UTF-8 `ByteBuf.toString` and applies Java `String.length`
afterward. The previous strict Rust UTF-8 rejection is an identified shared
codec gap, not a property of `BrandPayload`.

A bounded JDK 21 probe invoked the **unchanged original** `aab.a` and `aad.a`
codecs with the original bundled Netty libraries. It observed a brand and a
default-namespace `brand` decode as `BrandPayload`; an unknown 32,767-byte body
was consumed and discarded, while 32,768 bytes were rejected. A registered
30,000-character brand occupying 90,000 UTF-8 bytes was accepted, confirming
that the fallback bound is not a global payload cap. An invalid identifier was
rejected and pong retained `-2147483648`. All accepted bodies had no unread
bytes. A separate probe classloader avoided signed-JAR/default-package
collisions; no original class or JAR was modified. This observes original
codec behavior, not a native network session.

## Existing keepalive owner and integration findings

`session::KeepAlive` already owns the source common-listener timers, challenge,
pending flag, closed-listener timeout and latency arithmetic. Reconfiguration
retains the old owner until acknowledgement and constructs the next listener
with inherited latency and a fresh timer. This stage does not create a second
keepalive owner.

Fresh `arr.a(aac)` inspection confirms that a wrong or unsolicited well-formed
keepalive response on a dedicated server invokes the timeout disconnect; the
singleplayer-owner exception does not apply to this host. The existing PLAY
adapter emits that reason. The CONFIGURATION adapter previously propagated the
same native error without sending its CONFIGURATION disconnect packet. The
integration coordinator owns that narrow production correction and the common
decoder callsites. Malformed wire bodies remain decoder failures and must not
be relabelled as valid-but-unexpected keepalive responses.

This source ledger does not by itself establish a successful native build,
production callsite, runtime session or aggregate checklist completion. The
integration-stage evidence records those separately.
