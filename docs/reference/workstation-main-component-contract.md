# Main workstation component admission: Java 1.21

This is a source review of the Stonecutter and Smithing Main adapters, not a
claim that every item component or every vanilla menu context is supported.
The reviewed source baseline is `e18ad3781c2a94884925996e011466acc3dac5cf`,
with the uncommitted Smithing adapters present in the integration checkout on
2026-11-01. Subsequent admission work must be verified on its actual commit.

## Original source evidence

The reference is the unchanged original Java **1.21** inner server JAR, SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`, and official
mappings, SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`.
Inspection used **JDK 21 `javap -p -c`**, and `-v` for the record bootstrap
arguments. This was direct disassembly, not an REA MCP execution. Original Java
artifacts and raw disassembly remain outside the repository.

| Mapped source owner | Original evidence relevant to this boundary |
| --- | --- |
| `ItemStack` (`cuq`) | `isSameItemSameComponents`, `c(cuq,cuq)`, compares the item and the two `PatchedDataComponentMap` values with `Objects.equals`. The class does not override `equals(Object)` or `hashCode`. |
| `PatchedDataComponentMap` (`kr`) | `equals` compares the prototype component map and its patch map. Reference-keyed component types do not make every component **value** compare by reference. Each value's equality still matters. |
| `FoodProperties` (`cpr`) | Record equality includes `nutrition`, `saturation`, `canAlwaysEat`, `eatSeconds`, `usingConvertsTo` and `effects`. The record bootstrap explicitly includes its `Optional<ItemStack>` field. Two separately decoded conversion stacks therefore do not acquire equality from matching stack bytes. |
| `BundleContents` (`cxf`) | `equals` compares its weight and calls the `ItemStack` list comparator `cuq.a(List,List)`. |
| `ChargedProjectiles` (`cxg`), `ItemContainerContents` (`cxo`) | Their `equals` methods call the same `ItemStack` list comparator. A nested unresolved FOOD override is consequently relevant even when the outer item is not food. |
| `AbstractContainerMenu` (`cpu`) | `triggerSlotListeners`, `synchronizeSlotToRemote` and `synchronizeCarriedToRemote` use `ItemStack.matches`, `cuq.a(cuq,cuq)`. `doClick`, `moveItemStackTo` and `canItemQuickReplace` use `cuq.c(cuq,cuq)` for merging. The source owns separate local, remote and remote-carried baselines. |
| `Slot` (`crq`) | `safeInsert(ItemStack,int)`, `b(cuq,int)`, uses `cuq.c(cuq,cuq)` before merging. A valid stream codec and a valid maximum stack size do not replace this equality contract. |

The mapping names and method boundaries above distinguish ordinary stack
matching, which also checks counts, from same-item/component merging, which
does not. Neither operation permits an invented reference-identity allocator.

## Exact bounded FOOD admission rule

`inventory::ItemStack::same_item_same_components` currently compares decoded
component JSON values. That representation cannot tell whether a conversion
stack was shared by an original component, copied with that component, or
created by a different decode. Equal raw bytes are insufficient evidence.

An admission check may normalize a **detached observation** with the existing
`normalize_patch` policy. It must not rewrite the live stack, strip a component,
replace an input or reset a menu to make admission succeed.

| Observed state after source-proved normalization | FOOD identity admission |
| --- | --- |
| Empty stack | Admit; the source treats the optional stack as empty. |
| FOOD inherited from the actual pinned item's prototype | Admit under that pinned prototype ownership. |
| Explicit FOOD override removed by the existing original-stream elision mask | Admit; use only the already proved mask and payload checks. |
| FOOD removal | Admit for this particular reference-identity condition; there is no effective FOOD value. |
| Surviving explicit component **20** override | Reject as an unsupported native reference context before a dependent comparison. This conservative rule also rejects custom overrides whose narrower value equality has not been admitted. |

There are 41 FOOD prototypes in the pinned fixture. The existing normalization
policy proves 37 exact prototype overrides removable. Four keep their explicit
FOOD override even when its payload equals the prototype bytes:

| Item | Protocol item ID |
| --- | --- |
| `mushroom_stew` | 849 |
| `rabbit_stew` | 1120 |
| `beetroot_soup` | 1156 |
| `suspicious_stew` | 1193 |

These four items remain admissible with their **inherited** prototype FOOD.
Their explicit retained overrides must not be silently treated as that
prototype. The evidence is the existing
`fixtures/inventory_normalization_policy.json` and
`fixtures/item-components-1.21.json`; this review did not replace those fixtures
or create a new normalization policy.

### Nested stack boundary

In the pinned component schema, FOOD20 contains the optional conversion stack;
charged-projectiles29 and bundle-contents30 contain arrays of `NonEmptySlot`;
container52 contains an array of `Slot`. Rejecting surviving FOOD20 before
inspection of its child, then recursively admitting every decoded child in
29/30/52, covers this specific FOOD condition through nested item graphs.
Every inherited 29/30/52 prototype in the pinned fixture is an empty list.
This is not a proof of all component equality, registry or callback contexts.

Traversal must retain the typed decoder's consumed-byte boundaries, require the
whole component payload to finish, and reject an empty `NonEmptySlot`. The
current native decoder limits are 2 MiB per Slot input, array length 65,536,
component nesting depth 128 and 262,144 parser visits; container52 additionally
limits its list to 256 slots. These are existing native admission budgets, not
an assertion that they all originate in vanilla. A separate recursive admission
walk needs its own bounded depth/work accounting because each child decode
starts a fresh private parser budget.

## Production prerequisites beyond this component check

Both Main recipe adapters already require the actual unique configuration
feature packet and `MainItemGameplay::require_supported_floor`. Their
`PinnedDefaultRecipeContext` borrows the canonical `RecipeBook::vanilla` owner,
checks the exact enabled feature set and supplies the captured recipe encounter
orders. Smithing uses distinct input and result recipe orders. This admits the
captured default catalogue; it does not admit dynamic datapacks, replacement
trim registries or another enabled-feature snapshot.

Crafted-item callbacks are restricted to the captured original item classes.
The Stonecutter list covers shipped stonecutting results plus AIR; Smithing
covers captured base/result items plus AIR. The direct original query established
that their `Item.onCraftedBy`/postprocess owner is `Item`, whose postprocess body
returns. This is not permission to skip another item's override. Recipe/stat
publication must still use the real player's statistic and recipe-book owners
at the indexed source event. Empty advancement callbacks are admitted only by
the existing verified native empty-criteria owner; a custom registration or
reentrant listener requires its actual implementation.

Opening and actions additionally need the real player, canonical borrowed
inventory, actual block position/level, active window, source reach profile and
menu lifecycle owner. The derived result is never another owned input. Existing
bundle click overrides remain explicitly unsupported on the paths that require
them. The FOOD restriction is independent of these recipe, feature, item-class,
world and listener prerequisites.

## Main callsites requiring admission

The reviewed `stonecutter_preflight_phase` and `smithing_preflight_phase`
validate encoding and maximum-stack-size queries. Those checks do **not** prove
component identity. Checking only the final phase also misses an unsupported
input that was consumed, moved or discarded while preparing the operation.

| Boundary | Required timing and coverage |
| --- | --- |
| PLAY `0x0e` ContainerClick decode and dispatch in `main.rs` | Keep wire decoding truthful, then admit actual accepted prediction stacks and carried prediction before any native/fallback operation can install them as remote baselines. A decoded, well-formed custom component can still be outside the current native semantic profile. |
| Stonecutter/Smithing primary action inside the canonical transaction | Before `prepare_click`, `prepare_button`, `prepare_close` or broadcast, admit the actual input/result/cursor, canonical inventory and applicable client predictions. Admission must precede menu comparisons and any item/entity RNG, callbacks or publication. |
| Phased callback preflight | Admit every event's actual input/result/cursor and player-inventory snapshot as a backstop before the first real callback/drop/world effect. Preserve each event's snapshot rather than substituting the final inventory. |
| Opening and immediate external pickup broadcast | Admit the actual inventory and menu state before initialization or broadcast can compare/capture them. Initializing a replacement menu is not a repair for an old unsupported baseline. |
| PLAY `0x32` creative assignment/drop | Admit the decoded candidate before installation, baseline replacement, insertion/merge or item creation. At the reviewed baseline positive-slot assignment invokes `execute_menu_action` with `creative_attempt: None`; its separate `creative_attempt` checks do not cover that candidate. |
| Fallback `PlayerContainer` | Cover actual inventory/carried state and predictions before `PlayerContainer::click`, synchronization or insertion. This owner has its own private local/remote baselines; the workstation `MenuSync` guard does not protect it. |
| Retained synchronization baselines and transfer | The owner of `MenuSync.last`, `remote` and `remote_carried` must admit them before comparison or transfer. Checking only currently visible items and the newest packet is insufficient. Retained `PlayerContainer` baselines need the corresponding central-owner treatment. |

For an already retained unsupported stack, a rejected private plan must keep the
actual input/cursor/inventory and menu ownership intact. Closing is not a reason
to delete it, strip FOOD, fabricate equality or publish a fabricated drop. An
unsupported close that returns an error before commit is still an explicit
lifecycle blocker; it is not a completed vanilla close. Preventing admission at
the entry points avoids introducing this state into a fresh supported profile.
Previously loaded saves or historical predictions require explicit handling by
their real owner before unrestricted support can be claimed.

The narrow central admission check and the team-owned baseline guard complement
one another. They do not justify putting native unsupported-profile errors into
the generic packet decoder as if valid vanilla FOOD packets were malformed.

## Review outcome and handoff

At the reviewed baseline the component admission gap is concrete: structurally
equal custom FOOD can reach a semantic comparison through an ordinary player
slot or a client prediction, without ever being a valid station ingredient.
Neither the default-feature guard nor existing phase encoding preflight closes
that gap. A central Main guard is therefore required in addition to the shared
workstation baseline guard before claiming that broader production profile.

The original proposal was
[issue3 comment6564175069](https://github.com/Ferigone/Cinderstone/issues/3#issuecomment-6564175069).
The subsequent delivered source is reviewed below. The integration coordinator
separately assigned `src/main_workstation_components.rs` and primary action
callsites after this finding. No kernel, Main, normalization policy, progress
metric or deployment was changed by this documentation review, and no
comprehensive suite was run.

## Delivered team source review: PR73

The actual source at
[`b4bccbad9794896b4d1dffa68d0a02eea8dbff12`](https://github.com/Ferigone/Cinderstone/commit/b4bccbad9794896b4d1dffa68d0a02eea8dbff12)
was fetched and read directly, including both controller files and
`docs/reference/workstations-component-context.md`.
[Issue3 comment6564299008](https://github.com/Ferigone/Cinderstone/issues/3#issuecomment-6564299008)
records the source handoff. At this review,
[PR73](https://github.com/Ferigone/Cinderstone/pull/73) was open and non-draft,
head `5c54f98a203f04eab7819b5ae13a5d222af55122`; the comparison from the reviewed
source to that head changed only team status/reference documents. Publication
is proven. A merge, production invocation and successful live validation do not
follow from that fact.

| Actual callable API or internal boundary | Proven coverage and limit at `b4bccbad` |
| --- | --- |
| `workstations::menu::require_component_context(&ItemStack) -> Result<()>` | Public, observes a normalized clone and rejects a surviving **top-level** explicit FOOD20. Inherited FOOD and proved prototype elision are admitted. It does not inspect descendants in29/30/52. |
| `MenuSync::snapshot(top, inventory)` | Admits every canonical inventory slot, retained window0 cursor, external menu cursor and supplied top stack with the shallow guard before constructing a snapshot. |
| Private `matches(a,b)` | Guards both operands before same-item/component equality. This protects top-level FOOD in historical `last`, `remote` and `remote_carried` values when those values are compared. It does not expose those baselines for a host's recursive preflight. |
| `MenuSync::finish_click` | Admits carried prediction and accepted, in-range changed-slot predictions before installing their new remote baselines. Stonecutter/Smithing `prepare_click` already execute their private `do_click` before reaching this boundary, so it is not a pre-click admission gate. Out-of-range predictions are ignored by this source boundary. |
| `MenuSync::close_inputs_ordered` | Alive-connected removal admits current inventory, cursor and inputs before insertion. Dead/disconnected raw removal deliberately transfers exact owned values to indexed drops without semantic equality. That raw exception is not certification of an entire prepared controller plan. |
| `MenuSync::transfer_inventory_state` | Copies matching `last` and `remote` entries to `PlayerContainer` without a component guard. It neither inspects nested FOOD nor provides a public retained-baseline admission method. |
| `StonecutterMenu::initialize` / `broadcast_changes` / `broadcast_full_state` | Reach guarded shared snapshots. Full-state returns `Result<Vec<SyncEvent>>`, preserves the retained menu and advances its revision; it does not initialize a substitute owner. |
| `SmithingMenu::initialize` / `broadcast_changes` / `broadcast_full_state` | Reach guarded shared snapshots. Full-state is now actually delivered as `Result<Vec<SmithingMenuEvent>>`, with source phase snapshots, retained inputs/result/cursor/drag state and revision invalidation. |
| Stonecutter/Smithing `prepare_close` and plan validation | The controllers still capture whole inventory/menu observations and validate them with native equality. Stonecutter also checks its close-drop observation; Smithing validates its entire retained menu, including synchronization state. A raw dead-close transfer alone does not establish these extra comparison contexts. |

There is no delivered public `MenuSync` API that recursively certifies all
private baselines, and Stonecutter still exposes no public `sync()` accessor.
The shared guard is useful delivered code, but is shallower than the required
whole-graph FOOD boundary. The source owner must extend it or expose an actual
read-only admission API if the host needs to certify existing private state.
No accessor, recursive check or guard on transfer is assumed to exist merely
because it was requested.

### Central helper and transport follow-up

The separately assigned `main_workstation_components.rs` was also read in the
integration working tree. Its actual functions are
`require_supported_stack`, `require_supported_inventory` and
`require_supported_prediction`; it uses the existing normalization policy on
observations, rejects surviving FOOD20 and recursively walks effective29/30/52
with consumed-byte checks, `NonEmptySlot` checks and independent depth/work
limits. No concrete recursive-decoding or normalization defect was found in
that bounded source review. This does not certify its compilation, complete
callsites or unrelated component semantics.

Before the recovery implementation, central admission branches in PLAY0x0e
and0x32 were observed to log and ignore a refused candidate. That conserves
authoritative state, but does not
repair the client's already applied prediction. Because rejected predictions
were never installed as remote baselines, a later unchanged incremental
broadcast may send nothing. Explicit recovery needs the actual retained active
menu's authoritative full-state synchronization through the existing ordered
output owner. Window0 synchronization cannot stand in for an active workstation,
and recreating or reinitializing a menu is not such recovery. This is handling
of an explicitly unsupported native context, not original-Java custom-FOOD
compatibility.

The central prediction helper currently checks every supplied changed-slot
value. Vanilla and the delivered `finish_click` ignore out-of-range indices.
Rejecting an otherwise unused value is therefore a conservative admitted-profile
restriction; exact ignored-index handling requires the actual active menu's
accepted index range. This distinction must remain visible until handled by
the real dispatch owner.

A fresh process with checked saved inventories, checked restored item entities
and every mutation/prediction entry admitted can prevent unsupported baselines
from being introduced. That invariant needs complete owner/callsite evidence.
It must not be inferred for arbitrary existing controller objects, historical
baselines, imported inventories or saves from the shallow shared helper alone.

### Recovery implementation source follow-up

The subsequent integration source now contains
`Server::resync_current_player_menu(id) -> Result<()>` in
`src/main_menu_resync.rs`. Its async entry acquires the existing operation lock,
reads the actual clock/border, then acquires world and player owners. It checks
the actual native world/gameplay and canonical inventory context, rejects
multiple current external owners and calls only the selected retained menu.
PLAY0x0e and0x32 refusal branches call this entry before continuing. Rejected
predictions and candidates are never installed. The Stone publication copy
physically omits Smithing variants/calls; the working Smithing stage includes
its actual owner.

Stonecutter and Smithing recovery use their retained `broadcast_full_state`
wrappers and their real ordered callback/packet commit paths. Container,
furnace and crafting-table recovery use the existing retained remote-only
`full_sync`/`sendAllDataToRemote` operations, preserving local listener baselines;
no additional local-listener callbacks are fabricated for these remote-only
operations. Window0 uses a genuine `PlayerContainer::broadcast_full_state`
inside the canonical `SharedContainer` transaction. Actual local slot changes
are staged with `MainItemGameplay::prepare_menu_commit`, whose token commits
before the existing shared `PlayerEvent::Container` publisher sends the packet.
No new inventory, RNG owner, output writer or menu is created.

The source review also found a combined-profile error: a refused FOOD
prediction could enter recovery after an equipped interaction-range modifier
had invalidated the bounded workstation reach profile. Recovery originally
propagated `UnsupportedMenuReach` out of the connection instead of following
the normal conservative menu-retirement path. The new Stonecutter/Smithing
locked recovery helpers now catch only their own typed reach error and use the
existing actual alive-connected close with `send_close: true`. Owned cursor and
inputs follow the real return/drop owner; other errors remain explicit. This
correction was read in working Stonecutter/Smithing and the Stone publication
source. It does not add a dynamic range implementation.

The integration coordinator reported a successful complete Stone publication
binary build for source `5553015344f939f6bb255f414439e1c3b022af5a`, after merging
actual master `f390`, with Git tree
`adf8b3920e3a75b60fa7d19451602112539d1178`. The resulting binary SHA-256 is
`526842c949dd1ab039e0b71a017cb8ce794e0241d526c1344002ad6e4c922539`.
This is compiler-composition evidence for that exact Stone publication source,
including its admission/recovery and typed-reach retirement paths. The separate
working Smithing stage is not included in that result. No successful protocol
runtime, merged PR or deployment is claimed here; focused runtime verification
was still pending when this record was updated.
