//! Admission for the unresolved FOOD component reference-identity context.
//!
//! This guard observes actual stacks; it never strips a component from a live
//! inventory or replaces an unsupported value with its item's prototype.
use anyhow::{ensure, Context, Result};
use mc121_rust::{
    container::ContainerClick,
    inventory::{ItemStack, PlayerInventory},
    protocol::Reader,
};

// These mirror the existing native Slot parser's admission budgets, rather
// than asserting that Java imposes these particular aggregate limits. A
// separate guard budget is necessary because each child decode has a fresh
// private parser budget.
const MAX_SLOT_BYTES: usize = 2 * 1024 * 1024;
const MAX_ARRAY: i32 = 65_536;
const MAX_DEPTH: usize = 128;
const MAX_STACK_VISITS: usize = 262_144;

#[derive(Debug)]
pub(super) struct UnsupportedFoodIdentity;

impl std::fmt::Display for UnsupportedFoodIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(
            "unsupported FOOD component20 source identity: explicit FoodProperties require their actual nested ItemStack reference owner",
        )
    }
}

impl std::error::Error for UnsupportedFoodIdentity {}

#[derive(Default)]
struct GuardBudget {
    stack_visits: usize,
}

impl GuardBudget {
    fn visit(&mut self, depth: usize) -> Result<()> {
        self.stack_visits += 1;
        ensure!(
            depth <= MAX_DEPTH && self.stack_visits <= MAX_STACK_VISITS,
            "workstation component admission nesting/work limit exceeded",
        );
        Ok(())
    }
}

/// Certify only the bounded FOOD identity admission rule. Inherited prototypes
/// and removals are allowed. An explicit override is allowed to disappear only
/// through the original-stream prototype-elision policy already implemented by
/// ItemStack::normalize_patch, applied to this detached observation.
pub(super) fn require_supported_stack(stack: &ItemStack) -> Result<()> {
    inspect_stack(stack, &mut GuardBudget::default(), 0)?;
    if !stack.is_empty() {
        // Validate direct native constructors as well as decoded packets. This
        // is an observational encoding, never a replacement for the live value.
        let wire = stack.encode_canonical()?;
        ensure!(wire.len() <= MAX_SLOT_BYTES, "workstation Slot exceeds native admission size");
    }
    Ok(())
}

/// Inspect all46 retained InventoryMenu slots and its separate window0 cursor.
/// An external menu's cursor/top slots and private synchronization baselines
/// are additional actual owners, certified by their menu admission API.
pub(super) fn require_supported_inventory(inventory: &PlayerInventory) -> Result<()> {
    for (slot, stack) in inventory.slots().iter().enumerate() {
        require_supported_stack(stack)
            .with_context(|| format!("unsupported canonical inventory slot {slot}"))?;
    }
    require_supported_stack(inventory.carried())
        .context("unsupported retained InventoryMenu cursor")
}

/// Predictions become retained remote baselines even when they are not actual
/// inventory mutations. Check every supplied value before the kernel adopts
/// any changed-slot or carried prediction; never manufacture a replacement.
pub(super) fn require_supported_prediction(click: &ContainerClick) -> Result<()> {
    for (slot, stack) in &click.changed_slots {
        require_supported_stack(stack)
            .with_context(|| format!("unsupported predicted menu slot {slot}"))?;
    }
    require_supported_stack(&click.carried).context("unsupported predicted menu cursor")
}

fn inspect_stack(stack: &ItemStack, budget: &mut GuardBudget, depth: usize) -> Result<()> {
    budget.visit(depth)?;
    if stack.is_empty() {
        return Ok(());
    }
    let mut observation = stack.clone();
    observation.normalize_patch();
    if observation.components.iter().any(|component| component.type_id == 20) {
        return Err(UnsupportedFoodIdentity.into());
    }

    // These are precisely the other protocol767 component schemas containing
    // nested Slot values. Their original equality delegates to ItemStack's list
    // comparator, so checking only the outer stack would miss FOOD descendants.
    // The inherited FOOD prototype itself retains its genuine shared reference
    // owner; it is not treated as a newly decoded custom conversion stack.
    for component in [29, 30, 52] {
        let Some(payload) = observation.effective_component_bytes(component) else { continue; };
        let mut fields = Reader::new(payload);
        let length = fields.varint()?;
        ensure!((0..=MAX_ARRAY).contains(&length), "invalid nested workstation item-array length");
        if component == 52 {
            ensure!(length <= 256, "container item list exceeds256 slots");
        }
        for index in 0..length {
            let (child, used) = ItemStack::decode(fields.remaining())
                .with_context(|| format!("invalid component {component} child {index}"))?;
            fields.bytes(used)?;
            ensure!(component == 52 || !child.is_empty(), "empty nested item in NonEmptySlot component");
            inspect_stack(&child, budget, depth + 1)
                .with_context(|| format!("unsupported component {component} child {index}"))?;
        }
        fields.finish()?;
    }
    Ok(())
}
