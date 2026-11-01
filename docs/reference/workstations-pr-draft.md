# Add native workstation transactions and production owner contracts

[PR #5](https://github.com/Ferigone/Cinderstone/pull/5) against `master` from
`teams/workstations/ws-20261101-7c2f/menus`. Refs
[issue #3](https://github.com/Ferigone/Cinderstone/issues/3).
Created and read back after the feature branch was pushed normally.
The production integration request is [posted on issue #1](https://github.com/Ferigone/Cinderstone/issues/1#issuecomment-6563262903).

Adds callable Minecraft Java1.21/protocol767 workstation behavior and explicit
actual-owner contracts. Stonecutter, smithing, hopper/dispenser/shulker storage,
enchanting and crafter adapters have all seven ordinary native click paths,
with documented context restrictions, authoritative ownership, state IDs,
phased cursor/inventory callbacks, stale/replacement guards and ordered closes.

Native kernels cover merchant pricing/selection/trade stages, enchanting costs/
seeded selection, crafter processing, hopper transfer/cooldown, shulker lifecycle,
dispenser/dropper selection/behavior callbacks and personal ender storage, plus
bounded anvil/grindstone/loom/cartography/beacon/brewing/lectern and special-map
recipe contexts. Real registry, recipe, player/world/entity and advancement
owners remain required; no fake production implementations are supplied.

There are no new Main callsites. This reviewed callable foundation can merge
under the user's explicit authorization; opening/dispatch, real state-owner
binding/ticking/persistence and remaining contexts/families still need integration. `inventory.4`, `crafting.2` and `crafting.5` remain incomplete.
The [integration ledger](workstations-integration.md) gives exact source phase
contracts and [team status](../coordination/status/ws-20261101-7c2f.md) retains
source commits, owned files and dated publication history.

Only new workstation namespaces/adapters, additive RecipeBook smithing
predicates and mechanical exports change shared source. The standing export
exception adds `workstations` and
`container_workstation_{stonecutter,smithing,storage,enchanting,crafter}` only.
Main/runtime/world/player/container/furnace core, manifests, checklist metrics,
licensing and deployment files are unchanged.

Primary evidence: pinned original server and official mappings, direct JDK21
`javap`, and separately labeled restricted unchanged-Java helper observations.
No proprietary artifacts/raw disassembly or gameplay claims are included.

Earlier validation at source `7147da02c737360682386df4d73792cb8b03ae00`:

- `cargo test --locked --lib workstation` passed the focused native flows.
- `cargo build --locked --bin mc121-rust` passed.
- Both commands also passed against pinned master
  `404a5dab9c1b161b3587aa24e11703458c5a8edd` in a separate worktree with the exact
  source diff applied; no master merge or reserved Main edit.
- The original-base all-target check found unchanged shared persistence-smoke
  initializers missing existing Options/Server fields. That shared test was not
  changed by this team.
- No new protocol-client, live-menu or deployment validation is claimed.

A later publication pass repeated both focused native flows and the full Main
build against master `f40d8bf066ecb446b46a66e1d6d9465ffb233d6e`, including its
chat/session changes, in a separate source-overlay worktree. Both passed; no
master merge or Main edit occurred. The source remains `7147da0`. The posted
[integration request](workstations-integration-request.md) aligns publication
with that coordinator's ordered connection output and exact owner contracts.


Review source `3e934abc7db2fdb51c700e9b3e3959fb2cb8e096` corrects signed/zero slot insertion, actual
inventory callback phases, close/drop phases and reused-UI stale-plan guards.
It additionally supplies certified default recipe/feature context and the exact
central-requested retained full-state resync wrapper. See the
[review ledger](workstations-pr5-review.md) and its source/owner evidence.
The actual merge candidate7b5dc48 composes master664b7e6 with that exact source.
Full Main build and every workstation flow passed. The full library suite is
NOT green: two protocol/runtime-chunk assertions reproduce on clean master;
the ledger names their exact test cases and central report. Those source/test
files were not changed to hide the failures. No GitHub checks are configured.
