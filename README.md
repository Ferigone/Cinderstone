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
- One BLOCK tick queue shared by fire, supported redstone and container rechecks.
  Basic redstone includes levers, buttons, lamps, repeaters and torches.
- Native player, statistics, recipe-book, world and dropped-item persistence;
  bounded Java `player.dat` import/export for DataVersion 3953.

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

Connect with Minecraft Java **1.21** to `127.0.0.1:25565`. Offline mode is
restricted to a loopback bind address.

For an authenticated server reachable over TCP:

```bash
./target/release/mc121-rust \
  --online-mode \
  --bind 0.0.0.0 \
  --port 25565 \
  --world runtime/world.json
```

Remote players need a valid Minecraft Java account and an unchanged **1.21**
client. The public Railway endpoint is pending deployment; no live address is
published here yet.

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
readable source of truth is [`mc121-rust-progress.json`](progress/checklist.json).
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

Further tools are under `tools/`, with evidence under `evidence/`. A historical
stock graphical-client connection demonstrates that specific build's limited
interoperability; it does not validate the current reconstructed build or all
vanilla gameplay.

When contributing a mechanic, connect its actual world, inventory, RNG, scheduled
callback and persistence owners. Preserve observable ordering, document unsupported
paths and keep performance claims tied to equivalent workloads.

Cinderstone is an independent project and is not affiliated with Mojang or Microsoft.
