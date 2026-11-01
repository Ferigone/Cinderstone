//! Explicit dimension, game-rule and weather facts for the runtime item host.
//! Captured registry fields are decoded rather than inferred from a world name.
use crate::{
    level::LevelData,
    nbt::Tag,
    protocol::Reader,
    random::LegacyRandomSource,
    runtime_collision::CollisionBorder,
    runtime_item_world::{ItemWorldFacts, RainAvailability},
    tick::{WeatherChange, WeatherState},
    world::{MAX_Y, MIN_Y},
    world_rules::WorldRules,
};
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DimensionFacts {
    pub min_y: i32,
    pub height: i32,
    pub logical_height: i32,
    pub ultrawarm: bool,
    pub has_skylight: bool,
    pub has_ceiling: bool,
}
impl DimensionFacts {
    /// The serialized DimensionType codec writes these numeric/boolean types.
    pub fn from_tag(tag: &Tag) -> Result<Self> {
        let int = |name| match tag.get(name) {
            Some(Tag::Int(value)) => Ok(*value),
            _ => bail!("dimension has no serialized int {name}"),
        };
        let flag = |name| match tag.get(name) {
            Some(Tag::Byte(value)) => Ok(*value != 0),
            _ => bail!("dimension has no serialized boolean {name}"),
        };
        let facts = Self {
            min_y: int("min_y")?,
            height: int("height")?,
            logical_height: int("logical_height")?,
            ultrawarm: flag("ultrawarm")?,
            has_skylight: flag("has_skylight")?,
            has_ceiling: flag("has_ceiling")?,
        };
        ensure!(
            facts.height > 0
                && facts.height % 16 == 0
                && facts.min_y % 16 == 0
                && facts.logical_height >= 0
                && facts.logical_height <= facts.height,
            "unsupported dimension height layout"
        );
        facts
            .min_y
            .checked_add(facts.height)
            .context("dimension height overflow")?;
        Ok(facts)
    }
    pub fn require_runtime_layout(self) -> Result<()> {
        ensure!(
            self.min_y == MIN_Y && self.min_y.checked_add(self.height) == Some(MAX_Y + 1),
            "dimension layout is not supported by the current world backend"
        );
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct DimensionEntry {
    pub key: String,
    /// None is the actual absent registry data, never fabricated default facts.
    pub facts: Option<DimensionFacts>,
}
/// Payload of protocol767 RegistryData, without its packet ID.
pub fn decode_dimension_registry(payload: &[u8]) -> Result<Vec<DimensionEntry>> {
    let mut reader = Reader::new(payload);
    ensure!(
        reader.string(32767)? == "minecraft:dimension_type",
        "not a dimension-type registry"
    );
    let count = reader.varint()?;
    ensure!(
        (0..=65536).contains(&count),
        "invalid dimension entry count"
    );
    let mut entries = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let key = reader.string(32767)?;
        let facts = if reader.bool()? {
            let (tag, used) = Tag::decode_anonymous_prefix(reader.remaining())?;
            reader.bytes(used)?;
            Some(DimensionFacts::from_tag(&tag)?)
        } else {
            None
        };
        entries.push(DimensionEntry { key, facts });
    }
    reader.finish()?;
    Ok(entries)
}

#[derive(Clone, Debug)]
pub struct WorldContext {
    pub dimension: DimensionFacts,
    pub rules: WorldRules,
    weather: WeatherState,
}
impl WorldContext {
    /// Caller supplies the real new-world or persisted weather state.
    pub fn new(dimension: DimensionFacts, rules: WorldRules, weather: WeatherState) -> Self {
        Self {
            dimension,
            rules,
            weather,
        }
    }
    pub fn from_level(dimension: DimensionFacts, level: &LevelData) -> Result<Self> {
        Ok(Self::new(dimension, level.rules(), level.weather()?))
    }
    pub fn weather(&self) -> WeatherState {
        self.weather
    }
    pub fn advance_weather(&mut self, random: &mut LegacyRandomSource) -> Vec<WeatherChange> {
        self.weather.advance_with_legacy_random(
            self.dimension.has_skylight,
            self.rules.do_weather_cycle(),
            random,
        )
    }
    pub fn item_facts(
        &self,
        game_time: i64,
        border: CollisionBorder,
        has_passengers: bool,
    ) -> Result<ItemWorldFacts> {
        self.dimension.require_runtime_layout()?;
        Ok(ItemWorldFacts {
            game_time,
            minimum_build_height: self.dimension.min_y,
            ultrawarm: self.dimension.ultrawarm,
            border,
            rain: if self.weather.is_raining() {
                RainAvailability::ExposureUnavailable
            } else {
                RainAvailability::NotRaining
            },
            has_passengers,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use serde_json::Value;
    fn dimensions() -> Vec<DimensionEntry> {
        let original: Value =
            serde_json::from_str(include_str!("../fixtures/dimension-context-vectors.json"))
                .unwrap();
        let payload = STANDARD
            .decode(original["payload"].as_str().unwrap())
            .unwrap();
        let entries = decode_dimension_registry(&payload).unwrap();
        assert_eq!(
            entries.len(),
            original["dimensions"].as_array().unwrap().len()
        );
        for (entry, expected) in entries
            .iter()
            .zip(original["dimensions"].as_array().unwrap())
        {
            assert_eq!(entry.key, expected["key"]);
            assert_eq!(
                serde_json::to_value(entry.facts.unwrap()).unwrap(),
                expected["facts"]
            );
        }
        entries
    }
    #[test]
    fn captured_dimension_registry_matches_independent_client_and_rejects_other_world_layouts() {
        let entries = dimensions();
        for entry in entries {
            let facts = entry.facts.unwrap();
            assert_eq!(
                facts.require_runtime_layout().is_ok(),
                entry.key == "minecraft:overworld" || entry.key == "minecraft:overworld_caves"
            );
        }
    }
    #[test]
    fn actual_weather_and_rule_context_controls_rng_and_never_guesses_rain_exposure() {
        let dimension = dimensions()[0].facts.unwrap();
        let rules = WorldRules::read(Some(&Tag::Compound(vec![(
            "doWeatherCycle".into(),
            Tag::String("FALSE".into()),
        )])));
        let mut context = WorldContext::new(dimension, rules, WeatherState::default());
        let mut random = LegacyRandomSource::new(506);
        let before = random.state48();
        assert!(context.advance_weather(&mut random).is_empty());
        assert_eq!(random.state48(), before);
        assert!(matches!(
            context
                .item_facts(3, CollisionBorder::default(), false)
                .unwrap()
                .rain,
            RainAvailability::NotRaining
        ));
        let mut weather = WeatherState {
            raining: true,
            ..WeatherState::default()
        };
        weather.prepare_weather();
        let context = WorldContext::new(dimension, WorldRules::default(), weather);
        assert!(matches!(
            context
                .item_facts(4, CollisionBorder::default(), false)
                .unwrap()
                .rain,
            RainAvailability::ExposureUnavailable
        ));
    }
    #[test]
    fn embedded_nbt_prefix_leaves_following_packet_fields_outside_its_budget() {
        let bytes = [vec![1, 37], vec![0xFF; 128]].concat();
        let (tag, used) = Tag::decode_anonymous_prefix_with_limits(
            &bytes,
            crate::nbt::Limits {
                max_depth: 1,
                max_bytes: 2,
                max_elements: 2,
            },
        )
        .unwrap();
        assert_eq!(tag, Tag::Byte(37));
        assert_eq!(used, 2);
        assert!(Tag::decode_anonymous(&bytes).is_err());
    }
}
