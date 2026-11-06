# Recipe unlocks through the vanilla recipe advancements (#27, `crafting.4`)

Target: Minecraft Java 1.21. Recipes are unlocked the way vanilla unlocks them:
by the shipped `data/minecraft/advancement/recipes/**` advancements, whose
rewards are the recipes. There is no hard-coded unlock table.

## What runs

- **Catalogue** (`src/advancements/player.rs`, `RecipeAdvancements::vanilla`):
  - the 1,276 recipe advancements and `recipes/root`, parsed by the ported
    `Advancement.CODEC`;
  - their triggers are `inventory_changed` (1,391 criteria), `recipe_unlocked`
    (1,276), `enter_block` (9, the boats' `in_water`), `tick` (the crafting
    table's `unlock_right_away`) and `impossible` (root);
  - it is loaded once, before the server listens.
- **Per player** (`PlayerAdvancements`): the progress of each advancement. A
  criterion listens while neither it nor its advancement is done
  (`registerListeners` / `unregisterListeners`).
- **Trigger points** (`main_item_gameplay.rs`):
  - InventoryMenu `slotChanged` for inventory-backed slots: item pickup, menu
    clicks, creative slots, workstation outputs, and the first broadcast after
    login, which reports every non-empty slot;
  - `ServerPlayer.onInsideBlock` for `enter_block`;
  - `TICK` once per player per server tick.
- **Awards** (`main_recipe_advancements.rs`), once per tick after the connection
  phase. The chain is:
  1. `PlayerAdvancements.award`;
  2. `AdvancementRewards.grant`;
  3. `awardRecipesByKey`;
  4. `ServerRecipeBook.addRecipes`. Its `RecipeUnlockedTrigger` runs nested,
     before the ADD packet.

  Recipes that other owners add (furnace, smithing or stonecutter outputs) fire
  `recipe_unlocked` at the next award pass. Awards wait until the player's
  recipe-book INIT is built, as vanilla sends INIT before the first tick.
- **Persistence**:
  - the `advancements/<uuid>.json` document (`PlayerAdvancements.save`) is kept
    in the native player record (`PlayerLifecycle.advancements_json`);
  - an Anvil world also writes and imports `world/advancements/<uuid>.json`;
  - entries of trees that are not loaded are kept and written back unchanged.

## Evidence

- `tools/recipe_unlock_generate.py` builds `fixtures/recipe-unlock-vectors.json`:
  62 scripts, 316 pickups. The unchanged `InventoryChangeTrigger.TriggerInstance.matches`
  answers all 442 distinct recipe-tree `inventory_changed` criteria after each
  pickup. Every recipe advancement has one any-of requirement group, so the
  expected completions follow directly.
- `advancements::player_tests` checks two things:
  - each pickup completes the same advancements, and the same recipe set
    results;
  - the save/load round trip keeps progress and drops the listeners of
    completed advancements.
- `tools/recipe_unlock_live_probe.mjs` (stock protocol-767 client) showed:
  - a fresh player gets an empty INIT, then ADD `crafting_table`;
  - an oak log dropped from the creative inventory and picked up gives
    `collect`, `set_slot` and ADD `[oak_planks, oak_wood, charcoal]`;
  - after a server restart, INIT holds all four recipes and nothing is
    awarded again.

## Differences and limits

- **ADD packet timing.** The ADD packet is sent at the end of the tick that
  matched it, after that tick's slot packets. The original sends it from inside
  `broadcastChanges`, before the slot's `ContainerSetSlot`. The recipe-book
  state and the toast are the same.
- **Listener order.** Several advancements matched by one trigger are awarded in
  advancement-id order. The original's order is a `HashSet` order over
  identity-hashed predicates, so it is not reproducible.
- **Recipe advancements have no display.** No `ClientboundUpdateAdvancements`
  is sent. The other vanilla trees, with their displayed advancements, are not
  loaded (#26).
- **Unsupported criteria.** `player` predicates on these criteria are refused
  at load; none exist in the vanilla data.
- **Old progress files.** A file older than DataVersion 3097 needs the
  `ADVANCEMENTS` data fixers, which are not ported. Admission refuses it rather
  than dropping the progress.
- **Not part of this delivery.**
  - `doLimitedCrafting` stays refused when enabled; the default is off.
  - `ServerboundPlaceRecipe` is decoded and ignored.
  - `/recipe` needs the command system (#24).
