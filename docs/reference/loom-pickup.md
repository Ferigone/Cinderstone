# Actual LoomMenu synchronization during item pickup

`main_loom_pickup.rs` implements
`LoomRuntime::broadcast_for_inventory(id,PickupPlayer,&PlayerInventory,
&mut MainGameplayFrame) -> Result<Vec<Packet>>`. It is called only by the
existing canonical inventory transaction at the real ServerPlayer.take menu
synchronization boundary, after Collect and before private packet publication.
The native Body route and level-player suppression now include Loom. Root owns
legacy/central Main routing and production verification; these source changes
alone do not prove that a deployed server reaches the callback.

## Source and retained context

Evidence uses the original Java1.21 inner server SHA256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`
and official mappings SHA1 `31c77994d96f05ba25a870ada70f47f315330437`,
inspected with direct JDK21 `javap`, not a REA MCP call.

ServerPlayer.take (`aqv.a`, original lines996–998) invokes the actual current
menu's broadcast. ServerPlayer's real container listener (`aqv$2`, lines278–287)
triggers InventoryChanged only for slots backed by that same canonical player
Inventory. Its dataChanged method (line292) returns without a gameplay action.
The kernel's pure `LoomMenu::broadcast_changes` checks its actual binding,
callback capability and stack context, then emits phased synchronization.
It does not inspect block validity, read world game time or execute a result
take. The later player menu tick owns fresh validity and closure.

The pickup bridge checks the actual entity ID and UUID with retained
`NativeLoomContext::verify_pickup_actor`. It uses that context solely for its
real captured world/player/registry/tag/feature/callback binding. It never
manufactures access=true, time0, a new registry or a new actor/inventory owner.
Taking a result, executing a button/click and checking stillValid still require
the primary owner's fresh actual context/access at their proper phases.

## Callback and publication ownership

Only a clone of the retained menu's native comparison/encoding state is staged.
Its same menu lifetime, owned inputs and separate cursor are preserved. The
canonical player inventory and actual gameplay hooks are borrowed, never
replaced with an InventoryMenu or cloned hook owner.

Every returned event must be Sync before any hook runs. Inputs, preview, cursor
and retained inventory are admitted and encoded; all41 actual backing stacks
are checked for the source criterion's maximum-stack query. Player-backed
listeners must map Loom menu slots4..39 to canonical player slots9..44. The
three inputs/result have no player backing. Data listener0 must retain its
actual selected-pattern phase; its local baseline is already updated by the
kernel and its original ServerPlayer callback has no inventory dirty action.

The same actual MainGameplayFrame executes each source-phased player inventory
listener with its corresponding `phase.player_inventory`. Packet output is
returned once to the existing inventory host for its private publication.
After successful validated callbacks the retained menu receives the staged
comparison/encoding state. It is not broadcast again through window0.

Pure broadcast has no Inventory.setChanged/source counter increment, result
take sound, crafted/recipe award, XP or drop action. Any non-Sync event is an
explicit error rather than silently discarding an effect. InventoryChanged
counters emitted by actual click/button operations remain the primary Loom
transaction's responsibility and are not duplicated by this pickup phase.

`main_player_body` now includes `looms` in its real current-owner exclusivity
check alongside furnace, stonecutter and smithing. It routes actual Loom pickup
through `with_external_menu_sync_and_hooks`, preserving its existing one
`frame_current` and actual loaded-region/actor/canonical inventory owners.
Other menu routes and actual XP pickup retain their existing owners. The
ordinary equipment/held-stack/inventory tick reads continue to use canonical
player inventory, not temporary Loom inputs or derived result.

`main_rain_context` includes Loom in the real external-owner count and in the
fallback guard that rejects active menus without their actual menu phase.
It captures current external ownership **before** invoking that player's
`menu_tick`. Invalidity may close the Loom during that phase; the retained
suppression still prevents a second inactive InventoryMenu broadcast later in
the same player entry. The existing shared Item/Player/XP tick order, actual
rain/region provider and non-Loom behavior retain their previous contracts.

Root still owns central runtime construction, legacy pickup selection and the
exact per-player phase call sites. No focused compiler/runtime result or whole
Loom/Minecraft compatibility claim is recorded before central assembly.
