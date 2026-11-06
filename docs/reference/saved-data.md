# Saved data — `DimensionDataStorage` and the first owners

Target: Minecraft Java **1.21**, protocol **767**, `DataVersion` **3953**. Primary evidence is the
unchanged original `DimensionDataStorage`, `SavedData`, `RandomSequences` and `MapIndex` run
through the local oracle (`tools/loot_oracle.java saved-data` and `saved-data-mutants`) over a
real temporary directory. Pinned artifacts: bundler SHA-1 `450698d1863ab5180c25d7c804ef0fe6369dd1ba`,
inner server jar SHA-256 `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`,
official mappings SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`
([`fixtures/source-lock.json`](../../fixtures/source-lock.json)). Java artifacts and decompiled
text stay outside Git.

Scope: `src/saved_data/**`, work package WP-G2 stage 2 and stage-3 owners (`idcounts`, `random_sequences`,
`command_storage_*`, `chunks`; the scoreboard owner is in [scoreboard.md](scoreboard.md)) (delivery
`world_storage.4`, **partial**). It claims **no** checklist point and **no** Main integration:
nothing in `src/` outside the module calls it, and `src/level.rs`, `src/world_rules.rs` and
`src/storage.rs` are untouched.

## What is implemented

| Java | Rust | Notes |
| --- | --- | --- |
| `DimensionDataStorage` | `saved_data::DimensionDataStorage` | `get`, `compute_if_absent`, `set`, `save`, `read_tag_from_disk` |
| `SavedData`, `SavedData.Factory` | `SavedData` trait, `Factory<T>` | dirty flag, payload `save()`, constructor + deserializer + data-fix type |
| `NbtIo.readCompressed` / `read`, `writeCompressed` | `saved_data::file` | gzip when the first bytes are `1f 8b`, plain NBT otherwise; gzip on write |
| `NbtUtils.getDataVersion`, `addCurrentDataVersion` | `file::data_version`, `DATA_VERSION` | default 1343 when the file has no numeric `DataVersion` |
| `DataFixTypes.update` | `DataFixType::update` | exact for the two owned types (see below) |
| `MapIndex` (`idcounts`) | `saved_data::MapIndex` | `getFreeAuxValueForMap`, `load` with `getInt` conversion |
| `RandomSequences` (`random_sequences`) | `loot::sequences::RandomSequences` as a `SavedData`, `random_sequences_factory(seed)` | the type is the one merged with the loot port |
| `CommandStorage`, `CommandStorage.Container` (`command_storage_<namespace>`) | `saved_data::{CommandStorage, CommandStorageContainer}` | `get`, `set`, `keys`; the `HashMap` order of namespaces and paths; an empty compound removes an entry |
| `ForcedChunksSavedData` (`chunks`) | `saved_data::ForcedChunks` | the `Forced` long array in fastutil `LongOpenHashSet` order; `set_forced` is `ServerLevel.setChunkForced` |
| `java.util.HashMap<String, V>`, `LongOpenHashSet`, the `HashMap` order of a `CompoundTag` | `saved_data::java_collections::{JavaHashMap, LongOpenHashSet, to_java_order}` | table doubling and no shrinking; open addressing with back-shift deletion, rehash and shrink |

Behaviours the original showed (each pinned by a scenario):

- **A new object is not dirty.** `computeIfAbsent` constructs and caches it; nothing is written
  until something marks it dirty. A fresh `idcounts` is written only after the first map id.
- **Absence is cached.** A missing, unreadable or undeserializable file is cached as `null`;
  the error is only logged. A file that appears later is not seen until `set` or a new storage.
  An unreadable file is never overwritten unless the replacement object becomes dirty.
- **Reading.** A one-byte, empty, non-compound-root or truncated file is unreadable. A gzip
  trailer that is damaged *after* the NBT root is complete goes unnoticed (the Java reader is
  lazy), a damaged stream before that point does not; a second concatenated gzip member and
  bytes after the root are ignored; the root name is ignored; a missing `data` member is an
  empty compound.
- **Unknown files are never rewritten**: only cached dirty objects are saved.
- **Data fixers.** Inspecting `DataFixers` shows no fixer for `idcounts`, and one for
  `random_sequences`: `RandomSequenceSettingsFix` at version 3565 wraps the old `data` in
  `{sequences: data}` for files with `DataVersion < 3565`. Newer data is returned unchanged.
  `DataFixType::update` implements exactly that; any other saved-data type will need the data
  fixers of WP-G5 (the `scoreboard` type, added with the scoreboard model, accepts data already at the
  current version and nothing older), and an object that needs a fixer this library lacks is reported as
  `SavedDataError::DataFixerRequired` instead of being cached as absent.
- **`RandomSequences.load`.** `salt` is `getInt` (any numeric tag), the flags are read only when
  they are bytes, a sequence needs a `source` that `Codec.LONG_STREAM` reads as exactly two
  numbers (a long array, an int/byte array or a list of numerics, with `{"": x}` wrappers
  unwrapped) and an id `ResourceLocation.parse` accepts, where a bare name gains `minecraft:`;
  an all-zero state is replaced by the golden-ratio default. Entries are visited in `HashMap` key
  order. `reset` and `clear` do **not** mark the data dirty (only a draw does).
- **`MapIndex`.** Every numeric member is kept through `getInt` (a long truncates, a float floors),
  the first id handed out is 0, and the counter wraps like a Java `int`.
- **`CommandStorage`.** `get` returns an empty compound for a missing container or path; `set`
  creates the container on demand (not dirty until `put`, which always marks it dirty, even when it
  removes a path or the path was absent) and an empty compound removes the entry. `keys()` walks the
  namespaces *this instance* has loaded or created, in `HashMap` order, and within each the container's
  paths in `HashMap` order; a stored path that is not a valid resource-location path (`Bad`, a file edited by
  hand) makes the whole call throw. `load` stores every member of `contents` (a member that is not a
  compound becomes an empty compound), visiting them in the order the `contents` compound iterates.
  `save` fills a *fresh* `CompoundTag` in the container's iteration order, so the written order comes
  from a table sized for the entries now present, not for the ones once removed: a container that
  grew and shrank iterates in one order and is written in another. No data fixer exists for the type.
- **`ForcedChunksSavedData`.** `load` is `new LongOpenHashSet(getLongArray("Forced"))` (a missing or
  non-long-array member is empty; the table is sized for the array length, duplicates collapse). The
  array is written in `toLongArray()` order: the zero key (chunk 0,0) first, then the table from the
  last slot down; adding grows the table at 75% load, removing back-shifts the following keys and
  halves the table when the set is below a quarter of the load limit (never below the size it started
  with, and never below 16). No data fixer exists for the type.

## Evidence

`tools/saved_data_generate.py` records the original's result for every step of each scenario and
for every mutant.

- `fixtures/saved-data-vectors.json`: **60 scenarios, 195 steps**: lazy load, cache-of-absence,
  dirty tracking and the files `save` leaves alone, plain and damaged files (truncated stream,
  damaged and missing trailer, wrong CRC, two members, trailing bytes, wrong root, named root,
  missing end byte, missing `data`), data versions 1343, 3564, 3565, 3953, 4000, 99999 and none
  for both owned types, ids and draws, resets, clears and defaults of the random sequences.
  `put_snbt` steps record the file bytes the original wrote, so the Rust test writes identical files.
  After every `save` the directory listing, the `rewritten` and `gzip` flags and the decoded NBT
  of each file are compared.
- `fixtures/saved-data-mutants.json`: **646 one-step mutants** (members removed, each node
  replaced by 19 wrong-typed tags) of seven owner payloads, loaded through the original factories;
  the saved form after loading must match. This found four real differences in the already merged
  `RandomSequences` (numeric `salt`, id canonicalization, list-of-number sources, and `reset`/`clear`
  wrongly marking the data dirty); all are fixed and covered.

Compound member order is compared as a map: Java's order is a `HashMap` order that depends on
insertion, and no reader depends on it. Compressed bytes are **not** compared (the deflaters
differ). In the other direction, a one-off check (not a committed fixture) fed six files written by
this library (`idcounts` and `random_sequences` for three world seeds, after ids and draws) to the
original `DimensionDataStorage`: all six loaded, and the payload the original re-saved was equal
to this library's, member order aside.

### Command storage and forced chunks

`tools/saved_data_owners_generate.py` (oracle ops `cs_get`, `cs_set`, `cs_keys`, `fc_apply`, and the
`raw` stream of every saved file) adds `fixtures/saved-data-owners-vectors.json`: **162 scenarios
and 426 mutants generated** (66 scenarios and 86 mutants committed; `--full-out` keeps all), with
the original's answer for every step and the NBT stream of every file it wrote **byte for byte,
member order included**. They cover: files put on disk before the first access (plain, gzip, old
data versions, non-compound members, an empty `contents`, uppercase paths that make `keys()` throw),
random `set`/`get`/`keys`/`save` sequences over up to four namespaces and 55 paths with removals (the table
grows to 128 buckets and keeps it), reloading a written file, `chunks` add/remove sequences up to 325
positions (several rehashes and the shrink after most are removed), positions that collide in the
table, `get`/`compute`/`set_dirty` of `chunks`, `read_tag` at versions 1 … 4000 for both types, and 426
one-step mutants of `chunks.dat` / `command_storage_ns.dat` payloads. **All agree with zero
disagreements.** Five deliberate mutations of the port were each caught: no shrink on removal, no table
growth, the table resetting on removal, an ascending `toLongArray`, and `save` writing the in-memory order.

`cargo test --locked --lib saved_data` runs all of it in about a second.

## Deliberate differences from the original

- **Atomic replace.** The original truncates `<name>.dat` in place. This library writes
  `<name>.dat_new`, syncs it and renames it over the target, so a crash leaves the old or the new
  file. The resulting file is the same.
- **A failed write keeps the object dirty** (Java clears the flag whatever happens), so the next
  `save` retries; the failure is in `diagnostics()`.
- **The `data` folder is not created**; a missing folder is a logged write failure as in Java.
- A name that is empty, `.`, `..`, or contains a path separator or NUL is rejected (`a..b` is fine).
- `CommandStorage` registers a namespace the first time the instance obtains its container; the
  original registers it inside the factory lambda, which also runs when another `CommandStorage`
  over the same `DimensionDataStorage` loaded the file first. A server has one instance.
- A `HashMap` bin is never treeified here (eight keys in one bucket of a table of at least 64 buckets).
- Values stored by `CommandStorage::set` keep the member order the caller built them with (the
  crate builds compounds in `HashMap` order); values read from a file are re-ordered with
  `to_java_order`, as the original's `HashMap` would iterate them.

## Not implemented

`level.dat` and `PrimaryLevelData` field owners (stage 1), per-dimension storage (needs WP-G1),
save ordering and `saveAllChunks` (stage 4), the raid, map-data and structure-index owners, DataFixers
for any other type (WP-G5), the 2 MiB read quota
of an uncompressed `NbtIo.read` (this library bounds reads with the crate's NBT limits), and any
Main wiring.

## Reproducing the oracle

```bash
python3 tools/loot_oracle_setup.py --oracle-dir /path/outside/git      # JDK 21 required
python3 tools/saved_data_generate.py --oracle-dir /path/outside/git
python3 tools/saved_data_owners_generate.py --oracle-dir /path/outside/git [--full-out FILE]
cargo test --locked --lib saved_data
```
