# Grindstone: Java 1.21 source facts and native boundary

The reference is the unchanged Minecraft Java **1.21**, protocol **767** inner
server JAR, SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Official server mappings SHA-1 is
`31c77994d96f05ba25a870ada70f47f315330437`.
This stage used **JDK 21 `javap -p -c`**, not REA: `GrindstoneMenu` (`cqt`),
its `SimpleContainer` (`cqt$1`), input slots (`cqt$2`, `cqt$3`) and result slot (`cqt$4`),
`AbstractContainerMenu` (`cpu`), `Slot` (`crq`), `SimpleContainer` (`bra`),
`ResultContainer` (`crl`),
`EnchantmentHelper` (`dae`), `ItemEnchantments.Mutable` (`dai$a`),
`ItemStack` (`cuq`) and `Container` (`bqk`). Raw disassembly and proprietary
Java artifacts are outside the source checkout and are not redistributed.

## Computation and slot facts

- Menu type is 15, with input indices 0/1, result 2, player inventory 3–29 and
  hotbar 30–38. Both inputs accept a damageable item or an item with any normal
  or stored enchantments. The ordinary container maximum is 99, further bounded
  by the item's maximum. Counts above one in either input suppress the result.
- A single plain damaged item produces no result. A single enchanted item copies
  its component patch and removes non-curses from the crafting enchantment
  component: stored enchantments for an enchanted book, normal enchantments for
  other items. A different extra enchantment component is not silently removed.
- Two inputs must have the same item. Their maximum damage is the maximum of
  both values. Remaining durability is added with a five-percent integer bonus
  of that maximum; damage is clamped as `setDamageValue` does. Non-damageable
  pairs require maximum stack size at least two and identical components,
  and produce count two. The first item's unrelated component patch survives.
- Second-input curses only merge when the first item lacks that curse. An
  existing level-one curse is not upgraded by a second level-three curse.
  Non-curses are removed after merging. Tooltip visibility is inherited from
  the first crafting enchantment component. Repair cost resets to zero and
  applies `min(2L * cost + 1, Integer.MAX_VALUE)` once per retained curse.
  `cqt.removeNonCurses` calls `cpw.e`; this is a saturating long calculation,
  not wrapping integer arithmetic.
- An enchanted book without remaining curses becomes a normal book through
  `ItemStack.transmuteCopy`; this retains its component patch while changing
  the item prototype. Reconstructing a component-free book loses unrelated data.

## Take and lifecycle contracts

`GrindstoneState` owns exactly two inputs and one derived preview. `set_input`
recomputes atomically. `prepare_take(amount, registry)` does not consume an input
or RNG draw. Its token cannot be cloned through the public API, and commit
checks the exact captured inputs and preview. Ordinary `ResultContainer.removeItem`
ignores the requested amount and removes the **whole** result, including a
right-click or throw-one of a count-two result. `Slot.tryRemove` first rejects
a destination that cannot accept that whole count. The wrapper
`prepare_take(amount)` represents the actual destination transfer, with a partial
count possible during quick-move only. Quick-move consumes both inputs even
when only one of a count-two result fits; the remaining preview disappears. It
is not an additional owned item.

`commit_take` requires the real Level RNG callback. XP sums each non-curse's
actual registry minimum cost over both original inputs. Positive totals produce
`ceil(sum / 2.0) + Level.random.nextInt(ceil(sum / 2.0))`; bound one still draws.
Zero or overflow-negative totals do not draw and return zero. Creative mode has
no bypass. The source then awards actual experience orbs at the block centre
on the server, emits Level event **1042**, data **0**, clears input zero and
clears input one. Each clear runs `slotsChanged/createResult/broadcastChanges`;
the returned take record carries the intermediate and final previews for that
ordering. No item-crafted callback or recipe advancement is added here.

The owning adapter must stage the menu, actual Level RNG and actual orb/world
transaction together before publishing any of them. The native kernel does not
award player XP directly, create entities, invent a registry or bind curse tags.
`GrindstoneEnchantments` queries the actual holder/tag/minimum-cost owner and must
fail when unresolved. Closing drains only the two owned inputs, after the host
processes the actual carried stack. The preview is discarded. Validity requires
the real captured block/Level access and the ordinary distance check.

## Verification and integration limits

A focused native regression assigns32 actual registered enchantment holder IDs
to an explicitly labeled fixture curse tag and verifies the retained result
repair cost saturates at `Integer.MAX_VALUE`. This exercises the actual tag-owner
contract, not vanilla's ordinary two-curse tag or a Java gameplay flow.

A temporary local reflection harness invoked the **unchanged** `cqt.computeResult`
on an allocated menu object; it supplied vanilla registries and explicitly bound
the official curse tag. It did not simulate a Level, clicks or PLAY traffic.
Observed results were:

| Inputs | Java result |
| --- | --- |
| Single unenchanted iron sword, damage 200 | Empty |
| Iron swords damage 200/100; first Sharpness II + Vanishing I, second Unbreaking I + Vanishing III | Damage 38; Vanishing I; first tooltip visibility retained; repair cost 1 |
| Sharpness II enchanted book | Ordinary book |
| Two identical Vanishing I sticks | Two sticks, curse retained, repair cost 1 |

Focused native checks cover those source facts, an invalidated take token, the
bound-one RNG draw, a partial count-two take and close ownership. These checks
use a labeled fixture enchantment registry; that test fixture is not a production
world owner. Full compiler/runtime results belong in the team PR handoff.

## Native controller and exact callback phases

`container_workstation_grindstone.rs` supplies `GrindstoneMenu<Binding,
RegistrySnapshot>` over the existing single `GrindstoneState` preview and
`MenuSync` cursor. There is no retained live player inventory copy. All 36
player slots are borrowed through the host's canonical inventory: menu3–38 maps
to inventory9–44; SWAP additionally uses the actual offhand at45.

All seven ordinary modes are implemented: PICKUP, QUICK_MOVE, SWAP, CLONE,
THROW, QUICK_CRAFT and PICKUP_ALL. Result2 never accepts a placed stack or drag.
Creative CLONE deliberately copies up to the item's maximum without consuming
inputs or awarding XP, as the source does. No crafted-item/stat/recipe or
advancement callback is fabricated. Input-to-player quick move is forward;
result-to-player is reversed. Player-to-input tries0–1 whenever either input is
empty, with no hotbar fallback if that attempt fails. If both inputs are full,
main inventory and hotbar exchange. Nonempty compatible targets are merged by
`moveItemStackTo` before its empty-slot `mayPlace` checks.

The source `cqt$1.setChanged` first runs actual SimpleContainer listeners, then
`cqt.slotsChanged`. The latter calls the superclass broadcast **before**
recomputing the result, then `createResult` broadcasts the newly derived result.
The controller preserves both phases and every source Slot/container setter
notification; a zero-accept matching safeInsert still invokes the setters.
Signed safeInsert capacity for an overfull observed target is preserved. Result
container dirty calls are no-ops and cannot recompute a second result preview.

For normal PICKUP, the whole extracted result is on the cursor before onTake.
For SWAP to an empty hotbar/offhand, it is in that actual inventory slot first.
For THROW and PICKUP_ALL, safeTake's onTake runs before the caller drops the
item or grows the cursor. The controller reads the **current** inputs for XP,
so earlier PickupAll input removals cannot leave a cached XP award. The source
order is actual Level draw when positive, actual orb award at the captured block
centre (including the zero-amount call), event1042/data0, input0 container
setter with old/new preview broadcasts, then input1 setter and its broadcasts.
An item throw happens afterward. A single staged transaction must cover orb
allocation/merging and Player.drop's RNG and entity allocation together.

Outside-left PICKUP drops the whole cursor before clearing it; outside-right
splits one first. Drag completion requires the real Slot-identity HashSet's
encounter order from the host. One-slot creative drag recursively dispatches
PICKUP button2, which does nothing. Close follows removed/clearContainer:
cursor first, then removeItemNoUpdate for input0 and input1, returning each to
the actual inventory or dropping with traceItem=false. Removal does not award XP,
broadcast input changes or return the preview. The actual server close-window
packet precedes removal, and the central owner transfers old inventory sync
baselines to window0 afterward.

## Required real-owner transaction contract

`GrindstoneMenuHost` must bind the actual ServerPlayer and captured
ContainerLevelAccess Level/position/menu lifetime. A grindstone has no invented
block-entity identity. `registry_snapshot` captures the actual registry/curse-tag
view used by the item holder IDs; an owner change rejects the action. A checked
nonreused controller instance plus revision is required by `click` and `close`;
window reuse or structurally identical contents cannot revive an old token.
Public controller clones and replayable prepared result plans are not exposed.
Validity delegates the captured grindstone and actual player reach with +4.0.
The central packet/tick owner still performs the source selected-menu,
spectator, validity/close and disconnect dispatch checks.

`begin_stage` must privately stage **all** real owners and callbacks, not shallow
clone shared mutable world state. `publish_stage` validates the captured owner
revision and atomically publishes inventory, RNG, entities, ordered listeners
and packets; an error publishes nothing. The controller only replaces its menu
after successful publication. Host `preflight` rejects unavailable actual owners
and reentrant listener contexts. Read-only `GrindstonePhase` records the exact
callback entry inputs, preview, canonical inventory and cursor. The actual
Inventory.add/placeItemBack owner is responsible for its own internal mutation,
packet and overflow-drop phases; the mutable stack argument is the same logical
owned stack throughout that callback. SimpleContainer callbacks must retain
their real listeners, not substitute packets for dirty notifications.

The adapter rejects bundles and explicit food-component20 overrides before
click processing. Nested ItemStack identity in those FoodProperties values is
not established by equal wire bytes. Stable item prototype food values remain
supported. The host must reject unsupported mutable/reentrant listeners and
unknown drag identity/order; no fake curse registry, no-op world or direct player
XP replacement is supplied.

Focused native controller checks execute a restricted recording host with the
real native Level RNG algorithm. They cover all seven modes, whole count-two
right extraction and insufficient cursor capacity, partial shift with derived
leftover discarded, source old/new preview broadcasts, actual borrowed hotbar
mutation before XP, an orb failure rolling back inventory/menu/RNG together,
bound-one draw, XP-before-throw, outside cursor phases, stale/reused UI and
registry/player binding guards, and close returning only the two inputs. This
is a native controller scenario, not an original-Java click replay, production
world, PLAY runtime or complete conformance suite.

Main has no grindstone callsite in this stage. Actual enchantment/tag/minimum-cost
bindings, Level RNG/orb/event/drop transactions, canonical player callbacks and
server dispatch remain integration contracts. This bounded controller does not
complete `inventory.4`, `crafting.2` or `crafting.5`.
