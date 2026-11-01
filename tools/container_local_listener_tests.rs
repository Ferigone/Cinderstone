// Included inside container::tests; independent replay of unchanged cpu,
// aqv$1 and aqv$2 observations. No Java expected values drive native decisions.
#[test]
fn actual_java_local_slot_listener_baselines_and_packet_order() {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use serde_json::Value;
    fn raw(value: &Value) -> Vec<u8> {
        STANDARD.decode(value.as_str().unwrap()).unwrap()
    }
    fn decode(value: &Value) -> ItemStack {
        ItemStack::decode_exact(&raw(value)).unwrap()
    }
    fn input_stack(operation: &Value) -> ItemStack {
        let mut stack = decode(&operation["source_wire"]);
        if let Some(count) = operation["count"].as_i64() {
            if stack.item_id != 0 {
                stack.count = count as i32;
            }
        }
        stack
    }
    fn check(menu: &PlayerContainer, state: &Value, name: &str, step: usize) {
        for slot in 0..46 {
            assert_eq!(
                menu.inventory.slots()[slot].encode(),
                raw(&state["slots"][slot]),
                "{name}/{step}/physical{slot}"
            );
            assert_eq!(
                menu.local_slot_baseline()[slot].encode(),
                raw(&state["last_slots"][slot]),
                "{name}/{step}/local{slot}"
            );
            assert_eq!(
                menu.remote_slots[slot].encode(),
                raw(&state["remote_slots"][slot]),
                "{name}/{step}/remote{slot}"
            );
            if state["last_canonical_empty"][slot] == true {
                assert_eq!(
                    menu.local_slot_baseline()[slot],
                    ItemStack::empty(),
                    "{name}/{step}/local canonical EMPTY{slot}"
                );
            }
            if state["remote_canonical_empty"][slot] == true {
                assert_eq!(
                    menu.remote_slots[slot],
                    ItemStack::empty(),
                    "{name}/{step}/remote canonical EMPTY{slot}"
                );
            }
        }
        assert_eq!(
            menu.inventory.carried().encode(),
            raw(&state["carried"]),
            "{name}/{step}/carried"
        );
        assert_eq!(
            menu.remote_carried.encode(),
            raw(&state["remote_carried"]),
            "{name}/{step}/remote carried"
        );
        assert_eq!(
            menu.inventory.state_id() as i64,
            state["state_id"].as_i64().unwrap(),
            "{name}/{step}/state id"
        );
        assert_eq!(
            usize::from(menu.local_slot_listener),
            state["listener_count"].as_u64().unwrap() as usize,
            "{name}/{step}/listener"
        );
        assert_eq!(
            state["suppressed"], false,
            "single-listener native replay does not claim external suppression API"
        );
    }
    fn check_events(
        changes: &[LocalSlotChange],
        packets: &[Packet],
        state: &Value,
        name: &str,
        step: usize,
    ) {
        let expected = state["events"].as_array().unwrap();
        let mut actual_kinds = Vec::new();
        let mut change_index = 0;
        for packet_index in 0..=packets.len() {
            while change_index < changes.len()
                && changes[change_index].packet_offset == packet_index
            {
                actual_kinds.push("local");
                if changes[change_index].inventory_slot.is_some() {
                    actual_kinds.push("criteria");
                }
                change_index += 1;
            }
            if packet_index < packets.len() {
                actual_kinds.push("packet");
            }
        }
        assert_eq!(
            change_index,
            changes.len(),
            "{name}/{step}/callback packet offset"
        );
        assert_eq!(
            actual_kinds,
            expected
                .iter()
                .map(|e| e["kind"].as_str().unwrap())
                .collect::<Vec<_>>(),
            "{name}/{step}/ordered local→criteria→packet trace"
        );
        let local = expected
            .iter()
            .filter(|e| e["kind"] == "local")
            .collect::<Vec<_>>();
        assert_eq!(changes.len(), local.len(), "{name}/{step}/local count");
        for (actual, expected) in changes.iter().zip(local) {
            assert_eq!(
                actual.menu_slot as u64,
                expected["slot"].as_u64().unwrap(),
                "{name}/{step}/local slot"
            );
            assert_eq!(
                actual.inventory_slot.map(|v| v as u64),
                expected["inventory_slot"].as_u64(),
                "{name}/{step}/original aqv$2 inventory backing filter"
            );
            assert_eq!(
                actual.stack.encode(),
                raw(&expected["stack"]),
                "{name}/{step}/local copy value"
            );
            assert_eq!(
                actual.packet_offset as u64,
                expected["packet_offset"].as_u64().unwrap(),
                "{name}/{step}/local emission position"
            );
            assert_eq!(
                expected["same_as_last"], true,
                "original updates local cache before listener"
            );
            if expected["canonical_empty"] == true {
                assert_eq!(
                    actual.stack,
                    ItemStack::empty(),
                    "{name}/{step}/callback canonical empty"
                );
            } else {
                assert_eq!(
                    expected["same_as_live"], false,
                    "nonempty listener receives a copy"
                );
            }
        }
        let remote = expected
            .iter()
            .filter(|e| e["kind"] == "packet")
            .collect::<Vec<_>>();
        assert_eq!(packets.len(), remote.len(), "{name}/{step}/packet count");
        for (actual, expected) in packets.iter().zip(remote) {
            assert_eq!(
                actual.id as u64,
                expected["id"].as_u64().unwrap(),
                "{name}/{step}/packet id"
            );
            assert_eq!(
                actual.payload,
                raw(&expected["payload"]),
                "{name}/{step}/original aqv$1 packet bytes"
            );
        }
    }
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/container_local_listener.json")).unwrap();
    let mut cases = 0;
    let mut operations = 0;
    let mut callbacks = 0;
    for case in fixture["cases"].as_array().unwrap() {
        let query = &case["query"];
        if query["native"] != true {
            continue;
        }
        let expected = &case["expected"];
        let initial = &expected["initial"];
        let name = query["name"].as_str().unwrap();
        let mut inventory = PlayerInventory::new();
        for slot in 0..46 {
            inventory.set_slot_authoritative(slot, decode(&initial["slots"][slot]));
        }
        inventory.set_carried_authoritative(decode(&initial["carried"]));
        for _ in 0..query["state_id"].as_u64().unwrap() {
            inventory.increment_state_id();
        }
        let mut menu = PlayerContainer::from_inventory(inventory);
        // Actual constructor uses EMPTY for remoteSlots. from_inventory is a
        // host setup convenience, so explicitly restore the captured boundary.
        menu.remote_slots = initial["remote_slots"]
            .as_array()
            .unwrap()
            .iter()
            .map(decode)
            .collect();
        menu.remote_carried = decode(&initial["remote_carried"]);
        check(&menu, initial, name, 0);
        let mut retained = Vec::new();
        for (step, (op, state)) in query["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(expected["states"].as_array().unwrap())
            .enumerate()
        {
            let mut packets = Vec::new();
            match op["op"].as_str().unwrap() {
                "add" => packets = menu.enable_local_slot_events().unwrap(),
                "set_slot" => {
                    *menu
                        .inventory
                        .slot_mut(op["slot"].as_u64().unwrap() as usize)
                        .unwrap() = input_stack(op)
                }
                "count" => {
                    let slot = op["slot"].as_u64().unwrap() as usize;
                    let stack = menu.inventory.slot_mut(slot).unwrap();
                    if stack.item_id != 0 {
                        stack.count = op["count"].as_i64().unwrap() as i32;
                    }
                }
                "carried" => menu.inventory.set_carried_authoritative(input_stack(op)),
                "remote_slot" => {
                    menu.remote_slots[op["slot"].as_u64().unwrap() as usize] =
                        copy_stack(&input_stack(op))
                }
                "remote_carried" => menu.remote_carried = copy_stack(&input_stack(op)),
                "broadcast" => packets = menu.broadcast_changes().unwrap(),
                "full" => packets.push(menu.broadcast_full_state().unwrap()),
                "send_all" | "set_sync" => packets.push(menu.full_sync()),
                other => panic!("unsupported native operation {other}"),
            }
            let changes = menu.take_local_slot_changes();
            check_events(&changes, &packets, state, name, step);
            retained.extend(changes.iter().map(|change| change.stack.clone()));
            let expected_retained = state["retained_callback_stacks"].as_array().unwrap();
            assert_eq!(
                retained.len(),
                expected_retained.len(),
                "{name}/{step}/retained copy count"
            );
            for (actual, expected) in retained.iter().zip(expected_retained) {
                assert_eq!(
                    actual.encode(),
                    raw(expected),
                    "{name}/{step}/old callback copy must survive live stack mutation"
                );
            }
            check(&menu, state, name, step);
            callbacks += changes.len();
            operations += 1;
        }
        cases += 1;
    }
    assert_eq!(cases, 102);
    assert_eq!(operations, 1985);
    assert!(callbacks > 900);
}
