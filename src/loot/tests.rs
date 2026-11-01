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
use super::host::{EquipmentSlot, LootDamageSource, LootEntity, LootLevel};
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
}

impl LootLevel for TestLevel {
    fn game_time(&self) -> Result<i64> {
        Ok(0)
    }
    fn day_time(&self) -> Result<i64> {
        Ok(0)
    }
    fn is_raining(&self) -> Result<bool> {
        Ok(false)
    }
    fn is_thundering(&self) -> Result<bool> {
        Ok(false)
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
    let key = format!("minecraft:{}", case["t"].as_str().unwrap());
    let table = vanilla().get(&key).expect("table exists");
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
    };
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
    params.validate(table.param_set)?;

    let mut random = LegacyRandomSource::new(case["s"].as_i64().unwrap());
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

/// Component-aware equality of two `stacks|next` strings (the wire order of the
/// enchantment map is not part of the semantics).
fn same_result(actual: &str, expected: &str) -> bool {
    if actual == expected {
        return true;
    }
    let (Some((a_stacks, a_rng)), Some((e_stacks, e_rng))) =
        (actual.split_once('|'), expected.split_once('|'))
    else {
        return false;
    };
    if a_rng != e_rng {
        return false;
    }
    let split = |text: &str| -> Vec<String> {
        if text.is_empty() {
            Vec::new()
        } else {
            text.split(',').map(str::to_owned).collect()
        }
    };
    let (a, e) = (split(a_stacks), split(e_stacks));
    if a.len() != e.len() {
        return false;
    }
    a.iter().zip(&e).all(|(a, e)| {
        let decode = |text: &String| {
            let bytes: Vec<u8> = (0..text.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
                .collect();
            ItemStack::decode_exact(&bytes).unwrap()
        };
        let (a, e) = (decode(a), decode(e));
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
                        case["t"], case["s"]
                    ));
                }
            }
            Err(LootError::Unsupported(what)) => {
                *unsupported.entry(what).or_default() += 1;
            }
            Err(error) => mismatches.push(format!("{} seed {}: {error}", case["t"], case["s"])),
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
