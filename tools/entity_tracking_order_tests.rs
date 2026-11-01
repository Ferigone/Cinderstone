use super::*;
use serde_json::Value;

#[test]
fn actual_shipped_fastutil_mutation_layout_and_iteration() {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/entity_tracking_order.json")).unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 14, "pinned independent mutation corpus");
    let mut states = 0;
    let mut entries = 0;
    for case in cases {
        let mut map = TrackingEntityMap::<i32>::new();
        let name = case["name"].as_str().unwrap();
        for (index, operation) in case["ops"].as_array().unwrap().iter().enumerate() {
            let context = format!("{name} operation {index}");
            let old = match operation["op"].as_str().unwrap() {
                "put" => map.insert(
                    operation["key"].as_i64().unwrap() as i32,
                    operation["value"].as_i64().unwrap() as i32,
                ),
                "remove" => map.remove(operation["key"].as_i64().unwrap() as i32),
                "clear" => {
                    map.clear();
                    None
                }
                other => panic!("unknown original mutation {other}"),
            };
            let expected = &operation["expected"];
            assert_eq!(
                old,
                expected["old"].as_i64().map(|value| value as i32),
                "{context} old value"
            );
            assert_eq!(
                map.len(),
                expected["size"].as_u64().unwrap() as usize,
                "{context} size"
            );
            assert_eq!(map.is_empty(), expected["size"] == 0, "{context} empty");
            assert_eq!(
                map.capacity(),
                expected["capacity"].as_u64().unwrap() as usize,
                "{context} capacity"
            );
            assert_eq!(
                map.has_zero,
                expected["contains_zero"].as_bool().unwrap(),
                "{context} zero slot"
            );
            assert_eq!(
                max_fill(map.capacity()),
                expected["max_fill"].as_u64().unwrap() as usize,
                "{context} fill threshold"
            );
            assert_eq!(expected["min_capacity"], 32, "{context} default minN");
            let expected_entries: Vec<_> = expected["entries"]
                .as_array()
                .unwrap()
                .iter()
                .map(|entry| {
                    (
                        entry[0].as_i64().unwrap() as i32,
                        entry[1].as_i64().unwrap() as i32,
                    )
                })
                .collect();
            let actual: Vec<_> = map.entries().collect();
            assert_eq!(actual, expected_entries, "{context} ordered entries");
            assert_eq!(
                map.values().collect::<Vec<_>>(),
                expected["values"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_i64().unwrap() as i32)
                    .collect::<Vec<_>>(),
                "{context} values"
            );
            for &(key, value) in &expected_entries {
                assert_eq!(map.get(key), Some(value), "{context} get {key}");
                assert!(map.contains_key(key), "{context} contains {key}");
            }
            for key in [0, operation["key"].as_i64().unwrap_or(0) as i32] {
                let expected = expected_entries
                    .iter()
                    .find(|entry| entry.0 == key)
                    .map(|entry| entry.1);
                assert_eq!(map.get(key), expected, "{context} queried key {key}");
                assert_eq!(
                    map.contains_key(key),
                    expected.is_some(),
                    "{context} queried membership {key}"
                );
            }
            if let Some(keys) = expected["key_slots"].as_array() {
                assert_eq!(
                    map.keys,
                    keys.iter()
                        .map(|value| value.as_i64().unwrap() as i32)
                        .collect::<Vec<_>>(),
                    "{context} whole key layout"
                );
            }
            states += 1;
            entries += expected_entries.len();
        }
    }
    assert_eq!(states, 3003);
    println!("actual shipped fastutil: {states} mutation states, {entries} ordered entries");
}
