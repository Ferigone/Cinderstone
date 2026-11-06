# Data-pack tags — `TagLoader`

Target: Minecraft Java **1.21**, protocol **767**. Primary evidence is the unchanged original
`TagLoader`, `TagFile`, `TagEntry` and `DependencySorter`, run through the local oracle
(`tools/loot_oracle.java tags` and `export-tags`) over folder packs on disk. Pinned artifacts:
bundler SHA-1 `450698d1863ab5180c25d7c804ef0fe6369dd1ba`, inner server jar SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`, official mappings SHA-1
`31c77994d96f05ba25a870ada70f47f315330437` ([`fixtures/source-lock.json`](../../fixtures/source-lock.json)).
Java artifacts and decompiled text stay outside Git.

Scope: `src/datapack/**`, work package WP-E1 **stage 3** (delivery `registries.5`, **partial**). It
claims **no** checklist point and **no** Main integration; nothing outside the module calls it.
Pack discovery and `pack.mcmeta` (stage 1), the registry loader (stage 2), the `Update Tags` packet
and reload (stage 4) are not implemented.

## What is implemented

`datapack::tags`: `TagEntry` / `TagFile` (the codec: `"ns:id"`, `"#ns:tag"`, `{ "id", "required" }`,
`values` and `replace`), `load(packs, directory)` (`TagLoader.load`), `build(loaded, is_known)`
(`TagLoader.build` with `DependencySorter`) and `load_and_build`. Packs are given lowest priority
first; `directory` is the path below `data/<namespace>/` (`tags/item`).

Behaviours the oracle forced (each was a real mismatch before it was fixed):

- **An invalid file still creates its tag.** `TagLoader.load` calls `computeIfAbsent` *before* the
  codec runs, so a file whose JSON parses but whose structure the codec rejects leaves an empty tag
  (which then resolves to an empty collection); a file that is not even JSON creates nothing.
  `replace` only takes effect for a file the codec accepts.
- **Gson is lenient.** The files are read with `JsonParser.parseReader`: unquoted names and strings,
  single quotes and trailing `;` are accepted, trailing content is not
  (`components::lenient`). A `required` or `replace` that is not a JSON boolean is a codec error.
- **Resource listing is a `TreeMap`** over the *file* ids, compared by path first and then namespace
  (`tags/test/b.json` before `tags/test/blocks.json`).
- **`HashMap.computeIfAbsent` prepends.** The `loaded` map is filled with `computeIfAbsent`, which
  links a new node in *front* of its bucket's chain (`put` appends), so colliding tag ids iterate
  newest first. The `DependencySorter` map and the dependency sets are filled with `put`/`add`.
- **Cycles depend on `HashMap` order.** Edges are added in the iteration order of
  `HashMap<ResourceLocation, ..>` (hash `31 * namespace.hashCode() + path.hashCode()`), required
  edges for every tag before optional ones; an edge that would close a cycle is dropped, so which
  member of a cycle fails to resolve depends on that order. A `HashMultimap` keeps each tag's
  dependencies in a `HashSet` that starts with 4 buckets.
- **A tag that references itself crashes the original.** Once `A -> A` is in the graph, a later
  `isCyclic` check that walks through `A` recurses without end unless an earlier sibling already
  answered `true`; the JVM throws `StackOverflowError`, which aborts the whole reload.
  `build` returns `Err(DependencyOverflow)` in exactly those cases (65 of 6000 random scenarios).
- A required reference to a missing element or tag removes the tag from the result; an optional one
  is skipped. Members are deduplicated in first-seen order (`ImmutableSet`).

## Evidence

`tools/datapack_generate.py` records the original's resolved tags (or its `StackOverflowError`).

- `fixtures/datapack-tags-vectors.json`: **600 synthetic scenarios** (1–3 packs, 2–14 tags in three
  namespaces, element / `#tag` / optional / required entries, unknown ids and tags, `replace`,
  duplicates, dense random references, and 17 kinds of invalid file); 594 resolve, 6 overflow. The
  run of **6000** scenarios (5935 resolved, 65 overflow) agrees too; regenerate it with
  `--scenarios 6000` and run `DATAPACK_TAGS_FILE=<json> cargo test --lib datapack::tests::explore_scenarios -- --ignored`.
- `fixtures/datapack-vanilla-tags-1.21.json`: the 530 shipped `data/minecraft/tags/**` files (13
  directories, exported by the oracle). The Rust loader over the shipped item, block, enchantment,
  damage-type, entity-type, biome and instrument tags reproduces the resolved tag sets already
  captured in `fixtures/loot-registry-facts-1.21.json` for every tag, in the same member order.

`cargo test --locked --lib datapack` runs both in about a tenth of a second.

## Not implemented / limits

- The registry-bound result: `Registry.bindTags`, the `Update Tags` packet and its (identity-hash
  dependent) tag order.
- Pack discovery, `pack.mcmeta`, overlays, feature flags, zip packs; `ResourceManager` filtering
  (`filter` sections).
- Namespaces and paths use the 1.21 `ResourceLocation` character set; a file name the vanilla loader
  would reject is not listed.
- Log output (the original logs each skipped file and missing reference) is not reproduced.

## Reproducing the oracle

```bash
python3 tools/loot_oracle_setup.py --oracle-dir /path/outside/git      # JDK 21 required
python3 tools/datapack_generate.py --oracle-dir /path/outside/git
cargo test --locked --lib datapack
```
