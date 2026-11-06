//! Boss bars (`BossEvent`, `ServerBossEvent`, `CustomBossEvent`, `CustomBossEvents`) and the
//! `ClientboundBossEventPacket` they send.
//!
//! A library: nothing in `src/` outside this module calls it. The caller owns the players and
//! their connections: it passes player UUIDs in and sends the packets it takes out of
//! [`ServerBossEvent::take_outbox`]. Evidence and limits: `docs/reference/bossbar.md`.

pub mod packet;

#[cfg(test)]
mod tests;

use crate::components::java::{resource_location, resource_location_hash};
use crate::components::text::{hash_ordered_compound, Component};
use crate::nbt::{JavaString, Tag};
use crate::saved_data::java_collections::{JavaHashMap, JavaHashSet, ResourceId};
use crate::scoreboard::format::ChatFormatting;
use packet::{BossOperation, BossPacket};

/// A UUID as 16 big-endian bytes.
pub type Uuid = [u8; 16];

/// `BossEvent.BossBarColor`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BossBarColor {
    Pink,
    Blue,
    Red,
    Green,
    Yellow,
    Purple,
    White,
}

impl BossBarColor {
    pub const ALL: [BossBarColor; 7] = [
        Self::Pink,
        Self::Blue,
        Self::Red,
        Self::Green,
        Self::Yellow,
        Self::Purple,
        Self::White,
    ];

    pub fn name(self) -> &'static str {
        ["pink", "blue", "red", "green", "yellow", "purple", "white"][self.ordinal() as usize]
    }

    pub fn ordinal(self) -> i32 {
        self as i32
    }

    /// `BossBarColor.getFormatting`.
    pub fn formatting(self) -> ChatFormatting {
        match self {
            Self::Pink => ChatFormatting::Red,
            Self::Blue => ChatFormatting::Blue,
            Self::Red => ChatFormatting::DarkRed,
            Self::Green => ChatFormatting::Green,
            Self::Yellow => ChatFormatting::Yellow,
            Self::Purple => ChatFormatting::DarkBlue,
            Self::White => ChatFormatting::White,
        }
    }

    /// `byName`: white for an unknown name.
    pub fn by_name(name: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|color| color.name() == name)
            .unwrap_or(Self::White)
    }
}

/// `BossEvent.BossBarOverlay`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BossBarOverlay {
    Progress,
    Notched6,
    Notched10,
    Notched12,
    Notched20,
}

impl BossBarOverlay {
    pub const ALL: [BossBarOverlay; 5] = [
        Self::Progress,
        Self::Notched6,
        Self::Notched10,
        Self::Notched12,
        Self::Notched20,
    ];

    pub fn name(self) -> &'static str {
        [
            "progress",
            "notched_6",
            "notched_10",
            "notched_12",
            "notched_20",
        ][self.ordinal() as usize]
    }

    pub fn ordinal(self) -> i32 {
        self as i32
    }

    /// `byName`: progress for an unknown name.
    pub fn by_name(name: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|overlay| overlay.name() == name)
            .unwrap_or(Self::Progress)
    }
}

/// `Mth.clamp(float, float, float)`: `value < min ? min : Math.min(value, max)`. `Math.min` hands a
/// NaN back as it is (the operand, not a canonical NaN: `0.0f / 0.0f` on x86 has its sign bit
/// set, and the bits are visible in the progress packet).
fn clamp_f32(value: f32, min: f32, max: f32) -> f32 {
    if value < min {
        min
    } else if value.is_nan() || value <= max {
        value
    } else {
        max
    }
}

/// `ServerBossEvent`: a `BossEvent` that tells the players it has been added to about changes.
#[derive(Debug, Clone)]
pub struct ServerBossEvent {
    id: Uuid,
    name: Component,
    progress: f32,
    color: BossBarColor,
    overlay: BossBarOverlay,
    darken_screen: bool,
    play_boss_music: bool,
    create_world_fog: bool,
    visible: bool,
    players: Vec<Uuid>,
    outbox: Vec<(Uuid, BossPacket)>,
}

impl ServerBossEvent {
    /// `new ServerBossEvent(name, color, overlay)`; the original draws `id` at random
    /// (`Mth.createInsecureUUID`), so the caller supplies it.
    pub fn new(id: Uuid, name: Component, color: BossBarColor, overlay: BossBarOverlay) -> Self {
        Self {
            id,
            name,
            progress: 1.0,
            color,
            overlay,
            darken_screen: false,
            play_boss_music: false,
            create_world_fog: false,
            visible: true,
            players: Vec::new(),
            outbox: Vec::new(),
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn name(&self) -> &Component {
        &self.name
    }
    pub fn progress(&self) -> f32 {
        self.progress
    }
    pub fn color(&self) -> BossBarColor {
        self.color
    }
    pub fn overlay(&self) -> BossBarOverlay {
        self.overlay
    }
    pub fn should_darken_screen(&self) -> bool {
        self.darken_screen
    }
    pub fn should_play_boss_music(&self) -> bool {
        self.play_boss_music
    }
    pub fn should_create_world_fog(&self) -> bool {
        self.create_world_fog
    }
    pub fn is_visible(&self) -> bool {
        self.visible
    }
    /// The players added to the event (not the UUIDs a custom event remembers for offline players).
    pub fn players(&self) -> &[Uuid] {
        &self.players
    }

    /// The packets queued for players since the last call, in the order they were produced.
    pub fn take_outbox(&mut self) -> Vec<(Uuid, BossPacket)> {
        std::mem::take(&mut self.outbox)
    }

    /// `ClientboundBossEventPacket.createAddPacket`.
    pub fn add_packet(&self) -> BossPacket {
        BossPacket {
            id: self.id,
            operation: BossOperation::Add {
                name: self.name.clone(),
                progress: self.progress,
                color: self.color,
                overlay: self.overlay,
                darken_screen: self.darken_screen,
                play_music: self.play_boss_music,
                create_world_fog: self.create_world_fog,
            },
        }
    }

    /// `ClientboundBossEventPacket.createRemovePacket`.
    pub fn remove_packet(&self) -> BossPacket {
        BossPacket {
            id: self.id,
            operation: BossOperation::Remove,
        }
    }

    /// `createUpdateProgressPacket`.
    pub fn update_progress_packet(&self) -> BossPacket {
        BossPacket {
            id: self.id,
            operation: BossOperation::UpdateProgress(self.progress),
        }
    }

    /// `createUpdateNamePacket`.
    pub fn update_name_packet(&self) -> BossPacket {
        BossPacket {
            id: self.id,
            operation: BossOperation::UpdateName(self.name.clone()),
        }
    }

    /// `createUpdateStylePacket`.
    pub fn update_style_packet(&self) -> BossPacket {
        BossPacket {
            id: self.id,
            operation: BossOperation::UpdateStyle {
                color: self.color,
                overlay: self.overlay,
            },
        }
    }

    /// `createUpdatePropertiesPacket`.
    pub fn update_properties_packet(&self) -> BossPacket {
        BossPacket {
            id: self.id,
            operation: self.properties(),
        }
    }

    /// `broadcast`: nothing is sent while the event is hidden.
    fn broadcast(&mut self, operation: BossOperation) {
        if self.visible {
            let packet = BossPacket {
                id: self.id,
                operation,
            };
            for player in self.players.clone() {
                self.outbox.push((player, packet.clone()));
            }
        }
    }

    /// `setProgress`: a value that differs (a NaN always does) is stored and broadcast.
    pub fn set_progress(&mut self, progress: f32) {
        if progress != self.progress {
            self.progress = progress;
            self.broadcast(BossOperation::UpdateProgress(progress));
        }
    }

    pub fn set_color(&mut self, color: BossBarColor) {
        if color != self.color {
            self.color = color;
            self.broadcast(BossOperation::UpdateStyle {
                color: self.color,
                overlay: self.overlay,
            });
        }
    }

    pub fn set_overlay(&mut self, overlay: BossBarOverlay) {
        if overlay != self.overlay {
            self.overlay = overlay;
            self.broadcast(BossOperation::UpdateStyle {
                color: self.color,
                overlay: self.overlay,
            });
        }
    }

    fn properties(&self) -> BossOperation {
        BossOperation::UpdateProperties {
            darken_screen: self.darken_screen,
            play_music: self.play_boss_music,
            create_world_fog: self.create_world_fog,
        }
    }

    pub fn set_darken_screen(&mut self, value: bool) {
        if value != self.darken_screen {
            self.darken_screen = value;
            self.broadcast(self.properties());
        }
    }

    pub fn set_play_boss_music(&mut self, value: bool) {
        if value != self.play_boss_music {
            self.play_boss_music = value;
            self.broadcast(self.properties());
        }
    }

    pub fn set_create_world_fog(&mut self, value: bool) {
        if value != self.create_world_fog {
            self.create_world_fog = value;
            self.broadcast(self.properties());
        }
    }

    /// `setName`: only a different component is stored and broadcast.
    pub fn set_name(&mut self, name: Component) {
        if name != self.name {
            self.name = name.clone();
            self.broadcast(BossOperation::UpdateName(name));
        }
    }

    /// `addPlayer`: a new player is sent the add packet when the event is visible.
    pub fn add_player(&mut self, player: Uuid) {
        if !self.players.contains(&player) {
            self.players.push(player);
            if self.visible {
                let packet = self.add_packet();
                self.outbox.push((player, packet));
            }
        }
    }

    /// `removePlayer`: a player that was there is sent the remove packet when visible.
    pub fn remove_player(&mut self, player: Uuid) {
        if let Some(index) = self.players.iter().position(|p| *p == player) {
            self.players.remove(index);
            if self.visible {
                let packet = self.remove_packet();
                self.outbox.push((player, packet));
            }
        }
    }

    pub fn set_visible(&mut self, visible: bool) {
        if visible != self.visible {
            self.visible = visible;
            let packet = if visible {
                self.add_packet()
            } else {
                self.remove_packet()
            };
            for player in self.players.clone() {
                self.outbox.push((player, packet.clone()));
            }
        }
    }
}

/// `CustomBossEvent`: a boss bar made by `/bossbar`, remembered with the UUIDs of its players
/// (online or not), a value out of a maximum, and an id.
#[derive(Debug, Clone)]
pub struct CustomBossEvent {
    base: ServerBossEvent,
    text_id: String,
    players: JavaHashSet<Uuid>,
    value: i32,
    max: i32,
}

/// Failure of a load: the original throws, which stops loading `level.dat`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BossBarError {
    /// A key that `ResourceLocation.parse` rejects.
    InvalidId(String),
    /// A `Name` that `Component.Serializer.fromJson` rejects (including a missing one).
    Name(String),
    /// A `Players` entry that is not an int array of four.
    Uuid(String),
}

impl std::fmt::Display for BossBarError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidId(id) => write!(formatter, "invalid boss bar id {id}"),
            Self::Name(reason) => write!(formatter, "invalid boss bar name: {reason}"),
            Self::Uuid(reason) => write!(formatter, "invalid boss bar player: {reason}"),
        }
    }
}

impl std::error::Error for BossBarError {}

fn int_array_uuid(tag: &Tag) -> Result<Uuid, BossBarError> {
    let Tag::IntArray(ints) = tag else {
        return Err(BossBarError::Uuid(
            "Expected UUID-Tag to be of type minecraft:int_array".to_owned(),
        ));
    };
    if ints.len() != 4 {
        return Err(BossBarError::Uuid(format!(
            "Expected UUID-Tag to be of length 4, but found {}.",
            ints.len()
        )));
    }
    let mut out = [0u8; 16];
    for (index, int) in ints.iter().enumerate() {
        out[index * 4..index * 4 + 4].copy_from_slice(&int.to_be_bytes());
    }
    Ok(out)
}

fn uuid_int_array(uuid: Uuid) -> Tag {
    Tag::IntArray(
        (0..4)
            .map(|index| {
                i32::from_be_bytes(uuid[index * 4..index * 4 + 4].try_into().expect("4 bytes"))
            })
            .collect(),
    )
}

impl CustomBossEvent {
    /// `new CustomBossEvent(id, name)`: white, progress style, progress 0, value 0 of 100.
    pub fn new(text_id: &str, uuid: Uuid, name: Component) -> Result<Self, BossBarError> {
        let text_id = resource_location(text_id)
            .ok_or_else(|| BossBarError::InvalidId(text_id.to_owned()))?;
        let mut base =
            ServerBossEvent::new(uuid, name, BossBarColor::White, BossBarOverlay::Progress);
        base.set_progress(0.0);
        Ok(Self {
            base,
            text_id,
            players: JavaHashSet::new(),
            value: 0,
            max: 100,
        })
    }

    pub fn text_id(&self) -> &str {
        &self.text_id
    }
    pub fn value(&self) -> i32 {
        self.value
    }
    pub fn max(&self) -> i32 {
        self.max
    }
    pub fn base(&self) -> &ServerBossEvent {
        &self.base
    }
    pub fn base_mut(&mut self) -> &mut ServerBossEvent {
        &mut self.base
    }

    /// The UUIDs of the event's players, online or not, in `HashSet` order.
    pub fn player_uuids(&self) -> Vec<Uuid> {
        self.players.iter_ordered().into_iter().copied().collect()
    }

    pub fn add_player(&mut self, player: Uuid) {
        self.base.add_player(player);
        self.players.add(player);
    }

    pub fn add_offline_player(&mut self, player: Uuid) {
        self.players.add(player);
    }

    pub fn remove_player(&mut self, player: Uuid) {
        self.base.remove_player(player);
        self.players.remove(&player);
    }

    pub fn remove_all_players(&mut self) {
        for player in self.base.players().to_vec() {
            self.remove_player(player);
        }
        self.players.clear();
    }

    pub fn set_value(&mut self, value: i32) {
        self.value = value;
        self.base
            .set_progress(clamp_f32(value as f32 / self.max as f32, 0.0, 1.0));
    }

    pub fn set_max(&mut self, max: i32) {
        self.max = max;
        self.base
            .set_progress(clamp_f32(self.value as f32 / max as f32, 0.0, 1.0));
    }

    /// `setPlayers(collection)`: the players that are not in `online` leave, the ones in `online`
    /// that are not remembered join. True when anything changed.
    ///
    /// The original walks the new players in the order of a hash set of `ServerPlayer`s, which is
    /// ascending entity id for ids below the table size; `online` must be given in that order,
    /// because it decides the order inside the remembered-UUID set when UUIDs collide.
    pub fn set_players(&mut self, online: &[Uuid]) -> bool {
        let mut to_remove: JavaHashSet<Uuid> = JavaHashSet::new();
        for remembered in self.players.iter_ordered() {
            if !online.contains(remembered) {
                to_remove.add(*remembered);
            }
        }
        let mut to_add: Vec<Uuid> = Vec::new();
        for player in online {
            if !self.players.contains(player) && !to_add.contains(player) {
                to_add.push(*player);
            }
        }
        let changed = to_remove.len() > 0 || !to_add.is_empty();
        for uuid in to_remove
            .iter_ordered()
            .into_iter()
            .copied()
            .collect::<Vec<_>>()
        {
            if self.base.players().contains(&uuid) {
                self.remove_player(uuid);
            }
            self.players.remove(&uuid);
        }
        for player in to_add {
            self.add_player(player);
        }
        changed
    }

    /// `onPlayerConnect`: a remembered player that comes online is added again.
    pub fn on_player_connect(&mut self, player: Uuid) {
        if self.players.contains(&player) {
            self.add_player(player);
        }
    }

    /// `onPlayerDisconnect`: the event forgets the connection, not the UUID.
    pub fn on_player_disconnect(&mut self, player: Uuid) {
        self.base.remove_player(player);
    }

    /// `save(Provider)`.
    pub fn save(&self) -> Tag {
        let bool_tag = |value: bool| Tag::Byte(i8::from(value));
        let players = Tag::List {
            element_type: if self.players.is_empty() { 0 } else { 11 },
            values: self
                .players
                .iter_ordered()
                .into_iter()
                .map(|uuid| uuid_int_array(*uuid))
                .collect(),
        };
        hash_ordered_compound(vec![
            (
                "Name",
                Tag::String(JavaString::from(self.base.name.to_json_string().as_str())),
            ),
            ("Visible", bool_tag(self.base.visible)),
            ("Value", Tag::Int(self.value)),
            ("Max", Tag::Int(self.max)),
            (
                "Color",
                Tag::String(JavaString::from(self.base.color.name())),
            ),
            (
                "Overlay",
                Tag::String(JavaString::from(self.base.overlay.name())),
            ),
            ("DarkenScreen", bool_tag(self.base.darken_screen)),
            ("PlayBossMusic", bool_tag(self.base.play_boss_music)),
            ("CreateWorldFog", bool_tag(self.base.create_world_fog)),
            ("Players", players),
        ])
    }

    /// `CustomBossEvent.load`. `uuid` stands for the random UUID the original draws.
    pub fn load(tag: &Tag, text_id: &str, uuid: Uuid) -> Result<Self, BossBarError> {
        use crate::scoreboard::save::{get_boolean, get_int, get_string};
        let name_text = get_string(tag, "Name");
        let name_json = crate::components::lenient::parse(&name_text)
            .map_err(|error| BossBarError::Name(error))?;
        let name = Component::from_json(&name_json)
            .map_err(|error| BossBarError::Name(error.to_string()))?;
        let mut event = Self::new(text_id, uuid, name)?;
        event.base.visible = get_boolean(tag, "Visible");
        event.set_value(get_int(tag, "Value"));
        event.set_max(get_int(tag, "Max"));
        event
            .base
            .set_color(BossBarColor::by_name(&get_string(tag, "Color")));
        event
            .base
            .set_overlay(BossBarOverlay::by_name(&get_string(tag, "Overlay")));
        event
            .base
            .set_darken_screen(get_boolean(tag, "DarkenScreen"));
        event
            .base
            .set_play_boss_music(get_boolean(tag, "PlayBossMusic"));
        event
            .base
            .set_create_world_fog(get_boolean(tag, "CreateWorldFog"));
        // getList("Players", 11): another element type reads as an empty list.
        if let Some(Tag::List {
            element_type,
            values,
        }) = tag.get("Players")
        {
            if values.is_empty() || *element_type == 11 {
                for entry in values {
                    event.add_offline_player(int_array_uuid(entry)?);
                }
            }
        }
        Ok(event)
    }
}

/// `CustomBossEvents`: the custom boss bars of a server, in a `HashMap` keyed by id.
#[derive(Debug, Clone, Default)]
pub struct CustomBossEvents {
    events: JavaHashMap<ResourceId, CustomBossEvent>,
}

impl CustomBossEvents {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, id: &str) -> Option<&CustomBossEvent> {
        self.events.get(&ResourceId(resource_location(id)?))
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut CustomBossEvent> {
        let key = ResourceId(resource_location(id)?);
        self.events.get_mut(&key)
    }

    /// `create`: a new event replaces one with the same id.
    pub fn create(
        &mut self,
        id: &str,
        uuid: Uuid,
        name: Component,
    ) -> Result<&mut CustomBossEvent, BossBarError> {
        let event = CustomBossEvent::new(id, uuid, name)?;
        let key = ResourceId(event.text_id.clone());
        self.events.put(key.clone(), event);
        Ok(self.events.get_mut(&key).expect("just inserted"))
    }

    pub fn remove(&mut self, id: &str) {
        if let Some(canonical) = resource_location(id) {
            self.events.remove(&ResourceId(canonical));
        }
    }

    /// `getIds()`, in `HashMap` order.
    pub fn ids(&self) -> Vec<String> {
        self.events
            .iter_ordered()
            .into_iter()
            .map(|(id, _)| id.0.clone())
            .collect()
    }

    /// `save(Provider)`: a compound by id; its own `HashMap` order decides the written order.
    pub fn save(&self) -> Tag {
        let entries: Vec<(&str, Tag)> = self
            .events
            .iter_ordered()
            .into_iter()
            .map(|(id, event)| (id.0.as_str(), event.save()))
            .collect();
        hash_ordered_compound(entries)
    }

    /// `load(CompoundTag, Provider)`: events are read in the iteration order of the compound, so a
    /// failure leaves the ones before it loaded. `uuid_for` supplies the random UUID of each.
    pub fn load(
        &mut self,
        tag: &Tag,
        mut uuid_for: impl FnMut(&str) -> Uuid,
    ) -> Result<(), BossBarError> {
        let Tag::Compound(entries) = tag else {
            return Ok(());
        };
        let mut decoded: JavaHashMap<JavaString, &Tag> = JavaHashMap::default();
        for (key, value) in entries {
            decoded.put(key.clone(), value);
        }
        for (key, value) in decoded.iter_ordered() {
            let key = key.to_string_lossy().into_owned();
            let canonical =
                resource_location(&key).ok_or_else(|| BossBarError::InvalidId(key.clone()))?;
            // getCompound: a member of another type is an empty compound, whose Name then fails.
            let empty = Tag::Compound(Vec::new());
            let compound = match value {
                Tag::Compound(_) => *value,
                _ => &empty,
            };
            let event = CustomBossEvent::load(compound, &canonical, uuid_for(&canonical))?;
            self.events.put(ResourceId(canonical), event);
        }
        Ok(())
    }

    pub fn on_player_connect(&mut self, player: Uuid) {
        self.for_each_event(|event| event.on_player_connect(player));
    }

    pub fn on_player_disconnect(&mut self, player: Uuid) {
        self.for_each_event(|event| event.on_player_disconnect(player));
    }

    fn for_each_event(&mut self, mut f: impl FnMut(&mut CustomBossEvent)) {
        let ids = self.ids();
        for id in ids {
            if let Some(event) = self.events.get_mut(&ResourceId(id)) {
                f(event);
            }
        }
    }
}

/// `ResourceLocation.hashCode()`, re-exported for callers that key their own maps like the original.
pub fn id_hash(id: &str) -> i32 {
    resource_location_hash(id)
}
