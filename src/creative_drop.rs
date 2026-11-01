//! ServerGamePacketListenerImpl's creative negative-slot drop throttle.
//!
//! The host first checks creative mode, enabled item features, and block-entity
//! component replacement. Positive creative inventory assignments are separate.

use crate::inventory::ItemStack;
use anyhow::Result;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CreativeDropGate {
    drop_spam_tick_count: i32,
}

impl CreativeDropGate {
    pub fn from_count(drop_spam_tick_count: i32) -> Self {
        Self {
            drop_spam_tick_count,
        }
    }
    pub fn count(&self) -> i32 {
        self.drop_spam_tick_count
    }
    /// Listener.tick decrements positive counts; zero and negative values stay.
    pub fn tick(&mut self) {
        if self.drop_spam_tick_count > 0 {
            self.drop_spam_tick_count -= 1;
        }
    }
    /// True means invoke Player.drop(stack, traceItem=true), whose wrapper uses
    /// around=false. Empty negative-slot attempts also consume twenty points;
    /// Player.drop then creates no item entity. There is no acknowledgement.
    pub fn allow_negative_slot_drop(&mut self, slot: i16, stack: &ItemStack) -> Result<bool> {
        if slot >= 0 || self.drop_spam_tick_count >= 200 {
            return Ok(false);
        }
        if !stack.is_empty() && stack.count > stack.max_stack_size()? {
            return Ok(false);
        }
        self.drop_spam_tick_count = self.drop_spam_tick_count.wrapping_add(20);
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use serde_json::Value;

    #[test]
    fn actual_java_creative_negative_slot_spam_gate_and_full_listener_tick() {
        let fixture: Value =
            serde_json::from_str(include_str!("../fixtures/creative_drop_vectors.json")).unwrap();
        let mut checked = 0;
        let mut empty_accepted = 0;
        for case in fixture["cases"].as_array().unwrap() {
            let query = &case["query"];
            let expected = &case["expected"];
            let name = query["name"].as_str().unwrap();
            let mut gate = CreativeDropGate::from_count(query["spam"].as_i64().unwrap() as i32);
            for (step, (op, state)) in query["operations"]
                .as_array()
                .unwrap()
                .iter()
                .zip(expected["states"].as_array().unwrap())
                .enumerate()
            {
                let attempts = state["attempts"].as_array().unwrap();
                if op["op"] == "tick" {
                    gate.tick();
                    assert!(attempts.is_empty());
                } else {
                    let input = &expected["inputs"][step];
                    let stack = ItemStack::decode_exact(
                        &STANDARD.decode(input["wire"].as_str().unwrap()).unwrap(),
                    )
                    .unwrap();
                    let slot = op["slot"].as_i64().unwrap() as i16;
                    let creative = query
                        .get("creative")
                        .and_then(Value::as_bool)
                        .unwrap_or(true);
                    let allowed = creative
                        && input["enabled"] == true
                        && gate.allow_negative_slot_drop(slot, &stack).unwrap();
                    assert_eq!(allowed, !attempts.is_empty(), "{name}/{step}/drop gate");
                    if allowed {
                        assert_eq!(attempts.len(), 1);
                        assert_eq!(attempts[0]["around"], false);
                        assert_eq!(attempts[0]["trace_item"], true);
                        assert_eq!(
                            stack.encode(),
                            STANDARD
                                .decode(attempts[0]["stack"].as_str().unwrap())
                                .unwrap()
                        );
                        if stack.is_empty() {
                            empty_accepted += 1;
                        }
                    }
                    if slot < 0 {
                        assert!(
                            state["packets"].as_array().unwrap().is_empty(),
                            "negative drop has no ack"
                        );
                    }
                }
                assert_eq!(
                    gate.count() as i64,
                    state["spam"].as_i64().unwrap(),
                    "{name}/{step}/spam"
                );
                checked += 1;
            }
        }
        assert_eq!(fixture["cases"].as_array().unwrap().len(), 820);
        assert_eq!(checked, 3282);
        assert_eq!(empty_accepted, 55);
    }
}
