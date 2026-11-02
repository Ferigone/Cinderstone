# Immediate smithing-menu pickup synchronization

`main_smithing_pickup::SmithingRuntime::broadcast_for_inventory` composes the
actual current smithing menu with the existing item-pickup host. It is a Main
adapter, not another inventory, recipe evaluator or menu initializer.

## Source evidence

The original Minecraft Java 1.21 server and mappings are pinned by
`fixtures/source-lock.json`. The inspected inner server SHA-256 is
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`;
official mappings SHA-1 is `31c77994d96f05ba25a870ada70f47f315330437`.
This stage used direct JDK 21 `javap -p -c`, not an REA MCP execution or a
Java graphical/menu runtime observation.

- `ServerPlayer.take(Entity,int)` (`aqv.a`) first delegates to the superclass
  collect callback and then calls `containerMenu.broadcastChanges` (`cpu.d`)
  immediately. It does not synchronize an inactive InventoryMenu instead.
- The installed server-player `ContainerListener.slotChanged` (`aqv$2.a`)
  checks the actual slot's backing container. Inventory-backed slots dispatch
  `INVENTORY_CHANGED` using the actual player inventory and changed stack.
- Smithing's inherited input-container callback (`cqx$3.e`) calls its base
  `SimpleContainer.setChanged` and then `ItemCombinerMenu.slotsChanged`
  (`cqx.a(Container)`). That method broadcasts through the superclass before
  recomputing the result for input-container changes. This input/result
  callback sequence is distinct from a pickup broadcast, which changes no
  smithing input and must not replay `createResult`.

See [the smithing source ledger](workstations-smithing.md) for take/input
phases, recipe ownership and mandatory inventory-change counter callbacks.

## Actual owner contract

```rust
SmithingRuntime::broadcast_for_inventory(
    &mut self,
    id: i32,
    player: PickupPlayer,
    inventory: &PlayerInventory,
    hooks: &mut MainGameplayFrame<'_>,
) -> Result<Vec<Packet>>
```

Main retains the real world/player/shared-container ownership and invokes this
through `RuntimeInventoryHost::with_external_menu_sync_and_hooks`. That host
checks the actual registered player's identity before entering the callback.
The adapter checks that the supplied player ID selects this current menu,
stages the existing menu's broadcast baselines, validates/encodes its complete
synchronization stream, and invokes the same concrete gameplay frame for every
inventory-backed slot listener in source order. Each listener receives its own
`SmithingPhase.player_inventory`; it does not observe an inferred later final
inventory. The frame scans the actual canonical inventory and dispatches its
certified criterion owner. Unknown/nonempty unsupported criterion sources fail
through the existing guards.

The original menu contains three inputs and one result, followed by player
slots. `MenuSync` preserves these mappings:

| Smithing menu slots | Backing canonical player slots |
| --- | --- |
| 0, 1, 2: template, base, addition | None |
| 3: result preview | None |
| 4–30: main inventory | 9–35 |
| 31–39: hotbar | 36–44 |

The menu keeps its separate actual cursor. The adapter borrows the canonical
player inventory and frame, and never replaces or clones those owners. Its
prepared menu copy is committed only after its fallible comparisons, encodings
and supported listener callbacks finish. Returned packets remain in their
source order for the existing private pickup publication boundary after
Collect; no deferred full-content resend or second packet owner is installed.

`Inventory.setChanged`/`getTimesChanged` is a separate mandatory source counter,
not a protocol state ID and not an inventory criterion observation. A pure
`broadcastChanges` does not invoke it. This adapter requires synchronization-only
events and rejects `PlayerInventoryChanged`, effect or drop events rather than
silently discarding them or adding an artificial counter increment. Smithing
action transactions must execute their explicit `PlayerInventoryChanged`
events through the real canonical inventory owner at each recorded phase.

## Integration and validation boundary

The module is prepared for root's next smithing integration stage and remains
unexported until that stage is composed. Both native body pickup and the legacy
pickup path must select this actual current-menu callback exclusively. The
later per-player tick still owns block validity and closure; pickup does not
move or duplicate that phase. No input, result, drop or RNG owner changes here.

`main_rain_context` now includes the actual smithing runtime alongside furnace
and stonecutter ownership. The entry point without an actual player-menu
callback refuses an open smithing menu. The production level-player phase
rejects multiple current external owners and captures whether its sole menu
needs that phase **before** invoking the callback. If the callback closes an
invalid menu, the following host must not broadcast InventoryMenu a second
time: the original player tick has already performed its one menu broadcast.
The existing host/frame is retained; this does not create another gameplay
context or execute a second menu phase.

`MainItemGameplay::default_block_use` already serves the actual smithing opening
path through the same active-UUID and certified empty-criterion owner. No new
smithing-only substitute criterion context is necessary. Spectator/full-state
resynchronization is a different source operation: its owner must use the true
full-state kernel wrapper, rather than treating this incremental pickup
broadcast or a new initialization as equivalent.

`git diff --check` passed. The subsequent full Main compiler and protocol runtime
checks belong to the integration coordinator after the real runtime and helper
modules are exported. No production invocation, comprehensive conformance,
aggregate checklist closure or progress percentage is claimed by this module.
