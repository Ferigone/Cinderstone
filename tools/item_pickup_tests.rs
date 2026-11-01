use super::*;
use crate::{container::PlayerContainer, inventory::PlayerInventory};
use base64::Engine;
use serde_json::{json, Value};

fn snapshot(stack: &ItemStack, removed: bool) -> Value {
    json!({"count":if stack.is_empty(){0}else{stack.count},"raw_count":stack.count,"raw_item_id":stack.item_id,"empty":stack.is_empty(),"removed":removed,"stack_wire":base64::engine::general_purpose::STANDARD.encode(stack.encode())})
}
struct Host {
    container: PlayerContainer,
    creative: bool,
    client: bool,
    initially_removed: bool,
    events: Vec<Value>,
}
impl Host {
    fn event(&mut self, state: &ItemState, name: &str) -> &mut Value {
        let mut event = snapshot(&state.stack, state.removed);
        event["callback"] = json!(name);
        self.events.push(event);
        self.events.last_mut().unwrap()
    }
}
impl ItemPickupHost for Host {
    fn is_client_side(&self) -> bool {
        self.client
    }
    fn add_to_inventory(&mut self, stack: &mut ItemStack) -> Result<bool> {
        let accepted = self
            .container
            .add_pickup_to_inventory(stack, self.creative)?;
        let mut event = snapshot(stack, self.initially_removed);
        event["callback"] = json!("inventory_add");
        event["accepted"] = json!(accepted);
        self.events.push(event);
        Ok(accepted)
    }
    fn collect(&mut self, state: &ItemState, original: i32, _accepted: i32) -> Result<()> {
        self.event(state, "collect")["argument_count"] = json!(original);
        Ok(())
    }
    fn discarded(&mut self, state: &ItemState) -> Result<()> {
        self.event(state, "discard");
        Ok(())
    }
    fn after(&mut self, state: &ItemState, pickup: Pickup, original_item_id: i32) -> Result<()> {
        let event = self.event(state, "stat");
        event["argument_count"] = json!(pickup.packet_count);
        event["stat_item_id"] = json!(original_item_id);
        self.event(state, "on_pickup");
        Ok(())
    }
}

#[test]
fn original_player_touch_and_real_inventory_callback_states() {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/item-pickup-vectors.json")).unwrap();
    let decode = |wire: &Value| {
        ItemStack::decode_exact(
            &base64::engine::general_purpose::STANDARD
                .decode(wire.as_str().unwrap())
                .unwrap(),
        )
        .unwrap()
    };
    let mut callbacks = 0;
    let mut pickups = 0;
    let mut rejected_partial_transfers = 0;
    for (index, row) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let q = &row["query"];
        let expected = &row["expected"];
        let mut inventory = PlayerInventory::new();
        for (slot, wire) in expected["slot_wires"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            inventory.set_slot_authoritative(slot, decode(wire));
        }
        inventory
            .set_selected_hotbar(q["selected"].as_i64().unwrap() as i16)
            .unwrap();
        let mut item = ItemState::new(
            17,
            [0; 16],
            Position {
                x: 8.,
                y: 64.,
                z: 8.,
                yaw: 0.,
                pitch: 0.,
                on_ground: false,
            },
            Vec3::ZERO,
            decode(&expected["pickup_wire"]),
        )
        .unwrap();
        item.pickup_delay = q["pickup_delay"].as_i64().unwrap() as i32;
        let mut uuid = [0; 16];
        uuid[6] = 0x40;
        uuid[8] = 0x80;
        uuid[15] = 1;
        item.target = match q["target"].as_str().unwrap() {
            "none" => None,
            "match" => Some(uuid),
            "other" => {
                let mut other = uuid;
                other[15] = 2;
                Some(other)
            }
            _ => unreachable!(),
        };
        item.removed = q["removed"].as_bool().unwrap();
        item.take_metadata_dirty();
        let mut host = Host {
            container: PlayerContainer::from_inventory(inventory),
            creative: q["creative"].as_bool().unwrap(),
            client: q["client"].as_bool().unwrap(),
            initially_removed: item.removed,
            events: Vec::new(),
        };
        let original_count = item.stack.count;
        let result = item.try_pickup_with_callbacks(uuid, 19, &mut host).unwrap();
        assert_eq!(
            host.events,
            *expected["events"].as_array().unwrap(),
            "actual original pickup callbacks case{index}"
        );
        assert_eq!(
            snapshot(&item.stack, item.removed),
            expected["final"],
            "actual original source state case{index}"
        );
        assert_eq!(
            item.take_metadata_dirty(),
            expected["metadata_dirty_mask"].as_u64().unwrap() as u16,
            "pickup must not set Slot8"
        );
        let slots = host
            .container
            .inventory
            .slots()
            .iter()
            .map(|stack| json!(base64::engine::general_purpose::STANDARD.encode(stack.encode())))
            .collect::<Vec<_>>();
        assert_eq!(
            slots,
            expected["inventory_after"].as_array().unwrap().clone(),
            "actual original Inventory.add case{index}"
        );
        let collected = host
            .events
            .iter()
            .any(|event| event["callback"] == "collect");
        assert_eq!(result.is_some(), collected);
        if let Some(pickup) = result {
            assert_eq!(pickup.packet_count, original_count);
            pickups += 1;
        } else if item.stack.count < original_count {
            rejected_partial_transfers += 1;
        }
        callbacks += host.events.len();
    }
    println!(
        "ITEM_PICKUP_CONFORMANCE:{}",
        json!({"ok":true,"original_queries":fixture["cases"].as_array().unwrap().len(),"ordered_callback_snapshots":callbacks,"accepted_pickups":pickups,"rejected_partial_transfers":rejected_partial_transfers,"mismatches":0,"scope":"Original ItemEntity.playerTouch + real original Inventory.add versus native PlayerContainer + additive callback method. Collect empty/unremoved, discard empty/removed, stat and onPickup restored source; outer packet/menu/stat-counter side effects controlled."})
    );
}
