//! Real packet/body item-use composition for the native creative player.
//! Inventory edits are authoritative and await the ordinary menu broadcast.
use super::main_world_interactions::MainInteractionResult;
use super::*;
use mc121_rust::{
    entity_state::EntityPhysicalState,
    game_event_context::{self, GameEventPost},
    item_use::{self, ItemUseClock, ItemUseHost, JdkMathRandom, UseKind, UseSpec},
};

#[derive(Clone, Debug)]
struct NativeUse {
    uuid: [u8; 16],
    clock: ItemUseClock,
}
#[derive(Default)]
pub(super) struct MainItemUseRuntime {
    players: BTreeMap<i32, NativeUse>,
}
impl MainItemUseRuntime {
    pub fn register_native(&mut self, player: &PlayerState) -> Result<()> {
        ensure!(
            !self.players.contains_key(&player.entity_id),
            "item use player already registered"
        );
        self.players.insert(
            player.entity_id,
            NativeUse {
                uuid: player.profile.uuid,
                clock: ItemUseClock::default(),
            },
        );
        Ok(())
    }
    pub fn remove(&mut self, id: i32) {
        self.players.remove(&id);
    }
    pub fn is_using(&self, id: i32) -> bool {
        self.players.get(&id).is_some_and(|v| v.clock.is_using())
    }
}

struct NativeUseHost<'a> {
    players: &'a mut LivePlayers,
    id: i32,
    uuid: [u8; 16],
    position: Vec3,
    silent: bool,
    flags: u8,
    region: main_chunk_runtime::CurrentChunkQueryRegion,
    events: &'a broadcast::Sender<PlayerEvent>,
}
impl NativeUseHost<'_> {
    fn send(&self, recipients: Vec<i32>, packet: Packet) {
        let _ = self.events.send(PlayerEvent::WorldAudience {
            recipients: Arc::new(recipients),
            packets: Arc::new(vec![packet]),
        });
    }
    fn sound(
        &mut self,
        sound: i32,
        category: i32,
        volume: f32,
        pitch: f32,
        entity_sound: bool,
    ) -> Result<()> {
        // Entity.playSound's silence gate is inside the call, after the
        // caller has already evaluated its random pitch. Level.playSound's
        // Player.eat burp/inherited eat calls do not have this gate.
        if entity_sound && self.silent {
            return Ok(());
        }
        let seed = self.players.item_runtime.next_sound_seed()?;
        let mut w = Writer::new();
        w.varint(sound + 1)
            .varint(category)
            .i32((self.position.x * 8.) as i32)
            .i32((self.position.y * 8.) as i32)
            .i32((self.position.z * 8.) as i32)
            .f32(volume)
            .f32(pitch)
            .i64(seed);
        let range = f64::from(if volume > 1. { 16. * volume } else { 16. });
        let recipients = self
            .players
            .registry
            .snapshot()
            .into_iter()
            .filter(|p| {
                let dx = p.position.x - self.position.x;
                let dy = p.position.y - self.position.y;
                let dz = p.position.z - self.position.z;
                dx * dx + dy * dy + dz * dz < range * range
            })
            .map(|p| p.entity_id)
            .collect();
        self.send(
            recipients,
            Packet {
                id: 0x68,
                payload: w.into_inner(),
            },
        );
        Ok(())
    }
    fn used_stat(&mut self, stack: &ItemStack) -> Result<()> {
        self.players.gameplay.require_supported_floor()?;
        let name =
            inventory::item_name(stack.item_id).context("used item registry name missing")?;
        let key =
            mc121_rust::player_stats::stat_key("minecraft:used", &format!("minecraft:{name}"))
                .context("used item statistic missing")?;
        self.players
            .gameplay
            .stats_mut(self.uuid)?
            .increment(key, 1)?;
        Ok(())
    }
    fn food_finish(&mut self, stack: &ItemStack, spec: &UseSpec) -> Result<()> {
        let life = self
            .players
            .player_lifecycle
            .get_mut(&self.uuid)
            .context("item-use lifecycle missing")?;
        let mut food = mc121_rust::survival::FoodData {
            food_level: life.food.level,
            saturation: life.food.saturation,
            exhaustion: life.food.exhaustion,
            tick_timer: life.food.tick_timer,
            last_food_level: 20,
        };
        food.eat_properties(spec.nutrition, spec.saturation);
        life.food.level = food.food_level;
        life.food.saturation = food.saturation;
        self.used_stat(stack)?;
        let pitch = self.players.item_runtime.world_random_mut().next_f32() * 0.1 + 0.9;
        self.sound(UseSpec::burp_sound(), 7, 0.5, pitch, false)?;
        self.players.gameplay.require_supported_floor()?; // actual empty CONSUME_ITEM criterion owner
        let random = self.players.item_runtime.world_random_mut();
        let pitch = 1. + (random.next_f32() - random.next_f32()) * 0.4;
        self.sound(spec.sound, 6, 1., pitch, false)?;
        // Active food effects were checked before use; ItemStack.consume is
        // a real no-shrink branch for this player's instant-build ability.
        self.game_event("minecraft:eat")
    }
}
impl ItemUseHost for NativeUseHost<'_> {
    fn use_flags(&mut self, hand: i32, using: bool) -> Result<()> {
        if using {
            self.flags = (self.flags | 1) & !2 | if hand == 1 { 2 } else { 0 };
        } else {
            self.flags &= !1;
        }
        self.players.registry.set_entity_data(
            self.id,
            8,
            MetadataValue::Byte(self.flags),
            false,
        )?;
        Ok(())
    }
    fn game_event(&mut self, name: &'static str) -> Result<()> {
        let radius = UseSpec::game_event_radius(name)?;
        let mut frame = self.players.gameplay.frame_current(
            &self.players.registry,
            &self.players.chunk_runtime.manager,
            true,
            self.region,
        );
        game_event_context::post_to_empty_registries(
            &mut frame,
            GameEventPost {
                position: self.position,
                notification_radius: radius,
            },
        )?;
        Ok(())
    }
    fn using_criterion(&mut self, _: &ItemStack) -> Result<()> {
        self.players.gameplay.require_supported_floor()
    }
    fn trigger_effects(&mut self, spec: &UseSpec, count: i32) -> Result<()> {
        if spec.kind == UseKind::Food {
            if !spec.has_food {
                return Ok(());
            }
            let math = self
                .players
                .math_random
                .as_ref()
                .context("actual Math.random owner absent")?
                .clone();
            let random = self
                .players
                .player_random
                .get_mut(&self.id)
                .context("actual item-use entity RNG missing")?;
            // LivingEntity.spawnItemParticles, including Level.addParticle's
            // actual return-only server path. Each iteration consumes the
            // original three entity floats and one process-wide Math double.
            math.with_random(|global| {
                for _ in 0..count {
                    let _ = random.next_f32();
                    let _ = global.next_f64();
                    let _ = random.next_f32();
                    let _ = random.next_f32();
                }
                Ok(())
            })?;
            let volume = 0.5 + 0.5 * random.next_bounded_i32(2)? as f32;
            let pitch = (random.next_f32() - random.next_f32()) * 0.2 + 1.;
            self.sound(spec.sound, 7, volume, pitch, true)
        } else {
            let pitch = self.players.item_runtime.world_random_mut().next_f32() * 0.1 + 0.9;
            self.sound(spec.sound, 7, 0.5, pitch, true)
        }
    }
    fn complete_event(&mut self) -> Result<()> {
        let mut w = Writer::new();
        w.i32(self.id).u8(9);
        self.send(
            vec![self.id],
            Packet {
                id: 0x1f,
                payload: w.into_inner(),
            },
        );
        Ok(())
    }
    fn finish(&mut self, stack: &ItemStack, spec: &UseSpec, _hand: i32) -> Result<()> {
        match spec.kind {
            UseKind::Food => {
                if spec.has_food {
                    self.food_finish(stack, spec)
                } else {
                    Ok(())
                }
            }
            UseKind::Honey => {
                if spec.has_food {
                    self.food_finish(stack, spec)?;
                }
                self.players.gameplay.require_supported_floor()?; // second consume criterion in HoneyBottleItem
                self.used_stat(stack)?;
                // Native constructor-owned effect set is empty; removePoison
                // has no effect callback. Creative keeps the honey stack and
                // does not create the glass-bottle remainder.
                Ok(())
            }
            UseKind::Milk => {
                self.players.gameplay.require_supported_floor()?;
                self.used_stat(stack)?;
                // Same empty native effect owner as the supported body path.
                // Milk calls createFilledResult(..., false): even creative
                // inserts a bucket while retaining the unshrunk milk stack.
                let mut bucket = ItemStack::new(item_use::item_id("bucket")?, 1)?;
                let container = self
                    .players
                    .containers
                    .get(&self.id)
                    .context("milk inventory missing")?
                    .clone();
                container.transact(|menu| {
                    ensure!(
                        menu.add_pickup_to_inventory(&mut bucket, true)?,
                        "creative Inventory.add did not accept milk remainder"
                    );
                    Ok(((), vec![]))
                })?;
                Ok(())
            }
        }
    }
}

impl LivePlayers {
    fn item_use_host<'a>(
        &'a mut self,
        id: i32,
        position: Vec3,
        silent: bool,
        region: main_chunk_runtime::CurrentChunkQueryRegion,
        events: &'a broadcast::Sender<PlayerEvent>,
    ) -> Result<NativeUseHost<'a>> {
        self.gameplay.require_supported_floor()?;
        let player = self.registry.get(id).context("item-use player missing")?;
        let uuid = player.profile.uuid;
        let life = self
            .player_lifecycle
            .get(&uuid)
            .context("item-use lifecycle missing")?;
        ensure!(
            player.gamemode == 1
                && life.abilities.instant_build
                && life.abilities.invulnerable
                && life.dimension == "minecraft:overworld",
            "item-use native creative source unavailable"
        );
        let flags = match player.entity_data.get(8) {
            Some(MetadataValue::Byte(v)) => *v,
            _ => bail!("item-use synchronized flags unavailable"),
        };
        Ok(NativeUseHost {
            players: self,
            id,
            uuid,
            position,
            silent,
            flags,
            region,
            events,
        })
    }
    pub(super) fn tick_item_use_body(
        &mut self,
        id: i32,
        physical: &EntityPhysicalState,
        region: main_chunk_runtime::CurrentChunkQueryRegion,
        events: &broadcast::Sender<PlayerEvent>,
    ) -> Result<()> {
        let Some(mut runtime) = self.item_use_runtime.players.remove(&id) else {
            return Ok(());
        };
        let result = (|| {
            ensure!(
                runtime.uuid == physical.uuid,
                "item-use physical identity changed"
            );
            if let Some(active) = &runtime.clock.active {
                let stack = self
                    .containers
                    .get(&id)
                    .context("active-use inventory missing")?
                    .read(|m| m.inventory.selected_stack(active.hand).cloned())??;
                let spec = UseSpec::for_active_stack(&stack)?;
                let mut host = self.item_use_host(
                    id,
                    physical.position_vec(),
                    physical.silent,
                    region,
                    events,
                )?;
                runtime.clock.tick(&stack, spec.as_ref(), &mut host)?;
            }
            Ok(())
        })();
        self.item_use_runtime.players.insert(id, runtime);
        result
    }
}
impl Server {
    pub(super) async fn request_food_drink_use(
        &self,
        id: i32,
        hand: i32,
    ) -> Result<MainInteractionResult> {
        ensure!((0..=1).contains(&hand), "invalid use hand");
        let now = self
            .clock
            .read()
            .map_err(|_| anyhow!("clock poisoned"))?
            .game_time;
        let mut world = self.world.write().await;
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("players poisoned"))?;
        let Some(player) = players.registry.get(id).cloned() else {
            return Ok(MainInteractionResult::Rejected);
        };
        let stack = players
            .containers
            .get(&id)
            .context("use inventory missing")?
            .read(|m| m.inventory.selected_stack(hand).cloned())??;
        let spec = match UseSpec::for_stack(&stack) {
            Ok(Some(spec)) => spec,
            Ok(None) => return Ok(MainInteractionResult::Pass),
            Err(_) => return Ok(MainInteractionResult::Unsupported),
        };
        if !players.native_body_ready.contains(&id)
            || !players.item_use_runtime.players.contains_key(&id)
        {
            return Ok(MainInteractionResult::Unsupported);
        }
        players.require_current_native_source(&world)?;
        let position = Vec3::new(player.position.x, player.position.y, player.position.z);
        let min = (
            ((position.x - 17.).floor() as i32).div_euclid(16),
            ((position.z - 17.).floor() as i32).div_euclid(16),
        );
        let max = (
            ((position.x + 17.).floor() as i32).div_euclid(16),
            ((position.z + 17.).floor() as i32).div_euclid(16),
        );
        let LivePlayers {
            chunk_runtime,
            chunk_transports,
            ..
        } = &mut *players;
        let region = chunk_runtime.certify_current_query_region(
            &mut world,
            chunk_transports,
            &self.entity_events,
            now,
            min,
            max,
        )?;
        let mut runtime = players
            .item_use_runtime
            .players
            .remove(&id)
            .expect("validated item-use owner");
        let result = (|| {
            let mut host = players.item_use_host(
                id,
                position,
                matches!(
                    player.entity_data.get(4),
                    Some(MetadataValue::Boolean(true))
                ),
                region,
                &self.entity_events,
            )?;
            if spec.kind == UseKind::Food {
                let life = host
                    .players
                    .player_lifecycle
                    .get(&runtime.uuid)
                    .context("food lifecycle missing")?;
                if !(life.abilities.invulnerable || spec.always_eat || life.food.level < 20) {
                    return Ok(MainInteractionResult::Rejected);
                }
            }
            runtime.clock.start(hand, &stack, &spec, &mut host)?;
            Ok(MainInteractionResult::Applied)
        })();
        players.item_use_runtime.players.insert(id, runtime);
        result
    }
    pub(super) async fn release_item_use(&self, id: i32) -> Result<()> {
        let now = self
            .clock
            .read()
            .map_err(|_| anyhow!("clock poisoned"))?
            .game_time;
        let mut world = self.world.write().await;
        let mut players = self
            .players
            .write()
            .map_err(|_| anyhow!("players poisoned"))?;
        let Some(player) = players.registry.get(id).cloned() else {
            return Ok(());
        };
        if !players.item_use_runtime.is_using(id) {
            return Ok(());
        }
        players.require_current_native_source(&world)?;
        let position = Vec3::new(player.position.x, player.position.y, player.position.z);
        let min = (
            ((position.x - 17.).floor() as i32).div_euclid(16),
            ((position.z - 17.).floor() as i32).div_euclid(16),
        );
        let max = (
            ((position.x + 17.).floor() as i32).div_euclid(16),
            ((position.z + 17.).floor() as i32).div_euclid(16),
        );
        let LivePlayers {
            chunk_runtime,
            chunk_transports,
            ..
        } = &mut *players;
        let region = chunk_runtime.certify_current_query_region(
            &mut world,
            chunk_transports,
            &self.entity_events,
            now,
            min,
            max,
        )?;
        let mut runtime = players
            .item_use_runtime
            .players
            .remove(&id)
            .context("release item-use owner missing")?;
        let result = (|| {
            let mut host = players.item_use_host(
                id,
                position,
                matches!(
                    player.entity_data.get(4),
                    Some(MetadataValue::Boolean(true))
                ),
                region,
                &self.entity_events,
            )?;
            runtime.clock.stop(&mut host)
        })();
        players.item_use_runtime.players.insert(id, runtime);
        result
    }
}
