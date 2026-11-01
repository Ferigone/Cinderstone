//! Base Mob spawn and despawn phases from Minecraft Java 1.21.
//!
//! Pinned source: Mob (`btp`) lines 747–775, 892–901 and 1207–1215;
//! RandomSource (`ayw`) line 75. These are the base methods, not a replacement
//! for a subtype's overrides, NaturalSpawner, EntityType construction, world
//! registration or the actor's actual Level tick. They own no actor, RNG,
//! attributes, nearest-player index, scheduler, loot table or persistence state.
//! Missing owners must fail through a host callback. Earlier source mutations
//! and random draws remain consumed if a later callback fails.

pub type BlockPos = (i32, i32, i32);
pub const RANDOM_SPAWN_BONUS_ID: &str = "minecraft:random_spawn_bonus";
pub const RANDOM_SPAWN_BONUS_DEVIATION: f64 = 0.11485000000000001;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpawnModifierOperation {
    AddMultipliedBase,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpawnAttributeModifier {
    pub id: &'static str,
    pub amount: f64,
    pub operation: SpawnModifierOperation,
}

/// The actual actor's FOLLOW_RANGE AttributeInstance and the actual supplied
/// ServerLevelAccessor.getRandom owner. Handles retain their canonical identity
/// through callbacks; obtaining them must not construct or fork random state.
pub trait MobFinalizeSpawnHost {
    type Error;
    type LevelRandom;
    type FollowRangeAttribute;

    fn level_random(&mut self) -> Result<Self::LevelRandom, Self::Error>;
    /// The source requires this attribute to exist, before checking modifiers.
    fn follow_range_attribute(&mut self) -> Result<Self::FollowRangeAttribute, Self::Error>;
    fn has_modifier(
        &mut self,
        attribute: &Self::FollowRangeAttribute,
        id: &str,
    ) -> Result<bool, Self::Error>;
    fn level_random_next_f64(&mut self, random: &Self::LevelRandom) -> Result<f64, Self::Error>;
    fn add_permanent_modifier(
        &mut self,
        attribute: &Self::FollowRangeAttribute,
        modifier: SpawnAttributeModifier,
    ) -> Result<(), Self::Error>;
    fn level_random_next_f32(&mut self, random: &Self::LevelRandom) -> Result<f32, Self::Error>;
    /// Actual Mob.setLeftHanded, including its synchronized mob-flag setter.
    fn set_left_handed(&mut self, left_handed: bool) -> Result<(), Self::Error>;
}

/// Base Mob.finalizeSpawn, source lines 1207–1215. Difficulty and reason are
/// genuinely unused in this base method. The group value is returned unchanged,
/// retaining identity/nullability; no subtype/group state is manufactured.
///
/// RandomSource.triangle uses two nextDouble calls in order, not nextGaussian.
/// The bonus is permanent, and an existing modifier skips those two draws.
pub fn finalize_spawn<D, R, G, H: MobFinalizeSpawnHost>(
    _difficulty: &D,
    _reason: &R,
    group: G,
    host: &mut H,
) -> Result<G, H::Error> {
    let random = host.level_random()?;
    let attribute = host.follow_range_attribute()?;
    if !host.has_modifier(&attribute, RANDOM_SPAWN_BONUS_ID)? {
        let first = host.level_random_next_f64(&random)?;
        let second = host.level_random_next_f64(&random)?;
        let amount = 0.0 + RANDOM_SPAWN_BONUS_DEVIATION * (first - second);
        host.add_permanent_modifier(
            &attribute,
            SpawnAttributeModifier {
                id: RANDOM_SPAWN_BONUS_ID,
                amount,
                operation: SpawnModifierOperation::AddMultipliedBase,
            },
        )?;
    }
    let left_handed = host.level_random_next_f32(&random)? < 0.05;
    host.set_left_handed(left_handed)?;
    Ok(group)
}

pub trait MobSpawnRulesHost<T, R> {
    type Error;
    type BlockState;

    /// Exact MobSpawnType.SPAWNER identity. This is not isSpawner(reason), and
    /// TRIAL_SPAWNER must not enter this branch merely because of its category.
    fn reason_is_spawner(&mut self, reason: &R) -> Result<bool, Self::Error>;
    fn block_state(&mut self, at: BlockPos) -> Result<Self::BlockState, Self::Error>;
    fn block_is_valid_spawn(
        &mut self,
        state: &Self::BlockState,
        at: BlockPos,
        entity_type: &T,
    ) -> Result<bool, Self::Error>;
}

/// Static Mob.checkMobSpawnRules, source lines 892–893. The random argument is
/// genuinely unused. below() is computed before the SPAWNER identity check.
pub fn check_mob_spawn_rules<T, R, Q, H: MobSpawnRulesHost<T, R>>(
    entity_type: &T,
    reason: &R,
    position: BlockPos,
    _random: &Q,
    host: &mut H,
) -> Result<bool, H::Error> {
    let below = (position.0, position.1.wrapping_sub(1), position.2);
    if host.reason_is_spawner(reason)? {
        return Ok(true);
    }
    let state = host.block_state(below)?;
    host.block_is_valid_spawn(&state, below, entity_type)
}

/// The base instance Mob.checkSpawnRules method, source line 897, returns true.
/// Actual subtype dispatch and SpawnPlacements are the caller's responsibility.
pub fn base_check_spawn_rules<L, R>(_level: &L, _reason: &R) -> bool {
    true
}

pub trait MobSpawnObstructionHost {
    type Error;
    type Bounds;

    fn bounding_box(&mut self) -> Result<Self::Bounds, Self::Error>;
    fn contains_any_liquid(&mut self, bounds: &Self::Bounds) -> Result<bool, Self::Error>;
    /// LevelReader.isUnobstructed on the actual actor, including its actual
    /// bounding box and native entity/shape query semantics at this call site.
    fn actor_is_unobstructed(&mut self) -> Result<bool, Self::Error>;
}

/// Mob.checkSpawnObstruction, source line 901. The entity/shape query is skipped
/// when the actor's bounding box contains liquid; no empty-world fallback exists.
pub fn check_spawn_obstruction<H: MobSpawnObstructionHost>(host: &mut H) -> Result<bool, H::Error> {
    let bounds = host.bounding_box()?;
    Ok(!host.contains_any_liquid(&bounds)? && host.actor_is_unobstructed()?)
}

/// Live Mob.checkDespawn facts and mutations. The Level's actual nearest-player
/// query, category distances and virtual methods must remain real owners. A
/// Player is an opaque retained canonical handle, not a copied actor snapshot.
pub trait MobDespawnHost {
    type Error;
    type Player;

    fn difficulty_is_peaceful(&mut self) -> Result<bool, Self::Error>;
    fn should_despawn_in_peaceful(&mut self) -> Result<bool, Self::Error>;
    /// Actual Entity.discard; retain removal, brain/effect and world callbacks.
    /// Despawning is not the death/loot/XP path and must not manufacture drops.
    fn discard(&mut self) -> Result<(), Self::Error>;
    fn is_persistence_required(&mut self) -> Result<bool, Self::Error>;
    fn requires_custom_persistence(&mut self) -> Result<bool, Self::Error>;
    fn set_no_action_time(&mut self, time: i32) -> Result<(), Self::Error>;
    /// Level.getNearestPlayer(this, maximum_distance), with its real filters.
    fn nearest_player(
        &mut self,
        maximum_distance: f64,
    ) -> Result<Option<Self::Player>, Self::Error>;
    /// The retained player's distanceToSqr(this), evaluated once by this phase.
    fn player_distance_to_sqr(&mut self, player: &Self::Player) -> Result<f64, Self::Error>;
    fn category_despawn_distance(&mut self) -> Result<i32, Self::Error>;
    /// Actual virtual Mob.removeWhenFarAway; subtype behavior is not assumed.
    fn remove_when_far_away(&mut self, distance_sqr: f64) -> Result<bool, Self::Error>;
    fn category_no_despawn_distance(&mut self) -> Result<i32, Self::Error>;
    fn no_action_time(&mut self) -> Result<i32, Self::Error>;
    /// Consume this actor's actual retained RNG, not the Level RNG used above.
    fn entity_random_next_bounded_i32(&mut self, bound: i32) -> Result<i32, Self::Error>;
}

/// Mob.checkDespawn, source lines 747–775. This runner increments no clock and
/// schedules nothing; the actual ServerLevel entity phase owns its call site.
/// Java int multiplication wraps for the squared category distances.
pub fn check_despawn<H: MobDespawnHost>(host: &mut H) -> Result<(), H::Error> {
    if host.difficulty_is_peaceful()? && host.should_despawn_in_peaceful()? {
        host.discard()?;
        return Ok(());
    }
    if host.is_persistence_required()? || host.requires_custom_persistence()? {
        host.set_no_action_time(0)?;
        return Ok(());
    }
    let Some(player) = host.nearest_player(-1.0)? else {
        return Ok(());
    };
    let distance = host.player_distance_to_sqr(&player)?;
    let despawn_distance = host.category_despawn_distance()?;
    let despawn_distance_sqr = despawn_distance.wrapping_mul(despawn_distance);
    if distance > f64::from(despawn_distance_sqr) && host.remove_when_far_away(distance)? {
        // The original has no return here. Subsequent category/RNG callbacks
        // still run and can reach a second discard call on the same actor.
        host.discard()?;
    }
    let no_despawn_distance = host.category_no_despawn_distance()?;
    let no_despawn_distance_sqr = no_despawn_distance.wrapping_mul(no_despawn_distance);
    if host.no_action_time()? > 600
        && host.entity_random_next_bounded_i32(800)? == 0
        && distance > f64::from(no_despawn_distance_sqr)
        && host.remove_when_far_away(distance)?
    {
        host.discard()?;
    } else if distance < f64::from(no_despawn_distance_sqr) {
        host.set_no_action_time(0)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::random::LegacyRandomSource;
    use std::rc::Rc;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Reason {
        Natural,
        Spawner,
        TrialSpawner,
    }

    #[derive(Debug, PartialEq)]
    enum Call {
        LevelRandom,
        FollowRange,
        HasModifier(String),
        LevelDouble,
        Modifier(SpawnAttributeModifier),
        LevelFloat,
        LeftHanded(bool),
        Reason(Reason),
        Block(BlockPos),
        ValidSpawn(BlockPos, u32),
        Bounds,
        Liquid,
        Unobstructed,
        Peaceful,
        DespawnPeaceful,
        Persistence,
        CustomPersistence,
        NoAction(i32),
        Nearest(u64),
        PlayerDistance(u32),
        DespawnDistance,
        RemoveFar(u64),
        Discard,
        NoDespawnDistance,
        ReadNoAction,
        EntityRandom(i32),
    }

    struct Host {
        calls: Vec<Call>,
        level_random: LegacyRandomSource,
        entity_random: LegacyRandomSource,
        has_modifier: bool,
        fail_modifier: bool,
        fail_no_despawn_distance: bool,
        peaceful: bool,
        despawn_peaceful: bool,
        persistent: bool,
        custom_persistence: bool,
        player: Option<u32>,
        distance: f64,
        despawn_distance: i32,
        no_despawn_distance: i32,
        no_action: i32,
        remove_far: bool,
        discarded: usize,
        liquid: bool,
        unobstructed: bool,
        valid_spawn: bool,
    }

    impl Host {
        fn new() -> Self {
            Self {
                calls: Vec::new(),
                level_random: LegacyRandomSource::new(17),
                entity_random: LegacyRandomSource::new(923),
                has_modifier: false,
                fail_modifier: false,
                fail_no_despawn_distance: false,
                peaceful: false,
                despawn_peaceful: false,
                persistent: false,
                custom_persistence: false,
                player: Some(7),
                distance: 100_000.0,
                despawn_distance: 128,
                no_despawn_distance: 32,
                no_action: 601,
                remove_far: true,
                discarded: 0,
                liquid: false,
                unobstructed: true,
                valid_spawn: false,
            }
        }
    }

    impl MobFinalizeSpawnHost for Host {
        type Error = &'static str;
        type LevelRandom = u32;
        type FollowRangeAttribute = u32;

        fn level_random(&mut self) -> Result<u32, Self::Error> {
            self.calls.push(Call::LevelRandom);
            Ok(11)
        }
        fn follow_range_attribute(&mut self) -> Result<u32, Self::Error> {
            self.calls.push(Call::FollowRange);
            Ok(19)
        }
        fn has_modifier(&mut self, attribute: &u32, id: &str) -> Result<bool, Self::Error> {
            assert_eq!(*attribute, 19);
            self.calls.push(Call::HasModifier(id.to_owned()));
            Ok(self.has_modifier)
        }
        fn level_random_next_f64(&mut self, random: &u32) -> Result<f64, Self::Error> {
            assert_eq!(*random, 11);
            self.calls.push(Call::LevelDouble);
            Ok(self.level_random.next_f64())
        }
        fn add_permanent_modifier(
            &mut self,
            attribute: &u32,
            modifier: SpawnAttributeModifier,
        ) -> Result<(), Self::Error> {
            assert_eq!(*attribute, 19);
            self.calls.push(Call::Modifier(modifier));
            if self.fail_modifier {
                Err("attribute owner unavailable")
            } else {
                self.has_modifier = true;
                Ok(())
            }
        }
        fn level_random_next_f32(&mut self, random: &u32) -> Result<f32, Self::Error> {
            assert_eq!(*random, 11);
            self.calls.push(Call::LevelFloat);
            Ok(self.level_random.next_f32())
        }
        fn set_left_handed(&mut self, value: bool) -> Result<(), Self::Error> {
            self.calls.push(Call::LeftHanded(value));
            Ok(())
        }
    }

    impl MobSpawnRulesHost<u32, Reason> for Host {
        type Error = &'static str;
        type BlockState = u32;
        fn reason_is_spawner(&mut self, reason: &Reason) -> Result<bool, Self::Error> {
            self.calls.push(Call::Reason(*reason));
            Ok(*reason == Reason::Spawner)
        }
        fn block_state(&mut self, at: BlockPos) -> Result<u32, Self::Error> {
            self.calls.push(Call::Block(at));
            Ok(29)
        }
        fn block_is_valid_spawn(
            &mut self,
            state: &u32,
            at: BlockPos,
            entity_type: &u32,
        ) -> Result<bool, Self::Error> {
            assert_eq!(*state, 29);
            self.calls.push(Call::ValidSpawn(at, *entity_type));
            Ok(self.valid_spawn)
        }
    }

    impl MobSpawnObstructionHost for Host {
        type Error = &'static str;
        type Bounds = u32;
        fn bounding_box(&mut self) -> Result<u32, Self::Error> {
            self.calls.push(Call::Bounds);
            Ok(31)
        }
        fn contains_any_liquid(&mut self, bounds: &u32) -> Result<bool, Self::Error> {
            assert_eq!(*bounds, 31);
            self.calls.push(Call::Liquid);
            Ok(self.liquid)
        }
        fn actor_is_unobstructed(&mut self) -> Result<bool, Self::Error> {
            self.calls.push(Call::Unobstructed);
            Ok(self.unobstructed)
        }
    }

    impl MobDespawnHost for Host {
        type Error = &'static str;
        type Player = u32;
        fn difficulty_is_peaceful(&mut self) -> Result<bool, Self::Error> {
            self.calls.push(Call::Peaceful);
            Ok(self.peaceful)
        }
        fn should_despawn_in_peaceful(&mut self) -> Result<bool, Self::Error> {
            self.calls.push(Call::DespawnPeaceful);
            Ok(self.despawn_peaceful)
        }
        fn discard(&mut self) -> Result<(), Self::Error> {
            self.calls.push(Call::Discard);
            self.discarded += 1;
            Ok(())
        }
        fn is_persistence_required(&mut self) -> Result<bool, Self::Error> {
            self.calls.push(Call::Persistence);
            Ok(self.persistent)
        }
        fn requires_custom_persistence(&mut self) -> Result<bool, Self::Error> {
            self.calls.push(Call::CustomPersistence);
            Ok(self.custom_persistence)
        }
        fn set_no_action_time(&mut self, time: i32) -> Result<(), Self::Error> {
            self.calls.push(Call::NoAction(time));
            self.no_action = time;
            Ok(())
        }
        fn nearest_player(&mut self, maximum_distance: f64) -> Result<Option<u32>, Self::Error> {
            self.calls.push(Call::Nearest(maximum_distance.to_bits()));
            Ok(self.player)
        }
        fn player_distance_to_sqr(&mut self, player: &u32) -> Result<f64, Self::Error> {
            assert_eq!(Some(*player), self.player);
            self.calls.push(Call::PlayerDistance(*player));
            Ok(self.distance)
        }
        fn category_despawn_distance(&mut self) -> Result<i32, Self::Error> {
            self.calls.push(Call::DespawnDistance);
            Ok(self.despawn_distance)
        }
        fn remove_when_far_away(&mut self, distance: f64) -> Result<bool, Self::Error> {
            self.calls.push(Call::RemoveFar(distance.to_bits()));
            Ok(self.remove_far)
        }
        fn category_no_despawn_distance(&mut self) -> Result<i32, Self::Error> {
            self.calls.push(Call::NoDespawnDistance);
            if self.fail_no_despawn_distance {
                Err("category owner unavailable")
            } else {
                Ok(self.no_despawn_distance)
            }
        }
        fn no_action_time(&mut self) -> Result<i32, Self::Error> {
            self.calls.push(Call::ReadNoAction);
            Ok(self.no_action)
        }
        fn entity_random_next_bounded_i32(&mut self, bound: i32) -> Result<i32, Self::Error> {
            self.calls.push(Call::EntityRandom(bound));
            self.entity_random
                .next_bounded_i32(bound)
                .map_err(|_| "invalid source bound")
        }
    }

    #[test]
    fn finalize_retains_group_identity_exact_level_draws_and_failure_prefix() {
        let mut host = Host::new();
        let group = Rc::new(42);
        let mut expected_random = host.level_random.clone();
        let first = expected_random.next_f64();
        let second = expected_random.next_f64();
        let amount = 0.0 + 0.11485000000000001 * (first - second);
        let left = expected_random.next_f32() < 0.05;
        let actor_random_before = host.entity_random.state48();
        let returned = finalize_spawn(&(), &Reason::Natural, group.clone(), &mut host).unwrap();
        assert!(Rc::ptr_eq(&group, &returned));
        assert_eq!(host.level_random.state48(), expected_random.state48());
        assert_eq!(host.entity_random.state48(), actor_random_before);
        assert_eq!(
            host.calls,
            [
                Call::LevelRandom,
                Call::FollowRange,
                Call::HasModifier(RANDOM_SPAWN_BONUS_ID.to_owned()),
                Call::LevelDouble,
                Call::LevelDouble,
                Call::Modifier(SpawnAttributeModifier {
                    id: RANDOM_SPAWN_BONUS_ID,
                    amount,
                    operation: SpawnModifierOperation::AddMultipliedBase,
                }),
                Call::LevelFloat,
                Call::LeftHanded(left),
            ]
        );

        host.calls.clear();
        let mut expected_random = host.level_random.clone();
        let left = expected_random.next_f32() < 0.05;
        let returned: Option<Rc<i32>> =
            finalize_spawn(&(), &Reason::Natural, None, &mut host).unwrap();
        assert!(returned.is_none());
        assert_eq!(host.level_random.state48(), expected_random.state48());
        assert_eq!(
            host.calls,
            [
                Call::LevelRandom,
                Call::FollowRange,
                Call::HasModifier(RANDOM_SPAWN_BONUS_ID.to_owned()),
                Call::LevelFloat,
                Call::LeftHanded(left),
            ]
        );

        let mut failing = Host::new();
        failing.fail_modifier = true;
        let mut consumed = failing.level_random.clone();
        consumed.next_f64();
        consumed.next_f64();
        assert_eq!(
            finalize_spawn(&(), &Reason::Natural, (), &mut failing),
            Err("attribute owner unavailable")
        );
        assert_eq!(failing.level_random.state48(), consumed.state48());
        assert!(matches!(failing.calls.last(), Some(Call::Modifier(_))));
        assert!(!failing.has_modifier);
    }

    #[test]
    fn spawner_identity_world_query_order_and_obstruction_short_circuit() {
        let mut host = Host::new();
        let before = host.level_random.state48();
        let pos = (2, i32::MIN, 4);
        assert_eq!(
            check_mob_spawn_rules(&37, &Reason::Spawner, pos, &(), &mut host),
            Ok(true)
        );
        assert_eq!(host.calls, [Call::Reason(Reason::Spawner)]);
        host.calls.clear();
        let below = (2, i32::MAX, 4);
        assert_eq!(
            check_mob_spawn_rules(&37, &Reason::TrialSpawner, pos, &(), &mut host),
            Ok(false)
        );
        assert_eq!(
            host.calls,
            [
                Call::Reason(Reason::TrialSpawner),
                Call::Block(below),
                Call::ValidSpawn(below, 37)
            ]
        );
        assert_eq!(host.level_random.state48(), before);
        assert!(base_check_spawn_rules(&(), &Reason::Natural));

        host.calls.clear();
        host.liquid = true;
        assert_eq!(check_spawn_obstruction(&mut host), Ok(false));
        assert_eq!(host.calls, [Call::Bounds, Call::Liquid]);
        host.calls.clear();
        host.liquid = false;
        host.unobstructed = false;
        assert_eq!(check_spawn_obstruction(&mut host), Ok(false));
        assert_eq!(host.calls, [Call::Bounds, Call::Liquid, Call::Unobstructed]);
    }

    #[test]
    fn despawn_continues_after_discard_and_consumes_actor_rng_before_near_check() {
        let mut host = Host::new();
        let level_before = host.level_random.state48();
        let mut consumed = host.entity_random.clone();
        assert_eq!(consumed.next_bounded_i32(800), Ok(0));
        check_despawn(&mut host).unwrap();
        assert_eq!(host.discarded, 2);
        assert_eq!(host.entity_random.state48(), consumed.state48());
        assert_eq!(host.level_random.state48(), level_before);
        assert_eq!(
            host.calls,
            [
                Call::Peaceful,
                Call::Persistence,
                Call::CustomPersistence,
                Call::Nearest((-1.0f64).to_bits()),
                Call::PlayerDistance(7),
                Call::DespawnDistance,
                Call::RemoveFar(100_000.0f64.to_bits()),
                Call::Discard,
                Call::NoDespawnDistance,
                Call::ReadNoAction,
                Call::EntityRandom(800),
                Call::RemoveFar(100_000.0f64.to_bits()),
                Call::Discard,
            ]
        );

        let mut nearby = Host::new();
        nearby.distance = 1.0;
        check_despawn(&mut nearby).unwrap();
        assert_eq!(nearby.entity_random.state48(), consumed.state48());
        assert_eq!(
            &nearby.calls[nearby.calls.len() - 2..],
            [Call::EntityRandom(800), Call::NoAction(0)]
        );
        assert_eq!(nearby.no_action, 0);
        assert_eq!(nearby.discarded, 0);

        let mut failing = Host::new();
        failing.fail_no_despawn_distance = true;
        let actor_before = failing.entity_random.state48();
        assert_eq!(
            check_despawn(&mut failing),
            Err("category owner unavailable")
        );
        assert_eq!(failing.discarded, 1);
        assert_eq!(failing.entity_random.state48(), actor_before);
        assert_eq!(failing.calls.last(), Some(&Call::NoDespawnDistance));
    }

    #[test]
    fn despawn_early_exits_strict_thresholds_and_java_integer_squares() {
        let mut peaceful = Host::new();
        peaceful.peaceful = true;
        peaceful.despawn_peaceful = true;
        check_despawn(&mut peaceful).unwrap();
        assert_eq!(
            peaceful.calls,
            [Call::Peaceful, Call::DespawnPeaceful, Call::Discard]
        );

        let mut persistent = Host::new();
        persistent.persistent = true;
        check_despawn(&mut persistent).unwrap();
        assert_eq!(
            persistent.calls,
            [Call::Peaceful, Call::Persistence, Call::NoAction(0)]
        );

        let mut no_player = Host::new();
        no_player.player = None;
        check_despawn(&mut no_player).unwrap();
        assert_eq!(
            no_player.calls,
            [
                Call::Peaceful,
                Call::Persistence,
                Call::CustomPersistence,
                Call::Nearest((-1.0f64).to_bits())
            ]
        );

        let mut boundary = Host::new();
        boundary.distance = 32.0 * 32.0;
        boundary.no_action = 600;
        let actor_before = boundary.entity_random.state48();
        check_despawn(&mut boundary).unwrap();
        assert_eq!(boundary.discarded, 0);
        assert_eq!(boundary.no_action, 600);
        assert_eq!(boundary.entity_random.state48(), actor_before);

        let mut overflow = Host::new();
        overflow.despawn_distance = 50_000;
        overflow.no_despawn_distance = 50_000;
        overflow.distance = 0.0;
        overflow.no_action = 600;
        check_despawn(&mut overflow).unwrap();
        assert_eq!(overflow.discarded, 1); // Java 50_000*50_000 is negative.
        assert_eq!(overflow.no_action, 600);
    }
}
