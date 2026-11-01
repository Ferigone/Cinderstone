# Connection-owned chat listener

Target: Minecraft Java **1.21**, protocol **767**, pinned by
`fixtures/source-lock.json`. This ledger records direct JDK21 `javap -c -p`
inspection of the original dedicated server. It is not an REA MCP execution or
a Java runtime observation. Raw disassembly remains outside the repository.

## Pinned methods

Official mappings name `ServerGamePacketListenerImpl` as `aru`:

| Original method | Mapped method | Native responsibility |
| --- | --- | --- |
| `handleChat`, lines1279–1302 | `a(agq)` | Apply last-seen update before text dispatch and decode the actual message |
| `handleChatCommand`, lines1306–1310 | `a(ago)` | Shared text gate, then unsigned command attempt and spam charge |
| `performUnsignedChatCommand`, lines1313–1320 | `b(String)` | Enforced signable arguments produce an error, rather than unsigned execution |
| `handleSignedChatCommand`, lines1324–1332 | `a(agp)` | Last-seen validation before the shared text gate |
| `collectSignedArguments`, lines1358–1386 | `a(agp,xr,xg)` | Exact parsed ranges, packet signature order and chain mutation |
| `tryHandleChat`, lines1411–1427 | `a(String,Runnable)` | Illegal characters, hidden chat, reset action time, enqueue runnable |
| `unpackAndApplyLastSeen`, lines1431–1438 | `a(xg$b)` | Validate against the recipient's actual tracked messages |
| `detectRateSpam`, lines1461–1465 | `r()` | Add20; reject above200 unless actual operator or singleplayer owner |
| `handleChatAck`, lines1470–1476 | `a(agn)` | Apply the actual offset or disconnect on failure |
| `addPendingMessage`, lines1538–1553 | `a(xp)` | Signature cache/ack ownership and pending-message bound |
| `handleChatSessionUpdate`, lines1869–1894 | `a(agr)` | Key-data equality, replacement expiry order and trusted certificate check |
| `resetPlayerChatState`, lines1917–1925 | `a(xq)` | Replace decoder, then publish player chat session through the chat chain |

`MinecraftServer.getProfileKeySignatureValidator` delegates directly to
`Services.profileKeySignatureValidator`; it does not test online mode.
`DedicatedServer.enforceSecureProfile` (`apn.aB`, lines456–457) is the conjunction
of the configured property, online mode and a nonempty PROFILE_KEY service-key
collection. A native caller must supply that current effective value, including
changes in the actual trusted-key owner.

## Callable native owner

`src/chat_listener.rs` implements one `ChatListener` per actual PLAY listener.
It owns the incoming 20-entry `LastSeenMessagesValidator`, the current remote
session and its decoder, and the listener's chat spam counter. It does not own
the recipient's outgoing `MessageSignatureCache`, a network writer, player
registry, text filter, command dispatcher or a substitute service trust root.

Its public operations are:

- `handle_chat`: source acknowledgment and text gates, followed by the current
  signed or unsigned decoder. The returned message still requires the actual
  filter/decorator and ordered broadcast owner.
- `handle_unsigned_command`: text gates and the actual parsed-signable/enforced
  profile rule. An accepted command is not an execution result.
- `handle_signed_command`: acknowledgment and text gates, then native signing
  context collection from exact host-supplied original argument ranges.
- `reject_signed_command_unsupported`: preserve acknowledgment/text effects
  while explicitly refusing an unimplemented native parser context. It never
  labels an unsupported command's signatures as valid.
- `handle_ack`, `add_pending`, `tick` and `advance_ticks`: real per-recipient
  acknowledgment ownership and actual connection-listener tick accounting.
- `handle_session_update`: current key-data identity, expiry replacement check,
  trusted service certificate validation and decoder replacement.

`ChatOutcome` and `ChatCommandOutcome` carry ordered `ChatEffect`s and an explicit
`reset_last_action` request. The command outcome also exposes
`charge_command_spam`; the host charges after the command attempt, including
command signature/parse/execution failure after the shared gate. Chat spam is
charged only through `after_chat_broadcast` after the actual asynchronous chat
pipeline has completed. A chat decode error returns before that charge.

## Source ordering preserved

Incoming chat and signed-command acknowledgments are applied **before** illegal
character or client chat-visibility checks. A malformed update disconnects with
`multiplayer.disconnect.chat_validation_failed`. The owner deliberately keeps
partial validator effects, including an offset applied before a later invalid
bitmap fails; it does not clone and silently roll back the source owner.

Illegal UTF-16 units are section sign, units below U+0020 and U+007F. Illegal
text disconnects with `multiplayer.disconnect.illegal_characters`. Hidden chat
(`2`) instead sends red `chat.disabled.options` without executing or resetting
action time. SYSTEM visibility (`1`) can still submit chat and commands; the
actual recipient broadcast policy separately determines what it receives.

There is no independent listener `lastChatTimeStamp`/`checkChatMessageState`
field or gate in the pinned1.21 artifact. Signed timestamp ordering belongs to
the actual `SignedMessageChain` decoder. This port does not invent a second
wall-clock rejection or a pause/online-mode gate absent from these methods.

Command signatures are collected in packet order. A supplied signature name
absent from the actual parsed signable arguments breaks the decoder before the
red `chat.disabled.invalid_command_signature` failure. If a required parsed
argument remains absent after the supplied signature loop, the same error is
reported without an additional chain-break mutation. Empty supplied signatures
cause each actual signable argument to pass through the decoder as an unsigned
body. A failed decoder produces its red `chat.disabled.*` message, not an
automatic transport disconnect.

The spam counter uses Java signed-int addition, adding20 after the source
operation; only a resulting value strictly greater than200 is rejected. Actual
operator membership and the actual singleplayer-owner predicate exempt the
disconnect. Positive spam counters decrease by one per listener tick; zero or
negative counters do not decrease. Coalesced native observations must supply a
lossless count of actual listener ticks. Current exemption status is supplied
after command execution, so self-deop cannot retain its earlier exemption.

Only a message with an actual signature enters `add_pending`. The bound checks
the real tracked list size after insertion and disconnects only above4096 with
`multiplayer.disconnect.too_many_pending_chats`. The host first sends/enqueues
the real recipient packet, pushes its real signature cache, and then invokes
this owner; an unsent or invented message is not a valid pending entry.

## Chat session replacement

Session equality in the listener compares `ProfilePublicKey.Data`, rather than
the remote session UUID. Identical key data with a new session UUID is ignored.
A replacement whose expiry precedes the current key's expiry disconnects with
`multiplayer.disconnect.expired_public_key`, before consulting the service-key
validator. A missing validator/empty trust key set logs and ignores the
update. If a validator exists, certificate validation uses the actual sender
UUID; an invalid certificate disconnects using its native validation error.

There is no additional server-side offline-mode bypass for session updates.
An expired first key can pass certificate creation and later fail the decoder's
own expiry gate. Key expiry is not fabricated as an earlier certificate check.
After successful validation the listener replaces its session and decoder; the
host must preserve the original ordered future-chain publication of the actual
player chat session and its player-info update.

## Integration boundaries

The implementation returns native effects and accepted data to the production
host. The host retains the real network-thread/server-thread queue boundaries,
filter/decorator futures, idle-action owner, raw operator predicates, recipient
visibility and signing context. Arbitrary Brigadier parsing and selector ranges
are not manufactured here. Unsupported parsing, custom chat decoration,
non-default filtering and scheduling contexts remain explicit where the host
does not yet provide those owners. This source ledger alone is not Main wiring,
runtime success, whole-Minecraft compatibility or a checklist completion claim.
