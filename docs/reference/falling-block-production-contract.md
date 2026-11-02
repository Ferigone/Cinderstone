# Falling block: central Main integration contract

This stage targets Java 1.21, protocol 767. The original inner server has SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`;
the official mappings have SHA-1
`31c77994d96f05ba25a870ada70f47f315330437`. Evidence is direct JDK21 `javap`
inspection and explicitly identified original-runtime fact extraction, not REA
MCP execution or Main gameplay observation.

The inherited Anvil block callbacks are already ported in
`workstations::anvil_block` (PR83). The central adapter reuses that implementation.
It does not replace a falling actor with a dropped item or mark unsupported
free-fall as a successful stationary tick.

## Implemented adapter source

`main_falling_block_world.rs` lends the actual `RuntimeWorld`,
`NativeEntitySource`, global BLOCK `LevelTickScheduler`, shared sub-tick counter,
dimension facts and game time. Creation verifies the supported backend, dimension
layout and genuine pending/certified source image. Callback validation binds the
invoked state to its actual Block singleton and requires the real loaded queue.
It does not query the below state or minimum height ahead of the Java branch.

Scheduling uses `block_ticks::schedule` with NORMAL priority. The existing
scheduler increments the single shared counter before deduplication, including
duplicate attempts. It records real source-history use; the outer runtime must
also retain its `was_used` flag and include the queue in its normal checkpoint.

`main_falling_block.rs` implements `AnvilBlockHost` using those borrows. Its
on-place, shape-update and scheduled-tick functions call the existing source
kernel. The inherited superclass shape update returns the invoked state after
the real schedule attempt, as pinned `BlockBehaviour.updateShape` does.
Air and FIRE-tag reads use the existing original-state facts. Exact liquid and
no-context replaceability consume the complete original-registry fact fixture
`falling-block-state-predicates.json`, checking its source identity, schema,
26,684-state domain, range validity and SHA-256 binding to the actual compiled
placement catalogue. Missing/foreign facts and unknown states are errors.
False is an observed value only inside that complete certified domain, not a
fallback. Nonempty fluid is not a replacement for `BlockState.liquid()`.

The source `FallingBlockEntity.fall`, official lines 92–96, has these phases:

1. Normalize WATERLOGGED to false in the carried state, when the property exists.
2. Construct the actual falling actor at `(x + .5, y, z + .5)` using the shared
   entity ID allocator and actual constructor RNG/UUID sequence.
3. Replace the origin with the **original state's** fluid legacy block, flags3.
   This is a real world commit, including its ordered callbacks.
4. Attempt actual `addFreshEntity`; Java ignores both world/add boolean results.
5. Return the same constructed object. Anvil's `falling` callback then configures
   that object's hurt damage `(2f, 40)`, even if registration returned false.

Pure helpers provide the carried-state normalization, source spawn coordinates
and original-fluid replacement data. They do not construct, mutate, register,
publish or certify an entity. The mandatory `ActualFallingBlockOwner` associated
capability can represent the actual object with or without a registered handle.
A generation handle alone cannot represent a rejected add attempt.

## Ordered plan obligations: exactly one enqueue

The existing native transaction planner commits data and then executes its
ordered callback obligations. For Anvil writes, `Planner.write` now records one
`ScheduleBlockTick(delay2)` immediately after the actual changed `Write`, before
neighbour notices and recursive shape propagation. It also does so on a changed
property of the same Block: direct `javap dvi` of
`LevelChunk.setBlockState`, official lines 223–289, shows the server's new-state
`onPlace` call at bytecode385–394 after the state write, without an old/new Block
identity gate. An unchanged state returns before that callback.

Every Anvil `Planner.update_shape` now records its own schedule attempt before
returning the source superclass's unchanged state, in every direction. Duplicate
queue keys remain distinct **attempts** and consume the actual shared sub-tick
counter in event order. The selected block-item plan's older Anvil tick copy is
filtered during `plan_place`, because the write already supplied its obligation.
Standalone placement plans retain their explicit callback requirements; other
falling families and their existing plan behavior are unchanged.

Main must execute each recorded `ScheduleBlockTick` through the real scheduler
once. Its fluid/redstone callback phase must recognize the already represented
Anvil `onPlace`, not call the adapter's on-place enqueue again for the same
write. Similarly it must not repeat a planned shape enqueue. The callable
`on_place`/`update_shape` adapters remain available for a genuinely direct source
callback path that has not already recorded those obligations. An aggregate
queue count cannot detect incorrect duplicate attempt consumption.

## Pending central contracts at base 26c4d2e

| Owner | Exact required connection |
| --- | --- |
| Actual actor/index | Retain the single `FallingBlockState` and its real RNG; add `EntityKind::FallingBlock` to the existing shared index; use actual registration/movement/removal and source query order. |
| Level tick list | The actual same-generation falling actor must run at its real entry among Player/Item/XP entities. A separate loop or sorted-ID replay changes ordering. |
| Constructor | Borrow the shared `Server.entities` ID allocator and real process constructor seed owner. No fabricated UUID, Level RNG substitute, fixed seed or cloned independent allocator. Finish fallible capability checks before entropy acquisition; do not promise rollback of process entropy. |
| World callbacks | The origin flags3 write and later landing writes must execute actual fluid/redstone/falling shape/on-place callbacks, lighting and source-history transitions. Storage-only `edit_block` does not prove this contract. |
| Scheduler admission | `main_block_ticks.rs` must execute each ordered delay2 Anvil obligation once and dispatch after rereading the current Block singleton; properties of the same Block remain valid. |
| Native source | `main_native_source.rs` must certify only actual implemented on-place, shape, queued tick and saved falling-owner callbacks. Do not relax arbitrary unsupported callback checks. |
| Physics/landing | The real movement host must invoke collision/fall damage with the retained actor; concrete-water raycasts, replacement/survival, Anvil wear, drops and removal require their actual owners. |
| Tracking | Actual falling-block type, carried-state spawn data, metadata8 start position, shared visibility/pairing and motion publication must use the same actor. |
| Persistence | Extend the genuine complete native checkpoint with all owned falling states/RNGs and actual global tick order, preserving world/source binding and shared next ID. No edited world/hash can substitute for observed history. |

The Anvil scheduled callback first tests the actual below state in this order:
air, FIRE tag, liquid property, no-context replaceability. Minimum build height
is read only when below is free. An actual unchanged-original bootstrap found
liquid=true on water80–95, lava96–111 **and bubble column12960–12961**; waterlogged
solids are not liquid solely because they contain fluid. Class `LiquidBlock`
is therefore not a complete liquid predicate.

## Validation and limits

Main modules and concrete native entity/world capabilities are now composed;
the integration coordinator is performing focused compilation. The generic
borrowed adapters alone remain contracts, while the concrete native path below
uses retained actual owners. No Anvil placement, falling actor, socket menu or restart PASS is
claimed here. The previous actual Anvil harness stopped before menu opening at
the unported delay2 falling callback. No settled scene or checkpoint provenance
was fabricated to bypass that missing owner. Checklist scope/version and values
are unchanged.

## Concrete native callback assembly

`main_falling_block_spawn.rs` now supplies the actual native Anvil scheduled
callback and `ActualFallingBlockEditServices`. The scheduled callback borrows
the real world and LivePlayers, constructs with the shared entity counter and
process Entity RNG, executes the original-state fluid replacement with flags3,
then performs actual UUID/add admission. An unsuccessful add retains the same
constructed Box for the subsequent hurt configuration; no refund or replacement
ItemEntity is invented.

The landing service instead borrows only the disjoint actual world, item/index,
gameplay, chunk/light, source, BLOCK/shared sub-tick, random-tick-index and
publication owners. This permits the retained canonical falling actor to stay
addressable in its real Box throughout movement and callbacks. The first native
profile certifies actual inherited callbacks and a fluid/fire-free neighborhood
before mutation. Active redstone/fluid/fire callbacks, block-entity lifecycle,
loot and other non-quiescent plan effects remain explicit unsupported errors.
They are not accepted as empty callbacks.

A supported edit uses the actual source token, native atomic block plan,
random-tick index updates, ordered Anvil schedule attempts, lighting settlement,
source completion and block outbox. The normal Main plan/drain also recognizes
the already represented Anvil onPlace obligation, avoiding a second enqueue and
extra shared sub-tick increment. Scheduled admission requires the retained
actual configuration-backed damage owner installed once after runtime restore;
no persisted boolean or unconditional callback enable substitutes for it.

Movement receives a genuine current query-region certificate and an immutable
same-lock identity view of the actual XP/Falling stores. The views are captured
before borrowing the current actor and cannot manufacture an empty population,
old visibility, ghost players or a successful world add. Spawn-only callbacks
have no movement/drop context and keep those optional capabilities absent.

The integration coordinator performs the focused compiler/runtime validation.
These concrete source paths do not by themselves establish Anvil PLAY, falling
or restart success, complete generic FallingBlock behavior, or alter any fixed
checklist delivery.

## Future ordinary-family and water callback stage

The isolated follow-up based on `a204254` factors the genuine construction path
into `Server::preflight_native_falling_block_locked` and
`Server::construct_native_falling_block_locked`. Anvil and the separately
implemented ordinary sand/red sand/gravel adapter use the same real counter,
process constructor RNG, origin flags3 edit, UUID admission and canonical Box.
The common helper never enables Anvil damage; that remains the actual virtual
Anvil callback after the add attempt. Existing compatibility-named Anvil
schedule validators also recognize the exact supported ordinary source classes.

The original quiescent callback path is retained. The separate nested
`main_falling_block_callbacks.rs` profile removes one concrete restriction:
a single falling solid/air write may execute actual liquid WATER callbacks.
It borrows the complete existing `MainFluidRuntime`, including its simulation,
loaded/saved queues, health and the one shared sub-tick counter. The additive
`observe_change_with_parts` wrapper constructs the same `FluidTickContext` and
calls the existing `FluidSimulation::on_block_changed_with_flags`; no fluid
algorithm, queue, random stream or source owner is replaced.

The source phase matters. A new solid FallingBlock onPlace schedules its BLOCK
tick before neighboring water callbacks. The admitted profile therefore runs
the real full water callback at the ordered NeighbourNotice after that actual
FallingBlock schedule. The new-state fluid onPlace is proved literally empty.
Only exact base Block/AirBlock and LiquidBlock neighbors are admitted, and no
other block/waterlogged shape schedule may interleave. Consequently the real
kernel's neighbor and liquid-shape schedule order is preserved, including both
source/flowing singleton identity and counter increments on queue deduplication.
This supports a bounded ordinary water pool with base-block walls/floor; snowy,
plant, custom, neighboring FallingBlock and waterlogged shape contexts require
further phase-specific delivery.

Preflight checks the actual native source, current lighting, fluid history,
actual water-only neighborhood and callback declarations, real FULL fluid/BLOCK
halos, existing runtime health and dimension/rule facts before the write or
constructor entropy. Lava hardening/ignition, fire, active redstone, block
entities, loot, extra writes and unknown facts remain errors. The real Level RNG
is lent to the kernel even though this bounded scheduling-only water path draws
nothing. The shared fluid and BLOCK schedulers use the same retained counter.

This follow-up is source implementation awaiting the coordinator's focused
compiler/runtime check. It does not establish an ordinary/water gameplay pass,
generic concrete-powder/lava behavior, full FallingBlock compatibility or a
fixed-checklist completion. No Java binary or raw disassembly is published.
