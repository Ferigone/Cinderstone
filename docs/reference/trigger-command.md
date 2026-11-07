# Native trigger command (#203)

Non-op players execute `/trigger <objective> [add <value>|set <value>]` through
native Brigadier. The declared objective/integer arguments and live suggestions
use actual enabled trigger objectives for the caller. Successful default/add/set
operations return the resulting signed score (including wrapping) and consume
one enable. Complete parsing and read-only domain checks precede mutation;
syntax, unknown/non-trigger/disabled objectives and console execution leave
scores and locks intact and create no missing entries.

Main resolves the actual source holder, mutates the retained Scoreboard and
publishes its real tracked-objective score packets. Typed private feedback
preserves formatted objective display names. Success also follows original
operator broadcasting, including a non-op source, through the shared live
visibility/gamerule adapter. Existing scoreboard.dat persistence stores value
and lock together. The legacy trigger parser is no longer a Main route.

`tools/trigger_oracle.java`, compiled with `tools/admin_feedback_oracle.java`
using the [external pinned bootstrap/classpath](admin-feedback.md), executes
original TriggerCommand, ServerScoreboard and ScoreAccess with recording
transport. Run `cinderstone.oracle.trigger_oracle ORIGINAL_JAR
fixtures/trigger-vectors.json`. The focused Rust differential compares the
original tree, eight suggestion cases and fourteen execution cases: result,
typed feedback, value/lock state and score packet bytes. Proprietary artifacts
remain outside the checkout. This witness is not a full Java server flow.

After `npm ci` and a native build, run `node tools/trigger_flow.mjs
target/debug/mc121-rust REPORT.json`. The temporary native flow uses loopback
port25705. It checks actual non-op use, all three forms, invalid input retaining
enable/value, live completion, private/operator/score bytes, hidden-observer
feedback suppression while score packets continue, and native cold restart.
Ordered team packets bound absence assertions. [Report](../validation/trigger-runtime.json).

Full #24, other scoreboard roots, selectors/signing expansion and general
syntax-error context decoration remain out of scope. Scope_version1,
denominator100 and counts58/53 remain unchanged. Hosted checks stay under #19.
