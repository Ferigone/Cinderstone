# Player title display commands (#217)

Permission2 players and console use `title <online name>` with clear, reset,
title, subtitle, actionbar or times. Native grammar uses the original players,
component and TimeArgument wire templates. Times reuse the existing nonnegative
unit/rounding implementation; all arguments are parsed before effects. Name
lookup uses the actual online index, including case-insensitive/self targets.

Main sends the original title/subtitle/actionbar, clear/reset flag or timing
payload through the existing ordered target stream. Direct display packets are
independent of chat visibility. The client owns animations; no duplicate server
animation state is added. Shared typed success/result1 and operator policy use
the current rich target display name. Self-target display precedes its private
success. Console success and operator audience use the existing feedback owner.

Static component parsing/validation is shared with tellraw. Selector/UUID/offline
targets, dynamic contents and unsupported codec variants fail explicitly before
display. Serialization failure returns private failure without disconnecting the
source. General syntax-error decoration, dynamic resolution and client rendering
are not claimed.

`tools/title_oracle.java` compiles with admin_feedback/say helpers using the
[external pinned classpath](admin-feedback.md). Run
`cinderstone.oracle.title_oracle ORIGINAL_JAR fixtures/title-vectors.json`.
Original TitleCommand, ComponentArgument/ComponentUtils and feedback run against
recording connections. Two permission trees, seven completions and sixteen
selected executions cover every form, time units/negative bound, rich identity,
console/self/case/hidden targets, codec payloads and typed feedback. Unsupported
selector suggestions are excluded from the native literal-name comparison.
No full Java transport or client renderer is executed; proprietary artifacts
remain outside Git.

After `npm ci` and building, run `node tools/title_flow.mjs
 target/debug/mc121-rust REPORT.json`. Three temporary loopback25711 clients check
original tree projection, completions, exact display/private/admin packet bytes,
console/self/case ordering, hidden-chat display and private failures. An oversized
CJK component fails privately, then the same source executes a valid command;
only a short input prefix/length is recorded. Ordered team markers bound audience
assertions. [Report](../validation/title-runtime.json). Local all-target, focused
original title/tellraw regression and 52 server tests pass.

Full #24 remains open. Scope_version1, denominator100 and counts58/53 unchanged;
post-merge hosted verification stays under #19.
