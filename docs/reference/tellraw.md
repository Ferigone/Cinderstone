# Static tellraw delivery (#215)

Permission2 players and console use `tellraw <online name> <JSON component>`.
The native dispatcher preserves the original players argument flags, component
argument wire ID17 and selected UTF-16 ranges. Names resolve against the actual
online index, including case-insensitive/self targets. Strict JSON is parsed
before delivery; trailing input, including trailing spaces, fails as in the
selected original cases. Literal, translatable/static arguments, keybind,
siblings and supported styles/events use the existing Component codec.

Main sends overlay-false SystemChat through the existing ordered player event
stream to the actual target. The retained visibility owner allows FULL/SYSTEM
and suppresses HIDDEN. Result1 counts the target even when delivery is suppressed;
there is no success or operator echo. Selectors, UUID/offline targets, dynamic
score/selector/NBT contents (including nested/hover content) and unsupported
codec variants fail explicitly before any target delivery. Network serialization
failures also return private command failure; an oversized text does not escape
into the PLAY loop. General parser/error
message compatibility is not claimed.

`tools/tellraw_oracle.java` compiles with admin_feedback/say helpers using the
[external pinned classpath](admin-feedback.md). Run
`cinderstone.oracle.tellraw_oracle ORIGINAL_JAR fixtures/tellraw-vectors.json`.
Original TellRawCommand, ComponentArgument, ComponentUtils and
ServerPlayer.sendSystemMessage execute with original codecs and recording
connections. The witness has two permission trees, six completions and fourteen
selected executions; no full Java server/transport execution is claimed.
Unsupported selector suggestions are removed from the original projection when
comparing the native literal-name completion policy. Java artifacts stay outside
Git.

After `npm ci` and a build, run `node tools/tellraw_flow.mjs
 target/debug/mc121-rust REPORT.json`. Three temporary loopback25710 connections
check the decoded original tree projection, literal-name completions, selected
exact target-only packet bytes, console/case/self routing, visibility and private
failures with no target/admin echo. Ordered team markers bound each audience
assertion. A 22,000-CJK-character boundary input fails privately, followed by a
successful short message on the same connection; only a prefix/length is recorded.
[Runtime report](../validation/tellraw-runtime.json). The focused
original differential, all-target compilation and server tests pass.

Full #24 and dynamic/selector/offline/unsupported-component expansion remain
open. Scope_version1, denominator100 and counts58/53 stay unchanged. Hosted
verification remains under #19.
