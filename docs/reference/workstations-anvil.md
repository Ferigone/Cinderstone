# Anvil result and ownership contract

Source: Minecraft Java **1.21**, protocol **767**. The unmodified inner server
JAR SHA-256 is `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`;
official server mappings SHA-1 is `31c77994d96f05ba25a870ada70f47f315330437`.
The methods below were inspected directly with **JDK 21 `javap -p -c`** in this
session. This is bytecode evidence, not REA execution or a Java gameplay result.
Raw Java artifacts and disassembly stay outside the repository.

## Source facts

- `AnvilMenu` (`cpw`) `createInputSlotDefinitions` accepts every item into its
  two input slots, indices 0/1; result index 2 rejects insertion through
  `ItemCombinerMenu$2`. The protocol MenuType ID is 8. One DataSlot stores cost.
- `createResult` first requires a nonempty input with the effective enchantment
  component selected by `EnchantmentHelper.getComponentType`: stored
  enchantments for the actual enchanted-book item, ordinary enchantments
  otherwise. This checks presence, rather than whether enchantments are empty.
- Prior-work costs from both inputs add as Java longs. Material repair removes
  up to `maxDamage / 4` damage per material and costs one level per material
  consumed. Same-item sacrifice repair adds both remaining durabilities plus
  `maxDamage * 12 / 100`; an improvement costs two levels. Material validity is
  the actual item's `isValidRepairItem` callback, not a menu-local item list.
- Addition `STORED_ENCHANTMENTS` **presence** selects the source's book branch.
  Enchantment merging uses actual holders, compatibility/applicability, max
  levels, anvil costs and map encounter order. The native owner-backed branch
  below implements those rules; an absent actual owner explicitly rejects the
  branch. Existing base enchantments also survive supported renames/material
  repairs/unenchanted sacrifices without inventing a registry.
- `setItemName` filters each Java UTF-16 character with
  `StringUtil.isAllowedChatCharacter` (exclude U+00A7, values below U+0020 and
  U+007F), then rejects filtered strings longer than 50 UTF-16 units. The blank
  predicate accepts both `Character.isWhitespace` and `isSpaceChar`. Renaming
  compares the requested name to the actual resolved hover text. Removing an
  existing custom name or changing its displayed name adds one level.
- Result cost clamps prior-work plus operation cost to `[0, Integer.MAX_VALUE]`.
  Rename-only operations cap at 39, retain prior-work cost, and remain possible
  on expensive inputs. Other operations at 40 or more have no survival result.
  Increased prior-work is `min(2L * old + 1, Integer.MAX_VALUE)`.
- `onTake` charges the real player's levels unless `instabuild`, clears input 0,
  decrements only the recorded material amount (or clears input 1), resets cost
  to zero, then invokes the captured `ContainerLevelAccess` world callback.
  The callback checks the **actual current block** against the anvil tag. Only
  a non-instabuild player at an actual anvil consumes
  **`Player.getRandom().nextFloat()`**, with wear probability `< 0.12F`.
  Damaging the last anvil state removes it with `destroyBlock(pos, false)` and
  level event 1029; an ordinary damage uses setBlock flags 2 and event 1030.
  Non-damage paths also emit 1030. This RNG belongs to the player, not the level.
- `ItemCombinerMenu` (`cqx`) `stillValid` calls the subclass block predicate,
  then the actual player's `canInteractWithBlock(pos, 4.0)`. Closing clears only
  its owned input container; the result preview is not another owned item.
  Shared cursor/return/drop rules remain in the existing menu lifecycle.

## Implemented native boundary

`src/workstations/anvil.rs` implements name validation, supported result
computation, costs, material consumption, staged entire-result transactions and
the source wear/event callback through `AnvilWorldAccess`. The latter requires
the captured actual block and actual player RNG; preparing a result consumes
no randomness.
`AnvilContext` requires the actual item repair-rule owner and hover-name owner;
it supplies no fabricated registry or translation fallback. A missing
implementation is an integration boundary.

`prepare_take` observes current actual player levels/abilities and mutates no
input, XP, RNG, block or cursor. Its plan is only a proposed transfer;
`validate_take` must run before any real-owner changes. The host must reserve the
**entire** derived output in the destination, apply the level delta, commit
input/cost changes once, then invoke `finish_anvil_take` against the captured
actual owners in that order. This standalone whole-result plan remains available;
the menu controller below additionally handles the source partial-shift branch.
The prepared result is not a second owned stack. `close_inputs` returns only
input 0 and 1. Do not persist or return the derived output on menu closure.

## Native ordinary-menu controller

`src/container_workstation_anvil.rs` adds a bounded `AnvilMenu<K>` over the same
`AnvilState`; result computation remains in a single kernel function. Its
layout is the actual two input slots 0/1, derived result slot 2, player slots
3–38 (canonical inventory slots 9–44), and one level-cost DataSlot. It exposes
initialization, broadcast, rename, all seven ordinary click modes, close,
validity and inventory-baseline transfer. The caller supplies the actual
`AnvilMenuHost` transaction; no production default host is installed.

The new narrowly additive kernel hooks distinguish raw stack mutation from a
source container notification and identify `createResult`'s normal tail. Direct
`javap` inspection of `ItemCombinerMenu.slotsChanged` (`cqx.a(Container)`) shows
that it calls superclass `broadcastChanges` **before** recomputation.
`AnvilMenu.createResult` (`cpw.m`) broadcasts again at offsets 1066–1069 only
when it reaches its normal tail. Its early returns do not broadcast again.
Its empty/missing-enchantment first-input return precedes resetting the source
repair-material count, which the observed kernel path now preserves. The kernel
also emits source-granular field observations: cost becomes 1 before the first
input guard, the material count resets to 0 at `cpw.m` offset 133 before the
actual repair-item predicate at offset 172, and the completed material count is
assigned at offset 284 before the later hover-name callback. The controller's
mandatory observed-context wrapper forwards these changes to the actual phase
owner; a previous two-material preview cannot leak into the repair predicate or
a subsequent one-material hover-name callback. Ordinary
`Slot.setByPlayer` retains both the input-container and subsequent slot
`setChanged` boundaries; player inventory dirty callbacks remain distinct from
menu slot listeners and packets.

Result ownership follows `ResultContainer.removeItem` (`crl.a(int,int)`), which
ignores the requested amount and removes the entire derived stack. The inherited
`Slot.tryRemove` (`crq.a(int,int,Player)`) rejects a destination whose maximum
cannot hold that entire result because the result slot disallows modification.
Consequently right-clicking an empty cursor onto a renamed stack of 64 takes
all 64, rather than half. `ItemCombinerMenu.quickMoveStack` can insert only part
of a result, then still calls `onTake`: that consumes the inputs, charges the
levels and discards the remainder when clearing input 0 recomputes the preview.
The controller preserves this source branch instead of manufacturing a second
owned remainder. Input quick-move accepts every item and attempts input 0 then
1, as the inherited `canMoveIntoInputSlots` returns true.

Unlike SmithingMenu and StonecutterMenu, **AnvilMenu does not override
`canTakeItemForPickAll`**. The inherited AbstractContainerMenu method returns
true, so matching affordable anvil results participate in PICKUP_ALL, subject
to the whole-result capacity rule. CLONE does not invoke result `onTake`; THROW
and empty-hotbar/offhand SWAP do. There is no fabricated item-crafted, recipe,
statistic or advancement callback: `cqx$2.onTake` delegates directly to the
anvil's actual XP/input/world method.

Result `onTake` stages actual `Player.giveExperienceLevels` unless instabuild,
then the first input-container clear and its previous-preview/cost broadcast,
then the second input's recorded material decrement/clear and notification,
then cost reset, and finally `finish_anvil_take` against the actual player RNG
and captured current block. Cursor installation precedes result onTake for
PICKUP; PICKUP_ALL grows its cursor after safeTake/onTake. Output shift inserts
into the actual borrowed inventory before XP/input/world callbacks. The host
receives read-only `AnvilPhase` views at callbacks rather than replaying them
against the final menu. These views are observations, never inventory or
result owners.

`ServerGamePacketListenerImpl.handleRenameItem` (`aru.a(ahr)`) checks that the
current menu is an anvil and still valid, then invokes only `setItemName`. It
adds no unconditional broadcast. The controller applies the same block/reach
gate and relies on the recomputation tail for rename synchronization, including
the no-broadcast early-return case. Clicks use shared MenuSync state-ID,
client-prediction and stale-state correction. Active-menu and captured
player/Level/position/menu binding checks reject a substituted session. A
nonreused checked native instance plus revision token is required by click,
rename and close; protocol window reuse or structurally equal item contents do
not revive a stale token. Public controller cloning is not exposed. Successful
publication advances the native revision and verifies that the captured actual
binding survived callbacks. This token is an internal dispatch guard, not a
replacement for the wire state ID. The host must provide the actual
actor/window/spectator and owner preflight.

Closing returns the cursor first and only the two owned input stacks in order,
using the existing native return/drop engine and its per-insertion raw inventory
packets. Input removal does not recompute the derived preview during those
source callbacks. The preview and cost are discarded once closure finishes;
no result is returned or dropped. Dead/disconnected drops retain the original
cursor/input phases. Alive close insertion and overflow observations use the
same canonical staged player inventory, with scratch snapshots only to replay
the shared engine's ordered packet boundaries.

## Integration status and limits

No Main block-use/click/name-packet dispatch or production player/world adapter
is changed by this milestone. The central integrator must bind the actual
repair/hover owners, inventory/cursor transaction, player XP, player RNG,
captured block access, actual container listeners, world events and shared
synchronization. `AnvilMenuHost` borrows one canonical player inventory and
requires an actual player/Level/position/menu binding. The host and native menu
must be staged together: **any error discards the entire host stage and menu
stage before publication**. Private controller copies exist only during that
staging; the caller must retain actual canonical menu ownership through the
same transaction. Reentrant listeners and item click overrides such as bundles
are explicit unsupported contexts. Before any equality-dependent operation the
controller also invokes the shared `require_component_context` guard on actual
inputs/result/cursor/player stacks and, for clicks, client prediction stacks.
An explicit FoodProperties component 20 override with unresolved nested
ItemStack reference identity is rejected before transfer, callbacks, packet
baseline mutation or close. Stable prototypes and the original codec's proved
primitive-only prototype elisions remain supported; equal wire bytes alone do
not establish Java nested-stack identity. Multi-slot drag additionally
requires the actual selected Slot-identity HashSet encounter order.

Enchantment combining is callable only through the actual-owner capability
below. Its default is explicitly unavailable, so existing basic hosts still
reject stored-enchantment and enchanted same-item sacrifices rather than
fabricate a registry, compatibility, repair list or translation fallback. A
production enchantment registry/map/component adapter remains pending alongside
Main integration. Complete anvil gameplay and all-menu compatibility are not
claimed.

Focused native flows execute the new adapter rather than an unused test filter.
They observe repair cost/materials, old-preview and cost callback phases,
shared-inventory shrink before input callbacks, cursor/XP/player-RNG/block
ordering, whole-stack right pickup, partial output shift, result PICKUP_ALL,
ordinary drag/SWAP/CLONE/THROW, disconnected close without returning a preview,
initial packet layout, rename's early-return publication, native material-count
reset/assignment at actual repair and hover callbacks, unresolved food in actual
and prediction stacks, stale/reused/closed UI tokens, session/block refusal and
complete host-stage rollback for unsupported enchantment input.
The existing kernel checks retain the expensive rename-only cap, material
consumption, stale unsupported input atomicity and UTF-16 filtering. These are
native contract checks, not Java gameplay or comprehensive conformance runs.
The fixed aggregate deliveries `inventory.4` and `crafting.5` remain incomplete.


## Actual-owner enchantment combination increment

The same native computation now supports the `cpw.m` enchantment branch at
bytecode offsets 434–758 through `AnvilEnchantmentSession`. No second result
computation or preview is introduced. `AnvilContext`'s new capability defaults
to **None/unavailable**, preserving existing public callers and their explicit
unsupported boundary. The menu captures the actual enchantment snapshot at
opening and validates it before and after every action; a newly appearing,
disappearing, foreign or reloaded owner cannot silently replace it. Owner
changes must be handled by the central transaction while preserving owned
inputs, not retried with fabricated definitions.

The provider returns opaque actual holder/snapshot references, copies the base
`ItemEnchantments.Mutable` and its tooltip state into scratch ownership, and
supplies actual incoming entries, current keys/levels, holder equality,
definitions, supported-item membership, both exclusive sets, exact level setters
and selected-component serialization. It must reject foreign/unresolved values.
The scratch map owns no live input, player inventory or output. Queries consume
no RNG and do not mutate live owners. The provider's write-back replaces only
the chosen enchantment component, preserving unrelated output components and
base tooltip visibility. There is no production provider or fixed default
rarity/cost table in this milestone.

A single incoming entry is admitted without guessing an encounter order.
Multiple entries require the provider to certify the actual holder/hash-map
encounter order; a boolean default or protocol-ID/BTree/wire ordering is not a
source certificate. The native guard rejects uncertified multi-entry contexts.
The actual current mutable-map key order is also supplied after each setter.
This matters when incoming enchantments conflict: accepted earlier entries can
reject later entries, and the stack-count cost rule can also make order visible.

For each entry, native code reads the existing level, chooses incoming+1 for
equal levels or max(existing,incoming) otherwise, and clamps an accepted level
to the actual definition's maximum. `Enchantment.canEnchant` (`dac.c(ItemStack)`)
checks `supported_items`; creative or an exact enchanted-book base overrides
that applicability result. Neither overrides compatibility. Each incompatible
different current holder adds one level to operation cost, even when the
incoming entry was already inapplicable. A rejected entry records rejection;
an accepted entry uses **Mutable.set**, not upgrade/max(old,new), so an overlevel
base can be reduced to its definition maximum. `dai$a.set` removes nonpositive
levels and otherwise puts min(level,255). Normal decoded definitions require
max_level 1–255 and a nonnegative anvil_cost.

Each accepted entry adds `anvil_cost * accepted_level`, retaining Java signed
integer wrapping. Addition `STORED_ENCHANTMENTS` **presence**, even empty or on
a non-book item, changes its multiplier to max(1,anvil_cost/2), bypasses the
same-item requirement and suppresses the durability-combine bonus. However,
`EnchantmentHelper.getEnchantments` (`dae.b/d`) still reads stored component 23
only for the exact enchanted-book item and ordinary component 9 otherwise.
Thus presence and selected map type must not be conflated. If base count is
above one, each accepted entry sets operation cost to 40; subsequent rejected
entry penalties may increase it again. All-rejected with none accepted returns
empty/cost zero **before rename**, discarding any previous durability-work cost.
No curse/treasure restriction, primary-items test, enchanting-table tag, weight,
minimum/maximum enchantability, recipe, advancement or preview RNG is added.
Compatibility is `dac.areCompatible`: different holder identity and absence from
both actual exclusive sets.

Rename, long prior-work addition, operation-cost wrapping, total clamp,
rename-only cap39, survival result refusal at40+, saturating prior-work increase,
XP/input consumption and actual player-RNG/block callbacks retain the existing
source path. Final `dae.setEnchantments` writes only the output-selected 9/23
component after the prior-work component is set and only for a nonempty result.
The basic no-owner path continues to preserve existing enchantments for repairs
and rename without parsing holders it does not own.

Focused native fixtures use actual registered IDs and the pinned original
sharpness/smite/unbreaking resource max-level/anvil-cost facts. Their restricted
item/exclusive sets and ordered maps are explicitly test fixtures, not production
bindings, Java Fastutil observations or a gameplay oracle. Checks cover book
cost/level/tooltip behavior, lowering an overlevel base via exact set, book-base
applicability override, stored-presence versus exact-item map choice, creative
compatibility refusal, 40+ survival/creative behavior, uncertified multi-entry
rejection and certified conflicting-order outcomes. An adapter flow uses that
restricted owner through actual menu input/cursor/XP transitions and rejects
registry-epoch or capability drift without publication. No Main callsite or
complete enchantment gameplay is claimed.
