//! Actual native default-profile smithing recipes and source-phased callbacks.
//! All proposals borrow the existing canonical gameplay/book/catalogue owners;
//! no inventory, advancement registry, attribute defaults or RNG is fabricated.
use super::*;
use main_item_gameplay::MainItemGameplay;
use mc121_rust::{
    container::LocalSlotChange,
    container_workstation_smithing::{SmithingMenuEvent, SmithingPhase},
    crafting::{RecipeBook, RecipeInput},
    player_stats::{PlayerStats, StatKey},
    server_recipe_book::{RecipeBookHost, RecipeBookPacket, RecipeDescriptor, RecipeId},
    workstations::{
        menu::SyncEvent,
        recipe_context::PinnedDefaultRecipeContext,
        smithing::{SmithingEffect, SmithingRecipeOrder},
    },
};

// Original registry IDs for all captured smithing base/result items plus AIR.
// Direct original bootstrap inspection proves the VANILLA_SET feature subset
// and that both onCraftedBy/onCraftedPostProcess are declared by Item. Item's
// postprocess body returns. This is a bounded capability list, not all items.
const SOURCE_CALLBACK_ITEMS: &[i32] = &[
    0, 794, 838, 839, 840, 841, 842, 843, 844, 845, 846, 847,
    856, 857, 858, 859, 860, 861, 862, 863, 864, 865, 866, 867,
    868, 869, 870, 871, 872, 873, 874, 875, 876, 877, 878, 879,
];

fn callback_item(stack: &ItemStack) -> Result<i32> {
    stack.encode_canonical()?;
    let item = if stack.is_empty() { 0 } else { stack.item_id };
    ensure!(SOURCE_CALLBACK_ITEMS.binary_search(&item).is_ok(),
        "smithing callback requires the captured original item postprocess owner");
    Ok(item)
}

pub(super) struct NativeSmithingRecipes {
    enabled: BTreeSet<String>,
}
impl NativeSmithingRecipes {
    pub(super) fn new(fixtures: &Fixtures, gameplay: &MainItemGameplay) -> Result<Self> {
        gameplay.require_supported_floor()?;
        let packets = fixtures.configuration.iter().filter(|packet| packet.id == 0x0c).collect::<Vec<_>>();
        ensure!(packets.len() == 1, "smithing requires the actual unique feature-set packet");
        let mut fields = Reader::new(&packets[0].payload);
        ensure!(fields.varint()? == 1, "smithing requires the captured default enabled-feature profile");
        let enabled = BTreeSet::from([fields.string(32_767)?]);
        fields.finish()?;
        PinnedDefaultRecipeContext::new(RecipeBook::vanilla(), &enabled)?;
        Ok(Self { enabled })
    }
    fn context(&self) -> Result<PinnedDefaultRecipeContext<'_>> {
        PinnedDefaultRecipeContext::new(RecipeBook::vanilla(), &self.enabled)
    }
    pub(super) fn order(&self) -> Result<SmithingRecipeOrder> {
        self.context()?.smithing_order()
    }
    pub(super) fn result_enabled(&self, result: &ItemStack) -> Result<bool> {
        self.context()?.result_enabled(result)
    }
}

struct CertifiedRecipeHost { packets: Vec<Packet> }
impl RecipeBookHost for CertifiedRecipeHost {
    fn recipe_unlocked(&mut self, _book: &mut ServerRecipeBook, recipe: &RecipeDescriptor) -> Result<()> {
        ensure!(matches!(RecipeBook::vanilla().recipe_type(recipe.id.as_str()),
            Some("smithing_transform" | "smithing_trim")), "unowned smithing recipe-unlocked callback");
        // Only constructed after the actual gameplay registry has certified
        // the fresh native empty-criteria profile. Registered reward/predicate
        // contexts fail before this host exists, rather than being ignored.
        Ok(())
    }
    fn send_packet(&mut self, _book: &mut ServerRecipeBook, packet: RecipeBookPacket) -> Result<()> {
        self.packets.push(packet.encode()?);
        Ok(())
    }
}

pub(super) struct PreparedSmithingCallbacks {
    before_book: ServerRecipeBook,
    next_book: ServerRecipeBook,
    crafted: BTreeMap<usize, (StatKey, i32)>,
    packets: BTreeMap<usize, Vec<Packet>>,
    book_checkpoints: BTreeMap<usize, ServerRecipeBook>,
    level_events: BTreeMap<usize, (i32, i32)>,
    inventory_changes: BTreeSet<usize>,
}
impl PreparedSmithingCallbacks {
    pub(super) fn prepare(
        gameplay: &MainItemGameplay,
        book: &ServerRecipeBook,
        events: &[SmithingMenuEvent],
    ) -> Result<Self> {
        gameplay.require_supported_floor()?;
        let mut prepared = Self {
            before_book: book.clone(), next_book: book.clone(),
            crafted: BTreeMap::new(), packets: BTreeMap::new(),
            book_checkpoints: BTreeMap::new(), level_events: BTreeMap::new(),
            inventory_changes: BTreeSet::new(),
        };
        let mut pending_recipe = None;
        for (index, event) in events.iter().enumerate() {
            match event {
                SmithingMenuEvent::Effect { effect: SmithingEffect::CraftedBy { stack, amount }, .. } => {
                    ensure!(*amount >= 0 && *amount == stack.count,
                        "smithing crafted amount differs from its actual callback stack");
                    // ItemStack.onCraftedBy always awards, including AIR/count0
                    // after a full shift transfer; its Item callback is proved.
                    prepared.crafted.insert(index, (StatKey::new(1, callback_item(stack)?)?, *amount));
                }
                SmithingMenuEvent::Effect { effect: SmithingEffect::RecipeCrafted { recipe_id, inputs }, phase } => {
                    ensure!(phase.inputs == *inputs,
                        "smithing recipe criterion lost its pre-consumption input phase");
                    for item in inputs { item.encode_canonical()?; }
                    ensure!(matches!(RecipeBook::vanilla().recipe_type(recipe_id),
                        Some("smithing_transform" | "smithing_trim")), "unknown actual smithing recipe criterion");
                    let input = RecipeInput::Smithing {
                        template: inputs[0].clone(), base: inputs[1].clone(), addition: inputs[2].clone(),
                    };
                    let assembled = RecipeBook::vanilla().evaluate(recipe_id, &input)?
                        .context("smithing criterion inputs do not match the actual recipe")?;
                    ensure!(!assembled.stack.is_empty(), "smithing criterion has no actual assembled result");
                    callback_item(&assembled.stack)?;
                    ensure!(pending_recipe.is_none(), "overlapping smithing recipe callback");
                    pending_recipe = Some(RecipeId::parse(recipe_id)?);
                    // The actual native registry has no RecipeCrafted
                    // listeners. Custom/reentrant contexts failed above.
                }
                SmithingMenuEvent::Effect { effect: SmithingEffect::AwardRecipe { recipe_id }, .. } => {
                    let id = RecipeId::parse(recipe_id)?;
                    ensure!(pending_recipe.take().as_ref() == Some(&id),
                        "smithing award does not follow its actual recipe-crafted callback");
                    let descriptor = VanillaRecipeCatalogue::get().descriptor(&id)
                        .context("smithing recipe descriptor absent")?;
                    let mut host = CertifiedRecipeHost { packets: Vec::new() };
                    prepared.next_book.add_recipes(&[descriptor], &mut host)?;
                    prepared.packets.insert(index, host.packets);
                    prepared.book_checkpoints.insert(index, prepared.next_book.clone());
                }
                SmithingMenuEvent::Effect { effect: SmithingEffect::LevelEvent { event, data }, .. } => {
                    ensure!(*event == 1044 && *data == 0, "unknown smithing world-event callback");
                    prepared.level_events.insert(index, (*event, *data));
                }
                SmithingMenuEvent::Effect { effect: SmithingEffect::BeforeCreateResult { .. }, .. } => {
                    // observe_effects_from already converts this boundary to
                    // source Sync events before recomputation. An escaped
                    // marker is not an additional completed callback or resync.
                    bail!("smithing pre-recompute boundary must be consumed by its actual menu adapter");
                }
                SmithingMenuEvent::PlayerInventoryChanged { .. } => {
                    prepared.inventory_changes.insert(index);
                }
                SmithingMenuEvent::Sync { .. } | SmithingMenuEvent::Drop { .. } => {}
            }
        }
        ensure!(pending_recipe.is_none(), "smithing recipe callback has no actual award boundary");
        Ok(prepared)
    }
    pub(super) fn apply_stat_at(&self, index: usize, stats: &mut PlayerStats) -> Result<()> {
        if let Some((key, amount)) = self.crafted.get(&index) { stats.increment(*key, *amount)?; }
        Ok(())
    }
    pub(super) fn packets_at(&self, index: usize) -> &[Packet] {
        self.packets.get(&index).map_or(&[], Vec::as_slice)
    }
    pub(super) fn book_at(&self, index: usize) -> Option<&ServerRecipeBook> {
        self.book_checkpoints.get(&index)
    }
    pub(super) fn level_event_at(&self, index: usize) -> Option<(i32, i32)> {
        self.level_events.get(&index).copied()
    }
    pub(super) fn is_player_inventory_change_at(&self, index: usize) -> bool {
        self.inventory_changes.contains(&index)
    }
    pub(super) fn validate_book(&self, book: &ServerRecipeBook) -> Result<()> {
        ensure!(book == &self.before_book, "actual smithing recipe book changed during preparation");
        Ok(())
    }
    pub(super) fn into_book(self) -> ServerRecipeBook { self.next_book }
}

pub(super) fn smithing_event_phase(event: &SmithingMenuEvent) -> &SmithingPhase {
    match event {
        SmithingMenuEvent::Effect { phase, .. } | SmithingMenuEvent::Sync { phase, .. }
        | SmithingMenuEvent::PlayerInventoryChanged { phase }
        | SmithingMenuEvent::Drop { phase, .. } => phase,
    }
}

pub(super) fn smithing_preflight_phase(phase: &SmithingPhase) -> Result<()> {
    for item in &phase.inputs { item.encode_canonical()?; }
    phase.result_preview.encode_canonical()?;
    phase.carried.encode_canonical()?;
    for slot in (36..45).chain(9..36).chain((5..9).rev()).chain(std::iter::once(45)) {
        let stack = phase.player_inventory.slot(slot).context("smithing phase inventory slot absent")?;
        if !stack.is_empty() { stack.encode_canonical()?; stack.max_stack_size()?; }
    }
    Ok(())
}

/// Packet extraction does not execute actual slot listeners or dirty counters.
pub(super) fn smithing_sync_packets(events: &[SmithingMenuEvent]) -> Result<Vec<Packet>> {
    let _ = smithing_local_changes(events)?;
    Ok(events.iter().filter_map(|event| match event {
        SmithingMenuEvent::Sync { event: SyncEvent::Packet(packet), .. } => Some(packet.clone()),
        _ => None,
    }).collect())
}

pub(super) fn smithing_local_changes(events: &[SmithingMenuEvent]) -> Result<Vec<LocalSlotChange>> {
    let mut out = Vec::new();
    let mut packet_offset = 0;
    for item in events {
        match item {
            SmithingMenuEvent::Sync { event: SyncEvent::Packet(_), .. } => packet_offset += 1,
            SmithingMenuEvent::Sync { event: SyncEvent::DataListener { .. }, .. } =>
                bail!("smithing has no menu data listener slots"),
            SmithingMenuEvent::Sync {
                event: SyncEvent::SlotListener { menu_slot, player_slot, stack }, phase,
            } => {
                stack.encode_canonical()?;
                if let Some(slot) = player_slot {
                    ensure!((9..45).contains(slot) && *menu_slot + 5 == *slot,
                        "smithing listener does not point at the canonical player inventory");
                    smithing_preflight_phase(phase)?;
                    out.push(LocalSlotChange {
                        menu_slot: *slot,
                        inventory_slot: Some(if *slot < 36 { *slot } else { *slot - 36 }),
                        stack: stack.clone(), packet_offset,
                    });
                } else { ensure!(*menu_slot < 4, "unknown smithing backing container"); }
            }
            _ => {}
        }
    }
    Ok(out)
}
