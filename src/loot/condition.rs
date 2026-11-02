//! `LootItemCondition` and its 19 registered types.
//!
//! Compound conditions short-circuit exactly like `Util.allOf` / `Util.anyOf`,
//! so a random condition inside a later term draws only when Java would reach it.

use super::context::{EntityTarget, LootContext, Param, Visited};
use super::enchant;
use super::number::{IntRange, LevelBasedValue, NumberProvider};
use super::predicate::{DamageSourcePredicate, EntityPredicate, ItemPredicate, LocationPredicate, StatePropertiesPredicate};
use super::{LootError, Result};

/// `LootItemCondition`.
#[derive(Debug, Clone, PartialEq)]
pub enum Condition {
    Inverted(Box<Condition>),
    AnyOf(Vec<Condition>),
    AllOf(Vec<Condition>),
    RandomChance(NumberProvider),
    RandomChanceWithEnchantedBonus {
        unenchanted_chance: f32,
        enchanted_chance: LevelBasedValue,
        enchantment: String,
    },
    EntityProperties {
        predicate: Option<EntityPredicate>,
        entity: EntityTarget,
    },
    KilledByPlayer,
    EntityScores {
        entity: EntityTarget,
        scores: Vec<(String, IntRange)>,
    },
    BlockStateProperty {
        block: String,
        properties: Option<StatePropertiesPredicate>,
    },
    MatchTool(Option<ItemPredicate>),
    TableBonus {
        enchantment: String,
        values: Vec<f32>,
    },
    SurvivesExplosion,
    DamageSourceProperties(Option<DamageSourcePredicate>),
    LocationCheck {
        predicate: Option<LocationPredicate>,
        offset: [i32; 3],
    },
    WeatherCheck {
        raining: Option<bool>,
        thundering: Option<bool>,
    },
    Reference(String),
    TimeCheck {
        period: Option<i64>,
        value: IntRange,
    },
    ValueCheck {
        provider: NumberProvider,
        range: IntRange,
    },
    EnchantmentActiveCheck(bool),
}

/// Evaluate a list like `Util.allOf(list)`; the empty list is true.
pub fn all_of(conditions: &[Condition], ctx: &mut LootContext<'_>) -> Result<bool> {
    for condition in conditions {
        if !condition.test(ctx)? {
            return Ok(false);
        }
    }
    Ok(true)
}

impl Condition {
    /// `LootItemCondition.test(LootContext)`.
    pub fn test(&self, ctx: &mut LootContext<'_>) -> Result<bool> {
        match self {
            Self::Inverted(term) => Ok(!term.test(ctx)?),
            Self::AnyOf(terms) => {
                for term in terms {
                    if term.test(ctx)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            Self::AllOf(terms) => all_of(terms, ctx),
            Self::RandomChance(chance) => {
                let chance = chance.get_float(ctx)?;
                Ok(ctx.random().next_f32() < chance)
            }
            Self::RandomChanceWithEnchantedBonus {
                unenchanted_chance,
                enchanted_chance,
                enchantment,
            } => {
                let level = match ctx.entity(EntityTarget::Attacker) {
                    Some(entity) if entity.is_living() => {
                        enchant::entity_enchantment_level(entity, enchantment)?
                    }
                    _ => 0,
                };
                let chance = if level > 0 {
                    enchanted_chance.calculate(level)?
                } else {
                    *unenchanted_chance
                };
                Ok(ctx.random().next_f32() < chance)
            }
            Self::EntityProperties { predicate, entity } => {
                let Some(predicate) = predicate else {
                    return Ok(true);
                };
                let target = ctx.entity(*entity);
                let origin = ctx.params.origin;
                predicate.matches(ctx, origin, target)
            }
            Self::KilledByPlayer => Ok(ctx.has_param(Param::LastDamagePlayer)),
            Self::EntityScores { entity, scores } => {
                let Some(target) = ctx.entity(*entity) else {
                    return Ok(false);
                };
                let holder = target.scoreboard_name()?;
                for (objective, range) in scores {
                    match ctx.level().score(&holder, objective)? {
                        None => return Ok(false),
                        Some(value) => {
                            if !range.test(ctx, value)? {
                                return Ok(false);
                            }
                        }
                    }
                }
                Ok(true)
            }
            Self::BlockStateProperty { block, properties } => {
                let Some(state) = ctx.params.block_state else {
                    return Ok(false);
                };
                if super::predicate::block_state_block_id(state)? != *block {
                    return Ok(false);
                }
                match properties {
                    Some(properties) => properties.matches(state),
                    None => Ok(true),
                }
            }
            Self::MatchTool(predicate) => match &ctx.params.tool {
                None => Ok(false),
                Some(tool) => match predicate {
                    None => Ok(true),
                    Some(predicate) => predicate.test(tool),
                },
            },
            Self::TableBonus { enchantment, values } => {
                let level = match &ctx.params.tool {
                    Some(tool) => enchant::item_enchantment_level(tool, enchantment)?,
                    None => 0,
                };
                let index = level.min(values.len() as i32 - 1);
                let chance = usize::try_from(index)
                    .ok()
                    .and_then(|index| values.get(index).copied())
                    .ok_or_else(|| LootError::Invalid(format!("Index {index} out of bounds")))?;
                Ok(ctx.random().next_f32() < chance)
            }
            Self::SurvivesExplosion => match ctx.params.explosion_radius {
                Some(radius) => {
                    let chance = 1.0f32 / radius;
                    Ok(ctx.random().next_f32() <= chance)
                }
                None => Ok(true),
            },
            Self::DamageSourceProperties(predicate) => {
                let (Some(origin), Some(source)) = (ctx.params.origin, ctx.params.damage_source)
                else {
                    return Ok(false);
                };
                match predicate {
                    None => Ok(true),
                    Some(predicate) => predicate.matches(ctx, origin, source),
                }
            }
            Self::LocationCheck { predicate, offset } => {
                let Some(origin) = ctx.params.origin else {
                    return Ok(false);
                };
                match predicate {
                    None => Ok(true),
                    Some(predicate) => predicate.matches(
                        ctx,
                        origin[0] + f64::from(offset[0]),
                        origin[1] + f64::from(offset[1]),
                        origin[2] + f64::from(offset[2]),
                    ),
                }
            }
            Self::WeatherCheck { raining, thundering } => {
                if let Some(expected) = raining {
                    if *expected != ctx.level().is_raining()? {
                        return Ok(false);
                    }
                }
                match thundering {
                    Some(expected) => Ok(*expected == ctx.level().is_thundering()?),
                    None => Ok(true),
                }
            }
            Self::Reference(key) => {
                let resolver = ctx.resolver();
                let Some(condition) = resolver.predicate(key) else {
                    return Ok(false);
                };
                let visited = Visited::Predicate(key.clone());
                if !ctx.push_visited(visited.clone()) {
                    return Ok(false);
                }
                let result = condition.test(ctx);
                ctx.pop_visited(&visited);
                result
            }
            Self::TimeCheck { period, value } => {
                let mut time = ctx.level().day_time()?;
                if let Some(period) = period {
                    if *period == 0 {
                        return Err(LootError::Invalid("/ by zero".to_owned()));
                    }
                    time = time.wrapping_rem(*period);
                }
                value.test(ctx, time as i32)
            }
            Self::ValueCheck { provider, range } => {
                let value = provider.get_int(ctx)?;
                range.test(ctx, value)
            }
            Self::EnchantmentActiveCheck(active) => {
                let value = ctx
                    .params
                    .enchantment_active
                    .ok_or(LootError::MissingParam(Param::EnchantmentActive.name()))?;
                Ok(value == *active)
            }
        }
    }
}
