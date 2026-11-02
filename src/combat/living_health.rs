//! Original Java 1.21 generic LivingEntity health callbacks.
//!
//! `btn.f(brk,float)` (1750–1773) is the generic actuallyHurt body, not the
//! Player override. Health, absorption, attributes, tracker and actor identity
//! remain in their canonical owners. In particular a Player's synchronized
//! health/absorption are fields 9/15; this module creates no parallel state.
//! Every callback is mandatory and synchronous. Errors retain the executed
//! mutation prefix. Real virtual dispatch and Main integration remain external.

pub trait LivingActuallyHurtHost {
    type Error;
    type Source: ?Sized;
    /// A retained canonical entity handle; never resolve a reused numeric ID.
    type Entity;

    fn is_invulnerable_to(&mut self, source: &Self::Source) -> Result<bool, Self::Error>;
    /// Actual virtual armor reduction, including equipment durability and
    /// enchantment execution at its source callsites, not only the formula.
    fn damage_after_armor_absorb(
        &mut self,
        source: &Self::Source,
        amount: f32,
    ) -> Result<f32, Self::Error>;
    /// Actual virtual magic reduction with current effects/equipment owners.
    fn damage_after_magic_absorb(
        &mut self,
        source: &Self::Source,
        amount: f32,
    ) -> Result<f32, Self::Error>;
    fn absorption_amount(&mut self) -> Result<f32, Self::Error>;
    /// Actual final setAbsorptionAmount, including its current attribute clamp
    /// and virtual internal setter. No local absorption mirror is committed.
    fn set_absorption_amount(&mut self, amount: f32) -> Result<(), Self::Error>;
    /// DamageSource.getEntity: the causing entity, never the direct entity.
    fn source_causing_entity(
        &mut self,
        source: &Self::Source,
    ) -> Result<Option<Self::Entity>, Self::Error>;
    fn entity_is_server_player(&mut self, entity: &Self::Entity) -> Result<bool, Self::Error>;
    /// Causing ServerPlayer.awardStat(DAMAGE_DEALT_ABSORBED, amount).
    fn award_damage_dealt_absorbed(
        &mut self,
        player: &Self::Entity,
        amount: i32,
    ) -> Result<(), Self::Error>;
    fn combat_tracker_record_damage(
        &mut self,
        source: &Self::Source,
        amount: f32,
    ) -> Result<(), Self::Error>;
    fn health(&mut self) -> Result<f32, Self::Error>;
    /// Actual virtual setHealth with the current maximum-health attribute and
    /// synchronized-data dirtiness. Private health delivery is a later phase.
    fn set_health(&mut self, amount: f32) -> Result<(), Self::Error>;
    fn entity_damage_game_event(&mut self) -> Result<(), Self::Error>;
}

/// The generic body selected by LivingEntity.actuallyHurt. Player's override
/// must select player_damage::actually_hurt instead; it has different stats,
/// food exhaustion and absorption behavior.
pub fn actually_hurt<H: LivingActuallyHurtHost + ?Sized>(
    host: &mut H,
    source: &H::Source,
    amount: f32,
) -> Result<(), H::Error> {
    if host.is_invulnerable_to(source)? {
        return Ok(());
    }
    let amount = host.damage_after_armor_absorb(source, amount)?;
    let reduced = host.damage_after_magic_absorb(source, amount)?;
    let amount = java_max(reduced - host.absorption_amount()?, 0.);
    let absorption = host.absorption_amount()?;
    host.set_absorption_amount(absorption - (reduced - amount))?;
    let absorbed = reduced - amount;
    if absorbed > 0. && absorbed < 3.4028235e37_f32 {
        if let Some(causer) = host.source_causing_entity(source)? {
            if host.entity_is_server_player(&causer)? {
                host.award_damage_dealt_absorbed(&causer, java_round(absorbed * 10.))?;
            }
        }
    }
    // JVM fcmpl/ifne forwards NaN and rejects both signed zeros.
    if amount == 0. {
        return Ok(());
    }
    host.combat_tracker_record_damage(source, amount)?;
    let health = host.health()?;
    host.set_health(health - amount)?;
    // This second absorption subtraction is present in the generic body only.
    // Both this read and the preceding health read follow real callbacks.
    let absorption = host.absorption_amount()?;
    host.set_absorption_amount(absorption - amount)?;
    host.entity_damage_game_event()
}

pub trait LivingHealHost {
    type Error;
    fn health(&mut self) -> Result<f32, Self::Error>;
    /// Actual virtual setHealth: resolve current max health and publish through
    /// the real synchronized-data owner. Do not cache a second health field.
    fn set_health(&mut self, amount: f32) -> Result<(), Self::Error>;
}

/// LivingEntity.heal, source 1113–1117. The amount itself has no positivity
/// guard; negative, infinite and NaN amounts still reach setHealth when alive.
pub fn heal<H: LivingHealHost + ?Sized>(host: &mut H, amount: f32) -> Result<(), H::Error> {
    let health = host.health()?;
    if health > 0. {
        host.set_health(health + amount)?;
    }
    Ok(())
}

pub trait LivingKillCreditHost {
    type Error;
    type Entity;
    /// Canonical LivingEntity.lastHurtByPlayer field, not an attacker inferred
    /// from the final source or damage history. Each call is a fresh field read.
    fn last_hurt_by_player(&mut self) -> Result<Option<Self::Entity>, Self::Error>;
    fn last_hurt_by_mob(&mut self) -> Result<Option<Self::Entity>, Self::Error>;
}

/// LivingEntity.getKillCredit, source 1781–1787. Expiration/aliveness belongs
/// to baseTick; the getter adds no time or alive check and retains field rereads.
pub fn kill_credit<H: LivingKillCreditHost + ?Sized>(
    host: &mut H,
) -> Result<Option<H::Entity>, H::Error> {
    if host.last_hurt_by_player()?.is_some() {
        return host.last_hurt_by_player();
    }
    if host.last_hurt_by_mob()?.is_some() {
        return host.last_hurt_by_mob();
    }
    Ok(None)
}

fn java_max(a: f32, b: f32) -> f32 {
    if a.is_nan() {
        a
    } else if a == 0. && b == 0. && a.is_sign_negative() {
        b
    } else if a >= b {
        a
    } else {
        b
    }
}

fn java_round(value: f32) -> i32 {
    (f64::from(value) + 0.5).floor() as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::VecDeque, rc::Rc};

    struct Host {
        trace: Vec<String>,
        source: Rc<u32>,
        causer: Rc<u32>,
        health: f32,
        absorption: f32,
        reduced: f32,
        mutate: bool,
        invulnerable: bool,
        fail: Option<&'static str>,
        players: VecDeque<Option<Rc<u32>>>,
        mobs: VecDeque<Option<Rc<u32>>>,
    }
    impl Host {
        fn new() -> Self {
            Self {
                trace: vec![],
                source: Rc::new(767),
                causer: Rc::new(1),
                health: 20.,
                absorption: 3.,
                reduced: 7.,
                mutate: false,
                invulnerable: false,
                fail: None,
                players: VecDeque::new(),
                mobs: VecDeque::new(),
            }
        }
        fn call(&mut self, name: &str) -> Result<(), &'static str> {
            self.trace.push(name.into());
            if self.fail == Some(name) {
                Err("owner failure")
            } else {
                Ok(())
            }
        }
        fn source(&self, source: &Rc<u32>) {
            assert!(Rc::ptr_eq(source, &self.source));
        }
    }
    impl LivingActuallyHurtHost for Host {
        type Error = &'static str;
        type Source = Rc<u32>;
        type Entity = Rc<u32>;
        fn is_invulnerable_to(&mut self, s: &Self::Source) -> Result<bool, Self::Error> {
            self.source(s);
            self.call("invulnerable")?;
            Ok(self.invulnerable)
        }
        fn damage_after_armor_absorb(
            &mut self,
            s: &Self::Source,
            a: f32,
        ) -> Result<f32, Self::Error> {
            self.source(s);
            self.call("armor")?;
            assert_eq!(a, 10.);
            Ok(8.)
        }
        fn damage_after_magic_absorb(
            &mut self,
            s: &Self::Source,
            a: f32,
        ) -> Result<f32, Self::Error> {
            self.source(s);
            self.call("magic")?;
            assert_eq!(a, 8.);
            Ok(self.reduced)
        }
        fn absorption_amount(&mut self) -> Result<f32, Self::Error> {
            self.call("absorption")?;
            Ok(self.absorption)
        }
        fn set_absorption_amount(&mut self, a: f32) -> Result<(), Self::Error> {
            self.call(&format!("set_absorption={a}"))?;
            self.absorption = a;
            Ok(())
        }
        fn source_causing_entity(
            &mut self,
            s: &Self::Source,
        ) -> Result<Option<Self::Entity>, Self::Error> {
            self.source(s);
            self.call("causer")?;
            Ok(Some(self.causer.clone()))
        }
        fn entity_is_server_player(&mut self, e: &Self::Entity) -> Result<bool, Self::Error> {
            assert!(Rc::ptr_eq(e, &self.causer));
            self.call("server_player")?;
            Ok(true)
        }
        fn award_damage_dealt_absorbed(
            &mut self,
            e: &Self::Entity,
            a: i32,
        ) -> Result<(), Self::Error> {
            assert!(Rc::ptr_eq(e, &self.causer));
            self.call(&format!("dealt_absorbed={a}"))
        }
        fn combat_tracker_record_damage(
            &mut self,
            s: &Self::Source,
            a: f32,
        ) -> Result<(), Self::Error> {
            self.source(s);
            self.call(&format!("record={a}"))?;
            if self.mutate {
                self.health = 30.;
                self.absorption = 10.;
            }
            Ok(())
        }
        fn health(&mut self) -> Result<f32, Self::Error> {
            self.call("health")?;
            Ok(self.health)
        }
        fn set_health(&mut self, a: f32) -> Result<(), Self::Error> {
            self.call(&format!("set_health={a}"))?;
            self.health = a;
            if self.mutate {
                self.absorption = 12.;
            }
            Ok(())
        }
        fn entity_damage_game_event(&mut self) -> Result<(), Self::Error> {
            self.call("event")
        }
    }
    impl LivingHealHost for Host {
        type Error = &'static str;
        fn health(&mut self) -> Result<f32, Self::Error> {
            LivingActuallyHurtHost::health(self)
        }
        fn set_health(&mut self, a: f32) -> Result<(), Self::Error> {
            LivingActuallyHurtHost::set_health(self, a)
        }
    }
    impl LivingKillCreditHost for Host {
        type Error = &'static str;
        type Entity = Rc<u32>;
        fn last_hurt_by_player(&mut self) -> Result<Option<Self::Entity>, Self::Error> {
            self.call("player_field")?;
            Ok(self.players.pop_front().unwrap_or(None))
        }
        fn last_hurt_by_mob(&mut self) -> Result<Option<Self::Entity>, Self::Error> {
            self.call("mob_field")?;
            Ok(self.mobs.pop_front().unwrap_or(None))
        }
    }

    #[test]
    fn generic_damage_retains_identity_and_rereads_after_callbacks() {
        let mut host = Host::new();
        host.mutate = true;
        let source = host.source.clone();
        actually_hurt(&mut host, &source, 10.).unwrap();
        assert_eq!(
            host.trace,
            [
                "invulnerable",
                "armor",
                "magic",
                "absorption",
                "absorption",
                "set_absorption=0",
                "causer",
                "server_player",
                "dealt_absorbed=30",
                "record=4",
                "health",
                "set_health=26",
                "absorption",
                "set_absorption=8",
                "event"
            ]
        );
        assert_eq!((host.health, host.absorption), (26., 8.));
    }

    #[test]
    fn failures_keep_only_the_completed_prefix() {
        let mut host = Host::new();
        let source = host.source.clone();
        host.fail = Some("dealt_absorbed=30");
        assert_eq!(actually_hurt(&mut host, &source, 10.), Err("owner failure"));
        assert_eq!((host.health, host.absorption), (20., 0.));
        assert_eq!(host.trace.last().unwrap(), "dealt_absorbed=30");
        let mut host = Host::new();
        host.fail = Some("event");
        let source = host.source.clone();
        assert_eq!(actually_hurt(&mut host, &source, 10.), Err("owner failure"));
        assert_eq!((host.health, host.absorption), (16., -4.));
        // The mock records raw setter arguments. The real setter must clamp.
        assert_eq!(host.trace.last().unwrap(), "event");
    }

    #[test]
    fn absorbed_zero_invulnerable_and_nan_branches() {
        let mut host = Host::new();
        host.reduced = 2.;
        let source = host.source.clone();
        actually_hurt(&mut host, &source, 10.).unwrap();
        assert_eq!(host.health, 20.);
        assert_eq!(host.absorption, 1.);
        assert_eq!(host.trace.last().unwrap(), "dealt_absorbed=20");
        host.trace.clear();
        host.invulnerable = true;
        actually_hurt(&mut host, &source, 10.).unwrap();
        assert_eq!(host.trace, ["invulnerable"]);
        host.trace.clear();
        host.invulnerable = false;
        host.reduced = f32::NAN;
        actually_hurt(&mut host, &source, 10.).unwrap();
        assert!(host.health.is_nan() && host.absorption.is_nan());
        assert!(!host.trace.iter().any(|s| s == "causer"));
        assert_eq!(host.trace.last().unwrap(), "event");
        assert_eq!(java_max(-0., 0.).to_bits(), 0_f32.to_bits());
        let payload = f32::from_bits(0x7fc01234);
        assert_eq!(java_max(payload, 0.).to_bits(), payload.to_bits());
        assert_eq!(java_round(f32::NAN), 0);
        assert_eq!(java_round(f32::INFINITY), i32::MAX);
        assert_eq!(java_round(-1.5), -1);
    }

    #[test]
    fn healing_and_canonical_kill_credit_gates() {
        let mut host = Host::new();
        heal(&mut host, -5.).unwrap();
        assert_eq!(host.health, 15.);
        for h in [0., -0., f32::NAN] {
            host.health = h;
            host.trace.clear();
            heal(&mut host, 5.).unwrap();
            assert_eq!(host.trace, ["health"]);
        }
        let player = Rc::new(1);
        let mob = Rc::new(2);
        host.players = [Some(player.clone()), Some(player.clone())].into();
        host.mobs = [Some(mob.clone())].into();
        host.trace.clear();
        assert!(Rc::ptr_eq(
            &kill_credit(&mut host).unwrap().unwrap(),
            &player
        ));
        assert_eq!(host.trace, ["player_field", "player_field"]);
        host.players = [None].into();
        host.mobs = [Some(mob.clone()), Some(mob.clone())].into();
        host.trace.clear();
        assert!(Rc::ptr_eq(&kill_credit(&mut host).unwrap().unwrap(), &mob));
        assert_eq!(host.trace, ["player_field", "mob_field", "mob_field"]);
    }
}
