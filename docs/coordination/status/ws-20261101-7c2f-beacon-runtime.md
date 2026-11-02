# Beacon runtime source stage

Instance ws-20261101-7c2f; WORKSTATIONS issue3; state ready_for_integration.
Actual base26c4d2ec13b885e51025e105f1edeb0fe78f6309; separate worktree/branch
`teams/workstations/ws-20261101-7c2f/beacon-runtime`.
Claim: [6565838994](https://github.com/Ferigone/Cinderstone/issues/3#issuecomment-6565838994).
Initial preparation base wasfb3f3f9. Before the first source commit, the pristine
unpublished preparation branch was renamed and a fresh stage branch created at
central Loom merge26c4d2e, preserving all three source files and mechanical
exports byte-for-byte by SHA256 comparison. No master merge, force or reset.
Current Main/current-menu changes are therefore the actual source base.

Exact owned files are new `src/workstations/beacon_block_entity.rs`,
`beacon_effects.rs`, `beacon_data.rs`, three mechanical declarations in owned
`workstations/mod.rs`, new `docs/reference/workstations-beacon-runtime.md` and
this status. Existing beacon menu/payment/controller, native block-entity store,
Main/world/registry/living effects/advancement/save/manifest/checklist files
remain untouched. No ownership transfer of their actual services is inferred.

This source milestone ports the beam/pyramid/server-tick algorithms, ordered
effect/criterion/sound delegation, ContainerData selection and power-field
persistence helpers. Beam scan fields belong to one actual Beacon BE; levels
and powers must be the same canonical fields backing the existing BeaconHost
data. It creates no second menu/payment, living effect map, holder registry,
advancement owner or BE factory. Full Name/Lock/component/BE serialization
continues through its real outer owner, not these field helpers.

Actual Main invocation: none. Native Beacon registration/ticker/save, actual
effect holder and living addEffect owners, configured construction criteria and
transactional callback publication remain central/Team4 integration contracts.
The existing canonical RuntimeEntities::next_sound_seed supplies the retained
Level.threadSafeRandom; no new RNG or fake ItemBatch is needed. Mandatory host
staging must validate the same actual BE lifetime and world/tag/registry context
before one publication. Independent process-constructor acquisition is not
promised reversible. Missing capabilities must error before publication.

Primary evidence is pinned original Java1.21/mappings with direct JDK21 javap,
accurately labeled. Independent primary/source/API review found no remaining
scoped defect after clarifying source coordinate and constructor entropy
boundaries. Initial all-target compilation passed atfb3; final isolated
`cargo check --locked --all-targets` passed on actual base26c4d2e, followed by
`cargo test --locked --lib workstations::beacon_`. Controlled native fixtures
verify incremental/ARGB/base query order, old-beam versus published-beam phases,
stage refusal/publication failure, data sound ordering, power load/save and
effect geometry/arithmetic. They use bounded recorder/holder/world fixtures;
they are not actual living-effect/criterion/Level integration or Java gameplay
execution. Final fixture RNG calls use the existing native next_i64 API.
Changed-source formatting, staged-index repository hygiene and diff checks
passed. Publication follows this frozen source and ledger.

Frozen source SHA256 values:

- beacon_block_entity.rs:37c75b8f0dac1181b4ab43463d7c092b2fc184239a0d73a146e7dce42e7474ab
- beacon_data.rs:546263fa6dcb4be841452abea596dd52129624a3924152de444fb77ec5c2604b
- beacon_effects.rs:ebbc73a18421f4dbfb9df64c23a35786dfdb82c953dcfa985ce174236a4f8e93

Maximum capacity is seven; this stage used root plus five disjoint source/
review/evidence roles (six total). Source and primary review assignments have
completed, including the frozen ledger; root alone now publishes/verifies.
Other workers completed, with no synthetic servers. No aggregate
inventory.4/crafting.2/crafting.5 row or functional Beacon compatibility follows
from this kernel stage. Actual BE removal/deactivation lifecycle, complete
Name/Lock/component serialization and native ticking integration remain outer
work; a passing controlled fixture does not supply those missing owners.

Previous stage PR83 sourcef841b50 is merged asfb3f3f9 after actual CI41363735448.
Merged tree equals validated ebb0b0ea42d0d3489ecb7cc53c7abc79896b0967.
Automatic Railway70be3a7e reached SUCCESS/native1.21/protocol767 listening at
2026-11-01T23:55:23.048009037Z; central Loom release subsequently superseded it.
One replica and the original /data volume remain. Public TCP/status is
unverified because this executor has no TCP grants. No manual deploy/config
action occurred. Main-Anvil source5b0d6f0/docacf5d04/PR82 stays frozen for
central integration-only adaptation, with its actual world-callback refusal
and absent Anvil socket/restart proof recorded. No automatic cross-chat coding,
issue polling or monitoring continues after a turn stops.
