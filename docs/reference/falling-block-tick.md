# Falling block tick and damage source

This stage ports the native bodies of `FallingBlockEntity.tick` and
`causeFallDamage` from Minecraft Java 1.21 (protocol767). It does not establish
Main invocation, a real falling actor, successful Anvil placement or full
falling-block compatibility by itself.

The reference is the pinned original inner server SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`, with
official mappings SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`. Evidence is
direct JDK21 `javap -p -c` inspection, not REA execution or an observed original
gameplay run. Raw disassembly and proprietary binaries remain private.

| Original mapped method | Native implementation | Preserved behavior |
| --- | --- | --- |
| `FallingBlockEntity.tick`, lines134–229 (`cjg.l`) | `falling_block::tick::tick` | Air discard and immediate return; original Block captured before movement; wrapping time increment; Entity gravity; real SELF movement and portal handling; server/alive-or-duplicate gate; final velocity scaling. No invented `baseTick`. |
| `tick`, concrete/landing branches | `server_after_move` | Current carried state's ConcretePowder identity, WATER-tag query, speed greater than1 raycast from old to current position with COLLIDER/SOURCE_ONLY and actual actor context; ground/water landing, bounce, moving-piston exclusion, cancel-drop branch, directional replacement context, below-state/free query, survival, exact WATER singleton waterlogging and flags3. |
| `tick`, placement/publication | Same function | Successful setBlock rereads actual world state for the BlockUpdate packet, then discards, invokes captured original Block's Fallable.onLand, and transfers real block-entity data. Failed setBlock with drops disabled leaves the actor alive; invalid replacement/survival discards unconditionally. |
| `tick`, lines206–226 timeout | Same function | Strict time greater than100 with Y at/below minimum or above maximum, or time greater than600. Minimum/maximum queries short circuit. Timeout spawns permitted drop before discard and does not invoke onBrokenAfterFall. Ordinary failed landings discard before their broken/drop callbacks. |
| `tick`, block-data transfer | `transfer_block_data` | Real BE lookup/saveWithoutMetadata; all retained BlockData keys copied; actual loadWithComponents; only the real load exception is caught/logged; setChanged follows both loaded and caught-load outcomes. No fabricated BE or omitted coordinate keys. |
| `causeFallDamage`, lines239–267 (`cjg.a(float,float,DamageSource)`) | `falling_block::damage::cause_fall_damage` | HurtEntities gate, Mth.ceil(distance−1), negative early return, real Fallable damage-source override or fallingBlock source, original ordered Level query and target hurt callbacks, actual ENTITY RNG wear, carried-state change or cancelDrop. Returns false even after damage. |

Damage uses the original `NO_CREATIVE_OR_SPECTATOR.and(LIVING_ENTITY_STILL_ALIVE)`
predicate and actual bounds. Its host returns genuine target handles in the
world query's encounter order. Neither sorted IDs nor an independently
reconstructed target list is accepted. Damage magnitude is the source f32
multiply followed by Mth.floor and integer minimum. Cast/conditional/wrapping
helpers retain Mth behavior at NaN, infinities and integer limits.

Anvil wear checks the current state after all victim callbacks. Only an actual
Anvil-tag state with positive damage draws one float from the same retained
entity RNG, at threshold `0.05f + distance * 0.05f`. It does not draw from Level,
player, sound or a new random stream. The incoming damage source and multiplier
are unused by the original override.

The required `FallingBlockTickHost` and `FallingBlockDamageHost` methods have no
successful default implementations. Capability preflight must validate actual
owner/provenance without making later source world queries or consuming RNG.
BlockData transfer additionally requires the actual retained CompoundTag
HashMap key encounter owner. The kernel validates its complete unique logical
keyset and copies values in that supplied order. Neither retained NBT wire/Vec
order nor sorted keys substitutes for the source map/history. A serialized Tag
alone does not certify this capability; unavailable map ownership remains an
explicit error, including after resume. The ordinary Anvil state has no BE data.

Actual motion, source callbacks, registry predicates, drops, world mutation,
tracking, removal, portal transition, BE operations and damage owners are
mandatory. Registry/provenance/unsupported-owner errors are not the caught
block-entity load-exception case.

The Level tick-list owner must call `EntityPhysicalState::begin_level_tick`
before entering the body and preserve the same actor through nested movement,
fall-damage, block and damage callbacks. These kernels create no alternate
physical state or allocator. A caller staging native failure atomicity must
include its real world/scheduler/index/actor/RNG/output owners; the kernel does
not promise rollback by cloning an isolated actor. Main adapters, real actor
registration/index/tracking, scheduled callbacks and persistence remain the
central integration stage. Compiler/runtime observations will be recorded only
after they actually run; no completion or checklist change follows this note.

## Concrete retained Main host

`main_falling_block_physics::tick_falling_block` now accepts the actual retained
FallingBlockState and its disjoint FallingBlockPublication plus the one real
ActualFallingBlockEditServices. It validates the actual handle/current query
region, calls the Level entry's begin_level_tick once, then invokes the native
kernel. Entity.move borrows the genuine physical field and a separate view of
its remaining damage fields; it neither clones physical state nor removes the
Box from its canonical map. Position callbacks update the shared section index
immediately. Movement uses the actual native collision shapes, fluid cache,
current region and rain/light owner. Entity.handlePortal's null-processor path
still decrements an actual positive portal cooldown.

The configured damage owner is captured once from the actual outgoing Fixtures
and retained through one Arc. Victim selection reads the actual Player-only
LivingEntity class in the current shared index, current bodies, modes and
health. Actual creative/spectator exclusions can produce an empty result;
unsupported survival hurt remains an error rather than a fake callback. Wear
retains the actor's own RNG. This Main stage supports Anvil actors, inherited
movement/block callbacks and the explicitly certified quiescent native world
edit profile. Concrete powder, active portals, overridden replacement rules,
nonempty block-entity data and other unported callbacks remain unsupported.

Successful landing sends the source reread BlockUpdate through the actual
falling sender's current tracked recipients before discard. Anvil onLand and
onBrokenAfterFall use level events1031 and1029 respectively, with the actual
silent gate, integer position/radius64 audience and no sound RNG draw. The
inherited Entity extinguish sound uses NEUTRAL6, not ItemEntity's AMBIENT8;
this was checked directly in bsr.de returning avq.g.

The falling-drop path is Entity.spawnAtLocation, not Player.drop: resolve the
captured original Block's actual item, honor the empty-stack gate, evaluate
both actual Level.random velocity doubles before the actual Entity constructor,
then consume the shared ID and process constructor RNG, two UUID longs and
bob/yaw floats. Set the real default pickup delay10 and attempt actual native
Item registration. Player/Item UUIDs are read from their current owners; XP and
Falling UUID identities come from the same-lock immutable actual cohort view
captured before the retained Box borrow. Missing views fail before entropy.
The actual UUID rejection consumes construction and leaves the item unadded,
without refunding the falling actor or replacing it with another entity.

These are source implementation and ownership contracts. Compilation and live
falling/Anvil observations are recorded by the coordinator only after they run.
