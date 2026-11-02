//! Actual default LivingEntity Brain owner, specialized to its empty schema.
//!
//! LivingEntity291/295 supplies no memory modules or sensors. Ordinary Bat
//! inherits that provider. This module retains the source Brain fields, codec
//! and canonical default objects; it does not replace a missing Brain with a
//! successful callback. Sensor/behavior schemas and arbitrary saved Brain NBT
//! require the real registry codecs and are explicitly outside this domain.

use crate::nbt::Tag;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    convert::Infallible,
    fmt,
    hash::{Hash, Hasher},
    sync::{
        atomic::{AtomicI32, Ordering},
        Arc, OnceLock,
    },
};

/// Source Activity37–71. Construction is private: users of this domain reuse
/// the bootstrap's SAME registered IDLE object rather than constructing a name
/// or treating an ID as an actor/scheduler owner.
#[derive(Debug)]
pub struct BrainActivity {
    name: &'static str,
    hash_code: i32,
}
impl BrainActivity {
    fn new(name: &'static str) -> Self {
        let hash_code = crate::random::java_string_hash(&name.encode_utf16().collect::<Vec<_>>());
        Self { name, hash_code }
    }
    pub fn name(&self) -> &'static str {
        self.name
    }
    pub fn java_hash_code(&self) -> i32 {
        self.hash_code
    }
}
impl PartialEq for BrainActivity {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}
impl Eq for BrainActivity {}
impl Hash for BrainActivity {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_i32(self.hash_code);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrainKeyframe {
    pub time_stamp: i32,
    pub value: f32,
}

/// Source Timeline11–12/42–62. The default schedule has one real keyframe,
/// including its retained previousIndex field. Relaxed atomic access permits
/// sharing that source static safely; this domain's index always remains zero.
#[derive(Debug)]
struct DefaultTimeline {
    keyframes: Vec<BrainKeyframe>,
    previous_index: AtomicI32,
}
impl DefaultTimeline {
    fn idle_at_zero() -> Self {
        Self {
            keyframes: vec![BrainKeyframe {
                time_stamp: 0,
                value: 1.0,
            }],
            previous_index: AtomicI32::new(0),
        }
    }
    fn value_at(&self, time_stamp: i32) -> f32 {
        if self.keyframes.is_empty() {
            return 0.0;
        }
        let previous_index = self.previous_index.load(Ordering::Relaxed) as usize;
        let previous = self.keyframes[previous_index];
        let last = self.keyframes[self.keyframes.len() - 1];
        let wrapped = time_stamp < previous.time_stamp;
        let start = if wrapped { 0 } else { previous_index };
        let mut value = if wrapped { last.value } else { previous.value };
        for index in start..self.keyframes.len() {
            let keyframe = self.keyframes[index];
            if keyframe.time_stamp > time_stamp {
                break;
            }
            self.previous_index.store(index as i32, Ordering::Relaxed);
            value = keyframe.value;
        }
        value
    }
}

/// Source Schedule.EMPTY is the registered "empty" schedule with an IDLE
/// timeline at time zero, NOT an empty/no-op schedule. This is that actual
/// bounded schedule owner, retained by all default Brains.
#[derive(Debug)]
pub struct DefaultBrainSchedule {
    timelines: HashMap<Arc<BrainActivity>, DefaultTimeline>,
}
impl DefaultBrainSchedule {
    fn new(idle: Arc<BrainActivity>) -> Self {
        let mut timelines = HashMap::new();
        timelines.insert(Arc::clone(&idle), DefaultTimeline::idle_at_zero());
        Self { timelines }
    }
    /// Source Schedule63–67 evaluates each actual timeline and chooses its
    /// maximum. The bounded default schedule has exactly one entry; therefore
    /// there is no multi-entry HashMap iteration/tie-order approximation here.
    pub fn activity_at(&self, time_stamp: i32) -> Arc<BrainActivity> {
        let mut selected: Option<(&Arc<BrainActivity>, f32)> = None;
        for (activity, timeline) in &self.timelines {
            let value = timeline.value_at(time_stamp);
            if selected.as_ref().map_or(true, |(_, old)| value >= *old) {
                selected = Some((activity, value));
            }
        }
        selected
            .map(|(activity, _)| Arc::clone(activity))
            .unwrap_or_else(|| default_brain_bootstrap().idle_activity())
    }
    pub fn idle_keyframes(&self) -> &[BrainKeyframe] {
        &self.timelines[&default_brain_bootstrap().idle_activity()].keyframes
    }
}

/// Retained source static objects needed by the default provider. Central
/// registry integration must reuse these references for the listed keys.
/// This bootstrap does not claim the complete Activity/Schedule registries or
/// install a scheduler, world, actor factory or Main tick phase.
#[derive(Debug)]
pub struct DefaultBrainBootstrap {
    idle: Arc<BrainActivity>,
    empty: Arc<DefaultBrainSchedule>,
}
impl DefaultBrainBootstrap {
    pub const IDLE_REGISTRY_KEY: &'static str = "minecraft:idle";
    pub const EMPTY_SCHEDULE_REGISTRY_KEY: &'static str = "minecraft:empty";

    pub fn idle_activity(&self) -> Arc<BrainActivity> {
        Arc::clone(&self.idle)
    }
    pub fn empty_schedule(&self) -> Arc<DefaultBrainSchedule> {
        Arc::clone(&self.empty)
    }
}

/// Source Activity.IDLE and Schedule.EMPTY construction/identity, once for this
/// native object graph. Schedule16 registers/builds IDLE@0 with value1.
pub fn default_brain_bootstrap() -> &'static DefaultBrainBootstrap {
    static BOOTSTRAP: OnceLock<DefaultBrainBootstrap> = OnceLock::new();
    BOOTSTRAP.get_or_init(|| {
        let idle = Arc::new(BrainActivity::new("idle"));
        let empty = Arc::new(DefaultBrainSchedule::new(Arc::clone(&idle)));
        DefaultBrainBootstrap { idle, empty }
    })
}

/// A native boundary refusal, NOT the source codec's logged-error fallback.
/// Supporting these cases needs the actual MemoryModuleType registry, memory
/// codecs and DataResult/log side effects even though this schema is empty.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultBrainInputError {
    RootIsNotCompound,
    MissingMemories,
    MemoriesIsNotCompound,
    NonemptyMemoriesNeedRegistryCodecs,
}
impl fmt::Display for DefaultBrainInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unsupported default Brain NBT: {self:?}")
    }
}
impl std::error::Error for DefaultBrainInputError {}

/// Brain73–110's codec specialized to LivingEntity's exact empty memory and
/// sensor collections. Each provider owns a NEW codec; each resulting Brain
/// retains that SAME codec through its supplier. It is not a global ID codec.
#[derive(Debug)]
pub struct DefaultBrainCodec {
    memory_types: &'static [Infallible],
    sensor_types: &'static [Infallible],
}
impl DefaultBrainCodec {
    pub fn memory_type_count(&self) -> usize {
        self.memory_types.len()
    }
    pub fn sensor_type_count(&self) -> usize {
        self.sensor_types.len()
    }
    fn decode_empty_memories(&self, input: &Tag) -> Result<(), DefaultBrainInputError> {
        if !matches!(input, Tag::Compound(_)) {
            return Err(DefaultBrainInputError::RootIsNotCompound);
        }
        match input.get("memories") {
            None => Err(DefaultBrainInputError::MissingMemories),
            Some(Tag::Compound(entries)) if entries.is_empty() => Ok(()),
            Some(Tag::Compound(_)) => {
                Err(DefaultBrainInputError::NonemptyMemoriesNeedRegistryCodecs)
            }
            Some(_) => Err(DefaultBrainInputError::MemoriesIsNotCompound),
        }
    }
    fn encode_start(&self, brain: &DefaultLivingBrain) -> Tag {
        // Brain105/189/MemoryValue184 visit the REAL retained memory table.
        // Default memory keys are uninhabited; there is no codec/value to
        // invent or serialize. Outer fieldOf("memories") still emits a map.
        let memories = Vec::with_capacity(brain.memories.len());
        Tag::Compound(vec![("memories".into(), Tag::Compound(memories))])
    }
}

/// Source Brain.Provider57–64: two collections plus one newly created codec.
/// Infallible collection elements encode the genuine empty-schema invariant;
/// no fake sensor, memory value or behavior object can enter these tables.
#[derive(Debug)]
pub struct DefaultBrainProvider {
    memory_types: &'static [Infallible],
    sensor_types: &'static [Infallible],
    codec: Arc<DefaultBrainCodec>,
}
impl Default for DefaultBrainProvider {
    fn default() -> Self {
        Self::new()
    }
}
impl DefaultBrainProvider {
    pub fn new() -> Self {
        let memory_types = &[];
        let sensor_types = &[];
        Self {
            memory_types,
            sensor_types,
            codec: Arc::new(DefaultBrainCodec {
                memory_types,
                sensor_types,
            }),
        }
    }
    pub fn codec(&self) -> Arc<DefaultBrainCodec> {
        Arc::clone(&self.codec)
    }
    /// LivingEntity282–283 builds this actual NbtOps value before virtual
    /// makeBrain. This helper does not read arbitrary saved state or select a
    /// different mob's overridden provider.
    pub fn constructor_memories_nbt() -> Tag {
        Tag::Compound(vec![("memories".into(), Tag::Compound(Vec::new()))])
    }
    pub fn make_from_constructor_memories(&self) -> DefaultLivingBrain {
        let input = Self::constructor_memories_nbt();
        // This source-created value is inside the codec's supported domain;
        // no malformed-input fallback or swallowed error executes here.
        self.decode_empty_memories_nbt(&input)
            .expect("source-created Brain memories must be an empty NBT map")
    }
    /// Decode the supported empty-memory NbtOps map, including ordinary saved
    /// {memories:{}}. Unknown outer fields are ignored by source fieldOf.
    /// Every malformed/nonempty memory input explicitly returns a boundary
    /// error; this does not pretend to run Java's parse/log/fallback path.
    pub fn decode_empty_memories_nbt(
        &self,
        input: &Tag,
    ) -> Result<DefaultLivingBrain, DefaultBrainInputError> {
        self.codec.decode_empty_memories(input)?;
        Ok(DefaultLivingBrain::allocate(self))
    }
}

/// Callback-free view of the actual retained source collections/counters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DefaultBrainStateView {
    pub memories: usize,
    pub sensors: usize,
    pub behavior_priorities: usize,
    pub activity_requirements: usize,
    pub activity_memories_to_erase: usize,
    pub core_activities: usize,
    pub active_activities: usize,
    pub last_schedule_update: i64,
}

/// The decoded constructor path's pure Supplier.get retains the SAME codec
/// object. It is a real retained field object, with no substituted host call.
#[derive(Debug)]
struct DefaultBrainCodecSupplier {
    codec: Arc<DefaultBrainCodec>,
}
impl DefaultBrainCodecSupplier {
    fn get(&self) -> Arc<DefaultBrainCodec> {
        Arc::clone(&self.codec)
    }
}

/// Brain115–159's actual peractor state for the base Living provider. The empty
/// schema is a type invariant, not evidence that another mob's AI is empty.
/// Source-owned tables remain actual collections; no separately inferred
/// alive/dead flag or successful scheduler/AI callback is introduced.
#[derive(Debug)]
pub struct DefaultLivingBrain {
    codec_supplier: DefaultBrainCodecSupplier,
    memories: HashMap<Infallible, Option<Infallible>>,
    // Source LinkedHashMap: retain insertion order, with no entries for this
    // schema. There are no fabricated sensor objects or RNG draws.
    sensors: Vec<(Infallible, Infallible)>,
    available_behaviors_by_priority:
        BTreeMap<i32, HashMap<Arc<BrainActivity>, HashSet<Infallible>>>,
    schedule: Arc<DefaultBrainSchedule>,
    activity_requirements: HashMap<Arc<BrainActivity>, HashSet<Infallible>>,
    activity_memories_to_erase_when_stopped: HashMap<Arc<BrainActivity>, HashSet<Infallible>>,
    core_activities: HashSet<Arc<BrainActivity>>,
    active_activities: HashSet<Arc<BrainActivity>>,
    default_activity: Arc<BrainActivity>,
    last_schedule_update: i64,
}
impl DefaultLivingBrain {
    fn allocate(provider: &DefaultBrainProvider) -> Self {
        // Preserve source field-allocation sequence. Registering memory types,
        // creating sensors, adding their requires memories and applying decoded
        // MemoryValues all visit genuinely empty collections in this provider.
        let memories = HashMap::with_capacity(provider.memory_types.len());
        let sensors = Vec::with_capacity(provider.sensor_types.len());
        let available_behaviors_by_priority = BTreeMap::new();
        let schedule = default_brain_bootstrap().empty_schedule();
        let activity_requirements = HashMap::new();
        let activity_memories_to_erase_when_stopped = HashMap::new();
        let core_activities = HashSet::new();
        let active_activities = HashSet::new();
        let default_activity = default_brain_bootstrap().idle_activity();
        let last_schedule_update = -9999;
        let codec_supplier = DefaultBrainCodecSupplier {
            codec: Arc::clone(&provider.codec),
        };
        Self {
            codec_supplier,
            memories,
            sensors,
            available_behaviors_by_priority,
            schedule,
            activity_requirements,
            activity_memories_to_erase_when_stopped,
            core_activities,
            active_activities,
            default_activity,
            last_schedule_update,
        }
    }
    pub fn codec(&self) -> Arc<DefaultBrainCodec> {
        self.codec_supplier.get()
    }
    pub fn schedule(&self) -> Arc<DefaultBrainSchedule> {
        Arc::clone(&self.schedule)
    }
    pub fn default_activity(&self) -> Arc<BrainActivity> {
        Arc::clone(&self.default_activity)
    }
    pub fn state_view(&self) -> DefaultBrainStateView {
        DefaultBrainStateView {
            memories: self.memories.len(),
            sensors: self.sensors.len(),
            behavior_priorities: self.available_behaviors_by_priority.len(),
            activity_requirements: self.activity_requirements.len(),
            activity_memories_to_erase: self.activity_memories_to_erase_when_stopped.len(),
            core_activities: self.core_activities.len(),
            active_activities: self.active_activities.len(),
            last_schedule_update: self.last_schedule_update,
        }
    }
    /// Brain200–203 replaces values at EXISTING keys; it does not remove schema
    /// registrations. LivingEntity.remove705 reads its CURRENT retained Brain
    /// after parent Entity.remove and invokes this method on that object.
    pub fn clear_memories(&mut self) {
        for memory in self.memories.values_mut() {
            *memory = None;
        }
    }
    /// Brain470–471. Mob.removeFreeWill1552 invokes this AFTER selector removal.
    /// It does not stop running behaviors or clear unrelated Brain fields.
    pub fn remove_all_behaviors(&mut self) {
        self.available_behaviors_by_priority.clear();
    }
    /// Brain162 invokes its retained codec supplier's encodeStart. For this
    /// supported schema the result is success({memories:{}}); no logger side
    /// effect or source error/partial result exists on this path.
    pub fn serialize_start_nbt(&self) -> Tag {
        self.codec_supplier.get().encode_start(self)
    }
    /// Brain336/362–370. Default schema has no memories to erase; the active set
    /// still changes only when explicitly called, never during construction.
    pub fn use_default_activity(&mut self) {
        if self.active_activities.contains(&self.default_activity) {
            return;
        }
        self.active_activities.clear();
        self.active_activities
            .extend(self.core_activities.iter().cloned());
        self.active_activities
            .insert(Arc::clone(&self.default_activity));
    }
    pub fn is_active(&self, activity: &BrainActivity) -> bool {
        self.active_activities.contains(activity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_default_fixture_retains_distinct_brains_codec_and_shared_statics() {
        let provider = DefaultBrainProvider::new();
        let mut first = provider.make_from_constructor_memories();
        let second = provider.make_from_constructor_memories();
        let other_provider = DefaultBrainProvider::new();
        assert!(Arc::ptr_eq(&provider.codec(), &first.codec()));
        assert!(Arc::ptr_eq(&first.codec(), &second.codec()));
        assert!(!Arc::ptr_eq(&first.codec(), &other_provider.codec()));
        assert!(Arc::ptr_eq(&first.schedule(), &second.schedule()));
        assert!(Arc::ptr_eq(
            &first.default_activity(),
            &default_brain_bootstrap().idle_activity()
        ));
        assert_eq!(
            first.state_view(),
            DefaultBrainStateView {
                memories: 0,
                sensors: 0,
                behavior_priorities: 0,
                activity_requirements: 0,
                activity_memories_to_erase: 0,
                core_activities: 0,
                active_activities: 0,
                last_schedule_update: -9999,
            }
        );
        let idle = first.default_activity();
        assert!(!first.is_active(&idle));
        first.use_default_activity();
        assert!(first.is_active(&idle));
        assert!(!second.is_active(&idle));
        first.clear_memories();
        first.remove_all_behaviors();
        assert!(first.is_active(&idle));
        assert_eq!(
            first.serialize_start_nbt(),
            DefaultBrainProvider::constructor_memories_nbt()
        );
    }

    #[test]
    fn original_empty_schedule_has_real_idle_timeline_including_negative_time() {
        let bootstrap = default_brain_bootstrap();
        let schedule = bootstrap.empty_schedule();
        let idle = bootstrap.idle_activity();
        assert_eq!(idle.name(), "idle");
        assert_eq!(idle.java_hash_code(), 3_227_604);
        assert_eq!(
            schedule.idle_keyframes(),
            &[BrainKeyframe {
                time_stamp: 0,
                value: 1.0
            }]
        );
        for time in [i32::MIN, -24000, -1, 0, 1, 24000, i32::MAX] {
            assert!(Arc::ptr_eq(&schedule.activity_at(time), &idle));
        }
        let timeline = &schedule.timelines[&idle];
        assert_eq!(timeline.previous_index.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn unsupported_saved_inputs_refuse_instead_of_silently_emulating_codec_fallback() {
        let provider = DefaultBrainProvider::new();
        for (input, expected) in [
            (Tag::Int(3), DefaultBrainInputError::RootIsNotCompound),
            (
                Tag::Compound(vec![]),
                DefaultBrainInputError::MissingMemories,
            ),
            (
                Tag::Compound(vec![("memories".into(), Tag::Int(3))]),
                DefaultBrainInputError::MemoriesIsNotCompound,
            ),
            (
                Tag::Compound(vec![(
                    "memories".into(),
                    Tag::Compound(vec![(
                        "minecraft:last_slept".into(),
                        Tag::Compound(vec![("value".into(), Tag::Long(123))]),
                    )]),
                )]),
                DefaultBrainInputError::NonemptyMemoriesNeedRegistryCodecs,
            ),
        ] {
            assert_eq!(
                provider.decode_empty_memories_nbt(&input).unwrap_err(),
                expected
            );
        }
        let input = Tag::Compound(vec![
            ("unused".into(), Tag::Byte(1)),
            ("memories".into(), Tag::Compound(vec![])),
        ]);
        let brain = provider.decode_empty_memories_nbt(&input).unwrap();
        assert_eq!(
            brain.serialize_start_nbt(),
            DefaultBrainProvider::constructor_memories_nbt()
        );
    }
}
