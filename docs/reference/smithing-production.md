# Native Smithing production boundary

**Runtime status: focused actual Main flow passed.** The native SmithingMenu is
connected to Main at source `a4d5b460de69c1380c12323fe5dbedd77581f0d0`, tree
`e12ecb9193bc70c68b055551c51afcaba37687a3`, targeting Minecraft Java **1.21**,
protocol **767**. The full Main executable build passed. The actual offline
loopback flow completed at `2026-11-01T22:05:07.066206+00:00` on binary SHA-256
`d6e1df77a4698c763692c5c282028f55494d2724d89537d1dc96e71949c1df88`.
This document establishes the bounded production paths and observations below;
it does not complete the aggregate inventory/crafting checklist delivery or
establish full vanilla compatibility.

Earlier source ledgers describe their initially unexported adapter stages. This
later document records their actual Main assembly at the exact commit above.

## Main entries and canonical owners

| Production entry | Actual owner and operation |
| --- | --- |
| `main.rs`, `LivePlayers.smithing_tables` | Retain `SmithingRuntime`, whose `OpenSmithing` owns the captured block position and menu |
| PLAY block use `0x38` | `use_native_smithing_table` reads the actual block/actor/hand/reach context, retires a prior external menu and allocates the shared native window counter |
| Container click `0x0e` | Admit actual Slot predictions, then dispatch `click_native_smithing_table` before other external owners and inactive InventoryMenu fallback |
| Container close `0x0f` | `close_current_player_menu` captures and removes the sole actual current owner under one operation/world/player interval; the wire window does not choose it |
| Actual player menu phase | `tick_native_smithing_menu_for_player_locked` broadcasts first, then checks the captured block and reach and closes invalid access |
| Actual item pickup | Native body and existing connection pickup paths select `broadcast_for_inventory` for the current SmithingMenu; they retain the existing gameplay frame and canonical inventory |
| Player capture | `inventory_snapshot` observes the canonical player inventory with this external menu's separate carried stack |
| Actual removal/reconfiguration | Remove retained InventoryMenu first, then Smithing with `DeadOrDisconnected`, save, and remove the player |
| `Server::save` | Ordinary autosave retains live menus; only the `shutting_down` branch retires external menus before the final checkpoint |
| Rejected component candidate | `resync_current_player_menu` selects the actual retained Smithing owner and its genuine full-state operation |

The real `SharedContainer` owns the canonical player inventory. Smithing owns
three temporary inputs and one external cursor; slot 3 is a derived result
preview. MenuType is 21, with 40 slots: inputs 0/1/2, result 3 and canonical player
slots 4..39. The retained InventoryMenu keeps its own cursor and 2x2 inputs.
There is no smithing block entity, second player inventory, connection writer or
independent entity-ID allocator. Smithing has no menu button action.

## Supported native profile

Mutable operations require the certified native flat-world source and an actual
alive, unmounted creative Overworld actor under the existing supported body and
lifecycle contracts. The source spectator path is read-only: clicks request the
real full state. That branch is present in source but was not exercised by this
creative flow. Survival, mounted actors, unsupported effects, imported unknown
sources and arbitrary custom actor profiles are outside this evidence.

`NativeSmithingRecipes` reads the same unique immutable feature packet sent by
Main and binds the packaged recipe/item data through
`PinnedDefaultRecipeContext`. It preserves the separate captured constructor
input/shift-routing order and result-selection order under default
`minecraft:vanilla` features. It does not invent recipe encounter order or
substitute an unknown recipe/trim registry. The callback capability list covers
the captured original base/result items and AIR, with original Item callback
ownership and required default features proved in the source ledger.

Actual gameplay must certify the native empty-criteria floor before callbacks
are prepared: unsupported registered advancement predicates/rewards, statistic
objectives, listeners or reentrant item hooks are not silently ignored. Missing
imported files alone do not prove empty registries. Ordinary block opening
awards its actual interaction statistic, reads post-use world state and calls
the existing default block-use criterion owner. Spectator provider handling
precedes those ordinary effects.

Reach uses the actual eyes/pose, captured block AABB, default interaction range
and source verification buffer. Applicable equipped block-interaction-range
modifiers require an unavailable attribute-composition owner. Typed
`UnsupportedMenuReach` retires the actual connected menu during tick, queued
click or refusal recovery, preserving owned stacks. Other internal failures are
not converted into that profile boundary. None of those modifier activation
paths was runtime-exercised by this Smithing flow.

## Source phases, callbacks and publication

Every `SmithingMenuEvent` retains its own `SmithingPhase`: three inputs, current
derived result, canonical player inventory observation and separate cursor.
Input-container changes broadcast the changed inputs with the previous result
before recomputation. The kernel converts `BeforeCreateResult` into the ordered
Sync events at that boundary; the Main callback adapter rejects an escaped
marker instead of executing a second recomputation or collapsing phases into a
final image.

Normal taking preserves the source effect order: `ItemStack.onCraftedBy` and its
actual crafted amount, recipe-crafted with the three pre-consumption inputs,
recipe-book award, input decrements in order, then level event 1044/data 0. Actual
`ServerRecipeBook.add_recipes` state/checkpoints and packets remain at their
recorded event positions. Player-backed slot listeners receive their own phase
inventory. Temporary input/result listeners do not impersonate player slots.

`PlayerInventoryChanged` independently calls `set_changed()` on the real
canonical `PlayerInventory`, preserving its transient signed wrapping counter.
It is not a protocol state ID or a substitute inventory criterion. Full result
QuickMove has source code for the emptied-stack AIR/count 0 crafted callback,
without Stonecutter's additional preliminary Item callback; it was not executed
by this Smithing runtime flow.

Main prepares encodings, recipe/feature/callback owners, actual menu instance and
revision, canonical inventory, drop indices, statistics and real chunk/rain
facts before committing. It revalidates the plan before consumption. The
infallible replay installs actual statistic/book checkpoints, counter changes,
listener calls and packets in source event order. This bounded staging relies
on the certified empty-criteria profile; it does not reproduce arbitrary
exceptional, packet-publishing or reentrant Java callbacks.

Nonempty owned drops move once into the existing real ItemEntity preparation
path using the actual player RNG, Level.random, random factory and shared
entity-ID owner. `PreparedItemBatch::commit_with_world_random_ordered` publishes
each entity at its indexed Drop event through the existing FIFO; callbacks and
menu packets remain between those additions. Drop statistics advance after
world-add. Server CloseScreen precedes removal publication. Removal handles the
owned cursor before input 0, input 1 and input 2; the derived result is never an
additional return, drop or persisted item.

Immediate pickup preserves Collect-before-menu handling and the actual listener
encounter order. Its private packet batch is returned after callbacks. This is
supported under the empty-criteria owner; it does not prove packet/listener
interleaving for mutating or packet-publishing hooks. The later player phase
retains its separate broadcast/validity obligation. Immediate pickup was not
runtime-exercised here.

## Level event and RNG contract

Smithing sends `ClientboundLevelEventPacket`, protocol packet 0x28, event 1044,
data 0 and `global=false` at the captured **integer block X/Y/Z**. Main selects
actual same-dimension players with squared distance strictly less than 4096
(radius 64) from that integer origin, including the actor when in range. This is
the original `ServerLevel.levelEvent`/`PlayerList.broadcast` boundary, not the
centered sound-packet audience used by Stonecutter.

This server level event consumes no sound seed and no additional sound RNG
draw. Client event playback is separate from server sound-packet generation.
The actual runtime flow observed the actor's packet at 10,64,8; it did not verify
multi-player radius or dimension-edge audiences.

## Admission and genuine full-state recovery

PLAY CreativeSlot and ContainerClick predictions pass the shared component
admission guard before candidate stacks/predictions enter canonical state.
After the existing proven normalization, surviving explicit FOOD20 requires
its unresolved source nested-ItemStack reference owner and is refused with
typed `UnsupportedFoodIdentity`; inherited prototypes remain admissible. The
guard also visits nested Slot values in components 29/30/52. This is a defined
unsupported custom-identity boundary, not a fabricated codec or equality rule.

On refusal, Main logs `native_component_admission_refused` and calls
`resync_current_player_menu`. Its operation/world/player interval selects the
sole actual current menu; Smithing's `Resync` requests full state from that
retained menu with its real revision, inputs, preview, player slots and cursor.
It does not execute a dummy click, reinitialize a menu or publish inactive
window 0 as an external-menu correction. Source wiring is present at the pinned
commit. The separate Stonecutter FOOD flow proves its Stonecutter branch;
Smithing refusal/full-state recovery and nested-component admission were not
runtime-exercised by this transform flow.

## Actual focused runtime evidence

The [sanitized report](../validation/smithing-flow.json) records independent raw
protocol framing, compression, Slot and NBT observations. The private harness
SHA-256 is
`d99a714f54a855a4987568cc519573eff17543d22bfa1a0ada36ce695f955d8e`.
It waited for actual spawn-chunk publication and batch acknowledgement before
block interaction, rather than treating teleport acknowledgement as chunk
readiness. The scenario began with fresh synthetic offline data and deliberately reused
its native checkpoints for cold restarts, with the exact source and binary
identified above.

| Actual production observation | Result |
| --- | --- |
| Place/use real SmithingTable | OpenScreen type 21, 40 slots |
| Pinned `minecraft:netherite_sword_smithing` | Template 1269 + diamond sword 838 + ingot 816 produce netherite sword 843, count 1 |
| Load actual inputs and take once | Input counts 2/1/2 became 1/0/1; cursor owned one output; derived preview became empty |
| Actual callback packet | Level event 1044 at 10,64,8,data 0,globalfalse |
| Client close with wire window 0 | Actual current Smithing closed; inventory held template 1, spare base sword 1, ingot 1 and output 1; cursor empty |
| Crafted statistic and native save | Netherite-sword crafted count 1; same player counts, no corresponding world drops |
| Cold restart with actual PLAY actor | Restored the same canonical output and remainders |
| Reopen with inputs 1/1/1, disconnect without taking preview | Saved player retained output 1; real item checkpoint held template 1, base sword 1, ingot 1, output 0 |
| Another restart without PLAY actor | Accepted that nonempty native item checkpoint and retained exact counts |

Each initial base sword occupied its own legal max-stack-size 1 player slot.
The scenario began with two templates, two base swords and two ingots, performed
one recipe, and conserved one remaining set plus one owned result. No derived
preview was duplicated. Every isolated process completed the actual SIGINT
shutdown path with exit 0. Private worlds, player identifiers, raw traces, logs
and binaries are excluded from the public report.

The last no-PLAY restart proves checkpoint acceptance and count retention; it
does not prove restored entity tracking/ticking readiness. The native save flow
does not establish Java player.dat/world interoperability, arbitrary crash
recovery for live temporary menus, or ordinary autosave timing. Armor trims,
custom component semantics, QuickMove/drag/overflow variants, immediate pickup,
queued-range retirement, spectator behavior, public/graphical clients and
multi-player event audiences remain unproven by this runtime witness. Unknown
custom feature/recipe/registry contexts and reentrant hooks remain unsupported.
No full workstation, performance or aggregate checklist-delivery claim follows.

The source evidence uses direct JDK 21 `javap` and bounded original-class and
registry inspection, not REA MCP execution. The original 1.21 server and official
mappings are pinned in [source-lock.json](../../fixtures/source-lock.json).
See [transactions and event proof](smithing-transactions.md),
[immediate pickup](smithing-pickup.md),
[the native kernel source ledger](workstations-smithing.md) and
[recipe-context ownership](workstations-recipe-context.md) for narrower source
contracts; those ledgers' earlier unexported-stage wording is historical.
