# Native player statistics — bounded 1.21 contract

Pinned original inner JAR: `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`, protocol 767, DataVersion 3953.

`src/player_stats.rs` implements a server-side value map, dirty set, AwardStats payload and canonical current-version JSON data. Gameplay callers must invoke it at the original trigger point. It does not implement scoreboard objectives, advancement criteria, old DataFixer upgrades or every gameplay trigger.

## Original methods and arithmetic

The checked-in `player_stats_*_bytecode.txt` files are `javap -p -c` output from the original classes. `awa` is StatsCounter, `avv` is ServerStatsCounter, `avw` is Stat, `avy` is StatType, `avz` is Stats and `aby` is ClientboundAwardStatsPacket.

`awa.b(Player, Stat, int)` widens both operands to long, adds, applies `Math.min(sum, 2147483647L)`, converts with `l2i`, then calls virtual `setValue`. It clamps only the upper end. In particular, `INT_MIN + INT_MIN` becomes zero, and `INT_MIN + -1` becomes `INT_MAX`. The native implementation preserves these results. `avv.setValue` always adds the key to its dirty set, including repeated equal values and explicit zero. Absent lookups return zero without inserting a value.

Original `sendStats` copies and clears the dirty set, builds a map from the current values and calls the outer connection sender. The connection observer checks that dirty is already empty at that call. Repeated sends still emit an empty AwardStats packet. A request to mark all dirty covers existing stored keys, including zero values.

The exact original registry contains 8,060 valid pairs: mined 1,060; crafted/used/broken/picked_up/dropped 1,333 each; killed/killed_by 130 each; custom 75. Pickup is type 4, dropped is type 5, custom is type 8, and custom `minecraft:drop` is value 22. The actual original clientbound GameProtocols template assigns AwardStats packet ID 4. The packet payload is count followed by VarInt type, VarInt value and VarInt counter for each entry.

## Pickup and drop trigger boundaries

The pickup inputs come from the existing original `ItemEntity.playerTouch` and original `Inventory.add` fixture, including partial transfers that return false and therefore award no pickup statistic. The award uses the captured original item type and original count, after collect/discard/count-restore handling. `award_pickup(item_id, original_count)` accepts exactly those captured values; it must not substitute accepted count or the now-empty inventory argument.

The separate drop helper calls unchanged original `ServerPlayer.drop(ItemStack, around, traceItem)` and its unchanged `Player.drop` superclass. Only the outer world-add and awardStat effects are intercepted. Every award calls real original StatsCounter.increment. It demonstrates:

1. Null construction produces no world-add attempt and no awards.
2. A constructed item is passed to `Level.addFreshEntity` before awards. That method's false return does not suppress awards.
3. Without traceItem, no drop statistics are awarded.
4. With traceItem, a nonempty returned entity stack selects the ITEM_DROPPED item type; its amount is read from the original argument at this call site.
5. The custom DROP statistic increments by one after that item statistic. It also increments when the controlled world callback leaves the returned stack empty, so long as construction succeeded and traceItem is true.

`DropStatEvent` exposes these facts rather than treating world insertion success as the award gate. The ordinary runtime can pass its known unchanged stack facts. A future world-add hook that mutates either stack must pass the post-callback facts at the same original point.

## Oracle and comparison scope

`player_stats_generate.py` executes unchanged counter arithmetic, dirty handling, sendStats, current-version parseLocal/toJson and actual original stream codecs. The network boundary merely records the packet and current dirty set. `player_stats_drop_generate.py` executes the unchanged original drop methods. `player_stats_check.py` replays both frozen corpora before running the three native tests and writes `evidence/player-stats-conformance.json`.

The counter corpus covers 570 histories and 4,840 snapshots, including signed extremes, repeated sets, dirty draining, all registry IDs, 74 actual pickup award events and canonical current-version JSON loads. The drop corpus covers 672 cases with AIR/stone/ender-pearl, empty/negative/positive/extreme counts, both trace/around booleans, both world-add return values and controlled post-add stack mutations.

Original HashSet/Object2IntOpenHashMap iteration can vary with object identity. Multi-entry unordered packets are compared as decoded maps. Explicit actual Object2IntLinkedOpenHashMap input, and all empty or single-entry packets, compare literal payload bytes. Native JSON and network map ordering is canonical; semantic equality is the contract for the unordered original data.

Current-version JSON loads merge values directly without creating new dirty entries and preserve any dirty entries already present. Stored zero and negative values are serialized. The native loader deliberately rejects old versions, unknown names and noncanonical numeric values instead of silently claiming DataFixer or malformed-file compatibility. Disk-path ownership and save timing remain the runtime host's responsibility.
