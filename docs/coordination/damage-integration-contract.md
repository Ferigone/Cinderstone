# Player health and damage integration contract

This handoff is for the entities/survival team. It describes the actual current
owners and the remaining production integration boundary. It is not a claim of
completed player damage, death, respawn or full vanilla compatibility.

## Checklist boundary

`survival_combat.2` is the existing delivery **health and damage formulas**.
Its callable arithmetic exists in `src/survival.rs`; its Main integration remains
incomplete. `survival_combat.3` separately owns the complete damage/death/respawn
lifecycle. Reusing healing in a real food callback does not finish either row.
Keep the fixed checklist definition and `scope_version: 1`; do not count a
diagnostic call, unused adapter or creative immunity return as damage integration.

## Existing implementation and owners

| Area | Existing code and actual owner | Integration boundary |
| --- | --- | --- |
| Health clamp and healing | `survival::HealthState::{set_health, heal}`; Main food healing now calls `HealthState::heal` | The real synchronized health remains registry metadata field 9; the numeric receiver does not create another persistent health owner |
| Absorption subtraction | `HealthState::{set_absorption, subtract_damage}` | Actual Player absorption is metadata field 15; damage must read its actual current value and attribute maximum |
| Armor | `armor_effectiveness`, `damage_after_armor`, `damage_after_armor_with_modifier` | Requires actual damage-source tags, armor/toughness attributes, equipment durability and applicable weapon-enchantment effects |
| Magic and resistance | `damage_after_magic`, `damage_after_resistance` | Requires the real active-effect and equipment-enchantment evaluators; bypass tags decide whether each stage is called |
| Difficulty scaling | `difficulty_scaled_damage` | Requires the actual source scaling mode, causing entity and current world difficulty |
| Falling | `calculate_fall_damage`, `calculate_player_fall_damage` | Requires actual fall distance, block callback multiplier, player attributes, may-fly ability and source/gamerule immunity |
| Food and health delivery | `src/main_player_food.rs`, `src/player_food.rs` | Food state belongs to `PlayerLifecycle`; private health packets are sent at the existing later health-sync phase |
| Persistence | `src/player_nbt.rs` | `Health` and `AbsorptionAmount` come from metadata fields 9 and 15; preserve that lineage instead of adding independent saved copies |

The current food callback already verifies player identity, actual supported
native lifecycle, absence of unowned active effects and the supported current
maximum-health attribute/equipment context. The healing reuse preserves those
checks. Only field 9 is committed; absorption, food state and NBT ownership are
unchanged. Healing still obeys the positive-current-health gate and cannot revive
a dead player.

The ordinary body profile is currently an alive, unmounted creative player.
`player_body::tick` supplies a false hurt result for ordinary base-tick fire/lava
under the established creative-invulnerability profile. The starvation callback
also returns false for creative invulnerability and rejects an unsupported real
hurt/death path. These are not positive player-damage integration.

`PlayerMoveWorld::on_below_world` currently explicitly rejects the missing
LivingEntity damage/death owner. Replacing that error with a direct health
subtraction would skip required behavior and would not complete the delivery.

## Pinned Java evidence and ordering

Use the same pinned Java 1.21 server and official mappings recorded in
[`../reference/furnace-menu-lifetime.md`](../reference/furnace-menu-lifetime.md).
The relevant mapped classes are `Player` (`cmx`), `LivingEntity` (`btn`),
`ServerPlayer` (`aqv`), `CombatTracker` (`brh`) and `DamageTypeTags` (`awg`).
The following facts were read from the actual pinned JAR with JDK 21 `javap` and
its data resources, rather than inferred from the Rust formulas:

- `LivingEntity.onBelowWorld` invokes its virtual `hurt` with the actual
  out-of-world source and amount `4.0f`.
- `data/minecraft/damage_type/out_of_world.json` specifies exhaustion `0.0`
  and scaling `when_caused_by_living_non_player`. This source has no causing
  entity in the below-world callback.
- The pinned out-of-world tags include `bypasses_invulnerability`,
  `bypasses_armor`, `bypasses_resistance`, `no_knockback` and
  `always_most_significant_fall`. These facts do not justify assuming that all
  source tags or every later callback are bypassed.
- `Player.hurt` applies actual invulnerability/ability gates, checks its live
  state, performs the source-dependent difficulty scaling and enters
  `LivingEntity.hurt`. The creative gate is bypassed only by the real source tag.
- `LivingEntity.hurt` owns shielding, source-specific modifiers, the last-hurt
  comparison and invulnerability window, hurt durations, attacker attribution,
  damage-event publication, impact/knockback conditions, alive/death dispatch,
  hurt sound, last-damage source/time and criteria/effect callbacks.
- The positive `Player.actuallyHurt` path applies armor and magic reductions,
  updates absorption, awards absorbed-damage statistics when applicable, causes
  source exhaustion, records damage in `CombatTracker`, changes synchronized
  health, awards damage-taken statistics and emits the entity-damage game event.
  Preserve this source order; a health-only setter is insufficient.
- `CombatTracker.recordDamage` first performs its own status recheck, then
  stores the actual source, amount, fall context and fall distance, and records
  the entity tick time. Its combat-entry behavior depends on a living causing
  entity and actual alive state.

Java binaries and raw JVM output remain private analysis inputs and must not be
committed. Capture additional facts for equipment effects, sounds, packet fields
and tick ordering before implementing those specific branches.

## Required production wiring

1. Keep synchronized health and absorption in their existing entity-data owner.
   A runtime damage owner may hold UUID-bound cooldown, hurt-animation,
   last-source/time and combat-tracker state, but must not silently duplicate
   health or fabricate an empty equipment/effect context.
2. Introduce actual typed damage-source facts from the configured 1.21 registry
   and tags, including the causing/direct entity and position where the source
   supplies them. Preserve source scaling and gamerule predicates.
3. Resolve the supported armor/toughness, maximum-health/absorption, fall and
   effect attributes from actual equipment and lifecycle ownership. Equipment
   damage, shield use, enchantment effects and resistance must execute at their
   original call sites or fail before partially committing a hit.
4. Port the bounded real `ServerPlayer.hurt` → `Player.hurt` →
   `LivingEntity.hurt` → `Player.actuallyHurt` path, including numerical
   reductions, absorption, statistics, exhaustion, combat tracking, game events,
   packets, sound and RNG consumption. Reuse the callable formulas at these
   actual stages, not in an unrelated command or dummy calculation.
5. Invoke it from a real existing producer such as accepted fall handling,
   starvation, fire/lava or below-world processing only after that producer's
   player profile and surrounding lifecycle are genuinely supported. A new
   arbitrary health-changing command is not a substitute for this wiring.
6. Advance cooldown and hurt/combat state in the actual player/entity phase,
   with the source's clock and ordering. Existing
   `EntityPhysicalState::invulnerable_time` is an owned field, not permission to
   invent a second timer or omit its real decrement.
7. Commit mutations and publish effects under the existing world/player lock
   discipline. Keep ordinary metadata delivery and the later private health
   packet phase distinct. Preserve native checkpoint and Java NBT round trips.

## Acceptance and explicit blockers

A reviewable integration should demonstrate an actual connected player's health
changing through a genuine production damage producer and the existing numeric
formula stages, with the matching absorption, exhaustion/statistics, combat and
network effects. It must also preserve creative rejection for ordinary damage
and use actual source tags for exceptions. Source stages skipped by a bypass
must be skipped deliberately; that bypass alone does not demonstrate integration
of the armor, resistance or protection formulas.

After implementation, use a focused runtime flow that records the actual
producer, source, attribute/equipment/effect context, health/absorption before and
after, observable packets/effects and saved/restored values. A separate focused
healing observation should retain existing food/NBT/metadata ownership. Broad
conformance suites remain deferred; test or agent counts are not progress.

Current blockers include the actual player damage executor, equipment/effect
evaluation, source tags and source-specific callbacks, living hurt/cooldown and
combat-tracker state, and damage-side statistics/events/packets/sound ownership.
The current creative-only body profile also blocks treating a survival fall or
starvation producer as already available.

Full death, totem handling, death drops and XP, score/death messages, entity
removal, respawn and every damage context remain the separate unfinished
lifecycle delivery. Do not clamp a lethal hit to one health, silently ignore
death or claim the whole damage delivery from a nonlethal-only void adapter.
Until a coherent supported damage path exists, keep `survival_combat.2` Main
integration incomplete even though its numerical core and real healing reuse
are present.
