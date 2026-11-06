# Loot tables — integration contract for Main

Status: **library only**. `src/loot` has no Main callsite, and nothing in this
document is wired. It states what a production call needs, with the Java call
sites read from the pinned 1.21 server (official mappings, direct inspection).
Owner of every Main/runtime seam named here: the integration coordinator.

## API

```rust
let tables = crate::loot::vanilla::vanilla();            // 1178 parsed tables, a LootResolver
let table  = tables.get("minecraft:blocks/diamond_ore")?;
let mut ctx = LootContext::new(params, &mut random, &level, tables);
let stacks: Vec<ItemStack> = table.get_random_items(&mut ctx)?;   // Java getRandomItems(LootContext)
table.fill(&mut ctx, &mut container_slots)?;                      // Java fill(Container, …)
```

- `LootParams` carries the parameters; call `params.validate(table.param_set)` to get
  `LootParams.Builder.create(paramSet)`'s checks. Entities, damage source and block
  entity are borrowed trait objects (`LootEntity`, `LootDamageSource`,
  `LootBlockEntity`); the level is `LootLevel`.
- The random source is the caller's. `sequences::RollRandom::select(seed, table.random_sequence,
  &mut sequences, &mut level_random)` reproduces `LootContext.Builder.create`:
  non-zero seed → legacy LCG, else the table's named sequence, else the level RNG.
- Results are the stacks Java would hand to its consumer: already feature-filtered
  and split to the maximum stack size. They may include count-0 stacks; the caller
  must treat `ItemStack::is_empty()` stacks as Java does: `Block.popResource` draws its
  three position offsets from `level.random` *before* it skips an empty stack, so empty
  results still consume level randomness and then spawn nothing (it also honours the
  `doBlockDrops` game rule).
- Any `LootError::Unsupported` is an explicit "no native owner yet", never a guess.

## Call sites and the parameters they must supply

| Site (Java) | Table / param set | Parameters | Random source |
| --- | --- | --- | --- |
| `BlockBehaviour.getDrops(state, builder)` ← `Block.getDrops(state, level, pos, blockEntity, entity, tool)` | block's `minecraft:blocks/<id>` (`BuiltInLootTables.EMPTY` ⇒ no drops), set `block` | `ORIGIN = Vec3.atCenterOf(pos)`, `BLOCK_STATE`, `TOOL` (`ItemStack.EMPTY` when none), optional `THIS_ENTITY`, `BLOCK_ENTITY`, `EXPLOSION_RADIUS` | no seed → `random_sequence` |
| `LivingEntity.dropFromLootTable(source, causedByPlayer)` | `getLootTable()` = entity type default, set `entity` | `THIS_ENTITY`, `ORIGIN = position()`, `DAMAGE_SOURCE`, optional `ATTACKING_ENTITY`, `DIRECT_ATTACKING_ENTITY`; if `causedByPlayer && lastHurtByPlayer != null`: `LAST_DAMAGE_PLAYER` and `luck = player.getLuck()` | `getLootTableSeed()` (0 ⇒ sequence) |
| `RandomizableContainer.unpackLootTable(player)` | container's loot table, set `chest` | `ORIGIN = Vec3.atCenterOf(pos)`; with a player: `THIS_ENTITY`, `luck` | `fill(container, params, getLootTableSeed())` |

`popResource` (random item-entity offsets from `level.random`) and the
`GENERATE_LOOT` criterion trigger happen **outside** the loot call, after it returns /
before it runs respectively; they are Main's ordering to preserve.

## Owners the host traits stand in for

| Trait method | Real owner needed | State on master |
| --- | --- | --- |
| `LootLevel::{game_time, day_time, is_raining, is_thundering, dimension}` | Level clock/weather | exists in the world modules; adapter missing |
| `LootLevel::{is_loaded, block_state, biome, can_see_sky}` | chunk/biome access | block states yes; biomes not (worldgen team) |
| `LootLevel::smelting_result` | recipe manager | **default implementation uses `src/smelting.rs`** (vanilla default recipes) |
| `LootLevel::exploration_map` | structure search + map saved data | absent |
| `LootLevel::score`, `LootEntity::scoreboard_name` | scoreboard | absent (`commands_data.3`) |
| `LootEntity` | entity runtime: type ID, fire state, equipment, slime size, variants | entity facts exist per module; no unified view |
| `LootEntity::custom_name_nbt`, `LootEntity::player_profile`, `LootBlockEntity::nameable_custom_name_nbt` | `copy_name`, `fill_player_head`: the entity's or block entity's custom name and a player's `ResolvableProfile` network bytes | defaults answer `Unsupported` |
| `LootDamageSource` | damage pipeline | `src/living_damage.rs` / team-4 `src/combat` (source type + entities) |
| `LootBlockEntity::collect_components` | native block entities (`BlockEntity.collectComponents`) | chest/barrel/furnace owners exist; components not exported |
| `RandomSequences` persistence | `data/random_sequences.dat` saved-data lifecycle (`world_storage.4`) | type provided here; the file lifecycle is [`saved_data`](saved-data.md) (`random_sequences_factory(seed)` on a `DimensionDataStorage`), its Main call site is the coordinator's |

## Compatibility notes for the other teams

- Team 4 (`src/effects`, `src/combat`): `EnchantmentHelper.getEnchantmentLevel` for
  an entity is `loot::enchant::entity_enchantment_level` over `LootEntity::equipment`;
  it reads the ordinary `enchantments` component of every slot the enchantment is
  active in. No effect/attribute owner is touched.
- Team 3 (`src/workstations/enchanting.rs`): `EnchantmentHelper.selectEnchantment`
  is implemented in `loot::enchant::select_enchantment` against the captured
  enchantment definitions. Their `EnchantmentRegistry` trait and this fixture should
  converge on one registry owner; until then both are checked against the same
  original algorithm and neither depends on the other.
- Team 2 (worldgen): biome IDs here are plain resource-location strings supplied by
  `LootLevel::biome`; there is no dependency on worldgen types.

## Requested of the coordinator (none are blocking the library)

1. A production `LootLevel`/`LootEntity` adapter when block survival breaking or mob
   death is scheduled (`block_interactions.4`, `survival_combat.3/.5`).
2. Call `saved_data::DimensionDataStorage` for `random_sequences.dat` (`world_storage.4`, library delivered; see [saved-data.md](saved-data.md)): `compute_if_absent(&random_sequences_factory(seed), "random_sequences")` per level, `save()` in the vanilla save sequence.
3. Review `src/lib.rs`: one `pub mod loot;` line was added under the standing
   mechanical-export exception.
