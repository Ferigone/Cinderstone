# Data-pack repository — `pack.mcmeta`, discovery, selection and `WorldDataConfiguration`

Target: Minecraft Java **1.21**, protocol **767**, data-pack format **48**. Primary evidence is the
unchanged original `PackRepository`, `ServerPacksSource`, `FolderRepositorySource`, `Pack`,
`PackMetadataSection`, `FeatureFlagsMetadataSection`, `OverlayMetadataSection` and
`WorldDataConfiguration`, run through the local oracle (`tools/loot_oracle.java packs` and
`world-data-config`) over a `datapacks` folder prepared on disk. Pinned artifacts: bundler SHA-1
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`, inner server jar SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`, official mappings SHA-1
`31c77994d96f05ba25a870ada70f47f315330437` ([`fixtures/source-lock.json`](../../fixtures/source-lock.json)).
Java artifacts and decompiled text stay outside Git.

Scope: `src/datapack/{packs,config,zip}.rs`, work package WP-E1 **stage 1** (delivery
`registries.5`, **partial**). It claims **no** checklist point and **no** Main integration; nothing
outside the module calls it, and `src/level.rs` is untouched. The tag loader (stage 3) is
[datapack-tags.md](datapack-tags.md); the registry loader (stage 2) and reload (stage 4) are not
implemented.

## What is implemented

| Java | Rust |
| --- | --- |
| `Pack.readPackMetadata`, `PackMetadataSection`, `FeatureFlagsMetadataSection`, `OverlayMetadataSection`, `PackCompatibility.forVersion` | `packs::read_metadata`, `PackMetadata`, `Compatibility` |
| `ServerPacksSource` built-in packs | `packs::built_in_packs` (`vanilla`, `bundle`, `trade_rebalance`) |
| `FolderRepositorySource.loadPacks` / `discoverPacks` | `packs::discover_folder` (directories, `.zip` archives, non-pack entries) |
| `PackRepository` | `packs::PackRepository` (`reload`, `set_selected`, `add_pack`, `remove_pack`, selected ids and features) |
| `Pack.Position.insert` | `insert_by_position` (required packs) |
| `WorldDataConfiguration`, `DataPackConfig`, `FeatureFlags.CODEC` | `config::WorldDataConfiguration` (`from_nbt`, `to_nbt`) |
| `ZipFile.getEntry("pack.mcmeta")` | `zip::read_entry` (central directory, stored and deflate) |

Behaviours the oracle forced (each was a real mismatch before it was fixed):

- **Gson reads one value and stops.** `GsonHelper.parse` is not lenient, but it never checks what
  follows the first object: `{...},` and `{...} {}` are accepted, a leading byte-order mark is
  skipped, a top-level array or `null` is not. The reader is `components::lenient::parse_strict_prefix`
  (not `serde_json`): a raw tab, newline or NUL inside a string is accepted and a number keeps its
  text whatever its size (`pack_format: 1e400` is read as an `int` of 0).
- **`Codec.INT` takes numbers only.** A boolean `pack_format` rejects the pack; a boolean inside
  `supported_formats` makes that lenient field absent. A fractional number is truncated.
- **`supported_formats`** may be a number, a pair or `{min_inclusive, max_inclusive}`; a malformed
  or inverted range is ignored (lenient), and a range that does not contain `pack_format` is
  replaced by `[pack_format, pack_format]`.
- **A broken `features` or `overlays` section only drops that section**, never the pack; a broken
  `pack` section drops the pack. Unknown feature flags, a directory name outside `[-_a-zA-Z0-9.]`
  or an inverted overlay range make the whole section unreadable.
- **Selection.** `setSelected` keeps the order given, drops unknown ids and keeps duplicates;
  `getSelectedIds` is a set but `getSelectedPacks` is not; incompatible packs stay selectable.
  Available packs iterate in `TreeMap` order of their ids (`bundle`, `file/...`, `trade_rebalance`,
  `vanilla`; UTF-16 order). No pack of the server repository is `required`.
- **`WorldDataConfiguration`.** Each member is lenient on its own: a bad `DataPacks` keeps the
  default packs and still reads `enabled_features`. `DataPackConfig` has no `equals`, so the codec
  omits `DataPacks` only for the very default object it fell back to; a list read from the file that
  equals the default is written back in full. Feature flags are a set whose names are written in
  `HashSet<ResourceLocation>` order.

## Evidence

`tools/datapack_packs_generate.py` records the original's discovery and selection results.

- `fixtures/datapack-packs-vectors.json` (4 folders, 5 steps to 60 steps each):
  - **1393 `pack.mcmeta` variants** (a 1-in-4 sample of 349 is committed; `--sample 1` keeps all):
    11 base documents, every member removed and replaced by 18 wrong-typed values, and 31 text
    corruptions (truncation, trailing comma or content, byte-order mark, single quotes, comments,
    wrong top-level type, raw control characters inside strings, and numbers of unusual form or
    size such as `1e400`, `4294967344`, `48.9`, `03`, `+48`). 873 are accepted as packs;
    every accepted pack agrees on id, compatibility, description and title text, feature names,
    required/fixed/position and `shouldAddAutomatically`.
  - a **structure** folder (directories with and without `pack.mcmeta`, an empty `pack.mcmeta`,
    non-ASCII and spaced names, stored, deflated, nested, empty, corrupt and upper-case-extension
    zip files, stray files) with selection steps;
  - two **selection** folders with 60 random `select` / `add` / `remove` / `reload` steps each over
    real, feature and unknown ids; the selection and requested feature flags after every step agree.
- `fixtures/datapack-config-vectors.json`: **437** `WorldDataConfiguration` payloads (6 bases and
  their one-step NBT mutants, as `level.dat` is read); the re-encoded NBT agrees for all of them.

`cargo test --locked --lib datapack` runs everything in about half a second.

## Not implemented / limits

- Opening a pack's resources (`PackResources`), overlays beyond the list of applicable directory
  names, and feature-flag-gated registry loading.
- Symbolic links: the original follows them only when the world's `allowed_symlinks.txt` allows it;
  the port skips them. Zip files are read by a minimal reader (stored and deflate, no zip64,
  entry named exactly `pack.mcmeta` at the root).
- Pack format 48 is fixed; `Pack.Metadata` keeps no resource handle, and log output is not
  reproduced.
- `level.dat` itself: `WorldDataConfiguration` reads and writes the two members, but
  `src/level.rs` does not call it.
- A `pack_format` / `supported_formats` number written as a decimal above 2^53 (`1E30`) is read from
  its nearest `f64`, so its low 32 bits can differ from the original's exact `BigDecimal`
  (`components::java::json_number_int_value`); above 2^127, and for every integer spelling, they agree.

## Reproducing the oracle

```bash
python3 tools/loot_oracle_setup.py --oracle-dir /path/outside/git      # JDK 21 required
python3 tools/datapack_packs_generate.py --oracle-dir /path/outside/git
cargo test --locked --lib datapack
```
