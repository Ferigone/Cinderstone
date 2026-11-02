# Cinderstone

A native Rust server targeting **Minecraft Java Edition 1.21** and an unchanged Java client.

Cinderstone is an early, actively developed port of vanilla server behavior. The
server process runs Rust code; Java is used only as a reference during development.
**Full vanilla compatibility is not complete.** The current playable profile is a
native flat creative world with an unmounted player and no active status effects.

The goal is faithful behavior and strong performance. There is no benchmark yet
showing a performance advantage over a Java server running equivalent gameplay.

## Native entity pressure callbacks

Player, Item, XP and Falling callbacks now share the actual pressure kernel,
entity index, sound RNG and BLOCK scheduler. An isolated full build and
[one real Item activation/pickup/release flow](docs/reference/pressure-item-runtime.md)
passed; the actual report retains a tick-overload observation and makes no timing
or performance claim. Source and bounded callback ownership are documented in
[the pressure ledger](docs/reference/falling-block-pressure-plates.md). XP/Falling,
weighted/stone plate behavior and active networks have not been runtime-certified
by that single Item flow. This feature stage does not complete a checklist row.

## Native Anvil falling runtime

The [bounded native flow](docs/validation/falling-anvil-flow.json) passed: real Anvil placement, support removal, FallingBlock entity40, landing, rename/cost1/take, menu lifetime and native restart conserved owned blocks and swords. [Source evidence and limits](docs/reference/falling-anvil-runtime.md) pin the exact executable and callback/RNG owners. [PR97](https://github.com/Ferigone/Cinderstone/pull/97) merged as `68fba409`; automatic native offline startup was observed.

The [PR100 follow-up](https://github.com/Ferigone/Cinderstone/pull/100) adds ordinary sand/red_sand/gravel, bounded actual WATER callbacks, placement obstruction and source save-before-menu-removal. [Actual local observations](docs/reference/ordinary-falling-runtime.md) include all three ordinary falls and a real-bucket water landing. The removal witness conserves persistent inventory and real temporary-item drops; its grouped harness stopped on a private copy error, so it is not a complete removal/reconfiguration result.

A separate [genuine mid-flight save and resume](docs/reference/falling-midflight-local-keys.md) passed with the same actor body, UUID and RNG across a hidden restart, followed by chunk publication, resumed motion and one landing. It used an explicit offline-loopback local public-key file; production's default key source is unchanged. Earlier incomplete attempts remain separate. Active fire/redstone/BE contexts, special falling classes, passenger graphs, survival damage and Java-save import remain open. These bounded paths do not complete an aggregate checklist row.

## Native Grindstone and Anvil integration

The Grindstone Main path passed a [bounded actual protocol flow](docs/validation/grindstone-flow-supported.json): real enchantment removal, XP orb, level event1042, item drop, menu lifetime and native cold-checkpoint conservation. The [source and limits](docs/reference/grindstone-production.md) retain the separate earlier failure at unsupported equipped-enchantment effects. The supported flow keeps an inert stone selected; this is not unrestricted enchanted-equipment support.

The earlier Anvil placement blocker is resolved by the separately published bounded Falling runtime above. Grindstone/current-menu PR84 is merged as `f19197a`; Railway `7f39646a-63ba-4218-9419-e6f7cc70005b` reached SUCCESS/native offline listening at `2026-11-02T01:19:07.247485044Z`, one running replica and the retained volume. Partial workstation stages do not complete an aggregate checklist row.

## Native Loom Main

The Loom uses actual configured banner patterns/template tags, canonical inputs,
source-phased callbacks and the existing item/sound owners. Full Main build and
[local protocol flow](docs/validation/loom-flow.json) passed: patterned banner
component49, ingredient consumption, sound1429 and close/disconnect/native restart
conserved owned items without duplicating the preview. See
[production scope](docs/reference/loom-production.md). A current-kernel recheck
also passed: stale window0 clicks do not mutate the inactive menu, and all seven
fixture stones remain owned. Source phases retain the real inventory-counter
timeline; internal counter instrumentation is not claimed. [PR81](https://github.com/Ferigone/Cinderstone/pull/81) is merged as `26c4d2e`. Railway deployment `7e85486e-40a8-4fce-89e0-78b794fa815a` reached SUCCESS and native offline listening at `2026-11-02T00:03:45.613716489Z`, with one running replica and retained volume. Public PLAY is unverified; templates/custom and broader runtime paths remain open.

## Native Smithing Main

The three-input Smithing controller is connected to actual Main owners. A full
Main build and [local protocol flow](docs/validation/smithing-flow.json) passed:
netherite-sword transformation, actual level event1044, crafted statistics,
current-menu close, disconnect and native cold restart preserved owned items.
[Production scope](docs/reference/smithing-production.md) separates that observed
path from unexercised trim, custom component, pickup and range contexts. Its
[PR78](https://github.com/Ferigone/Cinderstone/pull/78) merged as `005e759`.
Railway deployment `138fde42-f5ce-467f-8970-b306eaa21c3e` reached SUCCESS and
native offline listening at `2026-11-01T22:24:46.700522705Z`, one running replica,
retained volume and no pending work. Public gameplay remains unverified.

## Native Stonecutter Main

The native Stonecutter now uses the real shared player inventory, recipe book,
statistics, ItemEntity/drop and per-player menu owners. Normal crafting,
quick-move, current-menu close, menu replacement, disconnect and native cold
restart passed a focused local protocol flow. The composed master candidate also
passed the same flow and the equipped-range retirement regression.
See [production scope and exact evidence](docs/reference/stonecutter-production.md).
Custom component reference identity, custom interaction ranges and arbitrary
reentrant callbacks remain unsupported; this is a bounded workstation delivery.
Railway deployment `f1add0f5-552d-41f5-9dd6-e1aa36a1b201` for master
`dddc0e7` reached SUCCESS and native offline listening at
`2026-11-01T22:05:31.290410832Z`, with one running replica and the existing volume.
The external status probe still fails DNS resolution; public gameplay remains
unverified. Smithing Main has separately passed its local flow. The fixed scope1 checklist remains **56/100 code**, **51/100 Main**.

## Native connection transport

Published source `87dda91` connects the configured packet-rate owner,
thirty-second raw read inactivity, shared common-packet decoding and
configurable compression to Main. The [focused local flow](docs/validation/connection-flow.json)
passed on local source `259e641`; its Git tree is identical to published `87dda91`.
Actual rate refusal, compression enabled/disabled, keepalive refusal, signed
long ping echo and incomplete-frame timeout refresh were observed.
See [production contracts and limits](docs/reference/connection-production.md).

The earlier connection release autodeployed master `1e72694` successfully and the native process began
listening in offline mode on 1 November 2026 at **21:19:15 Europe/Warsaw**.
One replica and the existing `/data` volume are retained. Public gameplay remains
unverified because this environment cannot resolve the external TCP hostname.
The fixed checklist remains **56/100 code** and **51/100 Main** at `scope_version: 1`.

## Native chat preview

Native player chat now uses actual session, signature, acknowledgment and
recipient owners. The [local chat flow](docs/validation/chat-flow.json) passed for
source `b3a0d1d`: Unicode messages, chat visibility, rejection and spam behavior,
and offline empty-signature command routing. A separate
[generated-key core smoke](docs/validation/chat-crypto-smoke.json) verifies native
cryptographic paths without claiming Mojang-issued trust or a public real-account
session. See [production scope](docs/reference/chat-production.md) for remaining
limits. The fixed checklist remains **56/100 code**, **51/100 Main**, scope1.

## Native administration preview

The server now connects native access lists, ordered login/configuration
admission, console and bounded unsigned player administration, operator command
trees and configured player capacity to actual live owners. The
[local admission flow](docs/validation/admission-flow.json) passed for source
`6c54018`; [configuration and limits](docs/reference/admission-production.md)
state the exact scope. Complete signed-command contexts, full profile/selector/parser compatibility
and remaining login transport behavior are still incomplete. This bounded stage
does not add a fixed-checklist point; indicators remain **56/100 code** and
**51/100 Main** at `scope_version: 1`.

## Compatibility

| Target | Version |
| --- | --- |
| Minecraft Java Edition | **1.21** |
| Network protocol | **767** |
| World/player DataVersion | **3953** |
| Server runtime | Native Rust; no JVM required |
| Client modifications | None required for the implemented protocol |

Matching a protocol number alone does not establish support for other Minecraft
versions. The behavior reference remains the original **1.21** server.

## Current scope

The compiled server connects these systems to its main runtime:

- Handshake, status, ping, compression, configuration registries, PLAY,
  keepalive, player tracking and inventory synchronization.
- Native PLAY-to-CONFIGURATION return and actual player readmission on the same
  connection, cookie-response rules and configured resource-pack task handling.
- Online-mode authentication with RSA, AES and the official session service.
  A real-account public-server login still needs end-to-end verification.
- Native player movement/body ticks, collision and pose handling, creative
  flight, water/lava movement, food state and supported food/drink use.
- Creative block editing, structural updates, doors, trapdoors, fence gates,
  buckets, scheduled water/lava, fire, live lighting and supported random ticks.
- Player inventory and 2×2 crafting; a native 3×3 crafting-table menu with recipe
  remainders, shift crafting and real shared-inventory transactions.
- Single/double chests and barrels with owned contents, menu interactions,
  viewers, sounds and native persistence.
- A bounded furnace, smoker and blast-furnace family: captured vanilla recipes
  and fuels, processing ticks, three-slot menus, cooking/burn counters, lit-state
  changes and native persistence.
- Furnace reward experience through real orb creation, shared ticking, pickup
  and tracking for the supported native player profile.
- One BLOCK tick queue shared by fire, supported redstone and container rechecks.
  Basic redstone includes levers, buttons, lamps, repeaters and torches.
- Native player, statistics, recipe-book, world and dropped-item persistence;
  bounded Java `player.dat` import/export for DataVersion 3953.

The [focused configuration flow](docs/validation/configuration-flow.json) passed
for source `643106e8eb60d970a8df67eb9acf0ab5f107c788` after a full Main build.
An actual native SIGHUP request returned an admitted PLAY connection through
CONFIGURATION and readmitted a new player on the same TCP connection without
reauthentication. Eleven emeralds, moved position and subsequent world
interaction were retained. The flow also covered initial known-pack negotiation
and brand order, cookie rejection in LOGIN/CONFIGURATION/PLAY, and optional and
required resource-pack response rules. Resource URLs were not fetched; this is
a local protocol-client result, not graphical-client or public gameplay proof.
The optimized hosted release reached **SUCCESS**, with one instance, the existing
volume and native offline listening at **15:25:08 Europe/Warsaw** on 1 November
2026. Public gameplay remains unverified because the external status probe fails
DNS resolution. Dynamic
registry reload, the general component codec and every malformed late-PLAY
decoder remain unfinished. See [current work](docs/CURRENT_WORK.md) for the exact
binary, scope and remaining limits.

The furnace Main integration passed a bounded local protocol-client flow covering
all three cooking types, menu transfers, lit/light updates, real XP pickup and
save/cold restart. The same binary uses the normal 20 TPS deadline and overload
clock. See the [flow report](docs/validation/furnace-preview-flow.json),
[current work](docs/CURRENT_WORK.md) and
[furnace source ledger](docs/reference/furnaces.md). This does not establish full
vanilla compatibility or a graphical-client check. The
[deployment observation](docs/validation/railway-preview.json) records the hosted
release build and offline startup separately.

The current removal implementation adds bounded creative destruction of native
furnaces, smokers and blast furnaces, staging their actual contents and stored
recipe XP in one entity/random transaction. It does not drop the furnace block
itself in creative mode. The [focused local removal flow](docs/validation/furnace-removal-flow.json)
passed for all three furnace types, actual item/XP entities and native cold
restart with item and experience conservation. This result applies to its
recorded binary; hosted deployment verification is separate. The newer
[open-menu flow](docs/validation/open-furnace-menu-flow.json) passed for an
ordinary furnace with two viewers during creative destruction. It preserves the
removed container until later player-phase closure, returns cursor contents once
and keeps a same-position replacement independent. Item and XP totals survived
cold restart. That recorded flow covers the alive-player inventory-return branch;
full-inventory overflow is not covered by it.

The newer [disconnected-cursor flow](docs/validation/disconnected-furnace-flow.json)
passed locally for source `07e290cc66411d0d1ededfae69a592899a2e5d10`.
With two protocol clients, disconnecting a player carrying five quartz in a
furnace menu created one real item entity observed by the remaining client.
The saved cursor and inventory contained no quartz, and the same item ID and
five quartz survived cold restart exactly once. Normal live menu closure still
returns the cursor to inventory. Its optimized hosted release and native offline startup are verified in the
[deployment observation](docs/validation/railway-preview.json); public gameplay
remains unverified. Player-death handling, full-inventory overflow conformance and the
complete Java `PlayerList.remove` persistence order remain incomplete.

The newer menu stage extends explicit close contexts to chest/barrel menus, the 3×3
crafting table and window0. Disconnect cleanup drops their owned cursors and
temporary crafting inputs, with window0 removal before the external menu.
Opening a block menu from window0 now retains window0's own cursor and 2×2 grid.
Alive returns use raw inventory insertion packets; external closes preserve
shared-slot synchronization baselines and publish the close packet first.
The full Main build and [focused local flow](docs/validation/disconnected-menus-flow.json)
passed for source `93438c6055ec755e85f528739a41073ef5563b5a`: all four
disconnect families, ordinary live chest return, retained window0 drop order and
item conservation through cold restart, without duplicating recipe previews.
Its optimized hosted release reached SUCCESS with native offline listening at
14:23:02 Europe/Warsaw on 1 November 2026, one running instance and the existing
persistent volume. Public gameplay remains unverified because the external
status probe fails environment DNS resolution.
See the [menu-close source ledger](docs/reference/menu-close-lifecycle.md).
Full player-save ordering, death/survival and mixed alive-return/overflow-drop
packet interleaving remain unfinished. Current fixed-checklist indicators are
56/100 code and 51/100 Main integration at `scope_version: 1`.

The newer bounded stage implements creative removal
of a **closed single dry chest or closed barrel**. It prepares all 27 owned
slots, actual item entities, level RNG and shared entity IDs using an opaque
container identity and a privately drained staging clone, with native source
history linked to the final world transition. This path creates neither XP nor
the container block's creative loot. Actual open menus, double/waterlogged
chests, unresolved loot and comparator receivers remain rejected boundaries.
The full Main build and [focused local removal flow](docs/validation/container-removal-flow.json)
passed for source `be2582a9ea032205c3885fd04e10ff1419d63fb9`: the sword guard
preserved contents, both closed block types produced real Items and became air,
and 37 dirt, two coal, 33 quartz and seven emeralds survived cold restart exactly.
The optimized hosted release reached SUCCESS and native offline startup on
1 November at 14:31:18 Europe/Warsaw, with one instance and the existing volume.
Public gameplay remains unverified because the external status probe fails DNS.
See the [deployment observation](docs/validation/railway-preview.json) and the
[container-removal source ledger](docs/reference/container-removal.md).

Comparator receivers, Mending repairs, custom datapacks and survival furnace
breaking remain unsupported. Menu actions combining output XP with item drops
are still rejected before commit. The recipe catalog represents the captured
default vanilla data.

These features have explicit limits. The full survival damage/death/respawn
pipeline, mobs and AI, complete terrain generation, all menus and block entities,
wire topology, observer lifecycle, comparators, pistons, TNT/explosions, vehicles,
portals, commands and datapack reload remain unfinished. A data codec or block ID
is not an implementation of its gameplay. Unsupported runtime owners are rejected
explicitly rather than replaced with invented state.

See the English [roadmap](docs/ROADMAP.md). The detailed
[compatibility ledger](docs/PARITY.md) includes historical evidence and is partly
in Polish; old results describe their original source and binary versions.

## Build and run

Build requirements are Rust/Cargo and the native C build tools required by the
vendored OpenSSL dependency, including a C compiler, make and Perl. Java, Node.js
and Python are **not required to run the Rust server**.

```bash
git clone https://github.com/Ferigone/Cinderstone.git
cd Cinderstone
cargo build --locked --release --bin mc121-rust
```

Run from the repository root so the captured protocol fixtures are available.
The executable is currently named `mc121-rust`.

For a local offline development session:

```bash
./target/release/mc121-rust --allow-offline
```

Connect with Minecraft Java **1.21** to `127.0.0.1:25565`. `--allow-offline`
is restricted to a loopback bind address unless `--allow-public-offline` is
explicitly added.

For an authenticated server reachable over TCP:

```bash
./target/release/mc121-rust \
  --online-mode \
  --bind 0.0.0.0 \
  --port 25565 \
  --world runtime/world.json
```

For a public server that accepts offline accounts:

```bash
./target/release/mc121-rust \
  --allow-offline --allow-public-offline \
  --bind 0.0.0.0 --world runtime/world.json
```

Public preview: **`hopper.proxy.rlwy.net:24418`**, using an unchanged Minecraft
Java **1.21** client. The preview accepts offline accounts: a Mojang session or
valid Java account is not required. Railway keeps the world on persistent storage
and deploys updates from `master`. See
[operational status](docs/CURRENT_WORK.md) for deployment verification.

Offline mode does not authenticate nicknames. Player UUIDs are derived from the
name and differ from online-account UUIDs, so existing online player data is not
automatically transferred when the login mode changes.

World changes, player data and native entity checkpoints must be kept together
for restart. Keep the world path on persistent storage when running a container.
`--online-mode` and `--allow-offline` are mutually exclusive.

Resource-pack offers are configured at startup through these CLI options and
container-entrypoint variables:

| CLI option | Container variable |
| --- | --- |
| `--resource-pack-url` | `MC_RESOURCE_PACK_URL` |
| `--resource-pack-sha1` | `MC_RESOURCE_PACK_SHA1` |
| `--resource-pack-id` | `MC_RESOURCE_PACK_ID` |
| `--resource-pack-prompt` | `MC_RESOURCE_PACK_PROMPT` |
| `--require-resource-pack` | `MC_RESOURCE_PACK_REQUIRED=true` |

The prompt is supplied as JSON within the implemented component profile.
Malformed JSON syntax is warned about and omitted. Requiring a pack without a
configured URL does not create an offer or an effective pack requirement. The
server offers the URL; the local validation does not download or apply a pack.
On Unix, SIGHUP requests the native operator `RETURN_TO_WORLD` transition for
admitted connections. It does not implement datapack or dynamic-registry reload.

Existing Java chunks can be opened with `--anvil-world /path/to/world`. This writes
to that directory, so use a copy while evaluating the port. Imported chunks do not
acquire missing runtime block/entity owners. Missing chunks return
`NotGenerated`; a complete generator is not available yet.

## Progress tracking

The fixed planning checklist has **100 named, unweighted delivery items** at
`scope_version: 1`:

| Checklist indicator | Recorded implementation scope |
| --- | ---: |
| Native code deliveries | **56/100** |
| Deliveries connected to the main server | **51/100** |

These are **planning-checklist indicators**, not percentages of Minecraft's
functional compatibility, engineering effort or remaining time. Partial features
can be implemented without completing a broader checklist item. The machine
readable source of truth is [`progress/checklist.json`](progress/checklist.json).
The denominator and definition change only with an explicit scope-version change.
The increment closes the existing `network.5` delivery; it does not redefine
other rows or establish a percentage of full Minecraft compatibility.

## Development

Porting and connecting real mechanics comes first. Focused compiler/runtime checks
help catch integration failures; broad compatibility comparison remains a separate
final stage. Test counts are not used to measure progress.

Independent Codex sessions use the [coordination guide](docs/coordination/README.md)
and its ready-to-copy team prompts. The real integration queue is
[issue #1](https://github.com/Ferigone/Cinderstone/issues/1), with assigned teams
for [world generation #2](https://github.com/Ferigone/Cinderstone/issues/2),
[workstations #3](https://github.com/Ferigone/Cinderstone/issues/3) and
[entities/survival #4](https://github.com/Ferigone/Cinderstone/issues/4).
Where issue comments are rejected with HTTP 403, the guide defines versioned
status files on each team's feature branch as the communication fallback.
Prompts and assigned issues do not establish that an independent session is
running; each session reports its observed agent capacity and actual work.

The implementation is guided by the original shipped **1.21** server artifact,
official mappings, packaged data and observed behavior. Artifact identities are
pinned in [`fixtures/source-lock.json`](fixtures/source-lock.json). Java 21 is
needed only for reference execution and extraction; it is not a server runtime
dependency. REA assists artifact inspection; it does not automatically translate
JVM code into Rust.

For reference tooling and existing checks:

```bash
cargo check --locked --bin mc121-rust
```

Reference tools are under `tools/`. A historical
stock graphical-client connection demonstrates that specific build's limited
interoperability; it does not validate the current reconstructed build or all
vanilla gameplay.

When contributing a mechanic, connect its actual world, inventory, RNG, scheduled
callback and persistence owners. Preserve observable ordering, document unsupported
paths and keep performance claims tied to equivalent workloads.

Cinderstone is an independent project and is not affiliated with Mojang or Microsoft.
