# Grindstone provider and actual Host dispatch

Target: Minecraft Java 1.21, protocol 767. Original server SHA-1:
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`; official mappings SHA-1:
`31c77994d96f05ba25a870ada70f47f315330437`; extracted inner-server SHA-256:
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
The initial stage inspected Team3 source `b4bccba`; the current stations
composition uses the genuine same-token copy/full-state APIs delivered in PR76
(`ed5d07f`/`ebc2c6c`). It independently used direct JDK21
`javap -p -c` on the pinned block/menu/result-slot classes. It does not claim an
REA MCP execution, Java gameplay observation or native production menu.

## Original provider and lifecycle facts

| Original method | Observed behavior |
| --- | --- |
| `GrindstoneBlock.useWithoutItem` (`djk.a`, source lines 169–175) | On the server, opens the actual menu provider, then awards `INTERACT_WITH_GRINDSTONE`. |
| `GrindstoneBlock.getMenuProvider` (`djk.b`, line 180) and its provider lambda | Constructs `GrindstoneMenu` with actual level/position access. The source title is **`container.grindstone_title`**, not a guessed translation key. Floor, wall and ceiling orientations are states of the same block/menu provider. |
| `GrindstoneMenu` constructor (`cqt`, lines 33–120) | MenuType 15; inputs 0/1, derived result 2 and actual player inventory slots 3–38. No block entity owns these temporary inputs. |
| `ServerPlayerGameMode.useItemOn` (`aqw.a`, lines 327–380) | The spectator menu-provider branch precedes ordinary hand/secondary-use checks. Ordinary default-block use is main-hand only and is suppressed by secondary use when either hand is nonempty. |
| `ServerPlayer.openMenu/initMenu` (`aqv.a`, lines 1153–1173 and 488–490) | Closes the actual external menu, advances the shared window counter, constructs the new menu, sends OpenScreen, initializes the local listener before its synchronizer, then installs the current menu. Opening does not remove the retained InventoryMenu cursor/grid. |
| `GrindstoneMenu.stillValid` (`cqt.b`, line 225) | Uses the actual captured grindstone block and player's interaction range with the default additional four blocks. |
| `GrindstoneMenu$4.onTake` and its level callback (`cqt$4.a`, lines 72–81/73–77) | Awards real XP orbs at the block centre through the actual ServerLevel, emits Level event 1042/data 0, then clears input 0 followed by input 1. There is no crafted-item statistic or recipe-book callback. |
| `GrindstoneMenu$4.getExperienceAmount` (`cqt$4.a`, lines 84–93) | Sums source non-curse minimum costs using Java integers. A positive sum draws `ceil(sum / 2) + Level.random.nextInt(ceil(sum / 2))`; a nonpositive sum draws nothing. A bound-one sample still invokes Level RNG. |

## Implemented provider-side code

`src/main_grindstone_dispatch.rs` supplies actual source gates and observations:

- `preflight_native_grindstone_use_locked` checks the real block, actor, hand,
  secondary use and reach under the caller's operation/world/player locks. It
  returns a provider decision, not a constructed or installed menu.
- An opening decision requires the existing genuine admitted native body or
  readonly spectator profile, actual current native world-source capability,
  certified configured enchantment/curse/feature packets and canonical inventory
  component admission. It runs before any old-menu removal, window allocation or
  OpenScreen. The configured registry observation is supplied by
  `NativeGrindstoneEnchantments`; no registry or curse set is invented here.
- `grindstone_access` implements `WorkstationAccess` from the actual current
  block and player observations. Eye/range evaluation is lazy after block
  identity, preserving the source's short circuit.
- The existing proven equipped slot-group reach guard and typed
  `UnsupportedMenuReach` boundary are reused unchanged. No second modifier
  applicability map or custom attribute calculation is introduced.
- `grindstone_open_screen` encodes the actual shared allocator's supplied window
  with the source MenuType/title. It does not allocate a window or create a menu.
- `finish_native_grindstone_open_locked` supplies the real block interaction
  statistic and post-use block/default-block criterion boundary, to be invoked
  only after the actual Host initialized and installed its menu. The spectator
  provider path skips those block callbacks.

## Actual Host and current-menu dispatch

The inspected native `GrindstoneMenu<K,S>` is not the Stonecutter/Smithing
prepared-event API. It requires an actual `GrindstoneMenuHost` for construction,
initialization, broadcast, click and close. That Host binds real player/level/
position identity and an actual registry snapshot. Its `begin_stage` and
`publish_stage` must stage all inventory, callback, RNG, item and XP owners
together; a placeholder binding, fixture RNG, direct player-XP grant or dummy
callback would violate the source contract.

The native source uses `players.grindstones` with an actual
`GrindstoneRuntime`/`OpenGrindstone`, and real locked opening/action owners.
The opening owner must construct its bound Host/menu before OpenScreen and run
the actual initializer after it. Click and close require the actual current
menu token and captured owners; they cannot be implemented by calling a generic
InventoryMenu or reinitializing a new menu. The temporary inputs and external
cursor must remain owned until the real connected/disconnected removal path.

`use_native_grindstone` now applies those gates before actual old-external-menu
closure and shared window allocation. It calls the real Host opening owner and
then the source block statistic/criterion boundary. The retained InventoryMenu
cursor/grid remain separate. `click_native_grindstone` resets the actual player
last-action owner before the window gate; an active Grindstone consumes and
ignores a different-window request so it cannot reach inactive window0.

Spectator clicks and `resync_native_grindstone_locked` use the real retained
full-state transaction without recomputing, resetting or executing a fake click.
Only the typed unsupported reach error is recovered by retiring the actual
current menu. Return versus drop uses actual connected health/removal facts;
other unavailable source contexts remain explicit errors.

The Host stages actual mixed Item/XP additions and repeated integer-orb awards
during their source callbacks. The furnace floating-reward path is not used.
Lifecycle and pickup have separate real owners. The stations branch is composing
their actual Main routes; source presence alone is not a socket/gameplay result,
a complete workstation delivery or vanilla compatibility evidence.

No full composition compiler, socket-flow or gameplay claim is made here.
Validation belongs to the coordinator after the actual Host and world owners
are composed. Raw disassembly and proprietary binaries remain outside the repo.
