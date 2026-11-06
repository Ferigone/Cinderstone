//! The advancement tree (WP-E5): `AdvancementTree.addAll`, the display layout
//! (`TreeNodePosition.run`) and the body of `ClientboundUpdateAdvancementsPacket`.
//!
//! The original keeps each node's children in a `ReferenceOpenHashSet`, so their order is the
//! identity-hash order of the node objects, which a port cannot reproduce. This port keeps them in
//! insertion order and lets the caller pass any order to [`layout`]; the oracle test passes the
//! original's own order and must then reproduce its positions and packet bytes exactly.

use super::definition::{Advancement, Display, Frame};
use super::progress::AdvancementProgress;
use crate::components::java::{hashmap_order_by_hash, string_hash};
use crate::inventory::ItemStack;
use crate::protocol::Writer;
use std::collections::BTreeMap;

/// `AdvancementTree`: nodes by id, roots and tasks in insertion order (`ObjectLinkedOpenHashSet`).
#[derive(Debug, Clone, Default)]
pub struct AdvancementTree {
    nodes: BTreeMap<String, Node>,
    roots: Vec<String>,
    tasks: Vec<String>,
}

#[derive(Debug, Clone)]
struct Node {
    parent: Option<String>,
    children: Vec<String>,
}

impl AdvancementTree {
    pub fn new() -> Self {
        Self::default()
    }

    /// `AdvancementTree.addAll`: insert every advancement whose parent is known, repeating until
    /// nothing more can be inserted; the ids left over (missing parents) are returned (the original
    /// logs "Couldn't load advancements").
    pub fn add_all<'a>(&mut self, advancements: impl IntoIterator<Item = (&'a str, &'a Advancement)>) -> Vec<String> {
        let mut pending: Vec<(&str, &Advancement)> = advancements.into_iter().collect();
        while !pending.is_empty() {
            let before = pending.len();
            pending.retain(|(id, advancement)| !self.try_insert(id, advancement));
            if pending.len() == before {
                break;
            }
        }
        pending.into_iter().map(|(id, _)| id.to_owned()).collect()
    }

    fn try_insert(&mut self, id: &str, advancement: &Advancement) -> bool {
        let parent = match &advancement.parent {
            Some(parent) if !self.nodes.contains_key(parent) => return false,
            other => other.clone(),
        };
        if let Some(parent) = &parent {
            self.nodes.get_mut(parent).expect("checked").children.push(id.to_owned());
            self.tasks.push(id.to_owned());
        } else {
            self.roots.push(id.to_owned());
        }
        self.nodes.insert(id.to_owned(), Node { parent, children: Vec::new() });
        true
    }

    pub fn contains(&self, id: &str) -> bool {
        self.nodes.contains_key(id)
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn roots(&self) -> &[String] {
        &self.roots
    }

    pub fn parent(&self, id: &str) -> Option<&str> {
        self.nodes.get(id)?.parent.as_deref()
    }

    /// The children of `id` in insertion order.
    pub fn children(&self, id: &str) -> &[String] {
        self.nodes.get(id).map_or(&[], |node| &node.children)
    }

    /// `AdvancementNode.root()`.
    pub fn root_of<'a>(&'a self, mut id: &'a str) -> &'a str {
        while let Some(parent) = self.parent(id) {
            id = parent;
        }
        id
    }

    /// `AdvancementTree.remove(ids)` (children first); returns the removed ids.
    pub fn remove(&mut self, ids: &[&str]) -> Vec<String> {
        let mut removed = Vec::new();
        for id in ids {
            if self.nodes.contains_key(*id) {
                self.remove_node(id, &mut removed);
            }
        }
        removed
    }

    fn remove_node(&mut self, id: &str, removed: &mut Vec<String>) {
        let children = self.nodes.get(id).map(|node| node.children.clone()).unwrap_or_default();
        for child in children {
            self.remove_node(&child, removed);
        }
        if let Some(node) = self.nodes.remove(id) {
            // The original never unlinks the node from its parent's child set.
            match node.parent {
                None => self.roots.retain(|root| root != id),
                Some(_) => self.tasks.retain(|task| task != id),
            }
            removed.push(id.to_owned());
        }
    }
}

/// One `TreeNodePosition`.
struct Position {
    id: String,
    parent: Option<usize>,
    previous_sibling: Option<usize>,
    child_index: i32,
    children: Vec<usize>,
    ancestor: usize,
    thread: Option<usize>,
    x: i32,
    y: f32,
    modifier: f32,
    change: f32,
    shift: f32,
}

struct Layout<'a> {
    nodes: Vec<Position>,
    has_display: &'a dyn Fn(&str) -> bool,
    children_of: &'a dyn Fn(&str) -> Vec<String>,
}

impl Layout<'_> {
    fn create(&mut self, id: &str, parent: Option<usize>, previous_sibling: Option<usize>, child_index: i32, x: i32) -> usize {
        let index = self.nodes.len();
        self.nodes.push(Position {
            id: id.to_owned(),
            parent,
            previous_sibling,
            child_index,
            children: Vec::new(),
            ancestor: index,
            thread: None,
            x,
            y: -1.0,
            modifier: 0.0,
            change: 0.0,
            shift: 0.0,
        });
        let mut last = None;
        for child in (self.children_of)(id) {
            last = self.add_child(index, &child, last);
        }
        index
    }

    fn add_child(&mut self, this: usize, child: &str, mut last: Option<usize>) -> Option<usize> {
        if (self.has_display)(child) {
            let child_index = self.nodes[this].children.len() as i32 + 1;
            let x = self.nodes[this].x + 1;
            let created = self.create(child, Some(this), last, child_index, x);
            self.nodes[this].children.push(created);
            Some(created)
        } else {
            for grandchild in (self.children_of)(child) {
                last = self.add_child(this, &grandchild, last);
            }
            last
        }
    }

    fn previous_or_thread(&self, i: usize) -> Option<usize> {
        self.nodes[i].thread.or_else(|| self.nodes[i].children.first().copied())
    }

    fn next_or_thread(&self, i: usize) -> Option<usize> {
        self.nodes[i].thread.or_else(|| self.nodes[i].children.last().copied())
    }

    fn first_walk(&mut self, i: usize) {
        if self.nodes[i].children.is_empty() {
            self.nodes[i].y = match self.nodes[i].previous_sibling {
                Some(previous) => self.nodes[previous].y + 1.0,
                None => 0.0,
            };
            return;
        }
        let mut default_ancestor: Option<usize> = None;
        for child in self.nodes[i].children.clone() {
            self.first_walk(child);
            default_ancestor = Some(self.apportion(child, default_ancestor.unwrap_or(child)));
        }
        self.execute_shifts(i);
        let children = &self.nodes[i].children;
        let midpoint = (self.nodes[children[0]].y + self.nodes[*children.last().expect("non-empty")].y) / 2.0;
        match self.nodes[i].previous_sibling {
            Some(previous) => {
                self.nodes[i].y = self.nodes[previous].y + 1.0;
                self.nodes[i].modifier = self.nodes[i].y - midpoint;
            }
            None => self.nodes[i].y = midpoint,
        }
    }

    fn second_walk(&mut self, i: usize, m: f32, depth: i32, mut minimum: f32) -> f32 {
        self.nodes[i].y += m;
        self.nodes[i].x = depth;
        if self.nodes[i].y < minimum {
            minimum = self.nodes[i].y;
        }
        let modifier = self.nodes[i].modifier;
        for child in self.nodes[i].children.clone() {
            minimum = self.second_walk(child, m + modifier, depth + 1, minimum);
        }
        minimum
    }

    fn third_walk(&mut self, i: usize, n: f32) {
        self.nodes[i].y += n;
        for child in self.nodes[i].children.clone() {
            self.third_walk(child, n);
        }
    }

    fn execute_shifts(&mut self, i: usize) {
        let mut shift = 0.0f32;
        let mut change = 0.0f32;
        for child in self.nodes[i].children.clone().into_iter().rev() {
            let node = &mut self.nodes[child];
            node.y += shift;
            node.modifier += shift;
            change += node.change;
            shift += node.shift + change;
        }
    }

    fn apportion(&mut self, v: usize, mut default_ancestor: usize) -> usize {
        let Some(previous) = self.nodes[v].previous_sibling else { return default_ancestor };
        let parent = self.nodes[v].parent.expect("a sibling has a parent");
        let mut vip = v;
        let mut vop = v;
        let mut vim = previous;
        let mut vom = self.nodes[parent].children[0];
        let mut sip = self.nodes[vip].modifier;
        let mut sop = self.nodes[vop].modifier;
        let mut sim = self.nodes[vim].modifier;
        let mut som = self.nodes[vom].modifier;
        while let (Some(next_vim), Some(prev_vip)) = (self.next_or_thread(vim), self.previous_or_thread(vip)) {
            vim = next_vim;
            vip = prev_vip;
            vom = self.previous_or_thread(vom).expect("the left contour is as deep");
            vop = self.next_or_thread(vop).expect("the right contour is as deep");
            self.nodes[vop].ancestor = v;
            let shift = self.nodes[vim].y + sim - (self.nodes[vip].y + sip) + 1.0;
            if shift > 0.0 {
                let ancestor = self.get_ancestor(vim, v, default_ancestor);
                self.move_subtree(ancestor, v, shift);
                sip += shift;
                sop += shift;
            }
            sim += self.nodes[vim].modifier;
            sip += self.nodes[vip].modifier;
            som += self.nodes[vom].modifier;
            sop += self.nodes[vop].modifier;
        }
        if let (Some(next), None) = (self.next_or_thread(vim), self.next_or_thread(vop)) {
            self.nodes[vop].thread = Some(next);
            self.nodes[vop].modifier += sim - sop;
        } else {
            if let (Some(previous), None) = (self.previous_or_thread(vip), self.previous_or_thread(vom)) {
                self.nodes[vom].thread = Some(previous);
                self.nodes[vom].modifier += sip - som;
            }
            default_ancestor = v;
        }
        default_ancestor
    }

    fn move_subtree(&mut self, this: usize, other: usize, shift: f32) {
        let subtrees = (self.nodes[other].child_index - self.nodes[this].child_index) as f32;
        if subtrees != 0.0 {
            self.nodes[other].change -= shift / subtrees;
            self.nodes[this].change += shift / subtrees;
        }
        self.nodes[other].shift += shift;
        self.nodes[other].y += shift;
        self.nodes[other].modifier += shift;
    }

    fn get_ancestor(&self, this: usize, v: usize, default_ancestor: usize) -> usize {
        let ancestor = self.nodes[this].ancestor;
        let parent = self.nodes[v].parent.expect("a sibling has a parent");
        if self.nodes[parent].children.contains(&ancestor) {
            ancestor
        } else {
            default_ancestor
        }
    }
}

/// `TreeNodePosition.run(root)`: the `(x, y)` display location of every visible advancement in
/// the root's tree. `children_of` gives each node's children in the order to use (the original's
/// is identity-hash order); `has_display` tells visible advancements, through which invisible
/// ones are skipped.
pub fn layout(root: &str, has_display: &dyn Fn(&str) -> bool, children_of: &dyn Fn(&str) -> Vec<String>) -> Result<BTreeMap<String, (f32, f32)>, String> {
    if !has_display(root) {
        return Err("Can't position children of an invisible root!".to_owned());
    }
    let mut layout = Layout { nodes: Vec::new(), has_display, children_of };
    let root_index = layout.create(root, None, None, 1, 0);
    layout.first_walk(root_index);
    let start = layout.nodes[root_index].y;
    let minimum = layout.second_walk(root_index, 0.0, 0, start);
    if minimum < 0.0 {
        layout.third_walk(root_index, -minimum);
    }
    Ok(layout.nodes.iter().map(|node| (node.id.clone(), (node.x as f32, node.y))).collect())
}

fn write_location(writer: &mut Writer, id: &str) {
    writer.string(id);
}

/// The icon as the network `ItemStack` (`ItemStack.STREAM_CODEC`).
fn icon_stack(display: &Display) -> Result<ItemStack, String> {
    let mut stack = crate::loot::enchant::new_stack(&display.icon.item).map_err(|e| e.to_string())?;
    stack.count = display.icon.count;
    for (name, value) in &display.icon.components {
        let (type_id, bytes) = match name.as_str() {
            "minecraft:item_name" => {
                let component = super::definition::flat_component(value).map_err(|e| e.to_string())?;
                (6, component.to_network_nbt().map_err(|e| e.to_string())?)
            }
            "minecraft:hide_additional_tooltip" => (14, Vec::new()),
            "minecraft:pot_decorations" => {
                // `PotDecorations.ordered()`: always four items, bricks for the missing faces.
                let mut items: Vec<String> = value.as_array().into_iter().flatten().filter_map(|v| v.as_str()).map(str::to_owned).collect();
                items.resize(4, "minecraft:brick".to_owned());
                let mut writer = Writer::new();
                writer.varint(4);
                for item in items {
                    let id = crate::loot::enchant::new_stack(&item).map_err(|e| e.to_string())?.item_id;
                    writer.varint(id);
                }
                (51, writer.into_inner())
            }
            "minecraft:banner_patterns" => {
                let layers = value.as_array().cloned().unwrap_or_default();
                let mut writer = Writer::new();
                writer.varint(layers.len() as i32);
                for layer in layers {
                    let pattern = layer["pattern"].as_str().unwrap_or_default();
                    let bare = pattern.strip_prefix("minecraft:").unwrap_or(pattern);
                    let index = crate::loot::function::BANNER_PATTERNS.iter().position(|p| *p == bare).ok_or("banner pattern")?;
                    let color = crate::loot::function::DYE_COLORS.iter().position(|c| Some(*c) == layer["color"].as_str()).ok_or("dye colour")?;
                    writer.varint(index as i32 + 1).varint(color as i32);
                }
                (49, writer.into_inner())
            }
            other => return Err(format!("icon component {other}")),
        };
        crate::loot::enchant::set_component(&mut stack, type_id, bytes);
    }
    Ok(stack)
}

/// `Advancement.write` for one holder (`AdvancementHolder.STREAM_CODEC`), with the display
/// location `position` (`DisplayInfo` x/y, zero when never laid out).
fn write_advancement(writer: &mut Writer, id: &str, advancement: &Advancement, position: (f32, f32)) -> Result<(), String> {
    write_location(writer, id);
    match &advancement.parent {
        Some(parent) => {
            writer.bool(true);
            write_location(writer, parent);
        }
        None => {
            writer.bool(false);
        }
    }
    match &advancement.display {
        None => {
            writer.bool(false);
        }
        Some(display) => {
            writer.bool(true);
            writer.raw(&display.title.to_network_nbt().map_err(|e| e.to_string())?);
            writer.raw(&display.description.to_network_nbt().map_err(|e| e.to_string())?);
            writer.raw(&icon_stack(display)?.encode());
            writer.varint(match display.frame {
                Frame::Task => 0,
                Frame::Challenge => 1,
                Frame::Goal => 2,
            });
            let mut flags = 0i32;
            if display.background.is_some() {
                flags |= 1;
            }
            if display.show_toast {
                flags |= 2;
            }
            if display.hidden {
                flags |= 4;
            }
            writer.raw(&flags.to_be_bytes());
            if let Some(background) = &display.background {
                write_location(writer, background);
            }
            writer.raw(&position.0.to_be_bytes()).raw(&position.1.to_be_bytes());
        }
    }
    let groups = &advancement.requirements.requirements;
    writer.varint(groups.len() as i32);
    for group in groups {
        writer.varint(group.len() as i32);
        for name in group {
            writer.string(name);
        }
    }
    writer.bool(advancement.sends_telemetry_event);
    Ok(())
}

/// `AdvancementRequirements.names()`: an `ObjectOpenHashSet` filled with `addAll` per requirement,
/// in its iteration order.
fn names_in_set_order(progress: &AdvancementProgress) -> Vec<String> {
    let mut set = crate::scoreboard::fastutil::OpenHashMap::<()>::new();
    for group in &progress.requirements().requirements {
        set.try_capacity(set.len() + group.len());
        for name in group {
            set.insert(name, ());
        }
    }
    set.keys().map(str::to_owned).collect()
}

/// `AdvancementProgress.serializeToNetwork`: the criteria in the iteration order of the original's
/// `HashMap`. The decoded criteria go through `Collectors.toMap` (a default `HashMap`), are copied
/// into a `HashMap` sized for them, then `update` adds the remaining names in `names()` order.
fn write_progress(writer: &mut Writer, progress: &AdvancementProgress, granted_order: &[&str]) {
    let all: Vec<&str> = progress.criteria().map(|(name, _)| name).collect();
    let granted: Vec<&str> = granted_order.iter().copied().filter(|name| all.contains(name)).collect();
    let granted_hashes: Vec<i32> = granted.iter().map(|name| string_hash(name)).collect();
    let copied: Vec<&str> = hashmap_order_by_hash(&granted_hashes, 16).into_iter().map(|i| granted[i]).collect();
    let names = names_in_set_order(progress);
    let mut inserted: Vec<&str> = copied.clone();
    for name in &names {
        if !inserted.contains(&name.as_str()) && all.contains(&name.as_str()) {
            inserted.push(name);
        }
    }
    let hashes: Vec<i32> = inserted.iter().map(|name| string_hash(name)).collect();
    // `new HashMap<>(m)`: `tableSizeFor(m.size() / 0.75 + 1)`.
    let initial = ((copied.len() as f32 / 0.75) + 1.0) as usize;
    let order = hashmap_order_by_hash(&hashes, initial.next_power_of_two());
    writer.varint(inserted.len() as i32);
    for index in order {
        let name = inserted[index];
        writer.string(name);
        match progress.criterion(name).and_then(|c| c.obtained) {
            Some(seconds) => {
                writer.bool(true);
                writer.raw(&(seconds * 1000).to_be_bytes());
            }
            None => {
                writer.bool(false);
            }
        }
    }
}

/// One entry of the packet's `progress` map: the advancement id, its progress, and the order in
/// which the progress's decoded criteria were inserted (for the original's `HashMap` order).
pub struct ProgressEntry<'a> {
    pub id: &'a str,
    pub progress: &'a AdvancementProgress,
    pub granted_order: Vec<&'a str>,
}

/// The body of `ClientboundUpdateAdvancementsPacket`. `added` and `removed` are written in the
/// given order: the original copies them with `List.copyOf` (order kept) and `Set.copyOf` /
/// `Map.copyOf` (a per-run randomised order), so only a single-element set or map has a fixed
/// byte form.
pub fn update_advancements_packet(
    reset: bool,
    added: &[(&str, &Advancement, (f32, f32))],
    removed: &[&str],
    progress: &[ProgressEntry<'_>],
) -> Result<Vec<u8>, String> {
    let mut writer = Writer::new();
    writer.bool(reset);
    writer.varint(added.len() as i32);
    for (id, advancement, position) in added {
        write_advancement(&mut writer, id, advancement, *position)?;
    }
    writer.varint(removed.len() as i32);
    for id in removed {
        write_location(&mut writer, id);
    }
    writer.varint(progress.len() as i32);
    for entry in progress {
        write_location(&mut writer, entry.id);
        write_progress(&mut writer, entry.progress, &entry.granted_order);
    }
    Ok(writer.into_inner())
}
