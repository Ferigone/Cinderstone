//! Java 1.21 merchant costs, payment/result ownership and staged callbacks.
//!
//! Offers, their object handles, the merchant and the trading player belong to
//! the actual host. No villager, offer generator or second live inventory is
//! constructed here. A MerchantStage is a PRIVATE stage of those real owners.
use crate::inventory::{Component, ItemStack, PlayerInventory};
use anyhow::{ensure, Result};
use std::fmt::Debug;

pub const MENU_TYPE: i32 = 19;
pub const MENU_SLOTS: usize = 39;

/// ItemCost requires only the listed components. Extra input components do not
/// make a payment fail. `stack` is the source display/cost stack with the actual
/// item prototype and expected component patch, not an owned payment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemCost {
    stack: ItemStack,
    expected: Vec<Component>,
}
impl ItemCost {
    pub fn new(item_id: i32, count: i32, expected: Vec<Component>) -> Result<Self> {
        ensure!(count > 0, "invalid merchant ItemCost count");
        let mut stack = ItemStack::new(item_id, count)?;
        ensure!(!stack.is_empty(), "merchant ItemCost cannot require air");
        let mut seen = std::collections::BTreeSet::new();
        for component in &expected {
            // FoodProperties can contain an ItemStack whose equality is Java
            // reference identity. Wire bytes do not establish that owner.
            ensure!(
                component.type_id != 20,
                "merchant food-component predicate requires unresolved ItemStack identity context"
            );
            ensure!(
                (0..57).contains(&component.type_id) && seen.insert(component.type_id),
                "invalid/duplicate merchant component predicate"
            );
        }
        stack.components = expected.clone();
        for component in &expected {
            stack.component_value(component.type_id)?;
        }
        stack.normalize_patch();
        Ok(Self { stack, expected })
    }
    pub fn stack(&self) -> &ItemStack {
        &self.stack
    }
    pub fn count(&self) -> i32 {
        self.stack.count
    }
    pub fn test(&self, candidate: &ItemStack) -> Result<bool> {
        if candidate.is_empty() || candidate.item_id != self.stack.item_id {
            return Ok(false);
        }
        for component in &self.expected {
            if candidate.component_value(component.type_id)?
                != self.stack.component_value(component.type_id)?
            {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

/// Value view of an actual MerchantOffer. Uses/demand/discount mutations remain
/// with that offer's owner; an active offer is addressed by its real host handle.
#[derive(Clone, Debug, PartialEq)]
pub struct MerchantOffer {
    pub cost_a: ItemCost,
    pub cost_b: Option<ItemCost>,
    pub result: ItemStack,
    pub uses: i32,
    pub max_uses: i32,
    pub reward_exp: bool,
    pub special_price_diff: i32,
    pub demand: i32,
    pub price_multiplier: f32,
    pub xp: i32,
}
impl MerchantOffer {
    pub fn modified_cost_count(&self) -> Result<i32> {
        // Java overflows the INT product before promotion, multiplies as float,
        // floors as Mth.floor(float), then wraps the additions before clamp.
        let demand_cost =
            self.cost_a.count().wrapping_mul(self.demand) as f32 * self.price_multiplier;
        let surcharge = java_floor_f32(demand_cost).max(0);
        let value = self
            .cost_a
            .count()
            .wrapping_add(surcharge)
            .wrapping_add(self.special_price_diff);
        let maximum = self.cost_a.stack.max_stack_size()?;
        ensure!(maximum >= 1, "invalid merchant cost stack maximum");
        Ok(value.max(1).min(maximum))
    }
    pub fn cost_a_stack(&self) -> Result<ItemStack> {
        let mut stack = self.cost_a.stack.clone();
        stack.count = self.modified_cost_count()?;
        Ok(stack)
    }
    pub fn is_out_of_stock(&self) -> bool {
        self.uses >= self.max_uses
    }
    pub fn update_demand(&mut self) {
        self.demand = self
            .demand
            .wrapping_add(self.uses)
            .wrapping_sub(self.max_uses.wrapping_sub(self.uses));
    }
    pub fn satisfied_by(&self, first: &ItemStack, second: &ItemStack) -> Result<bool> {
        if !self.cost_a.test(first)? || first.count < self.modified_cost_count()? {
            return Ok(false);
        }
        match &self.cost_b {
            Some(cost) => Ok(cost.test(second)? && second.count >= cost.count()),
            None => Ok(second.is_empty()),
        }
    }
    /// Does not check stock or increment uses: MerchantOffer.take does neither.
    pub fn take(&self, first: &mut ItemStack, second: &mut ItemStack) -> Result<bool> {
        if !self.satisfied_by(first, second)? {
            return Ok(false);
        }
        shrink(first, self.modified_cost_count()?);
        if let Some(cost) = &self.cost_b {
            shrink(second, cost.count());
        }
        Ok(true)
    }
}
fn java_floor_f32(value: f32) -> i32 {
    let integer = value as i32;
    if value < integer as f32 {
        integer.wrapping_sub(1)
    } else {
        integer
    }
}

#[derive(Clone, Debug)]
pub struct OfferEntry<Key> {
    pub key: Key,
    pub offer: MerchantOffer,
}

/// Private transactional view of the ACTUAL merchant, offers, trading player,
/// world, statistics, advancement and RNG owners. Binding identifies the actual
/// merchant AND player; StageKey identifies this particular private transaction.
/// Actual offer object handles must survive an ordered-list replacement.
/// Each callback synchronously updates this stage, so subsequent offer/XP reads
/// observe its effects. `publish` must atomically validate/publish all real-owner
/// changes (including staged output destination); on error publish nothing.
pub trait MerchantStage {
    type Binding: Clone + Eq + Debug;
    type OfferKey: Clone + Eq + Debug;
    type StageKey: Clone + Eq + Debug;
    fn binding(&self) -> Self::Binding;
    fn stage_key(&self) -> Self::StageKey;
    fn is_trading_player(&self) -> Result<bool>;
    fn offers(&self) -> Result<Vec<OfferEntry<Self::OfferKey>>>;
    fn offer(&self, key: &Self::OfferKey) -> Result<MerchantOffer>;
    fn villager_xp(&self) -> Result<i32>;
    fn item_crafted_by(&mut self, stack: &ItemStack, amount: i32) -> Result<()>;
    fn notify_trade(&mut self, key: &Self::OfferKey) -> Result<()>;
    fn notify_trade_updated(&mut self, result: &ItemStack) -> Result<()>;
    fn award_traded_stat(&mut self) -> Result<()>;
    fn override_xp(&mut self, xp: i32) -> Result<()>;
    /// MerchantMenu.quickMoveStack only: actual entity position/sound owner,
    /// neutral source, volume/pitch 1. Server non-entity contexts must reject.
    fn play_trade_sound(&mut self) -> Result<()>;
    fn clear_trading_player(&mut self) -> Result<()>;
    fn publish(&mut self) -> Result<()>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MerchantState<Binding, OfferKey> {
    binding: Binding,
    payments: [ItemStack; 2],
    preview: ItemStack,
    active_offer: Option<OfferKey>,
    selection_hint: i32,
    future_xp: i32,
}
#[derive(Clone, Copy, Debug)]
pub enum ResultTake {
    /// Slot.safeTake path. MerchantContainer.removeItem removes the WHOLE
    /// result, though the crafted count adds min(requested, result count).
    SafeTake { requested: i32 },
    /// Destination has already accepted this actual transfer amount. Source
    /// invokes onQuickCraft(original result, transferred), sound, then onTake
    /// with the remaining source stack and crafted count zero.
    QuickMove { transferred: i32 },
}

#[derive(Debug)]
pub struct MerchantPlan<Binding, OfferKey, StageKey> {
    before: MerchantState<Binding, OfferKey>,
    after: MerchantState<Binding, OfferKey>,
    stage_key: StageKey,
    taken: Vec<ItemStack>,
    inventory: Option<(Vec<ItemStack>, PlayerInventory)>,
}
impl<B, K, T> MerchantPlan<B, K, T> {
    /// Reserved owned stacks, published only after the complete stage commits.
    pub fn taken(&self) -> &[ItemStack] {
        &self.taken
    }
    pub fn next_state(&self) -> &MerchantState<B, K> {
        &self.after
    }
}

impl<B: Clone + Eq + Debug, K: Clone + Eq + Debug> MerchantState<B, K> {
    pub fn new(binding: B) -> Self {
        Self {
            binding,
            payments: [ItemStack::empty(), ItemStack::empty()],
            preview: ItemStack::empty(),
            active_offer: None,
            selection_hint: 0,
            future_xp: 0,
        }
    }
    pub fn payments(&self) -> &[ItemStack; 2] {
        &self.payments
    }
    pub fn preview(&self) -> &ItemStack {
        &self.preview
    }
    pub fn active_offer(&self) -> Option<&K> {
        self.active_offer.as_ref()
    }
    pub fn selection_hint(&self) -> i32 {
        self.selection_hint
    }
    pub fn future_xp(&self) -> i32 {
        self.future_xp
    }
    pub fn still_valid(
        &self,
        stage: &impl MerchantStage<Binding = B, OfferKey = K>,
    ) -> Result<bool> {
        Ok(self.binding == stage.binding() && stage.is_trading_player()?)
    }
    pub fn can_take_item_for_pick_all() -> bool {
        false
    }
    pub fn may_place(slot: usize) -> Result<bool> {
        ensure!(slot < 3, "invalid merchant top slot");
        Ok(slot != 2)
    }
    fn checked_stage(&self, stage: &impl MerchantStage<Binding = B, OfferKey = K>) -> Result<()> {
        ensure!(
            self.still_valid(stage)?,
            "merchant/trading-player binding is no longer valid"
        );
        Ok(())
    }
    fn plan<T>(&self, after: Self, stage_key: T, taken: Vec<ItemStack>) -> MerchantPlan<B, K, T> {
        MerchantPlan {
            before: self.clone(),
            after,
            stage_key,
            taken,
            inventory: None,
        }
    }
    pub fn prepare_payment<S: MerchantStage<Binding = B, OfferKey = K>>(
        &self,
        slot: usize,
        mut stack: ItemStack,
        stage: &mut S,
    ) -> Result<MerchantPlan<B, K, S::StageKey>> {
        self.checked_stage(stage)?;
        ensure!(slot < 2, "invalid merchant payment slot");
        if !stack.is_empty() {
            stack.count = stack.count.min(stack.max_stack_size()?.min(99));
        }
        let mut next = self.clone();
        next.payments[slot] = canonical(stack);
        next.update_sell_item(stage)?;
        Ok(self.plan(next, stage.stage_key(), vec![]))
    }
    pub fn prepare_selection<S: MerchantStage<Binding = B, OfferKey = K>>(
        &self,
        hint: i32,
        stage: &mut S,
    ) -> Result<MerchantPlan<B, K, S::StageKey>> {
        self.checked_stage(stage)?;
        let mut next = self.clone();
        next.selection_hint = hint;
        next.update_sell_item(stage)?;
        Ok(self.plan(next, stage.stage_key(), vec![]))
    }
    pub fn prepare_refresh<S: MerchantStage<Binding = B, OfferKey = K>>(
        &self,
        stage: &mut S,
    ) -> Result<MerchantPlan<B, K, S::StageKey>> {
        self.checked_stage(stage)?;
        let mut next = self.clone();
        next.update_sell_item(stage)?;
        Ok(self.plan(next, stage.stage_key(), vec![]))
    }
    pub fn prepare_remove_payment<S: MerchantStage<Binding = B, OfferKey = K>>(
        &self,
        slot: usize,
        amount: i32,
        stage: &mut S,
    ) -> Result<MerchantPlan<B, K, S::StageKey>> {
        self.checked_stage(stage)?;
        ensure!(slot < 2 && amount > 0, "invalid merchant payment removal");
        let mut next = self.clone();
        let taken = split(&mut next.payments[slot], amount);
        if !taken.is_empty() {
            next.update_sell_item(stage)?;
        }
        Ok(self.plan(
            next,
            stage.stage_key(),
            if taken.is_empty() {
                vec![]
            } else {
                vec![taken]
            },
        ))
    }
    fn update_sell_item<S: MerchantStage<Binding = B, OfferKey = K>>(
        &mut self,
        stage: &mut S,
    ) -> Result<()> {
        self.active_offer = None;
        let (first, second) = if self.payments[0].is_empty() {
            (&self.payments[1], &self.payments[0])
        } else {
            (&self.payments[0], &self.payments[1])
        };
        if first.is_empty() {
            self.preview = ItemStack::empty();
            self.future_xp = 0;
            return Ok(());
        }
        let offers = stage.offers()?;
        if !offers.is_empty() {
            let mut selected = recipe_for(&offers, first, second, self.selection_hint)?;
            if selected
                .as_ref()
                .is_none_or(|entry| entry.offer.is_out_of_stock())
            {
                self.active_offer = selected.as_ref().map(|entry| entry.key.clone());
                selected = recipe_for(&offers, second, first, self.selection_hint)?;
            }
            if let Some(entry) = selected.filter(|entry| !entry.offer.is_out_of_stock()) {
                self.active_offer = Some(entry.key);
                self.preview = limited(entry.offer.result)?;
                self.future_xp = entry.offer.xp;
            } else {
                self.preview = ItemStack::empty();
                self.future_xp = 0;
            }
        }
        // Java leaves an old preview/futureXp when a nonempty payment's actual
        // offer list becomes empty, but clears activeOffer. Such a preview is
        // never promoted into an owned result by this API.
        stage.notify_trade_updated(&self.preview)
    }
    pub fn prepare_take<S: MerchantStage<Binding = B, OfferKey = K>>(
        &self,
        action: ResultTake,
        stage: &mut S,
    ) -> Result<MerchantPlan<B, K, S::StageKey>> {
        self.checked_stage(stage)?;
        ensure!(!self.preview.is_empty(), "empty merchant result");
        ensure!(
            self.active_offer.is_some(),
            "unsupported stale merchant preview without actual offer owner"
        );
        let mut next = self.clone();
        let original = next.preview.clone();
        let (taken, on_take_stack, crafted_amount) = match action {
            ResultTake::SafeTake { requested } => {
                ensure!(requested > 0, "invalid merchant result request");
                next.preview = ItemStack::empty();
                // Slot.tryRemove calls setByPlayer on the now-empty result;
                // Slot.setChanged regenerates it before MerchantResultSlot.onTake.
                next.update_sell_item(stage)?;
                (
                    original.clone(),
                    original.clone(),
                    requested.min(original.count),
                )
            }
            ResultTake::QuickMove { transferred } => {
                ensure!(
                    transferred > 0 && transferred <= original.count,
                    "invalid merchant result transfer"
                );
                let mut remainder = original.clone();
                shrink(&mut remainder, transferred);
                stage.item_crafted_by(&original, transferred)?;
                stage.play_trade_sound()?;
                next.preview = remainder.clone();
                next.update_sell_item(stage)?;
                (counted(&original, transferred), remainder, 0)
            }
        };
        stage.item_crafted_by(&on_take_stack, crafted_amount)?;
        if let Some(key) = next.active_offer.clone() {
            let offer = stage.offer(&key)?;
            let mut payments = next.payments.clone();
            let [first, second] = &mut payments;
            let paid = offer.take(first, second)? || offer.take(second, first)?;
            if paid {
                // Merchant.notifyTrade, not this menu, owns uses, rewards and
                // trade criteria. Subsequent updates read the actual changed offer.
                // Both original payment ItemStacks were shrunk before
                // notifyTrade or either setItem in the original container.
                next.payments = payments;
                stage.notify_trade(&key)?;
                stage.award_traded_stat()?;
                // Each setItem then recomputes the result in source order.
                next.update_sell_item(stage)?;
                next.update_sell_item(stage)?;
            }
            // The source overrideXp is outside the successful-take condition.
            let xp = stage.villager_xp()?.wrapping_add(offer.xp);
            stage.override_xp(xp)?;
        }
        Ok(self.plan(next, stage.stage_key(), vec![taken]))
    }
    /// Server select-trade packet order: setSelectionHint, then tryMoveItems.
    /// Old payments return to borrowed inventory in reverse menu-slot order;
    /// if either cannot move, already-completed returns remain part of this
    /// stage. Filling scans ordinary inventory then hotbar and gathers matching
    /// components up to the item's stack maximum, not merely the recipe cost.
    pub fn prepare_selection_and_autofill<S: MerchantStage<Binding = B, OfferKey = K>>(
        &self,
        hint: i32,
        inventory: &PlayerInventory,
        stage: &mut S,
    ) -> Result<MerchantPlan<B, K, S::StageKey>> {
        self.checked_stage(stage)?;
        let mut next = self.clone();
        let mut work = inventory.clone();
        next.selection_hint = hint;
        next.update_sell_item(stage)?;
        let offers = stage.offers()?;
        if hint >= 0 && (hint as usize) < offers.len() {
            let mut returned = true;
            for slot in 0..2 {
                let mut payment = next.payments[slot].clone();
                if !payment.is_empty() {
                    if !move_to_player_inventory(&mut payment, &mut work)? {
                        returned = false;
                        break;
                    }
                    next.payments[slot] = payment;
                    next.update_sell_item(stage)?;
                }
            }
            if returned && next.payments.iter().all(ItemStack::is_empty) {
                let current_offers = stage.offers()?;
                let offer = &current_offers
                    .get(hint as usize)
                    .ok_or_else(|| anyhow::anyhow!("merchant offer list changed during autofill"))?
                    .offer;
                next.fill_payment(0, &offer.cost_a, &mut work, stage)?;
                if let Some(cost) = &offer.cost_b {
                    next.fill_payment(1, cost, &mut work, stage)?;
                }
            }
        }
        let mut plan = self.plan(next, stage.stage_key(), vec![]);
        plan.inventory = Some((inventory.slots().to_vec(), work));
        Ok(plan)
    }
    fn fill_payment<S: MerchantStage<Binding = B, OfferKey = K>>(
        &mut self,
        payment_slot: usize,
        cost: &ItemCost,
        inventory: &mut PlayerInventory,
        stage: &mut S,
    ) -> Result<()> {
        for slot in 9..45 {
            let mut source = inventory.slot(slot).expect("canonical player slot").clone();
            if source.is_empty() || !cost.test(&source)? {
                continue;
            }
            let current = &self.payments[payment_slot];
            if !current.is_empty() && !source.same_item_same_components(current)? {
                continue;
            }
            let maximum = source.max_stack_size()?;
            let amount = (maximum - current.count).min(source.count);
            ensure!(
                amount >= 0,
                "unsupported overfull merchant payment during autofill"
            );
            let payment = counted(&source, current.count + amount);
            shrink(&mut source, amount);
            inventory.set_slot_authoritative(slot, source);
            self.payments[payment_slot] = limited(payment)?;
            self.update_sell_item(stage)?;
            if self.payments[payment_slot].count >= maximum {
                break;
            }
        }
        Ok(())
    }

    /// Host must process AbstractContainerMenu.removed's cursor first. This
    /// stages merchant.setTradingPlayer(null), then removes ONLY two payments.
    /// Caller returns/drops these by actual alive/disconnected ServerPlayer path.
    pub fn prepare_close<S: MerchantStage<Binding = B, OfferKey = K>>(
        &self,
        stage: &mut S,
    ) -> Result<MerchantPlan<B, K, S::StageKey>> {
        ensure!(
            self.binding == stage.binding(),
            "merchant close owner mismatch"
        );
        stage.clear_trading_player()?;
        let mut next = self.clone();
        let taken = std::mem::take(&mut next.payments)
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect();
        next.preview = ItemStack::empty();
        next.active_offer = None;
        next.future_xp = 0;
        Ok(self.plan(next, stage.stage_key(), taken))
    }
    pub fn commit<S: MerchantStage<Binding = B, OfferKey = K>>(
        &mut self,
        plan: MerchantPlan<B, K, S::StageKey>,
        inventory: Option<&mut PlayerInventory>,
        stage: &mut S,
    ) -> Result<Vec<ItemStack>> {
        ensure!(
            *self == plan.before
                && self.binding == stage.binding()
                && plan.stage_key == stage.stage_key(),
            "stale/rebound merchant transaction"
        );
        match (&plan.inventory, inventory.as_ref()) {
            (Some((before, _)), Some(actual)) => ensure!(
                actual.slots() == before,
                "merchant player inventory changed during preparation"
            ),
            (Some(_), None) => {
                anyhow::bail!("merchant transaction requires actual player inventory")
            }
            _ => {}
        }
        stage.publish()?;
        if let (Some((_, after)), Some(actual)) = (plan.inventory, inventory) {
            for slot in 9..45 {
                actual.set_slot_authoritative(
                    slot,
                    after.slot(slot).expect("canonical player slot").clone(),
                );
            }
        }
        *self = plan.after;
        Ok(plan.taken)
    }
}

fn recipe_for<K: Clone>(
    offers: &[OfferEntry<K>],
    first: &ItemStack,
    second: &ItemStack,
    hint: i32,
) -> Result<Option<OfferEntry<K>>> {
    if hint > 0 && (hint as usize) < offers.len() {
        let entry = &offers[hint as usize];
        return Ok(entry
            .offer
            .satisfied_by(first, second)?
            .then(|| entry.clone()));
    }
    for entry in offers {
        if entry.offer.satisfied_by(first, second)? {
            return Ok(Some(entry.clone()));
        }
    }
    Ok(None)
}
fn limited(mut stack: ItemStack) -> Result<ItemStack> {
    if !stack.is_empty() {
        stack.count = stack.count.min(stack.max_stack_size()?.min(99));
    }
    Ok(canonical(stack))
}
fn canonical(stack: ItemStack) -> ItemStack {
    if stack.is_empty() {
        ItemStack::empty()
    } else {
        stack
    }
}
fn counted(stack: &ItemStack, count: i32) -> ItemStack {
    let mut stack = stack.clone();
    stack.count = count;
    canonical(stack)
}
fn shrink(stack: &mut ItemStack, amount: i32) {
    stack.count = stack.count.wrapping_sub(amount);
    if stack.is_empty() {
        *stack = ItemStack::empty();
    }
}
fn split(stack: &mut ItemStack, amount: i32) -> ItemStack {
    let taken = counted(stack, amount.min(stack.count).max(0));
    shrink(stack, taken.count);
    taken
}

// MerchantMenu.tryMoveItems uses moveItemStackTo(3,39,true). This helper is
// limited to that exact borrowed player-inventory range and ordinary slot rules.
fn move_to_player_inventory(
    stack: &mut ItemStack,
    inventory: &mut PlayerInventory,
) -> Result<bool> {
    let before = stack.count;
    if stack.is_stackable()? {
        for slot in (9..45).rev() {
            let old = inventory.slot(slot).expect("canonical player slot");
            if !old.is_empty() && old.same_item_same_components(stack)? {
                let amount = stack
                    .count
                    .min((stack.max_stack_size()?.min(99) - old.count).max(0));
                if amount > 0 {
                    inventory.set_slot_authoritative(slot, counted(old, old.count + amount));
                    shrink(stack, amount);
                }
                if stack.is_empty() {
                    break;
                }
            }
        }
    }
    if !stack.is_empty() {
        for slot in (9..45).rev() {
            if inventory
                .slot(slot)
                .expect("canonical player slot")
                .is_empty()
            {
                let amount = stack.count.min(stack.max_stack_size()?.min(99));
                inventory.set_slot_authoritative(slot, counted(stack, amount));
                shrink(stack, amount);
                break;
            }
        }
    }
    Ok(stack.count != before)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{crafting::RecipeBook, protocol::Writer};
    fn item(name: &str, count: i32) -> ItemStack {
        ItemStack::new(RecipeBook::vanilla().item_id(name).unwrap(), count).unwrap()
    }
    fn cost(name: &str, count: i32) -> ItemCost {
        ItemCost::new(item(name, 1).item_id, count, vec![]).unwrap()
    }
    fn offer() -> MerchantOffer {
        MerchantOffer {
            cost_a: cost("emerald", 2),
            cost_b: Some(cost("coal", 1)),
            result: item("bread", 3),
            uses: 0,
            max_uses: 2,
            reward_exp: true,
            special_price_diff: -1,
            demand: 0,
            price_multiplier: 0.,
            xp: 5,
        }
    }
    // Explicit fixture stage, not a native merchant/villager implementation.
    // Mutations here are private staging changes until publish succeeds.
    #[derive(Clone)]
    struct FixtureStage {
        binding: (u64, u64),
        key: u64,
        offers: Vec<OfferEntry<usize>>,
        xp: i32,
        valid: bool,
        events: Vec<String>,
        published: usize,
        reject: bool,
    }
    impl FixtureStage {
        fn new(offers: Vec<MerchantOffer>) -> Self {
            Self {
                binding: (7, 9),
                key: 1,
                offers: offers
                    .into_iter()
                    .enumerate()
                    .map(|(key, offer)| OfferEntry { key, offer })
                    .collect(),
                xp: 0,
                valid: true,
                events: vec![],
                published: 0,
                reject: false,
            }
        }
    }
    impl MerchantStage for FixtureStage {
        type Binding = (u64, u64);
        type OfferKey = usize;
        type StageKey = u64;
        fn binding(&self) -> Self::Binding {
            self.binding
        }
        fn stage_key(&self) -> u64 {
            self.key
        }
        fn is_trading_player(&self) -> Result<bool> {
            Ok(self.valid)
        }
        fn offers(&self) -> Result<Vec<OfferEntry<usize>>> {
            Ok(self.offers.clone())
        }
        fn offer(&self, key: &usize) -> Result<MerchantOffer> {
            Ok(self
                .offers
                .iter()
                .find(|e| e.key == *key)
                .unwrap()
                .offer
                .clone())
        }
        fn villager_xp(&self) -> Result<i32> {
            Ok(self.xp)
        }
        fn item_crafted_by(&mut self, stack: &ItemStack, amount: i32) -> Result<()> {
            self.events
                .push(format!("crafted:{}:{amount}", stack.count));
            Ok(())
        }
        fn notify_trade(&mut self, key: &usize) -> Result<()> {
            self.events.push("trade".into());
            self.offers
                .iter_mut()
                .find(|e| e.key == *key)
                .unwrap()
                .offer
                .uses += 1;
            Ok(())
        }
        fn notify_trade_updated(&mut self, result: &ItemStack) -> Result<()> {
            self.events.push(format!("updated:{}", result.count));
            Ok(())
        }
        fn award_traded_stat(&mut self) -> Result<()> {
            self.events.push("stat".into());
            Ok(())
        }
        fn override_xp(&mut self, xp: i32) -> Result<()> {
            self.events.push(format!("xp:{xp}"));
            self.xp = xp;
            Ok(())
        }
        fn play_trade_sound(&mut self) -> Result<()> {
            self.events.push("sound".into());
            Ok(())
        }
        fn clear_trading_player(&mut self) -> Result<()> {
            self.events.push("clear-player".into());
            self.valid = false;
            Ok(())
        }
        fn publish(&mut self) -> Result<()> {
            ensure!(!self.reject, "fixture transaction rejected");
            self.published += 1;
            self.key += 1;
            Ok(())
        }
    }
    fn payment(
        state: &mut MerchantState<(u64, u64), usize>,
        slot: usize,
        stack: ItemStack,
        stage: &mut FixtureStage,
    ) -> Result<()> {
        let plan = state.prepare_payment(slot, stack, stage)?;
        assert!(state.commit(plan, None, stage)?.is_empty());
        Ok(())
    }
    #[test]
    fn component_subset_price_and_stock_selection_facts() -> Result<()> {
        let mut writer = Writer::new();
        writer.varint(4);
        let required = Component {
            type_id: 16,
            data: writer.into_inner(),
        };
        let cost = ItemCost::new(item("emerald", 1).item_id, 4, vec![required.clone()])?;
        let mut input = item("emerald", 10);
        input.components.push(required);
        let mut extra = Writer::new();
        extra.varint(2);
        input.components.push(Component {
            type_id: 3,
            data: extra.into_inner(),
        });
        assert!(cost.test(&input)?);
        assert!(!cost.test(&item("emerald", 10))?);
        let mut value = offer();
        value.cost_a = cost;
        value.demand = 3;
        value.price_multiplier = 0.2;
        value.special_price_diff = 0;
        assert_eq!(value.modified_cost_count()?, 6);
        value.price_multiplier = f32::NAN;
        assert_eq!(value.modified_cost_count()?, 4);
        value.price_multiplier = f32::INFINITY;
        assert_eq!(value.modified_cost_count()?, 1);
        value.price_multiplier = f32::NEG_INFINITY;
        assert_eq!(value.modified_cost_count()?, 1);
        value.price_multiplier = 0.2;
        value.special_price_diff = -20;
        assert_eq!(value.modified_cost_count()?, 1);
        value.uses = 7;
        value.max_uses = 12;
        value.demand = 3;
        value.update_demand();
        assert_eq!(value.demand, 5);
        let mut first = offer();
        first.cost_b = None;
        first.uses = first.max_uses;
        let mut second = first.clone();
        second.uses = 0;
        let mut stage = FixtureStage::new(vec![first, second]);
        let mut state = MerchantState::new(stage.binding);
        payment(&mut state, 0, item("emerald", 5), &mut stage)?;
        assert!(state.preview().is_empty()); // hint0 finds exhausted first offer
        let plan = state.prepare_selection(1, &mut stage)?;
        state.commit(plan, None, &mut stage)?;
        assert_eq!(state.active_offer(), Some(&1));
        assert_eq!(state.preview().count, 3);
        Ok(())
    }
    #[test]
    fn swapped_payments_whole_result_and_ordered_real_owner_obligations() -> Result<()> {
        let mut stage = FixtureStage::new(vec![offer()]);
        let mut state = MerchantState::new(stage.binding);
        payment(&mut state, 0, item("coal", 1), &mut stage)?;
        payment(&mut state, 1, item("emerald", 5), &mut stage)?;
        assert_eq!(state.future_xp(), 5);
        stage.events.clear();
        let plan = state.prepare_take(ResultTake::SafeTake { requested: 1 }, &mut stage)?;
        assert_eq!(plan.taken()[0].count, 3); // MerchantContainer removes all result
        assert_eq!(state.payments()[1].count, 5); // actual state still uncommitted
        let taken = state.commit(plan, None, &mut stage)?;
        assert_eq!(taken[0], item("bread", 3));
        assert_eq!(state.payments()[1].count, 4);
        assert!(state.payments()[0].is_empty());
        assert_eq!(
            stage.events,
            vec![
                "updated:3",
                "crafted:3:1",
                "trade",
                "stat",
                "updated:0",
                "updated:0",
                "xp:5"
            ]
        );
        assert_eq!(stage.offers[0].offer.uses, 1);
        assert_eq!(stage.xp, 5);
        // Close removes only payment ownership, after the merchant player clears.
        stage.events.clear();
        let plan = state.prepare_close(&mut stage)?;
        let taken = state.commit(plan, None, &mut stage)?;
        assert_eq!(taken, vec![item("emerald", 4)]);
        assert_eq!(stage.events, vec!["clear-player"]);
        assert!(state.preview().is_empty());
        Ok(())
    }
    #[test]
    fn quick_result_autofill_and_transaction_guards() -> Result<()> {
        let mut entry = offer();
        entry.cost_b = None;
        entry.result = item("bread", 5);
        entry.max_uses = 9;
        let mut stage = FixtureStage::new(vec![entry]);
        let mut state = MerchantState::new(stage.binding);
        let mut inventory = PlayerInventory::new();
        inventory.set_slot_authoritative(9, item("emerald", 10));
        inventory.set_slot_authoritative(36, item("emerald", 20));
        let plan = state.prepare_selection_and_autofill(0, &inventory, &mut stage)?;
        assert_eq!(plan.next_state().payments()[0].count, 30); // gathers beyond cost
        state.commit(plan, Some(&mut inventory), &mut stage)?;
        assert!(inventory.slot(9).unwrap().is_empty());
        assert!(inventory.slot(36).unwrap().is_empty());
        stage.events.clear();
        let plan = state.prepare_take(ResultTake::QuickMove { transferred: 2 }, &mut stage)?;
        assert_eq!(
            &stage.events[..4],
            &["crafted:5:2", "sound", "updated:5", "crafted:3:0"]
        );
        assert_eq!(state.commit(plan, None, &mut stage)?[0].count, 2);
        assert_eq!(state.payments()[0].count, 29);
        let before = state.clone();
        let plan = state.prepare_selection(0, &mut stage)?;
        stage.key += 1;
        assert!(state.commit(plan, None, &mut stage).is_err());
        assert_eq!(state, before);
        let plan = state.prepare_selection(0, &mut stage)?;
        stage.reject = true;
        assert!(state.commit(plan, None, &mut stage).is_err());
        assert_eq!(state, before);
        Ok(())
    }
}
