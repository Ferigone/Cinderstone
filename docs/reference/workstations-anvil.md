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
  levels, anvil costs and map encounter order. This stage explicitly rejects
  that branch and enchanted sacrifices; existing base enchantments survive
  supported renames/material repairs/unenchanted sacrifices.
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
input/cost changes once, then invoke `finish_anvil_take` against the captured actual owners in that order.
The prepared result is not a second owned stack. `close_inputs` returns only
input 0 and 1. Do not persist or return the derived output on menu closure.

## Integration status and limits

No Main block-use/click/name-packet dispatch or player/world adapter is changed
by this module. The central integrator must provide the actual repair/hover
owners, inventory/cursor transaction, player XP owner, player RNG, captured
block access, source world events, listener ordering and shared synchronization.
Partial result extraction, every click mode, enchantment combining, recipe-book
or advancement ownership and complete anvil gameplay are not claimed.

The focused native checks cover the expensive rename-only cap, material
consumption and discarded preview on close, and UTF-16/name filtering. They are
small native contract checks, not comprehensive conformance or Java runtime
observations. The fixed aggregate deliveries `inventory.4` and `crafting.5`
remain incomplete.
