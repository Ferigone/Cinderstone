# Menu removal at the death/respawn boundary

Contract requested by [Team 4](https://github.com/Ferigone/Cinderstone/issues/3#issuecomment-6563364550).
This records callable APIs and pinned Java source facts. It does not establish
a native `ServerPlayer.die`, death/respawn flow or new workstation Main callsite.
Central Main/runtime ownership and the single entity-event output FIFO remain
required; neither team gains those files through this contract.

## Primary evidence

The unchanged Java **1.21**, protocol **767** inner server JAR has SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`;
official mappings have SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`.
This note used direct **JDK 21 `javap -p -c`**, not REA or a gameplay observation.
Artifacts and raw disassembly stay outside the repository.

| Mapped class | Original | Inspected methods and mapped source lines |
| --- | --- | --- |
| `AbstractContainerMenu` | `cpu` | `removed` 558–569; `clearContainer` 572–586; `transferState` 772–786 |
| `InventoryMenu` | `cqw` | `removed` 133–142 |
| `CraftingMenu` / `ItemCombinerMenu` | `cqm` / `cqx` | `removed` and captured access callbacks |
| `ServerPlayer` | `aqv` | `die` 694–748; `closeContainer` 1213–1215; `doCloseContainer` 1219–1223; `disconnect` 1374–1379; `restoreFrom` 1415–1467; `drop` 1846–1860 |
| `Player` | `cmx` | `dropEquipment` 719–724; `destroyVanishingCursedItems` 727–733; `drop` 747–793; `remove` 1450–1456; `hasContainerOpen` 1485; `getBaseExperienceReward` 1861–1869 |
| `LivingEntity` | `btn` | `dropAllDeathLoot` 1431–1439; `dropExperience` 1445–1448 |
| `Inventory` | `cmw` | `dropAll` 497–506 |
| `PlayerList` / `ServerLevel` | `aur` / `aqu` | `remove` 381–406; `respawn` 469–530 / `removePlayerImmediately` 953–954 |

## Removal policy is not the death lifecycle

`MenuCloseContext::{AliveConnected, DeadOrDisconnected}` in
[`src/container.rs`](../../src/container.rs) selects disposition **when removal
actually runs**. The source cursor predicate is
`!player.isAlive() || serverPlayer.hasDisconnected()`; the disconnected flag is
the explicit `ServerPlayer.disconnect` state. A client close packet, absent
outgoing packet, save operation or spectator flag does not establish that state.

For a dead/disconnected owner, `AbstractContainerMenu.removed` invokes
`Player.drop(carried, false)` and clears that menu's cursor. `clearContainer`
removes each actual temporary input with `removeItemNoUpdate`, in ascending
container order, and invokes the same overload. It forwards
`around=false, traceItem=false`. Neither method consults `keepInventory` or
spectator status. For an alive connected owner, the corresponding operation is
`Inventory.placeItemBackInInventory`, including its insertion packets and any
overflow drop; see the [existing close ledger](menu-close-lifecycle.md).

`ServerPlayer.die(DamageSource)` has **no** call to menu `removed`,
`closeContainer` or `doCloseContainer`. Do not infer an immediate all-menu close
at zero health from the shared enum. That method emits the death game event,
handles death-message delivery, removes shoulder entities, applies the
forgive-dead-players rule, and, for a non-spectator, invokes actual death loot;
its later score/stat/killer/entity-event/combat/last-death-location callbacks
also belong to the real death owner.

When `Player.remove(RemovalReason)` actually runs, it first invokes its
superclass removal, then **retained window0 `InventoryMenu.removed`**, then
`doCloseContainer` only if the current menu is a distinct external menu.
Thus the menu-owned drop order is window0 cursor, window0 inputs 0–3, external
cursor, then that external menu's temporary inputs, if any. Opening a block menu
from window0 did not remove window0; both menus can retain distinct cursors.
Do not traverse both as the same current menu or drain shared player slots twice.

`ServerPlayer.doCloseContainer` invokes current-menu `removed`, transfers that
old menu's shared slot baselines to window0, then selects window0. It does not
send a close-screen packet. `closeContainer` sends that packet **before** calling
`doCloseContainer`. Consequently the removal path above does not justify adding
a close-screen packet at an invented death phase. A separate source call to
`closeContainer` must retain its packet-before-removal order.

`PlayerList.respawn(oldPlayer, keepEverything, reason)` removes the old player
from its player list and calls `ServerLevel.removePlayerImmediately` (which
invokes `Player.remove`) **before** constructing the replacement and calling
`restoreFrom`. This establishes a real old-owner removal boundary, not a native
respawn implementation or a claim that every death first removes there.

## Exact callable removal APIs and publication

The shared workstation helper is
[`MenuSync`](../../src/workstations/menu.rs):

```rust
pub fn close_inputs_ordered(
    &mut self,
    inputs: &mut [ItemStack],
    inventory: &mut PlayerInventory,
    context: MenuCloseContext,
) -> Result<MenuRemoval>;

pub struct MenuRemoval {
    pub outcome: ClickOutcome,
    pub events: Vec<MenuRemovalEvent>,
}
pub enum MenuRemovalEvent { Packet { index: usize }, Drop { index: usize } }
```

`ClickOutcome` contains `packets`, `drops: Vec<DroppedStack>` and `crafted`.
`DroppedStack { stack, around, trace_item }` retains actual drop arguments.
The helper processes **its own cursor first**, then only the supplied owned
temporary inputs. It clears these owned stacks and installs the staged inventory
only on success. In the dead/disconnected branch it inserts nothing into the
player inventory. Packet/drop indices refer to that single outcome; execute
each indexed drop once, at its event position. The compatibility
`close_inputs(...) -> Result<ClickOutcome>` loses cross-stack packet/drop
interleaving and is insufficient as a general publication schedule.

Existing foundations expose these signatures:

- `PlayerContainer::close_with_context(&mut self, context: MenuCloseContext)
  -> Result<ClickOutcome>`: window0 cursor, discard result cache, four inputs.
- `CraftingTableMenu::close_with_context(&mut self, inventory: &mut
  PlayerInventory, context: MenuCloseContext) -> Result<ClickOutcome>`: external
  cursor, nine temporary inputs; discard its result preview.
- `GenericContainerMenu::close_with_context(&mut self, inventory: &mut
  PlayerInventory, context: MenuCloseContext) -> Result<ClickOutcome>`: cursor
  only. `FurnaceMenu::close_with_context` has the same parameters using
  `FurnaceMenuCloseContext`, an alias of the shared context type.
  The actual block container's slots are preserved.

New adapter entry points, without Main dispatch, are:

- `StonecutterMenu::prepare_close(&self, inventory: &PlayerInventory, context:
  MenuCloseContext) -> Result<PreparedStonecutterAction>` and
  `SmithingMenu::prepare_close(...) -> Result<SmithingActionPlan>`: cursor then
  one stonecutter input or smithing template/base/addition. Validate each plan
  before staging effects; its consuming `commit(self, menu: &mut ..., inventory:
  &mut PlayerInventory)` revalidates and returns its publication/outcome.
  `StonecutterMenuEvent::Drop { index }` refers to `publication.outcome.drops`;
  `SmithingMenuEvent::Drop { index }` refers to `outcome.drops`. Their `Sync`
  events own packet publication; do not independently drain packets/drops.
- `StorageMenu<K>::prepare_close(&self, owner: &impl StorageContainer<Key = K>,
  inventory: &PlayerInventory, context: MenuCloseContext)
  -> Result<PreparedStorageAction<K>>`: cursor only, then `StopOpen` against the
  captured real owner. Its plan validates/commits that same owner, menu and
  canonical inventory. `StorageMenuEvent::{Sync, Drop, StopOpen}` retain source
  phase observations; those snapshots are not additional owned stacks.
- `CrafterMenu<K>::prepare_close(&self, state: &CrafterState, inventory:
  &PlayerInventory, context: MenuCloseContext, callbacks: &impl
  CrafterMenuCallbacks<Key = K>) -> Result<CrafterMenuPlan<K>>`: cursor only;
  preserve all nine block inputs and discard the nonowned preview. The consuming
  plan validates/commits menu, actual block state, inventory and owner callbacks.
- `EnchantingMenu<S, K>::close<H: EnchantingMenuHost<Snapshot = S, Binding = K>>(
  &mut self, context: MenuCloseContext, host: &mut H) -> Result<()>`: validates
  binding, preflights `EnchantingMenuAction::Close(context)`, then stages cursor
  and its two inputs through `stage_sync_events` / `stage_drop`. The host must
  privately stage its inventory and all effects, and discard that stage on error.

Anvil/grindstone/loom/cartography kernels expose their documented input-drain
helpers; they do not perform a complete menu removal. Merchant
`prepare_close` clears the actual trading-player reference and returns only its
two owned payments after the host handles the cursor. Beacon similarly owns its
temporary payment. These require their family-specific callback adapters; a
generic input drain must not silently replace those callbacks.

Persistent storage remains with the real block/player/entity owner: chest,
barrel, furnace, brewing stand, hopper, dispenser/dropper, shulker and crafter
contents, plus the player's personal ender contents, are **not** menu close
drops. `stopOpen`, merchant detachment and other lifecycle callbacks remain
actual-owner operations. Preview results are derived, never extra items.

## Central sequencing, drops and outstanding death owners

The existing Main disconnect path is
`Server::cleanup_player_admitted` in [`src/main.rs`](../../src/main.rs):
`close_inventory_menu_with_context_locked(..., DeadOrDisconnected)`, actual
external furnace/crafting/native-container close, save, then actor removal.
It uses the existing [`main_drop_commit`](../../src/main_drop_commit.rs)
`menu_action_with_items` transaction with real Item runtime, player/world RNG,
entity-ID counter, gameplay/stat preparation and the native
`spawn_player_drop_with_factory` constructor path. This is bounded disconnect
composition, not the source's complete death, respawn or `PlayerList.remove`
lifecycle. In particular Java `PlayerList.remove` saves **before** its later
level/player removal; the current menu-first native cleanup is not proof of
that save order. The current native body path explicitly rejects a dead Player
without its real death owner.

Central sequencing must retain actual player/world/menu lifetimes while
preparing, validating and committing real owners. Publish each Item/world-add,
callback, listener and packet at its source phase through the **one existing
entity-event/output FIFO**. Use actual drop position/eye height, item components,
player RNG, Level RNG, entity RNG factory/UUID, IDs, pickup delay, world-add,
tracking and post-add statistics. A `DroppedStack` or preview clone does not
allocate another entity. A block-break `Containers.dropContents` call has
different RNG, splitting and pickup rules and cannot replace `Player.drop`.

Team 4 identifies central transport `1e72694` as the owner of actual send
counting, frame writes and rate termination. Enqueue callbacks must use that
owner, not add a writer. Precommit preparation can fail without publishing;
once a source mutation or output prefix has committed, a later terminal close
must not roll it back or recreate its drops on retry. These are distinct
transaction and connection-publication phases, not a promised atomic socket send.

The precise remaining central/Team 4 contracts are:

1. The canonical lethal-damage/death callback, actual alive transition and
   subsequent tick/removal/respawn phase, including death messages, combat,
   stats/criteria, shoulder entities and spectator gates. Do not implement death
   by merely choosing `DeadOrDisconnected`.
2. Real gamerule and inventory/enchantment owners. `ServerPlayer.die` skips its
   death-loot call for spectators. `Player.dropEquipment` calls its superclass,
   then, only when `keepInventory` is false, destroys actual vanishing-cursed
   items and invokes `Inventory.dropAll`. That visits persistent player main,
   armor and offhand contents using `Player.drop(stack, true, false)`, distinct
   from menu removal's `false, false`. Do not feed menu cursors/previews or
   persistent block inventories into that traversal.
3. Actual loot-table/custom-loot and XP-orb owners. `LivingEntity.dropAllDeathLoot`
   handles its loot/custom-loot rule branch, then equipment, then experience.
   Player base XP is zero with `keepInventory` or spectator, otherwise the
   source's level-based reward capped at 100. Loot/world/random callbacks and
   actual XP orbs cannot be replaced by a menu-level XP grant or a no-op host.
4. Exact save/checkpoint and old/new player ownership. `restoreFrom` has separate
   `keepEverything` and `keepInventory`/old-spectator branches for inventory/XP;
   personal ender storage survives through its actual player owner. Central
   respawn must own dimension/position, connection transfer, initialization,
   persistence and output order. A replacement player must not inherit another
   owned copy of already removed cursors or temporary inputs.

Unknown owner, identity-sensitive/reentrant callback or unsupported death
context must fail explicitly before dependent effects are published. Existing
focused normal/disconnect checks do not validate death or justify completion of
`inventory.4`, `crafting.2` or `crafting.5`.
