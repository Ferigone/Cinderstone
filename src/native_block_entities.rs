//! Actual owned chest/barrel and furnace-family block entities. Palette reads alone never create
//! inventories. Nonempty destruction requires actual drop owners; unresolved
//! loot tables and unsupported removal contexts remain errors.
use crate::{
    geometry::Direction,
    inventory::{self, ItemStack},
    nbt::{NbtRoot, Tag},
    protocol::Writer,
};
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
pub type BlockPosition = (i32, i32, i32);
#[path = "native_furnace_entities.rs"]
mod furnace_entities;
pub use furnace_entities::{
    furnace_kind, furnace_name, furnace_type_id, FurnaceImage, FurnaceOwnerKey,
    FurnaceRemovalSnapshot, FurnaceViewerRetention, NativeFurnace,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainerKind {
    Chest,
    Barrel,
}
impl ContainerKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Chest => "minecraft:chest",
            Self::Barrel => "minecraft:barrel",
        }
    }
    pub fn type_id(self) -> i32 {
        match self {
            Self::Chest => 1,
            Self::Barrel => 26,
        }
    }
}
pub fn container_kind(state: u32) -> Option<ContainerKind> {
    match inventory::block_state_name(state)? {
        "chest" => Some(ContainerKind::Chest),
        "barrel" => Some(ContainerKind::Barrel),
        _ => None,
    }
}
pub fn static_conductor(state: u32) -> Result<bool> {
    #[derive(Deserialize)]
    struct Facts {
        blocks: Vec<Block>,
        signal_facts: Vec<u8>,
    }
    #[derive(Deserialize)]
    struct Block {
        class: String,
        min: u32,
        max: u32,
    }
    static FACTS: std::sync::OnceLock<Facts> = std::sync::OnceLock::new();
    let facts = FACTS.get_or_init(|| {
        serde_json::from_str(include_str!("../fixtures/placement-1.21.json"))
            .expect("original conductor facts")
    });
    let b = facts
        .blocks
        .iter()
        .find(|b| (b.min..=b.max).contains(&state))
        .context("unknown conductor state")?;
    ensure!(
        !matches!(b.class.as_str(), "dmy" | "dsu"),
        "dynamic conductor requires actual context"
    );
    Ok(facts
        .signal_facts
        .get(state as usize)
        .context("unknown conductor state")?
        & 1
        != 0)
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerImage {
    pub position: BlockPosition,
    pub state: u32,
    pub kind: ContainerKind,
    pub slots: Vec<ItemStack>,
    pub custom_name: Option<Vec<u8>>,
    pub lock: String,
    pub retained_nbt: Option<Vec<u8>>,
}
/// Identity of an actual live chest/barrel object. Private clones retain it;
/// replacement/import allocates another key. Menus and removal receipts do not
/// survive restart, so this key is intentionally absent from saved images/NBT.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContainerOwnerKey(u64);
/// Original closed container captured before a different-block replacement.
/// Callers cannot construct this receipt from selected contents. Item drops
/// use all 27 original slots; only a private world clone may be drained before
/// its immediate final block transition, never a published empty live owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContainerRemovalSnapshot {
    image: ContainerImage,
    owner_key: ContainerOwnerKey,
    replacement_state: u32,
    viewers: Vec<i32>,
    next_recheck: Option<i64>,
}
impl ContainerRemovalSnapshot {
    pub fn image(&self) -> &ContainerImage {
        &self.image
    }
    pub fn owner_key(&self) -> ContainerOwnerKey {
        self.owner_key
    }
    pub fn replacement_state(&self) -> u32 {
        self.replacement_state
    }
    pub fn slots(&self) -> &[ItemStack] {
        &self.image.slots
    }
    pub fn viewers(&self) -> &[i32] {
        &self.viewers
    }
}
#[derive(Clone, Debug)]
pub struct NativeContainer {
    pub image: ContainerImage,
    owner_key: ContainerOwnerKey,
    viewers: BTreeSet<i32>,
    next_recheck: Option<i64>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedNativeBlockEntities {
    pub format: String,
    pub entries: Vec<ContainerImage>,
    pub rechecks: Vec<(BlockPosition, i64)>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub furnaces: Vec<FurnaceImage>,
    /// None identifies a legacy save that recorded furnace images but no
    /// ticker order. Its record encounter order becomes the registration order
    /// on restore; the unavailable original pre-save order is not reconstructed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub furnace_tick_order: Option<Vec<BlockPosition>>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContainerKey(pub Vec<BlockPosition>);
#[derive(Clone, Debug)]
pub enum ContainerEffect {
    ScheduleBlockTick {
        position: BlockPosition,
        state: u32,
        delay: i64,
    },
    Sound {
        origin: crate::geometry::Vec3,
        sound: &'static str,
        volume: f32,
    },
    GameEvent {
        position: BlockPosition,
        name: &'static str,
    },
    BlockEvent {
        position: BlockPosition,
        state: u32,
        id: i32,
        value: i32,
    },
    SetOpen {
        position: BlockPosition,
        state: u32,
        open: bool,
    },
}
#[derive(Clone, Debug, Default)]
pub struct NativeBlockEntities {
    entries: BTreeMap<BlockPosition, NativeContainer>,
    furnaces: BTreeMap<BlockPosition, NativeFurnace>,
    // Lookup order and ticking order have different owners. The map locates a
    // block entity; this list retains actual fixture registration order.
    furnace_tick_order: Vec<BlockPosition>,
    next_furnace_owner_key: u64,
    next_container_owner_key: u64,
}
impl NativeContainer {
    pub fn owner_key(&self) -> ContainerOwnerKey {
        self.owner_key
    }
    pub fn state(&self) -> u32 {
        self.image.state
    }
    pub fn has_items(&self) -> bool {
        self.image.slots.iter().any(|s| !s.is_empty())
    }
    pub fn to_nbt(&self) -> Result<Tag> {
        let mut root = if let Some(raw) = &self.image.retained_nbt {
            NbtRoot::decode(raw)?.tag
        } else {
            Tag::Compound(vec![])
        };
        let Tag::Compound(ref mut values) = root else {
            bail!("container retained tag is not compound")
        };
        let original_items = values
            .iter()
            .rev()
            .find(|(n, _)| n.as_str() == Some("Items"))
            .and_then(|(_, v)| {
                if let Tag::List { values, .. } = v {
                    Some(values.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default();
        values.retain(|(n, _)| {
            !matches!(
                n.as_str(),
                Some(
                    "id" | "x"
                        | "y"
                        | "z"
                        | "Items"
                        | "Lock"
                        | "CustomName"
                        | "LootTable"
                        | "LootTableSeed"
                )
            )
        });
        values.push(("id".into(), Tag::String(self.image.kind.name().into())));
        for (name, n) in [
            ("x", self.image.position.0),
            ("y", self.image.position.1),
            ("z", self.image.position.2),
        ] {
            values.push((name.into(), Tag::Int(n)));
        }
        let mut items = Vec::new();
        for (i, stack) in self.image.slots.iter().enumerate() {
            if !stack.is_empty() {
                let original = original_items.iter().find(|t| {
                    if let Tag::Compound(v) = t {
                        v.iter()
                            .rev()
                            .find(|(n, _)| n.as_str() == Some("Slot"))
                            .is_some_and(
                                |(_, t)| matches!(t,Tag::Byte(slot)if *slot as u8 as usize==i),
                            )
                    } else {
                        false
                    }
                });
                let mut tag = crate::item_nbt::encode_item_stack(stack, original)?;
                tag.insert("Slot", Tag::Byte(i as i8))?;
                items.push(tag);
            }
        }
        values.push((
            "Items".into(),
            Tag::List {
                element_type: if items.is_empty() { 0 } else { 10 },
                values: items,
            },
        ));
        if !self.image.lock.is_empty() {
            values.push(("Lock".into(), Tag::String(self.image.lock.clone().into())));
        }
        if let Some(name) = &self.image.custom_name {
            let tag = Tag::decode_anonymous(name)?;
            values.push(("CustomName".into(), tag));
        }
        Ok(root)
    }
}
impl NativeBlockEntities {
    pub fn furnace_images(&self) -> Vec<FurnaceImage> {
        self.furnace_records().map(|e| e.image.clone()).collect()
    }
    pub fn furnace_positions(&self) -> Vec<BlockPosition> {
        self.furnace_tick_order.clone()
    }
    pub fn furnace_records(&self) -> impl Iterator<Item = &NativeFurnace> {
        self.furnace_tick_order.iter().map(|p| {
            self.furnaces
                .get(p)
                .expect("registered furnace owner missing")
        })
    }
    fn validate_furnace_tick_order(&self) -> Result<()> {
        ensure!(
            self.furnace_tick_order.len() == self.furnaces.len(),
            "furnace ticker registration count differs"
        );
        let mut seen = BTreeSet::new();
        let mut owners = BTreeSet::new();
        for p in &self.furnace_tick_order {
            ensure!(seen.insert(*p), "duplicate furnace ticker registration");
            let e = self
                .furnaces
                .get(p)
                .context("furnace ticker registration has no owner")?;
            ensure!(
                e.image.position == *p,
                "furnace ticker position differs from owner"
            );
            ensure!(
                e.owner_key.0 != 0 && owners.insert(e.owner_key),
                "furnace object identity is missing or duplicated"
            );
        }
        Ok(())
    }
    fn install_furnace(&mut self, p: BlockPosition, mut entry: NativeFurnace) -> Result<()> {
        ensure!(
            entry.image.position == p,
            "installed furnace position differs"
        );
        entry.validate()?;
        let key = self
            .next_furnace_owner_key
            .checked_add(1)
            .context("native furnace object identity exhausted")?;
        entry.owner_key = FurnaceOwnerKey(key);
        self.next_furnace_owner_key = key;
        let new_registration = self
            .furnaces
            .get(&p)
            .is_none_or(|old| old.kind() != entry.kind());
        if new_registration {
            self.furnace_tick_order.retain(|old| *old != p);
            self.furnace_tick_order.push(p);
        }
        self.furnaces.insert(p, entry);
        Ok(())
    }
    fn remove_furnace(&mut self, p: BlockPosition) {
        if self.furnaces.remove(&p).is_some() {
            self.furnace_tick_order.retain(|old| *old != p);
        }
    }
    pub fn furnace_entry(&self, p: BlockPosition) -> Result<&NativeFurnace> {
        self.furnaces
            .get(&p)
            .context("actual native furnace owner missing")
    }
    pub fn furnace_entry_mut(&mut self, p: BlockPosition) -> Result<&mut NativeFurnace> {
        self.furnaces
            .get_mut(&p)
            .context("actual native furnace owner missing")
    }
    pub fn furnace_owner_key(&self, p: BlockPosition) -> Result<FurnaceOwnerKey> {
        let key = self.furnace_entry(p)?.owner_key();
        ensure!(key.0 != 0, "native furnace object identity missing");
        Ok(key)
    }
    pub fn furnace_validate_at(&self, p: BlockPosition, state: u32) -> Result<()> {
        let kind = furnace_kind(state).context("unsupported native furnace state")?;
        let entry = self.furnace_entry(p)?;
        ensure!(
            entry.image.position == p && entry.image.state == state && entry.kind() == kind,
            "native furnace world state differs"
        );
        entry.validate()
    }
    pub fn furnace_set_state(
        &mut self,
        p: BlockPosition,
        state: crate::furnace::FurnaceState,
    ) -> Result<()> {
        state.validate()?;
        let entry = self.furnace_entry_mut(p)?;
        ensure!(
            entry.kind() == state.kind,
            "native furnace engine kind differs"
        );
        entry.image.furnace_state = state;
        Ok(())
    }
    pub fn furnace_title(&self, p: BlockPosition) -> Result<Tag> {
        self.furnace_entry(p)?.title()
    }
    pub fn furnace_can_unlock(
        &self,
        p: BlockPosition,
        held: &ItemStack,
        spectator: bool,
    ) -> Result<bool> {
        self.furnace_entry(p)?.can_unlock(held, spectator)
    }
    pub fn furnace_start_open(&mut self, p: BlockPosition, entity_id: i32) -> Result<()> {
        self.furnace_entry_mut(p)?.viewers.insert(entity_id);
        Ok(())
    }
    pub fn furnace_stop_open(&mut self, p: BlockPosition, entity_id: i32) -> Result<()> {
        self.furnace_entry_mut(p)?.viewers.remove(&entity_id);
        Ok(())
    }
    pub fn furnace_viewers(&self, p: BlockPosition) -> Result<Vec<i32>> {
        Ok(self.furnace_entry(p)?.viewers.iter().copied().collect())
    }
    /// Capture a removal's actual three slots, used-recipe order and metadata.
    /// Changing LIT/facing retains the same owner and returns None, just as
    /// AbstractFurnaceBlock.onRemove returns for the same block identity.
    pub fn prepare_furnace_removal(
        &self,
        p: BlockPosition,
        old: u32,
        new: u32,
    ) -> Result<Option<FurnaceRemovalSnapshot>> {
        let Some(kind) = furnace_kind(old) else {
            ensure!(
                !self.furnaces.contains_key(&p),
                "orphan furnace at non-furnace removal state"
            );
            return Ok(None);
        };
        self.furnace_validate_at(p, old)?;
        ensure!(
            inventory::block_state_name(new).is_some(),
            "unknown furnace replacement state"
        );
        if furnace_kind(new) == Some(kind) {
            return Ok(None);
        }
        let entry = self.furnace_entry(p)?;
        Ok(Some(FurnaceRemovalSnapshot {
            image: entry.image.clone(),
            owner_key: self.furnace_owner_key(p)?,
            replacement_state: new,
            viewers: entry.viewers.iter().copied().collect(),
        }))
    }
    fn validate_furnace_removal_owner(&self, removal: &FurnaceRemovalSnapshot) -> Result<()> {
        let image = removal.image();
        self.furnace_validate_at(image.position, image.state)?;
        let entry = self.furnace_entry(image.position)?;
        ensure!(
            entry.owner_key() == removal.owner_key(),
            "furnace removal object identity changed"
        );
        ensure!(entry.image == *image, "furnace removal owner changed");
        ensure!(
            entry
                .viewers
                .iter()
                .copied()
                .eq(removal.viewers().iter().copied()),
            "furnace removal viewer ownership changed"
        );
        ensure!(
            furnace_kind(removal.replacement_state()) != Some(entry.kind()),
            "furnace removal receipt retains the same block identity"
        );
        Ok(())
    }
    /// Closed-menu removal remains guarded for callers without a retained
    /// container owner. This never discards viewer registrations.
    pub fn validate_furnace_removal(&self, removal: &FurnaceRemovalSnapshot) -> Result<()> {
        self.validate_furnace_removal_owner(removal)?;
        ensure!(
            removal.viewers().is_empty(),
            "active furnace removal requires actual retained menu owner"
        );
        Ok(())
    }
    /// Capture the old object for every actual menu referring to it. A caller
    /// cannot prove ownership by supplying only a subset or duplicate IDs.
    /// Slots are empty after Containers drops; timers and RecipesUsed survive
    /// until the last menu releases that removed container.
    pub fn prepare_furnace_viewer_retention(
        &self,
        removal: &FurnaceRemovalSnapshot,
        actual_viewers: &[i32],
    ) -> Result<FurnaceViewerRetention> {
        self.validate_furnace_removal_owner(removal)?;
        let viewers = actual_viewers.iter().copied().collect::<BTreeSet<_>>();
        ensure!(
            viewers.len() == actual_viewers.len(),
            "duplicate furnace menu owner"
        );
        ensure!(
            viewers
                .iter()
                .copied()
                .eq(removal.viewers().iter().copied()),
            "actual menus do not own every captured furnace viewer"
        );
        let mut image = removal.image().clone();
        image.furnace_state.slots = std::array::from_fn(|_| ItemStack::empty());
        image.furnace_state.validate()?;
        Ok(FurnaceViewerRetention {
            removal: removal.clone(),
            image,
        })
    }
    /// Permit only a PRIVATE staging world's immediate block transition after
    /// all viewers have an exact retained-container receipt. Generic transition
    /// guards stay unchanged; no cleared live BE or closed menu is published.
    pub fn drain_furnace_removal_retaining_viewers(
        &mut self,
        removal: &FurnaceRemovalSnapshot,
        retention: &FurnaceViewerRetention,
    ) -> Result<()> {
        self.validate_furnace_removal_owner(removal)?;
        ensure!(
            retention.removal == *removal,
            "retained furnace owner receipt differs"
        );
        let entry = self.furnace_entry_mut(removal.image().position)?;
        let mut next = retention.image.furnace_state.clone();
        next.commit_recipe_rewards()?;
        next.validate()?;
        entry.image.furnace_state = next;
        entry.viewers.clear();
        Ok(())
    }
    /// Permit the ordinary transition guard on a PRIVATE staged world clone.
    ///
    /// The host must first prepare real inventory drops and recipe XP using
    /// `removal`, then immediately replace the clone's block and publish only
    /// that final world in its shared entity/random transaction. Draining a
    /// clone is not an emitted live inventory/recipe-clear callback. The normal
    /// transition still removes the block entity and ticker; its nonempty and
    /// active-viewer guards remain unchanged for every other caller.
    pub fn drain_furnace_removal(&mut self, removal: &FurnaceRemovalSnapshot) -> Result<()> {
        self.validate_furnace_removal(removal)?;
        let mut next = removal.image().furnace_state.clone();
        next.slots = std::array::from_fn(|_| ItemStack::empty());
        next.commit_recipe_rewards()?;
        next.validate()?;
        self.furnace_entry_mut(removal.image().position)?
            .image
            .furnace_state = next;
        Ok(())
    }
    fn install_container(&mut self, p: BlockPosition, image: ContainerImage) -> Result<()> {
        ensure!(
            image.position == p
                && container_kind(image.state) == Some(image.kind)
                && image.slots.len() == 27,
            "installed container owner/state/slots differ"
        );
        let key = self
            .next_container_owner_key
            .checked_add(1)
            .context("native container object identity exhausted")?;
        self.next_container_owner_key = key;
        self.entries.insert(
            p,
            NativeContainer {
                image,
                owner_key: ContainerOwnerKey(key),
                viewers: BTreeSet::new(),
                next_recheck: None,
            },
        );
        Ok(())
    }
    fn validate_closed_container_removal_profile(
        &self,
        p: BlockPosition,
        state: u32,
    ) -> Result<()> {
        self.validate_at(p, state)?;
        let entry = self.entry(p)?;
        ensure!(entry.image.position == p, "container position differs");
        ensure!(
            entry.owner_key.0 != 0,
            "native container object identity missing"
        );
        ensure!(
            entry.viewers.is_empty(),
            "active container removal requires actual retained menu owners"
        );
        match entry.image.kind {
            ContainerKind::Chest => {
                ensure!(
                    inventory::block_state_property(state, "type").as_deref() == Some("single"),
                    "paired chest removal requires actual combined-container owner"
                );
                ensure!(
                    inventory::block_state_property(state, "waterlogged").as_deref()
                        == Some("false"),
                    "waterlogged chest removal requires actual fluid callback composition"
                );
            }
            ContainerKind::Barrel => ensure!(
                inventory::block_state_property(state, "open").as_deref() == Some("false"),
                "open barrel removal requires actual opener callback composition"
            ),
        }
        for stack in &entry.image.slots {
            stack.encode_canonical()?;
        }
        if let Some(raw) = &entry.image.retained_nbt {
            let Tag::Compound(values) = NbtRoot::decode(raw)?.tag else {
                bail!("container retained tag is not compound")
            };
            ensure!(
                !values
                    .iter()
                    .any(|(name, _)| name.as_str() == Some("LootTable")),
                "container removal requires actual unresolved loot-table owner"
            );
        }
        Ok(())
    }
    /// Capture the actual 27-slot owner before different-block replacement.
    /// Same-block state changes preserve that object and do not drop contents.
    /// This bounded path rejects viewers, paired/waterlogged chests and open
    /// barrels before any caller consumes shared RNG or allocates item IDs.
    pub fn prepare_container_removal(
        &self,
        p: BlockPosition,
        old: u32,
        new: u32,
    ) -> Result<Option<ContainerRemovalSnapshot>> {
        let Some(kind) = container_kind(old) else {
            ensure!(
                !self.entries.contains_key(&p),
                "orphan container at non-container removal state"
            );
            return Ok(None);
        };
        self.validate_at(p, old)?;
        ensure!(
            inventory::block_state_name(new).is_some(),
            "unknown container replacement state"
        );
        if container_kind(new) == Some(kind) {
            return Ok(None);
        }
        self.validate_closed_container_removal_profile(p, old)?;
        let entry = self.entry(p)?;
        Ok(Some(ContainerRemovalSnapshot {
            image: entry.image.clone(),
            owner_key: entry.owner_key,
            replacement_state: new,
            viewers: entry.viewers.iter().copied().collect(),
            next_recheck: entry.next_recheck,
        }))
    }
    pub fn validate_container_removal(&self, removal: &ContainerRemovalSnapshot) -> Result<()> {
        self.validate_closed_container_removal_profile(
            removal.image.position,
            removal.image.state,
        )?;
        let entry = self.entry(removal.image.position)?;
        ensure!(
            entry.owner_key == removal.owner_key,
            "container removal object identity changed"
        );
        ensure!(
            entry.image == removal.image,
            "container removal owner image changed"
        );
        ensure!(
            entry
                .viewers
                .iter()
                .copied()
                .eq(removal.viewers.iter().copied())
                && removal.viewers.is_empty(),
            "container removal viewer ownership changed"
        );
        ensure!(
            entry.next_recheck == removal.next_recheck,
            "container removal recheck ownership changed"
        );
        ensure!(
            inventory::block_state_name(removal.replacement_state).is_some()
                && container_kind(removal.replacement_state) != Some(entry.image.kind),
            "container removal receipt retains the same block identity"
        );
        Ok(())
    }
    /// Drain only a PRIVATE staged clone after preparing the real drop batch.
    /// Its caller must immediately perform the captured block replacement and
    /// publish only the final world in the shared item/random transaction.
    /// Ordinary transition guards still reject nonempty containers and viewers.
    pub fn drain_container_removal(&mut self, removal: &ContainerRemovalSnapshot) -> Result<()> {
        self.validate_container_removal(removal)?;
        self.entries
            .get_mut(&removal.image.position)
            .expect("validated exact container owner")
            .image
            .slots
            .fill(ItemStack::empty());
        Ok(())
    }
    pub fn images(&self) -> Vec<ContainerImage> {
        self.entries.values().map(|e| e.image.clone()).collect()
    }
    pub fn records(&self) -> impl Iterator<Item = &NativeContainer> {
        self.entries.values()
    }
    pub fn entry(&self, p: BlockPosition) -> Result<&NativeContainer> {
        self.entries
            .get(&p)
            .context("actual native container owner missing")
    }
    pub fn validate_at(&self, p: BlockPosition, state: u32) -> Result<()> {
        let kind = container_kind(state).context("unsupported native block entity")?;
        let e = self.entry(p)?;
        ensure!(
            e.image.state == state && e.image.kind == kind && e.image.slots.len() == 27,
            "native container state/slots differ"
        );
        Ok(())
    }
    pub fn validate_owned_at(&self, p: BlockPosition, state: u32) -> Result<()> {
        if furnace_kind(state).is_some() {
            self.furnace_validate_at(p, state)
        } else {
            self.validate_at(p, state)
        }
    }
    pub fn preflight_transition(&self, p: BlockPosition, old: u32, new: u32) -> Result<()> {
        if let Some(kind) = furnace_kind(old) {
            self.furnace_validate_at(p, old)?;
            if furnace_kind(new) != Some(kind) {
                let e = self.furnace_entry(p)?;
                ensure!(
                    !e.has_items(),
                    "nonempty furnace destruction requires actual drops owner"
                );
                ensure!(
                    !e.has_pending_rewards(),
                    "furnace destruction requires actual experience owner"
                );
                ensure!(
                    e.viewers.is_empty(),
                    "active furnace removal requires actual close owner"
                );
            }
        } else {
            ensure!(
                !self.furnaces.contains_key(&p),
                "orphan furnace at non-furnace state"
            );
        }
        if let Some(kind) = container_kind(old) {
            self.validate_at(p, old)?;
            if container_kind(new) != Some(kind) {
                let e = self.entry(p)?;
                ensure!(
                    !e.has_items(),
                    "nonempty container destruction requires actual drops owner"
                );
                ensure!(
                    e.viewers.is_empty(),
                    "active container removal requires actual close owner"
                );
            }
        } else {
            ensure!(
                !self.entries.contains_key(&p),
                "orphan container at non-container state"
            );
        }
        Ok(())
    }
    pub fn transition(&mut self, p: BlockPosition, old: u32, new: u32) -> Result<()> {
        self.preflight_transition(p, old, new)?;
        if let Some(kind) = furnace_kind(new) {
            self.entries.remove(&p);
            if let Some(e) = self.furnaces.get_mut(&p) {
                if e.kind() == kind {
                    e.image.state = new;
                    return Ok(());
                }
            }
            self.install_furnace(p, NativeFurnace::new(p, new, kind))?;
            return Ok(());
        }
        self.remove_furnace(p);
        match container_kind(new) {
            Some(kind) => {
                if let Some(e) = self.entries.get_mut(&p) {
                    if e.image.kind == kind {
                        e.image.state = new;
                        return Ok(());
                    }
                }
                self.install_container(
                    p,
                    ContainerImage {
                        position: p,
                        state: new,
                        kind,
                        slots: vec![ItemStack::empty(); 27],
                        custom_name: None,
                        lock: String::new(),
                        retained_nbt: None,
                    },
                )?;
            }
            None => {
                self.entries.remove(&p);
            }
        }
        Ok(())
    }
    pub fn restore(
        &mut self,
        saved: SavedNativeBlockEntities,
        mut block: impl FnMut(BlockPosition) -> u32,
    ) -> Result<()> {
        ensure!(
            saved.format == "mc121-native-containers-v1",
            "unknown native container save"
        );
        let mut next = Self {
            // A restore in this running process replaces objects even when
            // their serialized images are equal. Old receipts must not revive.
            next_container_owner_key: self.next_container_owner_key,
            ..Self::default()
        };
        for image in saved.entries {
            ensure!(
                !next.entries.contains_key(&image.position),
                "duplicate container position"
            );
            ensure!(
                container_kind(image.state) == Some(image.kind)
                    && block(image.position) == image.state
                    && image.slots.len() == 27,
                "saved container owner/state differs"
            );
            for s in &image.slots {
                s.encode_canonical()?;
            }
            if let Some(n) = &image.custom_name {
                Tag::decode_anonymous(n)?;
            }
            let p = image.position;
            next.install_container(p, image)?;
        }
        // Legacy record order is an observed restore encounter order only;
        // it cannot recover a ticker order never stored by that older build.
        let legacy_furnace_order: Vec<_> =
            saved.furnaces.iter().map(|image| image.position).collect();
        for image in saved.furnaces {
            let p = image.position;
            ensure!(
                !next.entries.contains_key(&p) && !next.furnaces.contains_key(&p),
                "duplicate furnace/container position"
            );
            ensure!(block(p) == image.state, "saved furnace world state differs");
            next.install_furnace(p, NativeFurnace::from_image(image)?)?;
        }
        next.furnace_tick_order = saved.furnace_tick_order.unwrap_or(legacy_furnace_order);
        next.validate_furnace_tick_order()?;
        for (p, t) in saved.rechecks {
            next.entries
                .get_mut(&p)
                .context("saved recheck has no container")?
                .next_recheck = Some(t);
        }
        *self = next;
        Ok(())
    }
    pub fn save(&self, _now: i64) -> Result<SavedNativeBlockEntities> {
        self.validate_furnace_tick_order()?;
        for e in self.entries.values() {
            for s in &e.image.slots {
                s.encode_canonical()?;
            }
        }
        for e in self.furnaces.values() {
            e.validate()?;
        }
        Ok(SavedNativeBlockEntities {
            format: "mc121-native-containers-v1".into(),
            entries: self.images(),
            furnaces: self.furnace_images(),
            furnace_tick_order: Some(self.furnace_tick_order.clone()),
            rechecks: self
                .entries
                .iter()
                .filter_map(|(p, e)| e.next_recheck.map(|t| (*p, t)))
                .collect(),
        })
    }
    pub fn load_nbt(&mut self, p: BlockPosition, state: u32, tag: Tag) -> Result<()> {
        if furnace_kind(state).is_some() {
            let entry = NativeFurnace::from_nbt(p, state, tag)?;
            ensure!(
                !self.entries.contains_key(&p),
                "furnace import overlaps another container owner"
            );
            self.install_furnace(p, entry)?;
            return Ok(());
        }
        ensure!(
            !self.furnaces.contains_key(&p),
            "container import overlaps a furnace owner"
        );
        let kind = container_kind(state).context("unsupported imported container")?;
        let Tag::Compound(values) = &tag else {
            bail!("containerNBT must be compound")
        };
        ensure!(
            !values.iter().any(|(n, _)| n.as_str() == Some("LootTable")),
            "unresolved container LootTable requires actual loot owner"
        );
        let mut slots = vec![ItemStack::empty(); 27];
        let mut lock = String::new();
        let mut name = None;
        for (k, v) in values {
            match (k.as_str(), v) {
                (
                    Some("Items"),
                    Tag::List {
                        element_type: 10,
                        values,
                    },
                ) => {
                    for item in values {
                        let Tag::Compound(v) = item else {
                            bail!("containeritem compound required")
                        };
                        let slot = v
                            .iter()
                            .rev()
                            .find(|(k, _)| k.as_str() == Some("Slot"))
                            .and_then(|(_, t)| {
                                if let Tag::Byte(n) = t {
                                    Some(*n as u8 as usize)
                                } else {
                                    None
                                }
                            });
                        if let Some(i) = slot.filter(|i| *i < 27) {
                            slots[i] = crate::item_nbt::decode_item_stack(item)?;
                        }
                    }
                }
                (Some("Items"), Tag::List { values, .. }) if values.is_empty() => {}
                (Some("Lock"), Tag::String(s)) => {
                    lock = s.as_str().context("unsupported surrogate Lock")?.to_owned()
                }
                (Some("CustomName"), _) => {
                    name = Some(v.encode_anonymous()?);
                }
                _ => {}
            }
        }
        let retained = NbtRoot {
            name: "".into(),
            tag,
        }
        .encode()?;
        self.install_container(
            p,
            ContainerImage {
                position: p,
                state,
                kind,
                slots,
                custom_name: name,
                lock,
                retained_nbt: Some(retained),
            },
        )?;
        Ok(())
    }
    pub fn resolve(
        &self,
        p: BlockPosition,
        mut block: impl FnMut(BlockPosition) -> Result<u32>,
    ) -> Result<ContainerKey> {
        let state = block(p)?;
        self.validate_at(p, state)?;
        if container_kind(state) != Some(ContainerKind::Chest) {
            return Ok(ContainerKey(vec![p]));
        }
        let kind = inventory::block_state_property(state, "type").context("chest type missing")?;
        if kind == "single" {
            return Ok(ContainerKey(vec![p]));
        }
        let facing = match inventory::block_state_property(state, "facing").as_deref() {
            Some("north") => Direction::North,
            Some("south") => Direction::South,
            Some("east") => Direction::East,
            Some("west") => Direction::West,
            _ => bail!("chest facing missing"),
        };
        let direction = if kind == "left" {
            facing.clockwise()
        } else if kind == "right" {
            facing.counter_clockwise()
        } else {
            bail!("invalid chest pair type")
        };
        let (dx, _, dz) = direction.step();
        let other = (p.0 + dx, p.1, p.2 + dz);
        let adjacent = block(other)?;
        self.validate_at(other, adjacent)?;
        ensure!(
            container_kind(adjacent) == Some(ContainerKind::Chest)
                && inventory::block_state_property(adjacent, "facing")
                    == inventory::block_state_property(state, "facing")
                && inventory::block_state_property(adjacent, "type").as_deref()
                    == Some(if kind == "left" { "right" } else { "left" }),
            "chest pair owner differs"
        );
        Ok(ContainerKey(if kind == "right" {
            vec![p, other]
        } else {
            vec![other, p]
        }))
    }
    pub fn read_slots(&self, key: &ContainerKey) -> Result<Vec<ItemStack>> {
        let mut slots = Vec::new();
        for p in &key.0 {
            slots.extend(self.entry(*p)?.image.slots.iter().cloned());
        }
        Ok(slots)
    }
    pub fn preflight_write_slots(&self, key: &ContainerKey, slots: &[ItemStack]) -> Result<()> {
        ensure!(
            slots.len() == key.0.len() * 27,
            "container slot count differs"
        );
        for p in &key.0 {
            self.entry(*p)?;
        }
        for s in slots {
            s.encode_canonical()?;
        }
        Ok(())
    }
    pub fn write_slots(&mut self, key: &ContainerKey, slots: &[ItemStack]) -> Result<()> {
        self.preflight_write_slots(key, slots)?;
        for (i, p) in key.0.iter().enumerate() {
            self.entries
                .get_mut(p)
                .unwrap()
                .image
                .slots
                .clone_from_slice(&slots[i * 27..(i + 1) * 27]);
        }
        Ok(())
    }
    pub fn can_unlock(
        &self,
        key: &ContainerKey,
        held: &ItemStack,
        spectator: bool,
    ) -> Result<bool> {
        if spectator {
            return Ok(true);
        }
        for p in &key.0 {
            let lock = &self.entry(*p)?.image.lock;
            if !lock.is_empty() {
                let Some(bytes) = held.effective_component_bytes(5) else {
                    return Ok(false);
                };
                let persistent = crate::item_nbt::text_component_from_wire(bytes)?;
                let Tag::String(json) = persistent else {
                    bail!("custom name codec returned non-JSON text")
                };
                let value: serde_json::Value =
                    serde_json::from_str(json.as_str().context("surrogate custom name")?)?;
                fn plain(v: &serde_json::Value) -> Result<String> {
                    match v {
                        serde_json::Value::String(s) => Ok(s.clone()),
                        serde_json::Value::Array(a) => {
                            a.iter().try_fold(String::new(), |mut out, v| {
                                out.push_str(&plain(v)?);
                                Ok(out)
                            })
                        }
                        serde_json::Value::Object(o) => {
                            let mut out = o
                                .get("text")
                                .and_then(|v| v.as_str())
                                .context(
                                    "contextual lock component requires actual text resolution",
                                )?
                                .to_owned();
                            if let Some(extra) = o.get("extra") {
                                for e in extra.as_array().context("text extra is not array")? {
                                    out.push_str(&plain(e)?);
                                }
                            }
                            Ok(out)
                        }
                        _ => bail!("invalid lock text component"),
                    }
                }
                if plain(&value)? != *lock {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
    pub fn title(&self, key: &ContainerKey) -> Result<Tag> {
        for p in &key.0 {
            if let Some(name) = &self.entry(*p)?.image.custom_name {
                let persistent = Tag::decode_anonymous(name)?;
                return Tag::decode_anonymous(&crate::item_nbt::text_component_to_wire(
                    &persistent,
                )?);
            }
        }
        Ok(Tag::Compound(vec![(
            "translate".into(),
            Tag::String(
                if key.0.len() == 2 {
                    "container.chestDouble"
                } else if self.entry(key.0[0])?.image.kind == ContainerKind::Barrel {
                    "container.barrel"
                } else {
                    "container.chest"
                }
                .into(),
            ),
        )]))
    }
    fn change_openers(
        &mut self,
        p: BlockPosition,
        new: BTreeSet<i32>,
        now: i64,
        from_recheck: bool,
    ) -> Result<Vec<ContainerEffect>> {
        let e = self
            .entries
            .get_mut(&p)
            .context("opener container missing")?;
        let old = e.viewers.len();
        let n = new.len();
        e.viewers = new;
        let mut effects = Vec::new();
        if (old == 0) != (n == 0) {
            let opening = n != 0;
            let state = e.image.state;
            let mut origin = crate::geometry::Vec3::new(
                f64::from(p.0) + 0.5,
                f64::from(p.1) + 0.5,
                f64::from(p.2) + 0.5,
            );
            if e.image.kind == ContainerKind::Chest {
                let ty = inventory::block_state_property(state, "type").unwrap_or_default();
                if ty != "left" {
                    if ty == "right" {
                        let f =
                            inventory::block_state_property(state, "facing").unwrap_or_default();
                        let facing = match f.as_str() {
                            "north" => Direction::North,
                            "south" => Direction::South,
                            "west" => Direction::West,
                            _ => Direction::East,
                        };
                        let (dx, _, dz) = facing.counter_clockwise().step();
                        origin.x += f64::from(dx) * 0.5;
                        origin.z += f64::from(dz) * 0.5;
                    }
                    effects.push(ContainerEffect::Sound {
                        origin,
                        sound: if opening {
                            "minecraft:block.chest.open"
                        } else {
                            "minecraft:block.chest.close"
                        },
                        volume: 0.5,
                    });
                }
            } else {
                let f = inventory::block_state_property(state, "facing").unwrap_or_default();
                let (dx, dy, dz) = match f.as_str() {
                    "north" => (0, 0, -1),
                    "south" => (0, 0, 1),
                    "west" => (-1, 0, 0),
                    "east" => (1, 0, 0),
                    "down" => (0, -1, 0),
                    _ => (0, 1, 0),
                };
                origin.x += f64::from(dx) / 2.;
                origin.y += f64::from(dy) / 2.;
                origin.z += f64::from(dz) / 2.;
                effects.push(ContainerEffect::Sound {
                    origin,
                    sound: if opening {
                        "minecraft:block.barrel.open"
                    } else {
                        "minecraft:block.barrel.close"
                    },
                    volume: 0.5,
                });
                effects.push(ContainerEffect::SetOpen {
                    position: p,
                    state,
                    open: opening,
                });
            }
            effects.push(ContainerEffect::GameEvent {
                position: p,
                name: if opening {
                    "minecraft:container_open"
                } else {
                    "minecraft:container_close"
                },
            });
        }
        if !from_recheck && old == 0 && n > 0 && e.next_recheck.is_none() {
            e.next_recheck = Some(now.wrapping_add(5));
            effects.push(ContainerEffect::ScheduleBlockTick {
                position: p,
                state: e.image.state,
                delay: 5,
            });
        }
        if e.image.kind == ContainerKind::Chest {
            effects.push(ContainerEffect::BlockEvent {
                position: p,
                state: e.image.state,
                id: 1,
                value: n as i32,
            });
        }
        if from_recheck && n > 0 {
            e.next_recheck = Some(now.wrapping_add(5));
            effects.push(ContainerEffect::ScheduleBlockTick {
                position: p,
                state: e.image.state,
                delay: 5,
            });
        }
        Ok(effects)
    }
    pub fn start_open(
        &mut self,
        key: &ContainerKey,
        id: i32,
        spectator: bool,
        now: i64,
    ) -> Result<Vec<ContainerEffect>> {
        if spectator {
            return Ok(vec![]);
        }
        let mut effects = Vec::new();
        for p in &key.0 {
            let mut viewers = self.entry(*p)?.viewers.clone();
            viewers.insert(id);
            effects.extend(self.change_openers(*p, viewers, now, false)?);
        }
        Ok(effects)
    }
    pub fn stop_open(
        &mut self,
        key: &ContainerKey,
        id: i32,
        spectator: bool,
        now: i64,
    ) -> Result<Vec<ContainerEffect>> {
        if spectator {
            return Ok(vec![]);
        }
        let mut effects = Vec::new();
        for p in &key.0 {
            let mut viewers = self.entry(*p)?.viewers.clone();
            viewers.remove(&id);
            effects.extend(self.change_openers(*p, viewers, now, false)?);
        }
        Ok(effects)
    }
    pub fn recheck(
        &mut self,
        now: i64,
        mut actual: impl FnMut(BlockPosition) -> Vec<i32>,
    ) -> Result<Vec<ContainerEffect>> {
        let positions = self
            .entries
            .iter()
            .filter_map(|(p, e)| e.next_recheck.filter(|t| *t <= now).map(|_| *p))
            .collect::<Vec<_>>();
        let mut effects = Vec::new();
        for p in positions {
            self.entries.get_mut(&p).unwrap().next_recheck = None;
            effects.extend(self.change_openers(p, actual(p).into_iter().collect(), now, true)?);
        }
        Ok(effects)
    }
    pub fn recheck_at(
        &mut self,
        p: BlockPosition,
        now: i64,
        actual: Vec<i32>,
    ) -> Result<Vec<ContainerEffect>> {
        self.entries
            .get_mut(&p)
            .context("scheduled container owner missing")?
            .next_recheck = None;
        self.change_openers(p, actual.into_iter().collect(), now, true)
    }
    pub fn chunk_entities(&self, cx: i32, cz: i32) -> Vec<&NativeContainer> {
        self.entries
            .values()
            .filter(|e| {
                e.image.position.0.div_euclid(16) == cx && e.image.position.2.div_euclid(16) == cz
            })
            .collect()
    }
    /// Chest/barrel and furnace-family inherit the empty getUpdateTag.
    pub fn write_chunk_entities(&self, cx: i32, cz: i32, out: &mut Vec<u8>) {
        let entries = self.chunk_entities(cx, cz);
        let mut w = Writer::new();
        let furnaces: Vec<_> = self
            .furnace_records()
            .filter(|e| {
                e.image.position.0.div_euclid(16) == cx && e.image.position.2.div_euclid(16) == cz
            })
            .collect();
        w.varint((entries.len() + furnaces.len()) as i32);
        for e in entries {
            let (x, y, z) = e.image.position;
            w.u8(((x.rem_euclid(16) << 4) | z.rem_euclid(16)) as u8)
                .i16(y as i16)
                .varint(e.image.kind.type_id())
                .raw(&[10, 0]);
        }
        for e in furnaces {
            let (x, y, z) = e.image.position;
            w.u8(((x.rem_euclid(16) << 4) | z.rem_euclid(16)) as u8)
                .i16(y as i16)
                .varint(furnace_type_id(e.kind()))
                .raw(&[10, 0]);
        }
        out.extend_from_slice(&w.into_inner());
    }
}
