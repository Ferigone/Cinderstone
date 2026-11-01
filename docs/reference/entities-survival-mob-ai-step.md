# Native Mob AI phases and bounded Bat behavior

Instance: `es-20261101-1425-a7c3`; team issue [#4](https://github.com/Ferigone/Cinderstone/issues/4).
Implementation base: `c1baf5c70f07e53b6b76edaf84be5da57f6bd40b`.
This note covers `src/mobs/ai_step.rs` and callable Bat fragments in
`src/mobs/bat.rs`. They do not establish a production actor or a complete delivery.

Target: original Minecraft Java 1.21, protocol 767. Server SHA1:
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`; official mappings SHA1:
`31c77994d96f05ba25a870ada70f47f315330437`; inner server SHA256:
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Evidence is direct JDK21 `javap` and explicitly labeled original runtime probes.
No REA execution is claimed. Raw disassembly and Java artifacts stay outside Git.

## Mob AI and Bat state phases

`Mob.serverAiStep` (`btp.fm`, 780–825) increments actual `noActionTime` with Java
integer wrapping, retains the actual level profiler, and ticks sensing. Full
selectors run when wrapping `tickCount + entityId` is even or a second
`tickCount` read is at most one; otherwise both run `tickRunningGoals(false)`.
Target selector precedes goal selector, navigation, custom AI, move/look/jump
controls and debug packets. Original profiler push/pop/popPush order and error
prefixes are retained. The inherited effective-AI/side/immobility branch remains
the actual `LivingEntity.aiStep` caller's responsibility.

Bat (`cez`) constructor 40–52 executes its complete inherited constructor before
its two animation states and server-only resting flag. Metadata 62–64 delegates
first, then defines flags zero. Resting methods 117–127 preserve other bits.
`BatState` owns only nullable targetPosition and the two original animation
states; it creates no health, physical state, RNG, selector or NoAI flag.

Tick 131–139 runs inherited tick first, then either zero velocity and actual
setPos at `floor(y)+1-getBbHeight()`, or vertical velocity multiplied by 0.6.
Animation setup stops one state and invokes startIfStopped on the other.
Original AnimationState (`bsm`, 15–17) uses `(long)tick*1000/20`, including integer
division for negative ticks. Renderer/update APIs are not implemented here.

Custom AI 143–197 retains the source's conditional draws and world positions:

- Resting reads ABOVE's block state but tests that retained state as a conductor
  at CURRENT position. A supporting ceiling permits nextInt(200), optional
  nextInt(360)/head yaw, then the real nearest-player targeting query. Wake
  precedes optional level event1025; silent was read before the ceiling query.
- Flight invalidates targets with source short-circuit empty-block/minimum-height
  queries. Retarget tests nextInt(30) before strict center distance, then draws
  7,7,6,7,7 in original x/y/z read order.
- Steering preserves exact double-promoted float constants and original Mth yaw.
  Forward input 0.5 precedes the second yaw read/set. Final nextInt(100) may test
  ABOVE's state at ABOVE and set resting.

Hurt 217–225 checks invulnerability, then wakes a server-side resting actor before
inherited hurt; a later inherited rejection does not undo wake.

## Additional implemented Bat overrides

| Source method/lines | Native behavior |
| --- | --- |
| isFlapping,57 | Non-resting and `(float)tickCount % 10 == 0`; narrowing occurs first. |
| getSoundVolume,68 | Exact float 0.1. |
| getVoicePitch,73 | Actual inherited pitch/RNG body, then float multiply by 0.95. |
| getAmbientSound,79–82 | Resting consumes nextInt(4); nonzero returns null. Flight skips that draw. Otherwise resolve canonical ambient sound. |
| getHurtSound/getDeathSound,87/92 | Canonical registered Bat sound handles; hurt-source argument unused. |
| isPushable/doPush/pushEntities,98/104/109 | False and two genuinely empty source bodies. |
| createAttributes,112–113 | Actual Mob builder, registered MAX_HEALTH holder, builder add 6.0, actual returned builder identity. |
| getMovementEmission,201 | Existing native entity_move::MovementEmission::Events. |
| checkFallDamage/isIgnoringBlockTriggers,208/212 | Genuinely empty fall body and true. |
| readAdditionalSaveData,230–232 | Inherited read, actual getByte(BatFlags), canonical flags setter. |
| addAdditionalSaveData,236–238 | Inherited write, actual flags read, signed-byte putByte(BatFlags). |

These source-backed empty bodies do not authorize empty inherited callbacks.
Sound values are opaque canonical SoundEvent handles; no numeric sound ID or
packet owner is invented. The attribute API requires the real supplier builder,
not an actor AttributeMap or a replacement MAX_HEALTH definition. NBT transfers
all eight flag bits and delegates actual numeric/default getByte semantics. It
creates no save format; original Bat NBT does not serialize target or animation.

## Static spawn predicate and calendar

Bat.checkBatSpawnRules 241–257 reads actual sea level and rejects y>=seaLevel
before light/date/RNG queries. Otherwise actual getMaxLocalRawBrightness is read
before the calendar helper. Bat.isHalloween 261–265 samples the real
operating-system/default-zone LocalDate.now equivalent once, then reads
DAY_OF_MONTH before MONTH_OF_YEAR from that same immutable date. It accepts
October 20–31 or November 1–3. World time, an assumed UTC day or a fixed date are
not fallbacks.

Halloween sets bound 7 and skips the boolean draw. Outside it, nextBoolean true
rejects immediately. Then brightness>nextInt(bound) rejects; equality proceeds
to native spawning::check_mob_spawn_rules with the same canonical type, reason,
position and supplied RNG. No independent RNG is created. The static predicate
is implemented; actual date/world/RNG owners and NaturalSpawner admission remain
integration requirements.

## APIs and retained-state composition

ai_step::server_ai_step requires MobAiStepHost's actual sensing, selectors,
navigation, customAI, controls, profiler and debug owner. BatHost retains actual
metadata/physical/living fields, inherited bodies, world/targeting and actor RNG.
BatState::construct/tick/custom_server_ai_step and bat::hurt are bounded entries.
Optional BatSoundHost, BatAttributeTemplateHost, BatNbtHost, BatLocalDateHost and
BatSpawnRulesHost add the implemented contracts above without changing BatHost.
No unavailable owner has a successful default; errors retain preceding changes.

The normal integration route is
`retained_bat::{tick_retained,custom_server_ai_step_retained}` with
`RetainedBatHost` and its actual actor/state lease. Every callback-free state
fragment releases its borrow before inherited/world/RNG callbacks. The ordinary
inherited Living tick can therefore dispatch Bat custom AI on the SAME object,
and a retained old actor remains addressable after live-entry replacement.
Source target-field rereads, nullable closer receiver/argument evaluation,
saved velocity/local positions and animation state mutations are preserved.
Typed source-null errors keep the mutation/side-effect prefix. Two independent
source/borrow reviews and actual-file nested-call flows passed. Existing direct
`&mut BatState` APIs stay compatible for isolated capability-limited bodies;
they are not the normal inherited-tick integration path. No actual production
actor or Main invocation is implied by either API.

## Validation and numeric evidence

Focused native callback flows passed for source phase/profiler order, wrapping
cadence, wake-before-rejection, animation/target mutation, conditional draw order
and failure prefixes. Added flows cover ambient suppression, inherited pitch,
inherited-first/all-bits NBT, MAX_HEALTH 6 builder ordering, one retained calendar
date/boundaries, world/date/draw order, actual native RNG state and short circuits.
The added Bat flows used a Rust 1.99.1 standalone harness and the compiled native
crate's actual entity_move enum/random module. Earlier phase checks used 1.85.1.
Controlled recorder hosts are not production actors/worlds. The coordinator owns
final serial actual-crate checks and exact source-commit evidence on the PR.

Unchanged-original MobMathProbe ran under OpenJDK
21.0.12.1+1-1-deb13u1-Debian. Source SHA256:
`69af9c411a8ebd180d7bee373eec03914a1a96404fcdcfbd4aeeeb124ca8d1fa`;
output SHA256:
`fd5d4f4fdca8c06be67abb58c3df952b0032045fcc426a65597a4317de75adae`.
Both actual 257-entry asin/cos tables were read and actual Mth.atan2/float-wrap
outputs observed. All native entries match. The native helpers retain inspected
atan2/fastInvSqrt raw-bit/signed-shift/correction/quadrant order; libm is not
substituted. Axes, quadrants, finite fractions, signed zero, infinities and NaN
classification match the recorded observations. Other JVM table-initialization
last-bit behavior was not observed. Animation timing and Math.signum are also
pinned directly from source.

Additional sound/template/NBT/date bodies are supported by direct bytecode and
the described native callback checks. No original-Java runtime observation of
those added bodies, actual Bat AI or production gameplay is claimed here.

## Pending actual actor integration

This is ordinary source behavior without a NoAI or creative-only substitute,
but it does not construct/admit a complete ordinary Bat. Central still owns
shared ID/UUID/generation and constructor RNG; canonical physical/living,
equipment/effect/attribute state; complete inherited travel/tick/hurt/death;
actual selectors/sensing/navigation/controls; and world/targeting/date/sound/NBT
owners. Empty Bat goal registration does not permit skipping those owners.

Shared EntityTickList registration/removal, tracking/ordered packets, chunks,
complete save/restore, loot/XP dispatch and production Main callsites remain
pending. The separate loot team owns its namespace. Implemented NBT bodies are
not persistence integration; a static predicate is not natural spawning or caps.
Other interactions and animation-renderer behavior remain outside this scope.
No whole entities, mobs_ai or survival checklist delivery is claimed.
