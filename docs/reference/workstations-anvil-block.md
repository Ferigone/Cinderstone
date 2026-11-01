# Anvil block callbacks and actual owner contracts

This bounded block-side port is separate from the Anvil menu controller. Its
claim is [issue3/6565604091](https://github.com/Ferigone/Cinderstone/issues/3#issuecomment-6565604091),
based on master `705bccbb7a5747de927501485721a38158924d92`. The
[Team4 boundary](https://github.com/Ferigone/Cinderstone/issues/4#issuecomment-6565604269)
retains generic FallingBlockEntity work under [WP53](https://github.com/Ferigone/Cinderstone/issues/53).
No entity factory, world, scheduler, Main, save or shared-runtime ownership is
transferred by the new `src/workstations/anvil_block.rs` module.

## Primary source evidence

Evidence is direct JDK21 `javap -p -c` inspection, version `21.0.12.1`, of the
original Java1.21 server and official mappings. This is not REA analysis,
decompiled Java or an executed Java gameplay comparison. The source lock pins
the outer server SHA1 `450698d1863ab5180c25d7c804ef0fe6369dd1ba` and mappings
SHA1 `31c77994d96f05ba25a870ada70f47f315330437`. The inspected inner server SHA256
is `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Java artifacts and raw disassembly remain outside Git.

| Original method, mapped source lines | Observed behavior |
| --- | --- |
| `FallingBlock` (`diu`) `onPlace`, 32–33 | Always schedules this Block singleton at this position after delay2. No support test or RNG draw occurs here. |
| `FallingBlock.updateShape`, 37–39 | Every direction schedules delay2 before invoking the superclass shape callback. The inspected default superclass returns the supplied state. |
| `FallingBlock.getDelayAfterPlace`, 56 | Returns2. |
| `FallingBlock.isFree`, 61 | Short-circuits in order: actual state `isAir`, actual `BlockTags.FIRE`, `liquid`, context-free `canBeReplaced`. |
| `FallingBlock.tick`, 44–50 | Reads the block below and calls `isFree` first. Only when free does it read the actual minimum build height; `position.y < minimum` returns. Otherwise it calls `FallingBlockEntity.fall`, then virtual `falling` on the returned entity. |
| `FallingBlockEntity` (`cjg`) constructor, 76–89 | Uses the real entity constructor; assigns carried state, blocksBuilding, position, zero velocity, old coordinates and synchronized start position. |
| `FallingBlockEntity.fall`, 92–96 | Constructs at `(x+0.5,y,z+0.5)`, clears WATERLOGGED on the carried state when present, replaces the origin with the **original** state's fluid legacy block using flags3, calls actual `addFreshEntity`, and returns that same entity. Both boolean results are ignored. |
| `AnvilBlock` (`dey`) `falling`, 97–98 | After `fall` returns, sets hurt parameters to `2.0f` and40 on that returned entity. |

The scheduled tick's supplied Level RandomSource is unused by this source
method. A supported stable tick reads the actual below-state predicate and
returns without constructing an entity or drawing RNG. This does not imply
that falling entity construction has no entropy or identity effects: the real
Entity constructor owns its ID, independent random source and UUID generation.
Neither a menu RNG nor the Level tick RNG can replace those owners.

Actual scheduling remains LevelAccessor/LevelTicks work. Creation of a scheduled
tick reads actual game time, uses NORMAL priority, and consumes the shared next
subtick order before enqueue/deduplication; duplicate schedule requests still
advance that order. ServerLevel's scheduled dispatch first checks the current
state against the scheduled Block singleton before invoking its tick. A
property-bearing state is not a different Block identity.

## Borrowed callback boundary

`AnvilBlockHost` separates its associated `Block` singleton from `BlockState`,
and borrows actual configured tag/property queries, world bounds and scheduling
owners. Its associated `FallingEntity` is an opaque actual object capability,
not a second stored entity or a cloneable preview.
`on_place`, `update_shape`, `is_free` and `scheduled_tick` preserve the source
queries and callback order; `ShapeUpdate` supplies the actual neighboring shape
arguments. `AnvilTickOutcome` is `NotFreeBelow`, `BelowMinimumBuildHeight` or
`Falling`; the first is an `isFree` result, not a support-face/sturdiness test.

The host must preflight the source action over the actual world, position and
Anvil Block owner before mutation. This native admission check must not query
below/minimum height early, draw RNG or publish effects. The caller stages
canonical world state, queue/shared subtick state, entity index, player/Level
and entity-owned state, and publication together. A late error must discard
those proposals. Actual process-global constructor seed-factory acquisition is
an independent persistent acquisition: it may advance even if the staged action
later fails. The host must document that boundary before acquisition, finish
capability checks first, and never fake a stream, reseed it or promise global
entropy rollback. `FallingEntity` preflight must certify both actual `fall_block`
and the later same-object hurt-setting capability before construction.

The scheduling callback must enqueue into the real shared BLOCK scheduler,
including NORMAL priority and shared subtick accounting. Existing placement
plans already carry the source onPlace scheduling obligation. Central must
execute each original schedule call exactly once: dispatching this callback
and independently replaying that plan obligation would allocate two subtick
orders even if the queue deduplicated their matching ticks.
The superclass shape callback follows scheduling. A non-free below-state must not
require a falling entity capability or invent a successful entity callback.

When the falling branch is reached, `fall_block` must perform the actual
constructor → origin-fluid replacement(flags3) → add attempt sequence and
return the same entity for `set_hurts_entities(2.0f,40)`. An unavailable falling
owner is an explicit unsupported context before those dependent mutations;
returning an invented handle, skipping the origin write or assuming successful
registration is insufficient. The actual host owns staged publication and the
documented acquisition/error boundaries; the pure module does not certify missing
entity, world or reentrant callback owners.

The free-below test runs when the scheduled tick executes. Initial non-free
support does not prove future quiescence: shape updates reschedule, and removing support
before a due tick reaches the actual falling entity boundary. A future generic
entity tick, collision, landing/damage, tracking or persistence implementation
remains separate Team4/central work.

## Current Main blockers and required integration

At the base source, `placement.rs::placement_plan_for_item` already records the
inherited FallingBlock delay through `plan_selected_state`;
`world_interactions.rs::plan_place` emits its `ScheduleBlockTick`. The real Main
path has separate gates:

1. `main_native_source.rs::callbacks_can_complete` delegates to
   `callbacks_can_complete_except_owned_removal`, which checks every write with
   `redstone::owns_change_callbacks_after_fluid`. FallingBlock's `diu` onPlace
   class is not admitted, so it fails with
   `unported native onPlace/onRemove owner` before source-edit certification.
2. `entity_persistence.rs::prepare_owned_edit_with_block_ticks` asks the actual
   `NativeBlockTickOwner`. Main's implementation in `main_native_source.rs`
   admits fire, supported redstone and container delay5; it has no Anvil delay2
   callback/save certificate. Passing the first gate alone does not pass this
   scheduled-owner gate.
3. `LivePlayers::schedule_native_block_tick` in `main_block_ticks.rs` still
   admits only those existing families. Its shared scheduler, readiness/halo,
   subtick order and save/load records are real owners. `tick_block_world` has
   fire/container/redstone dispatch, with no Anvil callback. Interaction drain
   reaches that enqueue through `main_world_interactions.rs`.

Central integration therefore needs exact callback dispatch, shape-update
ordering, delay2 scheduling, current-Block validation, shared subtick/readiness
and save/restart ownership together. It must retain real fluid/neighborhood
callbacks and later unsupported-falling refusal. Adding a whitelist entry or a
standalone successful support query does not implement these contracts. The new
module does not change any of these reserved files or establish a Main callsite.

## Recorded diagnostic and remaining limits

Root's external local PLAY diagnostic used binary SHA256
`c75783e2edd2124d86149d7b9bb42e1d73e68e7aec314e65f80cb15663fa8a3c`.
The synthetic `WsAnvilProbe` player joined, but the genuine source-edit attempt
was refused with `unported native onPlace/onRemove owner` and recorded
`callbacks_fabricated:false`, `validation_bypassed:false`. Its local
`/tmp/ws-anvil-play-ngydce2w/first-server.log` remains outside Git. It is evidence
of the callback gate, not successful Anvil placement, opening or menu gameplay.

No production callback, scheduler/save integration, ordinary falling Anvil
placement, Java gameplay comparison or full workstation checklist delivery is
claimed here. Central owns the actual integration and focused native proof;
comprehensive conformance remains deferred. `scope_version: 1` and the fixed
100-item checklist are unchanged.

## Focused native verification

The separate stage worktree used an isolated target directory. All-target
compilation passed, followed by the `--lib anvil_block` runtime filter.
The native fixture borrows real FlatWorld, BlockTickScheduler, the actual
schedule/subtick primitive and retained LegacyRandomSource. It proves
schedule-before-super for all directions, actual delay2/NORMAL queue ordering,
shared subtick advance even when duplicate requests leave only one pending
tick, and the short-circuit stable/minimum/unavailable-falling branches.
Its falling capability is `Infallible`: no entity is fabricated for a passing
test. Source-order review separately checks the falling delegation and hurt
configuration; successful actual falling remains unverified.

Independent primary-source/API review found the constructor entropy contract
overclaim and the source comments were corrected before freeze. No remaining
scoped defect was found. Staged-index repository hygiene, changed-source
formatting and diff checks passed. Local logs are `/tmp/ws-anvil-block-check.log`
and `/tmp/ws-anvil-block-native.log`, not committed CI artifacts. These focused
checks do not complete Main callback/save integration or an aggregate delivery.
