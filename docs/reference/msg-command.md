# Literal online-name whispers (#209)

Non-op players and console execute `msg <targets> <message>` with native
Brigadier and original tell/w redirects. This delivery accepts one literal
currently online name, including quoted names, native case-insensitive lookup
and self-send. The target argument advertises the original players-only entity
parser. Actual roster names supply live completion. Missing targets fail with
canonical domain feedback. Selectors and UUID targets fail explicitly; offline
lookup is unavailable. Literal message text retains spacing, quotes and Unicode
up to256 UTF-16 units using the existing MessageArgument.

Main resolves the actual retained roster and current scoreboard-decorated source
and target display names. It uses configured msg incoming/outgoing types and
the existing per-recipient packet encoder, visibility/filter/signature/pending
owners. Source outgoing precedes target incoming; self-send emits both. Only
those recipients receive packets. Console receives the original decorated
outgoing component. No additional success/admin echo or public chat log is
emitted. AuthorizedCommand retains the signed `message` substring; conservative
secure unsigned rejection and command-spam handling remain in force. No new
signing/trust mechanism is introduced.

`tools/msg_oracle.java` compiles with `tools/admin_feedback_oracle.java` and
`tools/say_oracle.java` using the [external pinned classpath](admin-feedback.md).
Run `cinderstone.oracle.msg_oracle ORIGINAL_JAR fixtures/msg-vectors.json`.
It executes original MsgCommand/MessageArgument, literal EntityArgument roster
lookup, real ServerPlayer/OutgoingChatMessage visibility and display names,
and original packet codecs with recording connections. External server/registry
setup is stubbed; original ChatType.bootstrap supplies separate holder ordering.
This is not a full Java server flow. One tree, eight suggestion cases and eleven
executions cover aliases/ranges/results, missing/syntax failures, private packets,
self-send and source/target CommandsOnly policy. Console feedback bytes also
have a focused differential. Proprietary artifacts remain outside the checkout.

After `npm ci` and building, run `node tools/msg_flow.mjs
target/debug/mc121-rust REPORT.json`. The temporary native flow uses loopback25707
and three non-op clients. Complete private packets compare original bodies/names
with each configured holder ID; unknown-target error bytes match directly.
It checks both rich live team identities, all aliases, console, self-send,
source/target settings including Hidden, Full restoration, explicit unsupported
input, late-join/leave completion and empty ops.json. Per-client ordered team
markers bound outsider/no-echo assertions. Console runtime output is checked
semantically; its component encoder is compared byte-for-byte in the differential.
[Report](../validation/msg-runtime.json). Focused signing/cache tests prove the
message range, secure unsigned rejection and source/target cache ownership;
synthetic delivery signatures do not claim real-account trust. Existing say/emote
differentials and server tests remain green.

Selector/UUID target resolution, offline profiles, external moderation,
real-account trust and full #24/#63 remain out of scope. Scope_version1,
denominator100 and counts58/53 unchanged. Hosted checks stay under #19.
