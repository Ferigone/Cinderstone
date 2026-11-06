# Scoreboard runtime (WP-E4 #25)

Main retains one server-wide `Scoreboard` in `MainItemGameplay`; existing model,
packet and NBT codecs remain the source of truth. `main_scoreboard.rs` sends teams
before displayed objectives and their scores at join. Pending changes acquire a
revision before selecting the join snapshot, so queued older changes cannot
replace the snapshot. All live updates use the existing ordered event stream;
losing its history still disconnects rather than fabricating missed effects.

Console and player `/scoreboard`, `/team` and `/trigger` mutate this same owner.
Scoreboard/team mutations require permission 2; an enabled trigger is usable by
its player at permission 0 and locks after use. Read-only criteria reject direct
score writes. Failed commands keep the previous board. Literal score holders,
`*`, `@s` and `@a` are supported. The broader selector/context and exact suggestion
catalog remain WP-E3 #24; unsupported selectors fail explicitly. The advertised
scoreboard roots use greedy arguments until that catalog is integrated.

Native holders carry the current team-decorated display component, including
click/insertion and player-entity hover identity, so `displayautoupdate` replaces
stale score display overrides. Command execution receives display-bearing holders
for the actual source and online targets, including trigger updates. The native-player hover form round-trips through
the existing JSON/NBT component model; other entity/item hover contexts stay open.

Player tick reads real synchronized health, absorption and air, saved food/XP,
and the admitted effect-free equipment projection. Native statistic producers
and workstation/drop commits update criterion scores using the original award amounts,
even when the cumulative statistic has saturated. Prepared transaction snapshots
track award sequence numbers so intermediate commits cannot replay an award. Team packets carry color,
prefix/suffix, visibility, friendly-invisibility and collision settings to the
vanilla client; chat resolves the current team-decorated name through the full
component codec, including font and hover styling. Survival damage,
death/kill consumers (#54), persistent/effect-modified attributes (#56), and
unsupported server entity interactions (#53) keep their existing boundaries.
The generic model and hooks for those future consumers are already present;
this delivery does not claim those gameplay systems exist.

`data/scoreboard.dat` is loaded from the Anvil world or the native world file's
parent. Version 3953 is accepted; other versions fail without overwriting the
file. Dirty saves write gzip NBT to a sibling temporary file, fsync it, rename,
then fsync its directory, before `level.dat`. Unknown root/payload extension
fields survive; the four owned payload fields are replaced, including removal
of an empty `DisplaySlots`. Native world/player/entity checkpoint formats and
the Railway volume are unchanged.

Evidence: the existing original-class model/packet/NBT fixtures remain in
`docs/reference/scoreboard.md`. `tools/scoreboard_runtime_probe.mjs` runs one flow
unchanged against native Main and the pinned unchanged Java server. It observes
the styled team prefix, membership and chat delivery, sidebar score from a second player's real
`leave_game` stat, denied non-op mutation, an enabled non-op trigger, display-name/number-format command grammar, automatic display refresh for
statistic and trigger updates, and cold
restart. `docs/validation/scoreboard-runtime.json` retains every scoreboard frame
and saved NBT for both servers. `PlayerScores` rows represent a map keyed by
(holder, objective), whose Java objective-identity iteration order is not semantic;
the comparison keys those rows while retaining their original order in the report.
All packet order and all NBT values, including component strings, are compared. This is evidence for that scenario,
not a full command grammar or general vanilla-parity claim.
