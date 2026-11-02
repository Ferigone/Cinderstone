# Cinderstone roadmap

Target: Minecraft Java Edition **1.21**, protocol **767**, DataVersion **3953**,
with an unchanged Java client and a native Rust server runtime.

This roadmap describes implementation work. A checked item means that the named
bounded delivery has code connected to the current runtime; it is not a declaration
of complete vanilla parity or comprehensive validation. **Partial** groups remain
open even when individual subfeatures work.

The separate fixed 100-item planning checklist currently records **56/100 code**
and **51/100 main-server integration**, at `scope_version: 1`. These indicators do
not measure behavioral coverage, engineering effort or remaining time. This more
detailed roadmap does not add points to, rename or replace that checklist.

## 1. Protocol and connection lifecycle — Partial

- [x] Handshake, server status and ping.
- [x] VarInt framing, packet compression, configuration registries and PLAY.
- [x] Keepalive and latency publication.
- [x] Online-mode RSA/AES and official session verification implementation.
- [x] Player tracking, metadata, equipment and chunk visibility.
- [ ] Verify a complete login with a real account against the public deployment.
- [x] Bounded native player chat, actual visibility, session/key ownership,
      signature/chain/acknowledgment/cache primitives and signed administration
      routing; Main build and [local chat flow](validation/chat-flow.json) passed.
- [ ] Complete secure chat, remaining filter/team/component/selector contexts and
      real-profile signed-message end-to-end verification.
- [x] Complete the existing `network.5` reconfiguration/cookie/resource-pack
      delivery with native Main calls and the recorded local configuration flow.
- [ ] Complete dynamic registry/datapack reload, general component handling,
      remaining custom payloads and malformed late-PLAY decoder equivalence.
- [x] Bounded native access lists, ordered LOGIN/configuration admission gates,
      real console/unsigned administration, operator permission/tree refresh and
      configured capacity with explicit stored bypass. The local
      [admission flow](validation/admission-flow.json) passed for source `6c54018`.
- [x] Connect bounded packet-rate enforcement, raw read inactivity, typed
      connection settings, configurable compression and common wire dispatch to
      Main; [local flow](validation/connection-flow.json) passed for local `259e641` (identical published source `87dda91`).
      Railway autodeployed master `1e72694` successfully.
- [ ] Complete admission limits, bans and administration, including remaining
      parser/profile/date contexts and full asynchronous transport equivalence.

The [configuration flow](validation/configuration-flow.json) passed on 1 November
2026 at 15:18:52 Europe/Warsaw for source
`643106e8eb60d970a8df67eb9acf0ab5f107c788`, after a full Main build. Native
SIGHUP moved an admitted connection PLAY → CONFIGURATION → actual new-player
readmission on the same TCP connection without reauthentication. Eleven emeralds,
position and subsequent world interaction were retained. Cookie rejection in
all three phases, initial known-pack/brand ordering and optional/required pack
task responses were exercised. Resource URLs were not fetched or applied.
SIGHUP is the native `RETURN_TO_WORLD` operator path, not datapack reload.
The optimized hosted release reached **SUCCESS**, with one instance, the existing
volume and native offline listening at **15:25:08 Europe/Warsaw** on 1 November
2026. Public gameplay remains unverified because the external status probe fails
DNS resolution. The
existing `network.5` row closes without changing scope or the 100-item denominator;
this is not a full behavioral-compatibility or performance claim.

## 2. Native world lifecycle — Partial

- [x] Native flat-world edits, scheduled water/lava and supported fire callbacks.
- [x] Live lighting after edits, heightmaps and chunk readiness ownership.
- [x] Shared BLOCK scheduling, including fire and supported container/redstone ticks.
- [x] Bounded native pressure callbacks share actual Player/Item/XP/Falling query and sound owners, synchronous entityInside and scheduled Server release; [one Item runtime witness](reference/pressure-item-runtime.md) passed.
- [ ] Complete active pressure/redstone networks, unsupported callback owners, and broader entity-family/weighted/stone runtime contexts.
- [x] Supported random ticks for grass/mycelium, farmland moisture and crop growth.
- [x] Native world/player/item checkpoint and restart owners.
- [x] Reject used/pending/stale native BLOCK restores without their genuine clock/FLUID shared-counter checkpoint, before world restoration/binding/RNG; [finite restore/rejection evidence](reference/scaffolding-counter-owners.md).
- [x] Source-certified canonical Block IDs/properties/state-entry order and actual retained configured Block-tag owner at fixture initialization.
- [x] Bounded fresh CompoundTag history, decoder-issued NBT ownership, source getCompound defaults and canonical EMPTY component branch as callable prerequisites.
- [ ] Connect complete Java block/entity import and full component/BlockEntity map/save/factory/menu owners.
- [x] Bounded concrete-powder production placement/shape/scheduler/landing and native resume using real retained owners; [one actual white-powder water-hardening flow](reference/concrete-powder-runtime.md) passed.
- [ ] Complete direct-water/adjacent-fluid player powder placement, mixed cascades, dynamic shape contexts, high-speed runtime coverage and full Java entity import.

The exact scaffolding interaction full-grid provider is callable, with original
topology and a focused constant-provider observation. Source-global static
collision and bucket OUTLINE admission remain separate; this prerequisite does
not establish new end-to-end scaffolding gameplay. The documented
[synchronous player-water commit contract](reference/concrete-powder-player-commit-contract.md)
is a future integration contract, not implemented admission.
- [x] Bounded native Anvil and ordinary sand/red_sand/gravel falling through actual constructor, scheduler, physics and tracker owners; actual ordinary falls and WATER landing are recorded in [source/runtime evidence](reference/ordinary-falling-runtime.md).
- [x] Single airborne native Falling checkpoint/hidden readiness/PLAY resume with retained body, UUID and RNG and one actual landing; [exact scope](reference/falling-midflight-local-keys.md).
- [ ] Complete special falling classes, active fire/redstone/block-entity neighborhoods, survival victims and Java entity/chunk-save interoperability.
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
Narrow water-movement statistic updates and FoodData healing now reuse native
core logic; `player_motion.3` and `survival_combat.2` remain incomplete.

## 4. Inventories and crafting — Partial

- [x] Canonical player inventory and all player-menu click modes.
- [x] Shared authoritative transactions, slot synchronization and cursor handling.
- [x] 2×2 inventory crafting and bounded recipe-book state.
- [x] 3×3 crafting table, recipe remainders and shift crafting.
- [x] Single/double chest and barrel owners, menus and native persistence.
- [x] Bounded native Stonecutter Main: canonical recipe/callback/transaction,
      immediate pickup, current-menu close, player phase and disconnect/save owners;
      focused local flows and [source evidence](reference/stonecutter-production.md).
- [x] Bounded Smithing Main composition, genuine retained full-state recovery and
      actual three-input transform/close/disconnect/native-restart witness;
      [production limits](reference/smithing-production.md) remain explicit.
- [x] Bounded native Loom Main with actual configured patterns, source-phased
      callbacks, sound/drop owners and [local pattern/lifetime/restart witness](validation/loom-flow.json).
- [x] Bounded Grindstone Main with configured enchantment removal, source-ordered real XP/item effects and [actual conservation evidence](reference/grindstone-production.md).
- [x] Bounded creative Anvil rename/take Main with real falling-block placement, current-menu lifetime and [local runtime witness](validation/falling-anvil-flow.json); broader enchantment/survival contexts remain open.
- [ ] Complete Loom templates/custom/reentrant/identity-order contexts and
      remaining runtime coverage; multi-target drag still needs its actual owner.
- [ ] Complete Smithing trim/custom contexts, remaining workstation families and
      their actual Main lifetime/owner contracts.
- [ ] Complete custom component reference identity, remaining workstation contexts
      and all remaining menu families before closing aggregate checklist rows.
- [ ] Complete furnace-family processing, rewards and world interactions.
- [x] Bounded furnace/smoker/blast-furnace engines, default recipes/fuels,
      authoritative three-slot menus, processing ticks, counters and lit states.
- [x] Native furnace persistence and real experience-orb reward integration.
- [x] Bounded creative furnace destruction with owned contents and stored recipe
      XP, with a recorded local item/experience-conservation and cold-restart flow.
- [x] Bounded active-viewer furnace-container lifetime, later player-phase close
      and independent same-position replacement, with a local two-viewer flow.
- [x] Bounded disconnected furnace-menu cursor drop through actual item ownership,
      with a local two-client observation and exact item conservation on restart.
- [x] Bounded shared close contexts for window0, crafting-table and chest/barrel
      cursors and temporary grids; full Main build and local conservation flow
      passed for the recorded source and binary.
- [x] Retention of the window0 cursor/grid across external menu
      opening and window0-before-external removal during disconnect cleanup.
- [x] Bounded source save-before-menu-removal with persistent-only inventory projection, real leave_game and once-only cleanup ownership; [observed conservation and remaining runtime limits](reference/ordinary-falling-runtime.md).
- [x] Verify the shared menu-close stage's optimized hosted release and native
      offline startup, with one instance and the existing persistent volume.
- [x] Bounded creative removal of a closed single dry chest or
      closed barrel, using actual 27-slot contents, Item/RNG/ID ownership and
      native source-history transition; full Main build and local conservation
      flow passed for the recorded source and binary.
- [x] Verify hosted release and native offline startup of the bounded closed
      single-container removal stage, preserving the existing volume.
- [ ] Player-death cursor handling, full-inventory overflow conformance, complete
      `PlayerList.remove` persistence ordering, survival breaking and remaining
      world-removal contexts.
- [ ] Comparator receivers, Mending repairs and combined menu output-XP/item-drop paths.
- [ ] Custom cooking datapacks and dynamic recipe reload.
- [ ] Hoppers and inventory transfer between block entities.
- [ ] All remaining menus, workstations and special containers.
- [ ] Full recipe-book placement, unlocks, advancement rewards and limited crafting.
- [ ] Remaining complex item components and context-dependent recipe processing.
- [ ] Full loot, enchantment, durability and tool behavior.

The newer bounded removal stage is published for a closed
single dry chest or closed barrel. Opaque container identity and its 27 actual
slots are checked before a privately drained staging clone and native source
history prepare the world transition. Actual contents use the existing native
Item construction, level RNG and shared entity IDs; no XP or creative block-loot
item is constructed. The source order is contents, comparator notification, then
block-entity removal; only a certified empty comparator-receiver path is
supported. Actual open menus, including spectator menus, double/waterlogged
chests, unresolved loot and comparator receivers remain rejected boundaries.
The full Main build and
[focused local flow](validation/container-removal-flow.json) passed for source
`be2582a9ea032205c3885fd04e10ff1419d63fb9` on 1 November 2026 at
14:27:29 Europe/Warsaw. The sword guard preserved the chest and its contents;
subsequent closed chest/barrel removal produced actual Items, left air and no
native container block entities, and conserved exactly 37 dirt, two coal,
33 quartz and seven emeralds after cold restart. Its optimized hosted release
reached SUCCESS and native offline listening at 14:31:18 Europe/Warsaw, with one
instance and the existing volume. Public gameplay remains unverified because
the external status probe fails DNS. See the
[container-removal source ledger](reference/container-removal.md). This bounded
addition does not complete a full fixed checklist row or change the indicators.

The earlier menu stage extends close/disconnect handling to chest/barrel,
crafting-table and window0 ownership. Disconnected cursors and temporary 3×3/2×2
inputs use real item drops; derived result previews are not returned as owned
items. Opening a block menu from window0 retains the old inventory menu's cursor
and grid. Disconnect removal handles window0 before the actual external menu.
Alive insertion packets, old shared-slot local/remote baseline transfer and
server close-before-return/drop publication follow the bounded
[source ledger](reference/menu-close-lifecycle.md). The full Main build and
[focused local flow](validation/disconnected-menus-flow.json) passed for source
`93438c6055ec755e85f528739a41073ef5563b5a` on 1 November 2026 at
14:20:00 Europe/Warsaw. The flow covered all four disconnect families, ordinary
connected chest return, retained window0 cursor/grid ordering (Item IDs 3, 4,
then 5), and conservation of all owned-item totals after cold restart without
duplicating displayed hay-block or crafting-table recipe results. This is a
local protocol-client result. Its optimized hosted deployment
`737cb031-94ce-45d0-9f15-cd3633b80b1a` reached SUCCESS and native offline
listening at 14:23:02 Europe/Warsaw, with one instance and the existing volume.
Current public gameplay remains unverified because of environment DNS failure;
these observations do not verify the later container-removal stage. Full Java player-save
ordering, integrated death/survival, mixed alive-return/overflow-drop packet
interleaving and complete tick phases remain open. These bounded menu additions
did not close a full checklist row; the current indicators are recorded above.

The [active-viewer flow](validation/open-furnace-menu-flow.json) passed for an
ordinary furnace with two viewers and an independent breaker. The old container
retains its identity and counters after its slots are dropped; a replacement at
the same coordinates does not become that container. The actual later Level
player phase broadcasts, validates and closes each old menu, with the close
packet before cursor return. Item and XP totals survived cold restart without
new replacement entity IDs. That recorded path covers alive creative players;
the newer disconnect result below has its own source and binary scope.

The [disconnected-cursor flow](validation/disconnected-furnace-flow.json) passed
locally for source `07e290cc66411d0d1ededfae69a592899a2e5d10` on 1 November
2026 at 13:55:31 Europe/Warsaw. After an actual disconnect with five quartz on a
furnace cursor, the remaining protocol client observed one real item entity.
The saved player cursor and inventory contained zero quartz, and item ID 3 with
five quartz survived cold restart exactly once. Normal live closure retains the
inventory-return branch. The optimized hosted release and native offline startup are verified separately
in the [deployment observation](validation/railway-preview.json). Public gameplay,
death handling, full-inventory overflow conformance and the complete
Java `PlayerList.remove` persistence order remain open. This bounded subitem
does not change the fixed checklist denominator or `scope_version: 1`.

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
- [x] Bounded native Anvil FallingBlock actor: real scheduling/constructor, shared index/tick/tracking, actual landing and paired atomic native entity save; [local source/runtime evidence](reference/falling-anvil-runtime.md), PR97 under review.
- [ ] Generic falling types, active callback contexts, passenger/damage profiles and mid-flight native-resume runtime proof; separate future source is not this primary proof.
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
