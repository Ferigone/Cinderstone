//! Actual owned chest/barrel block entities. Palette reads alone never create
//! inventories. Unsupported nonempty destruction and loot tables are errors.
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
#[derive(Clone, Debug)]
pub struct NativeContainer {
    pub image: ContainerImage,
    viewers: BTreeSet<i32>,
    next_recheck: Option<i64>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedNativeBlockEntities {
    pub format: String,
    pub entries: Vec<ContainerImage>,
    pub rechecks: Vec<(BlockPosition, i64)>,
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
}
impl NativeContainer {
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
    pub fn preflight_transition(&self, p: BlockPosition, old: u32, new: u32) -> Result<()> {
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
        match container_kind(new) {
            Some(kind) => {
                if let Some(e) = self.entries.get_mut(&p) {
                    if e.image.kind == kind {
                        e.image.state = new;
                        return Ok(());
                    }
                }
                self.entries.insert(
                    p,
                    NativeContainer {
                        image: ContainerImage {
                            position: p,
                            state: new,
                            kind,
                            slots: vec![ItemStack::empty(); 27],
                            custom_name: None,
                            lock: String::new(),
                            retained_nbt: None,
                        },
                        viewers: BTreeSet::new(),
                        next_recheck: None,
                    },
                );
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
        let mut next = Self::default();
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
            next.entries.insert(
                p,
                NativeContainer {
                    image,
                    viewers: BTreeSet::new(),
                    next_recheck: None,
                },
            );
        }
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
        for e in self.entries.values() {
            for s in &e.image.slots {
                s.encode_canonical()?;
            }
        }
        Ok(SavedNativeBlockEntities {
            format: "mc121-native-containers-v1".into(),
            entries: self.images(),
            rechecks: self
                .entries
                .iter()
                .filter_map(|(p, e)| e.next_recheck.map(|t| (*p, t)))
                .collect(),
        })
    }
    pub fn load_nbt(&mut self, p: BlockPosition, state: u32, tag: Tag) -> Result<()> {
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
        self.entries.insert(
            p,
            NativeContainer {
                image: ContainerImage {
                    position: p,
                    state,
                    kind,
                    slots,
                    custom_name: name,
                    lock,
                    retained_nbt: Some(retained),
                },
                viewers: BTreeSet::new(),
                next_recheck: None,
            },
        );
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
    /// Original BlockEntity.getUpdateTag is an empty CompoundTag for chest/barrel.
    pub fn write_chunk_entities(&self, cx: i32, cz: i32, out: &mut Vec<u8>) {
        let entries = self.chunk_entities(cx, cz);
        let mut w = Writer::new();
        w.varint(entries.len() as i32);
        for e in entries {
            let (x, y, z) = e.image.position;
            w.u8(((x.rem_euclid(16) << 4) | z.rem_euclid(16)) as u8)
                .i16(y as i16)
                .varint(e.image.kind.type_id())
                .raw(&[10, 0]);
        }
        out.extend_from_slice(&w.into_inner());
    }
}
