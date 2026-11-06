//! `TimerQueue<MinecraftServer>` — the `ScheduledEvents` member of `level.dat` (`/schedule`):
//! named events with a trigger time and a function or function-tag callback.

use crate::components::java::resource_location;
use crate::nbt::{JavaString, Tag};

/// The two callbacks of `TimerCallbacks.SERVER_CALLBACKS`; the id is a canonical `namespace:path`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimerCallback {
    /// `FunctionCallback` (`minecraft:function`).
    Function(String),
    /// `FunctionTagCallback` (`minecraft:function_tag`).
    FunctionTag(String),
}

/// A fired event, as `tick` hands it to its callback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FiredEvent {
    pub name: JavaString,
    pub trigger_time: i64,
    pub callback: TimerCallback,
}

#[derive(Debug, Clone)]
struct Event {
    trigger_time: i64,
    sequential_id: u64,
    name: JavaString,
    callback: TimerCallback,
}

/// The queue and the table the original keeps next to it. The table (a Guava `HashBasedTable`
/// over `LinkedHashMap`s) is the set of `(name, trigger time)` cells in order of the rows' first
/// insertion; it is only the queue's *index*, and `tick` removes the cell `(name, now)` rather than
/// `(name, trigger time)`, so a cell outlives an event that fired late.
#[derive(Debug, Clone, Default)]
pub struct TimerQueue {
    queue: Vec<Event>,
    sequential_id: u64,
    /// Rows in insertion order, each holding the trigger times of its cells.
    rows: Vec<(JavaString, Vec<i64>)>,
}

/// `CompoundTag.getLong` on a member: any numeric tag through `NumericTag.getAsLong` (a float is
/// cast, a double floored).
fn get_long(tag: &Tag, key: &str) -> i64 {
    match tag.get(key) {
        Some(Tag::Byte(n)) => i64::from(*n),
        Some(Tag::Short(n)) => i64::from(*n),
        Some(Tag::Int(n)) => i64::from(*n),
        Some(Tag::Long(n)) => *n,
        Some(Tag::Float(n)) => *n as i64,
        Some(Tag::Double(n)) => n.floor() as i64,
        _ => 0,
    }
}

fn get_string(tag: &Tag, key: &str) -> JavaString {
    match tag.get(key) {
        Some(Tag::String(text)) => text.clone(),
        _ => JavaString::from(""),
    }
}

/// `TimerCallbacks.deserialize`: `None` for an unknown `Type` or a callback `Name` that
/// `ResourceLocation.parse` rejects (the original logs and skips the event).
fn deserialize_callback(tag: &Tag) -> Option<TimerCallback> {
    let callback = match tag.get("Callback") {
        Some(inner @ Tag::Compound(_)) => inner.clone(),
        _ => Tag::Compound(Vec::new()),
    };
    let kind = resource_location(get_string(&callback, "Type").to_string_lossy().as_ref())?;
    let id = || resource_location(get_string(&callback, "Name").to_string_lossy().as_ref());
    match kind.as_str() {
        "minecraft:function" => id().map(TimerCallback::Function),
        "minecraft:function_tag" => id().map(TimerCallback::FunctionTag),
        _ => None,
    }
}

impl TimerQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// `new TimerQueue(callbacks, dynamic.get("ScheduledEvents").asStream())`: the events of a
    /// list, in order. Anything that is not a list holds no events, and an element that is not
    /// a compound (after unwrapping `{"": x}`, as `NbtOps` does for a list of compounds) is skipped.
    pub fn load(scheduled_events: Option<&Tag>) -> Self {
        let mut queue = Self::new();
        if let Some(Tag::List {
            element_type,
            values,
        }) = scheduled_events
        {
            for element in values {
                let element = match element {
                    Tag::Compound(entries)
                        if *element_type == 10 && entries.len() == 1 && entries[0].0 == "" =>
                    {
                        &entries[0].1
                    }
                    other => other,
                };
                if let Tag::Compound(_) = element {
                    if let Some(callback) = deserialize_callback(element) {
                        queue.schedule(
                            get_string(element, "Name"),
                            get_long(element, "TriggerTime"),
                            callback,
                        );
                    }
                }
            }
        }
        queue
    }

    fn has_cell(&self, name: &JavaString, time: i64) -> bool {
        self.rows
            .iter()
            .any(|(row, times)| row == name && times.contains(&time))
    }

    /// `schedule`: an event whose `(name, time)` cell exists is ignored.
    pub fn schedule(&mut self, name: JavaString, trigger_time: i64, callback: TimerCallback) {
        if self.has_cell(&name, trigger_time) {
            return;
        }
        self.sequential_id += 1;
        match self.rows.iter_mut().find(|(row, _)| *row == name) {
            Some((_, times)) => times.push(trigger_time),
            None => self.rows.push((name.clone(), vec![trigger_time])),
        }
        self.queue.push(Event {
            trigger_time,
            sequential_id: self.sequential_id,
            name,
            callback,
        });
    }

    /// `remove(name)`: every cell of the row goes (the number is the cells', stale ones included);
    /// the events still queued under that name are dropped.
    pub fn remove(&mut self, name: &JavaString) -> usize {
        let Some(index) = self.rows.iter().position(|(row, _)| row == name) else {
            return 0;
        };
        let (_, times) = self.rows.remove(index);
        self.queue.retain(|event| event.name != *name);
        times.len()
    }

    /// `getEventsIds()`: the names that have a cell, in order of first insertion.
    pub fn events_ids(&self) -> Vec<JavaString> {
        self.rows.iter().map(|(row, _)| row.clone()).collect()
    }

    fn earliest(&self) -> Option<usize> {
        (0..self.queue.len()).min_by_key(|index| {
            (
                self.queue[*index].trigger_time,
                self.queue[*index].sequential_id,
            )
        })
    }

    /// One step of `tick(server, now)`: the earliest event due at `now`, taken off the queue, or
    /// `None`. The caller runs its callback (which may schedule more) and calls again, as the
    /// original's loop does. The cell removed is `(name, now)`, not `(name, trigger time)`.
    pub fn pop_due(&mut self, now: i64) -> Option<FiredEvent> {
        let index = self.earliest()?;
        if self.queue[index].trigger_time > now {
            return None;
        }
        let event = self.queue.remove(index);
        if let Some(row) = self.rows.iter().position(|(name, _)| *name == event.name) {
            if let Some(cell) = self.rows[row].1.iter().position(|time| *time == now) {
                self.rows[row].1.remove(cell);
                if self.rows[row].1.is_empty() {
                    self.rows.remove(row);
                }
            }
        }
        Some(FiredEvent {
            name: event.name,
            trigger_time: event.trigger_time,
            callback: event.callback,
        })
    }

    /// The queued events in firing order: trigger time, then the order they were scheduled in.
    fn sorted(&self) -> Vec<&Event> {
        let mut events: Vec<&Event> = self.queue.iter().collect();
        events.sort_by_key(|event| (event.trigger_time, event.sequential_id));
        events
    }

    /// `store()`: the list written to `level.dat`.
    pub fn store(&self) -> Tag {
        use crate::components::text::hash_ordered_compound;
        let values: Vec<Tag> = self
            .sorted()
            .into_iter()
            .map(|event| {
                let (kind, id) = match &event.callback {
                    TimerCallback::Function(id) => ("minecraft:function", id),
                    TimerCallback::FunctionTag(id) => ("minecraft:function_tag", id),
                };
                let callback = hash_ordered_compound(vec![
                    ("Name", Tag::String(JavaString::from(id.as_str()))),
                    ("Type", Tag::String(JavaString::from(kind))),
                ]);
                hash_ordered_compound(vec![
                    ("Name", Tag::String(event.name.clone())),
                    ("TriggerTime", Tag::Long(event.trigger_time)),
                    ("Callback", callback),
                ])
            })
            .collect();
        Tag::List {
            element_type: if values.is_empty() { 0 } else { 10 },
            values,
        }
    }
}
