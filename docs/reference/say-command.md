# Literal say command (#205)

Permission2 players and console execute `say <message>` through native Brigadier.
The MessageArgument network type and permission-filtered tree are real. Literal
text retains spacing, quotes and Unicode up to256 UTF-16 units. Unavailable
selector expansion is explicitly rejected before delivery. Result is1; there
is no private/admin success echo.

Main uses the actual retained chat audience and configured `minecraft:say_command`
registry ID, current team-decorated player display name or console Server name.
A signed command retains its AuthorizedCommand message and verified literal
range. Existing per-recipient filtering, visibility, packet encoding, signature
cache/pending-message, secure-profile and command-spam owners remain in use.
Console broadcasts have no player filtering source. Commands Only/Hidden
recipients receive no player chat; Full recipients do. Normal chat/team paths
retain their player source after the shared audience gained an optional console
source. No signing or trust mechanism is added.

`tools/say_oracle.java` compiles with `tools/admin_feedback_oracle.java` using the
[external pinned bootstrap/classpath](admin-feedback.md). Run
`cinderstone.oracle.say_oracle ORIGINAL_JAR fixtures/say-vectors.json`.
It executes original SayCommand registration/MessageArgument resolution and
result, current Player display names and original disguised-chat codec. A
recording PlayerList broadcast sink replaces actual audience delivery; original
ChatType.bootstrap provides registry holders. This is not a full Java server
flow. The differential checks three permission trees and eight execution cases.
Proprietary artifacts remain outside the checkout.

After `npm ci` and building, run `node tools/say_flow.mjs
target/debug/mc121-rust REPORT.json`. The temporary native flow uses loopback
port25706 and a permission2 player/non-op observer. Complete selected packets
compare original body/name bytes with the actual configured say registry ID
(the witness bootstrap's holder order is separate). It covers player/console,
permission denial, live rich team identity, visibility suppression/restoration,
syntax/selector/length rejection and no success echo. Ordered team packets at
each recipient bound absence assertions. [Report](../validation/say-runtime.json).
Focused signing tests cover secure unsigned rejection and downstream signed
cache/visibility delivery; the latter uses a disclosed synthetic signature,
while cryptographic authorization remains covered by existing listener tests.

Selectors, external moderation, real-account trust and full #24/#63 remain out
of scope. Scope_version1, denominator100 and counts58/53 are unchanged. Hosted
checks remain under #19.
