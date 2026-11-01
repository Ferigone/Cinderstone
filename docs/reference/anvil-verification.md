# Anvil Main source composition

Instance `ws-20261101-7c2f`, issue3; exact narrow grant comment6564589182.
Source branch `teams/workstations/ws-20261101-7c2f/anvil-main` starts at actual
master `0ad717b1ab6c591d6a9fd0f76564bc8e8eb2a981`. It adds only the five granted
Main files and anvil reference notes. Existing Main/shared/runtime/manifest and
deployment files are untouched. Actual production callsites remain central.

## External compiler and native flow evidence

`/tmp/ws-anvil-main-probe-20261101a` is a NEW temporary source composition,
not an edited repository Main. Its library/current Main base is local
`41cd850e6cae926b168be211f26b393f510f7513`, whose tree equals the actual PR76
merge. It links the five owned new modules and adds only the agreed module
declarations/`players.anvils` field plus two requested pure gameplay methods to
new temporary copies. Those methods reuse actual MainItemGameplay validation
and prepare_menu_commit, dropping the prepared token without committing.
This proves the proposed source composes; it does not prove actual master
contains those methods, wires dispatch, or invokes the new namespace. The
later temporary proposal also connects actual use/click/rename, current-menu
recovery, cross-open, real player body/pickup, close and save/shutdown phases.
These remain new external source copies, never replacements for Root's files.
Root's concurrent Loom PR81 changes several of the same callsites: compose its
actual current-owner hooks rather than copying this older baseline over them.

Final checks use an isolated target directory. A previous shared-target check
resolved stale metadata from another checkout; that result was discarded.
The isolated external `cargo check --locked --bin mc121-rust` passed, followed
by `cargo test --locked --bin mc121-rust main_anvil`. All focused native
context/runtime/pickup/lifecycle cases passed. These execute real native
SharedContainer, PreparedItemBatch, MainItemGameplay, core menu and owner
fixtures. They are not a Java gameplay oracle or a production socket flow.

After the source/callsite review, the same isolated filter passed all ten
focused native cases, including removal/loan/restore of the actual physical
owner. Corrections add Smithing close-before-Anvil, real LastAction reset and
the genuine body-loan pickup admission. Independent source review found no
remaining actionable defect in those three scoped changes. `rustfmt --check`
on changed source and `git diff --check` passed. The matching isolated log is
`/tmp/ws-anvil-final-wired-unit-flows.log`; it is local evidence, not a durable
CI artifact. No full-conformance or production-client result is inferred.

The composition records hashes of all five input modules. Root refreshes them
after freeze, before handoff. No Java/class/language asset, private save,
runtime checkpoint or compiled binary is committed. Per-family notes separate
direct javap evidence, original-bootstrap method/name observations and native
fixtures.

## Actual required central seams

- Declare the five modules and initialize `players.anvils: NativeAnvilRuntime`.
- Route actual use/click/rename, retained resync, player-phase tick, cross-open,
  close/disconnect, save/shutdown and immediate pickup through the exported
  methods in the accompanying contract. Preserve window0 cursor/grid.
- Supply `MainItemGameplay::validate_active_player` and
  `MainGameplayFrame::preflight_inventory_slot_changes` from the precise
  requested actual-owner checks in issue3/6564847678. They remain probe-only
  until central implements them or explicitly assigns those sections. Central
  acknowledged exact implementation and receipt of9531608 in issue3/6565372188;
  the corrective three-module delta was coordinated in6565534787 before
  publication. Neither acknowledgement makes the methods present on master.
- Run the actual incoming component/profile guard before each canonical slot
  or pickup mutation. A rejection must resync/retire the actual current Anvil,
  never fall through to hidden InventoryMenu state.

The admitted source slice is real alive unmounted creative Overworld/default
configuration, finite source-proved item/name methods and no unresolved
enchantment holders. Actual lifecycle XP is read; survival mutation/wear,
tag-dependent repairs, nonliteral/unknown text, nonempty9/23 and unavailable
multi-slot encounter order remain explicit errors. Source callbacks/default
criteria are certified from their actual owners. Independent readiness and
process factory entropy acquisition are not falsely promised rollback; actual
canonical gameplay/player-Level streams/items/IDs/outbox publication is staged.

No whole Anvil, all-menu, fixed checklist or full Minecraft compatibility is
claimed by this source milestone.
