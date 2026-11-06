# Player list command (#183)

Console and every player can execute `/list` and `/list uuids` through native
Brigadier. Main snapshots the actual admitted actors in admission order, the
current scoreboard and the configured maximum. `/list` uses the existing
Player.getDisplayName implementation, including current team prefix/suffix/color
and identity hover/click/insertion style. `/list uuids` uses plain names and
canonical UUIDs. Joining, disconnecting and rejoining change the next result.
The result is the online count. Feedback stays private to the source and honors
the player's `sendCommandFeedback`; console feedback remains available.

The original-class witness executes `ListPlayersCommand` and original player
display formatting, replacing only external world/team lookup. It compares
empty/single/multiple roster components, results, grammar and permission trees.
Regenerate with `tools/list_command_vectors.py --named-jar ... --original-jar ...
--libraries ... --out fixtures/list-command-vectors.json`; use the existing
pinned bootstrap/remapper and keep proprietary files outside the checkout.

`node tools/list_command_flow.mjs BINARY PINNED_JAR EXISTING_EULA REPORT` runs the
real Java/native protocol flow in isolated temporary directories after `npm ci`.
It requires existing EULA acceptance and ports25641/25642. It compares complete
feedback bytes, tree and suggestions with non-op execution, roster changes,
current team formatting, invalid-input denial, console output, privacy and
feedback suppression. It does not touch Railway's data.

[Runtime evidence](../validation/list-command-runtime.json) records the executed
source/binary identity. Syntax-error rendering and other command migrations are
outside this delivery. The existing player-capacity policy is unchanged. Full
#24 remains open, commands_data.1 partial, scope1/100 and counts58/53 unchanged.
Hosted verification remains tracked under #19.
