use super::*;
use crate::{
    entity::{PlayerProfile, Position},
    entity_spatial::Visibility,
    geometry::Aabb,
    random::LegacyRandomSource,
    runtime_entities::{RuntimeEntities, RuntimeEntityEvent},
    runtime_item_world::ItemSound,
};
use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};

type Trace = Arc<Mutex<Vec<String>>>;

struct Hooks {
    trace: Trace,
    other_dimension: BTreeSet<i32>,
    reject_game_events: bool,
    reject_player_ticks: bool,
}
impl Hooks {
    fn new(trace: Trace) -> Self {
        Self {
            trace,
            other_dimension: BTreeSet::new(),
            reject_game_events: false,
            reject_player_ticks: false,
        }
    }
    fn record(&self, value: String) {
        self.trace.lock().unwrap().push(value);
    }
}
impl RuntimeGameplayHooks for Hooks {
    fn level_player_tick(&mut self, player: EntityHandle) -> Result<()> {
        self.record(format!("player_tick:{}:{}", player.id, player.generation));
        anyhow::ensure!(!self.reject_player_ticks, "actual player tick unavailable");
        Ok(())
    }
    fn profile(&mut self, event: ProfileEvent) {
        self.record(format!("profile:{event:?}"));
    }
    fn player_in_overworld(&self, player: &PlayerState) -> Result<bool> {
        Ok(!self.other_dimension.contains(&player.entity_id))
    }
    fn hit_ground(&mut self, state: &EntityPhysicalState, block: u32) -> Result<()> {
        self.record(format!("hit_ground:{}:{block}", state.entity_id));
        anyhow::ensure!(
            !self.reject_game_events,
            "actual game-event listeners unavailable"
        );
        Ok(())
    }
    fn splash(&mut self, state: &EntityPhysicalState) -> Result<()> {
        self.record(format!("splash:{}", state.entity_id));
        anyhow::ensure!(
            !self.reject_game_events,
            "actual game-event listeners unavailable"
        );
        Ok(())
    }
    fn entity_damage(
        &mut self,
        state: &EntityPhysicalState,
        stack: &ItemStack,
        health: i32,
        cause: Option<i32>,
    ) -> Result<()> {
        self.record(format!(
            "damage:{}:{}:{health}:{cause:?}",
            state.entity_id, stack.count
        ));
        Ok(())
    }
    fn discarded(
        &mut self,
        state: &EntityPhysicalState,
        stack: &ItemStack,
        health: i32,
    ) -> Result<()> {
        self.record(format!(
            "discard:{}:{}:{health}",
            state.removed, stack.count
        ));
        Ok(())
    }
    fn award_pickup_stat(&mut self, _: PickupPlayer, item: i32, count: i32) -> Result<()> {
        self.record(format!("stat:{item}:{count}"));
        Ok(())
    }
    fn inventory_slot_changed(
        &mut self,
        _: PickupPlayer,
        menu_slot: usize,
        inventory: &PlayerInventory,
        changed_stack: &ItemStack,
    ) -> Result<()> {
        assert_eq!(inventory.slot(menu_slot).unwrap(), changed_stack);
        self.record(format!("local:{menu_slot}:{}", changed_stack.count));
        Ok(())
    }
    fn on_item_pickup(&mut self, _: PickupPlayer, state: &ItemState, pickup: Pickup) -> Result<()> {
        self.record(format!(
            "on_pickup:{}:{}:{}",
            state.removed, state.stack.count, pickup.accepted_count
        ));
        Ok(())
    }
}

fn position(x: f64, y: f64, z: f64) -> Position {
    Position {
        x,
        y,
        z,
        yaw: 0.,
        pitch: 0.,
        on_ground: true,
    }
}
fn player(id: i32, gamemode: i32, position: Position) -> PlayerState {
    let mut player = PlayerState::new(
        id,
        PlayerProfile::offline(format!("Host{id}")).unwrap(),
        position,
    )
    .unwrap();
    player.gamemode = gamemode;
    player
}
fn pickup_player(player: &PlayerState) -> PickupPlayer {
    PickupPlayer {
        entity_id: player.entity_id,
        uuid: player.profile.uuid,
        bounds: Aabb::player(
            Vec3::new(player.position.x, player.position.y, player.position.z),
            0.6,
            1.8,
        ),
        alive: true,
        spectator: player.gamemode == 3,
        vehicle_bounds: None,
    }
}
fn item(count: i32) -> ItemState {
    ItemState::new(
        17,
        [17; 16],
        position(0., 64., 0.),
        Vec3::ZERO,
        ItemStack::new(1, count).unwrap(),
    )
    .unwrap()
}
fn menu(trace: Trace, fill_with_one_free: bool) -> SharedContainer {
    let container = SharedContainer::with_publisher(Arc::new(move |update| {
        trace.lock().unwrap().push(format!(
            "menu:{:?}",
            update
                .packets
                .iter()
                .map(|packet| packet.id)
                .collect::<Vec<_>>()
        ));
    }));
    if fill_with_one_free {
        container
            .transact(|menu| {
                for slot in 9..=44 {
                    menu.inventory
                        .set_slot_authoritative(slot, ItemStack::new(1, 64)?);
                }
                menu.inventory
                    .set_slot_authoritative(36, ItemStack::new(1, 63)?);
                Ok(((), vec![menu.full_sync()]))
            })
            .unwrap();
    }
    container
        .transact(|menu| {
            let packets = menu.enable_local_slot_events()?;
            menu.take_local_slot_changes();
            Ok(((), packets))
        })
        .unwrap();
    container
}
fn runtime(trace: Trace) -> RuntimeEntities {
    let mut runtime =
        RuntimeEntities::new(LegacyRandomSource::new(7)).with_publisher(Arc::new(move |event| {
            let text = match event {
                RuntimeEntityEvent::Collected { pickup, .. } => format!(
                    "collect:{}:{}:{}",
                    pickup.packet_count, pickup.accepted_count, pickup.removed
                ),
                RuntimeEntityEvent::Removed { entity_id, .. } => format!("remove:{entity_id}"),
                RuntimeEntityEvent::Update { .. } => "update".into(),
                RuntimeEntityEvent::Effect { .. } => "effect".into(),
            };
            trace.lock().unwrap().push(text);
        }));
    runtime
        .set_chunk_visibility((0, 0), Visibility::Tracked)
        .unwrap();
    runtime
        .insert_item(item(5), LegacyRandomSource::new(9))
        .unwrap();
    runtime.publish_tracking(|_| true).unwrap();
    runtime
}

#[test]
fn actual_coordinator_collect_precedes_menu_and_preserves_callback_state() {
    // Empty inventory and creative overflow both exercise actual Inventory.add
    // through the shared menu, coordinator and ordered publisher adapters.
    for fill in [false, true] {
        let trace: Trace = Arc::default();
        let registered = player(1, 1, position(0., 64., 0.));
        let context = pickup_player(&registered);
        let mut players = EntityRegistry::new();
        players.register(registered).unwrap();
        let shared = menu(Arc::clone(&trace), fill);
        let containers = BTreeMap::from([(1, shared.clone())]);
        let mut runtime = runtime(Arc::clone(&trace));
        let mut hooks = Hooks::new(Arc::clone(&trace));
        trace.lock().unwrap().clear();
        runtime
            .player_touch(
                context,
                &mut RuntimeInventoryHost::new(&players, &containers, &mut hooks),
            )
            .unwrap();
        assert_eq!(runtime.item_count(), 0);
        assert_eq!(
            shared
                .read(|menu| menu.inventory.slot(36).unwrap().count)
                .unwrap(),
            if fill { 64 } else { 5 }
        );
        let trace = trace.lock().unwrap();
        let index = |name: &str| {
            trace
                .iter()
                .position(|entry| entry == name)
                .unwrap_or_else(|| panic!("missing {name} in {trace:?}"))
        };
        let private = trace
            .iter()
            .position(|entry| entry.starts_with("menu:") && entry != "menu:[]")
            .unwrap();
        assert_eq!(
            trace.first().unwrap(),
            "menu:[]",
            "Inventory.add has no private packets"
        );
        assert!(index("collect:5:5:false") < private);
        let local = index(if fill { "local:36:64" } else { "local:36:5" });
        assert!(index("collect:5:5:false") < local);
        assert!(local < private);
        assert!(private < index("remove:17"));
        assert!(index("remove:17") < index("discard:true:0:5"));
        assert!(index("discard:true:0:5") < index("stat:1:5"));
        assert!(index("stat:1:5") < index("on_pickup:true:5:5"));
        assert_eq!(
            trace
                .iter()
                .filter(|entry| entry.starts_with("collect:"))
                .count(),
            1
        );
    }
}

#[test]
fn survival_partial_rejection_keeps_menu_dirty_and_item_cache_unpublished() {
    let trace: Trace = Arc::default();
    let registered = player(1, 0, position(0., 64., 0.));
    let context = pickup_player(&registered);
    let mut players = EntityRegistry::new();
    players.register(registered).unwrap();
    let shared = menu(Arc::clone(&trace), true);
    let containers = BTreeMap::from([(1, shared.clone())]);
    let mut runtime = runtime(Arc::clone(&trace));
    let mut hooks = Hooks::new(Arc::clone(&trace));
    trace.lock().unwrap().clear();
    runtime
        .player_touch(
            context,
            &mut RuntimeInventoryHost::new(&players, &containers, &mut hooks),
        )
        .unwrap();
    let source = runtime.item(17).unwrap();
    assert_eq!(source.stack.count, 4);
    assert_eq!(
        source.metadata_dirty, 0,
        "in-place remainder does not dirty Slot8"
    );
    assert!(!source.removed);
    assert_eq!(
        runtime.snapshots().unwrap().1[0].entity.stack.count,
        5,
        "physical count cannot leak into published cache"
    );
    assert_eq!(
        shared
            .read(|menu| menu.inventory.slot(36).unwrap().count)
            .unwrap(),
        64
    );
    assert_eq!(
        *trace.lock().unwrap(),
        vec!["menu:[]".to_owned()],
        "false Inventory.add skips take/stat/onPickup"
    );
    RuntimeInventoryHost::new(&players, &containers, &mut hooks)
        .level_player_tick(EntityHandle {
            id: 1,
            generation: 1,
        })
        .unwrap();
    assert_eq!(
        *trace.lock().unwrap(),
        vec!["menu:[]", "player_tick:1:1", "local:36:64", "menu:[21]"],
        "the next level-player phase flushes partial changes through the distinct local listener before the Slot packet"
    );
}

#[test]
fn sound_uses_original_packet_and_current_strict_dimension_audience() {
    let fixture: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/fixtures/item-effects-vectors.json"
        ))
        .unwrap(),
    )
    .unwrap();
    let original = &fixture["sound_cases"][0];
    assert_eq!(original["sound_name"], "minecraft:entity.generic.burn");
    let mut state = item(1).physical;
    state.set_position(Vec3::new(0., 64., -0.5));
    let inside = f64::from_bits(16f64.to_bits() - 1);
    let mut players = EntityRegistry::new();
    for (id, gamemode, pos) in [
        (1, 1, position(0., 64., -0.5)),
        (2, 1, position(16., 64., -0.5)),
        (3, 1, position(inside, 64., -0.5)),
        (4, 1, position(0., 80., -0.5)),
        (5, 1, position(0., 64., -0.5)),
        (6, 3, position(0., 64., -0.5)),
        (7, 1, position(0., 64., 15.5)),
    ] {
        players.register(player(id, gamemode, pos)).unwrap();
    }
    let containers = BTreeMap::new();
    let trace: Trace = Arc::default();
    let mut hooks = Hooks::new(trace);
    hooks.other_dimension.insert(5);
    let effect = RuntimeItemEffect::Sound {
        sound: ItemSound::GenericBurn,
        volume: 0.7,
        pitch: 1.6,
    };
    let first = RuntimeInventoryHost::new(&players, &containers, &mut hooks)
        .effect(&state, effect, Some(0))
        .unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].recipients, vec![1, 3, 6]);
    assert_eq!(
        first[0].packet.id,
        original["expected"]["packet_id"].as_i64().unwrap() as i32
    );
    let wire = original["expected"]["payload_hex"].as_str().unwrap();
    let bytes: Vec<u8> = (0..wire.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&wire[index..index + 2], 16).unwrap())
        .collect();
    state.set_position(Vec3::new(100., 64., 0.));
    assert_eq!(
        first[0].packet.payload, bytes,
        "effect encoded the captured callback position and seed"
    );
    state.set_position(Vec3::new(0., 64., -0.5));
    players
        .update_position(2, position(15., 64., -0.5))
        .unwrap();
    let second = RuntimeInventoryHost::new(&players, &containers, &mut hooks)
        .effect(&state, effect, Some(0))
        .unwrap();
    assert_eq!(second[0].recipients, vec![1, 2, 3, 6]);
    assert_eq!(
        first[0].recipients,
        vec![1, 3, 6],
        "previous delivery audience is captured, not deferred"
    );
}

#[test]
fn level_event_uses_original_block_origin_and_strict_sixty_four_radius() {
    let fixture: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/fixtures/item-effects-vectors.json"
        ))
        .unwrap(),
    )
    .unwrap();
    let original = &fixture["level_event_cases"][0];
    assert_eq!(original["event"], 1009);
    let mut state = item(1).physical;
    state.set_position(Vec3::new(0.25, 64., 0.25));
    let mut players = EntityRegistry::new();
    for (id, mode, pos) in [
        (1, 1, position(63., 64., 0.)),
        (2, 1, position(64., 64., 0.)),
        (3, 1, position(0., 64., 0.)),
        (4, 1, position(0., 128., 0.)),
        (5, 3, position(0., 64., 0.)),
    ] {
        players.register(player(id, mode, pos)).unwrap();
    }
    let containers = BTreeMap::new();
    let mut hooks = Hooks::new(Arc::default());
    hooks.other_dimension.insert(3);
    let effect = RuntimeItemEffect::Base(crate::entity_base::BaseEffect::MeltFrozen {
        position: (0, 64, 0),
        event: 1009,
        data: 1,
    });
    let deliveries = RuntimeInventoryHost::new(&players, &containers, &mut hooks)
        .effect(&state, effect, None)
        .unwrap();
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].recipients, vec![1, 5]);
    assert_eq!(
        deliveries[0].packet.id,
        original["expected"]["packet_id"].as_i64().unwrap() as i32
    );
    let wire = original["expected"]["payload_hex"].as_str().unwrap();
    let expected: Vec<u8> = (0..wire.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&wire[index..index + 2], 16).unwrap())
        .collect();
    assert_eq!(deliveries[0].packet.payload, expected);
}

#[test]
fn required_callbacks_and_invalid_identity_fail_explicitly() {
    let trace: Trace = Arc::default();
    let registered = player(1, 1, position(0., 64., 0.));
    let mut context = pickup_player(&registered);
    let mut players = EntityRegistry::new();
    players.register(registered).unwrap();
    let containers = BTreeMap::new();
    let mut hooks = Hooks::new(Arc::clone(&trace));
    let state = item(1);
    {
        let mut host = RuntimeInventoryHost::new(&players, &containers, &mut hooks);
        let mut source = ItemStack::new(1, 5).unwrap();
        assert!(host
            .add_to_inventory(context, 17, 5, &mut source)
            .unwrap_err()
            .to_string()
            .contains("container"));
        assert_eq!(source.count, 5);
        context.uuid[0] ^= 1;
        assert!(host
            .add_to_inventory(context, 17, 5, &mut source)
            .unwrap_err()
            .to_string()
            .contains("UUID"));
        assert_eq!(source.count, 5);
        host.profile(ProfileEvent::PushMove);
        host.level_player_tick(EntityHandle {
            id: 1,
            generation: 42,
        })
        .unwrap_err();
        assert!(host
            .effect(
                &state.physical,
                RuntimeItemEffect::HitGround { affected_block: 9 },
                None
            )
            .unwrap()
            .is_empty());
        assert!(host
            .effect(&state.physical, RuntimeItemEffect::Splash, None)
            .unwrap()
            .is_empty());
        host.entity_damage(&state.physical, &state.stack, 2, Some(99))
            .unwrap();
        assert!(host
            .effect(
                &state.physical,
                RuntimeItemEffect::HitGround { affected_block: 9 },
                Some(1)
            )
            .is_err());
    }
    assert_eq!(
        *trace.lock().unwrap(),
        vec![
            "profile:PushMove",
            "player_tick:1:42",
            "hit_ground:17:9",
            "splash:17",
            "damage:17:1:2:Some(99)"
        ]
    );
    hooks.reject_game_events = true;
    hooks.reject_player_ticks = true;
    let mut host = RuntimeInventoryHost::new(&players, &containers, &mut hooks);
    assert!(host
        .effect(&state.physical, RuntimeItemEffect::Splash, None)
        .unwrap_err()
        .to_string()
        .contains("listeners"));
    assert!(host
        .level_player_tick(EntityHandle {
            id: 1,
            generation: 42
        })
        .is_err());
}
