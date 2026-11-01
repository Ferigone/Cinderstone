use super::*;
use crate::{entity::Position, geometry::Vec3};
use serde_json::{json, Value};
use std::{cell::RefCell, rc::Rc};

struct Source {
    query: Value,
    trace: Rc<RefCell<Vec<Value>>>,
}
impl ItemDamageSource for Source {
    fn is_tag(&mut self, tag: DamageTag) -> Result<bool> {
        let name = match tag {
            DamageTag::BypassesInvulnerability => "bypass",
            DamageTag::Fire => "fire",
            DamageTag::Fall => "fall",
            DamageTag::Explosion => "explosion",
        };
        self.trace.borrow_mut().push(json!(format!("tag/{name}")));
        Ok(self.query[name].as_bool().unwrap())
    }
    fn is_creative_player(&mut self) -> Result<bool> {
        self.trace.borrow_mut().push(json!("creative_source"));
        Ok(self.query["creative"].as_bool().unwrap())
    }
    fn causing_entity(&mut self) -> Result<Option<i32>> {
        self.trace.borrow_mut().push(json!("causing_entity"));
        Ok(None)
    }
}
struct World {
    client: bool,
    trace: Rc<RefCell<Vec<Value>>>,
    effects: Vec<&'static str>,
}
impl ItemDamageWorld for World {
    fn is_client_side(&self) -> bool {
        self.client
    }
    fn entity_damage(
        &mut self,
        state: &mut EntityPhysicalState,
        _: &ItemStack,
        health: &mut i32,
        causing_entity: Option<i32>,
    ) -> Result<()> {
        assert!(causing_entity.is_none());
        self.effects.push("damage");
        self.trace.borrow_mut().push(json!({"event":"minecraft:entity_damage","health":health,"hurt_marked":state.hurt_marked,"removed":state.removed}));
        Ok(())
    }
    fn on_destroyed(
        &mut self,
        state: &mut EntityPhysicalState,
        stack: &mut ItemStack,
        _: &mut i32,
    ) -> Result<()> {
        assert!(!state.removed);
        self.effects.push("destroyed");
        apply_supported_destruction(stack)?;
        Ok(())
    }
    fn discarded(
        &mut self,
        state: &mut EntityPhysicalState,
        _: &ItemStack,
        _: &mut i32,
    ) -> Result<()> {
        assert!(state.removed);
        self.effects.push("discarded");
        Ok(())
    }
}

#[test]
fn original_item_hurt_predicate_order_numeric_state_and_callbacks() {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/item-hurt-vectors.json")).unwrap();
    let mut accepted = 0;
    let mut destroyed = 0;
    let mut predicate_queries = 0;
    for (index, row) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let q = &row["query"];
        let expected = &row["result"];
        let name = q["item"].as_str().unwrap();
        let id = (0..1333)
            .find(|id| destruction_hook(*id).unwrap().item_name == name)
            .unwrap();
        let mut stack = ItemStack::new(id, q["count"].as_i64().unwrap() as i32).unwrap();
        let mut state = EntityPhysicalState::new(
            1,
            [0; 16],
            Position {
                x: 8.0,
                y: 64.0,
                z: 8.0,
                yaw: 0.0,
                pitch: 0.0,
                on_ground: false,
            },
            Vec3::ZERO,
            0.25,
            0.25,
            0.2125,
        )
        .unwrap();
        state.invulnerable = q["invulnerable"].as_bool().unwrap();
        state.removed = q["removed"].as_bool().unwrap();
        state.hurt_marked = q["hurt_marked"].as_bool().unwrap();
        let mut health = q["health"].as_i64().unwrap() as i32;
        let amount =
            f32::from_bits(u32::from_str_radix(q["amount_bits"].as_str().unwrap(), 16).unwrap());
        let trace = Rc::new(RefCell::new(Vec::new()));
        let mut source = Source {
            query: q.clone(),
            trace: trace.clone(),
        };
        let mut world = World {
            client: q["client"].as_bool().unwrap(),
            trace: trace.clone(),
            effects: Vec::new(),
        };
        let result = hurt_item(
            &mut state,
            &mut stack,
            &mut health,
            &mut source,
            amount,
            &mut world,
        )
        .unwrap();
        let observed = json!({"accepted":result,"health":health,"hurt_marked":state.hurt_marked,"removed":state.removed,"fire_immune":item_fire_immune(&stack),"trace":*trace.borrow()});
        assert_eq!(&observed, expected, "original hurt case {index}");
        assert!(!state.has_impulse, "markHurt must not set hasImpulse");
        let effects: &[&str] = if !result || world.client {
            &[]
        } else if health <= 0 {
            &["damage", "destroyed", "discarded"]
        } else {
            &["damage"]
        };
        assert_eq!(world.effects, effects, "ordered world effects {index}");
        accepted += usize::from(result);
        destroyed += usize::from(world.effects.contains(&"destroyed"));
        predicate_queries += trace.borrow().iter().filter(|v| v.is_string()).count();
    }
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 728);
    println!(
        "ITEM_DAMAGE_CONFORMANCE:{}",
        json!({"ok":true,"original_queries":728,"accepted":accepted,"destroyed":destroyed,"predicate_and_causer_queries":predicate_queries,"mismatches":0,"scope":"Original ItemEntity.hurt state, source query order, Java f32 health/F2I and damage callback snapshot; explicit destruction/discard callbacks follow original bytecode order. World callback implementation is controlled, not full runtime destruction."})
    );
}

#[test]
fn official_item_dispatch_facts_have_contiguous_registry_ids() {
    for id in 0..1333 {
        assert_eq!(destruction_hook(id).unwrap().item_id, id);
    }
    assert!(destruction_hook(-1).is_none());
    assert!(destruction_hook(1333).is_none());
}

#[test]
fn original_destruction_dispatch_and_exact_supported_stack_mutations() {
    use base64::Engine;
    let facts: Value =
        serde_json::from_str(include_str!("../fixtures/item-destroyed-facts.json")).unwrap();
    let decode = |wire: &Value| {
        base64::engine::general_purpose::STANDARD
            .decode(wire.as_str().unwrap())
            .unwrap()
    };
    let mut supported = 0;
    let mut mutated = 0;
    let mut deferred = 0;
    for row in facts["items"].as_array().unwrap() {
        let before = decode(&row["stack_wire_before"]);
        let mut stack = ItemStack::decode_exact(&before).unwrap();
        let is_base = row["declaring_class"] == "cul";
        assert_eq!(is_proved_no_op_destruction(&stack), is_base);
        let allowed = is_base
            || (row["declaring_class"] == "cso"
                && row["default_stack_contains_container"] == false);
        let result = apply_supported_destruction(&mut stack);
        if allowed {
            result.unwrap();
            assert_eq!(
                stack.encode(),
                decode(&row["stack_wire_after"]),
                "actual destruction stack {}",
                row["item_name"]
            );
            supported += 1;
            mutated += usize::from(!is_base);
        } else {
            assert!(
                result.is_err(),
                "unported destruction must fail {}",
                row["item_name"]
            );
            assert_eq!(stack.encode(), before, "rejected hook mutated stack");
            deferred += 1;
        }
    }
    assert_eq!((supported, mutated, deferred), (1300, 893, 33));
    println!(
        "ITEM_DESTRUCTION_CONFORMANCE:{}",
        json!({"ok":true,"original_registry_items":1333,"exact_supported_stack_states":supported,"in_place_container_mutations":mutated,"explicitly_deferred":deferred,"mismatches":0})
    );
}
