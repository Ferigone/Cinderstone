# Anvil block-side source stage

Instance: ws-20261101-7c2f; team WORKSTATIONS; issue3.
State: ready_for_integration; base705bccbb7a5747de927501485721a38158924d92.
Branch: teams/workstations/ws-20261101-7c2f/anvil-block, separate worktree.
Claim: issue3/6565604091; Team4 boundary: issue4/6565604269.

Exact files: new `src/workstations/anvil_block.rs`, one mechanical declaration
in owned `src/workstations/mod.rs`, new
`docs/reference/workstations-anvil-block.md`, this new stage status. No existing
Main/world/entity/scheduler/persistence/manifest/checklist file is assigned.
This separate status avoids overwriting the parallel Main-Anvil handoff.

The source ports only Anvil's inherited FallingBlock block callbacks: onPlace
and every updateShape schedule the actual Block identity after2 ticks; tick
checks actual support and minimum height, then delegates falling to its actual
opaque entity owner before setting Anvil hurt parameters. No new live registry,
entity, RNG, world, scheduler or save owner is supplied. Generic FallingBlockEntity
constructor/tick/tracking/persistence remains Team4 WP53/central ownership.
Missing required capabilities must return an explicit error.

Actual Main invocation: none in this stage. Central must bind the real onPlace/
shape callbacks, one global BLOCK queue/subtick/checkpoint, matching-Block tick
dispatch and native source proof. Existing placement-plan scheduled ticks and
callback execution must not schedule twice. Accepting an Anvil class in a
whitelist is not integration. The source evidence and exact caller contract are
in [the reference](../../reference/workstations-anvil-block.md).

Preceding Main source is frozen at5b0d6f0, documentationacf5d04, draft PR82.
Both source CI41362735611 jobs passed; isolated actual-Main composition and
focused native owner checks passed. A stock localhost protocol767 client
attempt could not prepare Anvil through genuine SourceEdit: actual refusal
`unported native onPlace/onRemove owner`. No guard bypass, fabricated callback
or successful Anvil socket/restart result. Exact note: PR82/anvil-verification.md.

Validation: isolated `cargo check --locked --all-targets` passed, followed by
`cargo test --locked --lib anvil_block`. Native fixtures use real FlatWorld,
BlockTickScheduler, schedule/subtick primitives and retained LegacyRandomSource:
all shape directions retain schedule-before-super, seven scheduling attempts
consume shared orders17..23 while deduplicating to one pending tick, and stable
support neither reads minimum height nor acquires a falling owner/RNG. An
unavailable falling owner fails before dependent mutation. These are native
owner contracts, not successful falling-entity or Java gameplay execution.
Staged-index repository hygiene, changed-source rustfmt and diff checks passed.
Independent source review corrected the process-constructor entropy boundary;
no further scoped defect was found. Source SHA256 is
`c89e05237240180ae0dffa424b6fc787a3c892a7165bf62af041165196281df7`.

Capacity maximum seven. Three source/review/evidence roles completed this stage;
root now publishes/verifies. A separate read-only Main/Loom composition review
is continuing; no subordinate block implementation or synthetic server runs.
Actual publication/PR commits follow the frozen source. No aggregate inventory.4/crafting.2/
crafting.5 completion or fixed checklist change is claimed. Cross-chat polling,
coding and deployment monitoring do not continue automatically after a turn ends.
