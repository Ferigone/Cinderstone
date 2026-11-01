use super::*;
use serde_json::{json, Value};

#[test]
fn captured_stock_server_player_constructor_rng_states() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../fixtures/player-rng-constructor-vectors.json"
    ))
    .unwrap();
    let mut modes = std::collections::BTreeMap::<String, usize>::new();
    for (index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let frames = case["observations"].as_array().unwrap();
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0]["stage"], "entity_rng_initialized");
        assert_eq!(frames[1]["stage"], "entity_exit");
        assert_eq!(frames[2]["stage"], "server_player_exit");
        let initial = frames[0]["state48"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap();
        // Reconstitute the observed raw input state through the already proved
        // LegacyRandomSource seed decoder; no expected output formula is used.
        let mut random = LegacyRandomSource::new((initial ^ 25_214_903_917) as i64);
        assert_eq!(random.state48(), initial);
        initialize_player_random(&mut random);
        for frame in &frames[1..] {
            assert_eq!(
                random.state48(),
                frame["state48"].as_str().unwrap().parse::<u64>().unwrap(),
                "actual stock constructor state{index}"
            );
            assert_eq!(frame["random_identity"], frames[0]["random_identity"]);
            assert_eq!(frame["random_class"], "dyz");
        }
        *modes
            .entry(case["client"]["mode"].as_str().unwrap().to_owned())
            .or_default() += 1;
    }
    assert_eq!(
        modes,
        std::collections::BTreeMap::from([
            ("adventure".into(), 3),
            ("creative".into(), 3),
            ("survival".into(), 3)
        ])
    );
    println!(
        "PLAYER_CONSTRUCTOR_RANDOM_CONFORMANCE:{}",
        json!({"ok":true,"actual_stock_server_players":9,"actual_constructor_observations":27,"native_initial_to_final_state_matches":9,"mismatches":0,"scope":"Actual plain ServerPlayer constructor through protocol767 clients in three modes; observer reads inherited RNG identity/state only. Native initialization consumes Entity UUID two nextLong calls. Global Math.random and separate spawn-location streams are outside this helper."})
    );
}
