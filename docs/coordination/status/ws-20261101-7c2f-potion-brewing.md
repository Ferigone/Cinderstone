# PotionBrewing default rule source stage

Instance ws-20261101-7c2f; WORKSTATIONS issue3; state ready_for_integration.
Base9b1e48bfd9be7deda6c6fa9663ba00e68d8f254f; separate branch/worktree
`teams/workstations/ws-20261101-7c2f/potion-brewing`.
Claim: [6566249731](https://github.com/Ferigone/Cinderstone/issues/3#issuecomment-6566249731).

Exact owned files are new `src/workstations/potion_brewing.rs` and
`potion_brewing_vanilla.rs`, two mechanical declarations in owned
`workstations/mod.rs`, new `docs/reference/workstations-potion-brewing.md`
and this status. Existing brewing engine/controller, crafting RecipeBook,
item_nbt, Main/world/native BE/effect/persistence files remain untouched.

The source implements the default ordered rule builder/provider through a
required actual context. Registration key specs are source facts, not a
replacement potion registry. Canonical item/potion holders, feature membership,
component31 decoding and fresh stack construction require their actual owner
and immutable snapshot binding. Missing services error; no default/fake registry,
raw-ID holder, FixtureMixes or crafting RecipeBook stands in for PotionBrewing.

An unchanged public result borrows the exact original input. The existing
value-return brewing trait adapter requires an actual component-owner copy
certificate preserving identity or rejects unsupported context. A successful
mix creates one fresh count1 stack with target prototypes and potion-only
contents, discarding input custom color/effects and unrelated patches. Ingredient
consumption, post-shrink remainder, drop/RNG/ID/event/dirty ownership stays with
the existing BE engine and actual outer transaction.

Actual Main invocation: none. The inspected tree lacks a canonical builtin
Potion registry service and persistent component31 codec. Their construction,
feature/codec binding, BE ticking/save and world/menu callbacks remain central
contracts. Drinking, projectile and living effect behavior stays with Team4's
actual owners. This source stage cannot enable native brewing by itself.

Primary preparation is direct JDK21 javap of pinned original Java1.21/mappings.
An actual original-JAR bootstrap registry observation separately verified all
57 ordered source calls, expanded3 containers/2 container mixes/63 potion mixes,
and46 declared Potion keys. Native source specifications match those records
exactly; that observation executes no world/gameplay. Source StartMix calls
retain their outer target-feature gate instead of unconditional flattening.

Independent primary/API review is clean. Isolated
`cargo check --locked --all-targets` passed at9b1e48b, then
`cargo test --locked --lib workstations::potion_brewing` passed. Controlled
context checks cover source container/match gates, fresh result versus borrowed
unchanged ownership, feature short-circuiting, directional holders, required
identity-copy refusal and replaced snapshots. They provide no actual builtin
Potion registry, component owner, Level/Main binding or gameplay proof.
Changed-source formatting, staged-index repository hygiene and diff checks
passed. Source and ledger are frozen; publication follows.

Frozen SHA256:

- potion_brewing.rs:c16cc75177e0974bedd36f2fc67316b9fbe4782a029880aa5d70bc99271055c0
- potion_brewing_vanilla.rs:bc54d8df98079706545cd8c3c1badb5290363faaab8bd41a1c583a16a265036a

The provider's snapshot_key binds registry/features/components only. Menu
ContextKey must also bind the actual Level-owned PotionBrewing object/list
lifetime, because distinct actual recipe lists may share a registry context.
No new local counter or data equality substitutes for that actual owner.

Maximum capacity seven; root plus two disjoint source implementers, independent
primary/API reviewer and evidence writer were used (five total). All four
assignments completed; root alone publishes/verifies. No synthetic server runs.
Exact commits, publication and remaining contracts follow the frozen source. No
scope_version/checklist, Main/deployment configuration or licensing changes.
No cross-chat coding/polling/monitoring continues automatically after a turn ends.
