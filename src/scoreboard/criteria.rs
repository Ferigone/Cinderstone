//! `ObjectiveCriteria` and `ObjectiveCriteria.RenderType`.

use super::format::ChatFormatting;
use crate::components::java::resource_location;
use crate::loot::facts::facts;
use serde::Deserialize;
use std::sync::OnceLock;

/// `ObjectiveCriteria.RenderType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderType {
    Integer,
    Hearts,
}

impl RenderType {
    pub fn id(self) -> &'static str {
        match self {
            Self::Integer => "integer",
            Self::Hearts => "hearts",
        }
    }

    /// `Enum.ordinal()`.
    pub fn ordinal(self) -> i32 {
        match self {
            Self::Integer => 0,
            Self::Hearts => 1,
        }
    }

    /// `RenderType.byId(name)`: anything unknown is `integer`.
    pub fn by_id(name: &str) -> Self {
        if name == "hearts" {
            Self::Hearts
        } else {
            Self::Integer
        }
    }
}

/// An `ObjectiveCriteria`: a name, whether scores are read-only and the default render type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectiveCriteria {
    name: String,
    read_only: bool,
    render_type: RenderType,
}

/// `ObjectiveCriteria.byName` can throw (`ResourceLocationException`) where it would return empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidCriteriaName(pub String);

#[derive(Deserialize)]
struct StatType {
    id: String,
    registry: String,
}

#[derive(Deserialize)]
struct StatFacts {
    stat_types: Vec<StatType>,
    custom_stats: Vec<String>,
}

fn stat_facts() -> &'static StatFacts {
    static FACTS: OnceLock<StatFacts> = OnceLock::new();
    FACTS.get_or_init(|| {
        serde_json::from_str(include_str!("../../fixtures/scoreboard-facts-1.21.json"))
            .expect("scoreboard facts")
    })
}

impl ObjectiveCriteria {
    fn custom(name: &str, read_only: bool, render_type: RenderType) -> Self {
        Self {
            name: name.to_owned(),
            read_only,
            render_type,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    pub fn default_render_type(&self) -> RenderType {
        self.render_type
    }

    pub fn dummy() -> Self {
        Self::custom("dummy", false, RenderType::Integer)
    }

    /// The built-in (non-statistic) criteria, in registration order.
    pub fn custom_criteria() -> Vec<ObjectiveCriteria> {
        let mut all = vec![
            Self::custom("dummy", false, RenderType::Integer),
            Self::custom("trigger", false, RenderType::Integer),
            Self::custom("deathCount", false, RenderType::Integer),
            Self::custom("playerKillCount", false, RenderType::Integer),
            Self::custom("totalKillCount", false, RenderType::Integer),
            Self::custom("health", true, RenderType::Hearts),
            Self::custom("food", true, RenderType::Integer),
            Self::custom("air", true, RenderType::Integer),
            Self::custom("armor", true, RenderType::Integer),
            Self::custom("xp", true, RenderType::Integer),
            Self::custom("level", true, RenderType::Integer),
        ];
        for prefix in ["teamkill", "killedByTeam"] {
            for color in &ChatFormatting::ALL[..16] {
                all.push(Self::custom(
                    &format!("{prefix}.{}", color.name()),
                    false,
                    RenderType::Integer,
                ));
            }
        }
        all
    }

    /// `ObjectiveCriteria.byName`: a built-in name, or `<stat type>:<value>` with `.` for `:`
    /// (`minecraft.mined:minecraft.stone`). `Ok(None)` is an empty `Optional`.
    pub fn by_name(name: &str) -> Result<Option<Self>, InvalidCriteriaName> {
        if let Some(found) = Self::custom_criteria()
            .into_iter()
            .find(|criteria| criteria.name == name)
        {
            return Ok(Some(found));
        }
        let Some((kind, value)) = name.split_once(':') else {
            return Ok(None);
        };
        // `ResourceLocation.bySeparator(.., '.')` throws on an invalid character.
        let by_dot = |text: &str| -> Result<String, InvalidCriteriaName> {
            let (namespace, path) = match text.split_once('.') {
                Some(("", path)) => ("minecraft", path),
                Some((namespace, path)) => (namespace, path),
                None => ("minecraft", text),
            };
            resource_location(&format!("{namespace}:{path}"))
                .ok_or_else(|| InvalidCriteriaName(name.to_owned()))
        };
        let kind = by_dot(kind)?;
        let Some(stat_type) = stat_facts().stat_types.iter().find(|stat| stat.id == kind) else {
            return Ok(None);
        };
        // The original resolves the type before it parses the value, so an unknown type never throws.
        let value = by_dot(value)?;
        let known = match stat_type.registry.as_str() {
            "minecraft:custom_stat" => stat_facts().custom_stats.iter().any(|id| *id == value),
            "minecraft:block" => facts().registries.block.iter().any(|id| *id == value),
            "minecraft:item" => facts().registries.item.iter().any(|id| *id == value),
            "minecraft:entity_type" => facts().registries.entity_type.iter().any(|id| *id == value),
            _ => false,
        };
        Ok(known.then(|| {
            let key = |id: &str| id.replace(':', ".");
            Self::custom(
                &format!("{}:{}", key(&kind), key(&value)),
                false,
                RenderType::Integer,
            )
        }))
    }
}
