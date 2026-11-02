//! Java 1.21 Beacon subclass fields over the actual inherited BlockEntity.
//!
//! Direct pinned JDK21 javap of dqc and bqu is the primary evidence. This
//! module creates no registry, Level, living actor, menu, or ContainerData(this)
//! capture. The actual arena/menu factory must retain that same BE separately.
use super::beacon_block_entity::BeaconScanState;
use crate::{
    block_entity::{BlockEntityComponentMap, NativeBlockEntityBase},
    block_entity_compound::{NativeFreshCompound, NativeFreshCompoundOwner},
    connection_cookies::CookieKey,
    enchanting_block_entity::{EnchantingComponentBuilder, EnchantingComponentInput},
    inventory::ItemStack,
    native_block_entities::BlockPosition,
    nbt::{JavaString, Tag},
    source_nbt_decode::SourceCompoundView,
};
use anyhow::{bail, ensure, Context, Result};
use std::{
    fmt::Debug,
    sync::{Arc, OnceLock},
};

pub const BEACON_ID: &str = "minecraft:beacon";
pub const DEFAULT_NAME_KEY: &str = "container.beacon";

/// Read-only access to the actual built-in effect registry and VALID_EFFECTS.
/// Holder clones retain the real objects. Identity includes the real owner and
/// generation, never equal tables or a locally fabricated revision counter.
pub trait BeaconPowerRegistry<E: Clone> {
    type Identity: Clone + Eq + Debug;
    fn identity(&self) -> Result<Self::Identity>;
    fn validate_identity(&self, actual: &Self::Identity) -> Result<()>;
    fn builtin_effect_id(&self, effect: &E) -> Result<i32>;
    fn builtin_effect_by_id(&self, id: i32) -> Result<Option<E>>;
    fn builtin_effect_by_key(&self, key: &CookieKey) -> Result<Option<E>>;
    fn builtin_effect_key(&self, effect: &E) -> Result<Option<CookieKey>>;
    fn is_valid_beacon_effect(&self, effect: &E) -> Result<bool>;
}

/// Retained source BEACON_EFFECTS/VALID_EFFECTS objects. The four immutable
/// tiers contain [speed,haste], [resistance,jump], [strength], [regeneration]
/// using the actual built-in holders, and contains uses their real equality.
/// This is a class constant owner, not a new or reconstructed effect registry.
pub trait BeaconClassEffects<E>: Send + Sync {
    fn tier(&self, index: usize) -> Result<&[E]>;
    fn contains(&self, effect: &E) -> Result<bool>;
}

/// Actual class initialization callbacks, in source order. No default provides
/// guessed holders or translates a placeholder Component. The runtime retains
/// ONE BeaconClass outside world snapshots for this source class lifetime.
pub trait BeaconClassInitializer<E, C> {
    fn initialize_effect_constants(&mut self) -> Result<Arc<dyn BeaconClassEffects<E>>>;
    fn translatable_default_name(&mut self, key: &'static str) -> Result<C>;
}

enum ClassState<E, C> {
    Uninitialized,
    Ready {
        effects: Arc<dyn BeaconClassEffects<E>>,
        default_name: C,
    },
    Failed,
}

/// C clones retain the SAME actual immutable Component object. Parsing another
/// component, copying a String, or reconstructing equal text is not that
/// source identity. E clones likewise retain the same actual effect holders.
pub struct BeaconClass<E, C> {
    state: ClassState<E, C>,
}
impl<E, C> BeaconClass<E, C> {
    pub fn uninitialized() -> Self {
        Self {
            state: ClassState::Uninitialized,
        }
    }
    pub fn initialize(&mut self, owner: &mut impl BeaconClassInitializer<E, C>) -> Result<()> {
        match &self.state {
            ClassState::Ready { .. } => return Ok(()),
            ClassState::Failed => bail!("Beacon source class initialization previously failed"),
            ClassState::Uninitialized => {}
        }
        // Source class initialization errors are sticky; never reconstruct or
        // retry the first source prefix after its actual owner failed.
        self.state = ClassState::Failed;
        let effects = owner.initialize_effect_constants()?;
        for (index, length) in [2, 2, 1, 1].into_iter().enumerate() {
            ensure!(
                effects.tier(index)?.len() == length,
                "actual Beacon class tier shape differs from Java 1.21"
            );
        }
        let default_name = owner.translatable_default_name(DEFAULT_NAME_KEY)?;
        self.state = ClassState::Ready {
            effects,
            default_name,
        };
        Ok(())
    }
    pub fn is_initialized(&self) -> bool {
        matches!(self.state, ClassState::Ready { .. })
    }
    pub fn default_name(&self) -> Result<&C> {
        match &self.state {
            ClassState::Ready { default_name, .. } => Ok(default_name),
            _ => bail!("actual Beacon class DEFAULT_NAME is unavailable"),
        }
    }
    pub fn effects(&self) -> Result<&dyn BeaconClassEffects<E>> {
        match &self.state {
            ClassState::Ready { effects, .. } => Ok(effects.as_ref()),
            _ => bail!("actual Beacon class effect constants are unavailable"),
        }
    }
}

/// Actual immutable source LockCode record. Equality is UTF-16 key value,
/// independent of record identity. Present empty strings construct a fresh
/// record; absent/wrong-type NBT returns the actual NO_LOCK singleton.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct BeaconLockCode {
    key: JavaString,
}
impl BeaconLockCode {
    pub fn no_lock() -> Arc<Self> {
        static NO_LOCK: OnceLock<Arc<BeaconLockCode>> = OnceLock::new();
        Arc::clone(NO_LOCK.get_or_init(|| Arc::new(Self { key: "".into() })))
    }
    pub fn from_key(key: JavaString) -> Arc<Self> {
        Arc::new(Self { key })
    }
    pub fn key(&self) -> &JavaString {
        &self.key
    }
    pub fn is_empty(&self) -> bool {
        self.key.is_empty()
    }
    pub fn from_tag(source: SourceCompoundView<'_>) -> Arc<Self> {
        match source.tag().get("Lock") {
            Some(Tag::String(key)) => Self::from_key(key.clone()),
            _ => Self::no_lock(),
        }
    }
    pub fn unlocks_with(
        &self,
        stack: &ItemStack,
        names: &mut impl BeaconLockNameOwner,
    ) -> Result<bool> {
        if self.key.is_empty() {
            return Ok(true);
        }
        Ok(names.custom_name_string(stack)?.as_ref() == Some(&self.key))
    }
}

/// Actual ItemStack.get(CUSTOM_NAME), then that Component.getString(). No item
/// emptiness/ID shortcut or guessed translation/JSON/literal fallback exists.
pub trait BeaconLockNameOwner {
    fn custom_name_string(&mut self, actual_stack: &ItemStack) -> Result<Option<JavaString>>;
}

/// Required canonical BEACON type/state/fresh-object factory. Its returned
/// object is newly constructed by the real owner with that canonical type and
/// genuine EMPTY component singleton. Freshness field checks below supplement
/// this capability; they cannot certify an arbitrary reused object by value.
pub trait BeaconBaseFactory<T, L, M: BlockEntityComponentMap> {
    fn create_beacon_base(
        &mut self,
        position: BlockPosition,
        state: u32,
    ) -> Result<NativeBlockEntityBase<T, L, M>>;
}

/// Actual Beacon subclass fields. Not Clone: a stage or arena must not duplicate
/// this live object or manufacture a second name/lock/beam/power owner.
/// C/E clones retain the same actual Component/holder references; they are not
/// newly parsed components, copied text or reconstructed numeric holders.
/// ContainerData(this), actual type registration, world indexing and session
/// retention remain with the real arena/factory. No stand-in token is supplied.
pub struct BeaconBlockEntity<T, L, M: BlockEntityComponentMap, E: Clone, C: Clone> {
    pub(crate) base: NativeBlockEntityBase<T, L, M>,
    pub(crate) scan: BeaconScanState,
    pub(crate) levels: i32,
    pub(crate) primary: Option<E>,
    pub(crate) secondary: Option<E>,
    pub(crate) name: Option<C>,
    pub(crate) lock: Arc<BeaconLockCode>,
    pub(crate) class: Arc<BeaconClass<E, C>>,
}

pub trait BeaconLevelHeight<L> {
    fn min_build_height(&mut self, actual_level: &L) -> Result<i32>;
}
pub trait BeaconImplicitComponentInput<C>: EnchantingComponentInput<C> {
    fn lock(&mut self) -> Result<Option<Arc<BeaconLockCode>>>;
}
pub trait BeaconImplicitComponentBuilder<C>: EnchantingComponentBuilder<C> {
    fn set_lock(&mut self, actual: Arc<BeaconLockCode>) -> Result<()>;
}
pub trait BeaconCompoundRemoval {
    type Compound;
    fn remove(&mut self, actual: &mut Self::Compound, key: &JavaString) -> Result<()>;
}
impl BeaconCompoundRemoval for NativeFreshCompoundOwner {
    type Compound = NativeFreshCompound;
    fn remove(&mut self, actual: &mut Self::Compound, key: &JavaString) -> Result<()> {
        NativeFreshCompoundOwner::remove(self, actual, key);
        Ok(())
    }
}

/// Required live retained-BE and actual Level sound capability. Sound runs on
/// the original captured Level/position and may mutate/remove/reattach the same
/// BE. Completed seed/output/callback prefixes persist on an error; no retry.
/// The retained capability survives removal/reentry; it must not redirect to
/// the current coordinate index or reject the call's own mutable revisions.
pub trait BeaconRemovalHost<B, L> {
    fn require_actual_retained(&self, actual: &B) -> Result<()>;
    fn play_deactivation_sound(
        &mut self,
        actual: &mut B,
        level: &L,
        position: BlockPosition,
    ) -> Result<()>;
}

/// The real shared BaseContainerBlockEntity.canUnlock and BeaconMenu factory.
/// can_unlock must preserve its actual spectator/main-hand/name/message/sound
/// behavior; creative is not a substitute. Successful menu construction retains
/// this SAME BE and its actual ContainerData(this), not a coordinate/token or a
/// copied levels/powers image. The captured Level/position build the real access.
pub trait BeaconMenuFactory<B, L, C> {
    type Inventory: ?Sized;
    type Player: ?Sized;
    type Menu;
    fn require_actual_retained(&self, actual: &B) -> Result<()>;
    fn can_unlock(
        &mut self,
        actual: &mut B,
        player: &Self::Player,
        lock: Arc<BeaconLockCode>,
        display_name: C,
    ) -> Result<bool>;
    fn create_beacon_menu(
        &mut self,
        actual: &mut B,
        id: i32,
        inventory: &Self::Inventory,
        level: L,
        position: BlockPosition,
    ) -> Result<Self::Menu>;
}

impl<T, L, M: BlockEntityComponentMap, E: Clone, C: Clone> BeaconBlockEntity<T, L, M, E, C> {
    pub fn construct(
        position: BlockPosition,
        state: u32,
        class: Arc<BeaconClass<E, C>>,
        factory: &mut impl BeaconBaseFactory<T, L, M>,
    ) -> Result<Self> {
        ensure!(
            class.is_initialized(),
            "actual Beacon class must initialize before construction"
        );
        let base = factory.create_beacon_base(position, state)?;
        ensure!(
            base.block_position() == position
                && base.block_state() == state
                && !base.has_level()
                && !base.is_removed()
                && base.components().is_empty()
                && base.components().is_empty_singleton(),
            "canonical Beacon factory did not return a fresh source base"
        );
        Ok(Self {
            base,
            scan: BeaconScanState::default(),
            levels: 0,
            primary: None,
            secondary: None,
            name: None,
            lock: BeaconLockCode::no_lock(),
            class,
        })
    }
    pub fn base(&self) -> &NativeBlockEntityBase<T, L, M> {
        &self.base
    }
    pub fn base_mut(&mut self) -> &mut NativeBlockEntityBase<T, L, M> {
        &mut self.base
    }
    pub fn scan(&self) -> &BeaconScanState {
        &self.scan
    }
    pub fn scan_mut(&mut self) -> &mut BeaconScanState {
        &mut self.scan
    }
    pub fn levels(&self) -> i32 {
        self.levels
    }
    pub fn set_levels_raw(&mut self, value: i32) {
        self.levels = value;
    }
    pub fn primary(&self) -> Option<&E> {
        self.primary.as_ref()
    }
    pub fn secondary(&self) -> Option<&E> {
        self.secondary.as_ref()
    }
    pub fn set_primary_raw(&mut self, value: Option<E>) {
        self.primary = value;
    }
    pub fn set_secondary_raw(&mut self, value: Option<E>) {
        self.secondary = value;
    }
    pub fn lock(&self) -> &Arc<BeaconLockCode> {
        &self.lock
    }
    pub fn set_lock_raw(&mut self, actual: Arc<BeaconLockCode>) {
        self.lock = actual;
    }
    pub fn class(&self) -> &Arc<BeaconClass<E, C>> {
        &self.class
    }
    pub fn get_custom_name(&self) -> Option<&C> {
        self.name.as_ref()
    }
    pub fn set_custom_name(&mut self, actual: Option<C>) {
        self.name = actual;
    }
    pub fn get_name(&self) -> Result<C> {
        match &self.name {
            Some(actual) => Ok(actual.clone()),
            None => Ok(self.class.default_name()?.clone()),
        }
    }
    pub fn get_display_name(&self) -> Result<C> {
        self.get_name()
    }
    /// Base attachment precedes the actual Level min-height query. An error
    /// retains that attachment prefix and does not reset other Beacon fields.
    pub fn set_level(&mut self, actual: L, heights: &mut impl BeaconLevelHeight<L>) -> Result<()> {
        self.base.set_level(Some(actual));
        let minimum =
            heights.min_build_height(self.base.level().expect("just attached actual Level"))?;
        self.scan.after_set_level(minimum);
        Ok(())
    }
    pub fn apply_implicit_components(
        &mut self,
        input: &mut impl BeaconImplicitComponentInput<C>,
    ) -> Result<()> {
        // The inherited source hook is empty. Assignments preserve their prefix.
        self.name = input.custom_name()?;
        self.lock = input.lock()?.unwrap_or_else(BeaconLockCode::no_lock);
        Ok(())
    }
    pub fn collect_implicit_components(
        &self,
        builder: &mut impl BeaconImplicitComponentBuilder<C>,
    ) -> Result<()> {
        builder.set_custom_name(self.name.clone())?;
        if self.lock.as_ref() != BeaconLockCode::no_lock().as_ref() {
            builder.set_lock(Arc::clone(&self.lock))?;
        }
        Ok(())
    }
    pub fn remove_components_from_tag<O: BeaconCompoundRemoval>(
        &self,
        target: &mut O::Compound,
        owner: &mut O,
    ) -> Result<()> {
        owner.remove(target, &"CustomName".into())?;
        owner.remove(target, &"Lock".into())
    }
    pub fn set_removed_live<H: BeaconRemovalHost<Self, L>>(&mut self, host: &mut H) -> Result<()>
    where
        L: Clone,
    {
        host.require_actual_retained(self)?;
        let level = self
            .base
            .level()
            .cloned()
            .context("Beacon removal has no actual attached Level")?;
        let position = self.base.block_position();
        // Source does this unconditionally, including repeated removed calls.
        host.play_deactivation_sound(self, &level, position)?;
        self.base.set_removed();
        Ok(())
    }
    pub fn create_menu<F: BeaconMenuFactory<Self, L, C>>(
        &mut self,
        id: i32,
        inventory: &F::Inventory,
        player: &F::Player,
        factory: &mut F,
    ) -> Result<Option<F::Menu>>
    where
        L: Clone,
    {
        factory.require_actual_retained(self)?;
        let lock = Arc::clone(&self.lock);
        let display_name = self.get_display_name()?;
        if !factory.can_unlock(self, player, lock, display_name)? {
            return Ok(None);
        }
        let level = self
            .base
            .level()
            .cloned()
            .context("Beacon menu has no actual attached Level")?;
        let position = self.base.block_position();
        Ok(Some(factory.create_beacon_menu(
            self, id, inventory, level, position,
        )?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block_entity_empty_components::NativeEmptyComponentMap;
    use std::cell::RefCell;

    // Controlled owners over the genuine NativeBlockEntityBase/EMPTY service.
    // These type, Level, Component and effect handles are fixture objects, not
    // an actual configured registry, world, actor, criterion or Main binding.
    type E = Arc<()>;
    type C = Arc<JavaString>;
    #[derive(Debug)]
    struct Level(i32);
    type L = Arc<Level>;
    type Entity = BeaconBlockEntity<Arc<()>, L, NativeEmptyComponentMap, E, C>;

    struct Effects {
        tiers: [Vec<E>; 4],
    }
    impl BeaconClassEffects<E> for Effects {
        fn tier(&self, index: usize) -> Result<&[E]> {
            self.tiers
                .get(index)
                .map(Vec::as_slice)
                .context("fixture tier absent")
        }
        fn contains(&self, effect: &E) -> Result<bool> {
            Ok(self.tiers.iter().flatten().any(|e| Arc::ptr_eq(e, effect)))
        }
    }
    struct Initializer {
        trace: RefCell<Vec<&'static str>>,
        name: C,
    }
    impl BeaconClassInitializer<E, C> for Initializer {
        fn initialize_effect_constants(&mut self) -> Result<Arc<dyn BeaconClassEffects<E>>> {
            self.trace.borrow_mut().push("effects");
            Ok(Arc::new(Effects {
                tiers: [
                    vec![Arc::new(()), Arc::new(())],
                    vec![Arc::new(()), Arc::new(())],
                    vec![Arc::new(())],
                    vec![Arc::new(())],
                ],
            }))
        }
        fn translatable_default_name(&mut self, key: &'static str) -> Result<C> {
            ensure!(key == DEFAULT_NAME_KEY, "wrong source key");
            self.trace.borrow_mut().push("name");
            Ok(Arc::clone(&self.name))
        }
    }
    struct Factory {
        actual_type: Arc<()>,
        calls: usize,
    }
    impl BeaconBaseFactory<Arc<()>, L, NativeEmptyComponentMap> for Factory {
        fn create_beacon_base(
            &mut self,
            pos: BlockPosition,
            state: u32,
        ) -> Result<NativeBlockEntityBase<Arc<()>, L, NativeEmptyComponentMap>> {
            self.calls += 1;
            NativeBlockEntityBase::construct(
                Arc::clone(&self.actual_type),
                pos,
                state,
                NativeEmptyComponentMap::singleton(),
            )
        }
    }
    struct Heights {
        fail: bool,
    }
    impl BeaconLevelHeight<L> for Heights {
        fn min_build_height(&mut self, level: &L) -> Result<i32> {
            ensure!(!self.fail, "controlled height failure");
            Ok(level.0)
        }
    }
    struct Components {
        name: Option<C>,
        lock: Option<Arc<BeaconLockCode>>,
        lock_sets: usize,
    }
    impl EnchantingComponentInput<C> for Components {
        fn custom_name(&mut self) -> Result<Option<C>> {
            Ok(self.name.clone())
        }
    }
    impl BeaconImplicitComponentInput<C> for Components {
        fn lock(&mut self) -> Result<Option<Arc<BeaconLockCode>>> {
            Ok(self.lock.clone())
        }
    }
    impl EnchantingComponentBuilder<C> for Components {
        fn set_custom_name(&mut self, actual: Option<C>) -> Result<()> {
            self.name = actual;
            Ok(())
        }
    }
    impl BeaconImplicitComponentBuilder<C> for Components {
        fn set_lock(&mut self, actual: Arc<BeaconLockCode>) -> Result<()> {
            self.lock_sets += 1;
            self.lock = Some(actual);
            Ok(())
        }
    }
    struct Names {
        actual: JavaString,
        calls: usize,
    }
    impl BeaconLockNameOwner for Names {
        fn custom_name_string(&mut self, _: &ItemStack) -> Result<Option<JavaString>> {
            self.calls += 1;
            Ok(Some(self.actual.clone()))
        }
    }
    struct Removal {
        expected_level: L,
        reattach: L,
        calls: usize,
        fail: bool,
    }
    impl BeaconRemovalHost<Entity, L> for Removal {
        fn require_actual_retained(&self, _: &Entity) -> Result<()> {
            Ok(())
        }
        fn play_deactivation_sound(
            &mut self,
            actual: &mut Entity,
            level: &L,
            pos: BlockPosition,
        ) -> Result<()> {
            ensure!(
                Arc::ptr_eq(level, &self.expected_level) && pos == (1, 2, 3),
                "lost source argument lease"
            );
            self.calls += 1;
            actual.set_levels_raw(40 + self.calls as i32);
            actual
                .base_mut()
                .set_level(Some(Arc::clone(&self.reattach)));
            ensure!(!self.fail, "controlled sound-prefix failure");
            Ok(())
        }
    }

    #[test]
    fn retained_native_base_class_lock_and_lifecycle_prefixes() -> Result<()> {
        let default_name = Arc::new(JavaString::from("fixture canonical translatable Component"));
        let mut initializer = Initializer {
            trace: RefCell::new(Vec::new()),
            name: Arc::clone(&default_name),
        };
        let mut class = BeaconClass::<E, C>::uninitialized();
        class.initialize(&mut initializer)?;
        class.initialize(&mut initializer)?;
        assert_eq!(*initializer.trace.borrow(), ["effects", "name"]);
        let class = Arc::new(class);
        let mut factory = Factory {
            actual_type: Arc::new(()),
            calls: 0,
        };
        let mut entity = Entity::construct((1, 2, 3), 7, Arc::clone(&class), &mut factory)?;
        let other = Entity::construct((4, 5, 6), 7, Arc::clone(&class), &mut factory)?;
        assert_eq!(factory.calls, 2);
        assert!(Arc::ptr_eq(&entity.get_name()?, &default_name));
        assert!(Arc::ptr_eq(&other.get_display_name()?, &default_name));
        assert!(Arc::ptr_eq(entity.lock(), &BeaconLockCode::no_lock()));

        let raw = Tag::Compound(vec![("Lock".into(), Tag::String("".into()))])
            .encode_network_anonymous()?;
        let decoded = crate::source_nbt_decode::SourceDecodedNbt::decode_anonymous(&raw)?;
        let empty_record = BeaconLockCode::from_tag(decoded.root_compound().unwrap());
        assert!(!Arc::ptr_eq(&empty_record, &BeaconLockCode::no_lock()));
        assert_eq!(empty_record, BeaconLockCode::no_lock());
        entity.set_lock_raw(empty_record);
        let previous_lock = BeaconLockCode::from_key("previous builder key".into());
        let mut builder = Components {
            name: Some(Arc::clone(&default_name)),
            lock: Some(Arc::clone(&previous_lock)),
            lock_sets: 0,
        };
        entity.collect_implicit_components(&mut builder)?;
        assert!(builder.name.is_none());
        assert_eq!(builder.lock_sets, 0);
        assert!(Arc::ptr_eq(builder.lock.as_ref().unwrap(), &previous_lock));
        let custom = Arc::new(JavaString::from_utf16_lossless(vec![0xd800, 0x61]));
        entity.set_custom_name(Some(Arc::clone(&custom)));
        assert!(Arc::ptr_eq(&entity.get_name()?, &custom));
        let mut input = Components {
            name: None,
            lock: None,
            lock_sets: 0,
        };
        entity.apply_implicit_components(&mut input)?;
        assert!(entity.get_custom_name().is_none());
        assert!(Arc::ptr_eq(entity.lock(), &BeaconLockCode::no_lock()));

        let key = JavaString::from_utf16_lossless(vec![0xd800, 0x61]);
        let lock = BeaconLockCode::from_key(key.clone());
        let mut names = Names {
            actual: key,
            calls: 0,
        };
        assert!(BeaconLockCode::no_lock().unlocks_with(&ItemStack::empty(), &mut names)?);
        assert_eq!(names.calls, 0);
        assert!(lock.unlocks_with(&ItemStack::empty(), &mut names)?);
        assert_eq!(names.calls, 1);

        let first = Arc::new(Level(-64));
        entity.set_level(Arc::clone(&first), &mut Heights { fail: false })?;
        assert_eq!(entity.scan().last_check_y(), -65);
        let second = Arc::new(Level(-32));
        assert!(entity
            .set_level(Arc::clone(&second), &mut Heights { fail: true })
            .is_err());
        assert!(Arc::ptr_eq(entity.base().level().unwrap(), &second));
        assert_eq!(entity.scan().last_check_y(), -65);
        let third = Arc::new(Level(0));
        let mut removal = Removal {
            expected_level: second,
            reattach: Arc::clone(&third),
            calls: 0,
            fail: true,
        };
        assert!(entity.set_removed_live(&mut removal).is_err());
        assert!(!entity.base().is_removed());
        assert_eq!(entity.levels(), 41);
        assert!(Arc::ptr_eq(entity.base().level().unwrap(), &third));
        removal.expected_level = Arc::clone(&third);
        removal.fail = false;
        entity.set_removed_live(&mut removal)?;
        entity.set_removed_live(&mut removal)?;
        assert_eq!(removal.calls, 3);
        assert!(entity.base().is_removed());
        Ok(())
    }
}
