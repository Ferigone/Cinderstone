#[test]
fn actual_java_selected_drop_updates_prediction_before_drop_without_ack() {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use serde_json::Value;
    fn decode(value: &Value) -> ItemStack {
        ItemStack::decode_exact(&STANDARD.decode(value.as_str().unwrap()).unwrap()).unwrap()
    }
    fn check(menu: &PlayerContainer, state: &Value, name: &str, step: usize) {
        for slot in 0..46 {
            assert_eq!(
                menu.inventory.slots()[slot].encode(),
                STANDARD
                    .decode(state["slots"][slot].as_str().unwrap())
                    .unwrap(),
                "{name}/{step}/slot{slot}"
            );
            assert_eq!(
                menu.remote_slots[slot].encode(),
                STANDARD
                    .decode(state["remote_slots"][slot].as_str().unwrap())
                    .unwrap(),
                "{name}/{step}/remote{slot}"
            );
        }
        assert_eq!(
            menu.inventory.carried().encode(),
            STANDARD.decode(state["carried"].as_str().unwrap()).unwrap(),
            "{name}/{step}/carried"
        );
        assert_eq!(
            menu.remote_carried.encode(),
            STANDARD
                .decode(state["remote_carried"].as_str().unwrap())
                .unwrap(),
            "{name}/{step}/remote carried"
        );
        assert_eq!(
            menu.inventory.state_id() as i64,
            state["state_id"].as_i64().unwrap(),
            "{name}/{step}/state"
        );
    }
    fn packets(actual: &[Packet], expected: &Value, name: &str, step: usize) {
        let expected = expected.as_array().unwrap();
        assert_eq!(actual.len(), expected.len(), "{name}/{step}/packet count");
        for (actual, expected) in actual.iter().zip(expected) {
            assert_eq!(
                actual.id as i64,
                expected["id"].as_i64().unwrap(),
                "{name}/{step}/packet id"
            );
            assert_eq!(
                actual.payload,
                STANDARD
                    .decode(expected["payload"].as_str().unwrap())
                    .unwrap(),
                "{name}/{step}/packet bytes"
            );
        }
    }
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/selected_drop_vectors.json")).unwrap();
    let mut operations = 0;
    for case in fixture["cases"].as_array().unwrap() {
        let query = &case["query"];
        let expected = &case["expected"];
        let initial = &expected["initial"];
        let name = query["name"].as_str().unwrap();
        let mut inventory = PlayerInventory::new();
        for slot in 0..46 {
            inventory.set_slot_authoritative(slot, decode(&initial["slots"][slot]));
        }
        inventory.set_carried_authoritative(decode(&initial["carried"]));
        inventory
            .set_selected_hotbar(query["selected"].as_i64().unwrap() as i16)
            .unwrap();
        for _ in 0..query["initial_state_id"].as_i64().unwrap() {
            inventory.increment_state_id();
        }
        let mut menu = PlayerContainer::from_inventory(inventory);
        menu.remote_slots = initial["remote_slots"]
            .as_array()
            .unwrap()
            .iter()
            .map(decode)
            .collect();
        menu.remote_carried = decode(&initial["remote_carried"]);
        check(&menu, initial, name, 0);
        for (step, (op, state)) in query["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(expected["states"].as_array().unwrap())
            .enumerate()
        {
            let outcome = menu.drop_selected(op["all"].as_bool().unwrap()).unwrap();
            let before = &state["before_broadcast"];
            check(&menu, before, name, step);
            packets(&outcome.packets, &before["packets"], name, step);
            assert!(outcome.crafted.is_empty());
            let expected_drops = before["drops"].as_array().unwrap();
            assert_eq!(
                outcome.drops.len(),
                expected_drops.len(),
                "{name}/{step}/drop count"
            );
            assert_eq!(
                !outcome.drops.is_empty(),
                state["returned"].as_bool().unwrap()
            );
            for (drop, expected) in outcome.drops.iter().zip(expected_drops) {
                assert_eq!(
                    drop.stack.encode(),
                    STANDARD.decode(expected.as_str().unwrap()).unwrap()
                );
                assert!(!drop.around);
                assert!(drop.trace_item);
            }
            let selected = 36 + menu.inventory.selected_hotbar();
            let attempts = before["attempts"].as_array().unwrap();
            assert_eq!(
                attempts.len(),
                1,
                "original ServerPlayer always calls drop, including EMPTY"
            );
            assert_eq!(attempts[0]["around"], false);
            assert_eq!(attempts[0]["trace_item"], true);
            assert_eq!(
                menu.inventory.slots()[selected].encode(),
                STANDARD
                    .decode(attempts[0]["inventory_selected"].as_str().unwrap())
                    .unwrap()
            );
            assert_eq!(
                menu.remote_slots[selected].encode(),
                STANDARD
                    .decode(attempts[0]["remote_selected"].as_str().unwrap())
                    .unwrap()
            );
            let deferred = if op["broadcast"] == true {
                menu.broadcast_changes().unwrap()
            } else {
                vec![]
            };
            check(&menu, &state["after_broadcast"], name, step);
            packets(&deferred, &state["after_broadcast"]["packets"], name, step);
            operations += 1;
        }
    }
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 226);
    assert_eq!(operations, 1096);
}
