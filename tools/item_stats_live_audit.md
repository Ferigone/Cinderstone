# Live Java/Rust item statistics, post-v5 stage

Run `python3 tools/check_java_item_stats.py --server-binary target/debug/mc121-rust`. It starts separate native and original-Java child servers, then runs the existing frozen `item_live_probe.mjs` stock scenario against each external server. It does not duplicate or alter the drop/pickup actions. The query-only `item_stats_query_probe.mjs` sends two REQUEST_STATS packets per connection and performs no inventory or movement experiment.

The run uses the same offline UUIDs for ItemDropper and ItemObserver before and after the frozen stock scenario. Canonical Java UUID stat files, legacy name stat files, player data and advancement files are checked absent before the private copy is launched. Original StatsCounter's empty constructor and default return value establish the initial semantic zero; missing entries in a dirty packet are recorded as NOT_PRESENT/NOT_DIRTY and never interpreted as zero values.

After the stock clients close, new clients with those same offline UUIDs explicitly request statistics. Dropper's packet contains ITEM_DROPPED stone8 and custom DROP2; Observer's contains ITEM_PICKED_UP stone8. An immediate second request on each connection has no intersection with that player's target keys. Other Java tick/movement statistics may appear and are preserved in raw reports; complete packet maps and packet-entry order are not compared.

A second reconnect happens after that drain. Original PlayerList marks all existing values dirty on each successful login, so the first request must again contain the same explicit targets. The new native login hook does the same. The immediate repeat again lacks the target keys. This tests same-process native UUID ownership, not native restart/file persistence.

The successful run is recorded in `evidence/item-stats-java-rust-live-conformance.json`, with exact binary/JAR/probe hashes and private-world paths. Both platforms passed all seven unchanged stock checks, all three target statistic counters matched, and the canonical reference world remained unchanged. The original outer JAR is SHA1 `450698d1863ab5180c25d7c804ef0fe6369dd1ba`; inner JAR SHA256 is `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.

The existing `item_stats_live_probe.mjs` and its native four-check report remain separate and unchanged. That one-player probe covers a different single-item sequence. This cross-platform wrapper uses the established two-player stock sequence with one Q drop and one Ctrl-Q drop.
