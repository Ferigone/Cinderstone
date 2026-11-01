use super::*;
use crate::{entity::Position, protocol::Reader};
use serde_json::Value;

fn hex(value: &str) -> Vec<u8> {
    (0..value.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&value[i..i + 2], 16).unwrap())
        .collect()
}
fn float(value: &Value) -> f32 {
    f32::from_bits(u32::from_str_radix(value.as_str().unwrap(), 16).unwrap())
}
fn vector(value: &Value) -> Vec3 {
    let double = |v: &Value| f64::from_bits(u64::from_str_radix(v.as_str().unwrap(), 16).unwrap());
    Vec3::new(double(&value[0]), double(&value[1]), double(&value[2]))
}
fn sound(name: &str) -> ItemSound {
    match name {
        "minecraft:entity.generic.splash" => ItemSound::GenericSplash,
        "minecraft:entity.generic.extinguish_fire" => ItemSound::GenericExtinguishFire,
        "minecraft:entity.generic.burn" => ItemSound::GenericBurn,
        _ => panic!("unexpected sound in original item effect fixture"),
    }
}
fn assert_packet(packet: &Packet, expected: &Value) {
    assert_eq!(packet.id, expected["packet_id"].as_i64().unwrap() as i32);
    assert_eq!(
        packet.payload,
        hex(expected["payload_hex"].as_str().unwrap())
    );
    let mut writer = Writer::new();
    writer.varint(packet.id).raw(&packet.payload);
    assert_eq!(
        writer.into_inner(),
        hex(expected["full_packet_hex"].as_str().unwrap())
    );
}

#[test]
fn original_sound_and_level_event_packet_codecs_and_range_facts() {
    let fixture = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/item-effects-vectors.json"
    ))
    .unwrap();
    let fixture: Value = serde_json::from_str(&fixture).unwrap();
    let cases = fixture["sound_cases"].as_array().unwrap();
    assert!(!cases.is_empty());
    let sounds = fixture["sounds"].as_array().unwrap();
    assert_eq!(sounds.len(), 3);
    for (index, case) in cases.iter().enumerate() {
        assert_eq!(case["source_ordinal"], 8);
        let name = case["sound_name"].as_str().unwrap();
        let position = vector(&case["position_bits"]);
        let broadcast = sound_packet(
            sound(name),
            position,
            float(&case["volume_bits"]),
            float(&case["pitch_bits"]),
            case["seed"].as_str().unwrap().parse().unwrap(),
        );
        assert_packet(&broadcast.packet, &case["expected"]);
        assert_eq!(
            (broadcast.radius as f32).to_bits(),
            u32::from_str_radix(case["expected"]["range_bits"].as_str().unwrap(), 16).unwrap(),
            "original sound range {index}"
        );
        let mut reader = Reader::new(&broadcast.packet.payload);
        let id = reader.varint().unwrap() - 1;
        let fact = sounds.iter().find(|fact| fact["name"] == name).unwrap();
        assert_eq!(id, fact["id"].as_i64().unwrap() as i32);
        assert!(fact["fixed_range_bits"].is_null());
        assert_eq!(reader.varint().unwrap(), 8);
        for axis in 0..3 {
            assert_eq!(
                reader.i32().unwrap(),
                case["expected"]["quantized"][axis].as_i64().unwrap() as i32
            );
        }
        assert_eq!(broadcast.origin.x.to_bits(), position.x.to_bits());
        assert_eq!(broadcast.origin.y.to_bits(), position.y.to_bits());
        assert_eq!(broadcast.origin.z.to_bits(), position.z.to_bits());
    }
    let cases = fixture["level_event_cases"].as_array().unwrap();
    assert!(!cases.is_empty());
    for case in cases {
        let p = &case["position"];
        let packet = level_event_packet(
            case["event"].as_i64().unwrap() as i32,
            (
                p[0].as_i64().unwrap() as i32,
                p[1].as_i64().unwrap() as i32,
                p[2].as_i64().unwrap() as i32,
            ),
            case["data"].as_i64().unwrap() as i32,
            case["global"].as_bool().unwrap(),
        );
        assert_packet(&packet, &case["expected"]);
    }
}

#[test]
fn world_broadcast_boundaries_seed_contract_and_nonpacket_effects() {
    let position = Vec3::new(0.03125, 64., 0.);
    let broadcast = sound_packet(ItemSound::GenericSplash, position, 1., 1., 7);
    assert!(broadcast.in_range(Vec3::new(16.015625, 64., 0.), true));
    assert!(!broadcast.in_range(Vec3::new(16.03125, 64., 0.), true));
    assert!(!broadcast.in_range(position, false));
    let state = EntityPhysicalState::new(
        17,
        [0; 16],
        Position {
            x: position.x,
            y: position.y,
            z: position.z,
            yaw: 0.,
            pitch: 0.,
            on_ground: false,
        },
        Vec3::ZERO,
        0.25,
        0.25,
        0.2125,
    )
    .unwrap();
    let sound = RuntimeItemEffect::Sound {
        sound: ItemSound::GenericSplash,
        volume: 1.,
        pitch: 1.,
    };
    assert!(encode_effect(&state, sound, None).is_err());
    assert!(encode_effect(&state, sound, Some(7)).unwrap().is_some());
    for effect in [
        RuntimeItemEffect::HitGround { affected_block: 1 },
        RuntimeItemEffect::Splash,
        RuntimeItemEffect::Base(BaseEffect::DiscardBelowWorld),
    ] {
        assert!(encode_effect(&state, effect, None).unwrap().is_none());
        assert!(encode_effect(&state, effect, Some(0)).is_err());
    }
    let melt = encode_effect(
        &state,
        RuntimeItemEffect::Base(BaseEffect::MeltFrozen {
            position: (-2, 63, 8),
            event: 1009,
            data: 1,
        }),
        None,
    )
    .unwrap()
    .unwrap();
    assert_eq!(melt.origin, Vec3::new(-2., 63., 8.));
    assert_eq!(melt.radius, 64.);
    assert!(melt.in_range(Vec3::new(61.75, 63., 8.), true));
    assert!(!melt.in_range(Vec3::new(62., 63., 8.), true));
    assert!(encode_effect(
        &state,
        RuntimeItemEffect::Base(BaseEffect::StopRiding { vehicle: 9 }),
        None
    )
    .is_err());
    assert!(encode_effect(
        &state,
        RuntimeItemEffect::Base(BaseEffect::LavaBurnSound {
            volume: 0.4,
            pitch: 2.
        }),
        None
    )
    .is_err());
}
