//! Original ItemEntity.hurt for the vanilla ITEM entity type.
//!
//! Damage-source predicates and world effects remain synchronous and explicit.
//! This does not implement arbitrary entity-type overrides or item destruction
//! hooks; a provider must dispatch or reject the actual onDestroyed target.
use crate::{
    entity_base::BaseDamage,
    entity_state::EntityPhysicalState,
    inventory::{self, ItemStack},
};
use anyhow::{bail, Result};
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DamageTag {
    BypassesInvulnerability,
    Fire,
    Fall,
    Explosion,
}

pub trait ItemDamageSource {
    fn is_tag(&mut self, tag: DamageTag) -> Result<bool>;
    fn is_creative_player(&mut self) -> Result<bool>;
    fn causing_entity(&mut self) -> Result<Option<i32>>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DamageSourceFacts {
    pub bypasses_invulnerability: bool,
    pub fire: bool,
    pub fall: bool,
    pub explosion: bool,
    pub creative_player: bool,
    pub causing_entity: Option<i32>,
}
impl From<BaseDamage> for DamageSourceFacts {
    fn from(_: BaseDamage) -> Self {
        // Exact signed-JAR on_fire/lava: IS_FIRE, no other queried tag,
        // no creative source and no causing entity.
        Self {
            fire: true,
            ..Self::default()
        }
    }
}
impl ItemDamageSource for DamageSourceFacts {
    fn is_tag(&mut self, tag: DamageTag) -> Result<bool> {
        Ok(match tag {
            DamageTag::BypassesInvulnerability => self.bypasses_invulnerability,
            DamageTag::Fire => self.fire,
            DamageTag::Fall => self.fall,
            DamageTag::Explosion => self.explosion,
        })
    }
    fn is_creative_player(&mut self) -> Result<bool> {
        Ok(self.creative_player)
    }
    fn causing_entity(&mut self) -> Result<Option<i32>> {
        Ok(self.causing_entity)
    }
}

pub trait ItemDamageWorld {
    fn is_client_side(&self) -> bool;
    /// Original ENTITY_DAMAGE context, after health changes and markHurt.
    fn entity_damage(
        &mut self,
        state: &mut EntityPhysicalState,
        stack: &ItemStack,
        health: &mut i32,
        causing_entity: Option<i32>,
    ) -> Result<()>;
    /// Actual ItemStack.onDestroyed dispatch, before discard. A generic no-op
    /// is valid only for a proved method/context, never for all item classes.
    fn on_destroyed(
        &mut self,
        state: &mut EntityPhysicalState,
        stack: &mut ItemStack,
        health: &mut i32,
    ) -> Result<()>;
    /// Removal notification after the removed flag is set. Host removal/index,
    /// passenger and vehicle effects must be implemented or explicitly rejected.
    fn discarded(
        &mut self,
        state: &mut EntityPhysicalState,
        stack: &ItemStack,
        health: &mut i32,
    ) -> Result<()>;
}

/// Vanilla ITEM's type fire-immunity fact is false. Its virtual override adds
/// the effective FIRE_RESISTANT component; empty stacks expose no components.
pub fn item_fire_immune(stack: &ItemStack) -> bool {
    stack.effective_component_bytes(21).is_some()
}

/// Preserve Java predicate short circuiting and `(int)((float)health - amount)`.
pub fn hurt_item(
    state: &mut EntityPhysicalState,
    stack: &mut ItemStack,
    health: &mut i32,
    source: &mut impl ItemDamageSource,
    amount: f32,
    world: &mut impl ItemDamageWorld,
) -> Result<bool> {
    if state.removed
        || (state.invulnerable
            && !source.is_tag(DamageTag::BypassesInvulnerability)?
            && !source.is_creative_player()?)
        || (source.is_tag(DamageTag::Fire)? && item_fire_immune(stack))
    {
        return Ok(false);
    }
    // The original queries IS_FALL even though ITEM is absent from the
    // signed-JAR fall_damage_immune entity-type tag.
    let _fall = source.is_tag(DamageTag::Fall)?;
    if !stack.is_empty()
        && inventory::item_name(stack.item_id) == Some("nether_star")
        && source.is_tag(DamageTag::Explosion)?
    {
        return Ok(false);
    }
    if stack.effective_component_bytes(21).is_some() && source.is_tag(DamageTag::Fire)? {
        return Ok(false);
    }
    if world.is_client_side() {
        return Ok(true);
    }
    state.hurt_marked = true;
    // Rust's saturating f32->i32 cast has Java f2i's NaN/infinity behavior.
    *health = (*health as f32 - amount) as i32;
    let causing_entity = source.causing_entity()?;
    world.entity_damage(state, stack, health, causing_entity)?;
    if *health <= 0 {
        world.on_destroyed(state, stack, health)?;
        state.removed = true;
        world.discarded(state, stack, health)?;
    }
    Ok(true)
}

#[derive(Clone, Debug, Deserialize)]
pub struct ItemDestructionHook {
    pub item_id: i32,
    pub item_name: String,
    pub item_class: String,
    pub declaring_class: String,
}
pub fn destruction_hook(item_id: i32) -> Option<&'static ItemDestructionHook> {
    static FACTS: OnceLock<Vec<ItemDestructionHook>> = OnceLock::new();
    let facts = FACTS.get_or_init(|| {
        #[derive(Deserialize)]
        struct Facts {
            items: Vec<ItemDestructionHook>,
        }
        let mut facts: Facts =
            serde_json::from_str(include_str!("../fixtures/item-destroyed-facts.json"))
                .expect("validated original item dispatch facts");
        facts.items.sort_by_key(|item| item.item_id);
        assert_eq!(facts.items.len(), 1333);
        assert!(facts
            .items
            .iter()
            .enumerate()
            .all(|(index, item)| item.item_id == index as i32));
        facts.items
    });
    usize::try_from(item_id)
        .ok()
        .and_then(|index| facts.get(index))
}
impl ItemDestructionHook {
    pub fn is_base_no_op(&self) -> bool {
        self.declaring_class == "cul"
    }
}

/// Only the exact base Item return is a no-op. BlockItem.onDestroyed mutates
/// CONTAINER even when it was absent, so that override must never pass here.
pub fn is_proved_no_op_destruction(stack: &ItemStack) -> bool {
    let id = if stack.is_empty() { 0 } else { stack.item_id };
    destruction_hook(id).is_some_and(ItemDestructionHook::is_base_no_op)
}

/// The bounded original destruction dispatch. BlockItem's absent-CONTAINER
/// branch performs an in-place set(CONTAINER,EMPTY), then skips dropping because
/// the previous value was null. No metadata setter or world/RNG call occurs.
pub fn apply_supported_destruction(stack: &mut ItemStack) -> Result<()> {
    let id = if stack.is_empty() { 0 } else { stack.item_id };
    let Some(hook) = destruction_hook(id) else {
        bail!("unknown item destruction target {id}");
    };
    if hook.is_base_no_op() {
        return Ok(());
    }
    if hook.declaring_class == "cso" && stack.effective_component_bytes(52).is_none() {
        stack.components.retain(|component| component.type_id != 52);
        stack.removed_components.retain(|id| *id != 52);
        stack.components.push(inventory::Component {
            type_id: 52,
            data: vec![0],
        });
        stack.normalize_patch();
        return Ok(());
    }
    bail!(
        "unported item destruction hook {} for {}",
        hook.declaring_class,
        hook.item_name
    )
}

#[cfg(test)]
#[path = "../tools/item_damage_tests.rs"]
mod tests;
