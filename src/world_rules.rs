//! Exact Minecraft Java1.21 GameRules NBT loading and canonical serialization.
//!
//! Missing/non-StringTag values preserve current values. Present invalid strings
//! deserialize to false/zero; integer command bounds are not NBT bounds. The
//! exact1.21 catalog is static and has no FeatureFlagSet/datapack-feature filter.
use crate::nbt::{JavaString, Tag};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleValue {
    Boolean(bool),
    Integer(i32),
}
impl RuleValue {
    pub fn serialize(self) -> String {
        match self {
            Self::Boolean(v) => v.to_string(),
            Self::Integer(v) => v.to_string(),
        }
    }
}

// Extracted from actual unchanged dcs() / GameRules.createTag, pinned in
// tools/world_rules_facts.json and independently compared by the oracle test.
const CATALOG: &[(&str, RuleValue)] = &[
    ("announceAdvancements", RuleValue::Boolean(true)),
    ("blockExplosionDropDecay", RuleValue::Boolean(true)),
    ("commandBlockOutput", RuleValue::Boolean(true)),
    ("commandModificationBlockLimit", RuleValue::Integer(32768)),
    ("disableElytraMovementCheck", RuleValue::Boolean(false)),
    ("disableRaids", RuleValue::Boolean(false)),
    ("doDaylightCycle", RuleValue::Boolean(true)),
    ("doEntityDrops", RuleValue::Boolean(true)),
    ("doFireTick", RuleValue::Boolean(true)),
    ("doImmediateRespawn", RuleValue::Boolean(false)),
    ("doInsomnia", RuleValue::Boolean(true)),
    ("doLimitedCrafting", RuleValue::Boolean(false)),
    ("doMobLoot", RuleValue::Boolean(true)),
    ("doMobSpawning", RuleValue::Boolean(true)),
    ("doPatrolSpawning", RuleValue::Boolean(true)),
    ("doTileDrops", RuleValue::Boolean(true)),
    ("doTraderSpawning", RuleValue::Boolean(true)),
    ("doVinesSpread", RuleValue::Boolean(true)),
    ("doWardenSpawning", RuleValue::Boolean(true)),
    ("doWeatherCycle", RuleValue::Boolean(true)),
    ("drowningDamage", RuleValue::Boolean(true)),
    ("enderPearlsVanishOnDeath", RuleValue::Boolean(true)),
    ("fallDamage", RuleValue::Boolean(true)),
    ("fireDamage", RuleValue::Boolean(true)),
    ("forgiveDeadPlayers", RuleValue::Boolean(true)),
    ("freezeDamage", RuleValue::Boolean(true)),
    ("globalSoundEvents", RuleValue::Boolean(true)),
    ("keepInventory", RuleValue::Boolean(false)),
    ("lavaSourceConversion", RuleValue::Boolean(false)),
    ("logAdminCommands", RuleValue::Boolean(true)),
    ("maxCommandChainLength", RuleValue::Integer(65536)),
    ("maxCommandForkCount", RuleValue::Integer(65536)),
    ("maxEntityCramming", RuleValue::Integer(24)),
    ("mobExplosionDropDecay", RuleValue::Boolean(true)),
    ("mobGriefing", RuleValue::Boolean(true)),
    ("naturalRegeneration", RuleValue::Boolean(true)),
    ("playersNetherPortalCreativeDelay", RuleValue::Integer(1)),
    ("playersNetherPortalDefaultDelay", RuleValue::Integer(80)),
    ("playersSleepingPercentage", RuleValue::Integer(100)),
    ("projectilesCanBreakBlocks", RuleValue::Boolean(true)),
    ("randomTickSpeed", RuleValue::Integer(3)),
    ("reducedDebugInfo", RuleValue::Boolean(false)),
    ("sendCommandFeedback", RuleValue::Boolean(true)),
    ("showDeathMessages", RuleValue::Boolean(true)),
    ("snowAccumulationHeight", RuleValue::Integer(1)),
    ("spawnChunkRadius", RuleValue::Integer(2)),
    ("spawnRadius", RuleValue::Integer(10)),
    ("spectatorsGenerateChunks", RuleValue::Boolean(true)),
    ("tntExplosionDropDecay", RuleValue::Boolean(false)),
    ("universalAnger", RuleValue::Boolean(false)),
    ("waterSourceConversion", RuleValue::Boolean(true)),
];
// Character.digit(char,10) on the pinned JDK21. All65,536 UTF-16 units are
// checked against this range representation by the original oracle bootstrap.
const DIGIT_ZEROS: &[u16] = &[
    0x30, 0x660, 0x6f0, 0x7c0, 0x966, 0x9e6, 0xa66, 0xae6, 0xb66, 0xbe6, 0xc66, 0xce6, 0xd66,
    0xde6, 0xe50, 0xed0, 0xf20, 0x1040, 0x1090, 0x17e0, 0x1810, 0x1946, 0x19d0, 0x1a80, 0x1a90,
    0x1b50, 0x1bb0, 0x1c40, 0x1c50, 0xa620, 0xa8d0, 0xa900, 0xa9d0, 0xa9f0, 0xaa50, 0xabf0, 0xff10,
];

/// IntegerValue.safeParse, including Java's BMP Character.digit semantics.
/// StringTag retains raw UTF-16; isolated surrogates are invalid characters,
/// not a missing/wrong-type rule value. Empty/invalid/out-of-range returns zero.
pub fn parse_integer(value: &JavaString) -> i32 {
    let mut units = value.iter_units().peekable();
    let negative = match units.peek() {
        Some(0x2d) => {
            units.next();
            true
        }
        Some(0x2b) => {
            units.next();
            false
        }
        _ => false,
    };
    if units.peek().is_none() {
        return 0;
    }
    let limit = if negative { i32::MIN } else { -i32::MAX };
    let mult_min = limit / 10;
    let mut result = 0i32;
    for unit in units {
        let index = DIGIT_ZEROS.partition_point(|&zero| zero <= unit);
        if index == 0 {
            return 0;
        }
        let digit = unit - DIGIT_ZEROS[index - 1];
        if digit >= 10 {
            return 0;
        }
        let digit = i32::from(digit);
        if result < mult_min {
            return 0;
        }
        result *= 10;
        if result < limit + digit {
            return 0;
        }
        result -= digit;
    }
    if negative {
        result
    } else {
        -result
    }
}

/// Boolean.parseBoolean is an exact case-insensitive comparison with "true";
/// whitespace and all other text are false. The oracle exhaustively checks
/// that its four ASCII letters have only ASCII uppercase/lowercase aliases.
pub fn parse_boolean(value: &JavaString) -> bool {
    let mut units = value.iter_units();
    for expected in [0x74, 0x72, 0x75, 0x65] {
        let Some(unit) = units.next() else {
            return false;
        };
        let unit = if (0x41..=0x5a).contains(&unit) {
            unit + 0x20
        } else {
            unit
        };
        if unit != expected {
            return false;
        }
    }
    units.next().is_none()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldRules {
    values: BTreeMap<&'static str, RuleValue>,
}
impl Default for WorldRules {
    fn default() -> Self {
        Self {
            values: CATALOG.iter().copied().collect(),
        }
    }
}
impl WorldRules {
    /// The supplied tag is the GameRules compound itself. None or any other
    /// tag type produces the original catalog defaults, without error.
    pub fn read(tag: Option<&Tag>) -> Self {
        let mut result = Self::default();
        if let Some(tag) = tag {
            result.load_from_tag(tag);
        }
        result
    }
    /// Original private GameRules.loadFromTag. Wrong/missing tag types preserve
    /// the current value; present invalid StringTags replace it with false/0.
    pub fn load_from_tag(&mut self, tag: &Tag) {
        for (name, value) in &mut self.values {
            if let Some(Tag::String(text)) = tag.get(name) {
                *value = match value {
                    RuleValue::Boolean(_) => RuleValue::Boolean(parse_boolean(text)),
                    RuleValue::Integer(_) => RuleValue::Integer(parse_integer(text)),
                };
            }
        }
    }
    pub fn get(&self, name: &str) -> Option<RuleValue> {
        self.values.get(name).copied()
    }
    pub fn boolean(&self, name: &str) -> Option<bool> {
        match self.get(name) {
            Some(RuleValue::Boolean(value)) => Some(value),
            _ => None,
        }
    }
    pub fn integer(&self, name: &str) -> Option<i32> {
        match self.get(name) {
            Some(RuleValue::Integer(value)) => Some(value),
            _ => None,
        }
    }
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, RuleValue)> + '_ {
        self.values.iter().map(|(&name, &value)| (name, value))
    }
    /// Original createTag's logical contents: all51 recognized keys as canonical
    /// strings, with unknown keys omitted. Compound byte/hash-map order is not
    /// promised; callers preserving unrelated level.dat data can merge keys.
    pub fn create_tag(&self) -> Tag {
        Tag::Compound(
            self.values
                .iter()
                .map(|(&name, &value)| (name.into(), Tag::String(value.serialize().into())))
                .collect(),
        )
    }
    pub fn spawn_chunk_radius(&self) -> i32 {
        self.integer("spawnChunkRadius").expect("catalog integer")
    }
    pub fn spectators_generate_chunks(&self) -> bool {
        self.boolean("spectatorsGenerateChunks")
            .expect("catalog boolean")
    }
    pub fn do_weather_cycle(&self) -> bool {
        self.boolean("doWeatherCycle").expect("catalog boolean")
    }
    pub fn random_tick_speed(&self) -> i32 {
        self.integer("randomTickSpeed").expect("catalog integer")
    }
    pub fn do_mob_spawning(&self) -> bool {
        self.boolean("doMobSpawning").expect("catalog boolean")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD, Engine};
    use serde_json::{json, Value};
    fn decode(value: &Value) -> Option<Tag> {
        if value.is_null() {
            None
        } else {
            Some(Tag::decode_anonymous(&STANDARD.decode(value.as_str().unwrap()).unwrap()).unwrap())
        }
    }
    fn snapshot(rules: &WorldRules) -> Value {
        let values = rules
            .iter()
            .map(|(name, value)| {
                (
                    name.to_owned(),
                    match value {
                        RuleValue::Boolean(v) => json!(v),
                        RuleValue::Integer(v) => json!(v),
                    },
                )
            })
            .collect::<serde_json::Map<_, _>>();
        let written = rules.create_tag();
        let written = rules
            .iter()
            .map(|(name, _)| {
                let Some(Tag::String(value)) = written.get(name) else {
                    panic!("canonical string missing")
                };
                (name.to_owned(), json!(value.as_str().unwrap()))
            })
            .collect::<serde_json::Map<_, _>>();
        json!({"values":values,"written":written})
    }
    #[test]
    fn actual_original_gamerules_nbt_load_and_canonical_values() {
        let fixture: Value =
            serde_json::from_str(include_str!("../fixtures/world_rules_vectors.json")).unwrap();
        let mut count = 0;
        assert_eq!(
            snapshot(&WorldRules::default()),
            fixture["facts"]["defaults"]
        );
        for case in fixture["cases"].as_array().unwrap() {
            let q = &case["query"];
            let tag = decode(&q["initial"]);
            let mut rules = WorldRules::read(tag.as_ref());
            for (index, expected) in case["expected"].as_array().unwrap().iter().enumerate() {
                if index > 0 {
                    rules.load_from_tag(&decode(&q["loads"][index - 1]).unwrap());
                }
                let expected = &fixture["pool"][expected.as_u64().unwrap() as usize];
                assert_eq!(snapshot(&rules), *expected, "{} state{index}", case["name"]);
                assert_eq!(
                    rules.spawn_chunk_radius(),
                    expected["values"]["spawnChunkRadius"].as_i64().unwrap() as i32
                );
                assert_eq!(
                    rules.spectators_generate_chunks(),
                    expected["values"]["spectatorsGenerateChunks"]
                        .as_bool()
                        .unwrap()
                );
                assert_eq!(
                    rules.do_weather_cycle(),
                    expected["values"]["doWeatherCycle"].as_bool().unwrap()
                );
                assert_eq!(
                    rules.random_tick_speed(),
                    expected["values"]["randomTickSpeed"].as_i64().unwrap() as i32
                );
                assert_eq!(
                    rules.do_mob_spawning(),
                    expected["values"]["doMobSpawning"].as_bool().unwrap()
                );
                count += 1;
            }
        }
        println!("original GameRules states compared: {count}");
        assert!(count >= 900);
    }
    #[test]
    fn every_utf16_unit_matches_original_jdk_digit_and_boolean_alias_facts() {
        let facts: Value =
            serde_json::from_str(include_str!("../tools/world_rules_facts.json")).unwrap();
        let zeros: Vec<u16> = facts["digit_zeros"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u16)
            .collect();
        assert_eq!(DIGIT_ZEROS, zeros);
        for unit in 0..=u16::MAX {
            let digit = zeros
                .iter()
                .find_map(|&zero| unit.checked_sub(zero).filter(|&d| d < 10));
            let expected = digit.map_or(0, |digit| 10 + i32::from(digit));
            assert_eq!(
                parse_integer(&JavaString::Utf16(vec![0x31, unit])),
                expected,
                "unit{unit:04x}"
            );
            for (index, key) in ['t', 'r', 'u', 'e'].into_iter().enumerate() {
                let expected = facts["boolean_char_aliases"][key.to_string()]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|v| v.as_u64().unwrap() == u64::from(unit));
                let mut text = vec![0x74, 0x72, 0x75, 0x65];
                text[index] = unit;
                assert_eq!(
                    parse_boolean(&JavaString::Utf16(text)),
                    expected,
                    "boolean letter{key} unit{unit:04x}"
                );
            }
        }
    }
}
