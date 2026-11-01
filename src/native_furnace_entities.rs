//! Owned furnace-family block entities. Disk state is separate from empty
//! vanilla chunk update tags and from the live menu/viewer owners.
use super::BlockPosition;
use crate::{
    furnace::{FurnaceKind, FurnaceState},
    inventory::{self, ItemStack},
    nbt::{NbtRoot, Tag},
};
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub fn furnace_kind(state: u32) -> Option<FurnaceKind> {
    match inventory::block_state_name(state)? {
        "furnace" => Some(FurnaceKind::Furnace),
        "blast_furnace" => Some(FurnaceKind::BlastFurnace),
        "smoker" => Some(FurnaceKind::Smoker),
        _ => None,
    }
}
pub fn furnace_name(kind: FurnaceKind) -> &'static str {
    match kind {
        FurnaceKind::Furnace => "minecraft:furnace",
        FurnaceKind::BlastFurnace => "minecraft:blast_furnace",
        FurnaceKind::Smoker => "minecraft:smoker",
    }
}
pub fn furnace_type_id(kind: FurnaceKind) -> i32 {
    // Captured 1.21 minecraft:block_entity_type registry, not menu IDs.
    match kind {
        FurnaceKind::Furnace => 0,
        FurnaceKind::Smoker => 27,
        FurnaceKind::BlastFurnace => 28,
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FurnaceImage {
    pub position: BlockPosition,
    pub state: u32,
    pub furnace_state: FurnaceState,
    pub custom_name: Option<Vec<u8>>,
    pub lock: String,
    pub retained_nbt: Option<Vec<u8>>,
}
/// Identity of one live furnace object in this native block-entity store.
/// This is not a position, block state or persistent identifier. Private world
/// clones retain it; replacing the object allocates another key. Menus do not
/// survive a server restart, so this identity is deliberately absent from NBT
/// and from the saved/certified world image.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FurnaceOwnerKey(pub(super) u64);
/// Exact original owner captured before a different-block replacement.
///
/// The fields are private to the block-entity owner so callers cannot invent a
/// removal receipt from selected slot/reward values. A host stages item drops
/// and experience from this image, drains only a private world clone, and must
/// complete that clone's block transition before publishing it. This is not a
/// claim that the Java onRemove callback clears RecipesUsed: its original
/// object retains those counters until base onRemove discards the object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FurnaceRemovalSnapshot {
    pub(super) image: FurnaceImage,
    pub(super) owner_key: FurnaceOwnerKey,
    pub(super) replacement_state: u32,
    pub(super) viewers: Vec<i32>,
}
impl FurnaceRemovalSnapshot {
    pub fn owner_key(&self) -> FurnaceOwnerKey {
        self.owner_key
    }
    pub fn image(&self) -> &FurnaceImage {
        &self.image
    }
    pub fn replacement_state(&self) -> u32 {
        self.replacement_state
    }
    pub fn slots(&self) -> &[ItemStack; 3] {
        &self.image.furnace_state.slots
    }
    pub fn pending_recipe_rewards(&self) -> Result<Vec<crate::furnace::FurnaceRecipeReward>> {
        self.image.furnace_state.pending_recipe_rewards()
    }
    pub fn viewers(&self) -> &[i32] {
        &self.viewers
    }
}
/// Exact removed container retained by its existing menus. Containers drops
/// empty the slots on that same object, while timers and RecipesUsed remain.
/// This receipt never represents a live block entity at the old position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FurnaceViewerRetention {
    pub(super) removal: FurnaceRemovalSnapshot,
    pub(super) image: FurnaceImage,
}
impl FurnaceViewerRetention {
    pub fn owner_key(&self) -> FurnaceOwnerKey {
        self.removal.owner_key()
    }
    pub fn image(&self) -> &FurnaceImage {
        &self.image
    }
    pub fn viewers(&self) -> &[i32] {
        self.removal.viewers()
    }
}
#[derive(Clone, Debug)]
pub struct NativeFurnace {
    pub image: FurnaceImage,
    pub(super) owner_key: FurnaceOwnerKey,
    pub(super) viewers: BTreeSet<i32>,
}
impl NativeFurnace {
    pub fn new(position: BlockPosition, state: u32, kind: FurnaceKind) -> Self {
        Self {
            image: FurnaceImage {
                position,
                state,
                furnace_state: FurnaceState::new(kind),
                custom_name: None,
                lock: String::new(),
                retained_nbt: None,
            },
            owner_key: FurnaceOwnerKey(0),
            viewers: BTreeSet::new(),
        }
    }
    pub fn state(&self) -> u32 {
        self.image.state
    }
    pub fn owner_key(&self) -> FurnaceOwnerKey {
        self.owner_key
    }
    pub fn kind(&self) -> FurnaceKind {
        self.image.furnace_state.kind
    }
    pub fn has_items(&self) -> bool {
        self.image.furnace_state.slots.iter().any(|s| !s.is_empty())
    }
    pub fn has_pending_rewards(&self) -> bool {
        !self.image.furnace_state.recipes_used.is_empty()
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            furnace_kind(self.image.state) == Some(self.kind()),
            "native furnace kind/state differs"
        );
        i16::try_from(self.image.position.1)
            .context("furnace Y cannot be encoded in chunk packet")?;
        self.image.furnace_state.validate()?;
        if let Some(raw) = &self.image.custom_name {
            Tag::decode_anonymous(raw)?;
        }
        if let Some(raw) = &self.image.retained_nbt {
            ensure!(
                matches!(NbtRoot::decode(raw)?.tag, Tag::Compound(_)),
                "retained furnace NBT is not compound"
            );
        }
        Ok(())
    }
    pub fn from_image(image: FurnaceImage) -> Result<Self> {
        let out = Self {
            image,
            owner_key: FurnaceOwnerKey(0),
            viewers: BTreeSet::new(),
        };
        out.validate()?;
        Ok(out)
    }
    pub fn title(&self) -> Result<Tag> {
        if let Some(raw) = &self.image.custom_name {
            let name = Tag::decode_anonymous(raw)?;
            return Tag::decode_anonymous(&crate::item_nbt::text_component_to_wire(&name)?);
        }
        let key = match self.kind() {
            FurnaceKind::Furnace => "container.furnace",
            FurnaceKind::BlastFurnace => "container.blast_furnace",
            FurnaceKind::Smoker => "container.smoker",
        };
        Ok(Tag::Compound(vec![(
            "translate".into(),
            Tag::String(key.into()),
        )]))
    }
    pub fn can_unlock(&self, held: &ItemStack, spectator: bool) -> Result<bool> {
        if spectator || self.image.lock.is_empty() {
            return Ok(true);
        }
        let Some(raw) = held.effective_component_bytes(5) else {
            return Ok(false);
        };
        let persistent = crate::item_nbt::text_component_from_wire(raw)?;
        let Tag::String(json) = persistent else {
            bail!("custom name codec returned non-JSON text");
        };
        let value: serde_json::Value =
            serde_json::from_str(json.as_str().context("surrogate custom name")?)?;
        fn plain(v: &serde_json::Value) -> Result<String> {
            match v {
                serde_json::Value::String(s) => Ok(s.clone()),
                serde_json::Value::Array(a) => a.iter().try_fold(String::new(), |mut out, v| {
                    out.push_str(&plain(v)?);
                    Ok(out)
                }),
                serde_json::Value::Object(o) => {
                    let mut out = o
                        .get("text")
                        .and_then(|v| v.as_str())
                        .context("contextual lock component requires actual text resolution")?
                        .to_owned();
                    if let Some(extra) = o.get("extra") {
                        for item in extra.as_array().context("text extra is not array")? {
                            out.push_str(&plain(item)?);
                        }
                    }
                    Ok(out)
                }
                _ => bail!("invalid lock text component"),
            }
        }
        Ok(plain(&value)? == self.image.lock)
    }
    pub fn to_nbt(&self) -> Result<Tag> {
        self.validate()?;
        let mut tag = if let Some(raw) = &self.image.retained_nbt {
            NbtRoot::decode(raw)?.tag
        } else {
            Tag::Compound(vec![])
        };
        let original_items = match tag.get("Items") {
            Some(Tag::List { values, .. }) => values.clone(),
            _ => vec![],
        };
        let Tag::Compound(values) = &mut tag else {
            bail!("retained furnace NBT is not compound");
        };
        values.retain(|(key, _)| {
            !matches!(
                key.as_str(),
                Some(
                    "id" | "x"
                        | "y"
                        | "z"
                        | "Items"
                        | "Lock"
                        | "CustomName"
                        | "BurnTime"
                        | "CookTime"
                        | "CookTimeTotal"
                        | "RecipesUsed"
                )
            )
        });
        values.push(("id".into(), Tag::String(furnace_name(self.kind()).into())));
        for (name, n) in [
            ("x", self.image.position.0),
            ("y", self.image.position.1),
            ("z", self.image.position.2),
        ] {
            values.push((name.into(), Tag::Int(n)));
        }
        let mut items = Vec::new();
        for (slot, stack) in self.image.furnace_state.slots.iter().enumerate() {
            if stack.is_empty() {
                continue;
            }
            let original = original_items
                .iter()
                .rev()
                .find(|item| numeric_byte(item.get("Slot")) as u8 as usize == slot);
            let mut item = crate::item_nbt::encode_item_stack(stack, original)?;
            item.insert("Slot", Tag::Byte(slot as i8))?;
            items.push(item);
        }
        values.push((
            "Items".into(),
            Tag::List {
                element_type: if items.is_empty() { 0 } else { 10 },
                values: items,
            },
        ));
        // AbstractFurnaceBlockEntity.saveAdditional uses i2s for all three.
        let engine = &self.image.furnace_state;
        values.push(("BurnTime".into(), Tag::Short(engine.lit_time as i16)));
        values.push((
            "CookTime".into(),
            Tag::Short(engine.cooking_progress as i16),
        ));
        values.push((
            "CookTimeTotal".into(),
            Tag::Short(engine.cooking_total_time as i16),
        ));
        let mut used = Vec::new();
        for (id, count) in engine.recipes_used_nbt_entries()? {
            used.push((id.into(), Tag::Int(count)));
        }
        values.push(("RecipesUsed".into(), Tag::Compound(used)));
        if !self.image.lock.is_empty() {
            values.push(("Lock".into(), Tag::String(self.image.lock.clone().into())));
        }
        if let Some(raw) = &self.image.custom_name {
            values.push(("CustomName".into(), Tag::decode_anonymous(raw)?));
        }
        Ok(tag)
    }
    pub fn from_nbt(position: BlockPosition, state: u32, tag: Tag) -> Result<Self> {
        let kind = furnace_kind(state).context("unsupported imported furnace state")?;
        let Tag::Compound(_) = &tag else {
            bail!("furnace NBT must be compound");
        };
        if let Some(Tag::String(id)) = tag.get("id") {
            ensure!(
                id.as_str() == Some(furnace_name(kind)),
                "imported furnace block entity ID differs"
            );
        }
        let mut out = Self::new(position, state, kind);
        if let Some(Tag::List {
            element_type: 10,
            values,
        }) = tag.get("Items")
        {
            for item in values {
                let slot = numeric_byte(item.get("Slot")) as u8 as usize;
                if slot < 3 {
                    out.image.furnace_state.slots[slot] = crate::item_nbt::decode_item_stack(item)?;
                }
            }
        }
        out.image.furnace_state.lit_time = numeric_short(tag.get("BurnTime")) as i32;
        out.image.furnace_state.cooking_progress = numeric_short(tag.get("CookTime")) as i32;
        out.image.furnace_state.cooking_total_time = numeric_short(tag.get("CookTimeTotal")) as i32;
        let fuel = crate::smelting::fuel_time(&out.image.furnace_state.slots[1])?;
        out.image.furnace_state.lit_duration = if kind == FurnaceKind::Furnace {
            fuel
        } else {
            fuel / 2
        };
        let mut used = Vec::new();
        if let Some(Tag::Compound(entries)) = tag.get("RecipesUsed") {
            for (name, value) in entries {
                used.push((
                    name.as_str()
                        .context("surrogate recipe identifier")?
                        .to_owned(),
                    numeric_int(Some(value)),
                ));
            }
        }
        out.image.furnace_state.load_recipes_used(used)?;
        if let Some(Tag::String(lock)) = tag.get("Lock") {
            out.image.lock = lock.as_str().context("surrogate furnace Lock")?.to_owned();
        }
        if let Some(name) = tag.get("CustomName") {
            out.image.custom_name = Some(name.encode_anonymous()?);
        }
        out.image.retained_nbt = Some(
            NbtRoot {
                name: "".into(),
                tag,
            }
            .encode()?,
        );
        out.validate()?;
        Ok(out)
    }
}
// CompoundTag numeric access converts NumericTags; wrong/missing types return0.
fn numeric_int(tag: Option<&Tag>) -> i32 {
    match tag {
        Some(Tag::Byte(v)) => *v as i32,
        Some(Tag::Short(v)) => *v as i32,
        Some(Tag::Int(v)) => *v,
        Some(Tag::Long(v)) => *v as i32,
        Some(Tag::Float(v)) => crate::entity_state::java_floor(*v as f64),
        Some(Tag::Double(v)) => crate::entity_state::java_floor(*v),
        _ => 0,
    }
}
fn numeric_short(tag: Option<&Tag>) -> i16 {
    numeric_int(tag) as i16
}
fn numeric_byte(tag: Option<&Tag>) -> i8 {
    numeric_int(tag) as i8
}
