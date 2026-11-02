//! Ordered/live Java 1.21 BeaconMenu effects and data-slot operations.
//!
//! Direct pinned JDK21 `javap -p -c` of `cpy.updateEffects`166–172,
//! `cpy.setData`94–96, `cpu.setData`, `crq.remove` and `bra.removeItem`
//! supplies the source order. This module constructs no menu, payment slot,
//! DataSlot capture, ContainerData(this), Level access or registry owner.
use super::{
    beacon::BeaconUpdate,
    beacon_entity::BeaconPowerRegistry,
    beacon_entity_data::{set_data_live, BeaconEntityDataHost},
};
use anyhow::Result;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeaconMenuLiveAction {
    UpdateEffects,
    SetData { index: i32, value: i32 },
}

/// Actual retained BeaconMenu, its payment Slot, captured DataSlots and access.
///
/// Admission validates the SAME menu object and its original Slot/Container,
/// ContainerData(this), DataSlot-list and ContainerLevelAccess captures. It
/// cannot resolve the player's current menu or a new BE at the same position,
/// substitute a local boolean/version, or reject the call's own reentrant
/// mutations. The inherited BE lease and built-in registry retain their source
/// identities across removal/replacement/reattachment. No new payment owner is
/// stored here; borrow the canonical existing BeaconState/slot/controller.
///
/// This live boundary preserves every completed state, callback and output
/// prefix on an error. It neither stages/clones/rolls back owners nor retries,
/// replays callbacks or republishes inspection receipts. The packet handler's
/// current-menu and stillValid checks belong outside these source methods.
pub trait BeaconMenuLiveHost: BeaconEntityDataHost {
    fn require_actual_retained_menu(&self, action: BeaconMenuLiveAction) -> Result<()>;

    /// Actual payment Slot.hasItem: read its CURRENT canonical stack after
    /// admission, with no creative/tier/selection-validity substitute.
    fn payment_has_item(&self) -> Result<bool>;

    /// Actual payment Slot.remove(1) on its CURRENT Container/slot, discarding
    /// the returned source stack. SimpleContainer.removeItem notifies its
    /// actual listeners only when the removed split is nonempty. Reentry may
    /// already have removed/replaced payment; do not clear a saved snapshot or
    /// add Slot.setChanged/onTake. Preserve removal/listener prefixes on error.
    fn remove_current_payment_one(&mut self) -> Result<()>;

    /// Execute the menu's originally captured ContainerLevelAccess callback:
    /// actual Level.blockEntityChanged(original position), after removal even
    /// when that removal found EMPTY. Do not redirect to the BE's newly attached
    /// Level, call BE.setChanged instead, or invent a missing access as NULL.
    fn captured_menu_block_entity_changed(&mut self) -> Result<()>;

    /// Actual super AbstractContainerMenu.setData: List<DataSlot>.get(index)
    /// then that SAME captured DataSlot.set(value). Negative/out-of-range
    /// indices fail BEFORE any setter or broadcast. The actual Beacon menu
    /// constructor registers three captured ContainerData-backed slots; neither
    /// a guessed index array nor raw ContainerData's unknown-index no-op proves
    /// those live captures. An unavailable list/capture must return an error.
    /// This method does not broadcast separately.
    fn set_captured_menu_data_slot(&mut self, index: i32, value: i32) -> Result<()>;

    /// One actual broadcastChanges over this retained menu's listeners, remote
    /// synchronizer and ordered output owner. No default/no-op publisher.
    fn broadcast_changes(&mut self) -> Result<()>;
}

/// Source updateEffects checks only payment. Encode primary, perform its live
/// ContainerData setter/callbacks, THEN encode and set secondary. Finally remove
/// one CURRENT payment and invoke the originally captured Level access. Direct
/// ContainerData setters here do not invoke Menu.setData or its broadcast.
pub fn update_effects_live<H: BeaconMenuLiveHost>(
    host: &mut H,
    primary: Option<H::Effect>,
    secondary: Option<H::Effect>,
) -> Result<BeaconUpdate> {
    host.require_actual_retained_menu(BeaconMenuLiveAction::UpdateEffects)?;
    if !host.payment_has_item()? {
        return Ok(BeaconUpdate::IgnoredNoPayment);
    }
    let primary = encode_effect(host, primary.as_ref())?;
    set_data_live(host, 1, primary)?;
    let secondary = encode_effect(host, secondary.as_ref())?;
    set_data_live(host, 2, secondary)?;
    host.remove_current_payment_one()?;
    host.captured_menu_block_entity_changed()?;
    Ok(BeaconUpdate::Updated)
}

/// Source BeaconMenu.setData delegates to the actual captured DataSlot list,
/// then broadcasts ONCE. Unlike raw BE ContainerData, invalid signed indices
/// fail before broadcasting; no hardcoded raw-data no-op is substituted.
pub fn set_menu_data_live<H: BeaconMenuLiveHost>(
    host: &mut H,
    index: i32,
    value: i32,
) -> Result<()> {
    host.require_actual_retained_menu(BeaconMenuLiveAction::SetData { index, value })?;
    host.set_captured_menu_data_slot(index, value)?;
    host.broadcast_changes()
}

fn encode_effect<H: BeaconMenuLiveHost>(host: &H, effect: Option<&H::Effect>) -> Result<i32> {
    let Some(effect) = effect else {
        return Ok(0);
    };
    let registry = host.effect_registry()?;
    let identity = registry.identity()?;
    registry.validate_identity(&identity)?;
    let id = registry.builtin_effect_id(effect)?;
    registry.validate_identity(&identity)?;
    // Source getId=-1 becomes zero. Do not reject/filter a holder here or
    // precompute secondary before the primary setter has executed.
    Ok(id.wrapping_add(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        block_entity::NativeBlockEntityBase,
        block_entity_empty_components::NativeEmptyComponentMap,
        connection_cookies::CookieKey,
        crafting::RecipeBook,
        geometry::Aabb,
        inventory::ItemStack,
        native_block_entities::BlockPosition,
        workstations::{
            beacon::BeaconState,
            beacon_block_entity::{
                self, BeaconAction, BeaconFields, BeaconLiveOwner, BeaconLiveTickHost,
                BeaconScanState,
            },
            beacon_effects::{
                BeaconEffectApplication, BeaconEffectsHost, BeaconSound, BeaconSoundHost,
            },
            beacon_entity::{
                BeaconBaseFactory, BeaconBlockEntity, BeaconClass, BeaconClassEffects,
                BeaconClassInitializer,
            },
        },
    };
    use anyhow::{bail, ensure, Context};
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
        sync::Arc,
    };

    #[derive(Debug)]
    struct Effect(CookieKey);
    type Holder = Arc<Effect>;
    #[derive(Clone, Debug)]
    struct Registry {
        holders: Arc<Vec<Holder>>,
        fail_secondary_encoding: Rc<Cell<bool>>,
        trace: Rc<RefCell<Vec<&'static str>>>,
    }
    impl PartialEq for Registry {
        fn eq(&self, other: &Self) -> bool {
            Arc::ptr_eq(&self.holders, &other.holders)
        }
    }
    impl Eq for Registry {}
    impl BeaconPowerRegistry<Holder> for Registry {
        type Identity = Self;
        fn identity(&self) -> Result<Self> {
            Ok(self.clone())
        }
        fn validate_identity(&self, actual: &Self) -> Result<()> {
            ensure!(self == actual, "controlled retained registry changed");
            Ok(())
        }
        fn builtin_effect_id(&self, effect: &Holder) -> Result<i32> {
            let id = self
                .holders
                .iter()
                .position(|actual| Arc::ptr_eq(actual, effect));
            if id == Some(1) {
                self.trace.borrow_mut().push("encode_secondary");
                ensure!(
                    !self.fail_secondary_encoding.get(),
                    "controlled secondary encoding failure"
                );
            } else {
                self.trace.borrow_mut().push("encode_primary");
            }
            Ok(id.map_or(-1, |id| id as i32))
        }
        fn builtin_effect_by_id(&self, id: i32) -> Result<Option<Holder>> {
            self.trace.borrow_mut().push("decode");
            Ok(usize::try_from(id)
                .ok()
                .and_then(|id| self.holders.get(id))
                .cloned())
        }
        fn builtin_effect_by_key(&self, key: &CookieKey) -> Result<Option<Holder>> {
            Ok(self.holders.iter().find(|actual| actual.0 == *key).cloned())
        }
        fn builtin_effect_key(&self, effect: &Holder) -> Result<Option<CookieKey>> {
            Ok(Some(effect.0.clone()))
        }
        fn is_valid_beacon_effect(&self, effect: &Holder) -> Result<bool> {
            Ok(self
                .holders
                .iter()
                .any(|actual| Arc::ptr_eq(actual, effect)))
        }
    }
    struct Constants(Arc<Vec<Holder>>);
    impl BeaconClassEffects<Holder> for Constants {
        fn tier(&self, index: usize) -> Result<&[Holder]> {
            Ok(match index {
                0 => &self.0[0..2],
                1 => &self.0[2..4],
                2 => &self.0[4..5],
                _ => &self.0[5..6],
            })
        }
        fn contains(&self, effect: &Holder) -> Result<bool> {
            Ok(self.0.iter().any(|actual| Arc::ptr_eq(actual, effect)))
        }
    }
    struct Initializer(Arc<Vec<Holder>>);
    impl BeaconClassInitializer<Holder, Arc<&'static str>> for Initializer {
        fn initialize_effect_constants(&mut self) -> Result<Arc<dyn BeaconClassEffects<Holder>>> {
            Ok(Arc::new(Constants(self.0.clone())))
        }
        fn translatable_default_name(&mut self, key: &'static str) -> Result<Arc<&'static str>> {
            ensure!(key == "container.beacon", "wrong controlled name request");
            Ok(Arc::new(key))
        }
    }
    struct Level;
    type Entity =
        BeaconBlockEntity<Arc<()>, Arc<Level>, NativeEmptyComponentMap, Holder, Arc<&'static str>>;
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
    struct CapturedSlot {
        index: i32,
        entity: *const Entity,
    }

    // Controlled callback/slot/registry fixture, not actual Main DataSlot,
    // ContainerData, Level/RNG/broadcast/listener/index owners. The BE/base,
    // EMPTY, payment BeaconState and scan kernel below are real implementations.
    // Pointer checks bind the same retained fixture objects, never a guessed
    // boolean/revision or a newly resolved current menu/coordinate owner.
    struct Owner {
        entity: Box<Entity>,
        retained_entity: *const Entity,
        payment: Box<BeaconState>,
        retained_menu: *const BeaconState,
        captured_slots: Vec<CapturedSlot>,
        captured_access: Arc<Level>,
        replacement_level: Arc<Level>,
        registry: Registry,
        callback_takes_payment: bool,
        callback_owned: Vec<ItemStack>,
        fail_access_after_remove: bool,
    }
    impl Owner {
        fn retained_entity(&self) -> Result<()> {
            ensure!(
                std::ptr::eq(&*self.entity, self.retained_entity),
                "controlled retained BE changed"
            );
            Ok(())
        }
        fn record(&self, event: &'static str) {
            self.registry.trace.borrow_mut().push(event);
        }
    }
    impl BeaconEntityDataHost for Owner {
        type Type = Arc<()>;
        type Level = Arc<Level>;
        type Components = NativeEmptyComponentMap;
        type Effect = Holder;
        type Name = Arc<&'static str>;
        type Registry = Registry;
        fn entity(&self) -> &Entity {
            &self.entity
        }
        fn entity_mut(&mut self) -> &mut Entity {
            &mut self.entity
        }
        fn require_actual_live(&self, _: BeaconAction) -> Result<()> {
            self.retained_entity()
        }
        fn effect_registry(&self) -> Result<Registry> {
            Ok(self.registry.clone())
        }
        fn is_client_side(&self, _: &Arc<Level>) -> Result<bool> {
            Ok(false)
        }
        fn play_power_select_sound(&mut self, _: Arc<Level>, _: BlockPosition) -> Result<()> {
            self.record("primary_sound");
            if self.callback_takes_payment {
                let removed = self.payment.replace_payment_raw(ItemStack::empty())?;
                self.callback_owned.push(removed);
            }
            self.entity.base.set_removed();
            self.entity
                .base
                .set_level(Some(self.replacement_level.clone()));
            Ok(())
        }
    }
    impl BeaconMenuLiveHost for Owner {
        fn require_actual_retained_menu(&self, _: BeaconMenuLiveAction) -> Result<()> {
            self.retained_entity()?;
            ensure!(
                std::ptr::eq(&*self.payment, self.retained_menu),
                "controlled retained menu changed"
            );
            Ok(())
        }
        fn payment_has_item(&self) -> Result<bool> {
            self.record("has_payment");
            Ok(self.payment.has_payment())
        }
        fn remove_current_payment_one(&mut self) -> Result<()> {
            self.record("remove_current");
            if self.payment.has_payment() {
                let _source_consumed = self.payment.replace_payment_raw(ItemStack::empty())?;
                self.record("simple_changed");
            }
            Ok(())
        }
        fn captured_menu_block_entity_changed(&mut self) -> Result<()> {
            ensure!(
                !Arc::ptr_eq(&self.captured_access, self.entity.base.level().unwrap()),
                "controlled access redirected to reattached Level"
            );
            self.record("captured_dirty");
            ensure!(
                !self.fail_access_after_remove,
                "controlled access failure after source removal"
            );
            Ok(())
        }
        fn set_captured_menu_data_slot(&mut self, index: i32, value: i32) -> Result<()> {
            self.record("data_slot_get");
            let index = usize::try_from(index).context("controlled negative DataSlot index")?;
            let slot = self
                .captured_slots
                .get(index)
                .context("controlled DataSlot index out of bounds")?;
            ensure!(
                slot.entity == self.retained_entity,
                "controlled DataSlot lost captured BE"
            );
            let captured_index = slot.index;
            set_data_live(self, captured_index, value)
        }
        fn broadcast_changes(&mut self) -> Result<()> {
            self.record("broadcast");
            Ok(())
        }
    }
    impl BeaconFields for Owner {
        type Effect = Holder;
        fn require_actual_stage(&self, _: BeaconAction) -> Result<()> {
            bail!("controlled live-only fixture")
        }
        fn position(&self) -> BlockPosition {
            self.entity.base.block_position()
        }
        fn is_client_side(&self) -> bool {
            false
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
        fn primary(&self) -> Option<&Holder> {
            self.entity.primary.as_ref()
        }
        fn secondary(&self) -> Option<&Holder> {
            self.entity.secondary.as_ref()
        }
        fn set_primary_raw(&mut self, effect: Option<Holder>) {
            self.entity.primary = effect;
        }
        fn set_secondary_raw(&mut self, effect: Option<Holder>) {
            self.entity.secondary = effect;
        }
        fn same_effect_holder(&self, left: &Holder, right: &Holder) -> Result<bool> {
            Ok(Arc::ptr_eq(left, right))
        }
    }
    impl BeaconLiveOwner for Owner {
        fn require_actual_live(&self, _: BeaconAction) -> Result<()> {
            self.retained_entity()
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
            _: BeaconEffectApplication<'_, Holder>,
        ) -> Result<()> {
            bail!("unreached controlled effect")
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
    fn live_menu_keeps_primary_callback_and_current_payment_prefixes() -> Result<()> {
        let holders = Arc::new(
            [
                "speed",
                "haste",
                "resistance",
                "jump_boost",
                "strength",
                "regeneration",
            ]
            .into_iter()
            .map(|key| Ok(Arc::new(Effect(CookieKey::parse(key)?))))
            .collect::<Result<Vec<_>>>()?,
        );
        let registry = Registry {
            holders: holders.clone(),
            fail_secondary_encoding: Rc::new(Cell::new(true)),
            trace: Rc::new(RefCell::new(Vec::new())),
        };
        let mut class = BeaconClass::uninitialized();
        class.initialize(&mut Initializer(holders.clone()))?;
        let mut entity = Box::new(BeaconBlockEntity::construct(
            (8, 0, 9),
            0,
            Arc::new(class),
            &mut Factory(Arc::new(())),
        )?);
        let captured_access = Arc::new(Level);
        entity.base.set_level(Some(captured_access.clone()));
        entity.scan.after_set_level(0);
        let retained_entity = &*entity as *const Entity;
        let mut payment = Box::new(BeaconState::default());
        payment.replace_payment_raw(ItemStack::new(
            RecipeBook::vanilla().item_id("emerald").unwrap(),
            1,
        )?)?;
        let retained_menu = &*payment as *const BeaconState;
        let mut owner = Owner {
            entity,
            retained_entity,
            payment,
            retained_menu,
            captured_slots: (0..3)
                .map(|index| CapturedSlot {
                    index,
                    entity: retained_entity,
                })
                .collect(),
            captured_access,
            replacement_level: Arc::new(Level),
            registry,
            callback_takes_payment: true,
            callback_owned: Vec::new(),
            fail_access_after_remove: false,
        };
        beacon_block_entity::tick_live(&mut owner)?;
        assert!(!owner.entity.scan.committed_sections().is_empty());
        let primary = Some(holders[0].clone());
        let secondary = Some(holders[1].clone());
        assert!(update_effects_live(&mut owner, primary.clone(), secondary.clone()).is_err());
        assert_eq!(
            &*owner.registry.trace.borrow(),
            &[
                "has_payment",
                "encode_primary",
                "primary_sound",
                "decode",
                "encode_secondary"
            ]
        );
        assert!(Arc::ptr_eq(
            owner.entity.primary.as_ref().unwrap(),
            &holders[0]
        ));
        assert!(owner.entity.secondary.is_none());
        assert!(!owner.payment.has_payment());
        assert_eq!(owner.callback_owned.len(), 1);
        assert_eq!(owner.callback_owned[0].count, 1);
        assert!(owner.entity.base.is_removed());
        owner.registry.trace.borrow_mut().clear();
        assert_eq!(
            update_effects_live(&mut owner, primary.clone(), secondary.clone())?,
            BeaconUpdate::IgnoredNoPayment
        );
        assert_eq!(&*owner.registry.trace.borrow(), &["has_payment"]);

        owner.payment.replace_payment_raw(ItemStack::new(
            RecipeBook::vanilla().item_id("diamond").unwrap(),
            1,
        )?)?;
        owner.callback_takes_payment = false;
        owner.registry.fail_secondary_encoding.set(false);
        owner.fail_access_after_remove = true;
        owner.registry.trace.borrow_mut().clear();
        assert!(update_effects_live(&mut owner, primary, secondary).is_err());
        assert_eq!(
            &*owner.registry.trace.borrow(),
            &[
                "has_payment",
                "encode_primary",
                "primary_sound",
                "decode",
                "encode_secondary",
                "decode",
                "remove_current",
                "simple_changed",
                "captured_dirty"
            ]
        );
        assert!(!owner.payment.has_payment());
        assert!(Arc::ptr_eq(
            owner.entity.secondary.as_ref().unwrap(),
            &holders[1]
        ));

        owner.registry.trace.borrow_mut().clear();
        assert!(set_menu_data_live(&mut owner, -1, 42).is_err());
        assert_eq!(&*owner.registry.trace.borrow(), &["data_slot_get"]);
        owner.registry.trace.borrow_mut().clear();
        set_menu_data_live(&mut owner, 0, 42)?;
        assert_eq!(owner.entity.levels, 42);
        assert_eq!(
            &*owner.registry.trace.borrow(),
            &["data_slot_get", "broadcast"]
        );
        Ok(())
    }
}
