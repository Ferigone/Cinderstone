# Java 1.21 living and player damage dispatch

Team issue: [#4](https://github.com/Ferigone/Cinderstone/issues/4).
Instance: `es-20261101-1425-a7c3`.
Base: `f1432ed8c57797397d79366e48182291e39c756a`.
Branch: `teams/entities-survival/es-20261101-1425-a7c3/living-damage`.

This stage ports damage control flow with mandatory synchronous actor callbacks.
It does not introduce a production living actor or complete
`survival_combat.2/.3/.4/.5`. Main, actual state/evaluator ownership, death/respawn,
packets and persistence remain integration work. A delegated virtual callback
is a required implementation contract, not evidence that its body is native.

## Reference identity and evidence type

`fixtures/source-lock.json` pins Minecraft Java 1.21, protocol 767, DataVersion
3953. The original server SHA-1 is
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`, size 51,623,779 bytes;
official mappings SHA-1 is `31c77994d96f05ba25a870ada70f47f315330437`,
size 7,454,609 bytes. Both size/hash checks passed. The unchanged inner server
SHA-256 is `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.

Static evidence is direct JDK 21 `javap -c -p -l`, with official mapped names and
original class line tables. Relevant classes are `LivingEntity` (`btn`),
`Player` (`cmx`), `ServerPlayer` (`aqv`), `DamageSource` (`brk`),
`DamageTypeTags` (`awg`), `CombatRules` (`brg`) and `CombatTracker` (`brh`).
REA tools were not exposed; no REA execution or Evidence ID is claimed.
Raw disassembly, binaries and probe artifacts remain outside Git.

The separate original-Java probe below executes original `LivingEntity.hurt`
with controlled generated callbacks/world/health/equipment/source contexts.
Those injected implementations do not establish a real world, native actor,
full damage evaluator or production runtime observation.

## Callable implementation

| Native entry point | Original source method and lines | Actual owner required |
| --- | --- | --- |
| `living_damage::hurt` | LivingEntity.hurt, 1133–1278 | `LivingDamageHost`: canonical actor/source and every called virtual operation |
| `player_damage::hurt` | Player.hurt, 945–977 | `PlayerHurtHost`: abilities, shoulder entities, difficulty and real parent living dispatch |
| `player_damage::actually_hurt` | Player.actuallyHurt, 1044–1070 | `PlayerActuallyHurtHost`: reductions, absorption, food/stat/combat/health/game-event owners |
| `player_damage::server_player_hurt` | ServerPlayer.hurt, 792–811 | `ServerPlayerHurtHost`: server properties, spawn timer, real causing entities/PvP and parent Player dispatch |
| `combat::reduction::get_damage_after_armor_absorb` | LivingEntity.getDamageAfterArmorAbsorb, 1699–1703 | Real tags, armor durability, attributes, direct-entity weapon and server enchantment executor |
| `combat::reduction::get_damage_after_magic_absorb` | LivingEntity.getDamageAfterMagicAbsorb, 1707–1746 | Actual effects, tags, resisted statistics and equipped enchantment protection |
| `combat::totem::check_totem_death_protection` | LivingEntity.checkTotemDeathProtection, 1290–1319 | `TotemHost`: canonical hand stacks, copied stack, actual stats/criteria/game event, health/effects and event publication |

Each entry point is fallible and stores no health, absorption, dead/alive flag,
entity identity, RNG or timer. Host callbacks have no default implementations.
Errors preserve the mutation/callback prefix already executed; callers must not
retry the entire hit as a transaction. Opaque entity/weapon/source handles must
retain the actual original object identity, including generations where native
IDs can be reused. A source name or registry ID alone is insufficient.

The only change to `survival.rs` adds the fallible
`try_damage_after_armor_with_modifier` helper. Existing APIs/bodies are unchanged;
reducers reuse existing armor, resistance and magic formulas. Only mechanical
`pub mod combat`, `pub mod living_damage`, `pub mod player_damage` declarations
are added to `src/lib.rs` under the coordinator's standing narrow exception.

## Damage ordering established by original source

Living hurt first checks actual invulnerability, logical side, dead/dying health
and fire resistance. Sleeping cleanup precedes resetting `noActionTime`.
Preserve the original amount separately for criteria. Shield damage receives
that amount, then the working amount becomes zero; nonprojectile direct living
attackers receive shield retaliation. Freezing amplification and helmet damage
follow, even when blocking made the working amount zero. Walk animation speed
changes before cooldown rejection.

If physical invulnerability time is greater than ten and the source does not
bypass cooldown, an amount no greater than `lastHurt` returns false after those
earlier callbacks. A stronger hit invokes `actuallyHurt(amount-lastHurt)` before
updating `lastHurt`, without a fresh hurt animation/event/knockback/sound phase.
A fresh hit installs `lastHurt` and invulnerability time 20 **before** its virtual
`actuallyHurt`, then sets hurt duration/time to ten. Failure at `actuallyHurt`
therefore leaves different canonical state prefixes for fresh and repeat hits.

Causing entity drives attacker attribution. `NO_ANGER` and the wind-charge/type
exception suppress mob anger attribution, but do not suppress player credit.
A tame-wolf source gets the source's specific player-credit behavior: set the
credit timer to 100, and install its owner only when that owner is a Player;
otherwise clear the credited player. This branch is not generic tame-animal
handling. Retain the original causing entity through later criteria.

Fresh hits publish shield event 29 or the source damage event, then conditionally
mark hurt, perform knockback and indicate damage. Knockback uses the direct
projectile's horizontal hurt direction when applicable; otherwise it queries
source position separately at the original null/X/Z callsites. Do not replace
the direct entity with its causing attacker or cache an earlier position.

Both fresh and stronger-repeat paths query actual dead/dying status after damage.
A lethal hit attempts real totem protection. Without protection, a fresh hit
makes the death sound, and both paths call `die`. An alive fresh hit calls the
hurt-sound method. The effective result is false for a fully blocked hit even
though callbacks may have run. An effective hit stores source then world game
time, invokes actual active-effect hurt callbacks, then runs victim/attacker
criteria and shield statistics. Effect iteration belongs to the actual owner.

Player hurt has its own invulnerability and ability/source-tag gates. It resets
no-action time before the dead gate and releases actual shoulder entities on the
server before difficulty scaling. Original difficulty comparisons read the
current world separately for Peaceful, Easy and Hard. Zero after scaling returns
false; other amounts enter the real parent living executor.

ServerPlayer adds a distinct wrapper: virtual invulnerability, the dedicated
server/PvP/fall exception, its separate spawn-protection timer and source-tag
exception, causing-Player permissions, causing-AbstractArrow owner permissions,
then the parent Player path. `spawnInvulnerableTime` is not physical
`invulnerableTime`. Skipping the parent wrappers would lose real source checks.

Player actual damage performs virtual armor then magic reduction; reads
absorption, computes health damage, reads absorption again and sets it; awards
absorbed statistics before the zero-health-damage return. A positive continuation
runs source exhaustion -> food callback -> CombatTracker -> live health read/set
-> damage-taken statistic -> ENTITY_DAMAGE game event. Do not use the existing
single numeric `HealthState.subtract_damage` operation in place of this sequence.

Java comparison, signed-zero and NaN branch behavior is retained. Statistics use
the original `Float.MAX_VALUE / 10` threshold and Java float-round semantics.
This does not promise arbitrary NaN payload identity: existing numerical helpers
normalize NaNs, and no gameplay claim is derived from NaN test inputs.

## Reductions and their real callbacks

Armor bypass skips durability and the formula. Otherwise virtual `hurtArmor`
runs first, so breaks can affect later attribute reads. Read floored actual
ARMOR, then toughness with its original double-to-float conversion. The source's
weapon comes from its **direct** entity. Only a nonnull weapon and actual
ServerLevel enter the server enchantment armor-effectiveness modifier, followed
by its extra 0–1 clamp. The no-weapon/nonserver branch has no extra clamp.

Magic's `BYPASSES_EFFECTS` returns the original amount before resistance or the
positive guard. Otherwise actual resistance and its bypass tag control the
wrapped integer amplifier arithmetic. Resisted damage awards the victim
ServerPlayer first, otherwise the causing ServerPlayer. Nonpositive remaining
damage returns positive zero; enchantment bypass is checked afterward. Only
actual server equipment/slot/enchantment protection is evaluated, and protection
greater than zero enters the existing magic formula. No synthetic empty armor,
resistance or enchantment context is provided.

Pinned resource inspection confirms, for example, `BYPASSES_INVULNERABILITY`
contains out_of_world/generic_kill, `BYPASSES_EFFECTS` contains starve and
`BYPASSES_ENCHANTMENTS` contains sonic_boom. The shipped inner jar has no
bypasses_cooldown tag resource. These are bounded artifact facts, not permission
to remove runtime tag queries or hardcode a datapack's registry membership.

## Central integration contract

The actual host must compose ServerPlayer -> Player -> LivingEntity -> virtual
Player actual-damage dispatch and bind its virtual reductions to the concrete
reducers, retaining source identity across every call. Calling a numeric helper
or only creative immunity does not satisfy this contract.

Canonical health/absorption remain metadata fields 9/15 in EntityRegistry;
hunger remains `PlayerLifecycle.food`; physical invulnerability time remains
`EntityPhysicalState.invulnerable_time`. New hurt/attacker/combat fields must
have one actual actor owner, including source/time and tick timestamps. Existing
saved `PlayerLifecycle.death_time` must not be duplicated. Server spawn
protection, walk animation and their actual tick phases also need explicit
ownership. ServerPlayer ticks its spawn timer separately from physical
invulnerability. No side timer is introduced by these modules.

Actual equipment durability/breaks, shield retaliation, armor/enchantment
evaluators, active effects, CombatTracker, criteria, statistics, totem/death,
knockback, sounds and game events are mandatory implementations. In particular:

- Totem protection checks the invulnerability-bypass tag, scans main/off hands
  in source order, copies the first matching stack **before** shrinking its
  canonical hand stack by one, then runs Player-only item-used statistic,
  used-totem criterion with that copy, and item-interact-finish game event.
  It sets health to one, clears actual effects, adds regeneration (900 ticks,
  amplifier 1), absorption (100, 1) and fire resistance (800, 0), then publishes
  event 35. Native `combat::totem` ports this ordering, including source default
  flags (nonambient, particles/icon visible), ignored effect-operation boolean
  results and consumed-stack retention on later callback failure. The caller
  must compose it into living hurt and implement every actual inventory,
  attribute/effect, statistic, criterion, game-event and packet callback.
- `die` has real removal/death guards, kill credit/score, sleeping cleanup,
  combat tracking, killer callback, death game event, loot/XP/wither-rose order,
  event 3 and dying pose. It cannot be replaced by setting a local dead flag.
- Hurt/death sounds resolve nullable actual sounds before their volume/pitch
  operations. Pitch consumes two actual entity RNG floats. Knockback's tiny
  direction retry uses global `Math.random`, not the entity RNG; packet sound
  seeds have a separate level owner.

Damage/status publication targets tracked viewers **and self** at the source
point; it is not a broadcast to every player. Damage-event packets retain
canonical type holder, causing/direct IDs and the optional **raw** source
position, not its computed fallback. Ordinary dirty metadata delivery and later
private PlayerHealthSync SetHealth delivery remain distinct phases. Item-only
sound, game-event and damage adapters are insufficient for living actors.

Native persistence currently lacks hurt/combat owners. Original LivingEntity
save writes Health, HurtTime, HurtByTimestamp, DeathTime and absorption before
attributes/effects. Java export retains some unowned keys, which would become
stale if damage were admitted without explicit snapshot/export updates. Respawn
command action zero currently performs no respawn. Menus, death drops/XP,
messages, tracking/removal, actor transfer and persistence must join a real
death/respawn implementation before admitting the relevant production contexts.

No Main or reserved shared runtime/entity/persistence file changes are included.
The creative-only admission profile stays in force. These are callable pipeline
cores and an integration contract, not proof of a running survival server.

## Focused validation

Full library compilation passed after installing a local Rust toolchain suitable
for the unchanged lockfile. Rust 1.85.1 was too old for its existing ICU packages;
no manifest/lockfile downgrade was made. Exact final compiler/test results and
source commits are recorded in the per-instance status file.

Focused native checks cover actual callback argument/order and failure prefixes,
not fabricated production actors. Totem checks distinguish main/offhand
precedence, pre-consumption copied components/count, a criterion failure after
actual fixture consumption, bypass before hand queries and non-ServerPlayer
callback omission. The original-Java totem recovery probe exercises controlled
non-ServerPlayer callbacks; its Player-only statistic/criterion path has static
source inspection and native trace evidence, not a real ServerPlayer observation.

The original-Java controlled probe preserves original `btn.hurt` and its private
totem scan; generated adapter subclasses inject surrounding contexts/callbacks.
Its exact trace assertions passed for fresh hits, equal/stronger cooldown hits,
shield/freezing/helmet order, lethal full/repeat dispatch, totem consumption and
recovery, invulnerability-bypass rejection of totems, effect callback amounts and
failure prefixes. For example:

| Controlled original operation | Observation |
| --- | --- |
| Cooldown 12, previous amount 4, new amount 7 | `actuallyHurt(3)` sees previous 4; later effect callback receives 7; no fresh event/sound |
| Blocked incoming 8 with freezing and helmet | Shield receives 8, helmet receives 0, actual damage receives 0; event 29; return false |
| Unblocked incoming 8 with freezing and helmet | Helmet receives 40; actual damage and later effect callback receive 30 |
| Fresh actual-damage callback throws | `lastHurt=4`, invulnerability 20 already installed; hurt time remains zero |
| Lethal stronger cooldown hit | Totem scan/death dispatch still runs; fresh death sound is absent |

Probe source SHA-256:
`fb94f6d48b4869c5c1b8c7dd9d61551d308cf37a0de08a0542518ad4d93a2055`;
output SHA-256:
`784175884621c4c936e837857c4b7cf92b5a302fef9bb218d4de2dd96a5e8207`;
trace assertion script SHA-256:
`86b77bc14eca8abc7c49bcfbf10eec27b2853dd03c2016a2700230fa9ccecd6d`.
These artifacts remain in the instance scratch oracle directory. The probe does
not validate native Main, actual world/equipment evaluators, player health
clamping, production packets, native persistence or real survival gameplay.
Comprehensive conformance remains deferred; counts are not progress measures.
