# Native Main anvil integration contract

This is the concrete contract for the authorized native Main anvil stage.
The earlier request was
[issue3 comment6564502845](https://github.com/Ferigone/Cinderstone/issues/3#issuecomment-6564502845);
the central integrator granted the narrow Main assignment in
[comment6564589182](https://github.com/Ferigone/Cinderstone/issues/3#issuecomment-6564589182).
The source namespace is the five new files `main_anvil.rs`,
`main_anvil_context.rs`, `main_anvil_dispatch.rs`, `main_anvil_lifecycle.rs` and
`main_anvil_pickup.rs`. Root retains `main.rs`, runtime fields, exports, cross-menu,
body/rain pickup, save and recovery wiring. The earlier all-owner contract request
for grindstone is
[comment6564455352](https://github.com/Ferigone/Cinderstone/issues/3#issuecomment-6564455352).
This grant does not complete a checklist delivery or authorize deployment.

Main/runtime facts were first read at3053218, then refreshed after PR72 merged
at `dddc0e7807bf8caa91d5b57232e0535e6519f5ed`. Before first source publication,
the separate Main branch was based on actual PR76 merge
`0ad717b1ab6c591d6a9fd0f76564bc8e8eb2a981`; all five untracked source files
were preserved byte-for-byte. No master merge, reset or force push was used.
This base includes the real inventory counter and ordered item/menu publication
seams, mandatory Anvil host transaction `7690150`, retained fullresync and
one-shot actual-inventory pickup bridge `edfaf83`.

Final controller source is `edfaf836437ca9e15eef52c3140b021e74cdccfe`.
PR76's actual merged tree matches the tested latest-master composition.
Earlier0fa source lacked the mandatory host transaction; that concrete gap was
corrected and independently reviewed before merge. It is not treated as a
working publication capability in this stage. Current composition evidence and
remaining real Root seams are in [the verification note](anvil-verification.md).

## Transaction binding first

Use the existing single-owner staging approach: canonical inventory, player,
XP, player/Level RNG, item/drop, listener and ordered menu/item outbox proposals
prepare privately; publication is infallible once the live owners are committed. The
anvil controller's private staged UI is installed only after its fallible source
callbacks succeed. Reuse PR72's `Server.chat_dispatch` operation guard, followed by world → players
→ SharedContainer lock order. There is no await, listener reentry or publication
between staging and commit.

This is not universal rollback of independent environment acquisition or the
process-wide entity-constructor seed factory. The actual runtime ledger records
the certified source/chunk acquisition boundary; constructor preparation uses
the real process entropy factory rather than a fabricated seed stream. Those
owners are not rewound after a later refusal. Keep that limit distinct from
the canonical menu/inventory, retained player/Level RNG, item-store and ordered
menu/gameplay publication guarantee. A readiness backend's ability to publish
other chunk work does not establish that a particular read-only certification
path executed that callback; pin the inspected call chain and revision.

The earlier source0fa exposed the callback methods below without a host
transaction boundary. The now-merged correction
`AnvilMenuHost: AnvilContext + AnvilWorldAccess + WorkstationAccess + Sized`
now requires:

```rust
fn begin_stage(
    &self, token: AnvilMenuToken, action: AnvilMenuAction,
) -> Result<Self>;
fn publish_stage(
    &mut self, staged: Self, token: AnvilMenuToken,
) -> Result<()>;
```

Its private controller transaction executes callbacks on the detached actual
host, validates binding/registry/token and checked revision, then invokes
`publish_stage`. On success it installs the private UI with an infallible
assignment. Publication errors change no live owner. The ordered outbox remains
queued until the outer action/locks return; packets must not escape between
host publication and UI installation. Use this viable binding after its final
commit is pinned. No public Clone or prepared-menu workaround is proposed.

The remaining callback methods are `preflight_menu_action`,
`observe_menu_phase`, `stage_sync_events`, `input_container_set_changed`,
`player_inventory_set_changed`, `give_experience_levels`,
`add_to_player_inventory`, `stage_drop` and `quickcraft_iteration_order`.
The concrete Main adapter should own one private stage encompassing all of them.

```rust
// Actual runtime/binding names; host constructor signatures are proposed below.
type P = (i32, i32, i32);
struct NativeAnvilRuntime { menus: BTreeMap<i32, OpenAnvil> }
struct NativeAnvilBinding {
    player_id: i32,
    player_uuid: [u8; 16],
    position: P,
    window_id: u8,
    dimension: String,
}
struct OpenAnvil {
    position: P,
    binding: NativeAnvilBinding,
    menu: AnvilMenu<NativeAnvilBinding>,
}

impl<'a> NativeAnvilHost<'a> {
    fn bind_locked_owners(
        server: &'a Server,
        world: &'a mut WorldBackend,
        owners: NativeAnvilOwnerParts<'a>,
        actor: i32,
        binding: &NativeAnvilBinding,
        now: i64,
        border: CollisionBorder,
    ) -> Result<Self>;
    fn seal_publication(&mut self) -> Result<SealedMainAnvilCommit>;
    fn commit_sealed(&mut self, sealed: SealedMainAnvilCommit);
}
```

`NativeAnvilOwnerParts` is built by destructuring the real locked LivePlayers:
canonical SharedContainer, lifecycle/abilities/XP, retained player RNG, item
runtime/entity index, gameplay/statistics, experience-sync cache and actual
world/source/lighting/scheduler commit owners. It excludes `anvils.menus`, so the
caller can borrow the actual menu and host domains disjointly. Do not retain a
second mutable borrow of the whole LivePlayers inside the host.

The concrete adapter implements the mandatory trait's `begin_stage` and
`publish_stage` using these locked owner bindings and detached state. Its
`seal_publication` performs all fallible preparation/actual-base validation
before `commit_sealed`; the latter publishes the real owners and queues the
ordered outbox without further failure. The binding must separate live commit
handles from privately staged mutable domains; copying a shared mutable
Arc/Mutex handle is not staging. A staged Self may hold detached domain images
and read-only bindings while only the live Self retains exclusive commit
handles; it must not duplicate mutable owner references.

The concrete NativeAnvilBinding captures actual player ID/UUID, position,
window and dimension, with registry/session/source identities validated by the
actual context under the owned locks. Its menu token retains the UI lifetime;
registry snapshots remain in the context owner rather than being fabricated
as new binding counters.
An anvil has no block entity: do not invent a BE owner or per-block lifetime
token. An active menu follows its captured level/position's current anvil tag and
reach; same-position replacement does not revive a closed UI lifetime. Existing Main has one server-owned
`world: RwLock<WorldBackend>` and `LivePlayers.connection_admissions`; if the
central integrator needs a comparable level/session key, bind it to those actual
lifetimes. An absent level identity must not become a fabricated local counter.

`LivePlayers` would gain exactly `anvils: main_anvil::NativeAnvilRuntime`, initialized
empty by `LivePlayers::new`. Each open menu owns its two inputs, one derived
preview and external cursor through `AnvilMenu`. The canonical inventory remains
`players.containers[actor]`/`SharedContainer`/`PlayerContainer.inventory`.
The context's private PlayerContainer is a transaction image of that actual
owner, retaining window0's grid/cursor/baselines. Its `inventory()` and
`inventory_mut()` refer to that same staged inventory at every source callback.
There is no second live inventory, preview or XP balance.

All fallible drop construction, source-edit validation, component checks,
listener preparation, entity-ID validation and actual-base validation must
finish before the mandatory host publication commits anything. Controller or
publication errors discard the private host/UI stage. After successful
publication, the UI installation and remaining owner publication/outbox enqueue
must be infallible. Drain the ordered outbox only after the outer action has
returned and released its locks. Do not advance a live UI and then attempt a
fallible item/world operation.

## Five assigned modules and exact entry points

The file assignment is granted; the signatures below are the reviewable
composition contract being implemented. Existing callsite names in the next
section identify Root's separate central wiring.

| New module | Proposed responsibility/API |
| --- | --- |
| `main_anvil.rs` | `NativeAnvilRuntime::{window_id,has_open,pending_players,inventory_snapshot}`; retain `OpenAnvil` and binding; no provider defaults. `inventory_snapshot` overlays only the actual external cursor onto a clone of canonical inventory. |
| `main_anvil_context.rs` | `NativeAnvilHost::bind_locked_owners/seal_publication/commit_sealed`; implement the mandatory `begin_stage/publish_stage` and the published `AnvilMenuHost`, `AnvilContext`, `AnvilWorldAccess`, `WorkstationAccess` against actual staged owners. |
| `main_anvil_dispatch.rs` | `Server::use_native_anvil(id:i32,hand:i32,target:P)->Result<bool>` async; `click_native_anvil(id,&ContainerClick)->Result<bool>` async; `rename_native_anvil(id,&JavaString)->Result<bool>` async; internal `anvil_action_locked(&mut WorldBackend,&mut LivePlayers,id,AnvilRequest,now,CollisionBorder)->Result<bool>`. |
| `main_anvil_lifecycle.rs` | `Server::close_native_anvil(id,window:u8,send_close:bool)->Result<bool>` async; locked `close_anvil_with_context_locked(...,MenuCloseContext)`; `tick_native_anvil_menu_for_player_locked(world,players,id,now,border)`; `close_anvils_for_save_locked(world,players,now)`. |
| `main_anvil_pickup.rs` | `preflight_anvil_pickup(&LivePlayers,id,&ItemStack)` before canonical Inventory.add; `NativeAnvilRuntime::broadcast_for_inventory(id,PickupPlayer,&PlayerInventory,&mut MainGameplayFrame)->Result<Vec<Packet>>` at actual ServerPlayer.take. Retains the real menu/cursor and runs source local listeners against the same canonical inventory before returning its packet batch. No synthetic click, empty menu or window0 sync. |

`AnvilRequest` distinguishes broadcast, retained full-state recovery, rename,
click and close with explicit close context/send-close policy. Opening owns
its separate initialize phase. Obtain the
current `menu.token()` under the actual menu lock and call its `initialize`,
`broadcast_changes`, `set_item_name`, `click` or `close`. Tokens never come from
client data. A mismatched click window is consumed/ignored; close acts on the
current external menu rather than reviving the requested window.

The source0fa public menu lacked `broadcast_full_state`; the accepted frozen
transaction correction adds
`AnvilMenu::broadcast_full_state(&mut self, host: &mut H) -> Result<()>` and
`AnvilMenuAction::FullState`, retaining local listener baselines. Bind recovery
to that actual corrected API once committed. Extend current
`main_menu_resync::Server::resync_current_player_menu` with its anvil branch;
reuse that retained-current-menu operation rather than a new recovery selector.
Do not reuse window0's snapshot as the
contents of an active anvil or manufacture an invalid click to force a resend.

## Existing Main seams and proposed call phases

| Actual current owner/function | Required binding/call phase |
| --- | --- |
| `Server::use_native_stonecutter`, `use_native_furnace`, `use_native_crafting_table`, `use_native_container` in PLAY use-item-on handling | Add anvil dispatch before generic `interact_world_block`; preserve reach, hand, secondary-use and acknowledgement policy. Reuse `NativeContainerRuntime::allocate_window`, whose existing counter cycles1–100; no competing window counter. Close the actual external menu first, retain window0. Open packet is0x33, MenuType8, actual translatable title `container.repair`, followed by staged initialization. |
| `main.rs` PLAY0x0e | Reuse `main_workstation_components::require_supported_prediction`, then route active anvil alongside `click_native_stonecutter`, before window0 `execute_menu_action`. Decode with `ContainerClick::decode`. Preserve actual active-owner identity, spectator policy and stale-ID handling. |
| `main_connection_decode::validate_play`0x2a | It currently validates `Reader::string(32767)` only. Add actual active-anvil rename dispatch; convert to `nbt::JavaString`, then let `set_item_name`/`validate_name` apply Java filtering and50 UTF-16-unit limit. Do not treat every string packet as permission to rename another menu. |
| `main.rs` PLAY0x0f → `Server::close_current_player_menu` | Add anvil to this helper's actual `CurrentMenu` selection under the operation/world/player locks. Do not restore the old separately awaited window-selection pattern. The requested window remains ignored as an owner selector. |
| `Server::tick_item_world` → `LivePlayers::simulate_item_world_with_player_menus` | Extend the current furnace/`tick_native_stonecutter_menu_for_player_locked` callback dispatch to the actual active anvil. Broadcast its captured state first, then `still_valid`, then server close packet0x12 before cursor/input removal when invalid. Do not move this into the connection tick or global workstation polling. |
| `Server::capture_player_locked` | Include `players.anvils.inventory_snapshot` in the existing container/crafting/furnace/stonecutter cursor overlay chain. Retain actual lifecycle XP and abilities. Never include the derived preview as an owned saved item. |
| `Server::cleanup_player_admitted` | Keep `close_inventory_menu_with_context_locked(DeadOrDisconnected)` first; then close the actual anvil with that context before `save_player_locked` and runtime removal. Preserve cursor then input0 then input1 drops/RNG/IDs. Add anvil to orphan-menu checks. |
| `Server::save` shutdown branch | Add `close_anvils_for_save_locked` alongside existing furnace/stonecutter/crafting/container closes before snapshot capture. Alive save cleanup uses actual inventory returns and overflow drops; it is not the disconnected path. |
| Other menu openers and `close_inventory_menu_with_context_locked` | Include anvil in mutual exclusion/active-menu guards and close ordering. A new chest/furnace/crafting open must not leave anvil inputs/cursor orphaned. |

Periodic player capture currently does not have a format for restoring two live
anvil inputs or a menu session. They remain in the live `OpenAnvil`; normal
shutdown/disconnect must drain them before persistence. Crash recovery of active
temporary menus is a separate unresolved checkpoint contract. Do not silently
save the preview, discard inputs, or close every periodic autosave as though
that were Java behavior.

## Exact owner mapping and unresolved dependencies

- **XP/abilities:** read `LivePlayers.player_lifecycle[uuid].experience` and
  `.abilities.instant_build`, not a new field or a cached connection gamemode.
  `AnvilPlayer.experience_level` must be fresh at pickup. Main already persists
  `ExperienceData { level,total,progress }` through `PlayerData::capture` and
  `Server::persist_player_data`/`player_nbt::save_player_file`.
  `experience::give_experience_points` is not giveExperienceLevels. A real
  level-change adapter is still needed: Java `Player.giveExperienceLevels`
  adds the level delta, resets level/progress/total only on underflow, and its
  ServerPlayer override invalidates last-sent XP. Current
  `MainExperienceRuntime::finish_native_experience_player_tick` compares only
  total, so a deduction preserving total needs a central
  `invalidate_sent_experience(id)` seam before that next tick. Do not emit XP
  immediately or create an orb for an anvil cost. Existing current native
  movement admission is creative; a creative flow proves neither paid XP nor
  non-instabuild wear integration.
- **Inventory counter:** [PR72](https://github.com/Ferigone/Cinderstone/pull/72)
  merged during preparation. Reuse the actual canonical
  `PlayerInventory::{set_changed,get_times_changed}` transient counter, initialized
  by that inventory's constructor and not persisted. `set_changed` uses Java
  wrapping addition only at actual calls. No competing map/UI-local counter is
  proposed. `PreparedStonecutterCommit::commit` already invokes it at indexed
  player-change events through `SharedContainer::transact_with_menu_commit`.
  `MainItemGameplay` statistics/InventoryScan observations remain different owners.
- **Inventory.add:** reuse the actual staged `PlayerContainer` method
  `add_pickup_to_inventory(stack,instant_build)` for the Inventory.add operation;
  it preserves the remainder and remote baseline. Do not use the close-return
  helper as a substitute or broadcast window0 after every insertion.
- **Player RNG/drop:** retain `LivePlayers.player_random[id]`, initialized by
  `Server::allocate_player_entity` and never reseeded per action. Anvil wear
  consumes that player's `next_f32`, conditionally after XP/input/cost changes.
  Item construction instead also uses actual
  `players.item_runtime.world_random_mut()` and independent
  `random_seed::create_random` factories, existing `Server.entities`,
  `spawn_player_drop_with_factory`, `RuntimeEntities::prepare_items` and
  `main_drop_commit::DropCommitOwner`. They must be composed in source order;
  the existing `menu_action_with_items` batch helper is a seam to reuse/adapt,
  not permission to postpone all interleaved drops until after all callbacks.
- **World:** preserve `finish_anvil_take`: actual captured block read,
  conditional player wear draw, `AnvilBlock.damage`, flags2 damaged-state write
  or destroy-without-drop, then event1029/1030. Use
  `world_interactions::InteractionPlan/plan_edit`,
  `main_world_interactions::commit_fixture_plan`,
  `prepare_required_native_source_edit`, `observe_native_source_edit`,
  `complete_native_source_edits`, actual lighting/fluid/fire/redstone and BLOCK
  scheduler owners. A raw fixture `set_block` is not the callback transaction.
  Current `schedule_native_block_tick` rejects unported scheduled callbacks;
  falling-anvil callback coverage must be proven, not inferred from the generic
  plan type. Wear destruction must use actual destroy-without-drop callbacks;
  `Server::edit_world_block` has creative-player edit policy and is not that
  operation. Imported-world atomic edits are explicitly unsupported. Stage
  `item_effects::level_event_packet` through actual audience/order owners.
- **Gameplay/listeners:** current configured capability is
  `GameplaySourceScope::NativeFixtureEmptyCriteria`, enforced by
  `MainItemGameplay::require_supported_floor`; unknown imported criteria,
  objectives and game-event receivers fail. Use actual
  `inventory_slot_changed` at each inventory-backed SlotListener phase. The
  existing `prepare_menu_commit` coalesces against one final inventory; do not
  feed it all earlier callbacks with a final snapshot. Current Stonecutter
  prevalidates every phase, then calls `inventory_slot_changed` with that phase's
  inventory during its infallible ordered commit. Adapt that existing central
  binding, preserving host inventory at each anvil callback and rejecting
  unsupported criteria. `player_inventory_set_changed` is separate. Reuse
  `main_workstation_components::{require_supported_stack,require_supported_inventory}`
  for the current recursive FoodProperties identity/component admission boundary.
- **Outbox:** `SharedContainer::transact_with_menu_commit` (with the older
  `transact_with_commit` wrapper) owns canonical mutation,
  checked revision and ordered publish via `with_publisher` →
  `PlayerEvent::Container`; `PlayerEvent::WorldAudience` shares the actual entity
  event stream. Buffer menu packets, block events and Item additions in source
  order. Do not also call `send_packets` or broadcast window0 for packets already
  published by the host. `ordered_entity_input` treats lost history as terminal.
- **Repair item owner at the inspected master base:** there was no production
  `AnvilContext::is_valid_repair_item` implementation. The granted context
  stage below ports a finite verified native item catalogue; other actual item
  class/material/tag rules still require their owner. Recipe metadata,
  an always-false predicate or a test fixture cannot establish this owner.
- **Hover-name owner beyond the bounded context:** codecs in `item_nbt` and
  `Reader::string` are not `ItemStack.getHoverName().getString` resolution.
  Actual custom/item name components, translations, children and Java-string
  semantics require their real text/item owner. The granted finite original
  item strings and literal-component handling below do not resolve arbitrary
  translations or nonliteral text. Do not substitute an item ID, translation
  key, assumed English name or the test host's string.
- **Enchantment owner is unavailable in Main:** frozen source0fa exposes
  `enchantment_snapshot`, `validate_enchantment_snapshot` and
  `begin_enchantment_merge` plus actual holder/session encounter-order,
  definitions, compatibility, supported-item and write-back operations. Bind a
  real captured registry/tag/component owner. Default None explicitly lacks the
  capability; it does not turn empty components or fixture holder IDs into a
  production registry. Unsupported merge/reload contexts reject before effects.

The central `NativeStonecutterRecipes` provider is a recipe-owner binding,
not an anvil repair/text/enchantment provider. The current default connection
`RecipeContext::default()` does not resolve any
of those anvil providers. The granted Main milestone therefore binds the
named bounded provider and declares its supported component/profile slice. Opening a
UI, compiling this binding, or exercising fixture-only callbacks is not proof
of native rename/repair/enchantment operation.

The granted initial Main slice uses the context owner's finite
`PinnedNativeAnvilItems` repair/hover-name methods and literal text handling;
its own source ledger pins the original observations and exact supported
catalogue. It deliberately admits an unenchanted profile. Its recursive
`require_anvil_stack_context`, `require_anvil_inventory_context` and prediction
guard reject every nonempty ordinary/stored enchantment map with the typed
`UnsupportedAnvilEnchantmentContext`, including inventory, cursor, incoming
pickup and nested contents. The generic kernel's ability to retain a base
enchantment map does not supply the actual configured holder equality owner
for Main synchronization. No empty or protocol-ID holder provider substitutes
for that unavailable owner.

## Review/validation boundary

With the central assignment now granted, the first small stage must provide
actual providers and
prove actual native open → canonical inputs → rename/result take → close, plus
state-ID correction and disconnect conservation through real Main callsites.
Use existing configured creative actors honestly; validate paid XP/wear only
when their actual player/world profile is admitted. Force a fallible callback
before publication and show unchanged inventory, inputs, XP, retained player/
Level RNG, owned world edits, entity IDs and ordered menu/item outbox, while
separately recording independent acquisition/constructor-entropy effects.
A focused compiler/Main protocol flow is sufficient for this
stage; comprehensive conformance testing remains deferred.

This contract was prepared against the named source revisions and exact symbol
names, then moved into the granted Main worktree. Source implementation and
verification are reported per owned module; this document alone claims no
protocol-client, deployment or completed-checklist result.
