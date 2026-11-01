//! Source-ordered Minecraft Java 1.21 `LivingEntity.hurt` dispatch.
//!
//! This executor owns no health, death flag, entity identity or invulnerability
//! timer. Its host must operate on the actual actor and source. Every virtual
//! call and externally visible action is mandatory and fallible; an error keeps
//! the mutation/callback prefix already executed. It is not a damage formula or
//! a complete survival actor. `actually_hurt`, totem/death, equipment, active
//! effects, tracking, criteria and knockback (including its RNG) remain real
//! host dispatches, never successful default stubs.
//!
//! Evidence: official 1.21 `LivingEntity` (`btn.a(brk,float)`), Java source lines
//! 1133–1278; `DamageSource` (`brk`) and `DamageTypeTags` (`awg`) official mappings.
//! The original method's float operations, early returns and call order are
//! retained. Production Main wiring requires the integration coordinator.

/// The source tags consulted by this particular method, in source query order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DamageTag {
    IsFire,
    IsProjectile,
    IsFreezing,
    DamagesHelmet,
    BypassesCooldown,
    NoAnger,
    NoImpact,
    NoKnockback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActorTag {
    FreezeHurtsExtraTypes,
    NoAngerFromWindCharge,
}

/// Operations on the actual living actor and canonical damage source.
///
/// Implementations must preserve real identity in `Entity`; IDs must not be
/// resolved to replacement actors after removal/reuse. All state access is to
/// the actor's existing owner, especially physical `invulnerable_time` and
/// synchronized health read by `is_dead_or_dying`. The effect callback iterates
/// the actor's actual active effects in their native owner order.
pub trait LivingDamageHost {
    type Error;
    type Source;
    type Entity;

    fn is_invulnerable_to(&mut self, source: &Self::Source) -> Result<bool, Self::Error>;
    fn is_client_side(&mut self) -> Result<bool, Self::Error>;
    fn is_dead_or_dying(&mut self) -> Result<bool, Self::Error>;
    fn source_has_tag(
        &mut self,
        source: &Self::Source,
        tag: DamageTag,
    ) -> Result<bool, Self::Error>;
    fn has_fire_resistance(&mut self) -> Result<bool, Self::Error>;
    fn is_sleeping(&mut self) -> Result<bool, Self::Error>;
    fn stop_sleeping(&mut self) -> Result<(), Self::Error>;
    fn set_no_action_time(&mut self, value: i32) -> Result<(), Self::Error>;

    fn is_damage_source_blocked(&mut self, source: &Self::Source) -> Result<bool, Self::Error>;
    fn hurt_currently_used_shield(&mut self, amount: f32) -> Result<(), Self::Error>;
    fn source_direct_entity(
        &mut self,
        source: &Self::Source,
    ) -> Result<Option<Self::Entity>, Self::Error>;
    fn entity_is_living(&mut self, entity: &Self::Entity) -> Result<bool, Self::Error>;
    /// Actual virtual `blockUsingShield(attacker)` dispatch. The default Java
    /// implementation calls the attacker's `blockedByShield(this)`.
    fn block_using_shield(&mut self, attacker: &Self::Entity) -> Result<(), Self::Error>;
    fn actor_has_tag(&mut self, tag: ActorTag) -> Result<bool, Self::Error>;
    fn head_item_is_empty(&mut self) -> Result<bool, Self::Error>;
    fn hurt_helmet(&mut self, source: &Self::Source, amount: f32) -> Result<(), Self::Error>;
    fn set_walk_animation_speed(&mut self, speed: f32) -> Result<(), Self::Error>;

    fn invulnerable_time(&mut self) -> Result<i32, Self::Error>;
    fn set_invulnerable_time(&mut self, ticks: i32) -> Result<(), Self::Error>;
    fn last_hurt(&mut self) -> Result<f32, Self::Error>;
    fn set_last_hurt(&mut self, amount: f32) -> Result<(), Self::Error>;
    /// Actual virtual `actuallyHurt`, including the appropriate Player override,
    /// armor/effects/absorption/combat tracking/health/game-event/stat work.
    fn actually_hurt(&mut self, source: &Self::Source, amount: f32) -> Result<(), Self::Error>;
    fn set_hurt_duration(&mut self, ticks: i32) -> Result<(), Self::Error>;
    fn hurt_duration(&mut self) -> Result<i32, Self::Error>;
    fn set_hurt_time(&mut self, ticks: i32) -> Result<(), Self::Error>;

    fn source_causing_entity(
        &mut self,
        source: &Self::Source,
    ) -> Result<Option<Self::Entity>, Self::Error>;
    fn source_is_wind_charge(&mut self, source: &Self::Source) -> Result<bool, Self::Error>;
    /// Actual `setLastHurtByMob`, including its actor tick-count timestamp.
    fn set_last_hurt_by_mob(&mut self, attacker: &Self::Entity) -> Result<(), Self::Error>;
    fn entity_is_player(&mut self, entity: &Self::Entity) -> Result<bool, Self::Error>;
    fn entity_is_wolf(&mut self, entity: &Self::Entity) -> Result<bool, Self::Error>;
    fn wolf_is_tame(&mut self, wolf: &Self::Entity) -> Result<bool, Self::Error>;
    fn wolf_owner(&mut self, wolf: &Self::Entity) -> Result<Option<Self::Entity>, Self::Error>;
    fn set_last_hurt_by_player_time(&mut self, ticks: i32) -> Result<(), Self::Error>;
    fn set_last_hurt_by_player(&mut self, player: Option<&Self::Entity>)
        -> Result<(), Self::Error>;

    fn broadcast_entity_event(&mut self, event: u8) -> Result<(), Self::Error>;
    fn broadcast_damage_event(&mut self, source: &Self::Source) -> Result<(), Self::Error>;
    fn mark_hurt(&mut self) -> Result<(), Self::Error>;
    fn entity_is_projectile(&mut self, entity: &Self::Entity) -> Result<bool, Self::Error>;
    fn projectile_horizontal_hurt_direction(
        &mut self,
        projectile: &Self::Entity,
        source: &Self::Source,
    ) -> Result<(f64, f64), Self::Error>;
    fn source_position(&mut self, source: &Self::Source) -> Result<Option<[f64; 3]>, Self::Error>;
    /// Re-query the original source position at a non-null-required callsite.
    /// A disappearing/unavailable source position must return an error.
    fn source_position_required(&mut self, source: &Self::Source) -> Result<[f64; 3], Self::Error>;
    fn actor_x(&mut self) -> Result<f64, Self::Error>;
    fn actor_z(&mut self) -> Result<f64, Self::Error>;
    /// Actual virtual knockback, with actual resistance, movement and global
    /// Math.random owner (the tiny-direction retry loop does not use entity RNG).
    fn knockback(&mut self, strength: f64, x: f64, z: f64) -> Result<(), Self::Error>;
    fn indicate_damage(&mut self, x: f64, z: f64) -> Result<(), Self::Error>;

    /// Actual totem executor: it must check BYPASSES_INVULNERABILITY before
    /// scanning hands, then own consumption/stats/criteria/effects/event order.
    fn check_totem_death_protection(&mut self, source: &Self::Source) -> Result<bool, Self::Error>;
    /// Actual `makeSound(getDeathSound())`, including nullable sound behavior and
    /// the real actor/level sound RNG owners.
    fn play_death_sound(&mut self) -> Result<(), Self::Error>;
    fn die(&mut self, source: &Self::Source) -> Result<(), Self::Error>;
    fn play_hurt_sound(&mut self, source: &Self::Source) -> Result<(), Self::Error>;
    fn set_last_damage_source(&mut self, source: &Self::Source) -> Result<(), Self::Error>;
    fn level_game_time(&mut self) -> Result<i64, Self::Error>;
    fn set_last_damage_stamp(&mut self, game_time: i64) -> Result<(), Self::Error>;
    fn active_effects_on_mob_hurt(
        &mut self,
        source: &Self::Source,
        amount: f32,
    ) -> Result<(), Self::Error>;
    fn actor_is_server_player(&mut self) -> Result<bool, Self::Error>;
    fn entity_hurt_player_criterion(
        &mut self,
        source: &Self::Source,
        original: f32,
        adjusted: f32,
        blocked: bool,
    ) -> Result<(), Self::Error>;
    fn award_damage_blocked_by_shield(&mut self, tenths: i32) -> Result<(), Self::Error>;
    fn entity_is_server_player(&mut self, entity: &Self::Entity) -> Result<bool, Self::Error>;
    fn player_hurt_entity_criterion(
        &mut self,
        attacker: &Self::Entity,
        source: &Self::Source,
        original: f32,
        adjusted: f32,
        blocked: bool,
    ) -> Result<(), Self::Error>;
}

/// Execute `LivingEntity.hurt`; all health/death and world actions dispatch into
/// the supplied actual actor. A shielded hit can execute callbacks and return
/// false. A repeat hit can execute shield/equipment work before cooldown rejects
/// it. Neither case is safe to implement as a speculative all-or-nothing plan.
pub fn hurt<H: LivingDamageHost>(
    host: &mut H,
    source: &H::Source,
    mut amount: f32,
) -> Result<bool, H::Error> {
    if host.is_invulnerable_to(source)? {
        return Ok(false);
    }
    if host.is_client_side()? {
        return Ok(false);
    }
    if host.is_dead_or_dying()? {
        return Ok(false);
    }
    if host.source_has_tag(source, DamageTag::IsFire)? && host.has_fire_resistance()? {
        return Ok(false);
    }
    if host.is_sleeping()? && !host.is_client_side()? {
        host.stop_sleeping()?;
    }
    host.set_no_action_time(0)?;
    let original = amount;
    let mut blocked = false;
    let mut blocked_amount = 0.0;
    if amount > 0.0 && host.is_damage_source_blocked(source)? {
        host.hurt_currently_used_shield(amount)?;
        blocked_amount = amount;
        amount = 0.0;
        if !host.source_has_tag(source, DamageTag::IsProjectile)? {
            if let Some(direct) = host.source_direct_entity(source)? {
                if host.entity_is_living(&direct)? {
                    host.block_using_shield(&direct)?;
                }
            }
        }
        blocked = true;
    }
    if host.source_has_tag(source, DamageTag::IsFreezing)?
        && host.actor_has_tag(ActorTag::FreezeHurtsExtraTypes)?
    {
        amount *= 5.0;
    }
    if host.source_has_tag(source, DamageTag::DamagesHelmet)? && !host.head_item_is_empty()? {
        host.hurt_helmet(source, amount)?;
        amount *= 0.75;
    }
    host.set_walk_animation_speed(1.5)?;
    let mut full_hurt = true;
    if host.invulnerable_time()? as f32 > 10.0
        && !host.source_has_tag(source, DamageTag::BypassesCooldown)?
    {
        if amount <= host.last_hurt()? {
            return Ok(false);
        }
        let delta = amount - host.last_hurt()?;
        host.actually_hurt(source, delta)?;
        host.set_last_hurt(amount)?;
        full_hurt = false;
    } else {
        host.set_last_hurt(amount)?;
        host.set_invulnerable_time(20)?;
        host.actually_hurt(source, amount)?;
        host.set_hurt_duration(10)?;
        let duration = host.hurt_duration()?;
        host.set_hurt_time(duration)?;
    }
    let causing = host.source_causing_entity(source)?;
    if let Some(attacker) = causing.as_ref() {
        if host.entity_is_living(attacker)?
            && !host.source_has_tag(source, DamageTag::NoAnger)?
            && !(host.source_is_wind_charge(source)?
                && host.actor_has_tag(ActorTag::NoAngerFromWindCharge)?)
        {
            host.set_last_hurt_by_mob(attacker)?;
        }
        if host.entity_is_player(attacker)? {
            host.set_last_hurt_by_player_time(100)?;
            host.set_last_hurt_by_player(Some(attacker))?;
        } else if host.entity_is_wolf(attacker)? && host.wolf_is_tame(attacker)? {
            host.set_last_hurt_by_player_time(100)?;
            let owner = host.wolf_owner(attacker)?;
            if let Some(owner) = owner.as_ref() {
                if host.entity_is_player(owner)? {
                    host.set_last_hurt_by_player(Some(owner))?;
                } else {
                    host.set_last_hurt_by_player(None)?;
                }
            } else {
                host.set_last_hurt_by_player(None)?;
            }
        }
    }
    if full_hurt {
        if blocked {
            host.broadcast_entity_event(29)?;
        } else {
            host.broadcast_damage_event(source)?;
        }
        if !host.source_has_tag(source, DamageTag::NoImpact)? && (!blocked || amount > 0.0) {
            host.mark_hurt()?;
        }
        if !host.source_has_tag(source, DamageTag::NoKnockback)? {
            let mut x = 0.0;
            let mut z = 0.0;
            let direct = host.source_direct_entity(source)?;
            if let Some(projectile) = direct.as_ref() {
                if host.entity_is_projectile(projectile)? {
                    let pair = host.projectile_horizontal_hurt_direction(projectile, source)?;
                    x = -pair.0;
                    z = -pair.1;
                } else if host.source_position(source)?.is_some() {
                    // DamageSource.getSourcePosition is queried separately at
                    // each original callsite, rather than caching an actor pose.
                    x = host.source_position_required(source)?[0] - host.actor_x()?;
                    z = host.source_position_required(source)?[2] - host.actor_z()?;
                }
            } else if host.source_position(source)?.is_some() {
                x = host.source_position_required(source)?[0] - host.actor_x()?;
                z = host.source_position_required(source)?[2] - host.actor_z()?;
            }
            host.knockback(0.4000000059604645, x, z)?;
            if !blocked {
                host.indicate_damage(x, z)?;
            }
        }
    }
    if host.is_dead_or_dying()? {
        if !host.check_totem_death_protection(source)? {
            if full_hurt {
                host.play_death_sound()?;
            }
            host.die(source)?;
        }
    } else if full_hurt {
        host.play_hurt_sound(source)?;
    }
    let effective = !blocked || amount > 0.0;
    if effective {
        host.set_last_damage_source(source)?;
        let game_time = host.level_game_time()?;
        host.set_last_damage_stamp(game_time)?;
        host.active_effects_on_mob_hurt(source, amount)?;
    }
    if host.actor_is_server_player()? {
        host.entity_hurt_player_criterion(source, original, amount, blocked)?;
        if blocked_amount > 0.0 && blocked_amount < 3.4028235E37 {
            // Java Math.round(float), restricted here to a positive input.
            host.award_damage_blocked_by_shield((blocked_amount * 10.0).round() as i32)?;
        }
    }
    if let Some(attacker) = causing.as_ref() {
        if host.entity_is_server_player(attacker)? {
            host.player_hurt_entity_criterion(attacker, source, original, amount, blocked)?;
        }
    }
    Ok(effective)
}

#[cfg(test)]
mod tests {
    use super::*;

    // A callback recorder, not a production actor or a Java runtime oracle.
    #[derive(Default)]
    struct Source {
        tags: Vec<DamageTag>,
        direct: Option<i32>,
        causing: Option<i32>,
        position: Option<[f64; 3]>,
    }
    struct Recorder {
        health: f32,
        cooldown: i32,
        last: f32,
        duration: i32,
        shield: bool,
        server_player: bool,
        totem: bool,
        trace: Vec<String>,
        fail_at: Option<&'static str>,
    }
    impl Default for Recorder {
        fn default() -> Self {
            Self {
                health: 20.0,
                cooldown: 0,
                last: 0.0,
                duration: 0,
                shield: false,
                server_player: false,
                totem: false,
                trace: Vec::new(),
                fail_at: None,
            }
        }
    }
    impl Recorder {
        fn record(&mut self, value: impl Into<String>) -> Result<(), &'static str> {
            let value = value.into();
            let fail = self.fail_at.is_some_and(|at| value == at);
            self.trace.push(value);
            if fail {
                Err("injected callback failure")
            } else {
                Ok(())
            }
        }
    }
    impl LivingDamageHost for Recorder {
        type Error = &'static str;
        type Source = Source;
        type Entity = i32;
        fn is_invulnerable_to(&mut self, _: &Source) -> Result<bool, Self::Error> {
            Ok(false)
        }
        fn is_client_side(&mut self) -> Result<bool, Self::Error> {
            Ok(false)
        }
        fn is_dead_or_dying(&mut self) -> Result<bool, Self::Error> {
            Ok(self.health <= 0.0)
        }
        fn source_has_tag(&mut self, source: &Source, tag: DamageTag) -> Result<bool, Self::Error> {
            Ok(source.tags.contains(&tag))
        }
        fn has_fire_resistance(&mut self) -> Result<bool, Self::Error> {
            Ok(false)
        }
        fn is_sleeping(&mut self) -> Result<bool, Self::Error> {
            Ok(false)
        }
        fn stop_sleeping(&mut self) -> Result<(), Self::Error> {
            self.record("stop_sleeping")
        }
        fn set_no_action_time(&mut self, n: i32) -> Result<(), Self::Error> {
            self.record(format!("no_action={n}"))
        }
        fn is_damage_source_blocked(&mut self, _: &Source) -> Result<bool, Self::Error> {
            Ok(self.shield)
        }
        fn hurt_currently_used_shield(&mut self, n: f32) -> Result<(), Self::Error> {
            self.record(format!("shield={n}"))
        }
        fn source_direct_entity(&mut self, s: &Source) -> Result<Option<i32>, Self::Error> {
            Ok(s.direct)
        }
        fn entity_is_living(&mut self, e: &i32) -> Result<bool, Self::Error> {
            Ok(*e != 30)
        }
        fn block_using_shield(&mut self, e: &i32) -> Result<(), Self::Error> {
            self.record(format!("retaliate={e}"))
        }
        fn actor_has_tag(&mut self, _: ActorTag) -> Result<bool, Self::Error> {
            Ok(false)
        }
        fn head_item_is_empty(&mut self) -> Result<bool, Self::Error> {
            Ok(false)
        }
        fn hurt_helmet(&mut self, _: &Source, n: f32) -> Result<(), Self::Error> {
            self.record(format!("helmet={n}"))
        }
        fn set_walk_animation_speed(&mut self, n: f32) -> Result<(), Self::Error> {
            self.record(format!("walk={n}"))
        }
        fn invulnerable_time(&mut self) -> Result<i32, Self::Error> {
            Ok(self.cooldown)
        }
        fn set_invulnerable_time(&mut self, n: i32) -> Result<(), Self::Error> {
            self.cooldown = n;
            self.record(format!("cooldown={n}"))
        }
        fn last_hurt(&mut self) -> Result<f32, Self::Error> {
            Ok(self.last)
        }
        fn set_last_hurt(&mut self, n: f32) -> Result<(), Self::Error> {
            self.last = n;
            self.record(format!("last={n}"))
        }
        fn actually_hurt(&mut self, _: &Source, n: f32) -> Result<(), Self::Error> {
            self.health -= n;
            self.record(format!("actual={n}"))
        }
        fn set_hurt_duration(&mut self, n: i32) -> Result<(), Self::Error> {
            self.duration = n;
            self.record(format!("duration={n}"))
        }
        fn hurt_duration(&mut self) -> Result<i32, Self::Error> {
            Ok(self.duration)
        }
        fn set_hurt_time(&mut self, n: i32) -> Result<(), Self::Error> {
            self.record(format!("hurt_time={n}"))
        }
        fn source_causing_entity(&mut self, s: &Source) -> Result<Option<i32>, Self::Error> {
            Ok(s.causing)
        }
        fn source_is_wind_charge(&mut self, _: &Source) -> Result<bool, Self::Error> {
            Ok(false)
        }
        fn set_last_hurt_by_mob(&mut self, e: &i32) -> Result<(), Self::Error> {
            self.record(format!("mob={e}"))
        }
        fn entity_is_player(&mut self, e: &i32) -> Result<bool, Self::Error> {
            Ok(*e == 10)
        }
        fn entity_is_wolf(&mut self, e: &i32) -> Result<bool, Self::Error> {
            Ok(*e == 20)
        }
        fn wolf_is_tame(&mut self, _: &i32) -> Result<bool, Self::Error> {
            Ok(true)
        }
        fn wolf_owner(&mut self, _: &i32) -> Result<Option<i32>, Self::Error> {
            Ok(Some(10))
        }
        fn set_last_hurt_by_player_time(&mut self, n: i32) -> Result<(), Self::Error> {
            self.record(format!("player_time={n}"))
        }
        fn set_last_hurt_by_player(&mut self, e: Option<&i32>) -> Result<(), Self::Error> {
            self.record(format!("player={e:?}"))
        }
        fn broadcast_entity_event(&mut self, n: u8) -> Result<(), Self::Error> {
            self.record(format!("event={n}"))
        }
        fn broadcast_damage_event(&mut self, _: &Source) -> Result<(), Self::Error> {
            self.record("damage_event")
        }
        fn mark_hurt(&mut self) -> Result<(), Self::Error> {
            self.record("mark_hurt")
        }
        fn entity_is_projectile(&mut self, e: &i32) -> Result<bool, Self::Error> {
            Ok(*e == 30)
        }
        fn projectile_horizontal_hurt_direction(
            &mut self,
            _: &i32,
            _: &Source,
        ) -> Result<(f64, f64), Self::Error> {
            self.record("projectile_direction")?;
            Ok((2.0, 3.0))
        }
        fn source_position(&mut self, s: &Source) -> Result<Option<[f64; 3]>, Self::Error> {
            Ok(s.position)
        }
        fn source_position_required(&mut self, s: &Source) -> Result<[f64; 3], Self::Error> {
            s.position.ok_or("source position disappeared")
        }
        fn actor_x(&mut self) -> Result<f64, Self::Error> {
            Ok(1.0)
        }
        fn actor_z(&mut self) -> Result<f64, Self::Error> {
            Ok(1.0)
        }
        fn knockback(&mut self, n: f64, x: f64, z: f64) -> Result<(), Self::Error> {
            self.record(format!("knockback={n},{x},{z}"))
        }
        fn indicate_damage(&mut self, x: f64, z: f64) -> Result<(), Self::Error> {
            self.record(format!("indicate={x},{z}"))
        }
        fn check_totem_death_protection(&mut self, _: &Source) -> Result<bool, Self::Error> {
            self.record("totem")?;
            if self.totem {
                self.health = 1.0;
            }
            Ok(self.totem)
        }
        fn play_death_sound(&mut self) -> Result<(), Self::Error> {
            self.record("death_sound")
        }
        fn die(&mut self, _: &Source) -> Result<(), Self::Error> {
            self.record("die")
        }
        fn play_hurt_sound(&mut self, _: &Source) -> Result<(), Self::Error> {
            self.record("hurt_sound")
        }
        fn set_last_damage_source(&mut self, _: &Source) -> Result<(), Self::Error> {
            self.record("last_source")
        }
        fn level_game_time(&mut self) -> Result<i64, Self::Error> {
            Ok(234)
        }
        fn set_last_damage_stamp(&mut self, n: i64) -> Result<(), Self::Error> {
            self.record(format!("stamp={n}"))
        }
        fn active_effects_on_mob_hurt(&mut self, _: &Source, n: f32) -> Result<(), Self::Error> {
            self.record(format!("effects={n}"))
        }
        fn actor_is_server_player(&mut self) -> Result<bool, Self::Error> {
            Ok(self.server_player)
        }
        fn entity_hurt_player_criterion(
            &mut self,
            _: &Source,
            original: f32,
            adjusted: f32,
            blocked: bool,
        ) -> Result<(), Self::Error> {
            self.record(format!("hurt_criterion={original},{adjusted},{blocked}"))
        }
        fn award_damage_blocked_by_shield(&mut self, n: i32) -> Result<(), Self::Error> {
            self.record(format!("blocked_stat={n}"))
        }
        fn entity_is_server_player(&mut self, e: &i32) -> Result<bool, Self::Error> {
            Ok(*e == 10)
        }
        fn player_hurt_entity_criterion(
            &mut self,
            e: &i32,
            _: &Source,
            original: f32,
            adjusted: f32,
            blocked: bool,
        ) -> Result<(), Self::Error> {
            self.record(format!(
                "attack_criterion={e},{original},{adjusted},{blocked}"
            ))
        }
    }

    #[test]
    fn lethal_full_hit_runs_death_before_damage_effects_and_criteria() {
        let mut h = Recorder {
            health: 3.0,
            server_player: true,
            ..Recorder::default()
        };
        let s = Source {
            causing: Some(10),
            ..Source::default()
        };
        assert_eq!(hurt(&mut h, &s, 4.0), Ok(true));
        assert_eq!(h.health, -1.0);
        assert_eq!(
            h.trace,
            [
                "no_action=0",
                "walk=1.5",
                "last=4",
                "cooldown=20",
                "actual=4",
                "duration=10",
                "hurt_time=10",
                "mob=10",
                "player_time=100",
                "player=Some(10)",
                "damage_event",
                "mark_hurt",
                "knockback=0.4000000059604645,0,0",
                "indicate=0,0",
                "totem",
                "death_sound",
                "die",
                "last_source",
                "stamp=234",
                "effects=4",
                "hurt_criterion=4,4,false",
                "attack_criterion=10,4,4,false"
            ]
        );
    }
    #[test]
    fn blocked_repeat_executes_shield_and_helmet_before_cooldown_rejection() {
        let mut h = Recorder {
            cooldown: 20,
            last: 5.0,
            shield: true,
            ..Recorder::default()
        };
        let s = Source {
            direct: Some(10),
            tags: vec![DamageTag::DamagesHelmet],
            ..Source::default()
        };
        assert_eq!(hurt(&mut h, &s, 5.0), Ok(false));
        assert_eq!(
            h.trace,
            [
                "no_action=0",
                "shield=5",
                "retaliate=10",
                "helmet=0",
                "walk=1.5"
            ]
        );
        assert_eq!(h.health, 20.0);
    }
    #[test]
    fn delta_hit_runs_attribution_and_death_without_full_hit_effects() {
        let mut h = Recorder {
            health: 2.0,
            cooldown: 11,
            last: 5.0,
            ..Recorder::default()
        };
        let s = Source {
            causing: Some(20),
            tags: vec![DamageTag::NoAnger],
            ..Source::default()
        };
        assert_eq!(hurt(&mut h, &s, 8.0), Ok(true));
        assert_eq!(
            h.trace,
            [
                "no_action=0",
                "walk=1.5",
                "actual=3",
                "last=8",
                "player_time=100",
                "player=Some(10)",
                "totem",
                "die",
                "last_source",
                "stamp=234",
                "effects=8"
            ]
        );
    }
    #[test]
    fn full_block_returns_false_after_knockback_sound_and_player_criteria() {
        let mut h = Recorder {
            shield: true,
            server_player: true,
            ..Recorder::default()
        };
        let s = Source {
            direct: Some(30),
            causing: Some(10),
            tags: vec![DamageTag::IsProjectile],
            ..Source::default()
        };
        assert_eq!(hurt(&mut h, &s, 2.25), Ok(false));
        assert_eq!(
            h.trace,
            [
                "no_action=0",
                "shield=2.25",
                "walk=1.5",
                "last=0",
                "cooldown=20",
                "actual=0",
                "duration=10",
                "hurt_time=10",
                "mob=10",
                "player_time=100",
                "player=Some(10)",
                "event=29",
                "projectile_direction",
                "knockback=0.4000000059604645,-2,-3",
                "hurt_sound",
                "hurt_criterion=2.25,0,true",
                "blocked_stat=23",
                "attack_criterion=10,2.25,0,true"
            ]
        );
    }
    #[test]
    fn callback_failure_preserves_actual_mutation_prefix() {
        let mut h = Recorder {
            fail_at: Some("actual=4"),
            ..Recorder::default()
        };
        assert_eq!(
            hurt(&mut h, &Source::default(), 4.0),
            Err("injected callback failure")
        );
        assert_eq!(h.health, 16.0);
        assert_eq!(h.cooldown, 20);
        assert_eq!(h.last, 4.0);
        assert_eq!(
            h.trace,
            [
                "no_action=0",
                "walk=1.5",
                "last=4",
                "cooldown=20",
                "actual=4"
            ]
        );
    }
    #[test]
    fn totem_dispatch_owns_recovery_and_suppresses_death_callbacks() {
        let mut h = Recorder {
            health: 1.0,
            totem: true,
            ..Recorder::default()
        };
        assert_eq!(hurt(&mut h, &Source::default(), 2.0), Ok(true));
        assert_eq!(h.health, 1.0);
        assert!(h.trace.iter().any(|v| v == "totem"));
        assert!(!h
            .trace
            .iter()
            .any(|v| matches!(v.as_str(), "die" | "death_sound" | "hurt_sound")));
        assert!(h
            .trace
            .ends_with(&["last_source".into(), "stamp=234".into(), "effects=2".into()]));
    }
}
