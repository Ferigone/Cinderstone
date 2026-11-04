//! Ordered/live Java 1.21 BeaconBlockEntity ContainerData over its actual state.
//!
//! Primary evidence is direct pinned JDK21 `javap -p -c` of `dqc$1`
//! get92–96/set102–112, `cpy`'s holder codecs and `dqc.filterEffect`122.
//! No transaction, registry, Level, sound stream or second BE is constructed.
use super::{
    beacon_block_entity::BeaconAction,
    beacon_entity::{BeaconBlockEntity, BeaconPowerRegistry},
};
use crate::{block_entity::BlockEntityComponentMap, native_block_entities::BlockPosition};
use anyhow::{Context, Result};

pub const DATA_COUNT: i32 = 3;

/// Source ContainerData.getCount. There is no world or registry observation.
pub fn data_count() -> i32 {
    DATA_COUNT
}

/// Read the SAME actual Beacon object's raw levels or selected holder.
/// Unknown signed indices return zero. Null powers do not query a registry.
pub fn data_get<T, L, M, E, C, R>(
    entity: &BeaconBlockEntity<T, L, M, E, C>,
    registry: &R,
    index: i32,
) -> Result<i32>
where
    M: BlockEntityComponentMap,
    E: Clone,
    C: Clone,
    R: BeaconPowerRegistry<E> + ?Sized,
{
    let effect = match index {
        0 => return Ok(entity.levels),
        1 => entity.primary.as_ref(),
        2 => entity.secondary.as_ref(),
        _ => return Ok(0),
    };
    let Some(effect) = effect else {
        return Ok(0);
    };
    let identity = registry.identity()?;
    registry.validate_identity(&identity)?;
    let id = registry.builtin_effect_id(effect)?;
    registry.validate_identity(&identity)?;
    Ok(id.wrapping_add(1))
}

pub fn data_values<T, L, M, E, C, R>(
    entity: &BeaconBlockEntity<T, L, M, E, C>,
    registry: &R,
) -> Result<[i32; 3]>
where
    M: BlockEntityComponentMap,
    E: Clone,
    C: Clone,
    R: BeaconPowerRegistry<E> + ?Sized,
{
    Ok([
        data_get(entity, registry, 0)?,
        data_get(entity, registry, 1)?,
        data_get(entity, registry, 2)?,
    ])
}

/// The actual arena's retained Beacon and source Level/registry/sound owners.
///
/// `entity` and `entity_mut` always address the SAME canonical object, including
/// after a callback removes it, replaces its world index entry or reattaches it.
/// An equal object at the same position is not that lease. Admission validates
/// the actual retained capability, not a caller boolean or a mutable version
/// that the operation's own callbacks can invalidate. Only the capabilities
/// reached by this source index are required: index0 has no Level/registry use.
///
/// Level clones retain the actual source object; they are not world copies.
/// Registry leases likewise retain the canonical built-in holder owner rather
/// than resolving a newly attached Level's registry. Identity validation must
/// validate that retained owner, not redirect to its current index/replacement.
/// Read-only queries are nonreentrant. The sound callback may reenter/mutate the
/// real BE and owners. Completed mutation/RNG/output prefixes survive an error;
/// there is no clone, stage publication, rollback, replay or automatic retry.
pub trait BeaconEntityDataHost {
    type Type;
    type Level: Clone;
    type Components: BlockEntityComponentMap;
    type Effect: Clone;
    type Name: Clone;
    type Registry: BeaconPowerRegistry<Self::Effect>;

    fn entity(
        &self,
    ) -> &BeaconBlockEntity<Self::Type, Self::Level, Self::Components, Self::Effect, Self::Name>;
    fn entity_mut(
        &mut self,
    ) -> &mut BeaconBlockEntity<Self::Type, Self::Level, Self::Components, Self::Effect, Self::Name>;
    fn require_actual_live(&self, action: BeaconAction) -> Result<()>;
    fn effect_registry(&self) -> Result<Self::Registry>;
    fn is_client_side(&self, actual_level: &Self::Level) -> Result<bool>;

    /// Actual BEACON_POWER_SELECT at the captured source Level/BlockPos,
    /// BLOCKS, volume/pitch1. This uses that Level's existing sound owner:
    /// exactly one threadSafeRandom.nextLong BEFORE seeded broadcast, even
    /// without recipients. Do not draw from Level.random/player/local streams.
    /// The same actual BE remains retained if this callback removes/reattaches
    /// it; later field assignment still targets that object. Preserve a seed
    /// draw/output or reentrant field mutation if this or a later query errors.
    fn play_power_select_sound(
        &mut self,
        actual_level: Self::Level,
        position: BlockPosition,
    ) -> Result<()>;
}

/// Source ContainerData setter on the actual retained Beacon.
///
/// Unknown signed indices are no-ops before admission. Primary checks the
/// actual attached Level FIRST, then its RAW committed beam list (even at
/// levels0); sound precedes decode/filter/assignment for zero, invalid and
/// unchanged selections alike. Secondary is silent and does not read a Level.
pub fn set_data_live<H: BeaconEntityDataHost>(host: &mut H, index: i32, value: i32) -> Result<()> {
    if !(0..DATA_COUNT).contains(&index) {
        return Ok(());
    }
    host.require_actual_live(BeaconAction::SetData { index, value })?;
    match index {
        0 => host.entity_mut().levels = value,
        1 => {
            // Source dereferences this.level even when the raw beam is empty
            // or value is zero. Missing Level is unsupported, not server=false.
            let level = host
                .entity()
                .base
                .level()
                .context("live beacon primary setter requires its actual attached Level")?
                .clone();
            if !host.is_client_side(&level)? && !host.entity().scan.committed_sections().is_empty()
            {
                let position = host.entity().base.block_position();
                host.play_power_select_sound(level, position)?;
            }
            let effect = decode_filtered(host, value)?;
            host.entity_mut().primary = effect;
        }
        2 => {
            let effect = decode_filtered(host, value)?;
            host.entity_mut().secondary = effect;
        }
        _ => unreachable!(),
    }
    Ok(())
}

fn decode_filtered<H: BeaconEntityDataHost>(host: &H, value: i32) -> Result<Option<H::Effect>> {
    // Source decode zero returns null before touching BuiltInRegistries.
    if value == 0 {
        return Ok(None);
    }
    // Acquire this canonical registry lease only at the reached source query,
    // AFTER a preceding sound. It must not follow a reattached/current BE index.
    let registry = host.effect_registry()?;
    let identity = registry.identity()?;
    registry.validate_identity(&identity)?;
    let effect = registry.builtin_effect_by_id(value.wrapping_sub(1))?;
    let effect = match effect {
        Some(effect) if registry.is_valid_beacon_effect(&effect)? => Some(effect),
        _ => None,
    };
    registry.validate_identity(&identity)?;
    Ok(effect)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        block_entity::NativeBlockEntityBase,
        block_entity_empty_components::NativeEmptyComponentMap,
        connection_cookies::CookieKey,
        geometry::Aabb,
        workstations::{
            beacon_block_entity::{
                self, BeaconFields, BeaconLiveOwner, BeaconLiveTickHost, BeaconScanState,
            },
            beacon_effects::{
                BeaconEffectApplication, BeaconEffectsHost, BeaconSound, BeaconSoundHost,
            },
            beacon_entity::{
                BeaconBaseFactory, BeaconClass, BeaconClassEffects, BeaconClassInitializer,
            },
        },
    };
    use anyhow::{bail, ensure};
    use std::{
        cell::{Cell, RefCell},
        sync::Arc,
    };

    #[derive(Debug)]
    struct Effect(CookieKey);
    #[derive(Clone, Debug)]
    struct Identity(Arc<()>);
    impl PartialEq for Identity {
        fn eq(&self, other: &Self) -> bool {
            Arc::ptr_eq(&self.0, &other.0)
        }
    }
    impl Eq for Identity {}
    struct RegistryState {
        identity: Identity,
        holders: Vec<Arc<Effect>>,
        fail_lookup: Cell<bool>,
        trace: RefCell<Vec<&'static str>>,
    }
    #[derive(Clone)]
    struct Registry(Arc<RegistryState>);
    impl BeaconPowerRegistry<Arc<Effect>> for Registry {
        type Identity = Identity;
        fn identity(&self) -> Result<Identity> {
            Ok(self.0.identity.clone())
        }
        fn validate_identity(&self, actual: &Identity) -> Result<()> {
            ensure!(
                *actual == self.0.identity,
                "controlled retained registry mismatch"
            );
            Ok(())
        }
        fn builtin_effect_id(&self, effect: &Arc<Effect>) -> Result<i32> {
            Ok(self
                .0
                .holders
                .iter()
                .position(|actual| Arc::ptr_eq(actual, effect))
                .map_or(-1, |id| id as i32))
        }
        fn builtin_effect_by_id(&self, id: i32) -> Result<Option<Arc<Effect>>> {
            self.0.trace.borrow_mut().push("lookup");
            ensure!(
                !self.0.fail_lookup.get(),
                "controlled lookup failure after sound"
            );
            Ok(usize::try_from(id)
                .ok()
                .and_then(|id| self.0.holders.get(id))
                .cloned())
        }
        fn builtin_effect_by_key(&self, key: &CookieKey) -> Result<Option<Arc<Effect>>> {
            Ok(self
                .0
                .holders
                .iter()
                .find(|holder| holder.0 == *key)
                .cloned())
        }
        fn builtin_effect_key(&self, effect: &Arc<Effect>) -> Result<Option<CookieKey>> {
            Ok(Some(effect.0.clone()))
        }
        fn is_valid_beacon_effect(&self, effect: &Arc<Effect>) -> Result<bool> {
            Ok(self
                .0
                .holders
                .iter()
                .any(|actual| Arc::ptr_eq(actual, effect)))
        }
    }
    struct ClassEffects([Vec<Arc<Effect>>; 4]);
    impl BeaconClassEffects<Arc<Effect>> for ClassEffects {
        fn tier(&self, index: usize) -> Result<&[Arc<Effect>]> {
            Ok(&self.0[index])
        }
        fn contains(&self, effect: &Arc<Effect>) -> Result<bool> {
            Ok(self
                .0
                .iter()
                .flatten()
                .any(|actual| Arc::ptr_eq(actual, effect)))
        }
    }
    struct Initializer(Registry);
    impl BeaconClassInitializer<Arc<Effect>, Arc<&'static str>> for Initializer {
        fn initialize_effect_constants(
            &mut self,
        ) -> Result<Arc<dyn BeaconClassEffects<Arc<Effect>>>> {
            let h = &self.0 .0.holders;
            Ok(Arc::new(ClassEffects([
                vec![h[0].clone(), h[1].clone()],
                vec![h[2].clone(), h[3].clone()],
                vec![h[4].clone()],
                vec![h[5].clone()],
            ])))
        }
        fn translatable_default_name(&mut self, key: &'static str) -> Result<Arc<&'static str>> {
            ensure!(
                key == "container.beacon",
                "wrong controlled class name request"
            );
            Ok(Arc::new(key))
        }
    }
    struct Level {
        label: &'static str,
        client: bool,
    }
    type Entity = BeaconBlockEntity<
        Arc<()>,
        Arc<Level>,
        NativeEmptyComponentMap,
        Arc<Effect>,
        Arc<&'static str>,
    >;
    struct Factory(Arc<()>);
    impl BeaconBaseFactory<Arc<()>, Arc<Level>, NativeEmptyComponentMap> for Factory {
        fn create_beacon_base(
            &mut self,
            position: BlockPosition,
            state: u32,
        ) -> Result<NativeBlockEntityBase<Arc<()>, Arc<Level>, NativeEmptyComponentMap>> {
            NativeBlockEntityBase::construct(
                self.0.clone(),
                position,
                state,
                NativeEmptyComponentMap::singleton(),
            )
        }
    }

    // This is a controlled native owner, not a configured registry, actual
    // Level/arena, sound RNG/broadcast or Main integration. The inherited base
    // and canonical EMPTY owner are genuine shared implementations. The source
    // scan below is built by the existing live tick, never fabricated by
    // writing its private lists. No transaction implementation exists here.
    struct Owner {
        entity: Box<Entity>,
        retained: *const Entity,
        registry: Registry,
        replacement_level: Arc<Level>,
        sound_arguments: Vec<Arc<Level>>,
    }
    impl Owner {
        fn require_retained(&self) -> Result<()> {
            ensure!(
                std::ptr::eq(&*self.entity, self.retained),
                "controlled retained BE changed"
            );
            Ok(())
        }
    }
    impl BeaconEntityDataHost for Owner {
        type Type = Arc<()>;
        type Level = Arc<Level>;
        type Components = NativeEmptyComponentMap;
        type Effect = Arc<Effect>;
        type Name = Arc<&'static str>;
        type Registry = Registry;
        fn entity(&self) -> &Entity {
            &self.entity
        }
        fn entity_mut(&mut self) -> &mut Entity {
            &mut self.entity
        }
        fn require_actual_live(&self, _: BeaconAction) -> Result<()> {
            self.require_retained()
        }
        fn effect_registry(&self) -> Result<Registry> {
            Ok(self.registry.clone())
        }
        fn is_client_side(&self, level: &Arc<Level>) -> Result<bool> {
            Ok(level.client)
        }
        fn play_power_select_sound(
            &mut self,
            level: Arc<Level>,
            position: BlockPosition,
        ) -> Result<()> {
            ensure!(
                position == self.entity.base.block_position(),
                "wrong controlled sound position"
            );
            self.registry.0.trace.borrow_mut().push("sound");
            self.sound_arguments.push(level);
            // Inject reentry into this SAME canonical object. There is no
            // cloned actor/BE image to roll back if later holder lookup fails.
            self.entity.levels = 9;
            self.entity.primary = Some(self.registry.0.holders[2].clone());
            self.entity.base.set_removed();
            self.entity
                .base
                .set_level(Some(self.replacement_level.clone()));
            Ok(())
        }
    }
    impl BeaconFields for Owner {
        type Effect = Arc<Effect>;
        fn require_actual_stage(&self, _: BeaconAction) -> Result<()> {
            bail!("controlled live-only owner has no stage")
        }
        fn position(&self) -> BlockPosition {
            self.entity.base.block_position()
        }
        fn is_client_side(&self) -> bool {
            self.entity.base.level().unwrap().client
        }
        fn game_time(&self) -> i64 {
            1
        }
        fn min_build_height(&self) -> i32 {
            0
        }
        fn dimension_height(&self) -> i32 {
            64
        }
        fn scan(&self) -> &BeaconScanState {
            &self.entity.scan
        }
        fn scan_mut(&mut self) -> &mut BeaconScanState {
            &mut self.entity.scan
        }
        fn levels(&self) -> i32 {
            self.entity.levels
        }
        fn set_levels_raw(&mut self, value: i32) {
            self.entity.levels = value;
        }
        fn primary(&self) -> Option<&Arc<Effect>> {
            self.entity.primary.as_ref()
        }
        fn secondary(&self) -> Option<&Arc<Effect>> {
            self.entity.secondary.as_ref()
        }
        fn set_primary_raw(&mut self, effect: Option<Arc<Effect>>) {
            self.entity.primary = effect;
        }
        fn set_secondary_raw(&mut self, effect: Option<Arc<Effect>>) {
            self.entity.secondary = effect;
        }
        fn same_effect_holder(&self, left: &Arc<Effect>, right: &Arc<Effect>) -> Result<bool> {
            Ok(Arc::ptr_eq(left, right))
        }
    }
    impl BeaconLiveOwner for Owner {
        fn require_actual_live(&self, _: BeaconAction) -> Result<()> {
            self.require_retained()
        }
    }
    impl BeaconSoundHost for Owner {
        fn play_sound(&mut self, _: BeaconSound) -> Result<()> {
            bail!("unreached controlled scan sound")
        }
    }
    impl BeaconEffectsHost for Owner {
        type Player = ();
        type ServerPlayer = ();
        fn players_in_effect_box(&mut self, _: Aabb) -> Result<Vec<()>> {
            bail!("unreached controlled actor query")
        }
        fn server_players_in_activation_box(&mut self, _: Aabb) -> Result<Vec<()>> {
            bail!("unreached controlled criterion query")
        }
        fn add_beacon_effect(
            &mut self,
            _: &(),
            _: BeaconEffectApplication<'_, Arc<Effect>>,
        ) -> Result<()> {
            bail!("unreached controlled live effect")
        }
        fn trigger_construct_beacon(&mut self, _: &(), _: i32) -> Result<()> {
            bail!("unreached controlled criterion")
        }
    }
    impl BeaconLiveTickHost for Owner {
        type BlockState = ();
        fn world_surface_height(&mut self, _: i32, _: i32) -> Result<i32> {
            Ok(0)
        }
        fn block_state(&mut self, _: BlockPosition) -> Result<()> {
            Ok(())
        }
        fn beam_color(&self, _: &()) -> Result<Option<u32>> {
            Ok(Some(0xffff_ffff))
        }
        fn light_block(&mut self, _: &(), _: BlockPosition) -> Result<i32> {
            bail!("unreached controlled light query")
        }
        fn is_bedrock(&self, _: &()) -> Result<bool> {
            bail!("unreached controlled bedrock query")
        }
        fn is_beacon_base(&self, _: &()) -> Result<bool> {
            bail!("unreached controlled base query")
        }
    }

    #[test]
    fn live_primary_retains_actual_be_and_sound_prefix_before_lookup_failure() -> Result<()> {
        let holders = [
            "speed",
            "haste",
            "resistance",
            "jump_boost",
            "strength",
            "regeneration",
        ]
        .into_iter()
        .map(|key| Ok(Arc::new(Effect(CookieKey::parse(key)?))))
        .collect::<Result<Vec<_>>>()?;
        let registry = Registry(Arc::new(RegistryState {
            identity: Identity(Arc::new(())),
            holders,
            fail_lookup: Cell::new(false),
            trace: RefCell::new(Vec::new()),
        }));
        let mut class = BeaconClass::uninitialized();
        class.initialize(&mut Initializer(registry.clone()))?;
        let entity = Box::new(BeaconBlockEntity::construct(
            (8, 0, 9),
            0,
            Arc::new(class),
            &mut Factory(Arc::new(())),
        )?);
        let retained = &*entity as *const Entity;
        let replacement_level = Arc::new(Level {
            label: "replacement",
            client: false,
        });
        let mut owner = Owner {
            entity,
            retained,
            registry,
            replacement_level,
            sound_arguments: Vec::new(),
        };
        assert_eq!(data_count(), 3);
        // Neither an unknown index nor levels/secondary touches a Level.
        set_data_live(&mut owner, i32::MIN, 77)?;
        set_data_live(&mut owner, 0, -5)?;
        set_data_live(&mut owner, 2, 2)?;
        assert_eq!(data_values(&owner.entity, &owner.registry)?, [-5, 0, 2]);
        assert!(owner.entity.base.level().is_none());
        assert!(set_data_live(&mut owner, 1, 0).is_err());
        let original_level = Arc::new(Level {
            label: "original",
            client: false,
        });
        owner.entity.base.set_level(Some(original_level.clone()));
        owner.entity.scan.after_set_level(0);
        owner.entity.levels = 0;
        beacon_block_entity::tick_live(&mut owner)?;
        assert!(!owner.entity.scan.committed_sections().is_empty());
        owner.registry.0.trace.borrow_mut().clear();
        owner.registry.0.fail_lookup.set(true);
        assert!(set_data_live(&mut owner, 1, 2).is_err());
        assert_eq!(&*owner.registry.0.trace.borrow(), &["sound", "lookup"]);
        assert_eq!(owner.sound_arguments[0].label, "original");
        assert!(Arc::ptr_eq(&owner.sound_arguments[0], &original_level));
        assert!(Arc::ptr_eq(
            owner.entity.base.level().unwrap(),
            &owner.replacement_level
        ));
        assert!(owner.entity.base.is_removed());
        assert_eq!(owner.entity.levels, 9);
        assert!(Arc::ptr_eq(
            owner.entity.primary.as_ref().unwrap(),
            &owner.registry.0.holders[2]
        ));
        owner.require_retained()?;
        // Encoded zero still sounds, then clears the callback's primary, even
        // while registry lookup is unavailable. It never queries that owner.
        owner.registry.0.trace.borrow_mut().clear();
        set_data_live(&mut owner, 1, 0)?;
        assert_eq!(&*owner.registry.0.trace.borrow(), &["sound"]);
        assert!(owner.entity.primary.is_none());
        assert_eq!(owner.sound_arguments[1].label, "replacement");
        Ok(())
    }
}
