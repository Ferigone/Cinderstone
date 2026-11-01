# Default block-use criterion ownership

This bridge supports the explicit native fixture profile with an authoritative
empty criterion registry. It does not load vanilla advancement definitions or
implement location predicates, progress awards, rewards or advancement packets.

## Original source boundary

The Minecraft Java 1.21 server and official mappings are pinned in
`fixtures/source-lock.json`. Inner server SHA-256 is
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`;
official mappings SHA-1 is `31c77994d96f05ba25a870ada70f47f315330437`.
Direct JDK 21 `javap -p -c` inspection established these boundaries; this is
not an REA MCP execution or a Java gameplay observation.

| Official owner | Mapped method | Relevant behavior |
| --- | --- | --- |
| `ServerPlayerGameMode.useItemOn` | `aqw.a` | The ordinary main-hand default block-use branch calls the criterion after `useWithoutItem` returns a consuming result; the spectator provider branch returns earlier |
| `StonecutterBlock.useWithoutItem` | `dnw.a` | Open the menu, award `interact_with_stonecutter`, then return the consuming result |
| `CriteriaTriggers.DEFAULT_BLOCK_USE` | `an.O` | Owns a `DefaultBlockInteractionTrigger`, mapped `bl` |
| `DefaultBlockInteractionTrigger.trigger` | `bl.a(ServerPlayer,BlockPos)` | Eagerly read the player's actual server-level block at the target, build the block-use context, then dispatch its predicate through the shared trigger |
| `SimpleCriterionTrigger.trigger` | `dv.a(ServerPlayer,Predicate)` | Look up this player's registered listeners and return before predicate evaluation when they are absent or empty |

The eager context uses the target center, actual player and post-use block state.
`LootContext.Builder.create(Optional.empty())` selects the existing level RNG
reference without drawing a random value. The empty-listener branch does not
evaluate the location/player predicate or award a criterion. This does not
justify omitting the preceding actual block lookup.

## Native bridge and caller contract

`MainItemGameplay::default_block_use(uuid, target) -> Result<()>` is available
within Main. Its existing registry owns the bounded outcome:

1. `require_supported_floor` requires the explicit
   `GameplaySourceScope::NativeFixtureEmptyCriteria`, with no invalidated source,
   nonempty criteria, statistic objectives or unsupported block listeners.
2. The UUID must belong to an actual active player registered with this owner.
3. The existing `empty_criterion` path dispatches
   `GameplayCriterion::DefaultBlockUse` and appends
   `GameplayObservation::EmptyCriterion` to the existing bounded observation
   stream. This is a recorded empty dispatch, not an advancement award or a
   game packet.

The caller retains the actual world/player lock. Immediately after the ordinary
stonecutter use and interaction statistic, it queries the actual post-use target
through `world.block(target)` before invoking the bridge. `MainItemGameplay`
does not own the world; passing a position alone does not certify that lookup.
The position has no predicate consumer in the supported empty registry, so the
bridge does not construct a substitute world, player, block state or loot
context. The spectator provider path performs neither the block-use statistic
nor this default-use criterion dispatch.

Existing criterion variants and callbacks remain available. A caller can record
a real unsupported registration using
`register_unported_criterion(uuid, GameplayCriterion::DefaultBlockUse, identity)`;
the source-floor guard then rejects this bounded callback before recording an
empty outcome. Imported/unknown data and invalidated source coverage are also
rejected. An absent imported advancement file never establishes an empty owner.

## Validation and limits

This small bridge reuses the existing source/registration guards and observation
owner. `git diff --check` passed. The integration coordinator owns the subsequent
full Main build and real stonecutter runtime flow after all shared adapters are
wired. No broad tests, checklist increment, nonempty advancement support or
complete vanilla compatibility is claimed by this source stage.
