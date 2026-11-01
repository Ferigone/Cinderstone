//! Actual native-flat-profile recipe, statistic and recipe-book callbacks.
//! Preparation borrows the existing certified gameplay/book owners. It does not
//! create an inventory, advancement registry, RNG stream or entity-ID owner.
use super::*;
use mc121_rust::{
    container::LocalSlotChange,
    container_workstation_stonecutter::{StonecutterMenuEvent, StonecutterPhase},
    crafting::RecipeBook,
    player_stats::{PlayerStats, StatKey},
    server_recipe_book::{RecipeBookHost, RecipeBookPacket, RecipeDescriptor, RecipeId},
    workstations::{menu::SyncEvent, recipe_context::PinnedDefaultRecipeContext,
        stonecutter::{StonecutterEffect, StonecutterRecipes}},
};
use main_item_gameplay::MainItemGameplay;

// Original 1.21 registry IDs for all shipped stonecutting outputs plus AIR.
// Fresh original-bootstrap inspection proves requiredFeatures ⊆ VANILLA_SET
// and that onCraftedPostProcess is declared by Item (whose body returns).
// AIR matters: ItemStack.onCraftedBy still awards the zero-count AIR statistic
// after a successful full quick transfer. See the source ledger.
const SOURCE_CALLBACK_ITEMS: &[i32] = &[
    0, 3, 5, 7, 10, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
    24, 25, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109,
    110, 111, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 127, 128, 129,
    130, 131, 192, 193, 264, 265, 266, 267, 269, 270, 271, 272, 273, 274, 275, 276,
    277, 278, 279, 280, 296, 297, 304, 329, 340, 343, 346, 348, 350, 361, 362, 363,
    368, 370, 378, 380, 397, 398, 399, 400, 401, 402, 403, 404, 405, 406, 407, 408,
    409, 410, 411, 412, 413, 414, 415, 416, 417, 418, 422, 424, 425, 426, 506, 507,
    508, 511, 512, 513, 621, 622, 623, 624, 625, 626, 627, 628, 629, 630, 631, 632,
    633, 634, 635, 636, 637, 638, 639, 640, 641, 642, 643, 644, 645, 646, 647, 648,
    649, 650, 651, 652, 653, 654, 655, 1229, 1230, 1232, 1233, 1234, 1235, 1236, 1237, 1238,
    1311, 1312, 1313, 1314, 1315, 1316, 1317, 1318,
 ];

fn callback_item(stack: &ItemStack) -> Result<i32> {
    stack.encode_canonical()?;
    let id = if stack.is_empty() { 0 } else { stack.item_id };
    ensure!(SOURCE_CALLBACK_ITEMS.binary_search(&id).is_ok(),
        "stonecutter item callback requires the captured original postprocess owner");
    Ok(id)
}

/// Enabled features are read from the same immutable configuration snapshot
/// that Main sends. Only the certified native profile with VANILLA_SET is
/// admitted. Unknown/dynamic profiles are not treated as all-features-enabled.
pub(super) struct NativeStonecutterRecipes {
    enabled: BTreeSet<String>,
}
impl NativeStonecutterRecipes {
    pub(super) fn new(fixtures: &Fixtures, gameplay: &MainItemGameplay) -> Result<Self> {
        gameplay.require_supported_floor()?;
        let packets = fixtures.configuration.iter().filter(|packet| packet.id == 0x0c).collect::<Vec<_>>();
        ensure!(packets.len() == 1, "stonecutter requires the actual unique feature-set packet");
        let mut fields = Reader::new(&packets[0].payload);
        ensure!(fields.varint()? == 1,
            "stonecutter supports only the captured native default feature profile");
        let enabled = BTreeSet::from([fields.string(32_767)?]);
        fields.finish()?;
        PinnedDefaultRecipeContext::new(RecipeBook::vanilla(), &enabled)?;
        Ok(Self { enabled })
    }
    fn context(&self) -> Result<PinnedDefaultRecipeContext<'_>> {
        PinnedDefaultRecipeContext::new(RecipeBook::vanilla(), &self.enabled)
    }
}
impl StonecutterRecipes for NativeStonecutterRecipes {
    fn matching_recipes(&self, input: &ItemStack) -> Result<Vec<String>> {
        self.context()?.matching_recipes(input)
    }
    fn assemble(&self, recipe: &str, input: &ItemStack) -> Result<ItemStack> {
        let result = self.context()?.assemble(recipe, input)?;
        callback_item(&result)?;
        Ok(result)
    }
    fn result_enabled(&self, result: &ItemStack) -> Result<bool> {
        self.context()?.result_enabled(result)
    }
}

/// Only constructed after the actual gameplay registry proves the supported
/// empty-criteria profile. Recipe additions still mutate the actual staged
/// book and encode real network updates at the source award position.
struct CertifiedRecipeHost { packets: Vec<Packet> }
impl RecipeBookHost for CertifiedRecipeHost {
    fn recipe_unlocked(&mut self, _book: &mut ServerRecipeBook, recipe: &RecipeDescriptor) -> Result<()> {
        ensure!(RecipeBook::vanilla().recipe_type(recipe.id.as_str()) == Some("stonecutting"),
            "unexpected recipe-unlocked callback outside the stonecutter source catalogue");
        // The certified native registry has no advancement registrations; a
        // different profile is rejected before this host can be constructed.
        Ok(())
    }
    fn send_packet(&mut self, _book: &mut ServerRecipeBook, packet: RecipeBookPacket) -> Result<()> {
        self.packets.push(packet.encode()?);
        Ok(())
    }
}

/// One transaction's detached proposals. The caller retains exclusive actual
/// owner locks, validates the book again, applies statistic deltas in event
/// order (interleaved with drop statistics), and installs into_book exactly once.
/// Packet/sound markers are consumed at their original event indices.
pub(super) struct PreparedStonecutterCallbacks {
    before_book: ServerRecipeBook,
    next_book: ServerRecipeBook,
    crafted: BTreeMap<usize, (StatKey, i32)>,
    packets: BTreeMap<usize, Vec<Packet>>,
    book_checkpoints: BTreeMap<usize, ServerRecipeBook>,
    sounds: BTreeSet<usize>,
    default_input_changes: BTreeSet<usize>,
    player_inventory_changes: BTreeSet<usize>,
}
impl PreparedStonecutterCallbacks {
    pub(super) fn prepare(
        gameplay: &MainItemGameplay,
        book: &ServerRecipeBook,
        events: &[StonecutterMenuEvent],
    ) -> Result<Self> {
        gameplay.require_supported_floor()?;
        let mut prepared = Self {
            before_book: book.clone(), next_book: book.clone(),
            crafted: BTreeMap::new(), packets: BTreeMap::new(),
            book_checkpoints: BTreeMap::new(), sounds: BTreeSet::new(),
            default_input_changes: BTreeSet::new(), player_inventory_changes: BTreeSet::new(),
        };
        for (index, event) in events.iter().enumerate() {
            match event {
                StonecutterMenuEvent::ItemCraftedBeforeQuickMove { stack, .. } => {
                    // This is Item.onCraftedBy, not ItemStack.onCraftedBy.
                    // The actual captured Item.onCraftedPostProcess body has
                    // no mutation; it does not award an extra crafted stat.
                    callback_item(stack)?;
                }
                StonecutterMenuEvent::Effect { effect: StonecutterEffect::CraftedBy { stack, amount }, .. } => {
                    ensure!(*amount >= 0 && *amount == stack.count,
                        "stonecutter crafted callback amount differs from its source stack");
                    prepared.crafted.insert(index, (StatKey::new(1, callback_item(stack)?)?, *amount));
                }
                StonecutterMenuEvent::Effect { effect: StonecutterEffect::AwardUsedRecipe {
                    recipe_id, input_before_consumption,
                }, phase } => {
                    ensure!(phase.input == *input_before_consumption,
                        "stonecutter recipe criterion lost its actual pre-consumption input phase");
                    input_before_consumption.encode_canonical()?;
                    let input = mc121_rust::crafting::RecipeInput::Single(input_before_consumption.clone());
                    ensure!(RecipeBook::vanilla().recipe_type(recipe_id) == Some("stonecutting")
                        && RecipeBook::vanilla().evaluate(recipe_id, &input)?.is_some(),
                        "stonecutter recipe award differs from the captured pre-consumption input");
                    // RecipeCrafted observes this actual input. Its registered
                    // listener set is empty only in the certified native
                    // profile; unsupported reward/reentrant owners fail above.
                    let id = RecipeId::parse(recipe_id)?;
                    let descriptor = VanillaRecipeCatalogue::get().descriptor(&id)
                        .context("stonecutter recipe descriptor absent")?;
                    let mut host = CertifiedRecipeHost { packets: Vec::new() };
                    prepared.next_book.add_recipes(&[descriptor], &mut host)?;
                    prepared.packets.insert(index, host.packets);
                    prepared.book_checkpoints.insert(index, prepared.next_book.clone());
                }
                StonecutterMenuEvent::Effect { effect: StonecutterEffect::PlayTakeResultSound, .. } => {
                    prepared.sounds.insert(index);
                }
                StonecutterMenuEvent::Effect { effect: StonecutterEffect::InputConsumedAndResultRecomputed, .. } => {
                    bail!("stonecutter result recomputation must be handled by its prepared menu adapter");
                }
                StonecutterMenuEvent::InputChanged { phase } => {
                    phase.input.encode_canonical()?;
                    phase.result_preview.encode_canonical()?;
                    // crw's constructor binds Runnable to crw.q(), whose
                    // original bytecode returns. crw$1 already invokes the
                    // state/recompute path represented by this kernel phase.
                    // The native server menu has no replacement-listener API.
                    prepared.default_input_changes.insert(index);
                }
                StonecutterMenuEvent::PlayerInventoryChanged { .. } => {
                    // The caller must execute the actual canonical
                    // Inventory.setChanged counter at this event, not in a
                    // menu-local substitute or when extracting packets.
                    prepared.player_inventory_changes.insert(index);
                }
                StonecutterMenuEvent::Sync { .. } | StonecutterMenuEvent::Drop { .. } => {}
            }
        }
        Ok(prepared)
    }
    pub(super) fn apply_stat_at(&self, index: usize, stats: &mut PlayerStats) -> Result<()> {
        if let Some((key, amount)) = self.crafted.get(&index) { stats.increment(*key, *amount)?; }
        Ok(())
    }
    pub(super) fn packets_at(&self, index: usize) -> &[Packet] {
        self.packets.get(&index).map_or(&[], Vec::as_slice)
    }
    pub(super) fn is_sound_at(&self, index: usize) -> bool { self.sounds.contains(&index) }
    /// Complete only the source-default server listener captured at prepare.
    /// Its original crw.q() body returns; the kernel already performed the
    /// actual SimpleContainer/slotsChanged state update. Custom/reentrant
    /// listeners have no capability here and cannot be silently substituted.
    pub(super) fn complete_input_change_at(&self, index: usize) -> Result<()> {
        ensure!(self.default_input_changes.contains(&index),
            "unprepared stonecutter input callback");
        Ok(())
    }
    pub(super) fn is_player_inventory_change_at(&self, index: usize) -> bool {
        self.player_inventory_changes.contains(&index)
    }
    pub(super) fn book_at(&self, index: usize) -> Option<&ServerRecipeBook> {
        self.book_checkpoints.get(&index)
    }
    pub(super) fn validate_book(&self, book: &ServerRecipeBook) -> Result<()> {
        ensure!(book == &self.before_book, "stonecutter recipe book changed during preparation");
        Ok(())
    }
    pub(super) fn into_book(self) -> ServerRecipeBook { self.next_book }
}

/// Actual read-only source-boundary arguments, never another live owner.
pub(super) fn stonecutter_event_phase(event: &StonecutterMenuEvent) -> &StonecutterPhase {
    match event {
        StonecutterMenuEvent::Sync { phase, .. }
        | StonecutterMenuEvent::Effect { phase, .. }
        | StonecutterMenuEvent::InputChanged { phase }
        | StonecutterMenuEvent::PlayerInventoryChanged { phase }
        | StonecutterMenuEvent::Drop { phase, .. }
        | StonecutterMenuEvent::ItemCraftedBeforeQuickMove { phase, .. } => phase,
    }
}

/// Resolve every fallible query the real inventory-criterion scan will make
/// before an infallible publication, using that exact event's source snapshot.
pub(super) fn stonecutter_preflight_phase(phase: &StonecutterPhase) -> Result<()> {
    phase.input.encode_canonical()?;
    phase.result_preview.encode_canonical()?;
    phase.carried.encode_canonical()?;
    for slot in (36..45).chain(9..36).chain((5..9).rev()).chain(std::iter::once(45)) {
        let stack = phase.inventory.slot(slot).context("stonecutter phase inventory slot absent")?;
        if !stack.is_empty() { stack.encode_canonical()?; stack.max_stack_size()?; }
    }
    Ok(())
}

/// Extract packets only. The actual caller executes source-phased listeners
/// through its existing MainItemGameplay owner before packet publication.
pub(super) fn stonecutter_sync_packets(events: &[StonecutterMenuEvent]) -> Result<Vec<Packet>> {
    let _ = stonecutter_local_changes(events)?;
    Ok(events.iter().filter_map(|event| match event {
        StonecutterMenuEvent::Sync { event: SyncEvent::Packet(packet), .. } => Some(packet.clone()),
        _ => None,
    }).collect())
}

pub(super) fn stonecutter_local_changes(events: &[StonecutterMenuEvent]) -> Result<Vec<LocalSlotChange>> {
    let mut out = Vec::new();
    let mut packet_offset = 0;
    for item in events {
        match item {
            StonecutterMenuEvent::Sync { event: SyncEvent::Packet(_), .. } => packet_offset += 1,
            StonecutterMenuEvent::Sync { event: SyncEvent::DataListener { index, .. }, .. } =>
                ensure!(*index == 0, "unexpected stonecutter data listener"),
            StonecutterMenuEvent::Sync {
                event: SyncEvent::SlotListener { menu_slot, player_slot, stack }, phase,
            } => {
                stack.encode_canonical()?;
                if let Some(slot) = player_slot {
                    ensure!((9..45).contains(slot) && *menu_slot + 7 == *slot,
                        "stonecutter listener does not point at the canonical player inventory");
                    stonecutter_preflight_phase(phase)?;
                    out.push(LocalSlotChange {
                        menu_slot: *slot,
                        inventory_slot: Some(if *slot < 36 { *slot } else { *slot - 36 }),
                        stack: stack.clone(), packet_offset,
                    });
                } else { ensure!(*menu_slot < 2, "unknown stonecutter backing container"); }
            }
            _ => {}
        }
    }
    Ok(out)
}

/// Build the actual variable-range BLOCKS sound packet. The caller obtains
/// seed from its existing Level.threadSafeRandom owner at this event position.
pub(super) fn stonecutter_sound_packet(position: (i32, i32, i32), seed: i64) -> Packet {
    let mut fields = Writer::new();
    fields.varint(STONECUTTER_TAKE_SOUND_ID + 1).varint(4)
        .i32((i64::from(position.0) * 8 + 4) as i32)
        .i32((i64::from(position.1) * 8 + 4) as i32)
        .i32((i64::from(position.2) * 8 + 4) as i32)
        .f32(1.).f32(1.).i64(seed);
    Packet { id: 0x68, payload: fields.into_inner() }
}
// Filled from the pinned original bootstrap registry; see the source ledger.
const STONECUTTER_TAKE_SOUND_ID: i32 = 1431;
