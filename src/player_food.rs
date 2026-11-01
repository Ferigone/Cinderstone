//! Owned Player hunger and the food/health fragments of Player.tick/aiStep.
//!
//! This is deliberately a phase API: peaceful restoration belongs before the
//! inventory/LivingEntity aiStep, FoodData.tick and time statistics belong later
//! in Player.tick, and health delivery belongs in ServerPlayer.doTick. Calling
//! these fragments does not execute the rest of any of those methods.
//! The canonical hunger arithmetic/NBT representation is survival::FoodData.
use crate::{
    nbt::Tag,
    player_data,
    player_stats::{StatKey, CUSTOM},
    protocol::{Packet, Writer},
    survival::{Difficulty, FoodData, SurvivalAbilities},
};
use anyhow::Result;

#[derive(Clone, Copy, Debug)]
pub struct PlayerFoodFrame {
    pub difficulty: Difficulty,
    pub natural_regeneration: bool,
    /// Actual Player.tickCount, including its wrapping i32 behavior.
    pub tick_count: i32,
    pub is_client_side: bool,
    pub abilities: SurvivalAbilities,
}

/// Calls have no default implementation. Health remains in the entity owner's
/// existing synchronized health state; this module never makes a second copy.
/// Starvation must enter the real Player.hurt pipeline, including invulnerability,
/// cooldown, death, entity RNG and damage/stat effects at their actual sites.
pub trait PlayerFoodHost {
    fn health(&mut self) -> Result<f32>;
    fn max_health(&mut self) -> Result<f32>;
    /// LivingEntity.heal: positive-current-health gate and setHealth clamp.
    fn heal(&mut self, amount: f32) -> Result<()>;
    fn hurt_starvation(&mut self, amount: f32) -> Result<bool>;
    fn is_alive(&mut self) -> Result<bool>;
    fn is_discrete(&mut self) -> Result<bool>;
    fn is_sleeping(&mut self) -> Result<bool>;
    fn award_stat(&mut self, key: StatKey, amount: i32) -> Result<()>;
    fn send_health(&mut self, update: PlayerHealthUpdate) -> Result<()>;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerHealthUpdate {
    pub health: f32,
    pub food: i32,
    pub saturation: f32,
}
impl PlayerHealthUpdate {
    /// Actual ClientboundSetHealthPacket layout and 1.21 protocol ID.
    pub fn packet(self) -> Packet {
        let mut out = Writer::new();
        out.f32(self.health).varint(self.food).f32(self.saturation);
        Packet {
            id: 0x5d,
            payload: out.into_inner(),
        }
    }
}

/// ServerPlayer's delivery cache is connection/entity lifetime state, not NBT.
#[derive(Clone, Copy, Debug)]
pub struct PlayerHealthSync {
    pub last_sent_health: f32,
    pub last_sent_food: i32,
    pub last_food_saturation_zero: bool,
}
impl Default for PlayerHealthSync {
    fn default() -> Self {
        Self {
            last_sent_health: -1.0e8,
            last_sent_food: -99_999_999,
            last_food_saturation_zero: true,
        }
    }
}
impl PlayerHealthSync {
    /// ServerPlayer.resetSentInfo forces health delivery by changing this field.
    pub fn reset_sent_health(&mut self) {
        self.last_sent_health = -1.0e8;
    }
    /// Only saturation's zero/nonzero transition is part of the send gate.
    /// An ordinary nonzero saturation change alone sends no health packet.
    pub fn send_changes(
        &mut self,
        food: &FoodData,
        host: &mut impl PlayerFoodHost,
    ) -> Result<bool> {
        if host.health()? == self.last_sent_health
            && self.last_sent_food == food.food_level
            && (food.saturation == 0.) == self.last_food_saturation_zero
        {
            return Ok(false);
        }
        let update = PlayerHealthUpdate {
            health: host.health()?,
            food: food.food_level,
            saturation: food.saturation,
        };
        host.send_health(update)?;
        // Original fields are updated after the synchronous connection send;
        // health is read again rather than copied from the sent packet.
        self.last_sent_health = host.health()?;
        self.last_sent_food = food.food_level;
        self.last_food_saturation_zero = food.saturation == 0.;
        Ok(true)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PlayerFoodState {
    pub food: FoodData,
}
impl PlayerFoodState {
    /// New FoodData followed by loading the four saved hunger fields. The
    /// original readAdditionalSaveData leaves lastFoodLevel at constructor20.
    pub fn from_saved(saved: &player_data::FoodData) -> Self {
        Self {
            food: FoodData {
                food_level: saved.level,
                saturation: saved.saturation,
                exhaustion: saved.exhaustion,
                tick_timer: saved.tick_timer,
                last_food_level: 20,
            },
        }
    }
    pub fn saved(&self) -> player_data::FoodData {
        player_data::FoodData {
            level: self.food.food_level,
            saturation: self.food.saturation,
            exhaustion: self.food.exhaustion,
            tick_timer: self.food.tick_timer,
        }
    }
    pub fn read_nbt(&mut self, tag: &Tag) {
        self.food.read_nbt(tag);
    }
    pub fn write_nbt(&self, tag: &mut Tag) -> Result<()> {
        self.food.write_nbt(tag)
    }
    /// Legacy FoodData.eat(nutrition,saturationModifier).
    pub fn eat(&mut self, nutrition: i32, saturation_modifier: f32) {
        self.food.eat(nutrition, saturation_modifier);
    }
    /// FoodProperties already contains absolute saturation, not its modifier.
    /// This is the hunger mutation of eating; item-use stats, consume criteria,
    /// effects, stack shrink and remainder handling belong to the item executor.
    pub fn eat_properties(&mut self, nutrition: i32, saturation: f32) {
        self.food.eat_properties(nutrition, saturation);
    }
    pub fn can_eat(&self, always_eat: bool, abilities: SurvivalAbilities) -> bool {
        self.food.can_eat(always_eat, abilities)
    }
    /// FoodData.addExhaustion is unconditional and has no RNG or host effects.
    pub fn add_exhaustion(&mut self, amount: f32) {
        self.food.add_exhaustion(amount);
    }
    /// Player.causeFoodExhaustion, including creative-invulnerability and side
    /// gates. Regeneration below calls FoodData.addExhaustion directly instead.
    pub fn cause_exhaustion(&mut self, amount: f32, frame: PlayerFoodFrame) {
        self.food
            .cause_exhaustion(amount, frame.abilities, frame.is_client_side);
    }
    /// The peaceful fragment at Player.aiStep offsets17..149. No side gate is
    /// added: the original fragment itself executes on either logical side.
    pub fn peaceful_ai_step(
        &mut self,
        frame: PlayerFoodFrame,
        host: &mut impl PlayerFoodHost,
    ) -> Result<()> {
        if frame.difficulty != Difficulty::Peaceful || !frame.natural_regeneration {
            return Ok(());
        }
        if host.health()? < host.max_health()? && frame.tick_count % 20 == 0 {
            host.heal(1.)?;
        }
        if self.food.saturation < 20. && frame.tick_count % 20 == 0 {
            self.food.saturation += 1.;
        }
        if self.food.needs_food() && frame.tick_count % 10 == 0 {
            self.food.food_level = self.food.food_level.wrapping_add(1);
        }
        Ok(())
    }
    /// Exact FoodData.tick phase with synchronous, fallible real callbacks.
    /// A rejected callback leaves the original mutation prefix visible; callers
    /// must stop the phase rather than pretending subsequent effects ran.
    pub fn tick_food(
        &mut self,
        frame: PlayerFoodFrame,
        host: &mut impl PlayerFoodHost,
    ) -> Result<()> {
        self.food.last_food_level = self.food.food_level;
        if self.food.exhaustion > 4. {
            self.food.exhaustion -= 4.;
            if self.food.saturation > 0. {
                self.food.saturation = (self.food.saturation - 1.).max(0.);
            } else if frame.difficulty != Difficulty::Peaceful {
                self.food.food_level = self.food.food_level.wrapping_sub(1).max(0);
            }
        }
        if frame.natural_regeneration
            && self.food.saturation > 0.
            && is_hurt(host)?
            && self.food.food_level >= 20
        {
            self.food.tick_timer = self.food.tick_timer.wrapping_add(1);
            if self.food.tick_timer >= 10 {
                let amount = self.food.saturation.min(6.);
                host.heal(amount / 6.)?;
                self.food.add_exhaustion(amount);
                self.food.tick_timer = 0;
            }
        } else if frame.natural_regeneration && self.food.food_level >= 18 && is_hurt(host)? {
            self.food.tick_timer = self.food.tick_timer.wrapping_add(1);
            if self.food.tick_timer >= 80 {
                host.heal(1.)?;
                self.food.add_exhaustion(6.);
                self.food.tick_timer = 0;
            }
        } else if self.food.food_level <= 0 {
            self.food.tick_timer = self.food.tick_timer.wrapping_add(1);
            if self.food.tick_timer >= 80 {
                if host.health()? > 10.
                    || frame.difficulty == Difficulty::Hard
                    || (host.health()? > 1. && frame.difficulty == Difficulty::Normal)
                {
                    host.hurt_starvation(1.)?;
                }
                self.food.tick_timer = 0;
            }
        } else {
            self.food.tick_timer = 0;
        }
        Ok(())
    }
    /// Player.tick offsets182..256: server FoodData.tick followed by the five
    /// literal time-stat call sites. Invoke after the preceding Player/Living
    /// phases; this method does not substitute for them.
    pub fn player_tick_food_and_stats(
        &mut self,
        frame: PlayerFoodFrame,
        host: &mut impl PlayerFoodHost,
    ) -> Result<()> {
        if frame.is_client_side {
            return Ok(());
        }
        self.tick_food(frame, host)?;
        for value_id in [1, 2] {
            host.award_stat(
                StatKey {
                    type_id: CUSTOM,
                    value_id,
                },
                1,
            )?;
        }
        if host.is_alive()? {
            host.award_stat(
                StatKey {
                    type_id: CUSTOM,
                    value_id: 3,
                },
                1,
            )?;
        }
        if host.is_discrete()? {
            // Stats.CROUCH_TIME is registered as minecraft:sneak_time.
            host.award_stat(
                StatKey {
                    type_id: CUSTOM,
                    value_id: 5,
                },
                1,
            )?;
        }
        if !host.is_sleeping()? {
            host.award_stat(
                StatKey {
                    type_id: CUSTOM,
                    value_id: 4,
                },
                1,
            )?;
        }
        Ok(())
    }
}
fn is_hurt(host: &mut impl PlayerFoodHost) -> Result<bool> {
    // Player.isHurt reads health twice, with the positive-health short circuit.
    Ok(host.health()? > 0. && host.health()? < host.max_health()?)
}
