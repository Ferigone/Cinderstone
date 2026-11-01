# PR #5 review and autonomous integration record

Instance: `ws-20261101-7c2f`; assigned issue [#3](https://github.com/Ferigone/Cinderstone/issues/3).
PR: [#5](https://github.com/Ferigone/Cinderstone/pull/5).
The user explicitly authorized this instance to review and merge its own PR,
document actual results and continue independent work. The new authorization
was [recorded on the team issue](https://github.com/Ferigone/Cinderstone/issues/3#issuecomment-6563463929)
and [integration queue](https://github.com/Ferigone/Cinderstone/issues/1#issuecomment-6563464141).
It supersedes the earlier team no-merge instruction for this PR. Other teams'
file ownership and the existing fixed checklist definitions remain in force.

## Review scope and findings

Initial reviewed head: `f461e81a71a6a0feb6b27f71a1a6a95e9911a8b1`.
Initial master review baseline: `e8e8bc92a242eef8d568675bf94b0b1aae3ae336`.
Final source candidate: `3e934abc7db2fdb51c700e9b3e3959fb2cb8e096`.
Actual GitHub merge candidate: `7b5dc48ac485d0255864d497a01c95b95b53ed2d`,
with parents master `664b7e65abddf7a77871debc32d23964473f3c1e` and that source.
This includes the concurrently merged worldgen, effects and damage foundations.
Only the six workstation exports were relocated in `src/lib.rs`; all existing
shared exports are preserved. No master merge/rebase was pushed to the feature branch.
Six disjoint subordinate reviewers ran alongside root, within the observed
seven-slot capacity. Root alone performs Git mutations. Findings are source
behavior issues, not test-count-derived progress.

| Area | Concrete finding and disposition |
| --- | --- |
| Enchanting | Matching PICKUP incorrectly clamped negative `Slot.safeInsert` capacity. Corrected to preserve admitted target99/cursor1 -> target64/cursor36 and both actual inventory dirty callbacks. Commit `e68d3c4`. |
| Hopper/dispenser/shulker storage | Corrected the same signed-capacity mismatch while preserving zero-transfer callbacks. Added a nonreused UI lifetime so old action/close plans cannot validate after reopening the same real owner with a reused window ID. Real active-session binding for open plans remains Main-owned. Commit `db02af3`. |
| Crafter | Full matching input skipped source setter/dirty/intermediate broadcast when acceptance was zero; negative capacity could also lose excess items through the actual setter. Corrected both. Outside left-drop now observes the whole cursor before clear; right-drop observes the post-split cursor. Commit `58d504d`; independently cross-reviewed. |
| Stonecutter | Review found missing actual player-inventory dirty obligations, an incorrect self-hotbar SWAP shortcut and incomplete historical callback phases. Corrected signed/zero insertion, actual dirty obligations and complete callback/close phases. Focused checks and independent cross-review passed. Commit `f63c843`. |
| Smithing | Review found the corresponding missing player callbacks/self-swap behavior, signed-capacity clamp and omitted extra empty-target dirty callback. Corrected signed/zero insertion, actual player dirty callbacks, aliasing and per-input/cursor close phases. Focused checks and independent cross-review passed. Commit `64d96ce`. |
| Recipe context | Actual original resource/tag/RecipeManager calls produced default encounter order, stable preview-description sort and item feature metadata. `PinnedDefaultRecipeContext` borrows canonical recipes and actual features, rejects unsupported snapshots and delegates existing stonecutting assembly. Commit `a6d21ff`; independent review and native context flows passed. |
| Requested spectator resync | Exact retained `broadcast_full_state(&mut self, &PlayerInventory) -> Result<Vec<SyncEvent>>` wrapper added without reinitializing. Regression retains cursor/input/result/selection/drag, advances revision and rejects old prepared actions. Commit `3e934ab`. |
| Shared synchronization and other kernels | No additional concrete blocker in the inspected bounded paths. Missing registry/world/player/entity owners and remaining family click adapters remain explicit production limitations. |

Primary evidence remains the unchanged Java 1.21 server and official mappings,
inspected with direct JDK21 `javap -p -c`; relevant original methods include
`Slot.safeInsert` (`crq`), `AbstractContainerMenu` (`cpu`), actual player
`Inventory` (`cmw`) and `CrafterSlot` (`cqk`). Per-family ledgers retain exact
source boundaries. No REA execution, raw disassembly publication or original
Java gameplay observation is inferred from the review.

## Verification and publication

Each frozen correction passed its focused native regression before commit.
The production Main build passed on that exact merge candidate. Binary SHA-256:
`688e4c53c72c7ce8373439c03ec9fe051918d3bd8ea2cdfe5e035c6b93cab114`.
Its complete `src` tree is `30e4741a52823003ac9a359df2621205cdf131ed`.
Every workstation/context/adapter case in the full native library run passed.
The full library suite remains red because of two existing baseline failures:

- `protocol::tests::strings_enforce_utf8_and_java_utf16_limits`,
  `src/protocol.rs:502`, expects malformed UTF-8 `[1, 0xff]` to fail.
- `runtime_chunk_manager::tests::fixture_source_requires_observed_facts_and_rejects_null_factory_states`,
  `tools/runtime_chunk_manager_tests.rs:714`, expects the pending reason to
  contain `block-entity`.

Both exact fully qualified tests were independently executed on clean master
`664b7e6`, and reproduced the same assertions. The corresponding protocol,
runtime-chunk and test source bytes are unchanged by this PR. Initial short-name
`--exact` baseline filters executed no tests and were discarded; only the
subsequent actual one-test runs establish that baseline comparison. No reserved
core/test edit was made to turn those assertions green. These results were
[reported to the central owner](https://github.com/Ferigone/Cinderstone/issues/1#issuecomment-6563762797).
The focused workstation requirements and production compiler check pass; the
unrelated red baseline is explicitly retained in the merge review.
No merge or post-merge deployment result is claimed yet.

The initial PR head had no GitHub check runs or commit-status entries; a
`pending` aggregate with an empty status list is not a failed or passed test.
The local compiler/runtime evidence above is separate from GitHub checks.
The author cannot approve its own PR as an independent GitHub reviewer; the
source review and findings are recorded transparently instead.

## Integration boundary and next work

Merging this PR adds callable workstation foundations. It does not create new
Main invocation or complete `inventory.4`, `crafting.2` or `crafting.5`.
The coordinator's default [recipe/order request](workstations-recipe-context.md)
now has captured data and a native context API. Team 4's
[death/menu request](workstations-death-context.md) has an explicit source/API
contract. Canonical live Level/features, death/respawn and Main callbacks still
require actual owners. Main adapters must use canonical inventory/current
cursor, source callback phases, real recipe/stat/criterion/drop owners and the
single ordered player/entity output stream. There is no new writer or fake
world/advancement implementation.

The merge triggers the existing Railway master deployment. Its actual source,
release status, native listening, one-replica configuration, retained `/data`
volume and public TCP probe must be recorded after publication. A queued build
does not establish a successful release, and hosted health is not full gameplay.
