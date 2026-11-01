//! Detached views of the actual native anvil owners and their one-shot commit.
//!
//! The outer Main transaction holds the real Level/player/container locks. A
//! host stage owns only proposals and observations. Its publication capability
//! prepares every fallible callback and seals the one actual commit before UI
//! adoption. The outer same-owner transaction then commits it infallibly once;
//! this host queues no packet and never drains a network writer.
use super::{
    main_anvil_dispatch::ActualAnvilAccess, main_item_gameplay::MainItemGameplay,
    main_workstation_components,
};
use anyhow::{bail, ensure, Context, Result};
use mc121_rust::{
    container::{ContainerClick, DroppedStack, PlayerContainer},
    container_workstation_anvil::{AnvilMenuAction, AnvilMenuHost, AnvilMenuToken, AnvilPhase},
    inventory::{self, ItemStack, PlayerInventory},
    nbt::{JavaString, Tag},
    protocol::Reader,
    random::LegacyRandomSource,
    workstations::{
        anvil::{AnvilContext, AnvilPlayer, AnvilWorldAccess},
        menu::{SyncEvent, WorkstationAccess, WorkstationKind},
    },
};
use std::{cell::RefCell, fmt::Debug, rc::Rc, sync::Arc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct NativeAnvilBinding {
    pub(super) player_id: i32,
    pub(super) player_uuid: [u8; 16],
    pub(super) position: (i32, i32, i32),
    pub(super) window_id: u8,
    pub(super) dimension: String,
}

/// Ephemeral observation of the locked real gameplay owner. Copying this
/// value copies a certificate, never a registry or advancement/stat owner.
/// The publication callback must validate it against the same still-locked
/// owner before preparing actual callbacks and committing their effects.
#[derive(Clone, Copy, Debug)]
pub(super) struct NativeAnvilGameplayCertificate {
    owner_address: usize,
}
impl NativeAnvilGameplayCertificate {
    pub(super) fn new(gameplay: &MainItemGameplay) -> Result<Self> {
        gameplay.require_supported_floor()?;
        Ok(Self {
            owner_address: gameplay as *const MainItemGameplay as usize,
        })
    }
    pub(super) fn validate(self, gameplay: &MainItemGameplay) -> Result<()> {
        ensure!(
            self.owner_address == gameplay as *const MainItemGameplay as usize,
            "native anvil gameplay certificate belongs to another actual owner"
        );
        gameplay.require_supported_floor()
    }
}

#[derive(Debug)]
pub(super) struct UnsupportedAnvilEnchantmentContext;
impl std::fmt::Display for UnsupportedAnvilEnchantmentContext {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("native anvil enchantment components require their actual configured registry/holder identity owner")
    }
}
impl std::error::Error for UnsupportedAnvilEnchantmentContext {}

/// The initial Main slice is deliberately unenchanted while the canonical
/// configured holder provider is unavailable. Preserving bytes is insufficient
/// to establish source holder identity for generic component comparisons.
pub(super) fn require_anvil_stack_context(stack: &ItemStack) -> Result<()> {
    main_workstation_components::require_supported_stack(stack)?;
    fn inspect(stack: &ItemStack, depth: usize, visits: &mut usize) -> Result<()> {
        *visits += 1;
        ensure!(
            depth <= 128 && *visits <= 262_144,
            "anvil component context exceeds native admission budget"
        );
        if stack.is_empty() {
            return Ok(());
        }
        for id in [9, 23] {
            if let Some(payload) = stack.effective_component_bytes(id) {
                let mut fields = Reader::new(payload);
                let count = fields.varint()?;
                ensure!(count >= 0, "invalid anvil enchantment component count");
                if count > 0 {
                    return Err(UnsupportedAnvilEnchantmentContext.into());
                }
                fields.bool()?; // copied empty-map tooltip has no holder keys
                fields.finish()?;
            }
        }
        for id in [29, 30, 52] {
            let Some(payload) = stack.effective_component_bytes(id) else {
                continue;
            };
            let mut fields = Reader::new(payload);
            let count = fields.varint()?;
            ensure!(
                (0..=65_536).contains(&count),
                "invalid anvil nested-item component count"
            );
            for _ in 0..count {
                let (child, used) = ItemStack::decode(fields.remaining())?;
                fields.bytes(used)?;
                inspect(&child, depth + 1, visits)?;
            }
            fields.finish()?;
        }
        Ok(())
    }
    inspect(stack, 0, &mut 0)
}
pub(super) fn require_anvil_inventory_context(inventory: &PlayerInventory) -> Result<()> {
    for stack in inventory.slots().iter().chain([inventory.carried()]) {
        require_anvil_stack_context(stack)?;
    }
    Ok(())
}
pub(super) fn require_anvil_prediction_context(click: &ContainerClick) -> Result<()> {
    for stack in click.changed_slots.values().chain([&click.carried]) {
        require_anvil_stack_context(stack)?;
    }
    Ok(())
}

/// Observations retain the actual inventory at each source callback boundary.
/// InventoryChanged captures BEFORE its one Inventory.setChanged increment.
/// Returned observations are receipts, never instructions to repeat an effect.
#[derive(Clone, Debug)]
pub(super) enum NativeAnvilEvent {
    Sync {
        event: SyncEvent,
        phase: AnvilPhase,
        inventory: PlayerInventory,
    },
    InventoryChanged {
        phase: AnvilPhase,
        inventory: PlayerInventory,
    },
    InputChanged {
        phase: AnvilPhase,
        inventory: PlayerInventory,
    },
    Drop {
        owned: DroppedStack,
        phase: AnvilPhase,
        inventory: PlayerInventory,
    },
    LevelEvent {
        event: i32,
        data: i32,
        position: (i32, i32, i32),
        phase: AnvilPhase,
        inventory: PlayerInventory,
    },
}
impl NativeAnvilEvent {
    pub(super) fn phase(&self) -> &AnvilPhase {
        match self {
            Self::Sync { phase, .. }
            | Self::InventoryChanged { phase, .. }
            | Self::InputChanged { phase, .. }
            | Self::Drop { phase, .. }
            | Self::LevelEvent { phase, .. } => phase,
        }
    }
    pub(super) fn inventory(&self) -> &PlayerInventory {
        match self {
            Self::Sync { inventory, .. }
            | Self::InventoryChanged { inventory, .. }
            | Self::InputChanged { inventory, .. }
            | Self::Drop { inventory, .. }
            | Self::LevelEvent { inventory, .. } => inventory,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct NativeAnvilStage {
    pub(super) inventory: PlayerInventory,
    pub(super) player_random: LegacyRandomSource,
    pub(super) instabuild: bool,
    pub(super) experience_level: i32,
    pub(super) block: u32,
    pub(super) events: Vec<NativeAnvilEvent>,
}

/// Read-only source Item methods. This service does not own stacks, mutable
/// registry holders, player XP or callbacks. The native profile below is a
/// finite port of verified original methods, not a permissive item registry.
pub(super) trait NativeAnvilItemMethods: Debug + Send + Sync {
    fn is_valid_repair_item(&self, base: &ItemStack, addition: &ItemStack) -> Result<bool>;
    fn hover_name(&self, item: &ItemStack) -> Result<JavaString>;
}
#[derive(Debug)]
pub(super) struct PinnedNativeAnvilItems;

// Original bootstrap/runtime observations: each table row's runtime class and
// repair-method declaring class, and actual ItemStack.getHoverName().getString.
// See anvil-context.md. This is deliberately not the whole language asset.
const ITEM_FACTS: &[(&str, &str)] = &[
    ("stone", "Stone"),
    ("paper", "Paper"),
    ("iron_ingot", "Iron Ingot"),
    ("gold_ingot", "Gold Ingot"),
    ("diamond", "Diamond"),
    ("netherite_ingot", "Netherite Ingot"),
    ("leather", "Leather"),
    ("turtle_scute", "Turtle Scute"),
    ("armadillo_scute", "Armadillo Scute"),
    ("phantom_membrane", "Phantom Membrane"),
    ("breeze_rod", "Breeze Rod"),
    ("bow", "Bow"),
    ("crossbow", "Crossbow"),
    ("fishing_rod", "Fishing Rod"),
    ("flint_and_steel", "Flint and Steel"),
    ("shears", "Shears"),
    ("brush", "Brush"),
    ("trident", "Trident"),
    ("elytra", "Elytra"),
    ("mace", "Mace"),
    ("shield", "Shield"),
    ("wolf_armor", "Wolf Armor"),
    ("turtle_helmet", "Turtle Shell"),
    ("wooden_sword", "Wooden Sword"),
    ("wooden_pickaxe", "Wooden Pickaxe"),
    ("wooden_axe", "Wooden Axe"),
    ("wooden_shovel", "Wooden Shovel"),
    ("wooden_hoe", "Wooden Hoe"),
    ("stone_sword", "Stone Sword"),
    ("stone_pickaxe", "Stone Pickaxe"),
    ("stone_axe", "Stone Axe"),
    ("stone_shovel", "Stone Shovel"),
    ("stone_hoe", "Stone Hoe"),
    ("iron_sword", "Iron Sword"),
    ("iron_pickaxe", "Iron Pickaxe"),
    ("iron_axe", "Iron Axe"),
    ("iron_shovel", "Iron Shovel"),
    ("iron_hoe", "Iron Hoe"),
    ("golden_sword", "Golden Sword"),
    ("golden_pickaxe", "Golden Pickaxe"),
    ("golden_axe", "Golden Axe"),
    ("golden_shovel", "Golden Shovel"),
    ("golden_hoe", "Golden Hoe"),
    ("diamond_sword", "Diamond Sword"),
    ("diamond_pickaxe", "Diamond Pickaxe"),
    ("diamond_axe", "Diamond Axe"),
    ("diamond_shovel", "Diamond Shovel"),
    ("diamond_hoe", "Diamond Hoe"),
    ("netherite_sword", "Netherite Sword"),
    ("netherite_pickaxe", "Netherite Pickaxe"),
    ("netherite_axe", "Netherite Axe"),
    ("netherite_shovel", "Netherite Shovel"),
    ("netherite_hoe", "Netherite Hoe"),
    ("leather_helmet", "Leather Cap"),
    ("leather_chestplate", "Leather Tunic"),
    ("leather_leggings", "Leather Pants"),
    ("leather_boots", "Leather Boots"),
    ("chainmail_helmet", "Chainmail Helmet"),
    ("chainmail_chestplate", "Chainmail Chestplate"),
    ("chainmail_leggings", "Chainmail Leggings"),
    ("chainmail_boots", "Chainmail Boots"),
    ("iron_helmet", "Iron Helmet"),
    ("iron_chestplate", "Iron Chestplate"),
    ("iron_leggings", "Iron Leggings"),
    ("iron_boots", "Iron Boots"),
    ("golden_helmet", "Golden Helmet"),
    ("golden_chestplate", "Golden Chestplate"),
    ("golden_leggings", "Golden Leggings"),
    ("golden_boots", "Golden Boots"),
    ("diamond_helmet", "Diamond Helmet"),
    ("diamond_chestplate", "Diamond Chestplate"),
    ("diamond_leggings", "Diamond Leggings"),
    ("diamond_boots", "Diamond Boots"),
    ("netherite_helmet", "Netherite Helmet"),
    ("netherite_chestplate", "Netherite Chestplate"),
    ("netherite_leggings", "Netherite Leggings"),
    ("netherite_boots", "Netherite Boots"),
];
fn item_key(item: &ItemStack) -> Result<&'static str> {
    ensure!(
        !item.is_empty(),
        "anvil Item method requires a nonempty actual stack"
    );
    let name =
        inventory::item_name(item.item_id).context("unknown native anvil Item registry binding")?;
    Ok(name.strip_prefix("minecraft:").unwrap_or(name))
}
impl NativeAnvilItemMethods for PinnedNativeAnvilItems {
    fn is_valid_repair_item(&self, base: &ItemStack, addition: &ItemStack) -> Result<bool> {
        let base = item_key(base)?;
        ensure!(
            ITEM_FACTS.iter().any(|(name, _)| *name == base),
            "anvil repair Item method is outside the certified original item set: {base}"
        );
        let material = match base {
            "bow" | "crossbow" | "fishing_rod" | "flint_and_steel" | "shears" | "brush"
            | "trident" | "stone" | "paper" | "iron_ingot" | "gold_ingot" | "diamond"
            | "netherite_ingot" | "leather" | "turtle_scute" | "armadillo_scute"
            | "phantom_membrane" | "breeze_rod" => return Ok(false),
            "elytra" => "phantom_membrane",
            "mace" => "breeze_rod",
            "wolf_armor" => "armadillo_scute",
            "turtle_helmet" => "turtle_scute",
            "shield" => bail!("anvil shield repair requires the actual configured PLANKS tag"),
            value if value.starts_with("wooden_") => {
                bail!("anvil wooden-tier repair requires the actual configured PLANKS tag")
            }
            value if value.starts_with("stone_") => bail!(
                "anvil stone-tier repair requires the actual configured STONE_TOOL_MATERIALS tag"
            ),
            value if value.starts_with("leather_") => "leather",
            value if value.starts_with("chainmail_") || value.starts_with("iron_") => "iron_ingot",
            value if value.starts_with("golden_") => "gold_ingot",
            value if value.starts_with("diamond_") => "diamond",
            value if value.starts_with("netherite_") => "netherite_ingot",
            _ => bail!("uncertified original anvil repair Item method: {base}"),
        };
        // Ingredient.of(ItemLike) and ItemStack.is(Item) test item identity;
        // the addition's count/components do not change this predicate.
        Ok(!addition.is_empty() && item_key(addition)? == material)
    }
    fn hover_name(&self, item: &ItemStack) -> Result<JavaString> {
        // Source first CUSTOM_NAME, then ITEM_NAME, then Item.getName(stack).
        for id in [5, 6] {
            if let Some(bytes) = item.effective_component_bytes(id) {
                return literal_component_string(&Tag::decode_anonymous(bytes)?, 0);
            }
        }
        let key = item_key(item)?;
        ensure!(
            key != "shield" || item.effective_component_bytes(50).is_none(),
            "anvil colored shield name requires its actual component-sensitive Language owner"
        );
        let (_, name) = ITEM_FACTS
            .iter()
            .find(|(candidate, _)| *candidate == key)
            .with_context(|| {
                format!("anvil default hover name requires its actual Language/Item owner: {key}")
            })?;
        Ok((*name).into())
    }
}

/// Component.getString visits content then siblings and ignores style. No
/// lossy UTF-16 conversion; unresolved translation/keybind/selector/NBT content
/// needs the actual text owner instead of an invented English approximation.
fn literal_component_string(tag: &Tag, depth: usize) -> Result<JavaString> {
    ensure!(
        depth <= 128,
        "anvil literal text nesting exceeds native admission"
    );
    if let Tag::String(text) = tag {
        return Ok(text.clone());
    }
    let Tag::Compound(fields) = tag else {
        bail!("anvil text requires literal component ownership");
    };
    for (name, value) in fields {
        match name.as_str() {
            Some(
                "text" | "extra" | "bold" | "italic" | "underlined" | "strikethrough"
                | "obfuscated" | "color" | "font" | "insertion",
            ) => {}
            Some("type") if matches!(value, Tag::String(kind) if kind == "text") => {}
            _ => bail!("anvil hover text requires its actual nonliteral content owner"),
        }
    }
    let Some(Tag::String(text)) = tag.get("text") else {
        bail!("anvil hover text is not literal content");
    };
    let mut units = text.iter_units().collect::<Vec<_>>();
    if let Some(extra) = tag.get("extra") {
        let Tag::List { values, .. } = extra else {
            bail!("invalid anvil text siblings");
        };
        for sibling in values {
            // NBT ComponentSerialization wraps heterogeneous-list elements.
            let sibling = match sibling {
                Tag::Compound(fields) if fields.len() == 1 && fields[0].0 == "" => &fields[0].1,
                _ => sibling,
            };
            units.extend(literal_component_string(sibling, depth + 1)?.iter_units());
        }
    }
    Ok(JavaString::from_utf16_lossless(units))
}

type Publication<'a> =
    dyn FnOnce(NativeAnvilStage, AnvilMenuToken, AnvilMenuAction) -> Result<()> + 'a;
type CommitCapability<'a> = Rc<RefCell<Option<Box<Publication<'a>>>>>;

/// Ephemeral synchronous capability: never retained in LivePlayers or across
/// await. Rc shares only immutable observations and this one-shot capability;
/// every mutable owner proposal is copied into begin_stage's owned stage.
pub(super) struct NativeAnvilHost<'a> {
    binding: NativeAnvilBinding,
    access: Rc<ActualAnvilAccess>,
    items: Arc<dyn NativeAnvilItemMethods>,
    stage: NativeAnvilStage,
    phase: Option<AnvilPhase>,
    prepared: Option<(AnvilMenuToken, AnvilMenuAction)>,
    publish: CommitCapability<'a>,
}
impl<'a> NativeAnvilHost<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        binding: NativeAnvilBinding,
        access: ActualAnvilAccess,
        inventory: PlayerInventory,
        player_random: LegacyRandomSource,
        instabuild: bool,
        experience_level: i32,
        block: u32,
        _gameplay: NativeAnvilGameplayCertificate,
        publish: impl FnOnce(NativeAnvilStage, AnvilMenuToken, AnvilMenuAction) -> Result<()> + 'a,
    ) -> Result<Self> {
        ensure!(
            binding.dimension == "minecraft:overworld" && binding.window_id != 0,
            "native anvil requires its actual external Overworld binding"
        );
        ensure!(
            experience_level >= 0,
            "invalid actual player experience level"
        );
        require_anvil_inventory_context(&inventory)?;
        Ok(Self {
            binding,
            access: Rc::new(access),
            items: Arc::new(PinnedNativeAnvilItems),
            stage: NativeAnvilStage {
                inventory,
                player_random,
                instabuild,
                experience_level,
                block,
                events: Vec::new(),
            },
            phase: None,
            prepared: None,
            publish: Rc::new(RefCell::new(Some(Box::new(publish)))),
        })
    }
    fn require_staged(&self) -> Result<()> {
        ensure!(
            self.prepared.is_some(),
            "anvil callback requires a detached source action"
        );
        Ok(())
    }
    fn require_phase(phase: &AnvilPhase) -> Result<()> {
        for item in phase
            .inputs
            .iter()
            .chain([&phase.result_preview, &phase.carried])
        {
            require_anvil_stack_context(item)?;
        }
        Ok(())
    }
}
impl AnvilContext for NativeAnvilHost<'_> {
    fn is_valid_repair_item(&mut self, base: &ItemStack, addition: &ItemStack) -> Result<bool> {
        self.require_staged()?;
        self.items.is_valid_repair_item(base, addition)
    }
    fn hover_name(&mut self, item: &ItemStack) -> Result<JavaString> {
        self.require_staged()?;
        self.items.hover_name(item)
    }
    // The inherited None/error enchantment methods explicitly represent the
    // unavailable canonical registry. No protocol-ID holder stand-ins exist.
}
impl WorkstationAccess for NativeAnvilHost<'_> {
    fn block_matches(&self, kind: WorkstationKind) -> Result<bool> {
        self.access.block_matches(kind)
    }
    fn player_can_interact_with_block(&self, additional_range: f64) -> Result<bool> {
        self.access.player_can_interact_with_block(additional_range)
    }
}
impl AnvilWorldAccess for NativeAnvilHost<'_> {
    type BlockState = u32;
    fn block_state(&mut self) -> Result<u32> {
        self.require_staged()?;
        Ok(self.stage.block)
    }
    fn is_anvil(&self, state: &u32) -> Result<bool> {
        Ok(super::main_anvil_dispatch::is_anvil(*state))
    }
    fn player_random_next_float(&mut self) -> Result<f32> {
        // Creative source short-circuits before this wear draw. Even though
        // the actual player stream is captured, survival wear is not admitted.
        bail!("native anvil wear requires actual noncreative player/world ownership")
    }
    fn damaged_state(&self, _: &u32) -> Result<Option<u32>> {
        bail!("native anvil wear-state mutation is outside the admitted creative profile")
    }
    fn destroy_block_without_drop(&mut self) -> Result<()> {
        bail!("native anvil destruction is outside the admitted creative profile")
    }
    fn set_block(&mut self, _: u32, _: i32) -> Result<()> {
        bail!("native anvil block mutation is outside the admitted creative profile")
    }
    fn level_event(&mut self, event: i32, data: i32) -> Result<()> {
        self.require_staged()?;
        ensure!(
            self.stage.instabuild && event == 1030 && data == 0,
            "native anvil world callback is outside the admitted creative finish path"
        );
        let phase = self
            .phase
            .clone()
            .context("anvil level event has no source menu phase")?;
        self.stage.events.push(NativeAnvilEvent::LevelEvent {
            event,
            data,
            position: self.binding.position,
            phase,
            inventory: self.stage.inventory.clone(),
        });
        Ok(())
    }
}
impl AnvilMenuHost for NativeAnvilHost<'_> {
    type Binding = NativeAnvilBinding;
    fn binding(&self) -> &Self::Binding {
        &self.binding
    }
    fn begin_stage(&self, token: AnvilMenuToken, action: AnvilMenuAction) -> Result<Self> {
        ensure!(
            self.prepared.is_none() && self.publish.borrow().is_some(),
            "native anvil host is not a fresh live commit capability"
        );
        let mut stage = self.stage.clone();
        stage.events.clear();
        Ok(Self {
            binding: self.binding.clone(),
            access: self.access.clone(),
            items: self.items.clone(),
            stage,
            phase: None,
            prepared: Some((token, action)),
            publish: self.publish.clone(),
        })
    }
    fn publish_stage(&mut self, staged: Self, token: AnvilMenuToken) -> Result<()> {
        let (expected, action) = staged
            .prepared
            .context("native anvil publication has no detached action")?;
        ensure!(
            self.prepared.is_none()
                && expected == token
                && self.binding == staged.binding
                && Rc::ptr_eq(&self.publish, &staged.publish)
                && Rc::ptr_eq(&self.access, &staged.access)
                && Arc::ptr_eq(&self.items, &staged.items),
            "native anvil publication owner/token mismatch"
        );
        require_anvil_inventory_context(&staged.stage.inventory)?;
        // take() prevents callback reentry or reuse even if real preflight
        // rejects the candidate. A caller constructs a fresh host for retry.
        let publish = self
            .publish
            .borrow_mut()
            .take()
            .context("native anvil actual publication already consumed")?;
        publish(staged.stage.clone(), token, action)?;
        // The actual same-owner commit is sealed; no fallible work follows.
        // The kernel installs its private UI, then the outer transaction
        // applies that one infallible commit and queues its ordered outbox.
        self.stage = staged.stage;
        self.phase = staged.phase;
        Ok(())
    }
    fn inventory(&self) -> &PlayerInventory {
        &self.stage.inventory
    }
    fn inventory_mut(&mut self) -> &mut PlayerInventory {
        &mut self.stage.inventory
    }
    fn player(&self) -> Result<AnvilPlayer> {
        Ok(AnvilPlayer {
            instabuild: self.stage.instabuild,
            experience_level: self.stage.experience_level,
        })
    }
    fn preflight_menu_action(&mut self, action: AnvilMenuAction) -> Result<()> {
        self.require_staged()?;
        if matches!(
            action,
            AnvilMenuAction::Initialize | AnvilMenuAction::Rename | AnvilMenuAction::Click(_)
        ) {
            ensure!(
                self.stage.instabuild,
                "native anvil mutable action requires the actual admitted creative profile"
            );
        }
        require_anvil_inventory_context(&self.stage.inventory)
    }
    fn observe_menu_phase(&mut self, phase: &AnvilPhase) -> Result<()> {
        self.require_staged()?;
        Self::require_phase(phase)?;
        self.phase = Some(phase.clone());
        Ok(())
    }
    fn stage_sync_events(&mut self, events: Vec<SyncEvent>, phase: &AnvilPhase) -> Result<()> {
        self.observe_menu_phase(phase)?;
        for event in events {
            self.stage.events.push(NativeAnvilEvent::Sync {
                event,
                phase: phase.clone(),
                inventory: self.stage.inventory.clone(),
            });
        }
        Ok(())
    }
    fn input_container_set_changed(&mut self, phase: &AnvilPhase) -> Result<()> {
        self.observe_menu_phase(phase)?;
        // Source constructor installs no SimpleContainer listeners. The actual
        // Main gameplay certificate rejects a different/reentrant profile.
        self.stage.events.push(NativeAnvilEvent::InputChanged {
            phase: phase.clone(),
            inventory: self.stage.inventory.clone(),
        });
        Ok(())
    }
    fn player_inventory_set_changed(&mut self, phase: &AnvilPhase) -> Result<()> {
        self.observe_menu_phase(phase)?;
        self.stage.events.push(NativeAnvilEvent::InventoryChanged {
            phase: phase.clone(),
            inventory: self.stage.inventory.clone(),
        });
        self.stage.inventory.set_changed();
        Ok(())
    }
    fn give_experience_levels(&mut self, _: i32, _: &AnvilPhase) -> Result<()> {
        bail!("native anvil XP mutation requires the actual noncreative Player.giveExperienceLevels owner")
    }
    fn add_to_player_inventory(
        &mut self,
        stack: &mut ItemStack,
        phase: &AnvilPhase,
    ) -> Result<bool> {
        self.observe_menu_phase(phase)?;
        require_anvil_stack_context(stack)?;
        // Detached wrapper delegates to the existing native Inventory.add;
        // it neither creates another live inventory nor changes window0 sync.
        let mut inventory = PlayerContainer::from_inventory(self.stage.inventory.clone());
        let accepted = inventory.add_pickup_to_inventory(stack, self.stage.instabuild)?;
        self.stage.inventory = inventory.inventory;
        Ok(accepted)
    }
    fn stage_drop(&mut self, owned: DroppedStack, phase: &AnvilPhase) -> Result<()> {
        self.observe_menu_phase(phase)?;
        require_anvil_stack_context(&owned.stack)?;
        self.stage.events.push(NativeAnvilEvent::Drop {
            owned,
            phase: phase.clone(),
            inventory: self.stage.inventory.clone(),
        });
        Ok(())
    }
    fn quickcraft_iteration_order(&self, _: &[usize]) -> Result<Vec<usize>> {
        bail!("native anvil multi-slot drag requires the actual Slot identity HashSet encounter order")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::main_item_gameplay::GameplaySourceScope;
    use mc121_rust::{
        container::{ClickType, ContainerClick},
        container_workstation_anvil::AnvilMenu,
        crafting::RecipeBook,
        entity::{PlayerProfile, PlayerState, Position},
        inventory::Component,
        protocol::Writer,
        runtime_world::RuntimeWorld,
        world::FlatWorld,
    };
    use std::collections::BTreeMap;

    fn item(name: &str, count: i32) -> ItemStack {
        ItemStack::new(RecipeBook::vanilla().item_id(name).unwrap(), count).unwrap()
    }
    fn int_component(stack: &mut ItemStack, id: i32, value: i32) {
        let mut fields = Writer::new();
        fields.varint(value);
        stack.components.retain(|entry| entry.type_id != id);
        stack.components.push(Component {
            type_id: id,
            data: fields.into_inner(),
        });
        stack.normalize_patch();
    }
    fn click(window: u8, state: i32, slot: i16) -> ContainerClick {
        ContainerClick {
            window_id: window,
            state_id: state,
            slot,
            button: 0,
            kind: ClickType::Pickup,
            changed_slots: BTreeMap::new(),
            carried: ItemStack::empty(),
        }
    }
    struct Fixture {
        world: RuntimeWorld,
        player: PlayerState,
        gameplay: MainItemGameplay,
        binding: NativeAnvilBinding,
        actual: Rc<RefCell<NativeAnvilStage>>,
        fail_publish: Rc<RefCell<bool>>,
        publications: Rc<RefCell<usize>>,
    }
    impl Fixture {
        fn new() -> Result<Self> {
            let mut world = FlatWorld::new(0, 1, 2, 3, 0);
            let block = inventory::item_default_block_state(item("anvil", 1).item_id).unwrap();
            world.set_block(0, 64, 0, block);
            let player = PlayerState::new(
                42,
                PlayerProfile::offline("AnvilOwner")?,
                Position {
                    x: 0.5,
                    y: 64.,
                    z: 1.5,
                    yaw: 0.,
                    pitch: 0.,
                    on_ground: true,
                },
            )?;
            let mut inventory = PlayerInventory::new();
            let mut sword = item("iron_sword", 1);
            int_component(&mut sword, 3, 200);
            inventory.set_creative_slot(9, sword)?;
            inventory.set_creative_slot(10, item("iron_ingot", 2))?;
            let binding = NativeAnvilBinding {
                player_id: 42,
                player_uuid: player.profile.uuid,
                position: (0, 64, 0),
                window_id: 7,
                dimension: "minecraft:overworld".into(),
            };
            Ok(Self {
                world: RuntimeWorld::Fixture(world),
                player,
                gameplay: MainItemGameplay::new(GameplaySourceScope::NativeFixtureEmptyCriteria),
                binding,
                actual: Rc::new(RefCell::new(NativeAnvilStage {
                    inventory,
                    player_random: LegacyRandomSource::new(2718),
                    instabuild: true,
                    experience_level: 37,
                    block,
                    events: Vec::new(),
                })),
                fail_publish: Rc::new(RefCell::new(false)),
                publications: Rc::new(RefCell::new(0)),
            })
        }
        fn host(&mut self) -> Result<NativeAnvilHost<'static>> {
            let stage = self.actual.borrow().clone();
            let certificate = NativeAnvilGameplayCertificate::new(&self.gameplay)?;
            // This fixture commits only its detached owner record. Production
            // additionally validates the same actual gameplay owner and stages
            // ItemEntity/world/stat/listener owners under Main's real locks.
            let actual = self.actual.clone();
            let fail = self.fail_publish.clone();
            let publications = self.publications.clone();
            NativeAnvilHost::new(
                self.binding.clone(),
                super::super::main_anvil_dispatch::anvil_access(
                    &mut self.world,
                    &self.player,
                    self.binding.position,
                )?,
                stage.inventory,
                stage.player_random,
                stage.instabuild,
                stage.experience_level,
                stage.block,
                certificate,
                move |next, _, _| {
                    ensure!(!*fail.borrow(), "fixture rejects all actual publication");
                    *actual.borrow_mut() = next;
                    *publications.borrow_mut() += 1;
                    Ok(())
                },
            )
        }
        fn take(&mut self, menu: &mut AnvilMenu<NativeAnvilBinding>, slot: i16) -> Result<()> {
            menu.click(
                menu.token(),
                &click(7, menu.state_id(), slot),
                &mut self.host()?,
            )?;
            Ok(())
        }
    }

    #[test]
    fn actual_source_item_methods_and_literal_utf16_boundaries() -> Result<()> {
        let items = PinnedNativeAnvilItems;
        assert!(items.is_valid_repair_item(&item("iron_sword", 1), &item("iron_ingot", 3))?);
        assert!(items.is_valid_repair_item(&item("chainmail_boots", 1), &item("iron_ingot", 1))?);
        assert!(items.is_valid_repair_item(&item("wolf_armor", 1), &item("armadillo_scute", 1))?);
        assert!(!items.is_valid_repair_item(&item("bow", 1), &item("bow", 1))?);
        assert!(items
            .is_valid_repair_item(&item("wooden_axe", 1), &item("oak_planks", 1))
            .is_err());
        assert!(items
            .is_valid_repair_item(&item("shield", 1), &item("oak_planks", 1))
            .is_err());
        for (name, expected) in ITEM_FACTS {
            assert_eq!(
                items.hover_name(&item(name, 1))?,
                JavaString::from(*expected)
            );
        }
        let mut unknown = item("apple", 1);
        assert!(items.hover_name(&unknown).is_err());
        let name = Tag::Compound(vec![
            (
                "text".into(),
                Tag::String(JavaString::from_utf16_lossless(vec![0xd800])),
            ),
            ("bold".into(), Tag::Byte(1)),
            (
                "extra".into(),
                Tag::List {
                    element_type: 8,
                    values: vec![Tag::String("tail".into())],
                },
            ),
        ]);
        unknown.components.push(Component {
            type_id: 5,
            data: name.encode_network_anonymous()?,
        });
        assert_eq!(
            items.hover_name(&unknown)?.iter_units().collect::<Vec<_>>(),
            vec![0xd800, 116, 97, 105, 108]
        );
        let mut shield = item("shield", 1);
        int_component(&mut shield, 50, 0);
        assert!(items.hover_name(&shield).is_err());
        let mut enchanted = item("iron_sword", 1);
        let mut fields = Writer::new();
        fields.varint(1).varint(0).varint(1).bool(true);
        enchanted.components.push(Component {
            type_id: 9,
            data: fields.into_inner(),
        });
        assert!(require_anvil_stack_context(&enchanted)
            .unwrap_err()
            .is::<UnsupportedAnvilEnchantmentContext>());
        let mut nested = item("shulker_box", 1);
        let mut fields = Writer::new();
        fields.varint(1).raw(&enchanted.encode_canonical()?);
        nested.components.push(Component {
            type_id: 52,
            data: fields.into_inner(),
        });
        assert!(require_anvil_stack_context(&nested)
            .unwrap_err()
            .is::<UnsupportedAnvilEnchantmentContext>());
        let mut empty_map = item("iron_sword", 1);
        let mut fields = Writer::new();
        fields.varint(0).bool(false);
        empty_map.components.push(Component {
            type_id: 9,
            data: fields.into_inner(),
        });
        require_anvil_stack_context(&empty_map)?;
        Ok(())
    }

    #[test]
    fn native_context_creative_repair_stages_phases_once_and_rejects_late_publish() -> Result<()> {
        let mut fixture = Fixture::new()?;
        let mut menu = AnvilMenu::new(7, &fixture.host()?)?;
        menu.initialize(&mut fixture.host()?)?;
        fixture.take(&mut menu, 3)?; // canonical9 -> cursor
        {
            let actual = fixture.actual.borrow();
            let dirty = actual
                .events
                .iter()
                .filter_map(|event| match event {
                    NativeAnvilEvent::InventoryChanged { inventory, .. } => {
                        Some(inventory.get_times_changed())
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert!(
                !dirty.is_empty(),
                "actual player slot invokes Inventory.setChanged"
            );
            assert_eq!(dirty[0], 0, "event observes the pre-increment owner");
            for pair in dirty.windows(2) {
                assert_eq!(pair[1], pair[0].wrapping_add(1));
            }
            assert_eq!(
                actual.inventory.get_times_changed(),
                dirty.last().unwrap().wrapping_add(1)
            );
        }
        fixture.take(&mut menu, 0)?; // actual input0
        fixture.take(&mut menu, 4)?; // canonical10 -> cursor
        fixture.take(&mut menu, 1)?; // actual input1
        assert!(!menu.phase().result_preview.is_empty());
        let before_token = menu.token();
        let before_phase = menu.phase();
        let before = fixture.actual.borrow().clone();
        let publications = *fixture.publications.borrow();
        *fixture.fail_publish.borrow_mut() = true;
        assert!(fixture.take(&mut menu, 2).is_err());
        assert_eq!(menu.token(), before_token);
        assert_eq!(menu.phase().inputs, before_phase.inputs);
        assert_eq!(menu.phase().result_preview, before_phase.result_preview);
        assert_eq!(
            fixture.actual.borrow().inventory.slots(),
            before.inventory.slots()
        );
        assert_eq!(*fixture.publications.borrow(), publications);
        *fixture.fail_publish.borrow_mut() = false;
        fixture.take(&mut menu, 2)?;
        let actual = fixture.actual.borrow();
        assert_eq!(menu.carried().item_id, item("iron_sword", 1).item_id);
        assert_eq!(menu.carried().damage_value()?, 76); // two floor(250/4) repairs
        assert_eq!(actual.experience_level, 37); // actual creative XP is read, unchanged
        assert_eq!(
            actual.player_random.state48(),
            before.player_random.state48()
        );
        let events = &actual.events;
        assert!(events.iter().any(
            |event| matches!(event, NativeAnvilEvent::InputChanged { phase, .. }
            if phase.carried.item_id == item("iron_sword", 1).item_id)
        ));
        assert!(events.iter().any(|event| matches!(
            event,
            NativeAnvilEvent::LevelEvent {
                event: 1030,
                data: 0,
                ..
            }
        )));
        let counters = events
            .iter()
            .filter_map(|event| match event {
                NativeAnvilEvent::InventoryChanged { inventory, .. } => {
                    Some(inventory.get_times_changed())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        for pair in counters.windows(2) {
            assert_eq!(pair[1], pair[0].wrapping_add(1));
        }
        assert_eq!(
            actual.inventory.get_times_changed(),
            before
                .inventory
                .get_times_changed()
                .wrapping_add(counters.len() as i32)
        );
        Ok(())
    }

    #[test]
    fn gameplay_certificate_binds_actual_owner_and_unavailable_profiles_fail() -> Result<()> {
        let actual = MainItemGameplay::new(GameplaySourceScope::NativeFixtureEmptyCriteria);
        let other = MainItemGameplay::new(GameplaySourceScope::NativeFixtureEmptyCriteria);
        let certificate = NativeAnvilGameplayCertificate::new(&actual)?;
        certificate.validate(&actual)?;
        assert!(certificate.validate(&other).is_err());
        assert!(NativeAnvilGameplayCertificate::new(&MainItemGameplay::new(
            GameplaySourceScope::UnknownImported
        ))
        .is_err());
        Ok(())
    }
}
