use super::*;

/// `PresenceGuard::drop` now schedules `cleanup_player` on the runtime instead
/// of saving inline, so the test must wait for the real cleanup to finish
/// before it reads durable state. The wait is bounded and polls the actual
/// authoritative registry; it never substitutes a save of its own.
async fn wait_for_cleanup(server: &Arc<Server>, entity_id: i32) {
    for _ in 0..400 {
        if server.players.read().unwrap().registry.get(entity_id).is_none() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("player cleanup for entity {entity_id} did not finish");
}

#[tokio::test]
async fn native_main_save_rejoin_and_stale_presence_smoke() {
    let directory = std::env::temp_dir().join(format!(
        "mc121-main-save-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let options = Options {
        difficulty: 1,
        max_chained_neighbor_updates: 1_000_000,
        bind: IpAddr::V4(Ipv4Addr::LOCALHOST),
        port: 0,
        allow_offline: true,
        allow_public_offline: false,
        online_mode: false,
        fixtures: PathBuf::new(),
        world: directory.join("world.json"),
        anvil_world: None,
        view_distance: 2,
        resource_pack: None,
        resource_pack_required: false,
        server_directory: Some(directory.clone()),
        max_players: 20,
        enforce_whitelist: false,
        op_permission_level: 4,
        enforce_secure_profile: false,
        secure_profile_override: None,
        rate_limit: None,
        compression_threshold: None,
        loopback_service_keys: None,
    };
    // Reuse the production constructors so this fixture follows future fields.
    let connection_settings = mc121_rust::connection_settings::DedicatedConnectionSettings {
        enforce_secure_profile: false,
        ..Default::default()
    };
    let administration = main_administration::Administration::load(
        directory.clone(),
        main_administration::AdminSettings {
            max_players: options.max_players,
            whitelist_enabled: false,
            enforce_whitelist: options.enforce_whitelist,
            op_permission_level: options.op_permission_level,
        },
        main_admin_dispatch::wall_time_ms().unwrap(),
    )
    .unwrap();
    let facts = DimensionFacts {
        min_y: -64,
        height: 384,
        logical_height: 384,
        ultrawarm: false,
        has_skylight: true,
        has_ceiling: false,
    };
    let world = WorldBackend::Fixture(FlatWorld::new(0, 1, 10, 9, 39));
    let player_store = PlayerDataStore::new(directory.join("players"));
    let entity_store = EntityDataStore::new(directory.join("entities.json"));
    let source = entity_store.fresh_source(&options.world, &world).unwrap();
    let (events, _) = broadcast::channel(512);
    let (changes, _) = broadcast::channel(8);
    let (tick_events, _) = watch::channel(0);
    let mut players = LivePlayers::new(
        LegacyRandomSource::new(12345),
        LegacyRandomSource::new(67890),
        WorldContext::new(facts, WorldRules::default(), WeatherState::default()),
        2,
        true,
        events.clone(),
    );
    players.entity_source = Some(source);
    let actual_fixtures = fixtures(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/configuration.json"),
    ).await.unwrap();
    let server = Arc::new(Server {
        options,
        connection_settings,
        connection_rates: StdRwLock::new(BTreeMap::new()),
        fixtures: actual_fixtures,
        seed_hash: 0,
        world_seed: Some(0),
        world: RwLock::new(world),
        save_lock: Mutex::new(()),
        player_admission: Mutex::new(()),
        administration: StdRwLock::new(administration),
        connection_ticks: watch::channel(0).0,
        chat_keys: watch::channel(Arc::new(mc121_rust::chat_crypto::ServiceKeySet::empty())).0,
        chat_keys_attempted: watch::channel(false).0,
        chat_dispatch: Mutex::new(()),
        entities: AtomicI32::new(100),
        connection_sequence: AtomicU64::new(1),
        configuration_requests: broadcast::channel(64).0,
        changes,
        authenticator: None,
        session_verifier: None,
        players: StdRwLock::new(players),
        entity_events: events,
        clock: StdRwLock::new(TickClock::new(123, 456, true)),
        tick_events,
        epoch: Instant::now(),
        level: None,
        difficulty_locked: false,
        hardcore: false,
        console_command_position: Vec3::new(8.,64.,8.),
        border: StdRwLock::new(WorldBorder::default()),
        player_store,
        entity_store,
        shutting_down: AtomicBool::new(false),
    });
    let before_seed_clock = *server.clock.read().unwrap();
    let before_seed_random = server.players.read().unwrap().item_runtime.world_random_state48();
    server.execute_seed_command("seed", None, 4).unwrap();
    assert_eq!(*server.clock.read().unwrap(), before_seed_clock);
    assert_eq!(server.players.read().unwrap().item_runtime.world_random_state48(), before_seed_random);
    let profile = PlayerProfile::offline("NativeMainSave").unwrap();
    let uuid = profile.uuid;
    let position = Position {
        x: 31.5,
        y: 72.,
        z: -7.5,
        yaw: 37.,
        pitch: -11.,
        on_ground: true,
    };
    let register = |id: i32,
                    mut player: PlayerState,
                    inventory: inventory::PlayerInventory,
                    stats: PlayerStats,
                    lifecycle: PlayerLifecycle| {
        let mut players = server.players.write().unwrap();
        player.position = position;
        players.registry.register(player).unwrap();
        players
            .register_runtime_player(id, position, id as u64, LegacyRandomSource::new(90), uuid)
            .unwrap();
        players.gameplay.install_player_stats(uuid, stats).unwrap();
        let (container, _) =
            SharedContainer::from_container(PlayerContainer::from_inventory(inventory));
        players.containers.insert(id, container);
        players.player_lifecycle.insert(uuid, lifecycle);
    };
    let mut player = PlayerState::new(7, profile.clone(), position).unwrap();
    player
        .entity_data
        .set(9, MetadataValue::Float(7.5), false)
        .unwrap();
    let mut inventory = inventory::PlayerInventory::new();
    inventory
        .set_creative_slot(43, ItemStack::new(1, 7).unwrap())
        .unwrap();
    inventory.set_selected_hotbar(7).unwrap();
    let mut stats = PlayerStats::default();
    stats
        .set(mc121_rust::player_stats::StatKey::new(4, 1).unwrap(), 29)
        .unwrap();
    let mut lifecycle = PlayerLifecycle::new("minecraft:overworld", 1).unwrap();
    lifecycle.food.level = 9;
    register(7, player, inventory, stats, lifecycle);
    let mut book = ServerRecipeBook::default();
    book.set_open(
        mc121_rust::server_recipe_book::RecipeBookType::Crafting,
        true,
    );
    server
        .players
        .write()
        .unwrap()
        .recipe_books
        .insert(uuid, book);
    // Actual disconnect saves before removing the authoritative registry/menu.
    drop(PresenceGuard {
        entity_id: 7,
        server: Arc::clone(&server),
    });
    wait_for_cleanup(&server, 7).await;
    let mut saved = server.player_store.load(uuid).unwrap().unwrap();
    assert_eq!(saved.position, position);
    assert_eq!(saved.lifecycle.food.level, 9);
    assert_eq!(saved.inventory.selected_hotbar(), 7);
    assert_eq!(
        saved
            .stats
            .get(mc121_rust::player_stats::StatKey::new(4, 1).unwrap()),
        29
    );
    assert!(
        main_player_data::load_recipe_book(&saved.lifecycle)
            .unwrap()
            .settings()
            .booleans()[0]
    );
    saved
        .inventory
        .set_creative_slot(43, ItemStack::new(1, 11).unwrap())
        .unwrap();
    let mut replacement = PlayerState::new(8, profile, position).unwrap();
    saved.apply_to_player(&mut replacement).unwrap();
    register(
        8,
        replacement,
        saved.inventory,
        saved.stats,
        saved.lifecycle,
    );
    server
        .save_player_locked(&mut server.players.write().unwrap(), 8)
        .unwrap();
    // Old duplicate cleanup must not overwrite the replacement UUID record.
    drop(PresenceGuard {
        entity_id: 7,
        server: Arc::clone(&server),
    });
    // Entity 7 is already gone, so there is no registry change to await; give
    // the scheduled duplicate cleanup time to run before the assertion below.
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        server
            .player_store
            .load(uuid)
            .unwrap()
            .unwrap()
            .inventory
            .slot(43)
            .unwrap()
            .count,
        11
    );
    drop(PresenceGuard {
        entity_id: 8,
        server: Arc::clone(&server),
    });
    wait_for_cleanup(&server, 8).await;
    server.save().await.unwrap();
    let checkpoint = server.entity_store.load().unwrap().unwrap();
    let world = WorldBackend::Fixture(FlatWorld::new(0, 1, 10, 9, 39));
    let resumed = checkpoint.restore(&world).unwrap();
    assert_eq!(resumed.next_entity_id, 100);
    assert_eq!(resumed.time.clock.game_time, 123);
    assert_eq!(
        resumed.runtime.world_random_state48(),
        LegacyRandomSource::new(12345).state48()
    );
    assert_eq!(
        resumed.runtime.sound_random_state48(),
        Some(LegacyRandomSource::new(67890).state48())
    );
    std::fs::remove_dir_all(directory).unwrap();
}
