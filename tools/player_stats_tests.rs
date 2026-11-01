use super::*;
use crate::protocol::Reader;
use base64::Engine;

fn entries(values: &BTreeMap<StatKey, i32>) -> Value {
    Value::Array(
        values
            .iter()
            .map(
                |(key, value)| json!({"type_id":key.type_id,"value_id":key.value_id,"value":value}),
            )
            .collect(),
    )
}
fn keys(values: &BTreeSet<StatKey>) -> Value {
    serde_json::to_value(values).unwrap()
}
fn decode_packet(packet: &Packet) -> Value {
    assert_eq!(packet.id, AWARD_STATS_PACKET);
    let mut reader = Reader::new(&packet.payload);
    let count = reader.varint().unwrap();
    let mut values = BTreeMap::new();
    for _ in 0..count {
        let key = StatKey {
            type_id: reader.varint().unwrap(),
            value_id: reader.varint().unwrap(),
        };
        assert!(values.insert(key, reader.varint().unwrap()).is_none());
    }
    reader.finish().unwrap();
    entries(&values)
}

#[test]
fn original_counter_arithmetic_dirty_send_and_award_stats_codec() {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/player-stats-vectors.json")).unwrap();
    let mut snapshots = 0;
    let mut packets = 0;
    let mut exact_wire = 0;
    let mut pickup_awards = 0;
    for (case_index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let q = &case["query"];
        let server = q["server"].as_bool().unwrap();
        let mut stats = PlayerStats::default();
        for (step, (operation, expected)) in q["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(case["expected"]["states"].as_array().unwrap())
            .enumerate()
        {
            let op = operation["op"].as_str().unwrap();
            let mut result = Value::Null;
            let mut sent = Vec::new();
            let mut network = Vec::new();
            let key = || serde_json::from_value::<StatKey>(operation["key"].clone()).unwrap();
            match op {
                "get" => result = json!(stats.get(key())),
                "set" => stats
                    .set(key(), operation["value"].as_i64().unwrap() as i32)
                    .unwrap(),
                "increment" => {
                    stats
                        .increment(key(), operation["amount"].as_i64().unwrap() as i32)
                        .unwrap();
                }
                "pickup_award" => {
                    stats
                        .award_pickup(
                            operation["item_id"].as_i64().unwrap() as i32,
                            operation["original_count"].as_i64().unwrap() as i32,
                        )
                        .unwrap();
                    pickup_awards += 1;
                }
                "mark_all_dirty" => stats.mark_all_dirty(),
                "send" => {
                    sent.push(stats.send_stats_packet().unwrap());
                    network.push(json!({"dirty_at_send":keys(stats.dirty_keys())}));
                }
                "load_current" => stats
                    .load_current_stat_file_json(operation["json"].as_str().unwrap())
                    .unwrap(),
                "ordered_packet" => {
                    let input = operation["entries"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|entry| {
                            (
                                serde_json::from_value(entry["key"].clone()).unwrap(),
                                entry["value"].as_i64().unwrap() as i32,
                            )
                        })
                        .collect::<Vec<_>>();
                    sent.push(encode_award_stats(&input).unwrap());
                }
                _ => panic!("unknown original stat operation"),
            }
            assert_eq!(
                entries(stats.values()),
                expected["values"],
                "actual counter values case{case_index}step{step}"
            );
            assert_eq!(result, expected["result"]);
            if server {
                assert_eq!(
                    keys(stats.dirty_keys()),
                    expected["dirty"],
                    "actual dirty set case{case_index}step{step}"
                );
                assert_eq!(
                    serde_json::from_str::<Value>(&stats.to_stat_file_json().unwrap()).unwrap(),
                    expected["file"],
                    "actual canonical file data"
                );
            }
            assert_eq!(
                network,
                *expected["network"].as_array().unwrap(),
                "dirty clear precedes send"
            );
            assert_eq!(sent.len(), expected["packets"].as_array().unwrap().len());
            for (packet, original) in sent.iter().zip(expected["packets"].as_array().unwrap()) {
                assert_eq!(packet.id, original["packet_id"].as_i64().unwrap() as i32);
                assert_eq!(
                    decode_packet(packet),
                    original["values"],
                    "actual packet map case{case_index}step{step}"
                );
                packets += 1;
                if op == "ordered_packet" || original["values"].as_array().unwrap().len() <= 1 {
                    assert_eq!(
                        packet.payload,
                        base64::engine::general_purpose::STANDARD
                            .decode(original["wire"].as_str().unwrap())
                            .unwrap(),
                        "actual ordered packet bytes case{case_index}step{step}"
                    );
                    exact_wire += 1;
                }
            }
            snapshots += 1;
        }
    }
    println!(
        "PLAYER_STATS_CONFORMANCE:{}",
        json!({"ok":true,"original_histories":fixture["cases"].as_array().unwrap().len(),"operation_snapshots":snapshots,"award_stats_packet_maps":packets,"exact_ordered_or_single_entry_packet_bytes":exact_wire,"original_pickup_stat_events":pickup_awards,"mismatches":0,"scope":"Original StatsCounter/ServerStatsCounter shared arithmetic, server dirty and send semantics, actual packet codecs, canonical current3953 file data and real playerTouch stat inputs. Unordered map packets compare decoded entries; explicit LinkedHashMap and0/1entry packets compare literal bytes."})
    );
}

#[test]
fn all_original_registry_ids_and_canonical_current_file_roundtrip() {
    let original: Value =
        serde_json::from_str(include_str!("../fixtures/player-stats-registry.json")).unwrap();
    let mut count = 0;
    for kind in original["types"].as_array().unwrap() {
        for value in kind["values"].as_array().unwrap() {
            let key = StatKey::new(
                kind["type_id"].as_i64().unwrap() as i32,
                value["value_id"].as_i64().unwrap() as i32,
            )
            .unwrap();
            let (type_name, value_name) = stat_names(key).unwrap();
            assert_eq!(type_name, kind["type_name"].as_str().unwrap());
            assert_eq!(value_name, value["value_name"].as_str().unwrap());
            assert_eq!(stat_key(type_name, value_name), Some(key));
            count += 1;
        }
    }
    assert_eq!(count, 8060);
    assert!(StatKey::new(-1, 0).is_err());
    assert!(StatKey::new(9, 0).is_err());
    let input = r#"{"DataVersion":3953,"stats":{"minecraft:picked_up":{"minecraft:stone":-2147483648},"minecraft:custom":{"minecraft:drop":2147483647}}}"#;
    let mut stats = PlayerStats::from_current_stat_file_json(input).unwrap();
    assert!(stats.dirty_keys().is_empty());
    assert_eq!(
        serde_json::from_str::<Value>(&stats.to_stat_file_json().unwrap()).unwrap(),
        serde_json::from_str::<Value>(input).unwrap()
    );
    assert_eq!(stats.send_stats_packet().unwrap().payload, vec![0]);
    assert!(stats
        .load_current_stat_file_json(r#"{"DataVersion":1343,"stats":{}}"#)
        .is_err());
    assert!(stats
        .load_current_stat_file_json(
            r#"{"DataVersion":3953,"stats":{"minecraft:custom":{"minecraft:drop":0.5}}}"#
        )
        .is_err());
}

#[test]
fn original_server_player_drop_stat_gates() {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/player-stats-drop-vectors.json")).unwrap();
    let mut award_calls = 0;
    let mut world_rejections_with_awards = 0;
    let mut constructed_empty_with_custom_only = 0;
    for (case_index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let original = &case["expected"];
        let event = &original["event"];
        let input = DropStatEvent {
            constructed: event["constructed"].as_bool().unwrap(),
            trace_item: event["trace_item"].as_bool().unwrap(),
            returned_item_id: event["returned_item_id"].as_i64().map(|id| id as i32),
            original_count: event["original_count"].as_i64().unwrap() as i32,
        };
        let mut native = PlayerStats::default();
        native.award_drop(input).unwrap();
        assert_eq!(
            entries(native.values()),
            original["counter"]["values"],
            "original ServerPlayer.drop stat values case {case_index}"
        );
        assert_eq!(
            keys(native.dirty_keys()),
            original["counter"]["dirty"],
            "original ServerPlayer.drop dirty set case {case_index}"
        );
        assert_eq!(
            serde_json::from_str::<Value>(&native.to_stat_file_json().unwrap()).unwrap(),
            original["counter"]["file"]
        );

        // Actual unchanged aqv.drop executes the world callback first, then
        // the item statistic followed by custom DROP. Its boolean is ignored.
        let callbacks = original["callbacks"].as_array().unwrap();
        if !input.constructed {
            assert!(callbacks.is_empty());
            continue;
        }
        assert_eq!(callbacks[0]["callback"], "world_add");
        let expected_awards = if !input.trace_item {
            Vec::new()
        } else {
            let mut values = Vec::new();
            if let Some(item) = input.returned_item_id {
                values.push(json!({"callback":"award_stat","type_id":ITEM_DROPPED,"value_id":item,"amount":input.original_count}));
            }
            values.push(
                json!({"callback":"award_stat","type_id":CUSTOM,"value_id":CUSTOM_DROP,"amount":1}),
            );
            values
        };
        assert_eq!(
            &callbacks[1..],
            expected_awards.as_slice(),
            "literal original drop callback order case {case_index}"
        );
        award_calls += expected_awards.len();
        if !callbacks[0]["returned"].as_bool().unwrap() && !expected_awards.is_empty() {
            world_rejections_with_awards += 1;
        }
        if input.trace_item && input.returned_item_id.is_none() {
            constructed_empty_with_custom_only += 1;
        }
    }
    println!(
        "PLAYER_STATS_DROP_CONFORMANCE:{}",
        json!({"ok":true,"original_drop_cases":fixture["cases"].as_array().unwrap().len(),"award_stat_callbacks":award_calls,"world_rejections_still_awarding":world_rejections_with_awards,"constructed_empty_awards_custom_drop_only":constructed_empty_with_custom_only,"mismatches":0,"scope":"Unchanged original ServerPlayer.drop and Player.drop; environment addFreshEntity callback controlled and awardStat boundary records and invokes original StatsCounter.increment. Native checks callback-derived stat event, values, dirty keys and current-version file data. Scoreboards and advancements are outside this proof."})
    );
}
