# Loot tables — source ledger and evidence

Target: Minecraft Java **1.21**, protocol **767**. Primary evidence is the unchanged
original server run through a local differential oracle; secondary evidence is
direct inspection of the official-mapped classes. Pinned artifacts (SHA-1 of the
bundler jar `450698d1863ab5180c25d7c804ef0fe6369dd1ba`, inner server jar SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`, official
mappings SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`) come from
[`fixtures/source-lock.json`](../../fixtures/source-lock.json). Java artifacts,
remapped jars and decompiled or disassembled text stay outside Git.

Scope of this document: `src/loot/**`, delivery `commands_data.5` (**partial**).
It claims **no** checklist point and **no** Main integration; see
[loot-integration.md](loot-integration.md) for what Main would have to provide.

## What is implemented

| Java class(es) | Rust | Notes |
| --- | --- | --- |
| `LootTable`, `LootPool` | `loot::entry::{LootTable, LootPool}` | `getRandomItems`, `getRandomItemsRaw`, `fill`, stack splitter, nested-table loop guard |
| `LootPoolEntries` (8 entry types) | `loot::entry::{Entry, Singleton, SingletonKind}` | `item`, `tag` (both modes), `loot_table` (key and inline), `dynamic`, `empty`, `alternatives`, `sequence`, `group` |
| `NumberProviders`, `IntRange`, `LevelBasedValue` | `loot::number` | `constant`, `uniform`, `binomial`, `enchantment_level`, `score` (host), `storage` (explicit unsupported) |
| `LootItemConditions` (all 19) | `loot::condition::Condition` | `entity_scores`, `reference` and `time_check`/`weather_check` use the level/resolver traits |
| `LootItemFunctions` (22 of 40) | `loot::function::FunctionKind` | see the table below |
| `LootContext`, `LootParams`, `LootContextParamSets` (23 sets) | `loot::context` | builder validation (`allowed`/`required`) is `LootParams::validate` |
| `RandomSequences`, `RandomSequence` | `loot::sequences` | persistent Xoroshiro streams, dirty marking, NBT save/load |
| `EnchantmentHelper` (levels, `updateEnchantments`, `selectEnchantment`, `enchantItem`) | `loot::enchant` | enchantment definitions come from `fixtures/loot-registry-facts-1.21.json` |
| `critereon` predicates used by loot | `loot::predicate` | see "Predicates" |
| Vanilla data pack | `loot::vanilla`, `fixtures/loot-tables-1.21.json` | all 1178 tables parse |

Functions: `set_count`, `explosion_decay`, `limit_count`, `enchanted_count_increase`,
`apply_bonus` (3 formulas), `set_damage`, `furnace_smelt`, `enchant_randomly`,
`enchant_with_levels`, `set_enchantments`, `copy_components` (block-entity source),
`copy_state`, `set_potion`, `set_components` (armor trim only), `set_item`,
`set_name` (plain text and bare `translate`), `set_stew_effect`,
`set_ominous_bottle_amplifier`, `set_instrument`, `exploration_map` (host call),
`sequence`, `reference`.

Predicates: `MinMaxBounds`, `ItemPredicate` (`items`, `count`, and the
`enchantments`, `stored_enchantments`, `damage` sub-predicates), `EntityPredicate`
(`type`, `flags`, `equipment`, `type_specific` for `slime`, `fishing_hook` and the
variant predicates), `DamageSourcePredicate`, `LocationPredicate` (`position`,
`dimension`, `biomes`, `block`, `can_see_sky`), `BlockPredicate`,
`StatePropertiesPredicate` (exact matches; integer ranges).

## Behaviours the differential vectors forced into the port

These are easy to get wrong from reading alone; each one was a real mismatch
against the original before it was fixed.

- **`ItemStack.getCount()` is 0 for an empty stack.** A function can drive a count to
  zero or below (`set_count` with a negative `uniform`); the next `grow`, `set_count
  add`, `limit_count`, `apply_bonus`, splitter and `fill` arithmetic then restarts
  from zero. `enchant::count_of` is that accessor.
- **Empty stacks are produced.** `createStackSplitter` forwards a stack with
  `count < maxStackSize`, including count 0 or negative; the result list contains
  them (`explosion_decay` on wheat yields two empty stacks). Only the feature filter
  and over-size splitting touch a stack.
- **Expanded tag members bypass the entry's functions.** `TagEntry.expandTag` builds
  entries whose `createItemStack` creates a bare stack; the non-expanding tag entry
  does apply its functions to each member.
- **Function layers apply innermost first, per stack, at emission time**
  (entry → pool → table, and nested-table layers inside the entry's), so a function's
  draws happen before the next entry is expanded.
- **Compound conditions short-circuit** like `Util.allOf`/`anyOf`; a random
  condition in a later term draws only if reached. A one-entry pool rolls without a
  weight draw.
- **Source selection** (`LootContext.Builder.create`): an explicit non-zero seed is
  `RandomSource.create(seed)` (legacy LCG), else the table's `random_sequence`
  (persistent Xoroshiro), else the level random. `RollRandom::select` encodes it.
- **Unsupported clauses fail where Java would evaluate them.** `EntityPredicate`
  clauses without a native owner (`distance`, `movement`, `effects`, `nbt`,
  `vehicle`, `passenger`, `team`, `slots`, …) parse, are skipped when an earlier
  clause already returned false, and raise `LootError::Unsupported` otherwise.
- The multi-enchantment wire order in Java is the JVM identity-hash order of a
  `Holder` map and differs between runs; the comparison therefore decodes both stacks
  and compares them as values (`ItemStack::same_item_same_components`).

## Evidence

`tools/loot_oracle.java` runs the **unchanged** original `LootTable`,
`LootContext`, number providers, conditions, functions and predicates after loading
the vanilla data pack through the original resource, tag, registry and codec
classes. Only the `ServerLevel`, `MinecraftServer` and one `BlockEntity` are
generated stubs (ASM subclasses answering queries from the scenario; no gameplay);
entities are real `EntityType.create` objects with the scenario's fire state, slime
size, frog variant, fishing-hook water flag and main-hand stack.
`tools/loot_generate.py` derives the scenarios from the tables themselves (tools,
block-state properties, killers, fire, luck, seeds chosen from what each table
tests) and records, per case, every produced stack as Slot wire bytes plus the next
`nextInt()` of the roll's random source. The Rust test must reproduce both, so it
proves exact RNG consumption order, not just the items.

`fixtures/loot-oracle-vectors.json` holds **8540 cases**:

| Category | Cases |
| --- | ---: |
| Block tables (tools, states, explosions, block entity) | 5378 |
| Entity tables (killer, looting, fire, slime sizes, frog variants, damage sources) | 1223 |
| Chests / archaeology / gifts / fishing / barter / equipment / spawners / pots / dispensers / shearing | 664 |
| Hand-written synthetic tables (24; pools, bonus rolls, every entry type, enchant, potion, stew, instrument, smelt, conditions) | 282 |
| `LootTable.fill` into a 27- or 9-slot container, with and without pre-filled slots | 317 |
| Named random sequences, three consecutive rolls, saved stream state compared | 676 |

1174 of the 1178 vanilla tables are rolled. The four that are not
(`chests/shipwreck_map`, `entities/pillager`, `entities/player`, `entities/wither`)
need a real structure search, raid manager, player and scoreboard in the harness;
they parse natively but are not differentially checked.

`cargo test --locked --lib loot::` runs the vectors in about one second, parses all
1178 tables, and adds focused tests for Java number semantics, parameter-set
validation, unknown/unported types, short-circuit of unsupported predicate clauses,
random-sequence persistence/NBT and source selection.

What this does **not** establish: any Main, world, entity, packet or persistence
behaviour; the `score`, `storage`, `enchantment_level`, `entity_scores` and
`exploration_map` paths (implemented from the source, not oracle-run); data-pack
supplied tables, predicates or item modifiers; performance.

## Not implemented (explicit errors)

Functions: `set_custom_data`, `copy_custom_data`, `set_attributes`, `copy_name`,
`set_contents`, `modify_contents`, `filtered`, `set_loot_table`, `set_lore`,
`fill_player_head`, `set_banner_pattern`, `set_fireworks`, `set_firework_explosion`,
`set_book_cover`, `set_written_book_pages`, `set_writable_book_pages`,
`toggle_tooltips`, `set_custom_model_data`, and `set_components` for anything but an
armor trim. `set_name` with entity resolution or styled components. Predicate
clauses listed above, `ItemPredicate.components`, the remaining item sub-predicates,
`LocationPredicate` structures/light/fluid/smokey, `BlockPredicate.nbt`. JSON object
member order is serde_json's (sorted), observable only when two sibling number
providers both draw randomness; no shipped table does.

## Reproducing the oracle

```bash
python3 tools/loot_oracle_setup.py --oracle-dir /path/outside/git    # JDK 21 required
python3 tools/loot_generate.py --oracle-dir /path/outside/git
cargo test --locked --lib loot::
```

`loot_oracle_setup.py` downloads the pinned jar and mappings with size and SHA-1
verification (`tools/bootstrap_oracle.py`), renames the server classes with
`tools/loot_remap.java` and compiles `tools/loot_oracle.java`.
`fixtures/loot-tables-1.21.json` is the verbatim shipped
`data/minecraft/loot_table/**` JSON; `fixtures/loot-registry-facts-1.21.json` is
exported from the same oracle (`export-facts`: enchantment definitions in registry
order, resolved tags, registry orders, `Item.getEnchantmentValue`, instant mob
effects). Regenerating the vectors changes only the order of multi-enchantment maps.
