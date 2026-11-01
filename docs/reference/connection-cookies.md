# Connection cookies: Minecraft Java 1.21

This ledger records static behavior from the pinned official server artifact.
It does not establish successful runtime execution or complete protocol parity.

## Pinned source and method

- Outer server JAR SHA-1: `450698d1863ab5180c25d7c804ef0fe6369dd1ba`.
- Inner server JAR SHA-256:
  `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
- Official server mappings SHA-1: `31c77994d96f05ba25a870ada70f47f315330437`.
- Methods were inspected with the existing JDK 21 `javap -p -c` against that
  actual inner JAR. Raw disassembly and proprietary artifacts remain private.
  This is separate from the earlier REA archive-inventory evidence; no REA
  runtime observation or JVM decompilation is claimed.

Relevant mapped classes are `ClientboundCookieRequestPacket` (`abp`),
`ServerboundCookieResponsePacket` (`abs`), `ClientboundStoreCookiePacket` (`zv`),
`ResourceLocation` (`akr`), `FriendlyByteBuf` (`vw`), and the bounded byte-array
codec `ByteBufCodecs$10` (`yv$2`). Protocol registration order comes from
`LoginProtocols` (`aiv`), `ConfigurationProtocols` (`abj`) and `GameProtocols`
(`agh`), independently corroborated by the installed protocol-767 schema.

## Actual packet registrations

IDs are zero-based entries in each protocol direction's registration sequence.
They are not one common packet ID shared by the three phases.

| Packet | Login | Configuration | Play |
| --- | --- | --- | --- |
| Clientbound cookie request | `0x05` | `0x00` | `0x16` |
| Serverbound cookie response | `0x04` | `0x01` | `0x11` |
| Clientbound store cookie | unavailable | `0x0a` | `0x6b` |

The request contains one `ResourceLocation` key. The response contains that key
followed by a nullable byte array: a Boolean presence byte and, only when
present, a VarInt length and exactly that many payload bytes. Absent and present
empty payloads are distinct. Netty Boolean decoding accepts every nonzero
presence byte as true.

Store-cookie contains a key followed directly by a required length-prefixed
byte array, without the response's nullable marker. The payload codec limits
both reads and writes to **5,120 bytes**. Zero bytes are permitted; negative,
oversized or truncated lengths fail decoding. Trailing bytes fail the enclosing
packet decode.

`FriendlyByteBuf.readResourceLocation` reads a UTF string limited to 32,767
Java UTF-16 code units and then calls `ResourceLocation.parse`. The first colon
separates namespace and path. No colon, or a leading colon, uses `minecraft` as
the namespace. Namespace characters are lowercase ASCII letters, digits, `_`,
`-` and `.`; path characters additionally permit `/`. Empty paths are permitted
by this parser. A second colon is invalid because it is not a path character.
Encoding writes the canonical `namespace:path` form with the independent wire
string length limit.

## Vanilla server listeners do not accept cookie replies

`ServerLoginPacketListenerImpl.handleCookieResponse` (`arw.a(abs)`, mapped
lines 267–268) always calls disconnect with
`ServerCommonPacketListenerImpl.DISCONNECT_UNEXPECTED_QUERY`.

`ServerCommonPacketListenerImpl.handleCookieResponse` (`arr.a(abs)`, mapped
lines 104–105) performs the same unconditional disconnect. The configuration
and play listeners inherit that behavior. The translation key is
`multiplayer.disconnect.unexpected_query_response`.

The network codec validates the response first. A successfully decoded reply,
whether absent, empty, nonempty, repeated or associated with a constructed
request key, still reaches that unconditional rejection. These vanilla
listeners contain no pending cookie-request map, expected-key match, timeout,
server cookie cache, or acknowledgement of store-cookie. Adding such behavior
would be a deliberate application extension, not this source port.

## Native owner and integration boundary

`connection_cookies.rs` provides typed packet codecs and outgoing request/store
constructors, together with a `CookieConnection` phase owner. Main must route a
packet using its actual acknowledged protocol phase. The first valid cookie
response requests the current protocol's real disconnect and makes this cookie
listener terminal. Phase changes are limited to login → configuration,
configuration → play, and play → configuration; changing a closed owner fails.
The protocol task/acknowledgement host owns the timing of those transitions.

Request construction does not allocate a fictional pending query. Store
construction encodes the actual client instruction without claiming that its
delivery, client-side storage, transfer or later retrieval has been observed.
No cookie value is written to world/player storage or retained by the native
vanilla server listener. The existing server does not automatically issue
cookie request/store packets; their constructors are available to an actual
outgoing host operation. Production routing and runtime results are recorded
separately when integrated and exercised.
