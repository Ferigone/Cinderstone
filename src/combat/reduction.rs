//! LivingEntity's armor and magic absorption phases from Java 1.21.
//!
//! The existing survival module supplies the numerical formulas. This module
//! retains their actual source-tag, equipment, effect, enchantment and statistic
//! call boundaries. A numeric result does not establish production integration.

use crate::survival;

/// Resolve these against the actual damage type holder's tag membership.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DamageReductionTag {
    BypassesArmor,
    BypassesEffects,
    BypassesResistance,
    BypassesEnchantments,
}

/// Synchronous operations on the actual victim and damage source.
///
/// No callback has an empty or numerical default. In particular, hurt_armor
/// must run the actual equipment durability/break/enchantment path. A Weapon
/// is an opaque handle to the source's actual direct-entity weapon stack; it
/// must not be substituted with the causing entity's currently held stack.
/// Definition, entity, equipment, effect and RNG ownership remain with the
/// host. An unavailable owner must produce an error before that callback is
/// treated as executed. Earlier mutations are retained if a later call fails.
pub trait DamageReductionHost<S> {
    type Error;
    type Weapon;

    fn source_is(&mut self, source: &S, tag: DamageReductionTag) -> Result<bool, Self::Error>;

    fn hurt_armor(&mut self, source: &S, damage: f32) -> Result<(), Self::Error>;

    /// LivingEntity.getArmorValue: floor the actual ARMOR attribute double.
    fn armor_value(&mut self) -> Result<i32, Self::Error>;

    fn armor_toughness_attribute(&mut self) -> Result<f64, Self::Error>;

    fn source_weapon_item(&mut self, source: &S) -> Result<Option<Self::Weapon>, Self::Error>;

    fn is_server_level(&mut self) -> Result<bool, Self::Error>;

    /// EnchantmentHelper.modifyArmorEffectiveness, in the actual level with
    /// this victim, damage source and retained direct-entity weapon stack.
    fn modify_armor_effectiveness(
        &mut self,
        source: &S,
        weapon: &Self::Weapon,
        effectiveness: f32,
    ) -> Result<f32, Self::Error>;

    fn has_resistance_effect(&mut self) -> Result<bool, Self::Error>;

    /// Read the amplifier from the actual active RESISTANCE instance.
    fn resistance_amplifier(&mut self) -> Result<i32, Self::Error>;

    fn victim_is_server_player(&mut self) -> Result<bool, Self::Error>;

    fn causing_entity_is_server_player(&mut self, source: &S) -> Result<bool, Self::Error>;

    fn award_damage_resisted(&mut self, amount: i32) -> Result<(), Self::Error>;

    fn award_damage_dealt_resisted(&mut self, source: &S, amount: i32) -> Result<(), Self::Error>;

    /// EnchantmentHelper.getDamageProtection, including the actual equipped
    /// stacks, slot checks, enchantment definitions and their source context.
    fn enchantment_damage_protection(&mut self, source: &S) -> Result<f32, Self::Error>;
}

/// LivingEntity.getDamageAfterArmorAbsorb, source lines 1699–1703.
pub fn get_damage_after_armor_absorb<S, H: DamageReductionHost<S>>(
    source: &S,
    damage: f32,
    host: &mut H,
) -> Result<f32, H::Error> {
    if host.source_is(source, DamageReductionTag::BypassesArmor)? {
        return Ok(damage);
    }
    host.hurt_armor(source, damage)?;
    let armor = host.armor_value()? as f32;
    let toughness = host.armor_toughness_attribute()? as f32;
    let weapon = host.source_weapon_item(source)?;
    if let Some(weapon) = weapon {
        if host.is_server_level()? {
            return survival::try_damage_after_armor_with_modifier(
                damage,
                armor,
                toughness,
                |effectiveness| host.modify_armor_effectiveness(source, &weapon, effectiveness),
            );
        }
    }
    // The no-weapon/non-ServerLevel source branch has no additional 0..1 clamp.
    Ok(survival::damage_after_armor(damage, armor, toughness))
}

const STATISTIC_DAMAGE_LIMIT: f32 = 3.4028235e37;

/// Java Math.round(float): preserve the exact f32 product, then add one half
/// in f64 so an integer such as 8_388_609 is not first rounded upward in f32.
/// The statistic guards ensure a finite positive input; the cast also matches
/// Java's saturation once the result exceeds i32::MAX.
fn round_statistic(value: f32) -> i32 {
    (f64::from(value) + 0.5).floor() as i32
}

/// LivingEntity.getDamageAfterMagicAbsorb, source lines 1707–1746.
pub fn get_damage_after_magic_absorb<S, H: DamageReductionHost<S>>(
    source: &S,
    mut damage: f32,
    host: &mut H,
) -> Result<f32, H::Error> {
    if host.source_is(source, DamageReductionTag::BypassesEffects)? {
        // This precedes the positive-damage guard and preserves signed zero.
        return Ok(damage);
    }
    if host.has_resistance_effect()?
        && !host.source_is(source, DamageReductionTag::BypassesResistance)?
    {
        let amplifier = host.resistance_amplifier()?;
        let previous_damage = damage;
        damage = survival::damage_after_resistance(damage, amplifier);
        let resisted = previous_damage - damage;
        if resisted > 0.0 && resisted < STATISTIC_DAMAGE_LIMIT {
            if host.victim_is_server_player()? {
                host.award_damage_resisted(round_statistic(resisted * 10.0))?;
            } else if host.causing_entity_is_server_player(source)? {
                host.award_damage_dealt_resisted(source, round_statistic(resisted * 10.0))?;
            }
        }
    }
    if damage <= 0.0 {
        return Ok(0.0);
    }
    if host.source_is(source, DamageReductionTag::BypassesEnchantments)? {
        return Ok(damage);
    }
    let protection = if host.is_server_level()? {
        host.enchantment_damage_protection(source)?
    } else {
        0.0
    };
    if protection > 0.0 {
        damage = survival::damage_after_magic(damage, protection);
    }
    Ok(damage)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    enum Call {
        Tag(DamageReductionTag),
        HurtArmor(u32),
        Armor,
        Toughness,
        Weapon,
        ServerLevel,
        Modify(u32),
        HasResistance,
        ResistanceAmplifier,
        VictimPlayer,
        CausingPlayer,
        Resisted(i32),
        DealtResisted(i32),
        Protection,
    }

    struct Host {
        calls: Vec<Call>,
        bypass: Option<DamageReductionTag>,
        resistance: bool,
        victim_player: bool,
        causing_player: bool,
        weapon: bool,
        server_level: bool,
        modifier_failure: bool,
    }

    impl Host {
        fn new() -> Self {
            Self {
                calls: Vec::new(),
                bypass: None,
                resistance: false,
                victim_player: false,
                causing_player: false,
                weapon: false,
                server_level: true,
                modifier_failure: false,
            }
        }
    }

    impl DamageReductionHost<()> for Host {
        type Error = &'static str;
        type Weapon = u32;

        fn source_is(&mut self, _: &(), tag: DamageReductionTag) -> Result<bool, Self::Error> {
            self.calls.push(Call::Tag(tag));
            Ok(self.bypass == Some(tag))
        }
        fn hurt_armor(&mut self, _: &(), damage: f32) -> Result<(), Self::Error> {
            self.calls.push(Call::HurtArmor(damage.to_bits()));
            Ok(())
        }
        fn armor_value(&mut self) -> Result<i32, Self::Error> {
            self.calls.push(Call::Armor);
            Ok(10)
        }
        fn armor_toughness_attribute(&mut self) -> Result<f64, Self::Error> {
            self.calls.push(Call::Toughness);
            Ok(2.0)
        }
        fn source_weapon_item(&mut self, _: &()) -> Result<Option<Self::Weapon>, Self::Error> {
            self.calls.push(Call::Weapon);
            Ok(self.weapon.then_some(42))
        }
        fn is_server_level(&mut self) -> Result<bool, Self::Error> {
            self.calls.push(Call::ServerLevel);
            Ok(self.server_level)
        }
        fn modify_armor_effectiveness(
            &mut self,
            _: &(),
            weapon: &Self::Weapon,
            effectiveness: f32,
        ) -> Result<f32, Self::Error> {
            assert_eq!(*weapon, 42);
            self.calls.push(Call::Modify(effectiveness.to_bits()));
            if self.modifier_failure {
                Err("actual enchantment executor unavailable")
            } else {
                Ok(2.0)
            }
        }
        fn has_resistance_effect(&mut self) -> Result<bool, Self::Error> {
            self.calls.push(Call::HasResistance);
            Ok(self.resistance)
        }
        fn resistance_amplifier(&mut self) -> Result<i32, Self::Error> {
            self.calls.push(Call::ResistanceAmplifier);
            Ok(0)
        }
        fn victim_is_server_player(&mut self) -> Result<bool, Self::Error> {
            self.calls.push(Call::VictimPlayer);
            Ok(self.victim_player)
        }
        fn causing_entity_is_server_player(&mut self, _: &()) -> Result<bool, Self::Error> {
            self.calls.push(Call::CausingPlayer);
            Ok(self.causing_player)
        }
        fn award_damage_resisted(&mut self, amount: i32) -> Result<(), Self::Error> {
            self.calls.push(Call::Resisted(amount));
            Ok(())
        }
        fn award_damage_dealt_resisted(&mut self, _: &(), amount: i32) -> Result<(), Self::Error> {
            self.calls.push(Call::DealtResisted(amount));
            Ok(())
        }
        fn enchantment_damage_protection(&mut self, _: &()) -> Result<f32, Self::Error> {
            self.calls.push(Call::Protection);
            Ok(5.0)
        }
    }

    #[test]
    fn armor_executes_real_callbacks_in_source_order_and_stops_on_error() {
        let mut host = Host::new();
        host.weapon = true;
        host.modifier_failure = true;
        assert_eq!(
            get_damage_after_armor_absorb(&(), 10.0, &mut host),
            Err("actual enchantment executor unavailable")
        );
        assert_eq!(
            host.calls,
            vec![
                Call::Tag(DamageReductionTag::BypassesArmor),
                Call::HurtArmor(10.0_f32.to_bits()),
                Call::Armor,
                Call::Toughness,
                Call::Weapon,
                Call::ServerLevel,
                Call::Modify(survival::armor_effectiveness(10.0, 10.0, 2.0).to_bits()),
            ]
        );
        host.modifier_failure = false;
        assert_eq!(
            get_damage_after_armor_absorb(&(), 10.0, &mut host).unwrap(),
            0.0
        );
    }

    #[test]
    fn bypasses_do_not_execute_omitted_callbacks_or_erase_signed_zero() {
        let mut host = Host::new();
        host.bypass = Some(DamageReductionTag::BypassesArmor);
        assert_eq!(
            get_damage_after_armor_absorb(&(), -0.0, &mut host)
                .unwrap()
                .to_bits(),
            (-0.0_f32).to_bits()
        );
        assert_eq!(
            host.calls,
            vec![Call::Tag(DamageReductionTag::BypassesArmor)]
        );
        host.calls.clear();
        host.bypass = Some(DamageReductionTag::BypassesEffects);
        assert_eq!(
            get_damage_after_magic_absorb(&(), -0.0, &mut host)
                .unwrap()
                .to_bits(),
            (-0.0_f32).to_bits()
        );
        assert_eq!(
            host.calls,
            vec![Call::Tag(DamageReductionTag::BypassesEffects)]
        );
        host.bypass = None;
        assert_eq!(
            get_damage_after_magic_absorb(&(), -0.0, &mut host)
                .unwrap()
                .to_bits(),
            0.0_f32.to_bits()
        );
    }

    #[test]
    fn resistance_awards_victim_before_protection_and_uses_causer_only_as_fallback() {
        let mut host = Host::new();
        host.resistance = true;
        host.victim_player = true;
        host.causing_player = true;
        assert_eq!(
            get_damage_after_magic_absorb(&(), 10.0, &mut host).unwrap(),
            6.4
        );
        assert_eq!(
            host.calls,
            vec![
                Call::Tag(DamageReductionTag::BypassesEffects),
                Call::HasResistance,
                Call::Tag(DamageReductionTag::BypassesResistance),
                Call::ResistanceAmplifier,
                Call::VictimPlayer,
                Call::Resisted(20),
                Call::Tag(DamageReductionTag::BypassesEnchantments),
                Call::ServerLevel,
                Call::Protection,
            ]
        );
        host.calls.clear();
        host.victim_player = false;
        get_damage_after_magic_absorb(&(), 10.0, &mut host).unwrap();
        assert!(host.calls.contains(&Call::DealtResisted(20)));
        assert!(!host.calls.contains(&Call::Resisted(20)));
    }

    #[test]
    fn nonfinite_damage_keeps_java_branches_and_stat_rounding_uses_exact_product() {
        let mut host = Host::new();
        host.resistance = true;
        host.victim_player = true;
        assert!(get_damage_after_magic_absorb(&(), f32::NAN, &mut host)
            .unwrap()
            .is_nan());
        assert!(!host.calls.contains(&Call::VictimPlayer));
        assert!(host.calls.contains(&Call::Protection));
        assert_eq!(round_statistic(8_388_609.0), 8_388_609);
        assert_eq!(round_statistic(0.5), 1);
        assert_eq!(round_statistic(f32::MAX), i32::MAX);
    }
}
