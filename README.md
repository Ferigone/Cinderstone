# Cinderstone

A native Rust server targeting **Minecraft Java Edition 1.21** and an unchanged Java client.

Cinderstone is an early, actively developed port of vanilla server behavior. The
server process runs Rust code; Java is used only as a reference during development.
**Full vanilla compatibility is not complete.** The current playable profile is a
native flat creative world with an unmounted player and no active status effects.

The goal is faithful behavior and strong performance. There is no benchmark yet
showing a performance advantage over a Java server running equivalent gameplay.

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
packet interleaving remain unfinished. The fixed checklist stays 55/100 code
and 50/100 Main integration at `scope_version: 1`.

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

Existing Java chunks can be opened with `--anvil-world /path/to/world`. This writes
to that directory, so use a copy while evaluating the port. Imported chunks do not
acquire missing runtime block/entity owners. Missing chunks return
`NotGenerated`; a complete generator is not available yet.

## Progress tracking

The fixed planning checklist has **100 named, unweighted delivery items** at
`scope_version: 1`:

| Checklist indicator | Recorded implementation scope |
| --- | ---: |
| Native code deliveries | **55/100** |
| Deliveries connected to the main server | **50/100** |

These are **planning-checklist indicators**, not percentages of Minecraft's
functional compatibility, engineering effort or remaining time. Partial features
can be implemented without completing a broader checklist item. The machine
readable source of truth is [`progress/checklist.json`](progress/checklist.json).
The denominator and definition change only with an explicit scope-version change.

## Development

Porting and connecting real mechanics comes first. Focused compiler/runtime checks
help catch integration failures; broad compatibility comparison remains a separate
final stage. Test counts are not used to measure progress.

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
