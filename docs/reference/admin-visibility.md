# Shared administration feedback visibility (#199)

Main now filters private success/error feedback and operator broadcasts through
the actual player's current system-message visibility. Full and Commands Only
permit these messages; Hidden suppresses them. The existing permission,
gamerule, source-exclusion and console policies are retained. Difficulty/border
state packets remain independent of chat visibility. The listener's explicit
`chat.disabled.options` rejection of hidden-source commands is a separate path.

`tools/admin_visibility_oracle.java` executes original 1.21
`ServerPlayer.sendSystemMessage`, including its private visibility decision,
and `CommandSourceStack.sendSuccess/sendFailure/broadcastToAdmins`. Only the
connection's final send is recorded. Three visibility rows cover private,
failure, operator and console-origin packets. The original JAR is pinned by
size/SHA-1; this is an original-class witness, not a full Java protocol run.

Use the [existing external bootstrap/classpath](admin-feedback.md) and compile
both `tools/admin_feedback_oracle.java` (shared encoding/reflection helpers)
and `tools/admin_visibility_oracle.java` outside the checkout. Run
`cinderstone.oracle.admin_visibility_oracle ORIGINAL_JAR
fixtures/admin-visibility-vectors.json` with that classpath.

After `npm ci` and a native build, run `node tools/admin_visibility_flow.mjs
target/debug/mc121-rust REPORT.json`. It creates isolated temporary data on
loopback port25703. Two operators exercise Full → Hidden → Commands Only live
settings, player/console weather feedback, private failure and real difficulty
state changes. A later border-state event from the same ordered audience channel
precedes every absence assertion; no quiet-time inference is used. Exact selected
message bytes match the original witnesses. Hidden-source listener refusal and
subsequent restoration also pass. [Report](../validation/admin-visibility-runtime.json).

Additional command/signing/selector work and message paths outside the shared
administration dispatcher remain out of scope. Full #24/#64 and the aggregate
checklist row remain open. Scope_version1, denominator100 and58/53 are unchanged.
Railway/TCP checks are recorded under #19.
