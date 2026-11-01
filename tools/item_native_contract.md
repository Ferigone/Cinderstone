# Native item state and host contract

This describes the implemented primitive boundary, not complete server parity.
The reference is the unchanged Java 1.21 inner JAR, SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.

## One physical state

`ItemState.physical: EntityPhysicalState` is the authoritative physical state.
`Deref` preserves existing field access. `Entity.move`, base tick and item tick
mutate that same state; a host must not maintain independent default movement
or fluid copies. `position.on_ground` is the sole ground flag. Bounds are
retained explicitly because callbacks can observe their original timing.

`set_position_raw` updates coordinates/block position and invalidates the
cached inside-block state when the block coordinate changes. It returns whether
the original movement callback should run. Its numeric comparisons retain old
signed-zero coordinates when all coordinates compare equal.
`EntityMoveWorld.position_changed` runs after that update and before
`refresh_bounds`; a convenience `set_position` performs both stages without
the host callback.

The level calls `begin_level_tick` once, then `tick_full_body`. Standalone
controlled tests use `tick_full`, which includes that level step. Do not call
both. The old `ItemWorld`/`tick`/`Items::tick_all` path remains the scalar proof
interface and does not execute the full generic Entity lifecycle.

## Tick and synchronous effects

`FullItemWorld` extends `EntityBaseWorld`. The item body invokes the real base
ordering, then pickup-delay decrement, buoyancy/gravity, collision detection
with bounds deflated by `1e-7`, embedded escape, full movement, friction,
merge callback, age, a **second** water/lava update, impulse and despawn.
Merge callbacks execute at the original point and must observe/modify the
same shared world entity state and insertion/spatial order.

Base damage callbacks receive the physical state and the item's actual stack
and health at the original call site. Mutations/removal must be visible to the
remaining body. Removal during base tick does not itself stop the rest of
ItemEntity.tick. Explicit host callbacks cover portals, vehicles, particles,
fluid queries, RNG, sounds/events and leash state. Unsupported active contexts
must fail explicitly rather than return fabricated successful effects.

`has_impulse` and `hurt_marked` are separate fields. Item velocity changes and
fluid returns affect `has_impulse`; `Entity.markHurt` sets `hurt_marked`.
The server's tracking phase owns the appropriate clearing/publication timing.
The entity RNG and level RNG remain host-owned and must preserve actual draw
order. Entity.applyGravity is vector addition `(0,-gravity,0)`, including the
signed-zero effect of the X/Z additions.

## Dirty metadata and published views

`EntityPhysicalState.metadata_dirty` combines Entity fields 0..7 and item
Slot8. Setters preserve dirtiness if a value is restored within a tick.
`take_metadata_dirty` drains the complete mask. `take_generic_metadata_dirty`
clears only low bits 0..7, preserving bit8.

`ItemState::new` starts with Slot8 dirty, matching actual constructor setItem.
`set_stack` means replacement with a new Java ItemStack object, even when its
contents are equal, and marks bit8. `set_stack_count_in_place` does not mark
bit8. Merge destination uses replacement; merge source and pickup mutate the
existing stack count. The raw physical stack may therefore differ from the
last metadata stack cached by ServerEntity.

`stack_is_default_reference` records assignment of the canonical Java EMPTY
reference separately from a stack whose count becomes zero. The explicit
`set_stack_default_empty` setter changes that bit; in-place count mutations do
not. A cached nondefault metadata snapshot omits Slot8 only for the canonical
default reference. Dirty Slot8 still serializes its zero value when reset.

`dirty_metadata_packet(mask)` encodes the combined batch without draining it.
`metadata_packet` encodes the nondefault Entity fields plus Slot8 for a
published view. The host must maintain the actual cached tracker view and
publish dirty effects; comparing only the raw stack count per tick loses Java
reference semantics. The existing velocity-aware pairing API is retained.
Items advertise tracking range 6 chunks; loaded-chunk membership alone is not
the complete player tracking predicate.

## Evidence and current boundaries

* `python3 tools/entity_base_generate.py --check-rust` executes original
  Entity.baseTick with controlled lower hooks: 4,644 base calls, 1,648 default
  sprint/swimming decisions, 20,604 ordered callbacks and 6,292 dirty snapshots.
  The native two-test comparison passes; see
  `evidence/entity-base-conformance.json` for exact scope and hashes.
* `fixtures/item-full-vectors.json` contains 64 scenes / 2,616 successive states
  from the exact original cjh class, without Entity/ItemEntity overrides.
  `runtime_item_world` compares base tick, full movement, both fluid scans,
  support/inside/landing callbacks and final entity RNG. Its runtime report
  also records the independent Anvil comparison and 18 persistently submerged
  water scenes / 360 ticks, all with exact post-state and RNG matches.
* The three established item tests still pass after the shared-state and
  dirty-setter migration: scalar item physics, player drop/RNG and merges.
* `python3 tools/item_reference_generate.py --check-rust` generates 573 actual
  constructor/reference/count/merge dirty cases and exact original metadata
  bytes. All 324 emitted dirty packets now match after the independently proved
  inventory STREAM normalization fix. Do not weaken the packet assertion.
* `python3 tools/item_damage_check.py` verifies 728 hurt cases, all 1,333 actual
  destruction dispatch targets and 1,300 supported exact stack results. Base
  Item's hook is a no-op for 407 items. The 893 supported BlockItem defaults
  **mutate** absent CONTAINER to EMPTY in place; 33 present-container/bundle
  defaults remain explicitly deferred. This mutation does not mark Slot8 dirty.
* `python3 tools/item_pickup_check.py` executes original playerTouch and real
  original Inventory.add: 148 queries and 398 ordered callback snapshots.
  Native callbacks expose collect while empty/unremoved, discard while
  empty/removed, then stats/onPickup after the original count is restored.
  Native Inventory.add must retain the source item's data at zero count.

The full-tick corpora do not prove live server ordering/publication,
multi-item spatial merge order, player pickup, water-entry/exit splash,
lava/hazard ticks, portals, vehicles, leashes, all custom block callbacks, or
arbitrary Java component object identity. Those need their own original-runtime
evidence. The damage audit `tools/item_hurt_audit.md` remains separately scoped
as original-runtime prerequisites; `evidence/item-damage-conformance.json`
records the bounded native implementation and its remaining world limitations.
