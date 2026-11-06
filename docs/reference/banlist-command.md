# Native ban-list command

Standalone #190 under #24/#64, Java1.21 / protocol767.

Player and console `banlist`, `banlist players`, and `banlist ips` now parse
through native Brigadier at permission3. Main queries the retained ban owner only
after successful parsing. It retains player-before-IP grouping and each owner's
encounter order; the existing Rust HashMap is not claimed to reproduce Java's
bucket order. Counts, empty response, names/IPs, sources and reasons use the
original typed feedback. The name is a literal Component, not an argument String.
Main uses the canonical network component codec and keeps all query output private.

A query does not expire or mutate bans: like original StoredUserList.getEntries,
it includes expired entries still retained by the owner. Admission lookup and ban
mutations keep their existing behavior and files. No profile cache, remote lookup,
date/locale or collection implementation is introduced here.

## Evidence

The focused harness executes unchanged BanListCommands against actual original
UserBanList/IpBanList, including a retained expired player entry. It records usable
trees/completions at permissions0–4 and13 executions with result/component bytes.
The Rust fixture test checks native grammar, selected kinds and all feedback bytes.
Proprietary Java inputs and compiled classes stay outside the checkout.

```sh
python tools/banlist_command_vectors.py --named-jar /outside/server-named.jar --original-jar /outside/server-1.21.jar --libraries /outside/libraries --out fixtures/banlist-command-vectors.json
cargo check --locked --all-targets
cargo test --locked --lib commands::banlist_command
node tools/banlist_command_flow.mjs target/debug/mc121-rust /outside/server-1.21.jar /outside/eula.txt docs/validation/banlist-command-runtime.json
```

The real Java/native protocol comparison covers non-op denial, operator tree and
completion, actual filtered entries, private queries with another operator,
invalid syntax without mutation (including doubled separators rejected by Java,
previously accepted by the bounded parser), console output, IP pardon/re-query and cold
restart with retained operators and player bans. Each entry's bytes and membership
are exact; cross-language ordering within a map is explicitly normalized, while
player-before-IP grouping is checked before normalization. General syntax-error
rendering and timing are excluded. The root-prefix `b` comparison retains the
implemented ban/ban-ip/banlist roots. Console evidence checks emitted text/component,
not the differing log envelope. Full #24/#64 and their checklist rows remain partial.

Local build, focused differential test and live flow pass. Full library test
metadata exceeds the worker memory limit when combined with the live server;
hosted CI supplies the complete all-target/library/binary test gate before merge.
