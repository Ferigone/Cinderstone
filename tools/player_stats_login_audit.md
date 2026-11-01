# Original 1.21 statistics login baseline

This audit reads existing captures and canonical source files. It starts no
server and changes no core or world data. The pinned original inner-JAR SHA256
is `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.

Original `PlayerList.placeNewPlayer` (`aur.a(vt,aqv,arj)`) calls `aqv.I()` at
bytecode491 and `avv.c()` at494. This marks all stored keys dirty on every login,
including reconnect. `markAllDirty` adds the value-map key set, creating no absent
keys. `ServerStatsCounter` begins with an empty dirty set and only parses an
existing regular file; `StatsCounter` supplies default lookup0. PlayerList checks
the UUID JSON first and can migrate a safe legacy player-name JSON. Removing a
player saves statistics and removes its counter from the PlayerList UUID cache.
The existing aur dump is preserved, not rewritten. Method offsets and hashes are
in `evidence/player-stats-login-audit.json`.

Canonical source files for ItemDropper UUID
`3d622020-0be1-3537-84ba-cebc47523822` and ItemObserver UUID
`059dd245-1232-32ff-9a92-d0155dc90c73` are absent: UUID statistics, legacy
name statistics, playerdata and advancements. Thus fresh-process semantic0 is
justified separately by file absence, the constructors and no preceding item
actions. A missing packet entry means NOT_PRESENT/NOT_DIRTY, never wire value0.
Completed private Java stock worlds already contain drop8/customdrop2/pickup8;
they cannot be reused as fresh-zero worlds.

The new live run reconnects the same offline UUIDs after frozen stock closes its
clients. Its first requested packet explicitly contains dropped stone8 and
custom drop2 for ItemDropper, and picked-up stone8 for ItemObserver. A second
request on that same connection omits these target keys. Another reconnect
re-marks and explicitly returns the values again. Unrelated vanilla movement and
tick statistics are allowed; complete packet-map equality is not asserted.

`evidence/item-stats-java-rust-raw-audit.json` embeds all six raw query reports,
24 target projections, and both actual final Java statistics files with hashes.
`python3 tools/item_stats_raw_audit.py` repeats this read-only comparison against
the retained captures; it does not rerun either server. Native stat-file/restart
persistence, complete player ticks, recipes and advancements remain unverified.
