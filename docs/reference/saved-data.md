# Saved data — `DimensionDataStorage` and the first owners

Target: Minecraft Java **1.21**, protocol **767**, `DataVersion` **3953**. Primary evidence is the
unchanged original `DimensionDataStorage`, `SavedData`, `RandomSequences` and `MapIndex` run
through the local oracle (`tools/loot_oracle.java saved-data` and `saved-data-mutants`) over a
real temporary directory. Pinned artifacts: bundler SHA-1 `450698d1863ab5180c25d7c804ef0fe6369dd1ba`,
inner server jar SHA-256 `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`,
official mappings SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`
([`fixtures/source-lock.json`](../../fixtures/source-lock.json)). Java artifacts and decompiled
text stay outside Git.

Scope: `src/saved_data/**`, work package WP-G2 stage 2 and the first stage-3 owners (delivery
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

`cargo test --locked --lib saved_data` runs the scenarios, the mutants and seven focused tests in
about half a second.

## Deliberate differences from the original

- **Atomic replace.** The original truncates `<name>.dat` in place. This library writes
  `<name>.dat_new`, syncs it and renames it over the target, so a crash leaves the old or the new
  file. The resulting file is the same.
- **A failed write keeps the object dirty** (Java clears the flag whatever happens), so the next
  `save` retries; the failure is in `diagnostics()`.
- **The `data` folder is not created**; a missing folder is a logged write failure as in Java.
- Names containing a path separator or `..` are rejected.

## Not implemented

`level.dat` and `PrimaryLevelData` field owners (stage 1), per-dimension storage (needs WP-G1),
save ordering and `saveAllChunks` (stage 4), the scoreboard, command-storage, forced-chunk, raid,
map-data and structure-index owners, DataFixers for any other type (WP-G5), the 2 MiB read quota
of an uncompressed `NbtIo.read` (this library bounds reads with the crate's NBT limits), and any
Main wiring.

## Reproducing the oracle

```bash
python3 tools/loot_oracle_setup.py --oracle-dir /path/outside/git      # JDK 21 required
python3 tools/saved_data_generate.py --oracle-dir /path/outside/git
cargo test --locked --lib saved_data
```
