//! Minecraft Java 1.21 hunger and survival arithmetic.
//!
//! FoodData follows the original tick branches and calls injected heal/starve
//! actions in their original order. Damage-source tags, equipment enchantment
//! evaluation, hurt cooldowns, world/entity side effects and death handling are
//! caller responsibilities; these primitives do not claim that entire pipeline.

use crate::nbt::Tag;
use anyhow::Result;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Difficulty {
    Peaceful,
    Easy,
    #[default]
    Normal,
    Hard,
}
impl Difficulty {
    pub fn from_id(id: i32) -> Self {
        match id.rem_euclid(4) {
            0 => Self::Peaceful,
            1 => Self::Easy,
            2 => Self::Normal,
            _ => Self::Hard,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SurvivalAbilities {
    pub invulnerable: bool,
    pub may_fly: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FoodData {
    pub food_level: i32,
    pub saturation: f32,
    pub exhaustion: f32,
    pub tick_timer: i32,
    pub last_food_level: i32,
}
impl Default for FoodData {
    fn default() -> Self {
        Self {
            food_level: 20,
            saturation: 5.0,
            exhaustion: 0.0,
            tick_timer: 0,
            last_food_level: 20,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct FoodTickContext {
    pub difficulty: Difficulty,
    pub natural_regeneration: bool,
    pub health: f32,
    pub max_health: f32,
}
impl FoodTickContext {
    pub fn is_hurt(self) -> bool {
        self.health > 0.0 && self.health < self.max_health
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FoodAction {
    Heal(f32),
    Starve(f32),
}
impl FoodData {
    /// Legacy eat overload: saturation modifier is multiplied by nutrition*2.
    pub fn eat(&mut self, nutrition: i32, saturation_modifier: f32) {
        self.eat_properties(nutrition, (nutrition as f32 * saturation_modifier) * 2.0);
    }
    /// FoodProperties/data-component overload already supplies absolute saturation.
    pub fn eat_properties(&mut self, nutrition: i32, saturation: f32) {
        self.food_level = self.food_level.wrapping_add(nutrition).clamp(0, 20);
        self.saturation = java_clamp(self.saturation + saturation, 0.0, self.food_level as f32);
    }
    pub fn needs_food(&self) -> bool {
        self.food_level < 20
    }
    /// Player.canEat: invulnerability allows eating even when the bar is full.
    pub fn can_eat(&self, always_eat: bool, abilities: SurvivalAbilities) -> bool {
        abilities.invulnerable || always_eat || self.needs_food()
    }
    /// FoodData.addExhaustion does not itself check abilities or logical side.
    pub fn add_exhaustion(&mut self, amount: f32) {
        self.exhaustion = java_min(self.exhaustion + amount, 40.0);
    }
    /// Player.causeFoodExhaustion is gated by invulnerability and server side.
    pub fn cause_exhaustion(
        &mut self,
        amount: f32,
        abilities: SurvivalAbilities,
        is_client_side: bool,
    ) {
        if !abilities.invulnerable && !is_client_side {
            self.add_exhaustion(amount);
        }
    }
    pub fn tick(&mut self, context: FoodTickContext) -> Option<FoodAction> {
        let mut result = None;
        self.tick_with(context, |a| result = Some(a));
        result
    }
    pub fn tick_with(&mut self, context: FoodTickContext, mut action: impl FnMut(FoodAction)) {
        self.last_food_level = self.food_level;
        if self.exhaustion > 4.0 {
            self.exhaustion -= 4.0;
            if self.saturation > 0.0 {
                self.saturation = java_max(self.saturation - 1.0, 0.0);
            } else if context.difficulty != Difficulty::Peaceful {
                self.food_level = self.food_level.wrapping_sub(1).max(0);
            }
        }
        if context.natural_regeneration
            && self.saturation > 0.0
            && context.is_hurt()
            && self.food_level >= 20
        {
            self.tick_timer = self.tick_timer.wrapping_add(1);
            if self.tick_timer >= 10 {
                let amount = java_min(self.saturation, 6.0);
                action(FoodAction::Heal(amount / 6.0));
                self.add_exhaustion(amount);
                self.tick_timer = 0;
            }
        } else if context.natural_regeneration && self.food_level >= 18 && context.is_hurt() {
            self.tick_timer = self.tick_timer.wrapping_add(1);
            if self.tick_timer >= 80 {
                action(FoodAction::Heal(1.0));
                self.add_exhaustion(6.0);
                self.tick_timer = 0;
            }
        } else if self.food_level <= 0 {
            self.tick_timer = self.tick_timer.wrapping_add(1);
            if self.tick_timer >= 80 {
                if context.health > 10.0
                    || context.difficulty == Difficulty::Hard
                    || (context.health > 1.0 && context.difficulty == Difficulty::Normal)
                {
                    action(FoodAction::Starve(1.0));
                }
                self.tick_timer = 0;
            }
        } else {
            self.tick_timer = 0;
        }
    }
    /// Player.aiStep's peaceful restoration runs before the ordinary FoodData tick.
    /// The returned heal attempt still obeys LivingEntity.heal's alive check.
    pub fn peaceful_restore(
        &mut self,
        context: FoodTickContext,
        tick_count: i32,
    ) -> Option<FoodAction> {
        if context.difficulty != Difficulty::Peaceful || !context.natural_regeneration {
            return None;
        }
        let heal = (context.health < context.max_health && tick_count % 20 == 0)
            .then_some(FoodAction::Heal(1.0));
        if self.saturation < 20.0 && tick_count % 20 == 0 {
            self.saturation += 1.0;
        }
        if self.needs_food() && tick_count % 10 == 0 {
            self.food_level = self.food_level.wrapping_add(1);
        }
        heal
    }
    /// Vanilla checks only that foodLevel has some numeric NBT type. Once it
    /// does, missing or incorrectly typed companion keys read as numeric zero.
    pub fn read_nbt(&mut self, tag: &Tag) {
        if !tag.get("foodLevel").is_some_and(is_numeric) {
            return;
        }
        self.food_level = tag.get("foodLevel").map_or(0, numeric_int);
        self.tick_timer = tag.get("foodTickTimer").map_or(0, numeric_int);
        self.saturation = tag.get("foodSaturationLevel").map_or(0.0, numeric_float);
        self.exhaustion = tag.get("foodExhaustionLevel").map_or(0.0, numeric_float);
    }
    pub fn write_nbt(&self, tag: &mut Tag) -> Result<()> {
        tag.insert("foodLevel", Tag::Int(self.food_level))?;
        tag.insert("foodTickTimer", Tag::Int(self.tick_timer))?;
        tag.insert(
            "foodSaturationLevel",
            Tag::Float(nbt_float(self.saturation)),
        )?;
        tag.insert(
            "foodExhaustionLevel",
            Tag::Float(nbt_float(self.exhaustion)),
        )?;
        Ok(())
    }
}

/// Weapon enchantments in 1.21 may modify this effectiveness on the server.
/// Resolve those effects in the real equipment/world context, then call the
/// modifier overload; the ordinary overload corresponds to no weapon modifier.
pub fn armor_effectiveness(damage: f32, armor: f32, toughness: f32) -> f32 {
    let denominator = 2.0 + toughness / 4.0;
    java_clamp(armor - damage / denominator, armor * 0.2, 20.0) / 25.0
}
pub fn damage_after_armor(damage: f32, armor: f32, toughness: f32) -> f32 {
    damage * (1.0 - armor_effectiveness(damage, armor, toughness))
}
pub fn damage_after_armor_with_modifier(
    damage: f32,
    armor: f32,
    toughness: f32,
    modify: impl FnOnce(f32) -> f32,
) -> f32 {
    let effectiveness = java_clamp(
        modify(armor_effectiveness(damage, armor, toughness)),
        0.0,
        1.0,
    );
    damage * (1.0 - effectiveness)
}
pub fn damage_after_magic(damage: f32, protection: f32) -> f32 {
    damage * (1.0 - java_clamp(protection, 0.0, 20.0) / 25.0)
}
/// Call only when the source permits the active resistance effect.
pub fn damage_after_resistance(damage: f32, amplifier: i32) -> f32 {
    let reduction = amplifier.wrapping_add(1).wrapping_mul(5);
    let remaining = 25i32.wrapping_sub(reduction);
    java_max((damage * remaining as f32) / 25.0, 0.0)
}
pub fn difficulty_scaled_damage(damage: f32, scales: bool, difficulty: Difficulty) -> f32 {
    if !scales {
        return damage;
    }
    match difficulty {
        Difficulty::Peaceful => 0.0,
        Difficulty::Easy => java_min(damage / 2.0 + 1.0, damage),
        Difficulty::Normal => damage,
        Difficulty::Hard => (damage * 3.0) / 2.0,
    }
}
/// LivingEntity.calculateFallDamage, before player may-fly and source/gamerule
/// immunity gates. Attributes retain their Java double/float conversion order.
pub fn calculate_fall_damage(
    distance: f32,
    block_multiplier: f32,
    safe_fall_distance: f64,
    fall_damage_multiplier: f64,
    fall_immune: bool,
) -> i32 {
    if fall_immune {
        return 0;
    }
    let effective = (distance - safe_fall_distance as f32) * block_multiplier;
    java_ceil_int(effective as f64 * fall_damage_multiplier)
}

/// Player.causeFallDamage suppresses every fall when may-fly is granted,
/// even when the player is not currently flying. Source immunity/gamerules
/// and equipment reductions are applied by the caller after this calculation.
pub fn calculate_player_fall_damage(
    distance: f32,
    block_multiplier: f32,
    safe_fall_distance: f64,
    fall_damage_multiplier: f64,
    fall_immune: bool,
    abilities: SurvivalAbilities,
) -> i32 {
    if abilities.may_fly {
        0
    } else {
        calculate_fall_damage(
            distance,
            block_multiplier,
            safe_fall_distance,
            fall_damage_multiplier,
            fall_immune,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HealthState {
    pub health: f32,
    pub max_health: f32,
    pub absorption: f32,
    pub max_absorption: f32,
}
impl Default for HealthState {
    fn default() -> Self {
        Self {
            health: 20.0,
            max_health: 20.0,
            absorption: 0.0,
            max_absorption: 0.0,
        }
    }
}
impl HealthState {
    pub fn set_health(&mut self, health: f32) {
        self.health = java_clamp(health, 0.0, self.max_health);
    }
    pub fn heal(&mut self, amount: f32) {
        if self.health > 0.0 {
            self.set_health(self.health + amount);
        }
    }
    pub fn set_absorption(&mut self, absorption: f32) {
        self.absorption = java_clamp(absorption, 0.0, self.max_absorption);
    }
    /// Player.actuallyHurt's numeric absorption/health subtraction. Damage tags,
    /// armor/magic, invulnerability, cooldowns, exhaustion and side effects must
    /// have been resolved by the caller. Return damage reaching health.
    pub fn subtract_damage(&mut self, damage: f32) -> f32 {
        let unabsorbed = java_max(damage - self.absorption, 0.0);
        self.set_absorption(self.absorption - (damage - unabsorbed));
        if unabsorbed != 0.0 {
            self.set_health(self.health - unabsorbed);
        }
        unabsorbed
    }
}

fn java_min(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() {
        f32::NAN
    } else if a == 0.0 && b == 0.0 {
        if a.is_sign_negative() || b.is_sign_negative() {
            -0.0
        } else {
            0.0
        }
    } else if a <= b {
        a
    } else {
        b
    }
}
fn java_max(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() {
        f32::NAN
    } else if a == 0.0 && b == 0.0 {
        if a.is_sign_positive() || b.is_sign_positive() {
            0.0
        } else {
            -0.0
        }
    } else if a >= b {
        a
    } else {
        b
    }
}
fn java_clamp(value: f32, min: f32, max: f32) -> f32 {
    if value < min {
        min
    } else {
        java_min(value, max)
    }
}
fn java_ceil_int(value: f64) -> i32 {
    let n = value as i32;
    if value > n as f64 {
        n.wrapping_add(1)
    } else {
        n
    }
}
fn java_floor_int(value: f64) -> i32 {
    let n = value as i32;
    if value < n as f64 {
        n.wrapping_sub(1)
    } else {
        n
    }
}
fn is_numeric(tag: &Tag) -> bool {
    matches!(
        tag,
        Tag::Byte(_) | Tag::Short(_) | Tag::Int(_) | Tag::Long(_) | Tag::Float(_) | Tag::Double(_)
    )
}
fn numeric_int(tag: &Tag) -> i32 {
    match tag {
        Tag::Byte(v) => *v as i32,
        Tag::Short(v) => *v as i32,
        Tag::Int(v) => *v,
        Tag::Long(v) => *v as i32,
        Tag::Float(v) => java_floor_int(*v as f64),
        Tag::Double(v) => java_floor_int(*v),
        _ => 0,
    }
}
fn nbt_float(value: f32) -> f32 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}
fn numeric_float(tag: &Tag) -> f32 {
    match tag {
        Tag::Byte(v) => *v as f32,
        Tag::Short(v) => *v as f32,
        Tag::Int(v) => *v as f32,
        Tag::Long(v) => *v as f32,
        Tag::Float(v) => nbt_float(*v),
        Tag::Double(v) => {
            if *v == 0.0 {
                0.0
            } else {
                *v as f32
            }
        }
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    fn f(v: &Value) -> f32 {
        f32::from_bits(u32::from_str_radix(v.as_str().unwrap(), 16).unwrap())
    }
    fn d(v: &Value) -> f64 {
        f64::from_bits(u64::from_str_radix(v.as_str().unwrap(), 16).unwrap())
    }
    fn bits(v: f32) -> String {
        format!("{:08x}", if v.is_nan() { 0x7fc00000 } else { v.to_bits() })
    }
    fn food_output(food: FoodData, action: Option<FoodAction>) -> Value {
        let (heals, heal, hurts, hurt) = match action {
            Some(FoodAction::Heal(v)) => (1, v, 0, 0.0),
            Some(FoodAction::Starve(v)) => (0, 0.0, 1, v),
            None => (0, 0.0, 0, 0.0),
        };
        json!([
            food.food_level,
            bits(food.saturation),
            bits(food.exhaustion),
            food.tick_timer,
            food.last_food_level,
            heals,
            bits(heal),
            hurts,
            bits(hurt)
        ])
    }
    fn run(q: &Value) -> Value {
        match q["op"].as_str().unwrap() {
            "food_nbt" => {
                let a = &q["initial"];
                let mut food = FoodData {
                    food_level: a[0].as_i64().unwrap() as i32,
                    saturation: f(&a[1]),
                    exhaustion: f(&a[2]),
                    tick_timer: a[3].as_i64().unwrap() as i32,
                    last_food_level: a[4].as_i64().unwrap() as i32,
                };
                let mut tag = Tag::Compound(vec![]);
                for entry in q["entries"].as_array().unwrap() {
                    let value = match entry[1].as_str().unwrap() {
                        "byte" => Tag::Byte(entry[2].as_i64().unwrap() as i8),
                        "short" => Tag::Short(entry[2].as_i64().unwrap() as i16),
                        "int" => Tag::Int(entry[2].as_i64().unwrap() as i32),
                        "long" => Tag::Long(entry[2].as_i64().unwrap()),
                        "float" => Tag::Float(f(&entry[2])),
                        "double" => Tag::Double(d(&entry[2])),
                        "string" => Tag::String(entry[2].as_str().unwrap().into()),
                        _ => panic!(),
                    };
                    tag.insert(entry[0].as_str().unwrap(), value).unwrap();
                }
                food.read_nbt(&tag);
                let mut saved = Tag::Compound(vec![]);
                food.write_nbt(&mut saved).unwrap();
                json!({"state":food_output(food,None),"saved":[numeric_int(saved.get("foodLevel").unwrap()),numeric_int(saved.get("foodTickTimer").unwrap()),bits(numeric_float(saved.get("foodSaturationLevel").unwrap())),bits(numeric_float(saved.get("foodExhaustionLevel").unwrap()))]})
            }
            "armor" => json!(bits(damage_after_armor(
                f(&q["damage"]),
                f(&q["armor"]),
                f(&q["toughness"])
            ))),
            "magic" => json!(bits(damage_after_magic(
                f(&q["damage"]),
                f(&q["protection"])
            ))),
            "resistance" => json!(bits(damage_after_resistance(
                f(&q["damage"]),
                q["amplifier"].as_i64().unwrap() as i32
            ))),
            "fall_flight_gate" => json!(calculate_player_fall_damage(
                f(&q["distance"]),
                1.0,
                3.0,
                1.0,
                false,
                SurvivalAbilities {
                    invulnerable: false,
                    may_fly: true
                }
            )),
            "fall" => json!(calculate_fall_damage(
                f(&q["distance"]),
                f(&q["block_multiplier"]),
                d(&q["safe"]),
                d(&q["attribute_multiplier"]),
                q["immune"].as_bool().unwrap()
            )),
            "subtract" => {
                let mut state = HealthState {
                    health: f(&q["health"]),
                    max_health: f(&q["max"]),
                    absorption: f(&q["absorption"]),
                    max_absorption: f(&q["max_absorption"]),
                };
                state.subtract_damage(f(&q["damage"]));
                json!([bits(state.health), bits(state.absorption)])
            }
            "health" => {
                let mut state = HealthState {
                    max_health: f(&q["max"]),
                    max_absorption: f(&q["max_absorption"]),
                    ..HealthState::default()
                };
                state.set_health(f(&q["health"]));
                state.set_absorption(f(&q["absorption"]));
                let mut out = Vec::new();
                for a in q["steps"].as_array().unwrap() {
                    match a[0].as_str().unwrap() {
                        "heal" => state.heal(f(&a[1])),
                        "set" => state.set_health(f(&a[1])),
                        "absorb" => state.set_absorption(f(&a[1])),
                        _ => panic!(),
                    };
                    out.push(json!([
                        bits(state.health),
                        bits(state.absorption),
                        state.health > 0.0 && state.health < state.max_health
                    ]));
                }
                json!(out)
            }
            "food" => {
                let a = &q["initial"];
                let mut food = FoodData {
                    food_level: a[0].as_i64().unwrap() as i32,
                    saturation: f(&a[1]),
                    exhaustion: f(&a[2]),
                    tick_timer: a[3].as_i64().unwrap() as i32,
                    last_food_level: a[4].as_i64().unwrap() as i32,
                };
                let mut out = Vec::new();
                for a in q["steps"].as_array().unwrap() {
                    let mut result = None;
                    match a[0].as_str().unwrap() {
                        "eat" => food.eat(a[1].as_i64().unwrap() as i32, f(&a[2])),
                        "props" => food.eat_properties(a[1].as_i64().unwrap() as i32, f(&a[2])),
                        "exhaust" => food.add_exhaustion(f(&a[1])),
                        "cause" => food.cause_exhaustion(
                            f(&a[1]),
                            SurvivalAbilities {
                                invulnerable: a[2].as_bool().unwrap(),
                                may_fly: false,
                            },
                            a[3].as_bool().unwrap(),
                        ),
                        "caneat" => {
                            let mut state = food_output(food, None);
                            state.as_array_mut().unwrap().push(json!(food.can_eat(
                                a[1].as_bool().unwrap(),
                                SurvivalAbilities {
                                    invulnerable: a[2].as_bool().unwrap(),
                                    may_fly: false
                                }
                            )));
                            out.push(state);
                            continue;
                        }
                        "tick" | "peace" => {
                            let ctx = FoodTickContext {
                                difficulty: Difficulty::from_id(a[1].as_i64().unwrap() as i32),
                                natural_regeneration: a[2].as_bool().unwrap(),
                                health: f(&a[3]),
                                max_health: f(&a[4]),
                            };
                            result = if a[0] == "tick" {
                                food.tick(ctx)
                            } else {
                                food.peaceful_restore(ctx, a[5].as_i64().unwrap() as i32)
                            };
                        }
                        _ => panic!(),
                    }
                    out.push(food_output(food, result));
                }
                json!(out)
            }
            _ => panic!("unknown survival oracle operation"),
        }
    }
    #[test]
    fn actual_java_survival_vectors() {
        let vectors: Value =
            serde_json::from_str(include_str!("../fixtures/survival-vectors.json")).unwrap();
        for (index, case) in vectors["cases"].as_array().unwrap().iter().enumerate() {
            assert_eq!(
                run(&case["query"]),
                case["expected"],
                "survival case {index}: {}",
                case["query"]
            );
        }
    }
    #[test]
    fn exhaustion_strict_threshold_and_single_decrement() {
        let ctx = FoodTickContext {
            difficulty: Difficulty::Normal,
            natural_regeneration: false,
            health: 20.0,
            max_health: 20.0,
        };
        let mut data = FoodData {
            exhaustion: 4.0,
            ..FoodData::default()
        };
        data.tick(ctx);
        assert_eq!(data.saturation, 5.0);
        data.exhaustion = 40.0;
        data.tick(ctx);
        assert_eq!((data.saturation, data.exhaustion), (4.0, 36.0));
    }
    #[test]
    fn nbt_preserves_unknown_and_unclamped_state() {
        let mut tag = Tag::Compound(vec![]);
        tag.insert("custom", Tag::String("keep".into())).unwrap();
        let data = FoodData {
            food_level: -5,
            saturation: 25.0,
            exhaustion: 45.0,
            tick_timer: 79,
            last_food_level: 99,
        };
        data.write_nbt(&mut tag).unwrap();
        let mut loaded = FoodData::default();
        loaded.read_nbt(&tag);
        assert_eq!(loaded.food_level, -5);
        assert_eq!(loaded.saturation, 25.0);
        assert_eq!(loaded.exhaustion, 45.0);
        assert_eq!(loaded.tick_timer, 79);
        assert_eq!(loaded.last_food_level, 20);
        assert_eq!(tag.get("custom"), Some(&Tag::String("keep".into())));
        let mut partial = Tag::Compound(vec![]);
        partial.insert("foodLevel", Tag::Float(-0.25)).unwrap();
        loaded.read_nbt(&partial);
        assert_eq!(loaded.food_level, -1);
        assert_eq!(
            (loaded.saturation, loaded.exhaustion, loaded.tick_timer),
            (0.0, 0.0, 0)
        );
    }
}
