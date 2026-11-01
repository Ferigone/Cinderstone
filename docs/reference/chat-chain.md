# Signed chat bodies and chain ownership

Target: the pinned original Minecraft Java **1.21** dedicated server, protocol
**767**. The artifact identities are recorded in `fixtures/source-lock.json`;
inner server SHA-256:
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.

This ledger records direct JDK21 `javap -c -p` inspection using the official
mappings. It is not an REA MCP execution or an observation of Java gameplay.
Raw disassembly remains outside the repository.

| Original mapped owner | Obfuscated class | Native owner |
| --- | --- | --- |
| SignedMessageLink | `xu` | Sender/session UUIDs, index and descendant relation |
| SignedMessageBody | `xs` | Content, timestamp, salt and ordered last-seen signatures |
| PlayerChatMessage | `xp` | Link/signature/body, unsigned decoration and filter state |
| SignedMessageChain, Decoder | `xt`, `xt$1`, `xt$b` | Connection-owned next-link and previous signed timestamp |
| SignedMessageChain.DecodeException | `xt$a` | Exact chat-error translation identities |
| RemoteChatSession | `xq` | Client-supplied session ID plus a genuinely validated key |
| SignedMessageValidator.KeyBased | `xv$a` | Separate receive-side validator and permanently failed chain state |
| FilterMask | `xd` | Pass-through/full/partial type and Java UTF-16 mask positions |

## Signature updater bytes

These are ordinary fixed-width signature-updater bytes, **not VarInts or a
serialized packet**:

1. `PlayerChatMessage.updateSignature`: four-byte big-endian signed integer 1.
2. `SignedMessageLink.updateSignature`: sender UUID's 16 big-endian bytes,
   session UUID's 16 bytes, then the four-byte big-endian signed message index.
3. `SignedMessageBody.updateSignature`: eight-byte big-endian salt; eight-byte
   big-endian `Instant.getEpochSecond`; four-byte UTF-8 byte length; UTF-8 content.
4. `LastSeenMessages.updateSignature`: four-byte big-endian ordered list size,
   then every raw 256-byte signature in that order.

The wire timestamp is epoch milliseconds. The signature's seconds use floor
division for negative milliseconds, matching `Instant.getEpochSecond`, rather
than truncation toward zero. Hashing UTF-16 character count or milliseconds
would produce a different signature. Cryptographic verification uses the
validated actual profile key supplied by `chat_crypto`; no client key is
promoted into a services trust root here.

## Decoder state and source order

`SignedMessageDecoder` starts with link index 0 and previous timestamp
`Instant.EPOCH`. Its `unpack` order is:

1. Missing message signature: `chat.disabled.missingProfileKey`; keep state.
2. Profile key expired strictly before actual now:
   `chat.disabled.expiredProfileKey`; keep state.
3. Missing next link: `chat.disabled.chain_broken`.
4. Body timestamp earlier than the previous signed timestamp: clear the next
   link and return `chat.disabled.out_of_order_chat`.
5. Install the body timestamp, construct the actual signed message and verify
   SHA256withRSA. Failure clears the next link and returns
   `chat.disabled.invalid_signature`.
6. A valid message older than five minutes produces a warning indication but
   is still returned. Advance its link, or exhaust it after `i32::MAX`.

Timestamp equality is allowed. Missing/expired-key failures do not consume a
link. Signature/order failures break the connection's decoder permanently until
the actual listener installs a different validated session. These exceptions
are chat error components; they do not themselves request disconnection.

The unsigned decoder samples the actual enforce-secure-profile condition.
When enforced it returns the missing-profile-key error. Otherwise it discards
the input signature, timestamp, salt and last-seen body and constructs the
source `PlayerChatMessage.unsigned`: current server time, salt zero, empty
last-seen list and no signature. Its link is **index 0 with the nil session**.
It is not an invented negative-index signed message.

## Other message owners

`PlayerChatMessage` retains link, signed body, optional unsigned decoration and
filter state. Removing its signature creates a fresh unsigned link/body while
preserving decoration and filtering, as the source does. Server/client expiry
is strict current-time-after timestamp plus five/seven minutes respectively.

`RemoteChatSession.create_message_decoder` creates a new decoder from the real
sender, client session UUID and validated public key. Its receive-side
`KeyBasedMessageValidator` is separate from the exact-next-index server decoder:
after key/signature validation it accepts an identical repeated message or a
strictly descendant link. Invalid input permanently rejects subsequent input.

`SignedMessageEncoder` receives a caller-owned signer and retains no private
key. It advances before invoking that signer, matching consumption even when
the signing operation fails. Exhaustion returns no signature.

Partial filter masks retain BitSet word/bit order and ignore trailing zero
words for equality. `is_empty` means pass-through type, not an empty partial
BitSet. `apply_utf16` preserves Java char-array masking without silently
replacing an unpaired surrogate with a Rust replacement character.

## Public contract and boundaries

The callable APIs live in `src/chat_chain.rs`: `SignedMessageLink`,
`SignedMessageBody`, `PlayerChatMessage`, `FilterMask`, `SignedMessageDecoder`,
`MessageDecoder`, `DecodeError`, `DecodedMessage`, `RemoteChatSession`,
`KeyBasedMessageValidator`, `SignedMessageEncoder` and `signature_bytes`.
`MessageSignature` is the canonical `[u8; 256]` type from `chat_ack`.

Packet limits, acknowledgment updates, recipient caches, accepted dispatch and
network publication remain their actual owning modules' responsibilities.
The timestamp input is the wire/host millisecond representation; arbitrary
nanosecond-only Instant differences are not claimed. Rust content strings are
valid Unicode; complete Java malformed-string/Component equality and rich
filter-formatting behavior are not supplied by this module. Unsigned Component
comparison recognizes its generated bare literal representations only.

Main calls, source commits, compiler results and focused native runtime evidence
are recorded by the integration coordinator. This ledger alone does not prove
production integration, trusted public-client authentication or completion of a
fixed checklist item.
