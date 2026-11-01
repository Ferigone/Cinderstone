//! Differential checks against the unchanged original loot classes.
//!
//! `fixtures/loot-oracle-vectors.json` is produced by `tools/loot_generate.py`:
//! every vanilla table is rolled by the original server code under many
//! parameter/tool/seed combinations; each case records the produced stacks (as
//! Slot wire bytes) and the next `nextInt()` of the roll's random source. The
//! port must reproduce both, which proves not only the items but the exact RNG
//! consumption order.

use super::context::{LootContext, LootParams};
use super::enchant::{enchant, new_stack, set_component};
use super::host::{EquipmentSlot, LootBlockEntity, LootDamageSource, LootEntity, LootLevel};
use super::sequences::RandomSequences;
use super::vanilla::vanilla;
use super::{LootError, Result};
use crate::inventory::{self, ItemStack};
use crate::random::LegacyRandomSource;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::OnceLock;

fn default_states() -> &'static BTreeMap<String, u32> {
    static STATES: OnceLock<BTreeMap<String, u32>> = OnceLock::new();
    STATES.get_or_init(|| {
        let mut map = BTreeMap::new();
        for state in 0..40_000u32 {
            if let Some(name) = inventory::block_state_name(state) {
                let key = format!("minecraft:{name}");
                map.entry(key)
                    .or_insert_with(|| inventory::block_default_state(state).unwrap());
            }
        }
        map
    })
}

fn block_state_from_json(value: &Value) -> u32 {
    let name = value["name"].as_str().unwrap();
    let default = *default_states().get(name).unwrap_or_else(|| panic!("block {name}"));
    let mut overrides = BTreeMap::new();
    if let Some(properties) = value["properties"].as_object() {
        for (key, value) in properties {
            overrides.insert(key.clone(), value.as_str().unwrap().to_owned());
        }
    }
    inventory::block_state_with_properties(default, &overrides).unwrap()
}

fn stack_from_json(value: &Value) -> ItemStack {
    let id = value["id"].as_str().unwrap();
    if id == "minecraft:air" {
        return ItemStack::empty();
    }
    let mut stack = new_stack(id).unwrap();
    stack.count = value["count"].as_i64().unwrap() as i32;
    if let Some(levels) = value["components"]["minecraft:enchantments"]["levels"].as_object() {
        for (name, level) in levels {
            enchant(&mut stack, name, level.as_i64().unwrap() as i32).unwrap();
        }
    }
    if let Some(damage) = value["components"]["minecraft:damage"].as_i64() {
        let mut writer = crate::protocol::Writer::new();
        writer.varint(damage as i32);
        set_component(&mut stack, 3, writer.into_inner());
    }
    stack
}

struct TestLevel {
    biome: String,
    game_time: i64,
    day_time: i64,
    raining: bool,
    thundering: bool,
}

impl LootLevel for TestLevel {
    fn game_time(&self) -> Result<i64> {
        Ok(self.game_time)
    }
    fn day_time(&self) -> Result<i64> {
        Ok(self.day_time)
    }
    fn is_raining(&self) -> Result<bool> {
        Ok(self.raining)
    }
    fn is_thundering(&self) -> Result<bool> {
        Ok(self.thundering)
    }
    fn dimension(&self) -> Result<String> {
        Ok("minecraft:overworld".to_owned())
    }
    fn is_loaded(&self, _pos: [i32; 3]) -> Result<bool> {
        Ok(true)
    }
    fn block_state(&self, _pos: [i32; 3]) -> Result<u32> {
        Ok(0)
    }
    fn biome(&self, _pos: [i32; 3]) -> Result<String> {
        Ok(self.biome.clone())
    }
    fn can_see_sky(&self, _pos: [i32; 3]) -> Result<bool> {
        Ok(false)
    }
}

struct TestEntity {
    entity_type: String,
    on_fire: bool,
    mainhand: ItemStack,
    size: Option<i32>,
    variant: Option<String>,
    open_water: Option<bool>,
}

impl TestEntity {
    fn from_json(value: &Value) -> Self {
        Self {
            entity_type: value["type"].as_str().unwrap().to_owned(),
            on_fire: value["on_fire"].as_bool().unwrap_or(false),
            mainhand: value
                .get("mainhand")
                .map(stack_from_json)
                .unwrap_or_else(ItemStack::empty),
            // A Slime or MagmaCube without an explicit size has the default size 1.
            size: value["size"].as_i64().map(|size| size as i32).or_else(|| {
                matches!(
                    value["type"].as_str(),
                    Some("minecraft:slime" | "minecraft:magma_cube")
                )
                .then_some(1)
            }),
            variant: value["variant"].as_str().map(str::to_owned),
            open_water: value["open_water"].as_bool(),
        }
    }
}

impl LootEntity for TestEntity {
    fn type_id(&self) -> Result<String> {
        Ok(self.entity_type.clone())
    }
    fn is_on_fire(&self) -> Result<bool> {
        Ok(self.on_fire)
    }
    fn is_living(&self) -> bool {
        !matches!(
            self.entity_type.as_str(),
            "minecraft:fishing_bobber" | "minecraft:arrow"
        )
    }
    fn equipment(&self, slot: EquipmentSlot) -> Result<ItemStack> {
        Ok(if slot == EquipmentSlot::MainHand {
            self.mainhand.clone()
        } else {
            ItemStack::empty()
        })
    }
    fn slime_size(&self) -> Result<Option<i32>> {
        Ok(self.size)
    }
    fn fishing_hook_open_water(&self) -> Result<Option<bool>> {
        Ok(if self.entity_type == "minecraft:fishing_bobber" {
            self.open_water
        } else {
            None
        })
    }
    fn variant(&self, kind: &str) -> Result<Option<String>> {
        Ok(if self.entity_type == format!("minecraft:{kind}") {
            self.variant.clone()
        } else {
            None
        })
    }
}

/// A block entity whose implicit components are a custom name and a lock key,
/// both plain-string NBT components.
struct TestBlockEntity {
    custom_name: Option<String>,
    lock: Option<String>,
}

impl LootBlockEntity for TestBlockEntity {
    fn collect_components(&self) -> Result<Vec<crate::inventory::Component>> {
        let string_nbt = |text: &str| {
            crate::nbt::Tag::String(crate::nbt::JavaString::Utf8(text.to_owned()))
                .encode_network_anonymous()
                .map_err(|error| LootError::Host(error.to_string()))
        };
        let mut components = Vec::new();
        if let Some(name) = &self.custom_name {
            components.push(crate::inventory::Component { type_id: 5, data: string_nbt(name)? });
        }
        if let Some(key) = &self.lock {
            components.push(crate::inventory::Component { type_id: 55, data: string_nbt(key)? });
        }
        Ok(components)
    }
}

struct TestDamage {
    type_id: String,
    direct: Option<TestEntity>,
    source: Option<TestEntity>,
}

impl LootDamageSource for TestDamage {
    fn type_id(&self) -> Result<String> {
        Ok(self.type_id.clone())
    }
    fn direct_entity(&self) -> Option<&dyn LootEntity> {
        self.direct.as_ref().map(|e| e as &dyn LootEntity)
    }
    fn source_entity(&self) -> Option<&dyn LootEntity> {
        self.source.as_ref().map(|e| e as &dyn LootEntity)
    }
    fn is_direct(&self) -> Result<bool> {
        Ok(self.direct.is_none() && self.source.is_none())
    }
}

/// Run one recorded case; the result uses the fixture's `stacks|next` form.
fn run_case(case: &Value, header: &Value) -> std::result::Result<String, LootError> {
    let custom;
    let (key, table) = match case.get("ct").and_then(Value::as_u64) {
        Some(index) => {
            custom = super::parse::table(&header["custom_tables"][index as usize])?;
            ("minecraft:custom".to_owned(), &custom)
        }
        None => {
            let key = format!("minecraft:{}", case["t"].as_str().unwrap());
            let table = vanilla().get(&key).expect("table exists");
            (key, table)
        }
    };
    let pick = |name: &str, field: &str| -> Option<&Value> {
        case.get(field).and_then(Value::as_u64).map(|index| &header[name][index as usize])
    };
    let this_entity = pick("entities", "te").map(TestEntity::from_json);
    let attacker = pick("entities", "ae").map(TestEntity::from_json);
    let player = case.get("lp").map(|_| TestEntity::from_json(&serde_json::json!({"type": "minecraft:player"})));
    let damage = pick("damage_sources", "ds").map(|value| TestDamage {
        type_id: value["type"].as_str().unwrap().to_owned(),
        direct: value.get("direct_entity").map(TestEntity::from_json),
        source: value.get("source_entity").map(TestEntity::from_json),
    });
    let level_json = case
        .get("lv")
        .and_then(Value::as_u64)
        .map_or(&Value::Null, |index| &header["levels"][index as usize]);
    let level = TestLevel {
        biome: level_json["biome"].as_str().unwrap_or("minecraft:plains").to_owned(),
        game_time: level_json["game_time"].as_i64().unwrap_or(0),
        day_time: level_json["day_time"].as_i64().unwrap_or(0),
        raining: level_json["raining"].as_bool().unwrap_or(false),
        thundering: level_json["thundering"].as_bool().unwrap_or(false),
    };
    let block_entity = case.get("be").map(|value| TestBlockEntity {
        custom_name: value["custom_name"].as_str().map(str::to_owned),
        lock: value["lock"].as_str().map(str::to_owned),
    });
    let mut params = LootParams::default();
    if key != "minecraft:gameplay/piglin_bartering" {
        params.origin = Some([0.5, 64.5, 0.5]);
    }
    params.luck = case.get("l").and_then(Value::as_f64).unwrap_or(0.0) as f32;
    params.tool = pick("tools", "k").map(stack_from_json);
    params.block_state = pick("block_states", "b").map(block_state_from_json);
    params.explosion_radius = case.get("x").and_then(Value::as_f64).map(|x| x as f32);
    params.this_entity = this_entity.as_ref().map(|e| e as &dyn LootEntity);
    params.attacking_entity = attacker.as_ref().map(|e| e as &dyn LootEntity);
    params.last_damage_player = player.as_ref().map(|e| e as &dyn LootEntity);
    params.damage_source = damage.as_ref().map(|d| d as &dyn LootDamageSource);
    params.block_entity = block_entity.as_ref().map(|b| b as &dyn LootBlockEntity);
    params.validate(table.param_set)?;

    if let Some(sequence) = case.get("sq") {
        // Named random sequences: three consecutive rolls share one persistent stream.
        let mut sequences = RandomSequences::new(sequence["world_seed"].as_i64().unwrap());
        if let Some(salt) = sequence["salt"].as_i64() {
            sequences.set_seed_defaults(
                salt as i32,
                sequence["include_world_seed"].as_bool().unwrap(),
                sequence["include_sequence_id"].as_bool().unwrap(),
            );
        }
        let id = sequence["id"].as_str().unwrap();
        let mut rolls = Vec::new();
        for _ in 0..sequence["rolls"].as_u64().unwrap() {
            let mut roll_params = LootParams::default();
            roll_params.origin = params.origin;
            roll_params.tool = params.tool.clone();
            roll_params.block_state = params.block_state;
            roll_params.explosion_radius = params.explosion_radius;
            roll_params.this_entity = params.this_entity;
            roll_params.attacking_entity = params.attacking_entity;
            roll_params.last_damage_player = params.last_damage_player;
            roll_params.damage_source = params.damage_source;
            roll_params.luck = params.luck;
            let mut random = sequences.get(id);
            let mut ctx = LootContext::new(roll_params, &mut random, &level, vanilla());
            let stacks = table.get_random_items(&mut ctx)?;
            rolls.push(stacks.iter().map(|stack| hex(&stack.encode())).collect::<Vec<_>>().join(","));
        }
        let (lo, hi) = sequences.state(id).unwrap();
        return Ok(format!("{}|{},{}", rolls.join(";"), lo as i64, hi as i64));
    }
    let mut random = LegacyRandomSource::new(case["s"].as_i64().unwrap());
    if let Some(size) = case.get("f").and_then(Value::as_u64) {
        let mut container = vec![ItemStack::empty(); size as usize];
        if let Some(existing) = case.get("fe").and_then(Value::as_object) {
            for (slot, value) in existing {
                container[slot.parse::<usize>().unwrap()] = stack_from_json(value);
            }
        }
        {
            let mut ctx = LootContext::new(params, &mut random, &level, vanilla());
            table.fill(&mut ctx, &mut container)?;
        }
        let slots: Vec<String> = container.iter().map(|stack| hex(&stack.encode())).collect();
        return Ok(slots.join(","));
    }
    let stacks = {
        let mut ctx = LootContext::new(params, &mut random, &level, vanilla());
        table.get_random_items(&mut ctx)?
    };
    let mut parts: Vec<String> = stacks
        .iter()
        .map(|stack| hex(&stack.encode()))
        .collect();
    let joined = std::mem::take(&mut parts).join(",");
    Ok(format!("{joined}|{}", crate::random::RandomSource::next_i32(&mut random)))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn decode_slot(text: &str) -> ItemStack {
    let bytes: Vec<u8> = (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect();
    ItemStack::decode_exact(&bytes).unwrap()
}

/// Component-aware equality of two `stacks|next` strings (or bare slot lists for
/// container fills). The wire order of the enchantment map is the JVM's identity
/// hash order and is not part of the semantics, so stacks compare as values.
fn same_result(actual: &str, expected: &str) -> bool {
    if actual == expected {
        return true;
    }
    let (a_stacks, a_rng) = actual.split_once('|').unwrap_or((actual, ""));
    let (e_stacks, e_rng) = expected.split_once('|').unwrap_or((expected, ""));
    if a_rng != e_rng {
        return false;
    }
    let split = |text: &str| -> Vec<String> {
        if text.is_empty() {
            Vec::new()
        } else {
            text.split(|c| c == ',' || c == ';').map(str::to_owned).collect()
        }
    };
    if a_stacks.matches(';').count() != e_stacks.matches(';').count() {
        return false;
    }
    let (a, e) = (split(a_stacks), split(e_stacks));
    a.len() == e.len()
        && a.iter().zip(&e).all(|(a, e)| {
            let (a, e) = (decode_slot(a), decode_slot(e));
            a.count == e.count && a.same_item_same_components(&e).unwrap_or(false)
        })
}

#[test]
fn vanilla_tables_parse() {
    let tables = vanilla();
    assert_eq!(tables.len(), 1178);
}

#[test]
fn oracle_vectors_match_the_original_server() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../fixtures/loot-oracle-vectors.json")).unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    let mut mismatches = Vec::new();
    let mut unsupported: BTreeMap<String, usize> = BTreeMap::new();
    let mut checked = 0usize;
    for case in cases {
        match run_case(case, &fixture) {
            Ok(actual) => {
                checked += 1;
                let expected = case["r"].as_str().unwrap();
                if !same_result(&actual, expected) {
                    mismatches.push(format!(
                        "{} seed {}: expected {expected} got {actual}",
                        case.get("t").unwrap_or(&case["ct"]), case["s"]
                    ));
                }
            }
            Err(LootError::Unsupported(what)) => {
                *unsupported.entry(what).or_default() += 1;
            }
            Err(error) => mismatches.push(format!(
                "{} seed {}: {error}",
                case.get("t").unwrap_or(&case["ct"]),
                case["s"]
            )),
        }
    }
    eprintln!("checked {checked} cases; unsupported: {unsupported:?}");
    assert!(
        mismatches.is_empty(),
        "{} mismatches, first: {:#?}",
        mismatches.len(),
        &mismatches[..mismatches.len().min(12)]
    );
}

#[test]
fn java_number_semantics() {
    use super::mth::{clamp_f32, clamp_i32, floor, round};
    // Math.round(float) is floor(x + 0.5): halves round toward positive infinity.
    assert_eq!(round(2.5), 3);
    assert_eq!(round(-2.5), -2);
    assert_eq!(round(-0.5), 0);
    assert_eq!(round(0.499_999_97), 0);
    assert_eq!(round(f32::NAN), 0);
    assert_eq!(round(f32::MAX), i32::MAX);
    assert_eq!(round(f32::MIN), i32::MIN);
    // Mth.floor(float) is the integer floor, saturating like Java's (int) cast.
    assert_eq!(floor(-0.5), -1);
    assert_eq!(floor(-1.0), -1);
    assert_eq!(floor(f32::from_bits(3.0f32.to_bits() - 1)), 2);
    assert_eq!(floor(f32::NAN), 0);
    // Mth.clamp compares min first, so NaN passes through.
    assert_eq!(clamp_i32(5, 1, 3), 3);
    assert_eq!(clamp_i32(-5, 1, 3), 1);
    assert!(clamp_f32(f32::NAN, 0.0, 1.0).is_nan());
}

#[test]
fn parameter_sets_follow_the_original_builder_checks() {
    use super::context::{param_set, Param};
    let barter = param_set("minecraft:barter").unwrap();
    let mut params = LootParams::default();
    params.origin = Some([0.0; 3]);
    let message = params.validate(barter).unwrap_err().to_string();
    assert!(message.contains("not allowed"), "{message}");
    assert!(barter.allows(Param::ThisEntity) && !barter.allows(Param::Origin));
    let block = param_set("block").unwrap();
    let message = LootParams::default().validate(block).unwrap_err().to_string();
    assert!(message.contains("Missing required"), "{message}");
    assert!(param_set("minecraft:no_such_set").is_none());
}

#[test]
fn unknown_types_are_rejected_and_unported_ones_fail_only_when_run() {
    use super::parse;
    let reject = |json: &str| parse::table(&serde_json::from_str::<Value>(json).unwrap()).unwrap_err();
    assert!(matches!(
        reject(r#"{"pools":[{"rolls":1,"entries":[{"type":"minecraft:nope"}]}]}"#),
        LootError::Codec(_)
    ));
    assert!(matches!(
        reject(r#"{"functions":[{"function":"minecraft:nope"}]}"#),
        LootError::Codec(_)
    ));
    assert!(matches!(
        reject(r#"{"pools":[{"rolls":1,"entries":[],"conditions":[{"condition":"minecraft:nope"}]}]}"#),
        LootError::Codec(_)
    ));
    assert!(matches!(reject(r#"{"pools":[{"entries":[]}]}"#), LootError::Codec(_)));

    // A registered but unported function parses, then fails explicitly.
    let table = parse::table(
        &serde_json::from_str::<Value>(
            r#"{"type":"minecraft:chest","pools":[{"rolls":1,"entries":[
                {"type":"minecraft:item","name":"minecraft:stick",
                 "functions":[{"function":"minecraft:set_lore","lore":[]}]}]}]}"#,
        )
        .unwrap(),
    )
    .unwrap();
    let level = TestLevel {
        biome: "minecraft:plains".to_owned(),
        game_time: 0,
        day_time: 0,
        raining: false,
        thundering: false,
    };
    let mut params = LootParams::default();
    params.origin = Some([0.0; 3]);
    let mut random = LegacyRandomSource::new(1);
    let mut ctx = LootContext::new(params, &mut random, &level, vanilla());
    assert!(matches!(
        table.get_random_items(&mut ctx),
        Err(LootError::Unsupported(what)) if what.contains("set_lore")
    ));
}

#[test]
fn unsupported_predicate_clauses_error_only_when_reached() {
    use super::parse;
    // `distance` is not ported; a type mismatch ahead of it short-circuits to false
    // exactly like EntityPredicate.matches, and a matching type reaches the error.
    let condition = parse::condition(
        &serde_json::from_str::<Value>(
            r#"{"condition":"minecraft:entity_properties","entity":"this",
                "predicate":{"type":"minecraft:cow","distance":{"absolute":{"min":1}}}}"#,
        )
        .unwrap(),
    )
    .unwrap();
    let level = TestLevel {
        biome: String::new(),
        game_time: 0,
        day_time: 0,
        raining: false,
        thundering: false,
    };
    for (entity_type, expected_unsupported) in [("minecraft:pig", false), ("minecraft:cow", true)] {
        let entity = TestEntity::from_json(&serde_json::json!({ "type": entity_type }));
        let mut params = LootParams::default();
        params.origin = Some([0.0; 3]);
        params.this_entity = Some(&entity);
        let mut random = LegacyRandomSource::new(1);
        let mut ctx = LootContext::new(params, &mut random, &level, vanilla());
        let result = condition.test(&mut ctx);
        if expected_unsupported {
            assert!(matches!(result, Err(LootError::Unsupported(_))), "{result:?}");
        } else {
            assert_eq!(result, Ok(false));
        }
    }
}

#[test]
fn random_sequences_persist_and_round_trip_through_nbt() {
    use crate::random::RandomSource as _;
    let mut sequences = RandomSequences::new(123_456_789);
    let first = sequences.get("minecraft:blocks/stone").next_i32();
    let second = sequences.get("minecraft:blocks/stone").next_i32();
    assert_ne!(first, second, "the stream advances between rolls");
    assert!(sequences.is_dirty());
    // Another id is an independent stream.
    let other = sequences.get("minecraft:blocks/dirt").next_i32();
    let saved = sequences.save();
    let mut restored = RandomSequences::load(123_456_789, &saved).unwrap();
    assert_eq!(restored.state("minecraft:blocks/stone"), sequences.state("minecraft:blocks/stone"));
    assert_eq!(
        restored.get("minecraft:blocks/stone").next_i32(),
        sequences.get("minecraft:blocks/stone").next_i32()
    );
    let mut fresh = RandomSequences::new(123_456_789);
    assert_eq!(fresh.get("minecraft:blocks/dirt").next_i32(), other);
    fresh.clear_dirty();
    assert!(!fresh.is_dirty());
}

#[test]
fn explicit_seed_beats_sequence_beats_level_random() {
    use super::sequences::RollRandom;
    use crate::random::RandomSource as _;
    let mut sequences = RandomSequences::new(7);
    let mut level_random = LegacyRandomSource::new(99);
    let expected_seeded = LegacyRandomSource::new(5).next_i32();
    let mut seeded = RollRandom::select(5, Some("minecraft:x"), &mut sequences, &mut level_random);
    assert_eq!(seeded.next_i32(), expected_seeded);
    drop(seeded);
    assert!(sequences.state("minecraft:x").is_none(), "a seed never touches the sequence");

    let mut by_sequence = RollRandom::select(0, Some("minecraft:x"), &mut sequences, &mut level_random);
    by_sequence.next_i32();
    drop(by_sequence);
    assert!(sequences.state("minecraft:x").is_some());

    let expected_level = LegacyRandomSource::new(99).next_i32();
    let mut by_level = RollRandom::select(0, None, &mut sequences, &mut level_random);
    assert_eq!(by_level.next_i32(), expected_level);
}
