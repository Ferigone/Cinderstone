# Boss bars — `BossEvent`, `ServerBossEvent`, `CustomBossEvent(s)` and `ClientboundBossEventPacket`

Target: Minecraft Java **1.21**, protocol **767**, DataVersion **3953**. Primary evidence is the
unchanged original `CustomBossEvents`, `CustomBossEvent`, `ServerBossEvent`, `BossEvent` and
`ClientboundBossEventPacket`, run through the local oracle (`tools/loot_oracle.java bossbar` and
`bossbar-mutants`) with stub players whose connections record every frame the original sends. Pinned
artifacts: bundler SHA-1 `450698d1863ab5180c25d7c804ef0fe6369dd1ba`, inner server jar SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`, official mappings SHA-1
`31c77994d96f05ba25a870ada70f47f315330437` ([`fixtures/source-lock.json`](../../fixtures/source-lock.json)).
Java artifacts and decompiled text stay outside Git.

Scope: `src/bossbar/**`, work package WP-G2 **stage 1** (the `CustomBossEvents` owner of `level.dat`,
delivery `world_storage.4`, **partial**), written so `/bossbar` (WP-E3) and the Wither / Ender Dragon
(WP-M5) can reuse the same model. It claims **no** checklist point and **no** Main integration:
nothing in `src/` outside the module calls it, and `src/level.rs` is untouched.

## What is implemented

| Java | Rust |
| --- | --- |
| `BossEvent.BossBarColor`, `BossBarOverlay` (`byName`, `getFormatting`) | `bossbar::{BossBarColor, BossBarOverlay}` |
| `BossEvent` + `ServerBossEvent` (state, visibility, the player set, broadcasting) | `bossbar::ServerBossEvent` (changes queue `(player, packet)` pairs; `take_outbox`) |
| `ClientboundBossEventPacket` (six operations) | `bossbar::packet::{BossPacket, BossOperation}` (`body`, `frame`, packet id `0x0A`) |
| `CustomBossEvent` (`setValue`, `setMax`, `setPlayers`, `onPlayerConnect/Disconnect`, `save`, `load`) | `bossbar::CustomBossEvent` |
| `CustomBossEvents` (`create`, `get`, `remove`, `getIds`, `save`, `load`, connect/disconnect) | `bossbar::CustomBossEvents` |
| `HashMap<ResourceLocation, _>` / `HashSet<UUID>` iteration order | `saved_data::java_collections::{JavaHashMap, JavaHashSet}` with `JavaHash` for `JavaString`, `[u8; 16]` UUIDs and `ResourceId` |

The caller owns the players and their connections: it passes player UUIDs in and sends the
packets it takes out of the event. The original draws each event's UUID at random
(`Mth.createInsecureUUID`); the port takes it as an argument (`CustomBossEvents::load` takes a
function that supplies one per loaded event).

Behaviours the oracle showed (each pinned by scenarios or mutants):

- **Progress is `Mth.clamp((float) value / max, 0, 1)` with Java's NaN rules.** `0 / 0` is a NaN
  whose *bits* are on the wire (`0xFFC00000` on x86, the sign bit set; a canonical NaN differs), a
  positive value over zero is clamped to 1, a negative one to 0, and `setProgress` stores and
  broadcasts any value that is `!=` the old one, which a NaN always is: every `setValue` while the
  progress is NaN sends another update packet.
- **Nothing is sent while the bar is hidden.** `addPlayer`/`removePlayer` send the add/remove packet
  only when visible, `setVisible` sends add or remove to every current player, the property
  setters broadcast only when visible and only when the value changes (`setName` compares components
  with `equals`, so `"Hello"` and `{"text":"Hello"}` are equal and send nothing).
- **A custom event keeps two player sets.** The `ServerBossEvent` set holds the connected players it
  sends to; the `HashSet<UUID>` holds everyone who belongs, online or not, and is what is saved.
  `removePlayer` forgets the UUID, `onPlayerDisconnect` does not, `onPlayerConnect` adds a
  remembered UUID again, `addOfflinePlayer` only remembers, `removeAllPlayers` clears both.
  `setPlayers(online)` compares by UUID: remembered UUIDs not online leave, online players not
  remembered join, and a player whose UUID is remembered but not connected is **not** added (it needs
  `onPlayerConnect`); it returns whether anything changed.
- **Saved form.** `Name` is `Component.Serializer.toJson` text (Gson, compact), `Visible` /
  `DarkenScreen` / `PlayBossMusic` / `CreateWorldFog` are bytes, `Value` / `Max` ints, `Color` /
  `Overlay` the lower-case names, `Players` a list of 4-int arrays in `HashSet<UUID>` order (an empty
  list is typed "end"); the compound's own member order is `HashMap` order. `CustomBossEvents.save`
  is a fresh compound keyed by `namespace:path`, so its order comes from a table sized for the events
  present.
- **Loading** reads the events in the iteration order of the saved compound; a key that is not a valid
  resource location, a `Name` that Gson/the component codec rejects (including a missing one: the empty
  string is JSON `null`), or a `Players` entry that is not four ints throws, and the events loaded
  before it stay (`CustomBossEvents::load` returns the error and keeps them). `Visible` and the other
  booleans default to false when absent (a loaded bar is **hidden** unless the tag says otherwise),
  `Value` is applied before `Max` (the progress ends up the same), `Color`/`Overlay` fall back to white
  / progress, a `Players` list of another element type reads as empty, and a non-compound member of
  the events compound is an empty compound (so its `Name` fails).
- **UUID order.** `UUID.hashCode()` folds `mostSigBits ^ leastSigBits`; players that collide in the
  16-bucket table keep insertion order, so the order in the saved list depends on the order players
  were added, including the order `setPlayers` walks the new players (a hash set of `ServerPlayer`s,
  ascending entity id below the table size; `set_players` documents that its argument must be in that
  order).

## Evidence

`tools/bossbar_generate.py` records, per step, the state asked for, the saved NBT and the frames each
player was sent.

- `fixtures/bossbar-vectors.json`: a sample (46 scenarios; the full set is **146 scenarios with 5,145
  steps and 2,844 frames**, regenerable with `--full-out`): visibility transitions, progress arithmetic
  with zero, negative and extreme maxima and NaN/infinite/negative-zero progress, name equality, colour
  and overlay changes (including unknown names), the three flags, players added, removed and replaced,
  remembered UUIDs, connect/disconnect, up to ten remembered UUIDs (several collide in one bucket),
  several events (ids in `HashMap` order, replacement, removal), save-and-reload, all six packets for ten
  component names, and 120 random sequences of 10–60 steps over up to three events and ten players.
  Every `save` is compared as the NBT bytes (member order included) and every player's frames byte for
  byte; the order of frames *across* players is that of an identity hash set in the original and is
  not compared.
- 103 of **617 one-step mutants** (kept 1 in 6; 539 loaded, 78 rejected by the original: 76 name parse
  failures, 2 bad UUID entries) of three saved bars: members removed, every node replaced by wrong-typed
  tags. For each the loaded state and the saved NBT must match.
- **All agree with zero disagreements.** Six deliberate mutations of the port (broadcast while hidden,
  a canonical NaN, UUID kept on removal, a UUID hash that ignores half of the UUID, `setName`
  always broadcasting, disconnect forgetting the UUID) were each caught; a seventh, swapping the order of
  `setValue` and `setMax` on load, is an equivalent mutant (both end with the same progress and there is
  no player to tell).

`cargo test --locked --lib bossbar::` runs the fixture in under a second. The full set:
`BOSSBAR_FILE=<json> cargo test --lib bossbar::tests::explore -- --ignored`.

## Not implemented (explicit)

- The `/bossbar` command, any boss mob, and the player-facing flow (`ServerPlayer` lifecycle, join/
  leave hooks): the caller supplies UUIDs and sends the queued packets.
- The other fields of `PrimaryLevelData` (`CustomBossEvents` is read and written by the caller from the
  `level.dat` compound; this module does not touch `src/level.rs`).
- `CustomBossEvent.getDisplayName` (needs `ComponentUtils.wrapInSquareBrackets` and styles).
- Treeified `HashMap` bins (eight keys in one bucket of a table of at least 64 buckets).
- Fractional `Value`/`Max` tags beyond `getInt` (any numeric tag is converted as `getInt` does).

## Reproducing the oracle

```bash
python3 tools/loot_oracle_setup.py --oracle-dir /path/outside/git      # JDK 21 required
python3 tools/bossbar_generate.py --oracle-dir /path/outside/git --sample 6 [--full-out FILE]
cargo test --locked --lib bossbar::
```
