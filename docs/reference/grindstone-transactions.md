# Native grindstone transaction ownership

Status: source contract and native staging work in progress. An isolated native
mixed entity preparation/publication capability is implemented below and passed
the focused library compiler check. This document does
not establish a production Main call, a working result-take path, or completion
of an inventory checklist item. The kernel was published in PR73; the native
registry and opening/access helpers are separate integration work.

## Original evidence

The original Java 1.21 inner server has SHA256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Official server mappings have SHA1
`31c77994d96f05ba25a870ada70f47f315330437`.
Inspection used JDK21 `javap` directly, not a REA MCP service. Raw disassembly
and Java artifacts stay outside the repository.

Relevant mappings are `GrindstoneMenu -> cqt`, its result slot `cqt$4`,
`ExperienceOrb -> btc`, `Level -> dcw`, and `RandomSource -> ayw`.
`cqt$4.onTake` (original lines72–81) invokes the captured level/position
callback first, then clears input0, then input1 using their actual container
setters. The callback (lines73–77) calls actual `ExperienceOrb.award` at
`Vec3.atCenterOf(blockPos)`, emits level event1042/data0, and returns. A
grindstone has no block entity or stored XP balance.

`getExperienceFromItem` (lines97–108) visits the actual enchantment entries,
excludes entries in the actual curse tag, and accumulates minimum enchantment
costs with Java integer arithmetic. `getExperienceAmount` (lines84–93) adds
both input totals with wrapping integer addition. For a positive total it
computes `bound = (int)Math.ceil(total / 2.0)` and returns
`bound + level.random.nextInt(bound)`. The bound1 call still consumes the
actual random draw. A nonpositive total returns0 without this draw. The
subsequent `ExperienceOrb.award` invocation still happens for0.

The integer award is not a furnace recipe reward: no floating-point recipe XP
calculation or global `Math.random` rounding belongs here. Orb award splits
the integer into denominations, consumes actual Level RNG for its per-piece
merge selection, and uses each actual newly allocated entity's RNG for its
constructor. The captured block center is the award position; the player's
current position is not a substitute.

## Real kernel and native owners

PR73 source `b4bccbad9794896b4d1dffa68d0a02eea8dbff12`, published head
`5c54f98a203f04eab7819b5ae13a5d222af55122`, exports
`GrindstoneMenu<K,S>` and `GrindstoneMenuHost`. It uses one temporary
`GrindstoneState`, one menu cursor, and a borrowed canonical player inventory.
The host is transactional: `begin_stage` isolates every affected owner;
`publish_stage` validates and publishes the complete staged operation before
the kernel installs its next menu state. It is not the SmithingMenu phased
event-vector API.

Native binding must retain the actual actor identity, actual Level and captured
block position, actual menu lifetime, configured registry/tag/feature snapshot,
and the same canonical inventory throughout the action. A window number,
dimension string, equal stack contents, or newly created registry is not a
replacement for these identities. Opening must construct the menu before the
OpenScreen packet; initialization callbacks and actual ownership installation
follow that packet. Provider/access helpers currently expose the actual
MenuType15/title, block/range observations and post-open statistic/criterion
phases without inventing an active menu.

The native enchantment helper reads the actual immutable configuration packets
and certifies them against the pinned default profile. Its snapshot participates
in every kernel binding check. There are no crafted-item, recipe-book or recipe
award callbacks in the original grindstone result-take path.

## Ordered staging requirements

One real source action needs one proposal containing the canonical inventory,
menu cursor/inputs, actual player RNG, actual Level RNG, actual entity-ID owner,
actual shared entity index, real XP-orb runtime, drop statistics and ordered
listener/packet/effect publication. Every callback observes its actual
`GrindstonePhase`; source `Inventory.setChanged` is distinct from a slot
listener and increments the canonical transient counter only at that phase.

The XP stage must advance its retained Level RNG and orb/index proposal
**during** each `award_experience_orbs` callback. Quick-move can execute result
takes repeatedly. A later `getExperienceAmount` draw must see the intervening
orb merge draws and earlier staged orbs. Recording amounts and preparing all
rewards after the click changes the seed sequence and merge candidates.

Player drops use that same proposed Level RNG, player RNG, index and ID source
at their actual callback position. Result THROW runs result `onTake` before
the actual returned-item drop. Input/cursor overflow and ordinary THROW have
different trace/stat ownership. Source index insertion order, generation
handles and section encounter order must include the mixed Item/XP additions
in their original order. Sending packets in the right order after committing
all items first does not establish this state ordering.

Alive close returns the actual carried stack and the two inputs through
`Inventory.placeItemBackInInventory(stack,true)` and its real raw inventory
packets/overflow drops. Dead/disconnected close uses real drops. Result preview
is derived and is never returned or spawned; closing alone awards no XP.
Window0's actual cursor/baselines transfer only after removed finishes.

## Implemented mixed native registration capability

`runtime_entities.rs` now exposes `NativeEntityAddition`,
`ExperienceOrbRegistration`, `PreparedNativeEntityBatch` and
`NativeEntityBatchEvent`. `ExperienceOrbRegistration::from_state` validates the
actual externally owned orb state and its spawn/velocity representations,
then captures immutable registration facts. Physical orb state and RNG remain
with the actual XP owner. The token exposes no arbitrary index mutation.

`RuntimeEntities::prepare_native_entities` validates the whole ordered Item/XP
ID sequence and generation capacity, actual Item sender/spawn state and total
item outbox revision capacity before retaining an exclusive store borrow.
Dropping a prepared token changes no actual entity, index, outbox, tick list or
random owner. Item preparation and insertion share the original item-only
helpers, preserving the old prepared-item/sound API.

`commit_with_world_random_ordered` consumes that token and inserts the actual
entries in their proposed order. It reports `Before { ordinal }`, then
`Registered { ordinal, handle, kind }` after the actual insertion, and finally
`Complete { len }`, including the empty sequence. An Item is in its real
store, tracking outbox and tick owner before Registered. XP uses the real
index/section/tracking/tick registration before its corresponding Registered.
Callbacks receive only the existing narrow sound-seed capability. They cannot
register extra entities, reorder this token, or mutate the store. The actual
generated handle lets the XP owner validate/bind its prepared state at the
correct source position; drop statistics can follow their actual Item add.

Synchronous staging already consumed the real proposed Level RNG. The batch
installs its final state once under the same exclusive borrow; callback replay
must use validated prepared observations/effects and cannot issue new Level
RNG reads. New orb physical-state installation, merge checkpoints and packet
delivery remain the real XP owner's responsibility. The native Host must
finish all their fallible validation before committing this token.

Focused verification used source base
`5553015344f939f6bb255f414439e1c3b022af5a` plus runtime diff SHA256
`ed9d64c32b8761e6274ff4d1728d9286890a4973a027865950add44cbdac34cd`.
`cargo check --locked --offline --lib` passed in22.04s using a separate target
directory and the actual pinned dependencies. No test suite or Main runtime
check was run for this isolated capability. `git diff --check` also passed.

Read-only source review of the subsequent real native XP proposal and Host
confirmed that each award immediately advances its one staged index/Level RNG
and retains earlier orb merge state before another grindstone amount draw.
Direct `btc.award`/merge, `bsr` construction and `ayo.createInsecureUUID`
inspection confirms the per-denomination nextInt40, first matching actual
section result, merge count/age update, separate entity RNG, two UUID nextLong
draws and four orb constructor nextDouble draws. The Host's mixed replay pauses
at each insertion span, binds actual registered handles, applies drop stats
after Item add, and finishes each XP checkpoint before subsequent source
effects. Merge-only/zero awards consume no registration ordinal. This scoped
source review found no new concrete ordinal/RNG defect; it is not a Main
runtime validation or a complete workstation compatibility claim.

## Concrete integration prerequisites

1. A retained cloneable XP proposal must accept repeated integer awards against
   the same staged orb runtime, shared index, Level RNG and next entity ID. It
   must expose validated registration/publication checkpoints for the mixed
   Item/XP order, without a global Math.random owner.
2. The implemented mixed prepared batch must be connected to that real XP
   proposal and the Host's one ordered action timeline. The original item-only
   batch alone remains insufficient. Runtime verification of the complete
   native composition is pending; this additive library API is not Main
   integration proof.
3. The inspected GrindstoneMenu has only private `staged_copy`, no public Clone
   or prepared-action object. It also lacks a public full-state resync wrapper.
   The native owner must preflight every fallible world publication inside
   `publish_stage`, or obtain a source-preserving prepared/rollback capability;
   mutating the retained menu then discovering a failed item/XP preparation is
   not an atomic transaction. Spectator resync needs the real local listener
   phase plus full remote state, not only a fabricated content packet.
4. Root still owns Main exports, runtime field, actual opening/dispatch,
   per-player broadcast-before-validity tick, cross-menu close, current-window
   close, body/pickup, autosave, EOF/death and shutdown assembly. Ordinary
   autosave must retain open menus. Integration must preserve the existing
   outer chat-operation guard before world/player locks and never re-enter it
   from a locked helper.

These are specific ownership/API prerequisites. None is replaced by a dummy
host, empty registry, invented level identity, direct player XP increment or
furnace reward. No Main connection or runnable Grindstone action is claimed by
this reference note.
