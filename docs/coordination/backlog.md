# Work-package backlog

Planner record by instance `planner-20261101-d4b8`. The live source of truth is the
[master plan issue #14](https://github.com/Ferigone/Cinderstone/issues/14) and the
work-package issues it links; this file is a versioned snapshot taken against
master `874110c`. If they disagree, the issues win.

This backlog does not change `scope_version: 1`, the 100-item denominator, any
delivery definition or any completion flag. Only the integration coordinator
records checklist changes. Counts remain **56/100 code** and **51/100 Main**; they are
planning indicators, not behavioral compatibility, effort or remaining time.
Reaching 100/100 would not mean vanilla parity: parity and performance are tracked
separately (WP-Q1, WP-Q2) and need evidence from equal workloads.

## Lanes

| Lane | Scope | Default areas | Existing owner |
| --- | --- | --- | --- |
| A worldgen | W1, W2 after team issue #2 | `src/worldgen/**`, `src/density*` | team #2 |
| B workstations/inventory | I4 after team issue #3 | `src/workstations/**`, `src/container_workstation_*.rs` | team #3 |
| C entities/survival/items | S1-S5, I1-I3 | `src/living_entities/**`, `src/combat/**`, `src/effects/**` | team #4 |
| D mobs | M1-M5 | new `src/mobs/**`, `src/ai/**` | none yet |
| E data-driven | E1-E8 | new `src/commands/**`, `src/datapack/**`, `src/loot/**`, `src/advancements/**`, `src/scoreboard/**` | WP-E3 library stages (planner instance), WP-E4 stages 1-3 merged (#112); no team issue yet |
| F blocks/simulation | F1-F10 | new `src/blocks/**`, `src/redstone/**`, `src/pistons/**` | none yet |
| G world/persistence | G1-G6 | existing `storage.rs`, `level.rs`, `anvil.rs` only after an exact claim | coordinator for shared seams |
| H central Main | H1-H4, P1-P3, Q1-Q2 | `src/main*.rs`, manifests, deployment | coordinator only |

A lane without an owner needs its own team issue and prompt from the coordinator.
Until then a work package can be claimed by any instance with a compact claim
(`docs/coordination/README.md`). Reserved seams stay with the coordinator.

## Phases

- **0 hygiene:** P1 CI gate, P2 all-targets build, P3 public endpoint verification.
- **1 foundations:** E1, E2, E3, E4, E8, F1, F4, G1, G2, W1, S2, S3, S4, M2, M3, H1.
- **2 survival loop:** E5, E6, F2, F3, F6, F7, G3, G4, H2, H3, H4, I1, I2, I4, M1, S1, S5.
- **3 world breadth:** E7, F5, F8, F9, F10, G5, I3, M4, M5, W2.
- **4 hardening:** G6, Q1, Q2, parity gap closure, soak, hosted verification.

Cross-cutting blockers: **G1** multi-level runtime, **E1** dynamic registries and
datapacks, **S4** attributes and effects, **S2** generic entity types, **E2** loot.

## Index

| WP | Issue | Delivery | Row(s) | Phase | Lane | Blocked by |
| --- | --- | --- | --- | --- | --- | --- |
| WP-P1 | [#17](https://github.com/Ferigone/Cinderstone/issues/17) | CI gate for PRs and master | (Roadmap §9) | 0 | H | - |
| WP-P2 | [#18](https://github.com/Ferigone/Cinderstone/issues/18) | Repair cargo check --all-targets | (hygiene) | 0 | H | - |
| WP-P3 | [#19](https://github.com/Ferigone/Cinderstone/issues/19) | Public endpoint, real-client login, redeploy persistence | (Roadmap §1/§9) | 0 | H | needs human |
| WP-Q1 | [#20](https://github.com/Ferigone/Cinderstone/issues/20) | Differential parity harness vs unchanged Java server | (Roadmap §10) | 4 | H | - |
| WP-Q2 | [#21](https://github.com/Ferigone/Cinderstone/issues/21) | Equal-workload benchmarking and optimization | (Roadmap §10) | 4 | H | Q1 |
| WP-E1 | [#22](https://github.com/Ferigone/Cinderstone/issues/22) | Dynamic registries, tags, datapack loading/reload | registries.5 | 1 | E | - |
| WP-E2 | [#23](https://github.com/Ferigone/Cinderstone/issues/23) | Loot tables, predicates, item modifiers | commands_data.5 | 1 | E | E1 (loading), E4, I2 |
| WP-E3 | [#24](https://github.com/Ferigone/Cinderstone/issues/24) | Brigadier, selectors, command catalog | commands_data.1 | 1 | E | - |
| WP-E4 | [#25](https://github.com/Ferigone/Cinderstone/issues/25) | Scoreboard objectives, teams, criteria | commands_data.3 | 1 | E | G2, E3 |
| WP-E5 | [#26](https://github.com/Ferigone/Cinderstone/issues/26) | Advancements and criterion triggers | commands_data.4 | 2 | E | E1, E2, E4 |
| WP-E6 | [#27](https://github.com/Ferigone/Cinderstone/issues/27) | Recipe unlocks via advancements | crafting.4 | 2 | E | E5 |
| WP-E7 | [#28](https://github.com/Ferigone/Cinderstone/issues/28) | Functions, schedule, /reload | commands_data.2 | 3 | E | E1, E3 |
| WP-E8 | [#29](https://github.com/Ferigone/Cinderstone/issues/29) | Complex component identity/semantics | registries.4 | 1 | E | - |
| WP-F1 | [#30](https://github.com/Ferigone/Cinderstone/issues/30) | Full redstone | simulation.2 | 1 | F | (I3 for dispensers) |
| WP-F2 | [#31](https://github.com/Ferigone/Cinderstone/issues/31) | Pistons and moving blocks | simulation.3 | 2 | F | F1, F3, F4 |
| WP-F3 | [#32](https://github.com/Ferigone/Cinderstone/issues/32) | Ticking block-entity framework + types | simulation.4 | 2 | F | - |
| WP-F4 | [#33](https://github.com/Ferigone/Cinderstone/issues/33) | World/server tick phases with real owners | simulation.5 | 1 | F | - |
| WP-F5 | [#34](https://github.com/Ferigone/Cinderstone/issues/34) | Callbacks for every vanilla block | block_interactions.5 | 3 | F | F6, W1, F8, F10 (families) |
| WP-F6 | [#35](https://github.com/Ferigone/Cinderstone/issues/35) | Survival breaking, tools, drops | block_interactions.4 | 2 | F | E2, S4, I2, S3 |
| WP-F7 | [#36](https://github.com/Ferigone/Cinderstone/issues/36) | Waterlogging, bubble columns, fluid lifecycle | fluids.5 | 2 | F | G3 (persistence part) |
| WP-F8 | [#37](https://github.com/Ferigone/Cinderstone/issues/37) | Lightning, snow, ice, precipitation | environment.4 | 3 | F | S2, F4, W1 |
| WP-F9 | [#38](https://github.com/Ferigone/Cinderstone/issues/38) | Sleep, portals, dimension transitions | environment.5 | 3 | F | G1, G4, W2, S3 |
| WP-F10 | [#39](https://github.com/Ferigone/Cinderstone/issues/39) | Explosions and primed TNT | (mapping TBD) | 3 | F | S2, E2, S3/S4 |
| WP-G1 | [#40](https://github.com/Ferigone/Cinderstone/issues/40) | Multi-level (dimension) runtime | (enabler) | 1 | G/H | - |
| WP-G2 | [#41](https://github.com/Ferigone/Cinderstone/issues/41) | level.dat + SavedData lifecycle | world_storage.4 | 1 | G | - |
| WP-G3 | [#42](https://github.com/Ferigone/Cinderstone/issues/42) | Entity/chunk unload-reload lifecycle | chunk_lifecycle.4 | 2 | G | G1 (partial), S2 |
| WP-G4 | [#43](https://github.com/Ferigone/Cinderstone/issues/43) | Runtime chunk generation + generator Main integration | chunk_lifecycle.5, world_generation.1/.2 (Main) | 2 | G/A/H | G1, #2, W1 |
| WP-G5 | [#44](https://github.com/Ferigone/Cinderstone/issues/44) | Full Java interop + DataFixers + migration | persistence_ops.4 | 3 | G | G2, G3, S2, G1 |
| WP-G6 | [#45](https://github.com/Ferigone/Cinderstone/issues/45) | Administration, save-all, graceful shutdown | persistence_ops.5 | 4 | G/H | E3, G2, F4 |
| WP-W1 | [#46](https://github.com/Ferigone/Cinderstone/issues/46) | Biomes, carvers, features | world_generation.4 | 1 | A | #2 (.3), E1 |
| WP-W2 | [#47](https://github.com/Ferigone/Cinderstone/issues/47) | Structures + complete dimensions | world_generation.5 | 3 | A | W1, G1, G4, E1 |
| WP-I1 | [#48](https://github.com/Ferigone/Cinderstone/issues/48) | Item and food use actions | items.3 | 2 | C | S4, S2 (families) |
| WP-I2 | [#49](https://github.com/Ferigone/Cinderstone/issues/49) | Durability, enchantments, tools | items.4 | 2 | C | E1, S4, E8 |
| WP-I3 | [#50](https://github.com/Ferigone/Cinderstone/issues/50) | Ranged weapons and projectiles | items.5 | 3 | C | S2, S3, S4, I2 |
| WP-I4 | [#51](https://github.com/Ferigone/Cinderstone/issues/51) | Bundles, shulker/ender, special interactions | inventory.5 | 2 | B | E8, #3 |
| WP-S1 | [#52](https://github.com/Ferigone/Cinderstone/issues/52) | Boats, minecarts, passengers | entities.4 | 2 | C | S2 |
| WP-S2 | [#53](https://github.com/Ferigone/Cinderstone/issues/53) | EntityType framework + entity types | entities.5 | 1 | C | - |
| WP-S3 | [#54](https://github.com/Ferigone/Cinderstone/issues/54) | Damage, death, respawn in Main | survival_combat.3, survival_combat.2 (Main) | 1 | C/H | S4, central owners |
| WP-S4 | [#56](https://github.com/Ferigone/Cinderstone/issues/56) | Effects, attributes, potions in Main | survival_combat.4 | 1 | C | - |
| WP-S5 | [#57](https://github.com/Ferigone/Cinderstone/issues/57) | XP, loot, survival game rules | survival_combat.5 | 2 | C | E2, S3, S2 |
| WP-M1 | [#58](https://github.com/Ferigone/Cinderstone/issues/58) | Natural spawning, caps, despawn | mobs_ai.1 | 2 | D | S2, F4, M3/M4, W1 |
| WP-M2 | [#59](https://github.com/Ferigone/Cinderstone/issues/59) | Pathfinding and navigation | mobs_ai.2 | 1 | D | - |
| WP-M3 | [#60](https://github.com/Ferigone/Cinderstone/issues/60) | AI goals, sensors, brains, roster plan | mobs_ai.3 | 1 | D | S2, M2 |
| WP-M4 | [#61](https://github.com/Ferigone/Cinderstone/issues/61) | Animals, breeding, taming | mobs_ai.4 | 3 | D | M3, M1, S2, E5, E2 |
| WP-M5 | [#62](https://github.com/Ferigone/Cinderstone/issues/62) | Hostile mobs, bosses, raids, combat | mobs_ai.5 | 3 | D | M1-M3, S3, S4, E2, F10, I3, S5 |
| WP-H1 | [#63](https://github.com/Ferigone/Cinderstone/issues/63) | Secure chat closure | authentication.4 | 1 | H | E3, E4, P3 |
| WP-H2 | [#64](https://github.com/Ferigone/Cinderstone/issues/64) | Login limits, bans, administration closure | authentication.5 | 2 | H | E3, H1 |
| WP-H3 | [#65](https://github.com/Ferigone/Cinderstone/issues/65) | Movement handler Main integration | player_motion.3 (Main) | 2 | H | S4, S3, S1, H4 |
| WP-H4 | [#66](https://github.com/Ferigone/Cinderstone/issues/66) | Travel modes, vehicles, movement attributes | player_motion.5 | 2 | H | S4, S1, I3, F9 |

## Coverage of open checklist rows

Open means code or Main not complete in `docs/coordination/checklist-v1.json`. Definitions are the fixed, verbatim `delivery` strings.

| Row | Delivery (verbatim) | Code | Main | Tracked by |
| --- | --- | --- | --- | --- |
| `authentication.4` | Secure chat i podpisane wiadomości | no | no | [WP-H1](https://github.com/Ferigone/Cinderstone/issues/63) |
| `authentication.5` | Pełne limity, bany i administracja logowaniem | no | no | [WP-H2](https://github.com/Ferigone/Cinderstone/issues/64) |
| `registries.4` | Pełna tożsamość złożonych komponentów | no | no | [WP-E8](https://github.com/Ferigone/Cinderstone/issues/29) |
| `registries.5` | Dynamiczne rejestry i przeładowanie datapacków | no | no | [WP-E1](https://github.com/Ferigone/Cinderstone/issues/22) |
| `world_storage.4` | Pełny level.dat i saved-data lifecycle | no | no | [WP-G2](https://github.com/Ferigone/Cinderstone/issues/41) |
| `chunk_lifecycle.4` | Pełny cykl wyładowania encji/chunków | no | no | [WP-G3](https://github.com/Ferigone/Cinderstone/issues/42) |
| `chunk_lifecycle.5` | Generowanie brakujących chunków w runtime | no | no | [WP-G4](https://github.com/Ferigone/Cinderstone/issues/43) |
| `world_generation.1` | RNG i noise primitives | yes | no | [WP-G4](https://github.com/Ferigone/Cinderstone/issues/43) |
| `world_generation.2` | Density functions i router definitions | yes | no | [WP-G4](https://github.com/Ferigone/Cinderstone/issues/43) |
| `world_generation.3` | NoiseChunk, aquifery i powierzchnia | no | no | team #2 |
| `world_generation.4` | Pełne biomy, carving i features | no | no | [WP-W1](https://github.com/Ferigone/Cinderstone/issues/46) |
| `world_generation.5` | Struktury i kompletne wymiary generatora | no | no | [WP-W2](https://github.com/Ferigone/Cinderstone/issues/47) |
| `block_interactions.4` | Pełny survival breaking, drops i narzędzia | no | no | [WP-F6](https://github.com/Ferigone/Cinderstone/issues/35) |
| `block_interactions.5` | Pełne callbacki wszystkich bloków | no | no | [WP-F5](https://github.com/Ferigone/Cinderstone/issues/34) |
| `fluids.5` | Pełne waterlogging i tick lifecycle w serwerze | no | no | [WP-F7](https://github.com/Ferigone/Cinderstone/issues/36) |
| `player_motion.3` | Pełny handler ruchu i korekty | yes | no | [WP-H3](https://github.com/Ferigone/Cinderstone/issues/65) |
| `player_motion.5` | Pełne podróże, pojazdy i atrybuty ruchu | no | no | [WP-H4](https://github.com/Ferigone/Cinderstone/issues/66) |
| `inventory.4` | Skrzynie, piece i pozostałe menu bloków | no | no | team #3 |
| `inventory.5` | Bundles i kompletne interakcje specjalne | no | no | [WP-I4](https://github.com/Ferigone/Cinderstone/issues/51) |
| `crafting.2` | Specjalne/trim receptury i kontekst | yes | no | team #3 (Main) |
| `crafting.4` | Vanilla advancement unlocks/rewards | no | no | [WP-E6](https://github.com/Ferigone/Cinderstone/issues/27) |
| `crafting.5` | Crafting3x3 i wszystkie stacje | no | no | team #3 |
| `items.3` | Używanie jedzenia i przedmiotów | no | no | [WP-I1](https://github.com/Ferigone/Cinderstone/issues/48) |
| `items.4` | Durability, enchantments i narzędzia | no | no | [WP-I2](https://github.com/Ferigone/Cinderstone/issues/49) |
| `items.5` | Broń dystansowa i pociski | no | no | [WP-I3](https://github.com/Ferigone/Cinderstone/issues/50) |
| `entities.4` | Łodzie, minecarts i pasażerowie | no | no | [WP-S1](https://github.com/Ferigone/Cinderstone/issues/52) |
| `entities.5` | Pozostałe typy encji i ich lifecycle | no | no | [WP-S2](https://github.com/Ferigone/Cinderstone/issues/53) |
| `mobs_ai.1` | Naturalny spawning i limity mobów | no | no | [WP-M1](https://github.com/Ferigone/Cinderstone/issues/58) |
| `mobs_ai.2` | Pathfinding i nawigacja | no | no | [WP-M2](https://github.com/Ferigone/Cinderstone/issues/59) |
| `mobs_ai.3` | Cele i zachowania AI | no | no | [WP-M3](https://github.com/Ferigone/Cinderstone/issues/60) |
| `mobs_ai.4` | Zwierzęta i rozmnażanie | no | no | [WP-M4](https://github.com/Ferigone/Cinderstone/issues/61) |
| `mobs_ai.5` | Wrogowie, bossowie i walka mobów | no | no | [WP-M5](https://github.com/Ferigone/Cinderstone/issues/62) |
| `survival_combat.2` | Formuły zdrowia i obrażeń | yes | no | [WP-S3](https://github.com/Ferigone/Cinderstone/issues/54), team #4 (Main) + S3 |
| `survival_combat.3` | Kompletny damage/death/respawn | no | no | [WP-S3](https://github.com/Ferigone/Cinderstone/issues/54) |
| `survival_combat.4` | Efekty mikstur i atrybuty | no | no | [WP-S4](https://github.com/Ferigone/Cinderstone/issues/56) |
| `survival_combat.5` | XP, loot i reguły survival | no | no | [WP-S5](https://github.com/Ferigone/Cinderstone/issues/57) |
| `environment.4` | Pioruny, śnieg, lód i opady | no | no | [WP-F8](https://github.com/Ferigone/Cinderstone/issues/37) |
| `environment.5` | Sen, portale i przejścia wymiarów | no | no | [WP-F9](https://github.com/Ferigone/Cinderstone/issues/38) |
| `simulation.2` | Pełny redstone | no | no | [WP-F1](https://github.com/Ferigone/Cinderstone/issues/30) |
| `simulation.3` | Tłoki i ruchome kolizje | no | no | [WP-F2](https://github.com/Ferigone/Cinderstone/issues/31) |
| `simulation.4` | Pełny ticking block entities | no | no | [WP-F3](https://github.com/Ferigone/Cinderstone/issues/32) |
| `simulation.5` | Pełne fazy świata i serwera | no | no | [WP-F4](https://github.com/Ferigone/Cinderstone/issues/33) |
| `commands_data.1` | Komendy i uprawnienia operatorów | no | no | [WP-E3](https://github.com/Ferigone/Cinderstone/issues/24) |
| `commands_data.2` | Funkcje i datapack reload | no | no | [WP-E7](https://github.com/Ferigone/Cinderstone/issues/28) |
| `commands_data.3` | Scoreboard objectives i teams | no | no | [WP-E4](https://github.com/Ferigone/Cinderstone/issues/25) |
| `commands_data.4` | Advancements i kryteria | no | no | [WP-E5](https://github.com/Ferigone/Cinderstone/issues/26) |
| `commands_data.5` | Loot tables, predicates i item modifiers | no | no | [WP-E2](https://github.com/Ferigone/Cinderstone/issues/23) |
| `persistence_ops.4` | Pełne Java player.dat/region/entity interoperability | no | no | [WP-G5](https://github.com/Ferigone/Cinderstone/issues/44) |
| `persistence_ops.5` | Pełna administracja, save i graceful shutdown | no | no | [WP-G6](https://github.com/Ferigone/Cinderstone/issues/45) |

Rows tracked by team issues keep that owner: `world_generation.3` (#2), `inventory.4`,
`crafting.2`, `crafting.5` (#3), `survival_combat.2` Main (#4, with WP-S3).
WP-F10 (explosions) has no dedicated row; the coordinator decides its mapping.

## CI gate (WP-P1)

`.github/workflows/ci.yml` runs on every pull request and push to `master`:

- **repository hygiene** (`.github/scripts/repo_hygiene.py`): fails on tracked archives,
  Java binaries, world/player saves, credential files or secret patterns, raw JVM
  disassembly, missing license/notice files, and on any structural change to the fixed
  checklist (`scope_version` must stay 1, 100 rows, summary counts equal to the rows).
  It reads only tracked files and never needs the Java server JAR. A sanitized, public
  exception goes into `.github/hygiene-allowlist.txt` with a reason and is reviewed.
- **build**: the Dockerfile's Rust version, `cargo check --locked --bins --lib`,
  `cargo check --locked --all-targets`, `cargo test --locked --lib` and
  `cargo test --locked --bin mc121-rust`. These are build and regression gates,
  not a measure of behavioral compatibility or progress.

Railway redeploys every push to `master`. The repository owner should require both CI
jobs in branch protection; the planner has no tool for repository settings.

## Claiming and supervision

1. Pick a work package whose blockers are satisfied and verify it against current `master`.
2. Use a unique instance ID and branch `teams/<lane>/<instance-id>/<stage>` from current `origin/master`.
3. Post the compact claim from `docs/coordination/README.md` on the work-package issue.
4. Deliver small stages as pull requests that link the work package; request reserved-seam
   changes as integration contracts; renew the claim after each stage.
5. A stale comment is not ownership. The planner flags collisions and stale claims on #14;
   the coordinator resolves them. No background polling happens between sessions.

