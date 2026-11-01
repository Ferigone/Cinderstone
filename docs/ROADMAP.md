# Cinderstone roadmap

Target: Minecraft Java Edition **1.21**, protocol **767**, DataVersion **3953**,
with an unchanged Java client and a native Rust server runtime.

This roadmap describes implementation work. A checked item means that the named
bounded delivery has code connected to the current runtime; it is not a declaration
of complete vanilla parity or comprehensive validation. **Partial** groups remain
open even when individual subfeatures work.

The separate fixed 100-item planning checklist currently records **55/100 code**
and **50/100 main-server integration**, at `scope_version: 1`. These indicators do
not measure behavioral coverage, engineering effort or remaining time. This more
detailed roadmap does not add points to, rename or replace that checklist.

## 1. Protocol and connection lifecycle — Partial

- [x] Handshake, server status and ping.
- [x] VarInt framing, packet compression, configuration registries and PLAY.
- [x] Keepalive and latency publication.
- [x] Online-mode RSA/AES and official session verification implementation.
- [x] Player tracking, metadata, equipment and chunk visibility.
- [ ] Verify a complete login with a real account against the public deployment.
- [ ] Finish secure chat, signed-message handling and chat lifecycle.
- [ ] Complete reconfiguration, cookies, resource packs and custom payload handling.
- [ ] Complete admission limits, bans and administration.

## 2. Native world lifecycle — Partial

- [x] Native flat-world edits, scheduled water/lava and supported fire callbacks.
- [x] Live lighting after edits, heightmaps and chunk readiness ownership.
- [x] Shared BLOCK scheduling, including fire and supported container/redstone ticks.
- [x] Supported random ticks for grass/mycelium, farmland moisture and crop growth.
- [x] Native world/player/item checkpoint and restart owners.
- [ ] Complete entity and chunk unload/reload lifecycle.
- [ ] Complete all world phases, dimensions and saved-data owners.
- [ ] Support the remaining block callbacks and runtime block entities.
- [ ] Complete imported Java world interoperability and older-version DataFixing.

## 3. Player and survival — Partial

- [x] Native creative player body tick and shared movement state.
- [x] Collision, poses, creative flight and native water/lava movement.
- [x] Food state and supported food/drink use with real item-use timing.
- [x] Native player statistics and recipe-book persistence.
- [x] Bounded current-version Java `player.dat` import/export.
- [ ] Full survival movement and attribute/equipment effects.
- [ ] Complete damage, health, death and respawn pipeline.
- [ ] Potions, status effects and all special item-use actions.
- [ ] Elytra, climbing, complete powder-snow behavior and fall interactions.
- [ ] Sleep, portals, dimension travel, vehicles and passengers.

The current runtime profile is an alive, unmounted creative player without active
status effects. Implemented numerical survival routines do not remove that limit.

## 4. Inventories and crafting — Partial

- [x] Canonical player inventory and all player-menu click modes.
- [x] Shared authoritative transactions, slot synchronization and cursor handling.
- [x] 2×2 inventory crafting and bounded recipe-book state.
- [x] 3×3 crafting table, recipe remainders and shift crafting.
- [x] Single/double chest and barrel owners, menus and native persistence.
- [ ] Complete furnace-family processing, rewards and world interactions.
- [x] Bounded furnace/smoker/blast-furnace engines, default recipes/fuels,
      authoritative three-slot menus, processing ticks, counters and lit states.
- [x] Native furnace persistence and real experience-orb reward integration.
- [x] Bounded creative furnace destruction with owned contents and stored recipe
      XP, with a recorded local item/experience-conservation and cold-restart flow.
- [ ] Furnace removal with active menu viewers, survival breaking and remaining
      world-removal contexts.
- [ ] Comparator receivers, Mending repairs and combined menu output-XP/item-drop paths.
- [ ] Custom cooking datapacks and dynamic recipe reload.
- [ ] Hoppers and inventory transfer between block entities.
- [ ] All remaining menus, workstations and special containers.
- [ ] Full recipe-book placement, unlocks, advancement rewards and limited crafting.
- [ ] Remaining complex item components and context-dependent recipe processing.
- [ ] Full loot, enchantment, durability and tool behavior.

Active-viewer furnace removal is the current integration stage. It must retain
the original removed container for its existing menus, close invalid menus in the
actual later player phase, return cursor contents once and reject rebinding to a
new furnace at the same position. This work is **IN PROGRESS**; its checklist
entry remains open pending implementation and the recorded bounded flow.

The earlier creative block-removal implementation requires every furnace menu viewer
to close first. It stages actual container contents and stored recipe XP without
awarding the furnace block's loot item in creative mode. Its
[focused local flow](validation/furnace-removal-flow.json) passed for all three
furnace types, actual item/XP entities, normal cursor return, the sword guard and
cold restart with conserved item/experience totals. Hosted deployment remains a
separate verification scope; this result does not establish full vanilla parity.

The earlier bounded furnace Main integration passed its local protocol-client flow:
all three cooking types, menu transfers, lit/light updates, XP pickup and native
save/cold restart. The [recorded flow](validation/furnace-preview-flow.json)
identifies the exact binary and scope. It also exercised the normal 20 TPS
deadline clock. Sprint/freeze tick control and complete Java task-queue semantics
remain outside that bounded clock change. The family remains partial. Saved
native state retains block-entity registration order; legacy records restore
their recorded order. The result does not establish graphical-client or public
gameplay validation. Hosted release startup is recorded separately in the
[deployment observation](validation/railway-preview.json).

## 5. Redstone, TNT and moving blocks — Partial

- [x] Weak/direct signal queries and supported solid-block relay.
- [x] Levers, buttons and lamps.
- [x] Repeaters: delay selection, locking, scheduling priorities and pulse handling.
- [x] Torch inversion/burnout and source-derived pressure-plate logic.
- [ ] Complete pressure-plate callbacks for every native actor host.
- [ ] Wire topology, strength propagation and all related shape updates.
- [ ] Observer lifecycle and output scheduling.
- [ ] Comparators with their actual block-entity and analog-output owners.
- [ ] Pistons, moving-block collisions and block-entity movement.
- [ ] Primed TNT lifecycle, blast callbacks and persistent explosion state.
- [ ] Full block loot and actual item spawning during explosions.
- [ ] All remaining redstone receivers and cross-system callback ordering.

A supported signal query does not establish that the block producing the signal
has its complete runtime lifecycle. Unported receivers must remain explicit.

## 6. Mobs, entities and AI — Partial foundations

- [x] Bounded experience-orb creation, shared ticking, pickup and tracking for the
      supported native player profile.
- [ ] Remaining entity types and their complete persistence/replication lifecycle.
- [ ] Natural spawning, mob caps and despawning.
- [ ] Pathfinding, navigation, goals, sensors and behavior trees.
- [ ] Passive animals, breeding and entity interactions.
- [ ] Hostile mobs, bosses and combat.
- [ ] Projectiles, boats, minecarts and passengers.

## 7. Terrain generation — Partial foundations

- [x] Native RNG/noise primitives and density/router foundations.
- [ ] Complete stateful NoiseChunk interpolation and caches.
- [ ] Aquifers, terrain surfaces and biome selection.
- [ ] Carving, features, structures and complete dimensions.
- [ ] Generate missing chunks through the normal runtime lifecycle.

Existing Java chunks can be read without implying that a full native generator
exists. Missing chunks must not silently turn into flat terrain.

## 8. Commands and data-driven gameplay — Planned

- [ ] Commands, permissions and operator lifecycle.
- [ ] Functions and datapack reload.
- [ ] Full dynamic registries and tag ownership.
- [ ] Scoreboards, teams, objectives and criteria.
- [ ] Advancements, predicates, loot tables and item modifiers.

## 9. Hosting and operational use — Partial

- [x] Native server CLI with explicit online/offline modes.
- [x] Persistent native checkpoint owners for the supported profile.
- [x] Publish normal source files in the public repository.
- [ ] Establish reproducible CI builds.
- [x] Deploy the server with explicit login-mode configuration, persistent storage
      and a public TCP endpoint.
- [x] Connect repository updates to automatic deployment.
- [ ] Verify restart/redeploy behavior against persistent data.
- [ ] Complete administration, graceful shutdown and observability.

The deployment address and verified operational status belong in the README after
they exist. Railway service health alone does not prove successful Minecraft login.

## 10. Full parity and performance — Planned final stage

- [ ] Compare supported full gameplay flows against the unchanged Java 1.21 server.
- [ ] Close remaining behavioral mismatches without substituting dummy state.
- [ ] Run comprehensive compatibility and persistence validation.
- [ ] Benchmark equal player counts, worlds, simulation settings and mechanics.
- [ ] Reduce allocation, serialization and chunk-processing costs while preserving
      original RNG ownership, callback order and player-visible behavior.

Full vanilla compatibility remains the goal. It is not the current release status.
