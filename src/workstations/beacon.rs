//! Java 1.21 BeaconMenu's temporary payment and borrowed block-entity data.
//!
//! The actual BeaconBlockEntity retains levels, beam/structure state, effect
//! filtering, effects and callbacks. This menu owns only its one payment stack.
use crate::{container::DroppedStack, inventory::ItemStack};
use anyhow::{ensure, Result};

pub const MENU_TYPE: i32 = 9;
pub const PAYMENT_SLOT: usize = 0;
pub const MENU_SLOTS: usize = 37;
pub const DATA_SLOTS: usize = 3;

/// A registered holder from the real mob-effect registry, not a tier index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BeaconEffect {
    pub registry_id: i32,
}

/// Use the captured actual item tag and mob-effect registry. No local hardcoded
/// payment whitelist or fabricated registry is supplied by the workstation.
pub trait BeaconRegistry {
    fn is_payment_item(&self, stack: &ItemStack) -> Result<bool>;
    fn effect_registered(&self, registry_id: i32) -> Result<bool>;
}

/// Run the real BaseContainerBlockEntity.canUnlock check against the beacon's
/// LockCode, display name and actual player's main-hand item/spectator state.
/// This owner also publishes the source denial message and chest-locked sound.
pub trait BeaconOpenAccess {
    fn can_unlock(&mut self) -> Result<bool>;
}

/// Borrow the opened beacon's actual ContainerData and captured level/player.
/// The outer transaction must stage/roll back this owner's changes on failure.
/// In particular set_data(1) executes BeaconBlockEntity's real beam-dependent
/// selection sound and VALID_EFFECTS filtering before changing primary power;
/// set_data(2) runs its secondary filter. This menu must not duplicate them.
pub trait BeaconHost: BeaconRegistry {
    fn menu_still_valid(&self) -> Result<bool>;
    fn data(&self) -> Result<[i32; DATA_SLOTS]>;
    fn set_data(&mut self, index: usize, encoded_value: i32) -> Result<()>;
    /// Actual Level.blockEntityChanged at the captured block. Called after
    /// payment removal; payment_after is an observation of that source phase.
    fn block_entity_changed(&mut self, payment_after: &ItemStack) -> Result<()>;
}

#[derive(Clone, Debug, Default)]
pub struct BeaconState {
    payment: ItemStack,
    revision: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeaconUpdate {
    IgnoredInvalidMenu,
    IgnoredNoPayment,
    Updated,
}

/// A non-cloneable close plan. Staging it does not create an owned drop or
/// consume the canonical payment. The actual host prepares its Player.drop
/// transaction before committing this plan with the same menu state.
#[derive(Debug)]
pub struct BeaconClosePlan {
    payment: ItemStack,
    revision: u64,
}
impl BeaconClosePlan {
    pub fn payment(&self) -> &ItemStack {
        &self.payment
    }
    /// BeaconMenu.removed does not return payment to inventory, including for
    /// a living connected player. Player.drop(stack,false) expands to the
    /// three-argument drop with around=false and trace_item=false.
    pub fn commit(self, state: &mut BeaconState) -> Result<Option<DroppedStack>> {
        ensure!(
            state.revision == self.revision && state.payment == self.payment,
            "stale beacon close plan"
        );
        let stack = std::mem::take(&mut state.payment);
        state.revision = state.revision.wrapping_add(1);
        Ok((!stack.is_empty()).then_some(DroppedStack {
            stack,
            around: false,
            trace_item: false,
        }))
    }
}

impl BeaconState {
    /// BeaconBlockEntity.createMenu checks the actual lock owner before it
    /// constructs the temporary payment container or binds ContainerData.
    pub fn open(access: &mut impl BeaconOpenAccess) -> Result<Option<Self>> {
        Ok(access.can_unlock()?.then(Self::default))
    }
    pub fn payment(&self) -> &ItemStack {
        &self.payment
    }
    pub fn has_payment(&self) -> bool {
        !self.payment.is_empty()
    }
    pub fn may_place(stack: &ItemStack, registry: &impl BeaconRegistry) -> Result<bool> {
        Ok(!stack.is_empty() && registry.is_payment_item(stack)?)
    }
    /// Transfer ownership from the caller's actual cursor/inventory into the
    /// one-item temporary container, receiving the previous payment stack.
    pub fn set_payment(
        &mut self,
        stack: ItemStack,
        registry: &impl BeaconRegistry,
    ) -> Result<ItemStack> {
        ensure!(
            stack.is_empty() || (stack.count == 1 && Self::may_place(&stack, registry)?),
            "invalid beacon payment slot/item/count"
        );
        stack.encode_canonical()?;
        self.revision = self.revision.wrapping_add(1);
        Ok(std::mem::replace(
            &mut self.payment,
            if stack.is_empty() {
                ItemStack::empty()
            } else {
                stack
            },
        ))
    }

    /// Data is read from the actual block entity for each synchronization; no
    /// local copy of levels or selected powers becomes authoritative here.
    pub fn data(&self, host: &impl BeaconHost) -> Result<[i32; DATA_SLOTS]> {
        host.data()
    }

    /// ServerboundSetBeaconPacket requires the active menu to be BeaconMenu;
    /// dispatch must establish that identity before calling this method. The
    /// handler checks stillValid, then updateEffects only checks payment.
    /// There is no creative bypass, level-tier check or combination check here.
    pub fn update_effects(
        &mut self,
        primary: Option<BeaconEffect>,
        secondary: Option<BeaconEffect>,
        host: &mut impl BeaconHost,
    ) -> Result<BeaconUpdate> {
        if !host.menu_still_valid()? {
            return Ok(BeaconUpdate::IgnoredInvalidMenu);
        }
        if !self.has_payment() {
            return Ok(BeaconUpdate::IgnoredNoPayment);
        }
        let primary = encode_effect(primary, host)?;
        let secondary = encode_effect(secondary, host)?;
        let mut next = self.clone();
        // Actual ContainerData setters own their sound/filter callbacks.
        host.set_data(1, primary)?;
        host.set_data(2, secondary)?;
        next.payment = ItemStack::empty();
        next.revision = next.revision.wrapping_add(1);
        host.block_entity_changed(&next.payment)?;
        *self = next;
        Ok(BeaconUpdate::Updated)
    }

    /// BeaconMenu.setData invokes the actual data setter and then broadcasts.
    /// The adapter must immediately broadcast the returned real data snapshot.
    pub fn set_data(
        &mut self,
        index: usize,
        value: i32,
        host: &mut impl BeaconHost,
    ) -> Result<[i32; DATA_SLOTS]> {
        ensure!(index < DATA_SLOTS, "invalid beacon data index");
        host.set_data(index, value)?;
        host.data()
    }

    /// Source quickMoveStack only inserts a payment from an entire count-one
    /// player stack into an empty payment slot. Larger stacks move between
    /// main inventory/hotbar instead of splitting off one payment.
    pub fn quick_move_target(
        &self,
        slot: usize,
        stack: &ItemStack,
        registry: &impl BeaconRegistry,
    ) -> Result<Option<(std::ops::Range<usize>, bool)>> {
        if slot >= MENU_SLOTS {
            return Ok(None);
        }
        if slot == PAYMENT_SLOT {
            return Ok(Some((1..37, true)));
        }
        if !self.has_payment() && stack.count == 1 && Self::may_place(stack, registry)? {
            return Ok(Some((0..1, false)));
        }
        if slot < 28 {
            Ok(Some((28..37, false)))
        } else {
            Ok(Some((1..28, false)))
        }
    }

    /// Call only on the server after AbstractContainerMenu.removed has handled
    /// the real cursor. Prepare the real Player.drop effects, then commit.
    pub fn prepare_close(&self) -> BeaconClosePlan {
        BeaconClosePlan {
            payment: self.payment.clone(),
            revision: self.revision,
        }
    }
}

pub fn encode_effect(effect: Option<BeaconEffect>, registry: &impl BeaconRegistry) -> Result<i32> {
    let Some(effect) = effect else {
        return Ok(0);
    };
    ensure!(
        effect.registry_id >= 0 && registry.effect_registered(effect.registry_id)?,
        "unregistered beacon effect holder"
    );
    effect
        .registry_id
        .checked_add(1)
        .ok_or_else(|| anyhow::anyhow!("beacon effect ID overflow"))
}

/// Data value zero means absent; an unresolved/out-of-range holder decodes to
/// absent as the original registry's byId lookup does. Unavailable registry
/// ownership must return an error instead of claiming an absent holder.
pub fn decode_effect(encoded: i32, registry: &impl BeaconRegistry) -> Result<Option<BeaconEffect>> {
    let Some(id) = encoded.checked_sub(1).filter(|id| *id >= 0) else {
        return Ok(None);
    };
    Ok(registry
        .effect_registered(id)?
        .then_some(BeaconEffect { registry_id: id }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{crafting::RecipeBook, inventory};
    struct Host {
        valid: bool,
        data: [i32; 3],
        calls: Vec<String>,
        fail_secondary: bool,
    }
    impl BeaconRegistry for Host {
        fn is_payment_item(&self, stack: &ItemStack) -> Result<bool> {
            Ok(inventory::item_name(stack.item_id) == Some("emerald"))
        }
        fn effect_registered(&self, id: i32) -> Result<bool> {
            Ok((0..8).contains(&id))
        }
    }
    impl BeaconHost for Host {
        fn menu_still_valid(&self) -> Result<bool> {
            Ok(self.valid)
        }
        fn data(&self) -> Result<[i32; 3]> {
            Ok(self.data)
        }
        fn set_data(&mut self, index: usize, value: i32) -> Result<()> {
            ensure!(
                index != 2 || !self.fail_secondary,
                "test secondary owner unavailable"
            );
            self.calls.push(format!("set:{index}:{value}"));
            self.data[index] = value;
            Ok(())
        }
        fn block_entity_changed(&mut self, payment: &ItemStack) -> Result<()> {
            assert!(payment.is_empty());
            self.calls.push("dirty".into());
            Ok(())
        }
    }
    fn host() -> Host {
        Host {
            valid: true,
            data: [0; 3],
            calls: vec![],
            fail_secondary: false,
        }
    }
    fn emerald(count: i32) -> ItemStack {
        ItemStack::new(RecipeBook::vanilla().item_id("emerald").unwrap(), count).unwrap()
    }
    #[test]
    fn selection_uses_real_data_order_without_invented_tier_check() -> Result<()> {
        let mut host = host();
        let mut state = BeaconState::default();
        assert_eq!(
            state.update_effects(None, None, &mut host)?,
            BeaconUpdate::IgnoredNoPayment
        );
        assert!(state.set_payment(emerald(2), &host).is_err());
        state.set_payment(emerald(1), &host)?;
        assert_eq!(
            state.quick_move_target(1, &emerald(2), &host)?,
            Some((28..37, false))
        );
        let effect = Some(BeaconEffect { registry_id: 3 });
        assert_eq!(
            state.update_effects(effect, None, &mut host)?,
            BeaconUpdate::Updated
        );
        assert_eq!(host.calls, ["set:1:4", "set:2:0", "dirty"]);
        assert_eq!(state.data(&host)?, [0, 4, 0]);
        assert!(!state.has_payment());
        assert_eq!(decode_effect(4, &host)?, effect);
        Ok(())
    }
    #[test]
    fn invalid_menu_keeps_payment_and_close_drops_even_when_alive() -> Result<()> {
        let mut host = host();
        let mut state = BeaconState::default();
        state.set_payment(emerald(1), &host)?;
        host.valid = false;
        assert_eq!(
            state.update_effects(None, None, &mut host)?,
            BeaconUpdate::IgnoredInvalidMenu
        );
        assert!(host.calls.is_empty());
        let plan = state.prepare_close();
        assert!(state.has_payment());
        let dropped = plan.commit(&mut state)?.unwrap();
        assert_eq!(dropped.stack.count, 1);
        assert!(!dropped.around && !dropped.trace_item);
        assert!(!state.has_payment());
        assert!(state.prepare_close().commit(&mut state)?.is_none());
        Ok(())
    }
    #[test]
    fn host_failure_leaves_menu_payment_unconsumed() -> Result<()> {
        let mut host = host();
        let mut state = BeaconState::default();
        state.set_payment(emerald(1), &host)?;
        host.fail_secondary = true;
        assert!(state.update_effects(None, None, &mut host).is_err());
        assert!(state.has_payment());
        // The real host must roll back its staged callbacks as its contract
        // requires. This test only verifies the independently owned payment.
        assert_eq!(host.calls, ["set:1:0"]);
        Ok(())
    }
}
