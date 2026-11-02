# Actual GrindstoneMenu item-pickup synchronization

`GrindstoneRuntime::broadcast_for_inventory(id,PickupPlayer,&PlayerInventory,
&mut MainGameplayFrame)` is the actual current-menu callback for the existing
canonical inventory transaction's ServerPlayer.take boundary. Collect already
belongs to the real Item pickup owner; returned private packets follow it.
Central Body/legacy routing is owned by the Main assembler.

The original Java1.21 ServerPlayer.take/current-menu and inventory listener
contracts were inspected directly with JDK21 javap and official mappings,
using inner server SHA256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
ServerPlayer$2.slotChanged checks actual player-inventory backing. A grindstone
has no menu data slots. The later player menu phase owns fresh block validity
and closure; item pickup does not replace it with an inactive window0 broadcast.

The primary `OpenGrindstone::prepare_inventory_broadcast` supplies a genuine
same-owner kernel staged copy and synchronization-only Host bound to the actual
retained actor/level binding and configured enchantment/tag snapshot. It reads
the canonical inventory supplied by the caller. Unsupported world/take/XP/drop/
RNG operations are errors, not dummy owners, samples or successful empty hooks.
No validity or clock is needed by this pure kernel broadcast.

The bridge verifies the actual PickupPlayer identity against that retained
owner, preflights every returned phase using the shared real sync validator,
and dispatches player-backed SlotListener events through the same actual
MainGameplayFrame with their exact `phase.player_inventory`. Grindstone menu
slots3..38 map to canonical player slots9..44; the two inputs and result have
no player-inventory backing. Actual data listeners are rejected before hooks.

Only after successful validated callbacks does the bridge replace the retained
menu's comparison/encoding state. It preserves its actual binding/registry and
temporary inputs/separate cursor. It never constructs a PlayerContainer, new
inventory, alternate gameplay frame, fabricated XP owner or Level RNG stream.
Pure broadcast increments no Inventory.setChanged counter and invokes no take
event1042, XP, drop, recipe or crafted callback. Source action counter/effect
phases remain with the primary transaction.

Main must choose exactly one current external owner and retain the same frame
through Collect, these callbacks and private publication. Its level-player
window0 suppression must capture actual current ownership before menu_tick, so
closing invalid access cannot cause a second broadcast in the same player
entry. Runtime snapshots observe the actual external cursor separately from
window0; inputs/result are not additional held/equipped inventory. No Main
production call or runtime-flow result is established by this source handoff.
