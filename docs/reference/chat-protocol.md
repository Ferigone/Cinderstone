# Chat packet protocol and connection ownership

Target: original Minecraft Java **1.21**, protocol **767**. The pinned source
identities remain in `fixtures/source-lock.json`; inner server SHA-256:
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.

## Primary source inspection

The original classes were inspected with existing JDK21 `javap -c -p` and the
official mappings. Raw JVM disassembly stays outside the repository. This is
direct JVM source inspection, not an REA MCP operation or observed Java runtime.

| Original class | Mapped class | Native contract |
| --- | --- | --- |
| ServerboundChatPacket | `agq` | 256-UTF-16 message, epoch-millisecond Instant, salt, nullable fixed signature, last-seen update |
| ServerboundChatAckPacket | `agn` | Signed VarInt offset, retained for listener validation |
| ServerboundChatSessionUpdatePacket | `agr` | Remote session UUID and profile key data |
| RemoteChatSession.Data / ProfilePublicKey.Data | `xq$a` / `cna$a` | Session UUID, expiry, parsed RSA public key and service-certificate signature |
| ClientboundPlayerChatPacket | `adv` | Actual sender/index, nullable full signature, packed body, optional unsigned component, filter and bound chat type |
| ClientboundDisguisedChatPacket | `acx` | Component and bound chat type |
| ClientboundDeleteChatPacket | `acw` | Recipient-cache-packed signature |
| SignedMessageBody.Packed | `xs$a` | 256-UTF-16 content, millisecond Instant, salt and packed last-seen list |
| LastSeenMessages.Update / Packed | `xg$b` / `xg$a` | Three-byte fixed bit set; ordered packed signatures with decode bound 20 |
| MessageSignature.Packed | `xl$a` | Zero plus full 256-byte signature, otherwise cache index plus one |
| FilterMask | `xd` | Pass-through0, fully-filtered1, partial2 with a long-array BitSet |
| ChatType.Bound / ByteBufCodecs holder | `wv$a` / `yv$17` | Configured holder ID plus one, name component and optional target name |

The packet IDs were checked by counting the actual packet-type registrations in
`GameProtocols` (`agh`) for its clientbound/serverbound templates, including
the common packet types and bundle registration. The protocol-767 table was also
cross-checked against the installed `minecraft-data` 1.21 schema. The latter is
corroboration, not the behavior authority.
Clientbound IDs are `0x39` player chat, `0x1e` disguised chat, `0x1c` delete chat,
and `0x6c` system chat. Serverbound IDs are `0x06` message, `0x03` acknowledgement,
and `0x07` session update. Signed-command `0x05` decoding remains in the existing
`command_protocol` module; its envelope does not itself authorize execution.

## Callable wire API

`src/chat_wire.rs` exports complete-payload decoders:

- `decode_chat_message` returns `IncomingChatMessage` with the actual message,
  timestamp, salt, optional 256-byte signature and `LastSeenUpdate`.
- `decode_chat_ack` returns the signed offset without substituting a wire-layer
  policy for `LastSeenMessagesValidator`.
- `decode_chat_session_update` returns the existing `entity::ChatSession` wire
  shape. The key and certificate fields have source byte bounds of 512 and
  4,096 respectively. `ProfilePublicKeyData::new` performs RSA SPKI parsing and
  canonical encoding; trusted service validation and expiry remain separate.

Trailing payload bytes and truncated fields fail explicitly. Three-byte
acknowledgements retain all bits: the source decoder does not mask bits20–23.
The actual acknowledgement validator must see and reject those invalid bits at
its source-ordered validation boundary.

Outbound builders return the normal native `protocol::Packet`:
`encode_player_chat`, `encode_disguised_chat`, `encode_system_chat` and
`encode_delete_chat`. Generated components use the existing restricted native
component encoder; arbitrary component serialization is not claimed complete.
Partial filter masks serialize Java BitSet-style long words, trimming trailing
zero storage words, without changing their bit positions.

## Real registry and recipient cache requirements

`BoundChatType.registry_id` is the actual zero-based reference ID from this
connection's configured `minecraft:chat_type` registry. Its wire representation
is **ID + 1**, because zero selects an inline chat-type definition. The builder
does not invent a default registry ID or pretend that an unregistered holder
belongs to the client's registry. Inline decoration codecs are outside this
bounded API.

Each real recipient has its own `MessageSignatureCache`. `encode_player_chat`
packs the message's actual last-seen signatures against that recipient's
existing cache; `encode_delete_chat` uses that same owner. The top-level player
message signature is nullable but always full when present; cache packing
applies to last-seen references and deletion signatures.

Encoding does **not** advance a cache or add pending acknowledgements. The
actual listener/transport owner applies the delivery's cache update and pending
signature at the source-defined send phase. A temporary empty cache, a cache
shared across recipients, or an update for a suppressed/failed delivery is not
a substitute for that ownership. Missing or malformed cache references remain
observable failures in the cache owner.

## Boundaries and evidence

The wire module does not accept a profile certificate, validate a message chain,
approve timestamps or acknowledgements, implement recipient chat visibility,
filter text, or choose chat-type registry identity. Those are supplied by the
native listener, chain, crypto, acknowledgement and Main owners.

The existing primitive reader/writer contracts and Rust string representation
remain in force; this stage does not claim every malformed UTF-8/UTF-16 input or
the general Component/inline-holder codec. No compiler result, native runtime
flow, graphical-client session, public-network delivery, complete signed-chat
compatibility or checklist closure follows from this source ledger alone.
The integration coordinator records actual production calls and focused
compiler/runtime evidence separately.
