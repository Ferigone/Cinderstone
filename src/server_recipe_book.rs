//! Native 1.21 per-player recipe-book state, settings, packets and NBT.
//!
//! The current recipe catalogue and synchronous advancement/send callbacks are
//! explicit inputs. This module does not load datapacks, execute advancement
//! rewards or silently replace those systems with empty registries.
use crate::{
    entity_state::java_floor,
    nbt::{JavaString, Tag},
    protocol::{Packet, Reader, Writer},
};
use anyhow::{bail, ensure, Context, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::OnceLock,
};

pub const RECIPE_BOOK_PACKET_ID: i32 = 65;
pub const UPDATE_RECIPES_PACKET_ID: i32 = 119;
pub const RECIPE_BOOK_NBT_TAG: &str = "recipeBook";

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RecipeId(String);
impl RecipeId {
    pub fn parse(input: &str) -> Result<Self> {
        let (namespace, path) =
            input
                .split_once(':')
                .map_or(("minecraft", input), |(namespace, path)| {
                    (
                        if namespace.is_empty() {
                            "minecraft"
                        } else {
                            namespace
                        },
                        path,
                    )
                });
        ensure!(
            namespace
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-".contains(&b)),
            "invalid recipe namespace"
        );
        ensure!(
            path.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"/._-".contains(&b)),
            "invalid recipe path"
        );
        Ok(Self(format!("{namespace}:{path}")))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn from_java(input: &JavaString) -> Result<Self> {
        Self::parse(
            input
                .as_str()
                .context("recipe identifier contains invalid UTF-16")?,
        )
    }
}

impl TryFrom<String> for RecipeId {
    type Error = anyhow::Error;
    fn try_from(value: String) -> Result<Self> {
        Self::parse(&value)
    }
}
impl From<RecipeId> for String {
    fn from(value: RecipeId) -> Self {
        value.0
    }
}
impl fmt::Display for RecipeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum RecipeBookType {
    Crafting = 0,
    Furnace = 1,
    BlastFurnace = 2,
    Smoker = 3,
}
impl RecipeBookType {
    pub const ALL: [Self; 4] = [
        Self::Crafting,
        Self::Furnace,
        Self::BlastFurnace,
        Self::Smoker,
    ];
    pub fn from_ordinal(value: i32) -> Result<Self> {
        Self::ALL
            .get(usize::try_from(value)?)
            .copied()
            .context("unknown recipe book type")
    }
}
const SETTING_NAMES: [[&str; 2]; 4] = [
    ["isGuiOpen", "isFilteringCraftable"],
    ["isFurnaceGuiOpen", "isFurnaceFilteringCraftable"],
    [
        "isBlastingFurnaceGuiOpen",
        "isBlastingFurnaceFilteringCraftable",
    ],
    ["isSmokerGuiOpen", "isSmokerFilteringCraftable"],
];
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeSettings {
    pub open: bool,
    pub filtering: bool,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeBookSettings {
    states: [TypeSettings; 4],
}
impl RecipeBookSettings {
    pub fn get(&self, kind: RecipeBookType) -> TypeSettings {
        self.states[kind as usize]
    }
    pub fn set_open(&mut self, kind: RecipeBookType, value: bool) {
        self.states[kind as usize].open = value;
    }
    pub fn set_filtering(&mut self, kind: RecipeBookType, value: bool) {
        self.states[kind as usize].filtering = value;
    }
    pub fn set(&mut self, kind: RecipeBookType, open: bool, filtering: bool) {
        self.states[kind as usize] = TypeSettings { open, filtering };
    }
    pub fn booleans(&self) -> [bool; 8] {
        std::array::from_fn(|i| {
            if i % 2 == 0 {
                self.states[i / 2].open
            } else {
                self.states[i / 2].filtering
            }
        })
    }
    pub fn from_booleans(values: [bool; 8]) -> Self {
        Self {
            states: std::array::from_fn(|i| TypeSettings {
                open: values[i * 2],
                filtering: values[i * 2 + 1],
            }),
        }
    }
    pub fn read_network(input: &mut Reader<'_>) -> Result<Self> {
        let mut result = Self::default();
        for kind in RecipeBookType::ALL {
            result.set(kind, input.bool()?, input.bool()?);
        }
        Ok(result)
    }
    pub fn write_network(&self, output: &mut Writer) {
        for flag in self.booleans() {
            output.bool(flag);
        }
    }
    pub fn network_bytes(&self) -> [u8; 8] {
        self.booleans().map(u8::from)
    }
    pub fn from_nbt(compound: &Tag) -> Result<Self> {
        ensure!(
            matches!(compound, Tag::Compound(_)),
            "recipe settings require a compound"
        );
        let mut result = Self::default();
        for kind in RecipeBookType::ALL {
            let names = SETTING_NAMES[kind as usize];
            result.set(
                kind,
                nbt_boolean(compound.get(names[0])),
                nbt_boolean(compound.get(names[1])),
            );
        }
        Ok(result)
    }
    pub fn write_nbt(&self, compound: &mut Tag) -> Result<()> {
        ensure!(
            matches!(compound, Tag::Compound(_)),
            "recipe settings require a compound"
        );
        for kind in RecipeBookType::ALL {
            let value = self.get(kind);
            let names = SETTING_NAMES[kind as usize];
            compound.insert(names[0], Tag::Byte(i8::from(value.open)))?;
            compound.insert(names[1], Tag::Byte(i8::from(value.filtering)))?;
        }
        Ok(())
    }
    pub fn replace_from(&mut self, other: &Self) {
        self.states = other.states;
    }
}
fn nbt_boolean(value: Option<&Tag>) -> bool {
    // CompoundTag.getBoolean -> NumericTag.getAsByte. Floating tags use
    // Mth.floor (including its cast/wrapping behavior), then the low byte.
    match value {
        Some(Tag::Byte(v)) => *v != 0,
        Some(Tag::Short(v)) => *v as i8 != 0,
        Some(Tag::Int(v)) => *v as i8 != 0,
        Some(Tag::Long(v)) => *v as i8 != 0,
        Some(Tag::Float(v)) => java_floor(f64::from(*v)) as i8 != 0,
        Some(Tag::Double(v)) => java_floor(*v) as i8 != 0,
        _ => false,
    }
}

/// Facts about the actual RecipeHolder value supplied by a current manager.
/// is_special belongs to the value, not to the spelling of its identifier.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeDescriptor {
    pub id: RecipeId,
    pub is_special: bool,
}
pub trait RecipeLookup {
    fn lookup(&mut self, id: &RecipeId) -> Result<Option<RecipeDescriptor>>;
}
impl<F: FnMut(&RecipeId) -> Result<Option<RecipeDescriptor>>> RecipeLookup for F {
    fn lookup(&mut self, id: &RecipeId) -> Result<Option<RecipeDescriptor>> {
        self(id)
    }
}
#[derive(Debug)]
pub struct VanillaRecipeCatalogue {
    recipes: BTreeMap<RecipeId, RecipeDescriptor>,
}
impl VanillaRecipeCatalogue {
    pub fn get() -> &'static Self {
        static DATA: OnceLock<VanillaRecipeCatalogue> = OnceLock::new();
        DATA.get_or_init(|| {
            #[derive(Deserialize)]
            struct Facts {
                packet_id: i32,
                recipes: Vec<RecipeDescriptor>,
            }
            let facts: Facts =
                serde_json::from_str(include_str!("../fixtures/server-recipe-book-facts.json"))
                    .expect("original recipe-book facts");
            assert_eq!(facts.packet_id, RECIPE_BOOK_PACKET_ID);
            let recipes = facts
                .recipes
                .into_iter()
                .map(|recipe| (recipe.id.clone(), recipe))
                .collect::<BTreeMap<_, _>>();
            assert_eq!(recipes.len(), 1290);
            let native_ids = crate::crafting::RecipeBook::vanilla()
                .recipe_ids()
                .map(|id| RecipeId::parse(id).unwrap())
                .collect::<BTreeSet<_>>();
            assert_eq!(recipes.keys().cloned().collect::<BTreeSet<_>>(), native_ids);
            assert_eq!(
                recipes.values().filter(|recipe| recipe.is_special).count(),
                14
            );
            Self { recipes }
        })
    }
    pub fn descriptor(&self, id: &RecipeId) -> Option<RecipeDescriptor> {
        self.recipes.get(id).cloned()
    }
    pub fn iter(&self) -> impl Iterator<Item = &RecipeDescriptor> {
        self.recipes.values()
    }
    pub fn len(&self) -> usize {
        self.recipes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.recipes.is_empty()
    }
    /// Original normal-login declarations for the pinned vanilla 1.21 data.
    ///
    /// This is a static data packet, including the original tag-loader ordering;
    /// it is not a serializer for custom managers, datapacks or changed tags.
    /// Send it before the per-player recipe-book INIT packet.
    pub fn declarations_packet(&self) -> Packet {
        static PAYLOAD: OnceLock<Vec<u8>> = OnceLock::new();
        let payload = PAYLOAD.get_or_init(|| {
            #[derive(Deserialize)]
            struct Capture {
                protocol: i32,
                packet_id: i32,
                declared_recipe_count: usize,
                payload_bytes: usize,
                payload: String,
            }
            let capture: Capture = serde_json::from_str(include_str!(
                "../fixtures/server-recipe-book-natural-declarations.json"
            ))
            .expect("original normal-login recipe declarations");
            assert_eq!(capture.protocol, 767);
            assert_eq!(capture.packet_id, UPDATE_RECIPES_PACKET_ID);
            assert_eq!(capture.declared_recipe_count, self.len());
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(capture.payload)
                .expect("original recipe declarations base64");
            assert_eq!(bytes.len(), capture.payload_bytes);
            bytes
        });
        Packet {
            id: UPDATE_RECIPES_PACKET_ID,
            payload: payload.clone(),
        }
    }
}

impl RecipeLookup for &VanillaRecipeCatalogue {
    fn lookup(&mut self, id: &RecipeId) -> Result<Option<RecipeDescriptor>> {
        Ok(self.descriptor(id))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum RecipePacketState {
    Init = 0,
    Add = 1,
    Remove = 2,
}
/// Payload of ServerboundRecipeBookChangeSettingsPacket (ahp).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecipeBookSettingsChange {
    pub book_type: RecipeBookType,
    pub is_open: bool,
    pub is_filtering: bool,
}
impl RecipeBookSettingsChange {
    pub fn read(input: &mut Reader<'_>) -> Result<Self> {
        Ok(Self {
            book_type: RecipeBookType::from_ordinal(java_varint(input)?)?,
            is_open: input.bool()?,
            is_filtering: input.bool()?,
        })
    }
    pub fn decode_exact(payload: &[u8]) -> Result<Self> {
        let mut input = Reader::new(payload);
        let change = Self::read(&mut input)?;
        input.finish()?;
        Ok(change)
    }
}
/// Payload of ServerboundRecipeBookSeenRecipePacket (ahq).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecipeBookSeenRecipe {
    pub recipe: RecipeId,
}
impl RecipeBookSeenRecipe {
    pub fn read(input: &mut Reader<'_>) -> Result<Self> {
        Ok(Self {
            recipe: read_id(input)?,
        })
    }
    pub fn decode_exact(payload: &[u8]) -> Result<Self> {
        let mut input = Reader::new(payload);
        let seen = Self::read(&mut input)?;
        input.finish()?;
        Ok(seen)
    }
}
impl RecipePacketState {
    pub fn from_ordinal(value: i32) -> Result<Self> {
        match value {
            0 => Ok(Self::Init),
            1 => Ok(Self::Add),
            2 => Ok(Self::Remove),
            _ => bail!("unknown recipe packet state"),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeBookPacket {
    pub state: RecipePacketState,
    pub recipes: Vec<RecipeId>,
    pub highlights: Vec<RecipeId>,
    pub settings: RecipeBookSettings,
}
impl RecipeBookPacket {
    pub fn encode(&self) -> Result<Packet> {
        let mut output = Writer::new();
        output.varint(self.state as i32);
        self.settings.write_network(&mut output);
        write_ids(&mut output, &self.recipes)?;
        if self.state == RecipePacketState::Init {
            write_ids(&mut output, &self.highlights)?;
        }
        Ok(Packet {
            id: RECIPE_BOOK_PACKET_ID,
            payload: output.into_inner(),
        })
    }
    /// Actual codec read: trailing bytes remain available to the framing layer.
    pub fn read(input: &mut Reader<'_>) -> Result<Self> {
        let state = RecipePacketState::from_ordinal(java_varint(input)?)?;
        let settings = RecipeBookSettings::read_network(input)?;
        let recipes = read_ids(input)?;
        let highlights = if state == RecipePacketState::Init {
            read_ids(input)?
        } else {
            Vec::new()
        };
        Ok(Self {
            state,
            recipes,
            highlights,
            settings,
        })
    }
    pub fn decode_exact(payload: &[u8]) -> Result<Self> {
        let mut input = Reader::new(payload);
        let value = Self::read(&mut input)?;
        input.finish()?;
        Ok(value)
    }
}
fn write_ids(output: &mut Writer, ids: &[RecipeId]) -> Result<()> {
    output.varint(i32::try_from(ids.len()).context("recipe collection exceeds Java list length")?);
    for id in ids {
        ensure!(
            id.as_str().len() <= 32767,
            "recipe identifier exceeds FriendlyByteBuf UTF limit"
        );
        output.string(id.as_str());
    }
    Ok(())
}
fn read_ids(input: &mut Reader<'_>) -> Result<Vec<RecipeId>> {
    let length =
        usize::try_from(java_varint(input)?).context("negative recipe collection length")?;
    let mut result = Vec::new();
    for _ in 0..length {
        result.push(read_id(input)?);
    }
    Ok(result)
}
fn read_id(input: &mut Reader<'_>) -> Result<RecipeId> {
    let bytes =
        usize::try_from(java_varint(input)?).context("negative recipe identifier length")?;
    ensure!(
        bytes <= 32767 * 3,
        "recipe identifier encoded UTF length too large"
    );
    let text = std::str::from_utf8(input.bytes(bytes)?).context("invalid recipe UTF-8")?;
    ensure!(
        text.encode_utf16().count() <= 32767,
        "recipe identifier UTF length too large"
    );
    RecipeId::parse(text)
}
fn java_varint(input: &mut Reader<'_>) -> Result<i32> {
    // Unlike the general strict reader, original VarInt.read accepts unused
    // high bits in the fifth byte; a sixth byte is consumed before throwing.
    let mut result = 0_i32;
    for index in 0..6 {
        let byte = input.u8()?;
        ensure!(index < 5, "VarInt too big");
        result |= i32::from(byte & 127).wrapping_shl(index * 7);
        if byte & 128 == 0 {
            return Ok(result);
        }
    }
    unreachable!()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecipeLoadIssue {
    Unrecognized(RecipeId),
    ImproperlyFormatted(JavaString),
}
impl RecipeLoadIssue {
    pub fn message_template(&self) -> &'static str {
        match self {
            Self::Unrecognized(_) => "Tried to load unrecognized recipe: {} removed now.",
            Self::ImproperlyFormatted(_) => {
                "Tried to load improperly formatted recipe: {} removed now."
            }
        }
    }
    pub fn argument(&self) -> JavaString {
        match self {
            Self::Unrecognized(id) => id.as_str().into(),
            Self::ImproperlyFormatted(text) => text.clone(),
        }
    }
}
pub trait RecipeLoadLog {
    fn issue(&mut self, book: &mut ServerRecipeBook, issue: RecipeLoadIssue) -> Result<()>;
}
/// Callbacks run synchronously at the original mutation points. In particular
/// recipe rewards may reenter/mutate this same book before the outer loop next
/// checks known IDs. There is deliberately no default advancement no-op.
pub trait RecipeBookHost {
    fn recipe_unlocked(
        &mut self,
        book: &mut ServerRecipeBook,
        recipe: &RecipeDescriptor,
    ) -> Result<()>;
    fn send_packet(&mut self, book: &mut ServerRecipeBook, packet: RecipeBookPacket) -> Result<()>;
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServerRecipeBook {
    known: BTreeSet<RecipeId>,
    highlight: BTreeSet<RecipeId>,
    settings: RecipeBookSettings,
}
impl ServerRecipeBook {
    pub fn known(&self) -> &BTreeSet<RecipeId> {
        &self.known
    }
    pub fn highlights(&self) -> &BTreeSet<RecipeId> {
        &self.highlight
    }
    pub fn settings(&self) -> RecipeBookSettings {
        self.settings.clone()
    }
    pub fn set_settings(&mut self, settings: &RecipeBookSettings) {
        self.settings.replace_from(settings);
    }
    pub fn set_open(&mut self, kind: RecipeBookType, value: bool) {
        self.settings.set_open(kind, value);
    }
    pub fn set_filtering(&mut self, kind: RecipeBookType, value: bool) {
        self.settings.set_filtering(kind, value);
    }
    pub fn set_setting(&mut self, kind: RecipeBookType, open: bool, filtering: bool) {
        self.settings.set(kind, open, filtering);
    }
    pub fn apply_settings_change(&mut self, change: RecipeBookSettingsChange) {
        self.set_setting(change.book_type, change.is_open, change.is_filtering);
    }
    /// The listener resolves the ID through the current manager before removing
    /// its highlight. Unknown-manager IDs leave the book unchanged.
    pub fn seen_recipe(&mut self, id: &RecipeId, recipes: &mut impl RecipeLookup) -> Result<()> {
        if let Some(recipe) = recipes.lookup(id)? {
            self.remove_highlight(&recipe.id);
        }
        Ok(())
    }
    pub fn contains(&self, id: &RecipeId) -> bool {
        self.known.contains(id)
    }
    pub fn contains_holder(&self, recipe: Option<&RecipeDescriptor>) -> bool {
        recipe.is_some_and(|recipe| self.contains(&recipe.id))
    }
    pub fn will_highlight(&self, id: &RecipeId) -> bool {
        self.highlight.contains(id)
    }
    pub fn add_known(&mut self, recipe: &RecipeDescriptor) {
        if !recipe.is_special {
            self.add_id(recipe.id.clone());
        }
    }
    pub fn add_id(&mut self, id: RecipeId) {
        self.known.insert(id);
    }
    pub fn remove_id(&mut self, id: &RecipeId) {
        self.known.remove(id);
        self.highlight.remove(id);
    }
    pub fn add_highlight(&mut self, id: RecipeId) {
        self.highlight.insert(id);
    }
    pub fn remove_highlight(&mut self, id: &RecipeId) {
        self.highlight.remove(id);
    }
    /// Safe distinct-object copy. Rust's borrows exclude original Java's
    /// invalid self-copy, which clears state and then throws on missing types.
    pub fn copy_from(&mut self, other: &Self) {
        self.known.clone_from(&other.known);
        self.highlight.clone_from(&other.highlight);
        self.settings.replace_from(&other.settings);
    }
    pub fn add_recipes(
        &mut self,
        recipes: &[RecipeDescriptor],
        host: &mut impl RecipeBookHost,
    ) -> Result<i32> {
        let mut sent = Vec::new();
        let mut count = 0_i32;
        for recipe in recipes {
            if !self.known.contains(&recipe.id) && !recipe.is_special {
                self.add_id(recipe.id.clone());
                self.add_highlight(recipe.id.clone());
                sent.push(recipe.id.clone());
                host.recipe_unlocked(self, recipe)?;
                count = count.wrapping_add(1);
            }
        }
        if !sent.is_empty() {
            self.send(RecipePacketState::Add, sent, host)?;
        }
        Ok(count)
    }
    pub fn remove_recipes(
        &mut self,
        recipes: &[RecipeDescriptor],
        host: &mut impl RecipeBookHost,
    ) -> Result<i32> {
        let mut sent = Vec::new();
        let mut count = 0_i32;
        for recipe in recipes {
            if self.known.contains(&recipe.id) {
                self.remove_id(&recipe.id);
                sent.push(recipe.id.clone());
                count = count.wrapping_add(1);
            }
        }
        self.send(RecipePacketState::Remove, sent, host)?;
        Ok(count)
    }
    fn send(
        &mut self,
        state: RecipePacketState,
        recipes: Vec<RecipeId>,
        host: &mut impl RecipeBookHost,
    ) -> Result<()> {
        let packet = RecipeBookPacket {
            state,
            recipes,
            highlights: Vec::new(),
            settings: self.settings(),
        };
        host.send_packet(self, packet)
    }
    pub fn initial_packet(&self) -> RecipeBookPacket {
        RecipeBookPacket {
            state: RecipePacketState::Init,
            recipes: self.known.iter().cloned().collect(),
            highlights: self.highlight.iter().cloned().collect(),
            settings: self.settings(),
        }
    }
    pub fn send_initial(&mut self, host: &mut impl RecipeBookHost) -> Result<()> {
        let packet = self.initial_packet();
        host.send_packet(self, packet)
    }
    pub fn to_nbt(&self) -> Tag {
        const ORDER: [&str; 10] = [
            "recipes",
            "isBlastingFurnaceFilteringCraftable",
            "isSmokerGuiOpen",
            "isFilteringCraftable",
            "toBeDisplayed",
            "isFurnaceGuiOpen",
            "isGuiOpen",
            "isFurnaceFilteringCraftable",
            "isBlastingFurnaceGuiOpen",
            "isSmokerFilteringCraftable",
        ];
        let mut data = Tag::Compound(Vec::new());
        self.settings.write_nbt(&mut data).expect("fresh compound");
        let ids = |values: &BTreeSet<RecipeId>| Tag::List {
            element_type: if values.is_empty() { 0 } else { 8 },
            values: values
                .iter()
                .map(|id| Tag::String(id.as_str().into()))
                .collect(),
        };
        data.insert("recipes", ids(&self.known))
            .expect("fresh compound");
        data.insert("toBeDisplayed", ids(&self.highlight))
            .expect("fresh compound");
        Tag::Compound(
            ORDER
                .into_iter()
                .map(|name| (name.into(), data.get(name).unwrap().clone()))
                .collect(),
        )
    }
    pub fn load_nbt(
        &mut self,
        compound: &Tag,
        recipes: &mut impl RecipeLookup,
        log: &mut impl RecipeLoadLog,
    ) -> Result<()> {
        self.settings = RecipeBookSettings::from_nbt(compound)?;
        for (name, highlight) in [("recipes", false), ("toBeDisplayed", true)] {
            let Some(Tag::List {
                element_type,
                values,
            }) = compound.get(name)
            else {
                continue;
            };
            if *element_type != 8 && !values.is_empty() {
                continue;
            }
            for value in values {
                let Tag::String(text) = value else {
                    bail!("inconsistent string-list NBT input");
                };
                let Ok(id) = RecipeId::from_java(text) else {
                    log.issue(self, RecipeLoadIssue::ImproperlyFormatted(text.clone()))?;
                    continue;
                };
                match recipes.lookup(&id)? {
                    Some(recipe) => {
                        if highlight {
                            self.add_highlight(recipe.id)
                        } else {
                            self.add_known(&recipe)
                        }
                    }
                    None => log.issue(self, RecipeLoadIssue::Unrecognized(id))?,
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../tools/server_recipe_book_tests.rs"]
mod tests;
