# Java 1.21 player-menu local listener contract

Pinned source: `../mc121-oracle/server-inner-1.21.jar`, SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
The unchanged original classes are loaded from this JAR. Reproduce with
`python3 tools/container_local_listener_generate.py`.

`AbstractContainerMenu` (`cpu`) owns separate `lastSlots` (`l`) and
`remoteSlots` (`o`). `addSlot` appends canonical `ItemStack.EMPTY` to both.
The listener baseline belongs to the menu, not to each listener. It advances
even if the listener list is empty.

| Original operation | Local baseline/listeners | Remote baseline/packets |
| --- | --- | --- |
| `a(cqh)` / addSlotListener | Deduplicates the listener object, adds it, then calls `d()` globally | Same `d()` call can emit remote changes |
| `d()` / broadcastChanges | Per slot in ascending order: compare, copy, update `l`, invoke all listeners in insertion order | Immediately after each local slot: compare/copy/update `o`, invoke synchronizer; cursor follows all slots |
| `e()` / broadcastFullState | Visits every local slot, reporting only differences; then local data slots | Calls `b()` after the local loop |
| `b()` / sendAllDataToRemote | Does not read/update `l` or invoke local listeners | Copies every slot, carried and data values, then sends Initial, including when suppression is set |
| `a(cqi)` / setSynchronizer | No local action | Assigns synchronizer, then calls `b()` |
| `a(int,cuq)` / setRemoteSlot | No local action | Stores `stack.copy()` only; no packet |
| `b(int,cuq)` / setRemoteSlotNoCopy | No local action | Stores the supplied reference only; no packet |
| `h()` / suppressRemoteUpdates | Local comparisons and callbacks still run | Incremental slot/carried/data synchronization returns early |

The local comparison helper `cpu.a(int,cuq,Supplier)` stores the supplied copy
in `l` before invoking any callback. `d()` gives the local and remote helpers
the same Guava memoized copy supplier. Consequently the two baselines can hold
the same copied object after one incremental broadcast. `e()` performs its
local copies first, then `b()` makes independent remote copies. A nonempty
copy is independent from the live inventory stack. Every `ItemStack.copy()`
of an empty stack returns the canonical EMPTY object, even when the live
object still retains its item and component identity at count zero.

Original `ServerPlayer$1` (`aqv$1`) constructs actual `acp` Initial (0x13) and
`acr` Slot (0x15) packets and advances the menu's 15-bit state ID. The oracle
captures their original stream-codec payloads at the network boundary.
Original `ServerPlayer$2` (`aqv$2`) ignores `ResultSlot` (`crm`); otherwise it
invokes INVENTORY_CHANGED only when `slot.container == player.getInventory()`
by object identity. Actual `InventoryMenu` produces:

| Menu slots | Inventory backing slots | INVENTORY_CHANGED |
| --- | --- | --- |
| 0 | Result container | No |
| 1..4 | Crafting container | No |
| 5..8 | 39,38,37,36 | Yes |
| 9..35 | 9..35 | Yes |
| 36..44 | 0..8 | Yes |
| 45 | 40 | Yes |
| Cursor | No slot listener | No |

The prediction setters update only the remote baseline. A correct predicted
slot can therefore suppress its Slot packet while the original local listener
and INVENTORY_CHANGED gate still observe the change. Full remote synchronization
can also leave a stale local baseline until the next incremental/full local
broadcast. A newly added second listener receives no independent initial
snapshot if the menu's local baseline already matches.

The actual oracle covers 105 histories / 2,013 operations. Its 102 histories /
1,985 operations marked `native:true` use the native single-listener contract.
The remaining histories separately audit multiple listeners/removal, explicit
suppression and no-copy remote aliasing. They do not claim those APIs exist in
the native server. `last_remote_same_reference` and retained callback values
record Java's copy/alias evidence; native Rust comparison uses values and
canonical empty representation, without asserting Rust object identity.

Environmental boundaries: the existing `container_oracle` InventoryMenu
subclass disables recipe-result recalculation and supplies the equipment hook.
The local observer forwards its actual callback to an unchanged `aqv$2` object.
Only the `an.f` criteria object is temporarily replaced by a `cg` subclass
capturing trigger invocation, then restored in `finally`; no shipped class
bytecode is rewritten. Advancement predicates/rewards are outside this proof.
The observer does not mutate stacks or reenter the menu. Default stacks and
simple damage/name patches are included; arbitrary complex component
`Object.equals` behavior remains a separate compatibility boundary.

Bytecode evidence: `runtime/container-local-listener-bytecode.txt` contains
unchanged `cpu` and `aqv$2`. `cpu`: add listener 0..29; `b()` copies slots
0..46, carried 47..57, data 58..107 and Initial 108..142; `d()` visits each
slot's local helper before its remote helper, then carried; `e()` visits
local slots and data before `b()`;
local helper compares `ItemStack.matches`, updates `l`, then iterates listeners;
remote helper begins with the suppression guard. `aqv$2.a(cpu,int,cuq)` checks
ResultSlot at 7..15 and inventory identity at 16..28 before trigger at 31..46.
