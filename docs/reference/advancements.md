# Advancement progress — requirements, `AdvancementProgress` and the per-player progress file

Target: Minecraft Java **1.21**, protocol **767**, DataVersion **3953**. Primary evidence is the
unchanged original `PlayerAdvancements` progress codec — `DataFixTypes.ADVANCEMENTS.wrapCodec(
PlayerAdvancements.Data.CODEC, .., 1343)` over a document read the way `PlayerAdvancements.load`
reads it, then `AdvancementProgress.update(requirements)` and the save step — run through the
local oracle (`tools/loot_oracle.java advancement-progress`). Pinned artifacts: bundler SHA-1
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`, inner server jar SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`, official mappings SHA-1
`31c77994d96f05ba25a870ada70f47f315330437` ([`fixtures/source-lock.json`](../../fixtures/source-lock.json)).
Java artifacts and decompiled text stay outside Git.

Scope: `src/advancements/**`, work package WP-E5 **core** (the persistence and progress half of the
advancement system). It claims **no** checklist point and **no** Main integration; nothing outside
the module calls it.

Stage 1 adds the advancement *definitions* (`definition::parse`, described in the next section).
The tree and its layout, trigger evaluation, the `PlayerAdvancements` runtime and the
`Update Advancements` packet are **not** implemented (see the end).

## Definitions (`advancements::definition`)

`parse(json)` is `Advancement.CODEC` with its `validate`, over a file read with the lenient Gson
reader. It covers:

- **Top-level fields:** `parent`, `display`, `rewards`, `criteria` and `requirements`. A missing
  `requirements` means all criteria are required, and the requirements must exactly match the
  criteria.
- **`display`:** title, description, background, frame, `show_toast`, `announce_to_chat` and
  `hidden`.
- **The display icon** (`ItemStack.STRICT_CODEC`):
  - The item must be registered and not air.
  - `count` uses `orElse(1)`, so an unreadable or out-of-range count is 1. The count may not
    exceed the item's maximum stack size.
  - The component values are validated for the types vanilla icons use: `item_name`,
    `hide_additional_tooltip`, `pot_decorations` and `banner_patterns`. Any other component type
    answers `Unsupported`.
- **`rewards`:** experience, loot, recipes and function.
- **`sends_telemetry_event`.**
- **Criteria:** each criterion is decoded with its trigger's `TriggerInstance` codec. All 56
  registered triggers are covered.
  - `EntityPredicate.ADVANCEMENT_CODEC` reads a list of loot conditions, or else an entity
    predicate.
  - Fields that are lists of loot conditions (`ContextAwarePredicate.CODEC`) accept a list only.
  - The new predicate types are `DistancePredicate`, `MobEffectsPredicate` and `DamagePredicate`.
  - Item, entity, location, damage-source, state and bound predicates reuse `crate::loot::parse`.
  - A missing `conditions` member counts as an empty map.

Evidence comes from `tools/loot_oracle.java advancement-parse` (`Advancement.CODEC` over
`RegistryOps<JsonOps>`, with the vanilla registries and tags) and
`tools/advancement_parse_generate.py`:

- All 1,399 shipped advancements (`fixtures/advancements-vanilla-1.21.json`) are accepted by the
  original and parse here.
- Mutants are one-step changes of every distinct criterion, conditions, display and rewards
  fragment, plus hand-written requirement and envelope cases. There are 44,195 of them (5,567
  accepted by the original), and they agree with zero disagreements and nothing `Unsupported`.
  `fixtures/advancement-parse-vectors.json` keeps every 4th (11,049). Point
  `ADVANCEMENT_PARSE_VECTORS` at a `--sample 1` run to check all of them.

Some predicate clauses inside criteria are not validated by their codec (marked `[unvalidated]`
by the loot parser). `Advancement::unvalidated_clauses()` lists them, and while that list is
non-empty an accepted file is not established as accepted by the original.

- **Vanilla:** 12 shipped advancements contain such clauses. Seven use `location.structures`
  or item `components`, four use the `player` and `lightning` entity sub-predicates, and one uses
  the `jukebox_playable` item sub-predicate.
- **Mutants:** 45 of the 44,195 mutants count as unsupported for the same reason.

The parsed predicates are kept for the trigger stage. Nothing evaluates a trigger, and the
original's post-load `validate` (parameter-set warnings) is not reproduced. A file without
`requirements` gets one requirement per criterion in name order; the original uses the file's
member order, which `serde_json` does not keep.

## Trigger matching (`advancements::triggers`)

The first two trigger matchers cover every recipe advancement (WP-E6 #27).

- **`inventory_changed`** (`InventoryChangeTrigger`):
  1. The slot counts come first: empty, occupied, and full, where full means a stack at its
     maximum size.
  2. Then the item predicates:
     - no predicate matches anything;
     - a single predicate is tested against the changed stack only;
     - several predicates must each be matched by some non-empty slot.
- **`recipe_unlocked`:** the recipe id must equal the criterion's.

Every criterion's `player` predicate needs the player as a loot entity. When one is present, the
answer is `PlayerPredicate` and the caller evaluates it.

**Evidence:** `tools/loot_oracle.java inventory-trigger` runs the unchanged
`TriggerInstance.matches` over a real `Inventory`. The slot counts are computed as `trigger`
computes them, and that computation is transcribed. The criteria are the 456 distinct vanilla
`inventory_changed` conditions plus 10 hand-written ones, matched against 160 seeded 41-slot
inventories. All 74,560 answers agree (`fixtures/inventory-trigger-vectors.json`,
`tools/inventory_trigger_generate.py`).

## Tree, layout and the update packet (`advancements::tree`)

- **Tree:** `AdvancementTree::add_all` is `AdvancementTree.addAll`. It inserts every advancement
  whose parent is known, repeating until nothing more can be inserted, and returns the ids that
  could not be inserted. `remove` removes children first.
- **Layout:** `layout(root, has_display, children_of)` is `TreeNodePosition.run`, the
  Walker-style layout that sets each visible advancement's `(x, y)`. Invisible advancements are
  skipped through to their children.
- **Visibility:** `visibility(tree, node, is_done, has_display, hidden)` is
  `AdvancementVisibilityEvaluator.evaluateVisibility`. A node is visible when it or a descendant is
  done, or when the nearest of itself and its two closest ancestors that has a rule says show.
- **Packet:** `update_advancements_packet` writes the body of `ClientboundUpdateAdvancementsPacket`.
  It covers each holder's parent, display (title, description, icon `ItemStack`, frame, the flags
  integer, background, x/y), requirements and telemetry flag, then the removed ids and the
  progress map.
  - Criteria in a progress entry follow the original's `HashMap` order. That order is modelled as
    `Collectors.toMap`, then the copy into a map sized for the decoded entries, then
    `AdvancementRequirements.names()` (a fastutil `ObjectOpenHashSet` filled with `addAll`).

Two orders are not reproducible:

- **Child order:** the original keeps each node's children in a `ReferenceOpenHashSet`, which
  iterates in object identity-hash order. The port keeps insertion order and lets the caller pass
  any child order to `layout`.
- **Packet sets:** the packet's `removed` set and `progress` map are copied with `Set.copyOf` /
  `Map.copyOf`, whose iteration order is randomised per run. Only one-element sets and maps have
  a fixed byte form.

Evidence comes from `tools/loot_oracle.java advancement-tree` and
`tools/advancement_tree_generate.py` (`fixtures/advancement-tree-1.21.json`). The oracle loads
the vanilla tree through the unchanged `ServerAdvancementManager` and records, for every node,
its parent, its children in the original's iteration order and its display position. It also
writes an update packet containing all 1,399 holders sorted by id, one removed id, and the
progress of `adventure/kill_all_mobs` with three criteria granted.

The test `vanilla_tree_layout_and_packet_match_the_original` checks:

- the tree's parents, children and roots;
- all 122 display positions (bit-exact, using the original's child order);
- the visible sets the original reports for 60 sets of done advancements (from none to all,
  random densities);
- the 186,085-byte packet, byte for byte, and three more single-progress packets. Two of these
  carry fresh progress (the copied map starts empty, so the default 16-bucket table applies), and
  one carries a single decoded criterion.

## What is implemented

| Java | Rust |
| --- | --- |
| `AdvancementRequirements` (`allOf`, `anyOf`, `test`, `count`, `names`, `validate`, `size`) | `requirements::AdvancementRequirements` |
| `CriterionProgress` | `progress::CriterionProgress` (obtained instant as epoch seconds) |
| `AdvancementProgress` (`update`, `isDone`, `hasProgress`, `grantProgress`, `revokeProgress`, `getPercent`, `getFirstProgressDate`, `CODEC`) | `progress::AdvancementProgress` (`update`, `is_done`, `has_progress`, `grant_progress`, `revoke_progress`, `percent`, `first_progress_date`, `from_json`, `to_json`) |
| `PlayerAdvancements.load` document handling and `Data.CODEC` | `progress::parse_progress_file` |
| `PlayerAdvancements.save` / `asData` document | `progress::write_progress_file` |
| `DateTimeFormatter.ofPattern("yyyy-MM-dd HH:mm:ss Z")` as `ExtraCodecs.temporalCodec` | `time::{parse_obtained, format_obtained}` |
| Gson `JsonReader` with `setLenient(false)` and `Streams.parse` | `components::lenient::parse_strict_prefix` |

Behaviours the oracle forced (each one was a real mismatch against the original before it was
fixed; the first four are visible only through the executed code):

- **Dates are `SMART`-resolved.** `yyyy` is `appendValue(YEAR_OF_ERA, 4, 19, EXCEEDS_PAD)`: four
  digits, or a `+` and five or more (`+12345-…`); a `+` on four digits, five digits without a sign,
  a negative year, year `0000` and years above 999,999,999 are all rejected. Month and day must
  be two digits in 1–12 / 1–31, and a day past the end of the month is **clamped**
  (`2024-02-30` is February 29th, `2023-04-31` April 30th). `24:00:00` is the next midnight
  (applied after the clamp: `2024-02-30 24:00:00` is March 1st), `24:00:01`, `24:01:00`, hour 25+
  and second 60 are errors. The zone is `+HHMM` only (`+0000` or a sign with hours ≤ 23, minutes ≤ 59
  and a total of at most 18:00; `-0000` is accepted, `Z`, `+01:00`, `+01` are not). Digits are ASCII
  only. Written back, every instant is UTC (`+0000`, as on a server whose zone is UTC), with the
  *year of the era* (a year before 1 prints as the BCE year) and a `+` once it needs five digits.
- **A day `LocalDate` cannot hold is an error.** `+999999999-12-31 24:00:00` fails while reading; an
  instant that lands in year 1,000,000,000 after the offset (`+999999999-12-31 23:59:59 -1800`)
  reads, but cannot be written: the original throws from `save()`.
- **`done` is ignored when reading.** `Codec.BOOL.fieldOf("done").orElse(true)` swallows any value
  (a number, an object, `1e400`); `done` is recomputed from the requirements on every write.
  `criteria` is `optionalFieldOf`: absent or JSON `null` is "none", but a present value of the wrong
  type, or one bad date, rejects the **whole file**, including entries nobody asked for.
- **Two spellings of one advancement id reject the file** (`x` and `minecraft:x`, `:x`):
  `Codec.unboundedMap` reports `Duplicate entry for key`. Identical JSON member names are
  collapsed by Gson first (the last wins).
- **`DataVersion` is read with `JsonOps.getNumberValue`:** a number (`3953.5` is 3953, a value that
  overflows `int` wraps), never a string, boolean or `null` — those mean "no version" (1343). The
  key is removed before the map is decoded.
- **The reader is Gson's.** One value is read and what follows is never looked at (`{…},` and
  `{…} garbage` are fine); a byte-order mark is skipped; a raw control character (tab, newline,
  NUL) *inside a string* is accepted, which `serde_json` refuses; `\uD800` escapes are kept (a lone
  one becomes U+FFFD here, harmless because such a name can never equal a requirement name);
  `NaN`/`Infinity`, unquoted or single-quoted strings, comments, `;` and `=`, trailing commas and
  leading zeros are rejected; keywords are matched in any case (`TRUE`); a form feed is not
  whitespace; numbers keep their text whatever their size (`1e400` is a number).
- **While reading the lenient form** (`components::lenient::parse`, `JsonParser.parseString`) the
  same oracle showed three older deviations, fixed here: `[1,]` is `[1, null]` (and `[,1]` is
  `[null, 1]`), `{"a":1,}` is an error, and a comment *after* the value is an error (the end-of-
  document check runs with the reader back in strict mode).

## Data fixers

`wrapCodec` runs the `ADVANCEMENTS` data fixers from the file's `DataVersion` (1343 when absent).
The executed original shows the newest fixer that can change a progress file is `CriteriaRenameFix`
at schema **3097**; the earlier ones rename advancements (1501 recipes of the 1.12 → 1.13 move,
2100, 2503, 2846), criteria (3086, 3097) and biomes/entity names inside criteria. A file at
version 3097 or later passes through them unchanged (the vectors cover 3097, 3098, 3200, 3465, 3700,
3952 and 3953 with legacy content, and every one agrees). **A file older than 3097, or without a
`DataVersion`, is reported as `ProgressError::DataFixerRequired(version)`**: those fixers are not
ported. A server that reads such a file should do what the original does on a parse failure
(log it and start with no progress), or port the fixers first.

## Evidence

`tools/advancement_generate.py` feeds the original (`advancement-progress`: strict read of the
text, `wrapCodec`, `update` with a requirement list per id, re-encode, and each entry's `isDone`)
and records accepted or rejected, the re-encoded document, every `done` and the version the codec
read.

- `fixtures/advancement-progress-vectors.json`: a deterministic sample (1 in 16, plus the first
  row of every kind) of **19,074** generated progress files — 1,286 rows kept, 337 of them accepted.
  The generated set holds: one-step structural mutants of a five-advancement document (each member
  removed, each value replaced by 16 wrong-typed values) and 400 two-edit combinations; 468 date
  strings (hand list plus every field of a stamp set to edge values, calendar edges × end of day ×
  offsets); 7,354 date fuzz strings (every one-character delete/replace/insert of six stamps plus
  random field widths); 9,510 one-to-three edit mutants of the JSON *text* (control characters,
  surrogates, quotes, separators, byte-order marks, number forms); 71 hand-written text cases;
  30 `DataVersion` spellings and values; 23 legacy-content files at versions 1 … 3953; 27 id
  spellings; 470 requirement shapes (all-of, any-of, empty groups, unknown criteria, duplicates) and
  384 done-masks. **The full generated set agrees with the Rust port with zero disagreements.**
  Run it with `--full-out FILE` and
  `ADVANCEMENT_PARITY_FILE=FILE cargo test --lib advancements::tests::explore_parity -- --ignored`
  (about 6 MB, not committed).
- A row is compared on: accepted or rejected, the written document (as a JSON value, so the
  member order — an identity-hash order in the original — is not a contract) and every entry's
  `done`. Rows whose version is below 3097 must come back as `DataFixerRequired(version)` with
  the same version the original read.

`cargo test --locked --lib advancements::` runs the fixture plus unit tests (about half a second).

## Not implemented (explicit)

- `ServerAdvancementManager` loading from the selected packs, and the orphan warnings it logs.
  Definitions, the tree, the layout and the packet exist (above), but no trigger is evaluated.
  Icon components other than the four vanilla types are `Unsupported`.
- `PlayerAdvancements` runtime (listeners, `award`/`revoke`, rewards, chat announcements,
  `progressChanged`, the `/advancement` command) and `ClientboundUpdateAdvancementsPacket`
  (`criteria` are a `HashMap` in the original; the packet's map order would be Java's).
- The data fixers for files older than DataVersion 3097 (above).
- The text of the saved file: `write_progress_file` returns a JSON value; the original writes it
  with Gson's pretty printer (two-space indent, HTML-safe escaping) and an identity-hash member
  order. Invalid UTF-8 is a decoding error before the text reaches this module.
- A `DataVersion` written as a decimal above 2^53 is read from its nearest `f64`
  (`1E30` is 2^30 in the original, 0 here); nesting deeper than `components::lenient::MAX_DEPTH`
  (512) is rejected where the original fails at a stack-dependent depth of thousands.

## Reproducing the oracle

```bash
python3 tools/loot_oracle_setup.py --oracle-dir /path/outside/git      # JDK 21 required
python3 tools/advancement_generate.py --oracle-dir /path/outside/git --sample 16 [--full-out FILE]
cargo test --locked --lib advancements::
```
