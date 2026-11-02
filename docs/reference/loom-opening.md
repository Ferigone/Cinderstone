# Loom provider admission and actual context contract

Target: Minecraft Java 1.21, protocol 767. The original server SHA-1 is
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`, official mappings SHA-1 is
`31c77994d96f05ba25a870ada70f47f315330437`, and the extracted inner-server
SHA-256 is `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
This stage read the actual generic `LoomMenu<B>` in the Team3 source tree at
`b4bccba` and independently inspected the pinned `dkq`, `cra`, `cra$6` and `cpu`
classes with direct JDK21 `javap -p -c`. It does not claim REA MCP execution,
Java gameplay or production Main integration. Raw disassembly stays private.

## Original source contracts

| Original method | Observed behavior |
| --- | --- |
| `LoomBlock.useWithoutItem` (`dkq.a`, source lines 35–41) | On the server, opens the actual provider and then awards `INTERACT_WITH_LOOM`. |
| `LoomBlock.getMenuProvider` (`dkq.b`, line 46) and provider lambda | Constructs `LoomMenu` with actual level/block-position access and title **`container.loom`**. There is no inventory-owning block entity. |
| `LoomMenu` constructor (`cra`, lines 36–133) | MenuType 18, banner/dye/template inputs 0/1/2, result 3 and player inventory 4–39. Selected pattern DataSlot starts at zero; the first applicable slotsChanged callback, not an invented opening reset, changes that state. |
| `LoomMenu.getSelectablePatterns` (`cra.c`, lines 151–157) | An empty template resolves the actual `no_item_required` tag. A genuine BannerPatternItem resolves its own pattern tag; missing tags yield an empty list. Other item classes yield no selectable patterns. HolderSet encounter order is retained. |
| `LoomMenu.clickMenuButton` (`cra.a`, lines 142–147) | Checks the actual selectable-list bounds, stores the selected index and builds the result. It does not add a separate six-layer check absent from this method. |
| `LoomMenu` default update Runnable (`cra.r`, source line 38) and `registerUpdateListener` (lines 225–226) | The original server-default Runnable returns. Java permits replacing it. The native factory admits only that verified original default; custom/reentrant replacements still require their actual owner. |
| `LoomMenu$6.onTake` (`cra$6.a`, lines 102–118) | Removes one banner and dye, resets selection when those inputs are exhausted, invokes actual level/block sound handling and then superclass Slot.onTake/setChanged. It does not consume the template or add an item-crafted/recipe callback. |
| Result-slot sound callback (lines 109–114) | Uses actual world game time and retained lastSoundTime; sends the real loom take sound, BLOCKS category, volume/pitch 1, only once per changed tick. The long field begins at zero. |
| `AbstractContainerMenu.doClick` (`cpu.b`, lines 310–528) | Quick-craft targets are actual Slot objects in a HashSet; the original loop iterates that set. Sorted native slot indices do not establish its identity-based encounter order. |

The ordinary main-hand/secondary-use and readonly spectator provider paths come
from the same pinned `ServerPlayerGameMode.useItemOn` contract as other native
workstations. `ServerPlayer.openMenu` constructs the bound menu, sends OpenScreen,
initializes its actual local listener before its synchronizer and then installs
it as current. Opening retains the separate InventoryMenu cursor/grid.

## Implemented provider-side code

`src/main_loom_dispatch.rs` supplies:

- `preflight_native_loom_use_locked`: actual block, actor, hand, secondary-use,
  reach, native world/body, configured pattern/feature and canonical inventory
  component admission. A positive result carries the actual `NativeLoomPatterns`
  view and spectator flag; it is not a constructed or installed menu token.
  Admission precedes any previous-menu close, window allocation or OpenScreen.
- `loom_access`: actual captured block/player observations implementing
  `WorkstationAccess`. Block identity short-circuits before lazy eye/range work.
  The existing proven equipped-slot reach guard and typed boundary are reused
  unchanged, without another guessed attribute owner.
- `loom_open_screen`: source MenuType/title encoding for a window obtained from
  the real shared allocator. It does not allocate or construct an empty UI.
- `finish_native_loom_open_locked`: actual interaction statistic and post-use
  block/default-block criterion boundary, to run only after a real bound menu is
  initialized and installed. The spectator provider path skips those callbacks.
- `loom_unambiguous_drag_order`: the unique zero/one-target order. Multiple
  targets fail before any live action until the actual Slot-identity HashSet
  owner can supply its encounter order; indices are never sorted as a stand-in.
- `use_native_loom`, `click_native_loom`, `loom_button` and resync wrappers:
  actual operation -> world -> player lock ordering, current-window routing,
  source last-action reset and exact typed unsupported-reach retirement.
  Opening closes the single actual current external owner through its real
  lifecycle helper before allocating a window. Existing container, crafting,
  furnace, stonecutter, smithing and loom owners are recognized; a future
  grindstone owner is not fabricated or referenced before its integration.

The configured pattern registry, template-item tags, exact holder-list order
and feature requirements belong to `main_loom_patterns`. The provider does not
invent a registry ID, suffix-based template classification, missing tag contents
or per-item enabled result. The supported admission uses the certified native
body/readonly spectator and actual empty advancement/objective profile.

## Actual runtime contract and integration boundary

`main_loom::NativeLoomContext` now binds the actual stable Server.world lock,
actor entity/UUID, captured dimension/position and exact configured pattern
binding. It captures real block/pose/range and actual supplied world game time
under the existing owners. The private native factory seals the original
default/nonreentrant listener environment; an arbitrary boolean is not accepted
as an external callback capability. Pure pickup synchronization reuses the
retained actual context and never queries its old access/time observations.
Mutable actions refresh them before preparing the real kernel plan.

`open_loom_menu_locked` constructs the real bound menu, sends OpenScreen, invokes
its initializer, executes actual phase-local listeners/packets and installs the
owner. The provider then invokes the actual non-spectator block callbacks.
`main_loom` owns prepared clicks/buttons/close/broadcast/full-state transactions;
`main_loom_lifecycle` and `main_loom_pickup` supply their genuine future consumers.
See [loom-transactions.md](loom-transactions.md),
[loom-callbacks.md](loom-callbacks.md) and
[loom-sound-source.md](loom-sound-source.md).

The current working tree now includes the actual generic kernel and central
Main module/field/dispatch, player-phase, pickup and persistence/removal source
assembly by their assigned integrators. This provider/runtime assignment did
not edit those old files. That source wiring has not yet been compiled or
exercised at this handoff; reverse cross-menu wiring and the final bounded
runtime validation remain integration responsibilities. No compiler/runtime
validation, public deployment or checklist completion is claimed here.
No progress metrics were changed by this provider/runtime assignment.
