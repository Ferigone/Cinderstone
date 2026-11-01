use super::*;
use crate::{
    entity::Position, item_entity::ItemState, random::LegacyRandomSource, world::FlatWorld,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};

#[derive(Default)]
struct DamageHost {
    events: Vec<Value>,
    damage_stacks: Vec<Vec<u8>>,
    discarded_stacks: Vec<Vec<u8>>,
    reject_damage: bool,
}
impl RuntimeItemHost for DamageHost {
    fn position_changed(&mut self, _: &EntityPhysicalState) -> Result<()> {
        bail!("damage-only oracle must not move the entity")
    }
    fn profile(&mut self, _: ProfileEvent) {}
    fn effect(&mut self, _: &EntityPhysicalState, _: RuntimeItemEffect) -> Result<()> {
        bail!("unexpected effect in damage-only oracle")
    }
    fn entity_damage(
        &mut self,
        state: &EntityPhysicalState,
        stack: &ItemStack,
        health: i32,
        causing_entity: Option<i32>,
    ) -> Result<()> {
        if self.reject_damage {
            bail!("active damage listeners unavailable");
        }
        assert!(causing_entity.is_none());
        assert!(state.hurt_marked);
        assert!(!state.removed);
        self.events
            .push(json!({"event":"minecraft:entity_damage", "health":health,
            "hurt_marked":state.hurt_marked, "removed":state.removed}));
        self.damage_stacks.push(stack.encode());
        Ok(())
    }
    fn discarded(&mut self, state: &EntityPhysicalState, stack: &ItemStack, _: i32) -> Result<()> {
        assert!(state.removed);
        assert_eq!(self.events.len(), 1, "damage game event precedes removal");
        self.discarded_stacks.push(stack.encode());
        Ok(())
    }
}
fn facts() -> ItemWorldFacts {
    ItemWorldFacts {
        game_time: 1,
        minimum_build_height: MIN_Y,
        ultrawarm: false,
        border: CollisionBorder::default(),
        rain: RainAvailability::NotRaining,
        has_passengers: false,
    }
}
fn item(stack: ItemStack) -> ItemState {
    ItemState::new(
        17,
        [1; 16],
        Position {
            x: 8.,
            y: 64.,
            z: 8.,
            yaw: 0.,
            pitch: 0.,
            on_ground: false,
        },
        Vec3::ZERO,
        stack,
    )
    .unwrap()
}

#[test]
fn concrete_damage_callbacks_match_original_server_hurt_and_destruction() {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/item-hurt-vectors.json")).unwrap();
    let dispatch: Value =
        serde_json::from_str(include_str!("../fixtures/item-destroyed-facts.json")).unwrap();
    let mut compared = 0;
    let mut destroyed = 0;
    for (index, row) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let q = &row["query"];
        if q["client"].as_bool().unwrap() {
            continue;
        }
        let hook = dispatch["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|fact| fact["item_name"] == q["item"])
            .unwrap();
        let stack = ItemStack::new(
            hook["item_id"].as_i64().unwrap() as i32,
            q["count"].as_i64().unwrap() as i32,
        )
        .unwrap();
        let before = stack.encode();
        let mut item = item(stack);
        item.health = q["health"].as_i64().unwrap() as i32;
        item.invulnerable = q["invulnerable"].as_bool().unwrap();
        item.removed = q["removed"].as_bool().unwrap();
        item.hurt_marked = q["hurt_marked"].as_bool().unwrap();
        item.take_metadata_dirty();
        let mut world = RuntimeWorld::Fixture(FlatWorld::new(0, 1, 10, 9, 39));
        let mut random = LegacyRandomSource::new(12345);
        let mut host = DamageHost::default();
        let mut provider = RuntimeItemWorld::new(
            &mut world,
            &item.physical,
            &mut random,
            &mut host,
            facts(),
            &[],
        )
        .unwrap();
        let mut source = DamageSourceFacts {
            bypasses_invulnerability: q["bypass"].as_bool().unwrap(),
            fire: q["fire"].as_bool().unwrap(),
            fall: q["fall"].as_bool().unwrap(),
            explosion: q["explosion"].as_bool().unwrap(),
            creative_player: q["creative"].as_bool().unwrap(),
            causing_entity: None,
        };
        let amount =
            f32::from_bits(u32::from_str_radix(q["amount_bits"].as_str().unwrap(), 16).unwrap());
        let accepted = item_damage::hurt_item(
            &mut item.physical,
            &mut item.stack,
            &mut item.health,
            &mut source,
            amount,
            &mut provider,
        )
        .unwrap();
        let expected = &row["result"];
        assert_eq!(
            json!({"accepted":accepted,"health":item.health,"hurt_marked":item.hurt_marked,
            "removed":item.removed,"fire_immune":item_damage::item_fire_immune(&item.stack)}),
            json!({"accepted":expected["accepted"],"health":expected["health"],"hurt_marked":expected["hurt_marked"],
                "removed":expected["removed"],"fire_immune":expected["fire_immune"]}),
            "concrete hurt case {index}"
        );
        let events: Vec<_> = expected["trace"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|value| value.is_object())
            .cloned()
            .collect();
        assert_eq!(host.events, events, "concrete event snapshot {index}");
        let dies = accepted && item.health <= 0;
        assert_eq!(host.discarded_stacks.len(), usize::from(dies));
        for stack in host.damage_stacks {
            assert_eq!(stack, before, "destruction must follow damage event");
        }
        if dies {
            let after = STANDARD
                .decode(hook["stack_wire_after"].as_str().unwrap())
                .unwrap();
            assert_eq!(
                item.stack.encode(),
                after,
                "original destruction stack {index}"
            );
            assert_eq!(
                host.discarded_stacks[0], after,
                "host removal after destruction {index}"
            );
            destroyed += 1;
        } else {
            assert_eq!(item.stack.encode(), before);
        }
        assert_eq!(
            item.take_metadata_dirty(),
            0,
            "damage/in-place destruction does not dirty metadata"
        );
        assert!(!item.has_impulse, "markHurt is separate from hasImpulse");
        assert_eq!(
            random.next_i32(),
            dispatch["expected_entity_rng_next_int"].as_i64().unwrap() as i32
        );
        compared += 1;
    }
    assert_eq!((compared, destroyed), (440, 84));
}

#[test]
fn concrete_base_damage_uses_original_on_fire_and_lava_facts() {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/item-hurt-vectors.json")).unwrap();
    let dispatch: Value =
        serde_json::from_str(include_str!("../fixtures/item-destroyed-facts.json")).unwrap();
    let mut compared = 0;
    for row in fixture["cases"].as_array().unwrap() {
        let q = &row["query"];
        if !q["fire"].as_bool().unwrap()
            || ["bypass", "fall", "explosion", "creative", "client"]
                .iter()
                .any(|key| q[key].as_bool().unwrap())
        {
            continue;
        }
        for damage in [BaseDamage::OnFire, BaseDamage::Lava] {
            let hook = dispatch["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|fact| fact["item_name"] == q["item"])
                .unwrap();
            let mut item = item(
                ItemStack::new(
                    hook["item_id"].as_i64().unwrap() as i32,
                    q["count"].as_i64().unwrap() as i32,
                )
                .unwrap(),
            );
            item.health = q["health"].as_i64().unwrap() as i32;
            item.invulnerable = q["invulnerable"].as_bool().unwrap();
            item.removed = q["removed"].as_bool().unwrap();
            item.hurt_marked = q["hurt_marked"].as_bool().unwrap();
            let mut world = RuntimeWorld::Fixture(FlatWorld::new(0, 1, 10, 9, 39));
            let mut random = LegacyRandomSource::new(12345);
            let mut host = DamageHost::default();
            let mut provider = RuntimeItemWorld::new(
                &mut world,
                &item.physical,
                &mut random,
                &mut host,
                facts(),
                &[],
            )
            .unwrap();
            let amount = f32::from_bits(
                u32::from_str_radix(q["amount_bits"].as_str().unwrap(), 16).unwrap(),
            );
            let accepted = FullItemWorld::hurt_item(
                &mut provider,
                &mut item.physical,
                &mut item.stack,
                &mut item.health,
                damage,
                amount,
            )
            .unwrap();
            let expected = &row["result"];
            assert_eq!(
                json!([accepted, item.health, item.hurt_marked, item.removed]),
                json!([
                    expected["accepted"],
                    expected["health"],
                    expected["hurt_marked"],
                    expected["removed"]
                ])
            );
            let events: Vec<_> = expected["trace"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|v| v.is_object())
                .cloned()
                .collect();
            assert_eq!(host.events, events);
            compared += 1;
        }
    }
    assert_eq!(compared, 62);
}

#[test]
fn concrete_damage_requires_real_host_effects_and_supported_removal_context() {
    let mut world = RuntimeWorld::Fixture(FlatWorld::new(0, 1, 10, 9, 39));
    let mut random = LegacyRandomSource::new(12345);
    let mut host = DamageHost::default();
    let mut item = item(ItemStack::new(1, 1).unwrap());
    let mut provider = RuntimeItemWorld::new(
        &mut world,
        &item.physical,
        &mut random,
        &mut host,
        facts(),
        &[],
    )
    .unwrap();
    item.vehicle = Some(9);
    item.removed = true;
    assert!(ItemDamageWorld::discarded(
        &mut provider,
        &mut item.physical,
        &item.stack,
        &mut item.health
    )
    .unwrap_err()
    .to_string()
    .contains("riding/passenger"));
    item.vehicle = None;
    provider.facts.has_passengers = true;
    assert!(ItemDamageWorld::discarded(
        &mut provider,
        &mut item.physical,
        &item.stack,
        &mut item.health
    )
    .is_err());
    assert!(provider.host.discarded_stacks.is_empty());
    provider.facts.has_passengers = false;
    provider.host.reject_damage = true;
    item.removed = false;
    assert!(FullItemWorld::hurt_item(
        &mut provider,
        &mut item.physical,
        &mut item.stack,
        &mut item.health,
        BaseDamage::OnFire,
        1.
    )
    .unwrap_err()
    .to_string()
    .contains("active damage listeners"));
    assert!(provider.host.events.is_empty());
}

#[test]
fn concrete_damage_rejects_unported_container_and_bundle_destruction() {
    let dispatch: Value =
        serde_json::from_str(include_str!("../fixtures/item-destroyed-facts.json")).unwrap();
    let mut rejected = 0;
    for fact in dispatch["items"].as_array().unwrap() {
        if fact["declaring_class"] == "cul"
            || (fact["declaring_class"] == "cso"
                && !fact["default_stack_contains_container"].as_bool().unwrap())
        {
            continue;
        }
        let mut item = item(ItemStack::new(fact["item_id"].as_i64().unwrap() as i32, 1).unwrap());
        let mut world = RuntimeWorld::Fixture(FlatWorld::new(0, 1, 10, 9, 39));
        let mut random = LegacyRandomSource::new(12345);
        let mut host = DamageHost::default();
        let mut provider = RuntimeItemWorld::new(
            &mut world,
            &item.physical,
            &mut random,
            &mut host,
            facts(),
            &[],
        )
        .unwrap();
        let error = item_damage::hurt_item(
            &mut item.physical,
            &mut item.stack,
            &mut item.health,
            &mut DamageSourceFacts::default(),
            5.,
            &mut provider,
        )
        .unwrap_err();
        assert!(error.to_string().contains("unported item destruction hook"));
        assert!(!item.removed);
        assert_eq!(host.events.len(), 1);
        assert!(host.discarded_stacks.is_empty());
        rejected += 1;
    }
    assert_eq!(rejected, 33);
}
