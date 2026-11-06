use super::*;
use mc121_rust::{
    entity::{PlayerProfile, Position},
    geometry::{Aabb, Vec3},
};
use serde_json::{json, Value};

fn position() -> Position {
    Position {
        x: 0.,
        y: 64.,
        z: 0.,
        yaw: 0.,
        pitch: 0.,
        on_ground: true,
    }
}
fn player(id: i32, uuid: Uuid, name: &str) -> PlayerState {
    PlayerState::new(
        id,
        PlayerProfile {
            uuid,
            name: name.into(),
            properties: vec![],
        },
        position(),
    )
    .unwrap()
}
fn picker() -> PickupPlayer {
    PickupPlayer {
        entity_id: 1,
        uuid: [1; 16],
        bounds: Aabb::player(Vec3::new(0., 64., 0.), 0.6, 1.8),
        alive: true,
        spectator: false,
        vehicle_bounds: None,
    }
}
fn counter_values(stats: &PlayerStats) -> Value {
    Value::Array(
        stats
            .values()
            .iter()
            .map(
                |(key, value)| json!({"type_id":key.type_id,"value_id":key.value_id,"value":value}),
            )
            .collect(),
    )
}
fn label(uuid: Uuid) -> &'static str {
    if uuid == [1; 16] {
        "picker"
    } else if uuid == [2; 16] {
        "thrower"
    } else {
        panic!("unexpected fixture UUID")
    }
}

#[test]
fn original_stat_then_owned_empty_scoreboard_and_pickup_criteria() {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/main-item-gameplay-vectors.json")).unwrap();
    let mut compared = 0;
    let mut unsupported = 0;
    let mut original_contexts = 0;
    for (case_index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let q = &case["query"];
        if q.get("inventory").is_some() {
            continue;
        }
        let original = &case["expected"];
        let mut registry = EntityRegistry::new();
        registry.register(player(1, [1; 16], "Picker")).unwrap();
        registry.register(player(2, [2; 16], "Thrower")).unwrap();
        let manager = RuntimeChunkManager::new(2, 2);
        let mut gameplay = MainItemGameplay::new(GameplaySourceScope::NativeFixtureEmptyCriteria);
        gameplay.register_player(1, [1; 16]).unwrap();
        gameplay.register_player(2, [2; 16]).unwrap();
        let item_id = q["item_id"].as_i64().unwrap() as i32;
        let count = q["count"].as_i64().unwrap() as i32;
        let key = StatKey::new(4, item_id).unwrap();
        gameplay
            .stats_mut([1; 16])
            .unwrap()
            .set(key, q["old_value"].as_i64().unwrap() as i32)
            .unwrap();
        if q["registry"] == "other_player" {
            gameplay.register_unported_criterion(
                [3; 16],
                GameplayCriterion::ThrownItemPickedUpByPlayer,
                "unrelated_registered_listener".into(),
            );
            assert!(gameplay.require_supported_floor().is_err());
            unsupported += 1;
            continue;
        }
        if q["registry"] == "empty_sets" {
            for uuid in [[1; 16], [2; 16]] {
                for criterion in [
                    GameplayCriterion::ThrownItemPickedUpByEntity,
                    GameplayCriterion::ThrownItemPickedUpByPlayer,
                    GameplayCriterion::InventoryChanged,
                ] {
                    gameplay.criteria.entry((uuid, criterion)).or_default();
                }
            }
        }
        let mut item = ItemState::new(
            100,
            [100; 16],
            position(),
            Vec3::ZERO,
            ItemStack::new(item_id, count).unwrap(),
        )
        .unwrap();
        item.thrower = match q["owner"].as_str().unwrap() {
            "none" => None,
            "player" => Some([2; 16]),
            "entity" => Some([3; 16]),
            _ => unreachable!(),
        };
        {
            let mut frame = gameplay.frame(&registry, &manager, true);
            frame.award_pickup_stat(picker(), item_id, count).unwrap();
            let after = frame.on_item_pickup(
                picker(),
                &item,
                Pickup {
                    item_entity_id: 100,
                    player_entity_id: 1,
                    packet_count: count,
                    accepted_count: count,
                    removed: false,
                },
            );
            if q["owner"] == "entity" {
                assert!(after.is_err(), "unknown owner must require real resolution");
                unsupported += 1;
                continue;
            }
            after.unwrap();
            item.removed = true;
            frame.discarded(&item, &item.stack, item.health).unwrap();
        }
        let values = counter_values(gameplay.stats_mut([1; 16]).unwrap());
        assert_eq!(
            values, original["counter"]["values"],
            "original pickup stat case {case_index}"
        );
        assert_eq!(
            serde_json::to_value(gameplay.stats_mut([1; 16]).unwrap().dirty_keys()).unwrap(),
            original["counter"]["dirty"]
        );
        let native = gameplay
            .drain_observations()
            .into_iter()
            .filter_map(|event| match event {
                GameplayObservation::EmptyScoreboard { player, .. } => Some(
                    json!({"callback":"scoreboard","player":label(player),"counter_values":values}),
                ),
                GameplayObservation::EmptyCriterion { player, .. } => {
                    Some(json!({"callback":"advancements","player":label(player)}))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let expected = original["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| {
                event["callback"] == "scoreboard" || event["callback"] == "advancements"
            })
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(
            native, expected,
            "stat/scoreboard/R/S callback projection case {case_index}"
        );
        assert_eq!(
            original["world_rng_before"], original["world_rng_after"],
            "actual LootContext retains existing RNG without draws"
        );
        assert_eq!(original["scoreboard_objectives"], 0);
        assert_eq!(original["scoreboard_player_scores"], 0);
        assert_eq!(original["scoreboard_teams"], 0);
        assert_eq!(original["scoreboard_team_members"], 0);
        assert_eq!(original["original_entity_removed_executed"], true);
        original_contexts += original["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| event["callback"] == "level_context")
            .count();
        compared += 1;
    }
    println!(
        "MAIN_ITEM_GAMEPLAY_PICKUP_CONFORMANCE:{}",
        json!({"ok":true,"original_supported_pickup_cases":compared,"original_loot_contexts_constructed":original_contexts,"explicit_nonempty_or_unknown_owner_rejections":unsupported,"mismatches":0,"scope":"Original awardStat counter then empty scoreboard gate, followed by original LivingEntity and ServerPlayer onItemPickup with owned empty criterion registries. Compare externally meaningful scoreboard/advancement lookup order and exact counter values/dirty keys; original LootContext path executes and consumes no level RNG. Unknown entity owners and nonempty criteria are rejected."})
    );
}

#[test]
fn original_inventory_changed_scan_has_its_own_empty_registry() {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/main-item-gameplay-vectors.json")).unwrap();
    let mut compared = 0;
    let mut slot_callbacks = 0;
    for case in fixture["cases"].as_array().unwrap() {
        let q = &case["query"];
        let Some(inventory_slots) = q.get("inventory") else {
            continue;
        };
        let mut gameplay = MainItemGameplay::new(GameplaySourceScope::NativeFixtureEmptyCriteria);
        gameplay.register_player(1, [1; 16]).unwrap();
        let mut inventory = PlayerInventory::new();
        let order = (36..45)
            .chain(9..36)
            .chain((5..9).rev())
            .chain(std::iter::once(45));
        for (slot, source) in order.zip(inventory_slots.as_array().unwrap()) {
            let stack = ItemStack::new(
                source["item_id"].as_i64().unwrap() as i32,
                source["count"].as_i64().unwrap() as i32,
            )
            .unwrap();
            assert!(matches!(
                inventory.set_creative_slot(slot as i16, stack).unwrap(),
                mc121_rust::inventory::CreativeSlotAction::Assigned { .. }
            ));
        }
        let expected = &case["expected"]["events"][0];
        assert_eq!(expected["callback"], "inventory_counts");
        for menu_slot in 0..46 {
            let result = gameplay
                .inventory_slot_changed(
                    [1; 16],
                    menu_slot,
                    &inventory,
                    &inventory.slots()[menu_slot],
                )
                .unwrap();
            if menu_slot < 5 {
                assert_eq!(result, None);
            } else {
                let result = result.unwrap();
                assert_eq!(
                    json!({"callback":"inventory_counts","full":result.full,"empty":result.empty,"occupied":result.occupied}),
                    *expected
                );
                slot_callbacks += 1;
            }
        }
        assert_eq!(
            case["expected"]["world_rng_before"],
            case["expected"]["world_rng_after"]
        );
        let observations = gameplay.drain_observations();
        assert_eq!(observations.len(), 82);
        for pair in observations.as_chunks::<2>().0 {
            assert!(matches!(
                pair,
                [
                    GameplayObservation::InventoryScan { .. },
                    GameplayObservation::EmptyCriterion {
                        criterion: GameplayCriterion::InventoryChanged,
                        ..
                    }
                ]
            ));
        }
        compared += 1;
    }
    let facts: Value =
        serde_json::from_str(include_str!("../fixtures/main-item-gameplay-facts.json")).unwrap();
    assert_eq!(facts["default_vanilla_advancement_files"], 1399);
    assert_eq!(
        facts["default_vanilla_trigger_counts"]["minecraft:inventory_changed"],
        1419
    );
    assert_eq!(
        facts["default_vanilla_stone_inventory_examples"]
            .as_array()
            .unwrap()
            .len(),
        14
    );
    assert_eq!(facts["default_vanilla_recipe_unlocks_implemented"], false);
    println!(
        "MAIN_ITEM_GAMEPLAY_INVENTORY_CONFORMANCE:{}",
        json!({"ok":true,"original_inventory_scans":compared,"native_inventory_menu_listener_callbacks":slot_callbacks,"default_vanilla_recipe_unlocks_implemented":false,"mismatches":0,"scope":"Original InventoryChangeTrigger scans all41 inventory slots before empty SimpleCriterionTrigger gate. Native counts cover all1333 prototype IDs across the corpus and InventoryMenu listener slots5..45; local callback scheduling belongs to the menu owner. Explicit empty fixture registry does not represent vanilla's1419 inventory criteria or its14 exact stone recipe predicates."})
    );
}

#[test]
fn prepared_menu_plan_rolls_back_and_commits_stats_and_empty_listeners_together() {
    use mc121_rust::{
        container::PlayerContainer,
        player_stats::{DropStatEvent, ITEM_DROPPED},
    };
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/main-item-gameplay-vectors.json")).unwrap();
    let original = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["query"].get("inventory").is_some())
        .nth(1)
        .unwrap();
    let original_counts = &original["expected"]["events"][0];
    assert_eq!(original_counts["full"], 41);
    let mut inventory = PlayerInventory::new();
    for slot in 5..46 {
        inventory
            .set_creative_slot(slot, ItemStack::new(1, 64).unwrap())
            .unwrap();
    }
    let mut menu = PlayerContainer::from_inventory(inventory);
    assert!(menu.enable_local_slot_events().unwrap().is_empty());
    let changes = menu.take_local_slot_changes();
    assert_eq!(changes.len(), 41);
    let mut gameplay = MainItemGameplay::new(GameplaySourceScope::NativeFixtureEmptyCriteria);
    gameplay.register_player(1, [1; 16]).unwrap();
    let mut next_stats = gameplay.stats_mut([1; 16]).unwrap().clone();
    next_stats
        .award_drop(DropStatEvent {
            constructed: true,
            trace_item: true,
            returned_item_id: Some(1),
            original_count: 5,
        })
        .unwrap();
    let staged = gameplay
        .prepare_menu_commit(
            [1; 16],
            &menu.inventory,
            changes.clone(),
            next_stats.clone(),
        )
        .unwrap();
    // A later failed world/drop preflight drops the token: no state escaped.
    drop(staged);
    assert!(gameplay.stats_mut([1; 16]).unwrap().values().is_empty());
    assert!(gameplay.drain_observations().is_empty());
    gameplay
        .prepare_menu_commit([1; 16], &menu.inventory, changes, next_stats)
        .unwrap()
        .commit();
    assert_eq!(
        gameplay
            .stats_mut([1; 16])
            .unwrap()
            .get(StatKey::new(ITEM_DROPPED, 1).unwrap()),
        5
    );
    let observations = gameplay.drain_observations();
    assert_eq!(observations.len(), 82);
    for pair in observations.as_chunks::<2>().0 {
        match pair {
            [GameplayObservation::InventoryScan { counts, .. }, GameplayObservation::EmptyCriterion {
                criterion: GameplayCriterion::InventoryChanged,
                ..
            }] => {
                assert_eq!(
                    json!({"callback":"inventory_counts","full":counts.full,"empty":counts.empty,"occupied":counts.occupied}),
                    *original_counts
                );
            }
            _ => panic!("prepared listener ordering"),
        }
    }
    let mut invalid = menu.clone();
    invalid
        .inventory
        .set_creative_slot(36, ItemStack::new(1, 1).unwrap())
        .unwrap();
    invalid.broadcast_changes().unwrap();
    let mut changes = invalid.take_local_slot_changes();
    assert_eq!(changes.len(), 1);
    changes[0].inventory_slot = None;
    let next_stats = gameplay.stats_mut([1; 16]).unwrap().clone();
    assert!(gameplay
        .prepare_menu_commit([1; 16], &invalid.inventory, changes, next_stats)
        .is_err());
    assert!(gameplay.drain_observations().is_empty());
}

#[test]
fn successful_reconnect_marks_existing_stats_dirty_after_a_previous_drain() {
    let mut gameplay = MainItemGameplay::new(GameplaySourceScope::NativeFixtureEmptyCriteria);
    gameplay.register_player(1, [1; 16]).unwrap();
    assert_eq!(
        gameplay
            .stats_mut([1; 16])
            .unwrap()
            .send_stats_packet()
            .unwrap()
            .payload,
        vec![0]
    );
    let values = [
        (StatKey::new(5, 1).unwrap(), 8),
        (StatKey::new(8, 22).unwrap(), 2),
        (StatKey::new(4, 1).unwrap(), 0),
        (StatKey::new(8, 0).unwrap(), i32::MAX),
    ];
    for (key, value) in values {
        gameplay
            .stats_mut([1; 16])
            .unwrap()
            .set(key, value)
            .unwrap();
    }
    let first = gameplay
        .stats_mut([1; 16])
        .unwrap()
        .send_stats_packet()
        .unwrap();
    assert_eq!(
        gameplay
            .stats_mut([1; 16])
            .unwrap()
            .send_stats_packet()
            .unwrap()
            .payload,
        vec![0]
    );
    gameplay.remove_player(1);
    gameplay.register_player(2, [1; 16]).unwrap();
    let again = gameplay
        .stats_mut([1; 16])
        .unwrap()
        .send_stats_packet()
        .unwrap();
    assert_eq!(again,first,"successful login must publish every existing value again, including stored zero and saturated counters");
    assert_eq!(
        gameplay
            .stats_mut([1; 16])
            .unwrap()
            .send_stats_packet()
            .unwrap()
            .payload,
        vec![0]
    );
    assert!(gameplay.register_player(2, [1; 16]).is_err());
    assert_eq!(
        gameplay
            .stats_mut([1; 16])
            .unwrap()
            .send_stats_packet()
            .unwrap()
            .payload,
        vec![0],
        "a rejected registration is not a successful login"
    );
    gameplay.register_player(3, [2; 16]).unwrap();
    assert_eq!(
        gameplay
            .stats_mut([2; 16])
            .unwrap()
            .send_stats_packet()
            .unwrap()
            .payload,
        vec![0],
        "fresh UUID must not inherit another player's counters"
    );
    println!(
        "MAIN_ITEM_GAMEPLAY_RECONNECT_CONFORMANCE:{}",
        json!({"ok":true,"successful_reconnects_after_dirty_drain":1,"republished_stored_keys":values.len(),"stored_zero_republished":true,"saturated_value_republished":true,"mismatches":0,"scope":"Original PlayerList.placeNewPlayer bytecode494 invokes actual ServerStatsCounter.markAllDirty on each successful login; that method is independently original-executed in the counter oracle. Native same-process UUID owner now re-marks all existing keys after a prior drain. This does not implement restart/file persistence."})
    );
}

#[test]
fn unknown_imports_registrations_and_listener_sources_fail_closed() {
    assert!(MainItemGameplay::new(GameplaySourceScope::UnknownImported)
        .require_supported_floor()
        .is_err());
    let mut gameplay = MainItemGameplay::new(GameplaySourceScope::NativeFixtureEmptyCriteria);
    gameplay.register_player(1, [1; 16]).unwrap();
    assert_eq!(
        gameplay.phase_scope(),
        PlayerPhaseScope::NativeItemCallbacksOnly
    );
    assert!(gameplay.require_supported_floor().is_ok());
    gameplay.register_unported_criterion(
        [1; 16],
        GameplayCriterion::InventoryChanged,
        "real_registered_listener".into(),
    );
    assert!(gameplay.require_supported_floor().is_err());
    gameplay.remove_player(1);
    assert!(gameplay.require_supported_floor().is_ok());
    gameplay
        .register_unported_objective(StatKey::new(4, 1).unwrap(), "real_objective".into())
        .unwrap();
    assert!(gameplay.require_supported_floor().is_err());

    let mut gameplay = MainItemGameplay::new(GameplaySourceScope::NativeFixtureEmptyCriteria);
    let sensor = *listener_facts().states.iter().next().unwrap();
    gameplay.observe_block_change((0, 64, 0), sensor).unwrap();
    assert!(gameplay.require_supported_floor().is_err());
    gameplay.observe_block_change((0, 64, 0), 0).unwrap();
    assert!(gameplay.require_supported_floor().is_ok());
    gameplay.invalidate_world("unowned dynamic registration source");
    assert!(gameplay.require_supported_floor().is_err());

    let mut gameplay = MainItemGameplay::new(GameplaySourceScope::NativeFixtureEmptyCriteria);
    let registry = EntityRegistry::new();
    let manager = RuntimeChunkManager::new(2, 2);
    let mut frame = gameplay.frame(&registry, &manager, true);
    assert!(
        frame.get_loaded_chunk(0, 0).is_err(),
        "unconfirmed FULL must not be fabricated as absent getChunkNow"
    );
}

#[test]
fn transactional_awards_survive_saturation_and_each_snapshot_commits_once() {
    let mut gameplay=MainItemGameplay::new(GameplaySourceScope::NativeFixtureEmptyCriteria);
    gameplay.register_player(1,[1;16]).unwrap();
    gameplay.scoreboard_names.insert([1;16],"Alice".into());
    let key=mc121_rust::player_stats::stat_key("minecraft:custom","minecraft:drop").unwrap();
    gameplay.stats_mut([1;16]).unwrap().set(key,i32::MAX).unwrap();
    mc121_rust::scoreboard::commands::execute(&mut gameplay.scoreboard,
        "scoreboard objectives add drops minecraft.custom:minecraft.drop",None,4,&[]).unwrap();
    let mut prepared=gameplay.snapshot_player_stats([1;16]).unwrap();
    prepared.increment(key,3).unwrap();
    gameplay.replace_player_stats([1;16],prepared.clone()).unwrap();
    prepared.increment(key,2).unwrap();
    gameplay.replace_player_stats([1;16],prepared.clone()).unwrap();
    gameplay.replace_player_stats([1;16],prepared).unwrap();
    assert_eq!(gameplay.stats_mut([1;16]).unwrap().get(key),i32::MAX);
    assert_eq!(gameplay.scoreboard.player_score_info("Alice","drops").unwrap().value,5);
    let saved=gameplay.snapshot_player_stats([1;16]).unwrap().to_stat_file_json().unwrap();
    assert!(!saved.contains("award_cursor")&&!saved.contains("transaction_awards"));
}
