//! Original Java 1.21 DamageSource/DamageSources object ownership.
//!
//! Source: `brk`27–150, `brl`46–271 and immutable `brm`12–44. A source owns
//! only its original final holder/entity references and immutable Vec3 value.
//! Vec3 coordinates use the existing native geometry type; general Java Vec3
//! reference identity is outside this module's exposed operations. Registry/tag/key,
//! actor, weapon, ability, world and RNG owners are not constructed here.
//! Mandatory hosts retain their canonical objects across removal and ID reuse,
//! release all borrows before returning, and preserve callback error prefixes.
//! Cached environmental sources share an Arc identity; other factories create
//! fresh source objects. Clone a handle, never clone an actor or source state.

use crate::{geometry::Vec3, nbt::JavaString};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DamageScaling {
    Never,
    WhenCausedByLivingNonPlayer,
    Always,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DamageEffects {
    Hurt,
    Thorns,
    Drowning,
    Burning,
    Poking,
    Freezing,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeathMessageType {
    Default,
    FallVariants,
    IntentionalGameDesign,
}

/// The immutable actual DamageType value owned by a canonical holder. No
/// registry ID, tag membership, validation clamp or actor state is inferred.
#[derive(Debug)]
pub struct DamageType {
    message_id: JavaString,
    scaling: DamageScaling,
    exhaustion: f32,
    effects: DamageEffects,
    death_message_type: DeathMessageType,
}
impl DamageType {
    pub fn new(
        message_id: JavaString,
        scaling: DamageScaling,
        exhaustion: f32,
        effects: DamageEffects,
        death_message_type: DeathMessageType,
    ) -> Self {
        Self {
            message_id,
            scaling,
            exhaustion,
            effects,
            death_message_type,
        }
    }
    /// Original constructor31–32.
    pub fn standard(message_id: JavaString, scaling: DamageScaling, exhaustion: f32) -> Self {
        Self::with_effects(message_id, scaling, exhaustion, DamageEffects::Hurt)
    }
    /// Original constructor35–36.
    pub fn with_effects(
        message_id: JavaString,
        scaling: DamageScaling,
        exhaustion: f32,
        effects: DamageEffects,
    ) -> Self {
        Self::new(
            message_id,
            scaling,
            exhaustion,
            effects,
            DeathMessageType::Default,
        )
    }
    /// Original constructors39–44 use WHEN_CAUSED_BY_LIVING_NON_PLAYER.
    pub fn living_scaled(message_id: JavaString, exhaustion: f32, effects: DamageEffects) -> Self {
        Self::with_effects(
            message_id,
            DamageScaling::WhenCausedByLivingNonPlayer,
            exhaustion,
            effects,
        )
    }
    pub fn living_scaled_standard(message_id: JavaString, exhaustion: f32) -> Self {
        Self::living_scaled(message_id, exhaustion, DamageEffects::Hurt)
    }
    pub fn message_id(&self) -> &JavaString {
        &self.message_id
    }
    pub fn scaling(&self) -> DamageScaling {
        self.scaling
    }
    pub fn exhaustion(&self) -> f32 {
        self.exhaustion
    }
    pub fn effects(&self) -> DamageEffects {
        self.effects
    }
    pub fn death_message_type(&self) -> DeathMessageType {
        self.death_message_type
    }
}

/// Original DamageTypes ResourceKey constants. These are key selectors, NEVER
/// numerical registry IDs or independently fabricated Holder objects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DamageTypeKey {
    InFire,
    Campfire,
    LightningBolt,
    OnFire,
    Lava,
    HotFloor,
    InWall,
    Cramming,
    Drown,
    Starve,
    Cactus,
    Fall,
    FlyIntoWall,
    FellOutOfWorld,
    Generic,
    Magic,
    Wither,
    DragonBreath,
    DryOut,
    SweetBerryBush,
    Freeze,
    Stalagmite,
    FallingBlock,
    FallingAnvil,
    FallingStalactite,
    Sting,
    MobAttack,
    MobAttackNoAggro,
    PlayerAttack,
    Arrow,
    Trident,
    MobProjectile,
    Spit,
    WindCharge,
    Fireworks,
    Fireball,
    UnattributedFireball,
    WitherSkull,
    Thrown,
    IndirectMagic,
    Thorns,
    Explosion,
    PlayerExplosion,
    SonicBoom,
    BadRespawnPoint,
    OutsideBorder,
    GenericKill,
}

/// Actual RegistryAccess/Registry/Holder operations. Clone of Holder must retain
/// that SAME holder identity; type_value must retain its current actual value.
/// Tags are queried against its current bindings, not a source-time boolean
/// snapshot. ResourceKey matching preserves the canonical key/registry identity.
/// Unavailable registries, unbound values or missing required keys must fail at
/// their original operation, with no numerical-ID or empty-registry fallback.
pub trait DamageRegistryHost {
    type Error;
    type Registry;
    type Holder: Clone;
    type Tag;
    type ResourceKey;
    /// RegistryAccess.registryOrThrow(Registries.DAMAGE_TYPE), at this Level's
    /// actual DamageSources constructor callsite. Do not allocate another world.
    fn damage_type_registry(&mut self) -> Result<Self::Registry, Self::Error>;
    /// The retained registry's getHolderOrThrow(actual DamageTypes key).
    fn holder_or_throw(
        &mut self,
        registry: &Self::Registry,
        key: DamageTypeKey,
    ) -> Result<Self::Holder, Self::Error>;
    fn type_value(&mut self, holder: &Self::Holder) -> Result<Arc<DamageType>, Self::Error>;
    fn holder_is_tag(
        &mut self,
        holder: &Self::Holder,
        tag: &Self::Tag,
    ) -> Result<bool, Self::Error>;
    fn holder_is_key(
        &mut self,
        holder: &Self::Holder,
        key: &Self::ResourceKey,
    ) -> Result<bool, Self::Error>;
}

pub trait DamageEntityHost: DamageRegistryHost {
    /// A retained canonical Entity reference, not a current-registry ID lookup.
    /// Clone retains the SAME body even after removal, respawn or ID reuse.
    type Entity: Clone;
    type Weapon;
    /// Pure actual object-identity comparison. Equal IDs/UUIDs do not suffice.
    fn same_entity(
        &mut self,
        first: &Self::Entity,
        second: &Self::Entity,
    ) -> Result<bool, Self::Error>;
    fn entity_is_living(&mut self, entity: &Self::Entity) -> Result<bool, Self::Error>;
    fn entity_is_player(&mut self, entity: &Self::Entity) -> Result<bool, Self::Error>;
    /// Actual Player.getAbilities().instabuild, never invulnerable or gamemode.
    fn player_instabuild(&mut self, player: &Self::Entity) -> Result<bool, Self::Error>;
    fn entity_position(&mut self, entity: &Self::Entity) -> Result<Vec3, Self::Error>;
    /// Actual virtual Entity.getWeaponItem. Some(EMPTY) differs from Java null.
    fn entity_weapon_item(
        &mut self,
        entity: &Self::Entity,
    ) -> Result<Option<Self::Weapon>, Self::Error>;
}

/// Original four final source fields. No Clone/PartialEq implementation can
/// accidentally replace source object identity with a copied record or type ID.
#[derive(Debug)]
pub struct DamageSource<H, E> {
    type_holder: H,
    causing_entity: Option<E>,
    direct_entity: Option<E>,
    raw_position: Option<Vec3>,
}
impl<H, E> DamageSource<H, E> {
    fn create(
        type_holder: H,
        direct_entity: Option<E>,
        causing_entity: Option<E>,
        raw_position: Option<Vec3>,
    ) -> Arc<Self> {
        Arc::new(Self {
            type_holder,
            causing_entity,
            direct_entity,
            raw_position,
        })
    }
    /// Original holder-only constructor58–59; isDirect is true (null==null).
    pub fn unattributed(type_holder: H) -> Arc<Self> {
        Self::create(type_holder, None, None, None)
    }
    /// Original constructor46–47. Argument order is DIRECT, then CAUSING.
    pub fn from_entities(type_holder: H, direct: Option<E>, causing: Option<E>) -> Arc<Self> {
        Self::create(type_holder, direct, causing, None)
    }
    /// Original explicit-position constructor50–51; both entities are null.
    pub fn at_position(type_holder: H, position: Option<Vec3>) -> Arc<Self> {
        Self::create(type_holder, None, None, position)
    }
    pub fn type_holder(&self) -> &H {
        &self.type_holder
    }
    pub fn direct_entity(&self) -> Option<&E> {
        self.direct_entity.as_ref()
    }
    pub fn causing_entity(&self) -> Option<&E> {
        self.causing_entity.as_ref()
    }
    /// sourcePositionRaw134. Never derives a position from either entity.
    pub fn raw_position(&self) -> Option<Vec3> {
        self.raw_position
    }
    pub fn damage_type<X: DamageRegistryHost<Holder = H>>(
        &self,
        host: &mut X,
    ) -> Result<Arc<DamageType>, X::Error> {
        host.type_value(&self.type_holder)
    }
    pub fn food_exhaustion<X: DamageRegistryHost<Holder = H>>(
        &self,
        host: &mut X,
    ) -> Result<f32, X::Error> {
        Ok(self.damage_type(host)?.exhaustion())
    }
    pub fn message_id<X: DamageRegistryHost<Holder = H>>(
        &self,
        host: &mut X,
    ) -> Result<JavaString, X::Error> {
        Ok(self.damage_type(host)?.message_id().clone())
    }
    pub fn is_tag<X: DamageRegistryHost<Holder = H>>(
        &self,
        tag: &X::Tag,
        host: &mut X,
    ) -> Result<bool, X::Error> {
        host.holder_is_tag(&self.type_holder, tag)
    }
    pub fn is_key<X: DamageRegistryHost<Holder = H>>(
        &self,
        key: &X::ResourceKey,
        host: &mut X,
    ) -> Result<bool, X::Error> {
        host.holder_is_key(&self.type_holder, key)
    }
    pub fn is_direct<X: DamageEntityHost<Holder = H, Entity = E>>(
        &self,
        host: &mut X,
    ) -> Result<bool, X::Error> {
        match (&self.causing_entity, &self.direct_entity) {
            (None, None) => Ok(true),
            (Some(causing), Some(direct)) => host.same_entity(causing, direct),
            _ => Ok(false),
        }
    }
    pub fn weapon_item<X: DamageEntityHost<Holder = H, Entity = E>>(
        &self,
        host: &mut X,
    ) -> Result<Option<X::Weapon>, X::Error> {
        match self.direct_entity.as_ref() {
            Some(entity) => host.entity_weapon_item(entity),
            None => Ok(None),
        }
    }
    /// Source111–114: scaling uses the CAUSING entity, without querying world
    /// difficulty or substituting the direct projectile/its current owner.
    pub fn scales_with_difficulty<X: DamageEntityHost<Holder = H, Entity = E>>(
        &self,
        host: &mut X,
    ) -> Result<bool, X::Error> {
        match self.damage_type(host)?.scaling() {
            DamageScaling::Never => Ok(false),
            DamageScaling::Always => Ok(true),
            DamageScaling::WhenCausedByLivingNonPlayer => match self.causing_entity.as_ref() {
                Some(entity) => {
                    Ok(host.entity_is_living(entity)? && !host.entity_is_player(entity)?)
                }
                None => Ok(false),
            },
        }
    }
    pub fn is_creative_player<X: DamageEntityHost<Holder = H, Entity = E>>(
        &self,
        host: &mut X,
    ) -> Result<bool, X::Error> {
        match self.causing_entity.as_ref() {
            Some(entity) if host.entity_is_player(entity)? => host.player_instabuild(entity),
            _ => Ok(false),
        }
    }
    /// Source124–129: raw explicit position, otherwise CURRENT direct entity
    /// position. Missing actual owner errors; removed entities are not retargeted.
    pub fn source_position<X: DamageEntityHost<Holder = H, Entity = E>>(
        &self,
        host: &mut X,
    ) -> Result<Option<Vec3>, X::Error> {
        match (self.raw_position, self.direct_entity.as_ref()) {
            (Some(position), _) => Ok(Some(position)),
            (None, Some(entity)) => Ok(Some(host.entity_position(entity)?)),
            (None, None) => Ok(None),
        }
    }
}
impl<H, E: Clone> DamageSource<H, E> {
    /// Original single-entity constructor54–55 stores the SAME entity twice.
    pub fn from_entity(type_holder: H, entity: Option<E>) -> Arc<Self> {
        Self::from_entities(type_holder, entity.clone(), entity)
    }
}

/// Source cache positions only, not protocol registry IDs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvironmentalSource {
    InFire,
    Campfire,
    LightningBolt,
    OnFire,
    Lava,
    HotFloor,
    InWall,
    Cramming,
    Drown,
    Starve,
    Cactus,
    Fall,
    FlyIntoWall,
    FellOutOfWorld,
    Generic,
    Magic,
    Wither,
    DragonBreath,
    DryOut,
    SweetBerryBush,
    Freeze,
    Stalagmite,
    OutsideBorder,
    GenericKill,
}
impl EnvironmentalSource {
    pub const SOURCE_ORDER: [Self; 24] = [
        Self::InFire,
        Self::Campfire,
        Self::LightningBolt,
        Self::OnFire,
        Self::Lava,
        Self::HotFloor,
        Self::InWall,
        Self::Cramming,
        Self::Drown,
        Self::Starve,
        Self::Cactus,
        Self::Fall,
        Self::FlyIntoWall,
        Self::FellOutOfWorld,
        Self::Generic,
        Self::Magic,
        Self::Wither,
        Self::DragonBreath,
        Self::DryOut,
        Self::SweetBerryBush,
        Self::Freeze,
        Self::Stalagmite,
        Self::OutsideBorder,
        Self::GenericKill,
    ];
    pub fn key(self) -> DamageTypeKey {
        match self {
            Self::InFire => DamageTypeKey::InFire,
            Self::Campfire => DamageTypeKey::Campfire,
            Self::LightningBolt => DamageTypeKey::LightningBolt,
            Self::OnFire => DamageTypeKey::OnFire,
            Self::Lava => DamageTypeKey::Lava,
            Self::HotFloor => DamageTypeKey::HotFloor,
            Self::InWall => DamageTypeKey::InWall,
            Self::Cramming => DamageTypeKey::Cramming,
            Self::Drown => DamageTypeKey::Drown,
            Self::Starve => DamageTypeKey::Starve,
            Self::Cactus => DamageTypeKey::Cactus,
            Self::Fall => DamageTypeKey::Fall,
            Self::FlyIntoWall => DamageTypeKey::FlyIntoWall,
            Self::FellOutOfWorld => DamageTypeKey::FellOutOfWorld,
            Self::Generic => DamageTypeKey::Generic,
            Self::Magic => DamageTypeKey::Magic,
            Self::Wither => DamageTypeKey::Wither,
            Self::DragonBreath => DamageTypeKey::DragonBreath,
            Self::DryOut => DamageTypeKey::DryOut,
            Self::SweetBerryBush => DamageTypeKey::SweetBerryBush,
            Self::Freeze => DamageTypeKey::Freeze,
            Self::Stalagmite => DamageTypeKey::Stalagmite,
            Self::OutsideBorder => DamageTypeKey::OutsideBorder,
            Self::GenericKill => DamageTypeKey::GenericKill,
        }
    }
}

/// Attach ONE actual instance to its original Level. No Level, actor index,
/// registry or RNG is manufactured. The registry handle survives later current-
/// world changes; actual Entity.damageSources must select its current Level.
/// Specialized Java parameter types (LivingEntity, Player, AbstractArrow, etc.)
/// are erased to the retained Entity handle. Callers uphold their original
/// typed preconditions; no extra class checks are inserted. Nullable arguments
/// remain actual null references, not fabricated entities.
pub struct DamageSources<R, H, E> {
    registry: R,
    cached: [Arc<DamageSource<H, E>>; 24],
}
macro_rules! environmental_getters {
    ($($name:ident => $kind:ident),* $(,)?) => { $(
        pub fn $name(&self) -> Arc<DamageSource<H, E>> { self.environmental(EnvironmentalSource::$kind) }
    )* };
}
macro_rules! single_factories {
    ($($name:ident => $key:ident),* $(,)?) => { $(
        pub fn $name<X: DamageRegistryHost<Registry = R, Holder = H>>(&self, entity: Option<E>, host: &mut X) -> Result<Arc<DamageSource<H, E>>, X::Error> {
            self.single(DamageTypeKey::$key, entity, host)
        }
    )* };
}
macro_rules! paired_factories {
    ($($name:ident => $key:ident),* $(,)?) => { $(
        pub fn $name<X: DamageRegistryHost<Registry = R, Holder = H>>(&self, direct: Option<E>, causing: Option<E>, host: &mut X) -> Result<Arc<DamageSource<H, E>>, X::Error> {
            self.paired(DamageTypeKey::$key, direct, causing, host)
        }
    )* };
}
impl<R, H, E: Clone> DamageSources<R, H, E> {
    /// Original constructor46–72. Required key lookups execute in source order.
    /// A failure retains its actual host prefix; no partial cache is published.
    pub fn new<X: DamageRegistryHost<Registry = R, Holder = H>>(
        host: &mut X,
    ) -> Result<Self, X::Error> {
        let registry = host.damage_type_registry()?;
        let mut cached = Vec::with_capacity(24);
        for kind in EnvironmentalSource::SOURCE_ORDER {
            let holder = host.holder_or_throw(&registry, kind.key())?;
            cached.push(DamageSource::unattributed(holder));
        }
        let cached = cached
            .try_into()
            .unwrap_or_else(|_| unreachable!("fixed source cache length"));
        Ok(Self { registry, cached })
    }
    pub fn registry(&self) -> &R {
        &self.registry
    }
    pub fn environmental(&self, source: EnvironmentalSource) -> Arc<DamageSource<H, E>> {
        Arc::clone(&self.cached[source as usize])
    }
    environmental_getters! {
        in_fire => InFire, campfire => Campfire, lightning_bolt => LightningBolt,
        on_fire => OnFire, lava => Lava, hot_floor => HotFloor, in_wall => InWall,
        cramming => Cramming, drown => Drown, starve => Starve, cactus => Cactus,
        fall => Fall, fly_into_wall => FlyIntoWall, fell_out_of_world => FellOutOfWorld,
        generic => Generic, magic => Magic, wither => Wither, dragon_breath => DragonBreath,
        dry_out => DryOut, sweet_berry_bush => SweetBerryBush, freeze => Freeze,
        stalagmite => Stalagmite, out_of_border => OutsideBorder, generic_kill => GenericKill,
    }
    fn single<X: DamageRegistryHost<Registry = R, Holder = H>>(
        &self,
        key: DamageTypeKey,
        entity: Option<E>,
        host: &mut X,
    ) -> Result<Arc<DamageSource<H, E>>, X::Error> {
        let holder = host.holder_or_throw(&self.registry, key)?;
        Ok(DamageSource::from_entity(holder, entity))
    }
    fn paired<X: DamageRegistryHost<Registry = R, Holder = H>>(
        &self,
        key: DamageTypeKey,
        direct: Option<E>,
        causing: Option<E>,
        host: &mut X,
    ) -> Result<Arc<DamageSource<H, E>>, X::Error> {
        let holder = host.holder_or_throw(&self.registry, key)?;
        Ok(DamageSource::from_entities(holder, direct, causing))
    }
    single_factories! {
        falling_block => FallingBlock, anvil => FallingAnvil,
        falling_stalactite => FallingStalactite, sting => Sting,
        mob_attack => MobAttack, no_aggro_mob_attack => MobAttackNoAggro,
        player_attack => PlayerAttack, thorns => Thorns, sonic_boom => SonicBoom,
    }
    paired_factories! {
        arrow => Arrow, trident => Trident, mob_projectile => MobProjectile,
        spit => Spit, wind_charge => WindCharge, fireworks => Fireworks,
        wither_skull => WitherSkull, thrown => Thrown, indirect_magic => IndirectMagic,
    }
    /// Source228–231: no owner chooses the SINGLE-entity unattributed-fireball
    /// form, making the fireball itself both direct and causing.
    pub fn fireball<X: DamageRegistryHost<Registry = R, Holder = H>>(
        &self,
        fireball: Option<E>,
        causing: Option<E>,
        host: &mut X,
    ) -> Result<Arc<DamageSource<H, E>>, X::Error> {
        if causing.is_none() {
            self.single(DamageTypeKey::UnattributedFireball, fireball, host)
        } else {
            self.paired(DamageTypeKey::Fireball, fireball, causing, host)
        }
    }
    /// Source255: PLAYER_EXPLOSION iff BOTH references are nonnull. There is no
    /// causing-entity instanceof Player test in this original method.
    pub fn explosion<X: DamageRegistryHost<Registry = R, Holder = H>>(
        &self,
        direct: Option<E>,
        causing: Option<E>,
        host: &mut X,
    ) -> Result<Arc<DamageSource<H, E>>, X::Error> {
        let key = if direct.is_some() && causing.is_some() {
            DamageTypeKey::PlayerExplosion
        } else {
            DamageTypeKey::Explosion
        };
        self.paired(key, direct, causing, host)
    }
    /// Actual Explosion.getDirectSourceEntity first, getIndirectSourceEntity
    /// second, then the source factory; null explosion skips both callbacks.
    pub fn from_explosion<X: ExplosionSourceHost<Registry = R, Holder = H, Entity = E>>(
        &self,
        explosion: Option<&X::Explosion>,
        host: &mut X,
    ) -> Result<Arc<DamageSource<H, E>>, X::Error> {
        match explosion {
            Some(explosion) => {
                let direct = host.explosion_direct_entity(explosion)?;
                let causing = host.explosion_indirect_entity(explosion)?;
                self.explosion(direct, causing, host)
            }
            None => self.explosion(None, None, host),
        }
    }
    pub fn bad_respawn_point_explosion<X: DamageRegistryHost<Registry = R, Holder = H>>(
        &self,
        position: Option<Vec3>,
        host: &mut X,
    ) -> Result<Arc<DamageSource<H, E>>, X::Error> {
        let holder = host.holder_or_throw(&self.registry, DamageTypeKey::BadRespawnPoint)?;
        Ok(DamageSource::at_position(holder, position))
    }
}

pub trait ExplosionSourceHost: DamageEntityHost {
    type Explosion;
    fn explosion_direct_entity(
        &mut self,
        explosion: &Self::Explosion,
    ) -> Result<Option<Self::Entity>, Self::Error>;
    fn explosion_indirect_entity(
        &mut self,
        explosion: &Self::Explosion,
    ) -> Result<Option<Self::Entity>, Self::Error>;
}

/// Immutable source fields captured by original DamageEventPacket27–34. This
/// is not a wire encoder, registry-ID guess, audience selection or publication.
#[derive(Debug)]
pub struct DamageEventSnapshot<H> {
    pub entity_id: i32,
    pub source_type: H,
    pub source_cause_id: i32,
    pub source_direct_id: i32,
    pub source_position: Option<Vec3>,
}
pub trait DamageEventSnapshotHost: DamageEntityHost {
    /// Read this exact retained actor's CURRENT numeric ID at packet capture.
    fn entity_id(&mut self, entity: &Self::Entity) -> Result<i32, Self::Error>;
}
pub fn damage_event_snapshot<X: DamageEventSnapshotHost>(
    victim: &X::Entity,
    source: &DamageSource<X::Holder, X::Entity>,
    host: &mut X,
) -> Result<DamageEventSnapshot<X::Holder>, X::Error> {
    let entity_id = host.entity_id(victim)?;
    let source_type = source.type_holder().clone();
    let source_cause_id = match source.causing_entity() {
        Some(entity) => host.entity_id(entity)?,
        None => -1,
    };
    let source_direct_id = match source.direct_entity() {
        Some(entity) => host.entity_id(entity)?,
        None => -1,
    };
    Ok(DamageEventSnapshot {
        entity_id,
        source_type,
        source_cause_id,
        source_direct_id,
        source_position: source.raw_position(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::{Cell, RefCell},
        collections::{BTreeMap, BTreeSet},
        rc::Rc,
    };

    // Controlled registry/entity fixtures, never a production registry/actor.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Class {
        Other,
        Living,
        Player,
    }
    struct Entity {
        id: i32,
        class: Class,
        instabuild: bool,
        position: Vec3,
        weapon: Option<Rc<Cell<i32>>>,
        removed: bool,
    }
    type Actor = Rc<RefCell<Entity>>;
    #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
    struct Tag {
        registry: u32,
        name: &'static str,
    }
    #[derive(Debug)]
    struct Key {
        registry: u32,
        kind: DamageTypeKey,
    }
    struct HolderState {
        key: Rc<Key>,
        value: Option<Arc<DamageType>>,
        tags: BTreeSet<Tag>,
    }
    type Holder = Rc<RefCell<HolderState>>;
    struct RegistryState {
        id: u32,
        holders: BTreeMap<DamageTypeKey, Holder>,
    }
    type Registry = Rc<RefCell<RegistryState>>;
    type Sources = DamageSources<Registry, Holder, Actor>;
    struct Explosion {
        direct: Option<Actor>,
        causing: Option<Actor>,
    }
    type Blast = Rc<RefCell<Explosion>>;
    struct Host {
        registry: Registry,
        registries: Vec<Registry>,
        actors: Vec<Actor>,
        calls: Vec<String>,
        fail: Option<&'static str>,
        change_indirect: Option<Actor>,
    }
    fn all_keys() -> Vec<DamageTypeKey> {
        let mut keys: Vec<_> = EnvironmentalSource::SOURCE_ORDER
            .into_iter()
            .map(EnvironmentalSource::key)
            .collect();
        keys.extend([
            DamageTypeKey::FallingBlock,
            DamageTypeKey::FallingAnvil,
            DamageTypeKey::FallingStalactite,
            DamageTypeKey::Sting,
            DamageTypeKey::MobAttack,
            DamageTypeKey::MobAttackNoAggro,
            DamageTypeKey::PlayerAttack,
            DamageTypeKey::Arrow,
            DamageTypeKey::Trident,
            DamageTypeKey::MobProjectile,
            DamageTypeKey::Spit,
            DamageTypeKey::WindCharge,
            DamageTypeKey::Fireworks,
            DamageTypeKey::Fireball,
            DamageTypeKey::UnattributedFireball,
            DamageTypeKey::WitherSkull,
            DamageTypeKey::Thrown,
            DamageTypeKey::IndirectMagic,
            DamageTypeKey::Thorns,
            DamageTypeKey::Explosion,
            DamageTypeKey::PlayerExplosion,
            DamageTypeKey::SonicBoom,
            DamageTypeKey::BadRespawnPoint,
        ]);
        keys
    }
    impl Host {
        fn make_registry(id: u32) -> Registry {
            let holders = all_keys()
                .into_iter()
                .map(|kind| {
                    let value = Arc::new(DamageType::living_scaled_standard(
                        format!("{kind:?}").into(),
                        0.1,
                    ));
                    (
                        kind,
                        Rc::new(RefCell::new(HolderState {
                            key: Rc::new(Key { registry: id, kind }),
                            value: Some(value),
                            tags: BTreeSet::new(),
                        })),
                    )
                })
                .collect();
            Rc::new(RefCell::new(RegistryState { id, holders }))
        }
        fn new() -> Self {
            let registry = Self::make_registry(1);
            Self {
                registry: registry.clone(),
                registries: vec![registry],
                actors: vec![],
                calls: vec![],
                fail: None,
                change_indirect: None,
            }
        }
        fn actor(&mut self, id: i32, class: Class) -> Actor {
            let entity = Rc::new(RefCell::new(Entity {
                id,
                class,
                instabuild: false,
                position: Vec3::new(f64::from(id), 2., 3.),
                weapon: None,
                removed: false,
            }));
            self.actors.push(entity.clone());
            entity
        }
        fn mark(&mut self, call: impl Into<String>) -> Result<(), &'static str> {
            for actor in &self.actors {
                assert!(
                    actor.try_borrow_mut().is_ok(),
                    "actor borrow crosses callback"
                );
            }
            for registry in &self.registries {
                assert!(
                    registry.try_borrow_mut().is_ok(),
                    "registry borrow crosses callback"
                );
                let holders: Vec<_> = registry.borrow().holders.values().cloned().collect();
                for holder in holders {
                    assert!(
                        holder.try_borrow_mut().is_ok(),
                        "holder borrow crosses callback"
                    );
                }
            }
            let call = call.into();
            self.calls.push(call.clone());
            if self.fail == Some(call.as_str()) {
                Err("owner failure")
            } else {
                Ok(())
            }
        }
    }
    impl DamageRegistryHost for Host {
        type Error = &'static str;
        type Registry = Registry;
        type Holder = Holder;
        type Tag = Tag;
        type ResourceKey = Rc<Key>;
        fn damage_type_registry(&mut self) -> Result<Registry, Self::Error> {
            self.mark("registry")?;
            Ok(self.registry.clone())
        }
        fn holder_or_throw(
            &mut self,
            r: &Registry,
            key: DamageTypeKey,
        ) -> Result<Holder, Self::Error> {
            let id = r.borrow().id;
            self.mark(format!("lookup:{id}:{key:?}"))?;
            r.borrow()
                .holders
                .get(&key)
                .cloned()
                .ok_or("missing holder")
        }
        fn type_value(&mut self, h: &Holder) -> Result<Arc<DamageType>, Self::Error> {
            self.mark("value")?;
            h.borrow().value.clone().ok_or("unbound value")
        }
        fn holder_is_tag(&mut self, h: &Holder, tag: &Tag) -> Result<bool, Self::Error> {
            self.mark("tag")?;
            Ok(h.borrow().tags.contains(tag))
        }
        fn holder_is_key(&mut self, h: &Holder, key: &Rc<Key>) -> Result<bool, Self::Error> {
            self.mark("key")?;
            Ok(Rc::ptr_eq(&h.borrow().key, key))
        }
    }
    impl DamageEntityHost for Host {
        type Entity = Actor;
        type Weapon = Rc<Cell<i32>>;
        fn same_entity(&mut self, a: &Actor, b: &Actor) -> Result<bool, Self::Error> {
            self.mark("identity")?;
            Ok(Rc::ptr_eq(a, b))
        }
        fn entity_is_living(&mut self, a: &Actor) -> Result<bool, Self::Error> {
            self.mark("living")?;
            Ok(a.borrow().class != Class::Other)
        }
        fn entity_is_player(&mut self, a: &Actor) -> Result<bool, Self::Error> {
            self.mark("player")?;
            Ok(a.borrow().class == Class::Player)
        }
        fn player_instabuild(&mut self, a: &Actor) -> Result<bool, Self::Error> {
            self.mark("instabuild")?;
            Ok(a.borrow().instabuild)
        }
        fn entity_position(&mut self, a: &Actor) -> Result<Vec3, Self::Error> {
            self.mark("position")?;
            Ok(a.borrow().position)
        }
        fn entity_weapon_item(&mut self, a: &Actor) -> Result<Option<Self::Weapon>, Self::Error> {
            self.mark("weapon")?;
            Ok(a.borrow().weapon.clone())
        }
    }
    impl ExplosionSourceHost for Host {
        type Explosion = Blast;
        fn explosion_direct_entity(&mut self, e: &Blast) -> Result<Option<Actor>, Self::Error> {
            self.mark("explosion_direct")?;
            let direct = e.borrow().direct.clone();
            if let Some(actor) = self.change_indirect.take() {
                e.borrow_mut().causing = Some(actor);
            }
            Ok(direct)
        }
        fn explosion_indirect_entity(&mut self, e: &Blast) -> Result<Option<Actor>, Self::Error> {
            self.mark("explosion_indirect")?;
            Ok(e.borrow().causing.clone())
        }
    }
    impl DamageEventSnapshotHost for Host {
        fn entity_id(&mut self, a: &Actor) -> Result<i32, Self::Error> {
            let id = a.borrow().id;
            self.mark(format!("id:{id}"))?;
            Ok(id)
        }
    }

    #[test]
    fn constructor_cache_order_identity_and_registry_retention() {
        let mut host = Host::new();
        let sources = Sources::new(&mut host).unwrap();
        let expected: Vec<_> = std::iter::once("registry".into())
            .chain(
                EnvironmentalSource::SOURCE_ORDER
                    .into_iter()
                    .map(|s| format!("lookup:1:{:?}", s.key())),
            )
            .collect();
        assert_eq!(host.calls, expected);
        host.calls.clear();
        let first = sources.in_fire();
        let second = sources.in_fire();
        assert!(Arc::ptr_eq(&first, &second));
        assert!(!Arc::ptr_eq(&first, &sources.on_fire()));
        assert!(first.is_direct(&mut host).unwrap());
        assert!(host.calls.is_empty());
        assert!(first.causing_entity().is_none() && first.direct_entity().is_none());
        let old = host.registry.clone();
        let next = Host::make_registry(2);
        host.registries.push(next.clone());
        host.registry = next;
        let a = sources.mob_attack(None, &mut host).unwrap();
        let b = sources.mob_attack(None, &mut host).unwrap();
        assert!(!Arc::ptr_eq(&a, &b));
        assert!(Rc::ptr_eq(sources.registry(), &old));
        assert_eq!(host.calls, ["lookup:1:MobAttack", "lookup:1:MobAttack"]);
        assert_eq!(a.type_holder().borrow().key.registry, 1);
    }

    #[test]
    fn source_entity_identity_dynamic_direct_queries_and_null_distinction() {
        let mut host = Host::new();
        let sources = Sources::new(&mut host).unwrap();
        let direct = host.actor(7, Class::Other);
        let causer = host.actor(7, Class::Living);
        let direct_weapon = Rc::new(Cell::new(0)); // Actual nonnull EMPTY result.
        direct.borrow_mut().weapon = Some(direct_weapon.clone());
        causer.borrow_mut().weapon = Some(Rc::new(Cell::new(99)));
        let source = sources
            .arrow(Some(direct.clone()), Some(causer.clone()), &mut host)
            .unwrap();
        assert!(!source.is_direct(&mut host).unwrap()); // equal wire IDs, different objects
        assert!(Rc::ptr_eq(source.causing_entity().unwrap(), &causer));
        assert!(Rc::ptr_eq(source.direct_entity().unwrap(), &direct));
        assert!(Rc::ptr_eq(
            &source.weapon_item(&mut host).unwrap().unwrap(),
            &direct_weapon
        ));
        assert_eq!(
            source.source_position(&mut host).unwrap(),
            Some(Vec3::new(7., 2., 3.))
        );
        {
            let mut actor = direct.borrow_mut();
            actor.removed = true;
            actor.id = 20;
            actor.position = Vec3::new(8., 9., 10.);
        }
        let replacement = host.actor(20, Class::Other);
        replacement.borrow_mut().position = Vec3::new(90., 90., 90.);
        assert_eq!(
            source.source_position(&mut host).unwrap(),
            Some(Vec3::new(8., 9., 10.))
        );
        assert!(source.raw_position().is_none());
        let same = sources.mob_attack(Some(causer.clone()), &mut host).unwrap();
        assert!(same.is_direct(&mut host).unwrap());
        let one_null = sources.arrow(None, Some(causer), &mut host).unwrap();
        assert!(!one_null.is_direct(&mut host).unwrap());
        assert!(one_null.weapon_item(&mut host).unwrap().is_none());
        assert!(one_null.source_position(&mut host).unwrap().is_none());
        let raw = Vec3::new(-0., f64::NAN, 2.);
        let at = sources
            .bad_respawn_point_explosion(Some(raw), &mut host)
            .unwrap();
        host.calls.clear();
        let position = at.source_position(&mut host).unwrap().unwrap();
        assert_eq!(position.x.to_bits(), (-0_f64).to_bits());
        assert!(position.y.is_nan());
        assert!(at.is_direct(&mut host).unwrap());
        assert!(host.calls.is_empty());
        assert!(at.causing_entity().is_none() && at.direct_entity().is_none());
    }

    #[test]
    fn holder_queries_observe_reload_key_identity_and_raw_type_values() {
        let mut host = Host::new();
        let sources = Sources::new(&mut host).unwrap();
        let source = sources.freeze();
        let holder = source.type_holder().clone();
        let tag = Tag {
            registry: 1,
            name: "is_freezing",
        };
        assert!(!source.is_tag(&tag, &mut host).unwrap());
        holder.borrow_mut().tags.insert(tag.clone());
        assert!(source.is_tag(&tag, &mut host).unwrap());
        assert!(!source
            .is_tag(
                &Tag {
                    registry: 2,
                    name: "is_freezing"
                },
                &mut host
            )
            .unwrap());
        holder.borrow_mut().tags.clear();
        assert!(!source.is_tag(&tag, &mut host).unwrap());
        let original_key = holder.borrow().key.clone();
        let equal_name_other_object = Rc::new(Key {
            registry: original_key.registry,
            kind: original_key.kind,
        });
        assert!(source.is_key(&original_key, &mut host).unwrap());
        assert!(!source.is_key(&equal_name_other_object, &mut host).unwrap());
        let message = JavaString::from_utf16_lossless(vec![0xd800]);
        let value = Arc::new(DamageType::new(
            message,
            DamageScaling::Always,
            -0.,
            DamageEffects::Freezing,
            DeathMessageType::FallVariants,
        ));
        holder.borrow_mut().value = Some(value.clone());
        assert!(Arc::ptr_eq(&source.damage_type(&mut host).unwrap(), &value));
        assert_eq!(
            source.food_exhaustion(&mut host).unwrap().to_bits(),
            (-0_f32).to_bits()
        );
        assert_eq!(
            source
                .message_id(&mut host)
                .unwrap()
                .iter_units()
                .collect::<Vec<_>>(),
            [0xd800]
        );
        assert_eq!(
            source.damage_type(&mut host).unwrap().effects(),
            DamageEffects::Freezing
        );
        assert_eq!(
            source.damage_type(&mut host).unwrap().death_message_type(),
            DeathMessageType::FallVariants
        );
        host.calls.clear();
        assert!(source.scales_with_difficulty(&mut host).unwrap());
        assert_eq!(host.calls, ["value"]);
        holder.borrow_mut().value = None;
        assert_eq!(source.food_exhaustion(&mut host), Err("unbound value"));
        assert!(Arc::ptr_eq(&source, &sources.freeze()));
    }

    #[test]
    fn scaling_and_creative_queries_use_causer_and_instabuild() {
        let mut host = Host::new();
        let sources = Sources::new(&mut host).unwrap();
        let direct_player = host.actor(1, Class::Player);
        direct_player.borrow_mut().instabuild = true;
        let mob = host.actor(2, Class::Living);
        let player = host.actor(3, Class::Player);
        let nonliving = host.actor(4, Class::Other);
        let source = sources
            .arrow(Some(direct_player.clone()), Some(mob), &mut host)
            .unwrap();
        host.calls.clear();
        assert!(source.scales_with_difficulty(&mut host).unwrap());
        assert_eq!(host.calls, ["value", "living", "player"]);
        assert!(!source.is_creative_player(&mut host).unwrap());
        let source = sources
            .arrow(Some(nonliving.clone()), Some(player.clone()), &mut host)
            .unwrap();
        assert!(!source.scales_with_difficulty(&mut host).unwrap());
        assert!(!source.is_creative_player(&mut host).unwrap());
        player.borrow_mut().instabuild = true;
        assert!(source.is_creative_player(&mut host).unwrap());
        let source = sources.arrow(None, Some(nonliving), &mut host).unwrap();
        host.calls.clear();
        assert!(!source.scales_with_difficulty(&mut host).unwrap());
        assert_eq!(host.calls, ["value", "living"]);
        let source = sources.arrow(Some(direct_player), None, &mut host).unwrap();
        host.calls.clear();
        assert!(!source.scales_with_difficulty(&mut host).unwrap());
        assert!(!source.is_creative_player(&mut host).unwrap());
        assert_eq!(host.calls, ["value"]);
        let value = DamageType::living_scaled_standard("raw".into(), f32::from_bits(0x7fc01234));
        assert_eq!(value.exhaustion().to_bits(), 0x7fc01234);
        assert_eq!(
            (value.effects(), value.death_message_type()),
            (DamageEffects::Hurt, DeathMessageType::Default)
        );
    }

    #[test]
    fn factory_key_mapping_nullable_fireball_and_explosion_capture_order() {
        let mut host = Host::new();
        let sources = Sources::new(&mut host).unwrap();
        let direct = host.actor(5, Class::Other);
        let causing = host.actor(6, Class::Living);
        macro_rules! single {
            ($method:ident, $key:ident) => {{
                let source = sources.$method(Some(direct.clone()), &mut host).unwrap();
                assert_eq!(source.type_holder().borrow().key.kind, DamageTypeKey::$key);
                assert!(source.is_direct(&mut host).unwrap());
            }};
        }
        single!(falling_block, FallingBlock);
        single!(anvil, FallingAnvil);
        single!(falling_stalactite, FallingStalactite);
        single!(sting, Sting);
        single!(mob_attack, MobAttack);
        single!(no_aggro_mob_attack, MobAttackNoAggro);
        single!(player_attack, PlayerAttack);
        single!(thorns, Thorns);
        single!(sonic_boom, SonicBoom);
        macro_rules! paired {
            ($method:ident, $key:ident) => {{
                let source = sources
                    .$method(Some(direct.clone()), Some(causing.clone()), &mut host)
                    .unwrap();
                assert_eq!(source.type_holder().borrow().key.kind, DamageTypeKey::$key);
                assert!(Rc::ptr_eq(source.direct_entity().unwrap(), &direct));
                assert!(Rc::ptr_eq(source.causing_entity().unwrap(), &causing));
            }};
        }
        paired!(arrow, Arrow);
        paired!(trident, Trident);
        paired!(mob_projectile, MobProjectile);
        paired!(spit, Spit);
        paired!(wind_charge, WindCharge);
        paired!(fireworks, Fireworks);
        paired!(wither_skull, WitherSkull);
        paired!(thrown, Thrown);
        paired!(indirect_magic, IndirectMagic);
        let source = sources
            .fireball(Some(direct.clone()), None, &mut host)
            .unwrap();
        assert_eq!(
            source.type_holder().borrow().key.kind,
            DamageTypeKey::UnattributedFireball
        );
        assert!(source.is_direct(&mut host).unwrap());
        assert!(Rc::ptr_eq(source.causing_entity().unwrap(), &direct));
        let source = sources
            .fireball(Some(direct.clone()), Some(causing.clone()), &mut host)
            .unwrap();
        assert_eq!(
            source.type_holder().borrow().key.kind,
            DamageTypeKey::Fireball
        );
        let source = sources.fireball(None, None, &mut host).unwrap();
        assert!(source.is_direct(&mut host).unwrap());
        assert!(source.causing_entity().is_none());
        let blast = Rc::new(RefCell::new(Explosion {
            direct: Some(direct.clone()),
            causing: None,
        }));
        host.change_indirect = Some(causing.clone());
        host.calls.clear();
        let source = sources.from_explosion(Some(&blast), &mut host).unwrap();
        assert_eq!(
            host.calls,
            [
                "explosion_direct",
                "explosion_indirect",
                "lookup:1:PlayerExplosion"
            ]
        );
        assert!(Rc::ptr_eq(source.direct_entity().unwrap(), &direct));
        assert!(Rc::ptr_eq(source.causing_entity().unwrap(), &causing)); // Living, NOT Player
        let source = sources.explosion(None, Some(causing), &mut host).unwrap();
        assert_eq!(
            source.type_holder().borrow().key.kind,
            DamageTypeKey::Explosion
        );
        host.calls.clear();
        let source = sources.from_explosion(None, &mut host).unwrap();
        assert_eq!(host.calls, ["lookup:1:Explosion"]);
        assert!(source.is_direct(&mut host).unwrap());
    }

    #[test]
    fn construction_query_and_snapshot_failures_keep_exact_prefix() {
        let mut host = Host::new();
        host.registry
            .borrow_mut()
            .holders
            .remove(&DamageTypeKey::LightningBolt);
        assert!(matches!(Sources::new(&mut host), Err("missing holder")));
        assert_eq!(
            host.calls,
            [
                "registry",
                "lookup:1:InFire",
                "lookup:1:Campfire",
                "lookup:1:LightningBolt"
            ]
        );
        let mut host = Host::new();
        let sources = Sources::new(&mut host).unwrap();
        let victim = host.actor(1, Class::Player);
        let direct = host.actor(2, Class::Other);
        let causer = host.actor(3, Class::Living);
        let source = sources
            .arrow(Some(direct.clone()), Some(causer.clone()), &mut host)
            .unwrap();
        host.fail = Some("position");
        host.calls.clear();
        assert_eq!(source.source_position(&mut host), Err("owner failure"));
        assert_eq!(host.calls, ["position"]);
        host.fail = None;
        victim.borrow_mut().id = 10;
        causer.borrow_mut().id = 30;
        direct.borrow_mut().id = 20;
        host.calls.clear();
        let packet = damage_event_snapshot(&victim, &source, &mut host).unwrap();
        assert_eq!(host.calls, ["id:10", "id:30", "id:20"]);
        assert_eq!(
            (
                packet.entity_id,
                packet.source_cause_id,
                packet.source_direct_id
            ),
            (10, 30, 20)
        );
        assert!(Rc::ptr_eq(&packet.source_type, source.type_holder()));
        assert!(packet.source_position.is_none());
        let raw = sources
            .bad_respawn_point_explosion(Some(Vec3::new(4., 5., 6.)), &mut host)
            .unwrap();
        let packet = damage_event_snapshot(&victim, &raw, &mut host).unwrap();
        assert_eq!((packet.source_cause_id, packet.source_direct_id), (-1, -1));
        assert_eq!(packet.source_position, Some(Vec3::new(4., 5., 6.)));
        host.calls.clear();
        host.fail = Some("id:20");
        assert!(matches!(
            damage_event_snapshot(&victim, &source, &mut host),
            Err("owner failure")
        ));
        assert_eq!(host.calls, ["id:10", "id:30", "id:20"]);
        host.calls.clear();
        host.fail = Some("explosion_indirect");
        let blast = Rc::new(RefCell::new(Explosion {
            direct: Some(direct),
            causing: Some(causer),
        }));
        assert!(matches!(
            sources.from_explosion(Some(&blast), &mut host),
            Err("owner failure")
        ));
        assert_eq!(host.calls, ["explosion_direct", "explosion_indirect"]);
        host.calls.clear();
        host.fail = Some("lookup:1:Arrow");
        assert!(matches!(
            sources.arrow(None, None, &mut host),
            Err("owner failure")
        ));
        assert_eq!(host.calls, ["lookup:1:Arrow"]);
        assert!(Arc::ptr_eq(&sources.drown(), &sources.drown()));
    }
}
