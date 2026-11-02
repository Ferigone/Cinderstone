# Beacon ordered/live owner contract

Instance: ws-20261101-7c2f; WORKSTATIONS issue3; state ready_for_integration.
Base: 884a9ceb52bf3ed2344809ed67831893ef4bd556; separate worktree/branch
`teams/workstations/ws-20261101-7c2f/beacon-live-contract`.
Claim: [6566833766](https://github.com/Ferigone/Cinderstone/issues/3#issuecomment-6566833766).

Exact files are owned `src/workstations/beacon_block_entity.rs` and
`beacon_effects.rs`, owned `docs/reference/workstations-beacon-runtime.md`, new
`workstations-beacon-live-contract.md` and this new stage status. No Team4
actor/effect/callback/definition/arena, Main/world/ticker/index, current payment/
data/controller, persistence, manifest or checklist file is assigned or edited.

This reconciles Team4's actual live actor contract in issue3/6566707913. Its
actor APIs mutate canonical owners and preserve executed source prefixes on
failure/reentry. Calling them in a detached BE image or rolling back their
already executed callbacks is not a correct production binding.

The new additive ordered/live entry points require an actual owner-bound live
capability before mutation, then run the shared source algorithm directly over
the retained actual BE and actor/world services. They do not begin/publish a
stage, clone a BE/actor graph, roll back an executed prefix or retry an action.
Original Level/BlockPos argument leases stay captured even if the BE is removed,
replaced or reattached; captured actual objects do not redirect through reused
IDs/current index entries or fail mutable-version checks after their own valid
callbacks. Read-only/nonreentrant beam/world-query admission remains explicit.

Existing staged APIs remain compatible for explicitly proven restricted
domains. Their certificate does not authorize arbitrary canonical live actor
operations. Actual retained BE/actor arena, registry/definition/criterion/world
and sound owners are still required; this code registers none of them in Main.

Source, primary review and both ledgers are frozen. Root's isolated all-target
compiler check passed, followed by the focused workstations::beacon_ filter,
including both the earlier staged behavior and new live-only prefix case.
Formatting, staged-index repository hygiene and diff checks pass. Publication
and current-master composition follow this frozen source.
The non-Clone live fixture implements no TransactionHost and retains a boxed
BE and original Level arguments after injected removal/replacement/reattachment.
An activation callback error preserves its executed field/callback prefix and
leaves the replacement untouched. These controlled leases/recorders are not
production BE/actor/RNG/criterion/Main or Java gameplay proof. No additional
actor application, arena or source behavior is invented by that test.

Frozen SHA256:

- beacon_block_entity.rs:8999330558b41180a66b1fa702cfbe0381da2da61ec78b40b00f1103dbadcc71
- beacon_effects.rs:fdc88ac94591ae1c09b19cdaaef795196cd427b062c4979b12aa672eaa67b5e2

Maximum capacity seven; root plus two disjoint source authors, one independent
primary/API reviewer and evidence writer were used (five total). All four
assignments completed; root alone publishes/verifies. No synthetic servers run.
No aggregate checklist or full functional Beacon claim. No automatic
cross-chat activity or monitoring continues after a turn stops.
