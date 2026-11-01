# Native production chat ownership and integration

Target: the pinned original Minecraft Java **1.21**, protocol **767**. This
ledger records the actual production paths in the native dedicated server,
following core commit `66ebc57`, Main integration `1f56b98`, source-order fixes
`cea8569394faf60a8f6dbbbe15f2b84e4a0c4522` and registration rollback `396b589`.
The full Main build passed following that rollback. Exact final source/binary
identities and observation times belong to the coordinator's validation records.

The integration coordinator records the [focused protocol flow](../validation/chat-flow.json)
and [separate cryptographic fixture](../validation/chat-crypto-smoke.json).
The final protocol flow passed at `2026-11-01T16:53:46.366Z` for source
`b3a0d1d`. The crypto fixture uses generated local services/profile keys. Compilation, generated signatures and that local
fixture are not evidence of a real Mojang-authenticated certificate, a
graphical-client session or public-network gameplay.

## Actual players and connection-owned state

`LivePlayers.chat` owns `MainChatRuntime`. A native player is registered with
its actual entity ID, selected profile UUID/name and received client settings.
The profile is the existing session-service-verified or selected offline profile;
the client-claimed LOGIN UUID is not substituted. Each runtime entry owns:

- One `ChatListener`: its real incoming last-seen validator, current remote
  session/message decoder and chat spam counter.
- One outgoing `MessageSignatureCache` for that receiving PLAY connection.
- The actual client chat visibility and text-filtering flag.

The runtime retains actual registration order independently of asynchronously
allocated entity IDs. Its audience therefore follows the admitted order rather
than sorting presumed player creation times. Removal releases that entry. A
new PLAY listener after reconfiguration starts with a fresh acknowledgment
window, cache and decoder; those ephemeral listener owners are not persisted in
the world/player checkpoint. A chat-session update does not clear the existing
recipient cache or acknowledgment history.

If later native runtime-player registration fails, the rollback removes the
new chat entry and last-action owner along with the registry entry, avoiding an
orphaned UUID that could obstruct a later actual admission.

`ClientInformation` now retains the received chat-visibility enum and
text-filtering boolean. Initial configuration passes those actual values to
registration, and PLAY client-information updates replace the runtime settings.
Only FULL visibility receives player chat. SYSTEM may submit messages/commands
while receiving system responses; HIDDEN instead produces the source red
`chat.disabled.options` response at the text gate. The runtime does not invent
a chat-visibility value from operator permissions.

## Dispatch and source-ordered publication

`Server.handle_chat_packet` routes the actual PLAY packets:

| Serverbound packet | Production path |
| --- | --- |
| `0x03` Chat Ack | Decode the signed offset and apply the actual recipient validator |
| `0x04` Chat Command | Shared text gate, current effective enforcement/signable-argument rule, then the real administration host |
| `0x05` Signed Chat Command | Apply last-seen update, text gate and actual decoder/signable ranges before any authorized execution |
| `0x06` Chat Message | Apply last-seen update, text gate and decoder, then the supported native broadcast |
| `0x07` Chat Session Update | Wait for the first services-key attempt, parse the untrusted key and validate its certificate before publication |

Dispatch acquires `chat_dispatch`, then the existing `player_admission` owner.
Console administration takes the same sequence. The whole command attempt,
including actual administration mutation/effects and the later spam check,
remains serialized; a concurrent op/deop or admission change cannot be inserted
between a remembered exemption and the corresponding source callback. The
administration host executes outside the player write lock while the sequence
and admission guards remain owned. The post-command context is read again, so
a successful self-deop does not keep its previous spam exemption.

Chat packets, accepted session events, administration feedback, permission/tree
refreshes and their disconnects all use the existing ordered `entity_events`
stream. They are not split between unrelated channels that could let a terminal
spam disconnect overtake the accepted message, or permission/ban effects overtake
earlier feedback. Recipient IDs come from the actual admitted owners.

`MainChatOutcome.effects` is an ordered prefix. Main publishes that prefix before
reporting a later native encoding error. A decoded or authorized command is not
reported as an execution result merely because its envelope was valid.

## Registry identity, session replication and delivery state

`main_chat_keys::captured_chat_type` reads the registry bytes actually sent in
CONFIGURATION. It locates the serialized `minecraft:chat` entry in
`minecraft:chat_type` and retains its real holder index. Missing registry data
fails rather than installing a guessed chat type. The wire builder uses that
configured holder and the actual sender's supported default display component.

An accepted session updates `EntityRegistry` through `set_chat_session` and
publishes its revision-bearing `EntityEvent::ChatSession`. The packet and player
tracker paths emit PlayerInfo `INITIALIZE_CHAT` (`0x02` action) and update the
tracked session snapshot, including the self tab-list path. The following chat
message uses the same ordered publication stream, preserving the session-before-
message boundary. An unvalidated raw session is not copied into that registry.

For each actual recipient, `MainChatRuntime` packs the message against that
connection's old cache, appends its packet to the ordered send prefix, then
advances the cache and pending validator when a signature exists. Unsigned
messages do not become pending signed messages. The source limit above 4,096
tracked entries produces the actual recipient disconnect after its preceding
packet. A globally shared cache, guessed acknowledgments or rollback of an
already-applied invalid offset is not substituted for those owners.

The current dedicated profile uses the actual no-op text-filter behavior and
default no-team decoration. The sender is never filtered against itself;
other recipients use the source sender-or-recipient filtering predicate. Filter
masks and the fully-filtered response are represented by native core code, but
that does not imply an external filtering service or team decorator has run.

## Trusted services keys and effective enforcement

`main_chat_keys` owns the immutable services-key snapshots in a watch channel.
Its native task fetches the official HTTPS public-key response, records each
attempt, publishes successful snapshots and retains the previous good snapshot
when a later fetch fails. `ServiceKeyRefresh` supplies the source retry/refresh
cadence. This task belongs to the running server lifetime; it is not an agent
coding task or cross-chat wakeup mechanism.

First-attempt readiness is separate from successful availability. Session
updates always wait for that first attempt, including in offline mode. The
status/pre-PLAY enforcement paths wait only when the configured property and
online mode require evaluating services-key availability. They do not impose
that wait on an ordinary offline status connection.

The current dedicated predicate is:

```text
configured enforce-secure-profile
    AND online-mode
    AND current PROFILE_KEY services collection is nonempty
```

The configured property defaults to true. Main exposes
`--enforce-secure-profile true|false`, and the entrypoint maps
`MC_ENFORCE_SECURE_PROFILE` with the same default. This does not change the
existing explicit `MC_ONLINE_MODE` selection. The predicate is evaluated from
current keys rather than frozen at connection construction. If the source
validator is absent, a session update follows the warning/ignore branch; the
server does not accept a client-supplied root or install an always-valid verifier.

The crypto core separately validates the services certificate against the actual
profile UUID and its certified RSA key, then verifies chat with that player's
key. A synthetic local services root may exercise those algorithms in a private
fixture, but is never described as Mojang trust or installed in the production
key owner by this integration.

## Connection phases and remaining boundaries

The actual server connection phase ticks the admitted listeners' spam owners.
It does not infer a number of ticks from elapsed wall time or watch wakeups.
The real `player_last_actions` map is created with the player, reset when the
listener requests `resetLastActionTime`, and removed with the player. It uses
the server's monotonic epoch. This does not claim complete activity coverage or
idle-administration behavior beyond the connected calls.

Signed administration reasons are accepted only where the bounded native parser
proves the original raw `reason` range for its supported plain grammar.
Unsupported selectors, spacing/range contexts and parser behavior are explicit
limits, not fabricated successful signature verification. Acknowledgment and
text effects still occur before an unsupported signed context is reported.
Full Brigadier/context/selector handling and propagation through arbitrary
command families remain unfinished.

Other open boundaries include custom filtering/decorators and scoreboard teams,
general Component equality/codecs, full Java Instant precision, complete
asynchronous filter/chat-chain and lifecycle ordering, and unported command
families. The source JSON component and wire owners support their documented
generated shapes; they are not relabeled as the complete component delivery.

These production paths do **not** close `authentication.4` or otherwise change
the fixed 100-item checklist. `scope_version: 1`, its denominator and the
completion definitions remain owned by the integration coordinator.

Supporting pinned-source ledgers: [crypto](chat-crypto.md),
[chain/body](chat-chain.md), [acknowledgments/cache](chat-acknowledgments.md),
[wire protocol](chat-protocol.md) and [listener](chat-listener.md). They identify
direct JDK `javap` inspection accurately; none of these notes invents an REA MCP
execution or turns static source inspection into a runtime observation.

## Recorded final source and native observations

Final source: `b3a0d1dc0ed01de666c7bf56723ebfe30140e0be`; full Main build passed.
The [local protocol flow](../validation/chat-flow.json) passed at
`2026-11-01T16:53:46.366Z` on binary SHA-256
`509a4dfb7d95ce7d05403b169b5f4922f0d3482713ab6f961a003be9e3c662f0`.
The crypto report retains its actual `cea8569` library source and generated trust
fixture; it is not a Mojang-issued certificate or production Main test.

The final source retains the framing reader after terminal PLAY exit, half-closes
output and drains late input without gameplay dispatch until EOF or an explicit
one-second native bound. This transport adaptation prevents immediate unread-input
socket drops from losing an already-written reason. It is not a new vanilla game
rule or a proof of every Netty close/callback race. Reader ownership remains
unchanged across actual PLAY/CONFIGURATION reconfiguration.
