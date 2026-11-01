# Chat acknowledgements and recipient signature cache

Target: the original Minecraft Java **1.21** dedicated server, protocol **767**.
Artifact identities are pinned in `fixtures/source-lock.json`: server SHA-1
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`, official mappings SHA-1
`31c77994d96f05ba25a870ada70f47f315330437`, inner server SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.

The findings below come from direct JDK21 `javap -c -p` against that pinned
artifact and its official mappings. This is not an REA MCP execution, Java
runtime observation or proof of whole-server compatibility. Raw disassembly
remains outside the repository.

## Source owners and callable native state

| Official class / mapped class | Native owner and behavior |
| --- | --- |
| `MessageSignature` / `xl`, source lines25–27,49–54 | `MessageSignature = [u8;256]`; exact fixed width and byte equality |
| `LastSeenMessagesValidator` / `xi`, lines21–80 | `LastSeenMessagesValidator`; ordered pending/acknowledged entries and offset validation |
| `LastSeenTrackedEntry` / `xj`, lines3–5 | An internal copied signature plus its pending flag; acknowledgment only clears that flag |
| `LastSeenMessages.Update` / `xg$b`, lines59–67 | `LastSeenUpdate { offset:i32, acknowledged:[u8;3] }` |
| `MessageSignatureCache` / `xm`, lines28–79 | `MessageSignatureCache`; default128 entries, ordered lookup and source deque replacement |
| `MessageSignature.Packed` / `xl$a`, lines67–98 | `PackedMessageSignature::Full` or `Cached`; missing and invalid-reference outcomes stay distinct |
| `LastSeenMessages` / `xg`, lines25–32 | Ordered last-seen packing and signature contribution |

`src/chat_ack.rs` owns these primitives only. Its state is per actual receiving
connection/listener, not a shared global cache or a world/player save field.
The wire module owns packet framing and bounded collection codecs. The actual
listener owns acceptance, disconnects and the delivered-message lifecycle.

## Acknowledgement window

The standard server listener uses a 20-entry window. Construction initially
adds **20 empty slots**, so `tracked_messages_count()` starts at20. Only the
actual recipient's delivered signature may be passed to `add_pending`.

- `add_pending` suppresses a signature only when it is byte-equal to the last
  pending signature. It does not remove nonconsecutive duplicates. Offset
  application does not reset this remembered signature.
- `apply_offset` accepts signed offsets from0 through
  `tracked_messages_count - last_seen_count`, inclusive, and removes that
  leading range. Negative offsets or removal of the retained window fail.
- `apply_update` applies that offset **before** inspecting acknowledgement bits.
  The bitmap is Java `BitSet` little-endian order. Bits20–23 in the third byte
  are retained and rejected; they must not be masked away during wire decoding.
  `FriendlyByteBuf.readFixedBitSet` (`vw.e(int)`) reads the rounded-up byte array
  and calls `BitSet.valueOf(byte[])` without such masking.
- Each selected slot must contain an actual tracked signature. Selecting it
  clears its pending flag and appends that exact signature to the returned
  list, in ascending window-slot order.
- An unselected pending slot becomes empty. An already acknowledged surviving
  slot cannot be omitted by the next update; that update fails.

Failure is deliberately **not transactional**. An invalid high bit can follow
an already-applied valid offset; an invalid later slot can follow changes to
earlier slots. Java returns `Optional.empty` after those mutations. Native typed
errors preserve that failure boundary rather than rolling the owner back or
manufacturing an empty successful last-seen list. The owning server listener
must follow its source rejection/disconnect path.

`AckValidationError` distinguishes invalid offsets, out-of-range bits, selection
of an empty slot and omission of a previously acknowledged slot. These internal
diagnostics do not replace the listener's original wire disconnect component.
The listener's excessive-pending-message guard also remains with that actual
listener; no separate maximum or acknowledgement policy is invented here.

## Signature packing and cache replacement

`MessageSignatureCache::default()` creates128 empty slots. `index_of` performs
the source linear byte-equality scan and returns the first matching index or-1.
`pack` then chooses a cached reference or an actual full256-byte signature.
The packed wire convention is `id+1`:0 followed by256 bytes for a full signature,
otherwise the cached index plus1.

`lookup` and `unpack` preserve three separate outcomes:

- An actual full signature or occupied valid slot returns that signature.
- An empty valid cache slot is the source `Optional.empty` outcome.
- A negative/out-of-range cache reference is an explicit native error matching
  the source array-bounds exception boundary, rather than an empty or fabricated
  signature or an unchecked native process panic.

`pack_last_seen` retains list order. `unpack_last_seen` stops at the first empty
reference, without inventing omitted entries. Wire collection bounds remain
with the wire decoder; the source object constructor itself takes a list.

The cache advances through `push(last_seen, optional_message_signature)`:

1. Copy the body's ordered last-seen signatures into a deque, then append this
   message's signature only when it actually exists.
2. Capture the set of these incoming signature values once. Incoming duplicates
   remain in the deque; they are not deduplicated.
3. For each cache slot from0, remove the deque's **last** signature into the slot.
   If its displaced old signature is nonempty and absent from the captured
   incoming set, append that old signature to the deque's **front**.
4. Stop when the deque is empty or every cache slot has been processed. Remaining
   cache slots are unchanged; overflow deque entries are discarded.

This is not a generic LRU or an append-and-truncate shortcut. The original
package-level `push(List)` path is also exposed as `push_signatures`. There is no
invented expiry timer, invalidate-on-session-update operation or deletion-driven
cache reset. A new listener creates the source's new cache owner. Packing uses
the old cache; advancing it belongs to the corresponding actual outbound
message's source-ordered delivery path.

`last_seen_signature_bytes` implements the signature contribution independently
of the packed wire representation: a signed big-endian32-bit entry count,
followed by each ordered raw256-byte signature.

## Integration and validation boundary

This delivery supplies callable native acknowledgement/cache state and its
source evidence. Root owns module exports, concrete listener/Main wiring and
focused compiler/runtime validation. Formatting was checked with the existing
Rust toolchain. No independent build, broad test suite, runtime success,
checklist completion or performance improvement is claimed by this ledger.
