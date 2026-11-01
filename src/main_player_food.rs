//! Main's bounded native creative food callbacks and health delivery.
//!
//! These are explicit phase entry points, not a replacement Player.doTick. The
//! caller supplies the actual tick/difficulty/rule/attribute facts and invokes
//! pre/post aiStep at their respective Player call sites when those surrounding
//! phases exist. Hunger's saved fields remain owned only by PlayerLifecycle.
use super::{MainItemGameplay, PlayerEvent};
use anyhow::{bail, ensure, Context, Result};
use mc121_rust::{
    entity::{EntityRegistry, PlayerState},
    metadata::MetadataValue,
    player_data::PlayerLifecycle,
    player_food::{
        PlayerFoodFrame, PlayerFoodHost, PlayerFoodState, PlayerHealthSync, PlayerHealthUpdate,
    },
    player_stats::StatKey,
    survival::{Difficulty, FoodData, SurvivalAbilities},
};
use std::{collections::BTreeMap, sync::Arc};
use tokio::sync::broadcast;

#[derive(Clone, Copy, Debug)]
pub(super) struct MainFoodFacts {
    pub difficulty: Difficulty,
    pub natural_regeneration: bool,
    /// Actual physical Player.tickCount, never the connection/network counter.
    pub tick_count: i32,
    /// Actual current MAX_HEALTH attribute, including supported modifiers.
    pub max_health: f32,
}

pub(super) struct MainFoodServices<'a> {
    pub registry: &'a mut EntityRegistry,
    pub gameplay: &'a mut MainItemGameplay,
    pub events: &'a broadcast::Sender<PlayerEvent>,
}

#[derive(Clone, Debug)]
struct NativePlayerFood {
    uuid: [u8; 16],
    last_food_level: i32,
    health_sync: PlayerHealthSync,
}

#[derive(Default)]
pub(super) struct MainPlayerFoodRuntime {
    players: BTreeMap<i32, NativePlayerFood>,
}
impl MainPlayerFoodRuntime {
    pub fn register_native(
        &mut self,
        player: &PlayerState,
        lifecycle: &PlayerLifecycle,
    ) -> Result<()> {
        ensure!(
            !self.players.contains_key(&player.entity_id),
            "food player already registered"
        );
        supported_creative(player, lifecycle)?;
        health(player)?;
        self.players.insert(
            player.entity_id,
            NativePlayerFood {
                uuid: player.profile.uuid,
                // Original new FoodData followed by readAdditionalSaveData:
                // the save does not load lastFoodLevel.
                last_food_level: 20,
                health_sync: PlayerHealthSync::default(),
            },
        );
        Ok(())
    }
    pub fn remove(&mut self, id: i32) {
        self.players.remove(&id);
    }
    pub fn peaceful_ai_step(
        &mut self,
        id: i32,
        lifecycle: &mut PlayerLifecycle,
        facts: MainFoodFacts,
        services: MainFoodServices<'_>,
    ) -> Result<()> {
        self.run_phase(id, lifecycle, facts, services, FoodPhase::PeacefulAiStep)?;
        Ok(())
    }
    pub fn player_tick_food_and_stats(
        &mut self,
        id: i32,
        lifecycle: &mut PlayerLifecycle,
        facts: MainFoodFacts,
        services: MainFoodServices<'_>,
    ) -> Result<()> {
        self.run_phase(id, lifecycle, facts, services, FoodPhase::FoodAndTimeStats)?;
        Ok(())
    }
    /// Invoke in ServerPlayer.doTick's health-send slot, after Player.tick and
    /// item/menu/body effects that precede it. The targeted packet is appended
    /// synchronously under the caller's registry lock to the same event FIFO.
    pub fn send_health_changes(
        &mut self,
        id: i32,
        lifecycle: &mut PlayerLifecycle,
        facts: MainFoodFacts,
        services: MainFoodServices<'_>,
    ) -> Result<bool> {
        self.run_phase(id, lifecycle, facts, services, FoodPhase::SendHealth)
    }
    fn run_phase(
        &mut self,
        id: i32,
        lifecycle: &mut PlayerLifecycle,
        facts: MainFoodFacts,
        services: MainFoodServices<'_>,
        phase: FoodPhase,
    ) -> Result<bool> {
        ensure!(
            facts.max_health.is_finite() && facts.max_health > 0.,
            "actual player max-health attribute unavailable"
        );
        let runtime = self
            .players
            .get_mut(&id)
            .context("food player not registered")?;
        let player = services
            .registry
            .get(id)
            .context("food registry player missing")?;
        ensure!(
            runtime.uuid == player.profile.uuid,
            "food registry identity changed"
        );
        supported_creative(player, lifecycle)?;
        let mut food = owned_food(lifecycle, runtime.last_food_level);
        let frame = PlayerFoodFrame {
            difficulty: facts.difficulty,
            natural_regeneration: facts.natural_regeneration,
            tick_count: facts.tick_count,
            is_client_side: false,
            abilities: abilities(lifecycle),
        };
        let mut host = NativeFoodHost {
            id,
            uuid: runtime.uuid,
            max_health: facts.max_health,
            invulnerable: lifecycle.abilities.invulnerable,
            services,
        };
        let result = match phase {
            FoodPhase::PeacefulAiStep => food.peaceful_ai_step(frame, &mut host).map(|_| false),
            FoodPhase::FoodAndTimeStats => food
                .player_tick_food_and_stats(frame, &mut host)
                .map(|_| false),
            FoodPhase::SendHealth => runtime.health_sync.send_changes(&food.food, &mut host),
        };
        // Preserve a real mutation prefix even on a failed later callback;
        // PlayerLifecycle continues to be the sole saved hunger owner.
        lifecycle.food = food.saved();
        runtime.last_food_level = food.food.last_food_level;
        result
    }
}
#[derive(Clone, Copy)]
enum FoodPhase {
    PeacefulAiStep,
    FoodAndTimeStats,
    SendHealth,
}
fn abilities(lifecycle: &PlayerLifecycle) -> SurvivalAbilities {
    SurvivalAbilities {
        invulnerable: lifecycle.abilities.invulnerable,
        may_fly: lifecycle.abilities.may_fly,
    }
}
fn owned_food(lifecycle: &PlayerLifecycle, last_food_level: i32) -> PlayerFoodState {
    PlayerFoodState {
        food: FoodData {
            food_level: lifecycle.food.level,
            saturation: lifecycle.food.saturation,
            exhaustion: lifecycle.food.exhaustion,
            tick_timer: lifecycle.food.tick_timer,
            last_food_level,
        },
    }
}
fn supported_creative(player: &PlayerState, lifecycle: &PlayerLifecycle) -> Result<()> {
    ensure!(
        player.gamemode == 1 && lifecycle.abilities.invulnerable,
        "native food adapter requires the actual creative invulnerability branch"
    );
    ensure!(
        lifecycle.dimension == "minecraft:overworld",
        "native food dimension unavailable"
    );
    Ok(())
}
fn health(player: &PlayerState) -> Result<f32> {
    match player.entity_data.get(9) {
        Some(MetadataValue::Float(value)) => Ok(*value),
        _ => bail!("actual synchronized Player health missing"),
    }
}
struct NativeFoodHost<'a> {
    id: i32,
    uuid: [u8; 16],
    max_health: f32,
    invulnerable: bool,
    services: MainFoodServices<'a>,
}
impl NativeFoodHost<'_> {
    fn player(&self) -> Result<&PlayerState> {
        let player = self
            .services
            .registry
            .get(self.id)
            .context("food player removed during callback")?;
        ensure!(
            player.profile.uuid == self.uuid,
            "food callback player identity changed"
        );
        Ok(player)
    }
}
impl PlayerFoodHost for NativeFoodHost<'_> {
    fn health(&mut self) -> Result<f32> {
        health(self.player()?)
    }
    fn max_health(&mut self) -> Result<f32> {
        Ok(self.max_health)
    }
    fn heal(&mut self, amount: f32) -> Result<()> {
        let current = self.health()?;
        if current > 0. {
            let increased = current + amount;
            let next = if increased < 0. {
                0.
            } else if increased > self.max_health {
                self.max_health
            } else {
                increased
            };
            // Queue the actual health field for the normal metadata sender;
            // private SetHealth delivery is a separate later doTick call site.
            self.services.registry.set_entity_data(
                self.id,
                9,
                MetadataValue::Float(next),
                false,
            )?;
        }
        Ok(())
    }
    fn hurt_starvation(&mut self, _: f32) -> Result<bool> {
        if self.invulnerable {
            // Starvation does not bypass creative Player invulnerability.
            return Ok(false);
        }
        bail!("starvation requires the real Player damage/death/stat executor")
    }
    fn is_alive(&mut self) -> Result<bool> {
        // Registry membership establishes !removed for this live callback.
        Ok(self.health()? > 0.)
    }
    fn is_discrete(&mut self) -> Result<bool> {
        Ok(self.player()?.metadata.flags & 0x02 != 0)
    }
    fn is_sleeping(&mut self) -> Result<bool> {
        Ok(self.player()?.metadata.pose == 2)
    }
    fn award_stat(&mut self, key: StatKey, amount: i32) -> Result<()> {
        self.player()?;
        // Real stats update includes the currently certified empty scoreboard
        // objective path. Imported/nonempty gameplay registries are rejected.
        self.services.gameplay.require_supported_floor()?;
        self.services
            .gameplay
            .stats_mut(self.uuid)?
            .increment(key, amount)?;
        Ok(())
    }
    fn send_health(&mut self, update: PlayerHealthUpdate) -> Result<()> {
        self.player()?;
        self.services
            .events
            .send(PlayerEvent::WorldAudience {
                recipients: Arc::new(vec![self.id]),
                packets: Arc::new(vec![update.packet()]),
            })
            .map_err(|_| anyhow::anyhow!("food health delivery has no live event receiver"))?;
        Ok(())
    }
}

impl super::LivePlayers {
    /// Original peaceful restoration is before Player.aiStep's inventory/body
    /// and item-touch work. This entry point ports only that food fragment.
    pub(super) fn begin_native_food_slice(
        &mut self,
        id: i32,
        events: &broadcast::Sender<PlayerEvent>,
    ) -> Result<()> {
        let Some((uuid, facts)) = self.native_food_context(id)? else {
            return Ok(());
        };
        let lifecycle = self
            .player_lifecycle
            .get_mut(&uuid)
            .context("food player lifecycle missing")?;
        self.player_food_runtime.peaceful_ai_step(
            id,
            lifecycle,
            facts,
            MainFoodServices {
                registry: &mut self.registry,
                gameplay: &mut self.gameplay,
                events,
            },
        )
    }
    /// Main invokes this post-aiStep fragment after its existing item touch,
    /// before the drop limiter. It executes FoodData/time-stats then private
    /// health delivery; no inventory/AI/base/travel tick is invented here.
    pub(super) fn tick_native_food_slice(
        &mut self,
        id: i32,
        events: &broadcast::Sender<PlayerEvent>,
    ) -> Result<()> {
        let Some((uuid, facts)) = self.native_food_context(id)? else {
            return Ok(());
        };
        let lifecycle = self
            .player_lifecycle
            .get_mut(&uuid)
            .context("food player lifecycle missing")?;
        let runtime = &mut self.player_food_runtime;
        runtime.player_tick_food_and_stats(
            id,
            lifecycle,
            facts,
            MainFoodServices {
                registry: &mut self.registry,
                gameplay: &mut self.gameplay,
                events,
            },
        )?;
        runtime.send_health_changes(
            id,
            lifecycle,
            facts,
            MainFoodServices {
                registry: &mut self.registry,
                gameplay: &mut self.gameplay,
                events,
            },
        )?;
        Ok(())
    }
    fn native_food_context(&self, id: i32) -> Result<Option<([u8; 16], MainFoodFacts)>> {
        // No authoritative native physical owner means the earlier imported/
        // legacy transport scope remains active without this added food slice.
        let Some(physical) = self.player_motion_runtime.physical(id) else {
            return Ok(None);
        };
        let tick_count = physical.tick_count;
        let uuid = physical.uuid;
        let player = self.registry.get(id).context("food player missing")?;
        ensure!(
            player.profile.uuid == uuid,
            "food physical player identity changed"
        );
        // A native motion owner is established only from the owned constructor/
        // save lineage, whose profile has no active-effect producer. Empty
        // particles alone would not establish this source capability.
        ensure!(
            matches!(player.entity_data.get(10), Some(MetadataValue::Particles(values)) if values.is_empty())
                && matches!(
                    player.entity_data.get(11),
                    Some(MetadataValue::Boolean(false))
                ),
            "food native default-effect profile has changed"
        );
        self.containers
            .get(&id)
            .context("food player inventory missing")?
            .read(|menu| require_default_max_health(&menu.inventory))??;
        let natural_regeneration = self
            .world_context
            .rules
            .boolean("naturalRegeneration")
            .context("actual naturalRegeneration gamerule missing")?;
        Ok(Some((
            uuid,
            MainFoodFacts {
                difficulty: self.native_difficulty,
                natural_regeneration,
                tick_count,
                // Original fresh Player MAX_HEALTH attribute. Source lineage,
                // no-effect capability and live equipped modifiers are checked.
                max_health: 20.,
            },
        )))
    }
}

fn require_default_max_health(inventory: &mc121_rust::inventory::PlayerInventory) -> Result<()> {
    use mc121_rust::equipment::{EquipmentSlot, PlayerEquipment};
    let equipment = PlayerEquipment::from_inventory(inventory)?;
    for slot in EquipmentSlot::ALL {
        let stack = equipment.get(slot);
        if let Some(value) = stack.component_value(9)? {
            ensure!(
                value["enchantments"].as_array().is_some_and(Vec::is_empty),
                "food attribute/enchantment effects need their actual evaluator"
            );
        }
        if let Some(value) = stack.component_value(12)? {
            for attribute in value["attributes"]
                .as_array()
                .context("unparsed food equipment attribute component")?
            {
                let id = attribute["typeId"]
                    .as_u64()
                    .context("unparsed food equipment attribute ID")?;
                ensure!(id < 31, "unknown food equipment attribute ID {id}");
                // Actual built-in ATTRIBUTE ID18 is generic.max_health. Normal
                // sword/armour modifiers do not change this food phase's value.
                ensure!(
                    id != 18,
                    "food MAX_HEALTH modifier needs the actual attribute owner"
                );
            }
        }
    }
    Ok(())
}
