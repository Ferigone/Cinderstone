//! Source-ordered Minecraft Java 1.21 `Player.hurt` / `Player.actuallyHurt`.
//!
//! Source: pinned original server, official mappings `Player` (`cmx`), source
//! lines 945..977 and 1044..1070. The host keeps the real entity identity,
//! health, absorption, abilities, equipment, effects, combat tracker and world.
//! These entry points own none of that state. `ServerPlayer.hurt` (`aqv`, source
//! lines 792..811) is a separate wrapper below; parent `LivingEntity.hurt`,
//! death, respawn and the tick scheduler remain required actual owners.
//!
//! Every queried fact and side effect is fallible and mandatory. A failed later
//! callback preserves the completed mutation prefix; this is not a transaction.

/// A view of the world's current difficulty, not a second world-state owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerDifficulty {
    Peaceful,
    Easy,
    Normal,
    Hard,
}

/// Vanilla custom statistics used by `Player.actuallyHurt`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerDamageStat {
    DamageAbsorbed,
    DamageTaken,
}

pub trait PlayerDamageContext {
    /// The caller's actual DamageSource identity, including its type holder and
    /// direct/causing entities. Never replace it with just a numeric damage ID.
    type Source: ?Sized;
    type Error;

    /// Virtual `this.isInvulnerableTo(source)`, including the actual Player /
    /// ServerPlayer override and its game-rule checks at this call site.
    fn is_invulnerable_to(&mut self, source: &Self::Source) -> Result<bool, Self::Error>;
}

/// Required owners for Player's wrapper around the parent living damage path.
/// No default callback supplies an empty shoulder, alive flag or creative mode.
pub trait PlayerHurtHost: PlayerDamageContext {
    fn abilities_invulnerable(&mut self) -> Result<bool, Self::Error>;
    fn source_bypasses_invulnerability(
        &mut self,
        source: &Self::Source,
    ) -> Result<bool, Self::Error>;
    fn set_no_action_time(&mut self, ticks: i32) -> Result<(), Self::Error>;
    /// Query actual synchronized health / lifecycle, not a synthetic alive flag.
    fn is_dead_or_dying(&mut self) -> Result<bool, Self::Error>;
    fn is_client_side(&mut self) -> Result<bool, Self::Error>;
    /// Actual Player.removeEntitiesOnShoulder, including its RNG/world owners.
    fn remove_entities_on_shoulder(&mut self) -> Result<(), Self::Error>;
    fn source_scales_with_difficulty(&mut self, source: &Self::Source)
        -> Result<bool, Self::Error>;
    /// Invoked at each original Level.getDifficulty read; do not cache ahead of
    /// preceding callbacks or collapse the three source comparisons into one.
    fn difficulty(&mut self) -> Result<PlayerDifficulty, Self::Error>;
    /// The real parent LivingEntity.hurt executor. It must dispatch the virtual
    /// actuallyHurt back through Player's implementation and retain the source.
    fn living_hurt(&mut self, source: &Self::Source, amount: f32) -> Result<bool, Self::Error>;
}

/// Player.hurt, original source lines 945..977.
pub fn hurt<H: PlayerHurtHost + ?Sized>(
    host: &mut H,
    source: &H::Source,
    mut amount: f32,
) -> Result<bool, H::Error> {
    if host.is_invulnerable_to(source)? {
        return Ok(false);
    }
    if host.abilities_invulnerable()? && !host.source_bypasses_invulnerability(source)? {
        return Ok(false);
    }
    host.set_no_action_time(0)?;
    if host.is_dead_or_dying()? {
        return Ok(false);
    }
    if !host.is_client_side()? {
        host.remove_entities_on_shoulder()?;
    }
    if host.source_scales_with_difficulty(source)? {
        if host.difficulty()? == PlayerDifficulty::Peaceful {
            amount = 0.;
        }
        if host.difficulty()? == PlayerDifficulty::Easy {
            amount = java_min(amount / 2. + 1., amount);
        }
        if host.difficulty()? == PlayerDifficulty::Hard {
            amount = (amount * 3.) / 2.;
        }
    }
    // Java's ==0 test rejects both signed zeros, but forwards NaN and negatives.
    if amount == 0. {
        return Ok(false);
    }
    host.living_hurt(source, amount)
}

/// Required ServerPlayer.hurt contexts. In particular spawn protection is the
/// distinct ServerPlayer.spawnInvulnerableTime, not Entity.invulnerableTime.
pub trait ServerPlayerHurtHost: PlayerDamageContext {
    /// Identity-bearing handle to an actual entity, not just its entity type.
    type Entity;
    fn is_dedicated_server(&mut self) -> Result<bool, Self::Error>;
    fn is_pvp_allowed(&mut self) -> Result<bool, Self::Error>;
    fn source_is_fall(&mut self, source: &Self::Source) -> Result<bool, Self::Error>;
    fn spawn_invulnerable_time(&mut self) -> Result<i32, Self::Error>;
    fn source_bypasses_invulnerability(
        &mut self,
        source: &Self::Source,
    ) -> Result<bool, Self::Error>;
    /// DamageSource.getEntity, the causing entity. It is not getDirectEntity.
    fn source_causing_entity(
        &mut self,
        source: &Self::Source,
    ) -> Result<Option<Self::Entity>, Self::Error>;
    fn entity_is_player(&mut self, entity: &Self::Entity) -> Result<bool, Self::Error>;
    /// Virtual ServerPlayer.canHarmPlayer: actual PvP property first, then
    /// Player.canHarmPlayer including teams/friendly-fire rules.
    fn can_harm_player(&mut self, player: &Self::Entity) -> Result<bool, Self::Error>;
    fn entity_is_abstract_arrow(&mut self, entity: &Self::Entity) -> Result<bool, Self::Error>;
    /// Actual AbstractArrow.getOwner, including its real level/UUID lookup.
    fn arrow_owner(&mut self, arrow: &Self::Entity) -> Result<Option<Self::Entity>, Self::Error>;
    /// Parent Player.hurt, preserving source identity and repeated virtual
    /// invulnerability checks rather than skipping straight to LivingEntity.
    fn player_hurt(&mut self, source: &Self::Source, amount: f32) -> Result<bool, Self::Error>;
}

/// ServerPlayer.hurt, original source lines 792..811. Dedicated-server PvP fall
/// damage bypasses spawn protection, but still executes subsequent player gates.
pub fn server_player_hurt<H: ServerPlayerHurtHost + ?Sized>(
    host: &mut H,
    source: &H::Source,
    amount: f32,
) -> Result<bool, H::Error> {
    if host.is_invulnerable_to(source)? {
        return Ok(false);
    }
    let fall_exception =
        host.is_dedicated_server()? && host.is_pvp_allowed()? && host.source_is_fall(source)?;
    if !fall_exception
        && host.spawn_invulnerable_time()? > 0
        && !host.source_bypasses_invulnerability(source)?
    {
        return Ok(false);
    }
    if let Some(entity) = host.source_causing_entity(source)? {
        if host.entity_is_player(&entity)? && !host.can_harm_player(&entity)? {
            return Ok(false);
        }
        if host.entity_is_abstract_arrow(&entity)? {
            if let Some(owner) = host.arrow_owner(&entity)? {
                if host.entity_is_player(&owner)? && !host.can_harm_player(&owner)? {
                    return Ok(false);
                }
            }
        }
    }
    host.player_hurt(source, amount)
}

/// Required canonical owners for Player's actual health-damage callback.
pub trait PlayerActuallyHurtHost: PlayerDamageContext {
    /// Virtual LivingEntity.getDamageAfterArmorAbsorb, including armor damage,
    /// source tags, current armor/toughness and weapon-enchantment context.
    fn damage_after_armor_absorb(
        &mut self,
        source: &Self::Source,
        amount: f32,
    ) -> Result<f32, Self::Error>;
    /// Virtual LivingEntity.getDamageAfterMagicAbsorb, including effects,
    /// resistance statistics, enchantment and source-tag callbacks.
    fn damage_after_magic_absorb(
        &mut self,
        source: &Self::Source,
        amount: f32,
    ) -> Result<f32, Self::Error>;
    fn absorption_amount(&mut self) -> Result<f32, Self::Error>;
    /// Actual virtual setAbsorptionAmount; its attribute clamp remains owned by
    /// the living entity. This method receives the un-clamped source expression.
    fn set_absorption_amount(&mut self, amount: f32) -> Result<(), Self::Error>;
    /// Actual Player.awardStat, including the real statistic/scoreboard owners.
    fn award_stat(&mut self, stat: PlayerDamageStat, amount: i32) -> Result<(), Self::Error>;
    fn source_food_exhaustion(&mut self, source: &Self::Source) -> Result<f32, Self::Error>;
    /// Actual Player.causeFoodExhaustion, including side and ability gates.
    fn cause_food_exhaustion(&mut self, amount: f32) -> Result<(), Self::Error>;
    fn combat_tracker_record_damage(
        &mut self,
        source: &Self::Source,
        amount: f32,
    ) -> Result<(), Self::Error>;
    fn health(&mut self) -> Result<f32, Self::Error>;
    /// Actual virtual setHealth, with the current MAX_HEALTH attribute clamp.
    fn set_health(&mut self, amount: f32) -> Result<(), Self::Error>;
    /// Actual this.gameEvent(GameEvent.ENTITY_DAMAGE), with this actor as source.
    fn entity_damage_game_event(&mut self) -> Result<(), Self::Error>;
}

/// Player.actuallyHurt, original source lines 1044..1070. All absorption and
/// health reads occur where the JVM reads them, around mandatory callbacks.
pub fn actually_hurt<H: PlayerActuallyHurtHost + ?Sized>(
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
    if absorbed > 0. && absorbed < STAT_DAMAGE_LIMIT {
        host.award_stat(PlayerDamageStat::DamageAbsorbed, java_round(absorbed * 10.))?;
    }
    if amount == 0. {
        return Ok(());
    }
    let exhaustion = host.source_food_exhaustion(source)?;
    host.cause_food_exhaustion(exhaustion)?;
    host.combat_tracker_record_damage(source, amount)?;
    let health = host.health()?;
    host.set_health(health - amount)?;
    // This is only an upper bound in Java: NaN fails it, negatives satisfy it.
    if amount < STAT_DAMAGE_LIMIT {
        host.award_stat(PlayerDamageStat::DamageTaken, java_round(amount * 10.))?;
    }
    host.entity_damage_game_event()
}

// Exact float constant in Player's original constant pool (Float.MAX_VALUE/10).
const STAT_DAMAGE_LIMIT: f32 = 3.4028235E37_f32;

fn java_min(a: f32, b: f32) -> f32 {
    if a.is_nan() {
        a
    } else if a == 0. && b == 0. && b.is_sign_negative() {
        b
    } else if a <= b {
        a
    } else {
        b
    }
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
/// Java Math.round(float), including ties toward positive infinity, saturating
/// f2i outside range and NaN ->0. Float->double is exact before the half offset.
fn java_round(value: f32) -> i32 {
    ((value as f64 + 0.5).floor()) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Host {
        trace: Vec<String>,
        health: f32,
        absorption: f32,
        invulnerable: bool,
        ability_invulnerable: bool,
        bypasses: bool,
        dead: bool,
        scales: bool,
        difficulty: PlayerDifficulty,
        fail_stat: bool,
        dedicated: bool,
        fall: bool,
        spawn_time: i32,
        causer: Option<u32>,
        harm_player: bool,
    }
    impl Host {
        fn new() -> Self {
            Self {
                trace: Vec::new(),
                health: 20.,
                absorption: 3.,
                invulnerable: false,
                ability_invulnerable: false,
                bypasses: false,
                dead: false,
                scales: true,
                difficulty: PlayerDifficulty::Normal,
                fail_stat: false,
                dedicated: true,
                fall: false,
                spawn_time: 0,
                causer: None,
                harm_player: true,
            }
        }
        fn log(&mut self, s: impl Into<String>) {
            self.trace.push(s.into());
        }
    }
    impl PlayerDamageContext for Host {
        type Source = u32;
        type Error = &'static str;
        fn is_invulnerable_to(&mut self, s: &u32) -> Result<bool, Self::Error> {
            assert_eq!(*s, 767);
            self.log("invulnerable");
            Ok(self.invulnerable)
        }
    }
    impl PlayerHurtHost for Host {
        fn abilities_invulnerable(&mut self) -> Result<bool, Self::Error> {
            self.log("ability");
            Ok(self.ability_invulnerable)
        }
        fn source_bypasses_invulnerability(&mut self, s: &u32) -> Result<bool, Self::Error> {
            assert_eq!(*s, 767);
            self.log("bypasses");
            Ok(self.bypasses)
        }
        fn set_no_action_time(&mut self, t: i32) -> Result<(), Self::Error> {
            self.log(format!("no_action={t}"));
            Ok(())
        }
        fn is_dead_or_dying(&mut self) -> Result<bool, Self::Error> {
            self.log("dead");
            Ok(self.dead)
        }
        fn is_client_side(&mut self) -> Result<bool, Self::Error> {
            self.log("client");
            Ok(false)
        }
        fn remove_entities_on_shoulder(&mut self) -> Result<(), Self::Error> {
            self.log("shoulders");
            Ok(())
        }
        fn source_scales_with_difficulty(&mut self, s: &u32) -> Result<bool, Self::Error> {
            assert_eq!(*s, 767);
            self.log("scales");
            Ok(self.scales)
        }
        fn difficulty(&mut self) -> Result<PlayerDifficulty, Self::Error> {
            self.log("difficulty");
            Ok(self.difficulty)
        }
        fn living_hurt(&mut self, s: &u32, a: f32) -> Result<bool, Self::Error> {
            assert_eq!(*s, 767);
            self.log(format!("living={a}"));
            Ok(true)
        }
    }
    impl PlayerActuallyHurtHost for Host {
        fn damage_after_armor_absorb(&mut self, s: &u32, a: f32) -> Result<f32, Self::Error> {
            assert_eq!(*s, 767);
            self.log(format!("armor={a}"));
            Ok(a / 2.)
        }
        fn damage_after_magic_absorb(&mut self, s: &u32, a: f32) -> Result<f32, Self::Error> {
            assert_eq!(*s, 767);
            self.log(format!("magic={a}"));
            Ok(a)
        }
        fn absorption_amount(&mut self) -> Result<f32, Self::Error> {
            self.log("absorption");
            Ok(self.absorption)
        }
        fn set_absorption_amount(&mut self, a: f32) -> Result<(), Self::Error> {
            self.log(format!("set_absorption={a}"));
            self.absorption = a;
            Ok(())
        }
        fn award_stat(&mut self, s: PlayerDamageStat, a: i32) -> Result<(), Self::Error> {
            self.log(format!("stat={s:?}:{a}"));
            if self.fail_stat {
                Err("stat failure")
            } else {
                Ok(())
            }
        }
        fn source_food_exhaustion(&mut self, s: &u32) -> Result<f32, Self::Error> {
            assert_eq!(*s, 767);
            self.log("source_exhaustion");
            Ok(0.1)
        }
        fn cause_food_exhaustion(&mut self, a: f32) -> Result<(), Self::Error> {
            self.log(format!("exhaustion={a}"));
            Ok(())
        }
        fn combat_tracker_record_damage(&mut self, s: &u32, a: f32) -> Result<(), Self::Error> {
            assert_eq!(*s, 767);
            self.log(format!("combat={a}"));
            Ok(())
        }
        fn health(&mut self) -> Result<f32, Self::Error> {
            self.log("health");
            Ok(self.health)
        }
        fn set_health(&mut self, a: f32) -> Result<(), Self::Error> {
            self.log(format!("set_health={a}"));
            self.health = a;
            Ok(())
        }
        fn entity_damage_game_event(&mut self) -> Result<(), Self::Error> {
            self.log("entity_damage");
            Ok(())
        }
    }
    impl ServerPlayerHurtHost for Host {
        type Entity = u32;
        fn is_dedicated_server(&mut self) -> Result<bool, Self::Error> {
            self.log("dedicated");
            Ok(self.dedicated)
        }
        fn is_pvp_allowed(&mut self) -> Result<bool, Self::Error> {
            self.log("pvp");
            Ok(true)
        }
        fn source_is_fall(&mut self, s: &u32) -> Result<bool, Self::Error> {
            assert_eq!(*s, 767);
            self.log("fall");
            Ok(self.fall)
        }
        fn spawn_invulnerable_time(&mut self) -> Result<i32, Self::Error> {
            self.log("spawn_time");
            Ok(self.spawn_time)
        }
        fn source_bypasses_invulnerability(&mut self, s: &u32) -> Result<bool, Self::Error> {
            assert_eq!(*s, 767);
            self.log("server_bypasses");
            Ok(self.bypasses)
        }
        fn source_causing_entity(&mut self, s: &u32) -> Result<Option<u32>, Self::Error> {
            assert_eq!(*s, 767);
            self.log("causer");
            Ok(self.causer)
        }
        fn entity_is_player(&mut self, e: &u32) -> Result<bool, Self::Error> {
            self.log(format!("is_player={e}"));
            Ok(*e == 1)
        }
        fn can_harm_player(&mut self, p: &u32) -> Result<bool, Self::Error> {
            assert_eq!(*p, 1);
            self.log("can_harm");
            Ok(self.harm_player)
        }
        fn entity_is_abstract_arrow(&mut self, e: &u32) -> Result<bool, Self::Error> {
            self.log(format!("is_arrow={e}"));
            Ok(*e == 2)
        }
        fn arrow_owner(&mut self, e: &u32) -> Result<Option<u32>, Self::Error> {
            assert_eq!(*e, 2);
            self.log("arrow_owner");
            Ok(Some(1))
        }
        fn player_hurt(&mut self, s: &u32, a: f32) -> Result<bool, Self::Error> {
            self.log("player_hurt");
            hurt(self, s, a)
        }
    }

    #[test]
    fn server_spawn_fall_and_causing_arrow_gates() {
        let mut host = Host::new();
        host.dedicated = false;
        host.spawn_time = 20;
        assert_eq!(server_player_hurt(&mut host, &767, 3.), Ok(false));
        assert_eq!(
            host.trace,
            ["invulnerable", "dedicated", "spawn_time", "server_bypasses"]
        );
        host.trace.clear();
        host.dedicated = true;
        host.fall = true;
        assert_eq!(server_player_hurt(&mut host, &767, 3.), Ok(true));
        assert_eq!(
            &host.trace[..7],
            [
                "invulnerable",
                "dedicated",
                "pvp",
                "fall",
                "causer",
                "player_hurt",
                "invulnerable"
            ]
        );
        assert!(!host.trace.iter().any(|s| s == "spawn_time"));
        host.trace.clear();
        host.dedicated = false;
        host.spawn_time = 0;
        host.causer = Some(2);
        host.harm_player = false;
        assert_eq!(server_player_hurt(&mut host, &767, 3.), Ok(false));
        assert_eq!(
            host.trace,
            [
                "invulnerable",
                "dedicated",
                "spawn_time",
                "causer",
                "is_player=2",
                "is_arrow=2",
                "arrow_owner",
                "is_player=1",
                "can_harm"
            ]
        );
        assert!(!host.trace.iter().any(|s| s == "player_hurt"));
    }

    #[test]
    fn source_callback_order_and_partial_failure() {
        let mut host = Host::new();
        host.difficulty = PlayerDifficulty::Peaceful;
        assert_eq!(hurt(&mut host, &767, 10.), Ok(false));
        assert_eq!(
            host.trace,
            [
                "invulnerable",
                "ability",
                "no_action=0",
                "dead",
                "client",
                "shoulders",
                "scales",
                "difficulty",
                "difficulty",
                "difficulty"
            ]
        );
        host.trace.clear();
        host.difficulty = PlayerDifficulty::Easy;
        assert_eq!(hurt(&mut host, &767, 10.), Ok(true));
        assert_eq!(host.trace.last().unwrap(), "living=6");
        host.trace.clear();
        actually_hurt(&mut host, &767, 10.).unwrap();
        assert_eq!(host.health, 18.);
        assert_eq!(host.absorption, 0.);
        assert_eq!(
            host.trace,
            [
                "invulnerable",
                "armor=10",
                "magic=5",
                "absorption",
                "absorption",
                "set_absorption=0",
                "stat=DamageAbsorbed:30",
                "source_exhaustion",
                "exhaustion=0.1",
                "combat=2",
                "health",
                "set_health=18",
                "stat=DamageTaken:20",
                "entity_damage"
            ]
        );
        let mut fail = Host::new();
        fail.fail_stat = true;
        assert_eq!(actually_hurt(&mut fail, &767, 10.), Err("stat failure"));
        assert_eq!(fail.absorption, 0.);
        assert_eq!(fail.health, 20.);
        assert_eq!(fail.trace.last().unwrap(), "stat=DamageAbsorbed:30");
    }

    #[test]
    fn zero_absorbed_and_numeric_edges_follow_java() {
        let mut host = Host::new();
        actually_hurt(&mut host, &767, 4.).unwrap();
        assert_eq!(host.health, 20.);
        assert_eq!(host.absorption, 1.);
        assert_eq!(host.trace.last().unwrap(), "stat=DamageAbsorbed:20");
        assert_eq!(java_min(0., -0.).to_bits(), (-0_f32).to_bits());
        assert_eq!(java_max(-0., 0.).to_bits(), 0_f32.to_bits());
        assert!(java_min(1., f32::NAN).is_nan());
        assert!(java_max(1., f32::NAN).is_nan());
        assert_eq!(java_round(-1.5), -1);
        assert_eq!(java_round(f32::from_bits(0x3effffff)), 0);
        assert_eq!(java_round(f32::NAN), 0);
        assert_eq!(java_round(f32::INFINITY), i32::MAX);
        assert_eq!(java_round(f32::NEG_INFINITY), i32::MIN);
        host.trace.clear();
        host.scales = false;
        assert_eq!(hurt(&mut host, &767, -0.), Ok(false));
        assert!(!host.trace.iter().any(|t| t.starts_with("living=")));
        host.trace.clear();
        assert_eq!(hurt(&mut host, &767, f32::NAN), Ok(true));
        assert_eq!(host.trace.last().unwrap(), "living=NaN");
    }
}
