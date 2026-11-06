//! The registry loader and network codecs against the original.
//!
//! * `fixtures/configuration.json`: the CONFIGURATION packets the unchanged vanilla 1.21 server sent
//!   to a client that knows no packs. Loading the vanilla registry JSON
//!   (`fixtures/datapack-vanilla-registries-1.21.json`) must rebuild every `registry_data` payload byte
//!   for byte (enchantments excepted: their element codec is not ported).
//! * `fixtures/datapack-registry-vectors.json`: one-step mutants of every vanilla element, decoded and
//!   re-encoded by the original's codecs (`tools/datapack_registries_generate.py`).

use super::registries::{
    encode_element, load_registry, registry_data_payload, KnownIds, LoadedElement, PackRegistryFiles, RegistryError, SYNCHRONIZED,
};
use serde_json::Value;
use std::collections::BTreeMap;

fn vanilla() -> Value {
    serde_json::from_str(include_str!("../../fixtures/datapack-vanilla-registries-1.21.json")).unwrap()
}

pub(crate) fn known_ids(doc: &Value) -> KnownIds {
    let set = |key: &str| doc["known"][key].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_owned()).collect();
    KnownIds {
        items: set("item"),
        sound_events: set("sound_event"),
        biomes: set("worldgen/biome"),
        armor_materials: set("armor_material"),
        particle_types: set("particle_type"),
        simple_particle_types: set("simple_particle_type"),
    }
}

fn vanilla_pack(doc: &Value, registry: &str) -> PackRegistryFiles {
    PackRegistryFiles {
        pack_id: "vanilla".to_owned(),
        files: doc["registries"][registry]
            .as_object()
            .unwrap()
            .iter()
            .map(|(id, text)| (id.clone(), text.as_str().unwrap().to_owned()))
            .collect::<BTreeMap<_, _>>(),
    }
}

fn captured_payloads() -> BTreeMap<String, Vec<u8>> {
    use base64::Engine;
    let doc: Value = serde_json::from_str(include_str!("../../fixtures/configuration.json")).unwrap();
    let mut out = BTreeMap::new();
    for packet in doc["packets"].as_array().unwrap() {
        if packet["id"].as_i64() == Some(7) {
            let bytes = base64::engine::general_purpose::STANDARD.decode(packet["payload"].as_str().unwrap()).unwrap();
            let length = bytes[0] as usize;
            let name = String::from_utf8(bytes[1..1 + length].to_vec()).unwrap();
            out.insert(name, bytes);
        }
    }
    out
}

#[test]
fn vanilla_registry_data_matches_the_captured_packets() {
    let doc = vanilla();
    let known = known_ids(&doc);
    let captured = captured_payloads();
    assert_eq!(captured.len(), SYNCHRONIZED.len());
    for registry in SYNCHRONIZED {
        let elements: Vec<LoadedElement> = load_registry(&[vanilla_pack(&doc, registry)]).expect("vanilla loads");
        let wanted = &captured[&format!("minecraft:{registry}")];
        if registry == "enchantment" {
            // Order and ids still match; the element data is not ported.
            let payload = registry_data_payload(registry, &elements, &known, |_| true).unwrap();
            let ids_only = String::from_utf8_lossy(&payload).to_string();
            for element in &elements {
                assert!(ids_only.contains(&element.id));
            }
            assert!(matches!(encode_element(registry, &elements[0].json, &known), Err(RegistryError::Unsupported(_))));
            continue;
        }
        let payload = registry_data_payload(registry, &elements, &known, |_| false).unwrap();
        assert!(payload == *wanted, "{registry}: rebuilt payload differs from the capture ({} vs {} bytes)", payload.len(), wanted.len());
    }
}

#[test]
fn a_later_pack_replaces_an_element_and_order_is_path_first() {
    let mut low = PackRegistryFiles { pack_id: "vanilla".into(), files: BTreeMap::new() };
    low.files.insert("minecraft:b".into(), r#"{"asset_id":"minecraft:b","translation_key":"x"}"#.into());
    low.files.insert("zz:a".into(), r#"{"asset_id":"zz:a","translation_key":"y"}"#.into());
    let mut high = PackRegistryFiles { pack_id: "file/custom".into(), files: BTreeMap::new() };
    high.files.insert("minecraft:b".into(), r#"{"asset_id":"minecraft:b","translation_key":"z"}"#.into());
    let loaded = load_registry(&[low, high]).unwrap();
    let ids: Vec<(&str, &str)> = loaded.iter().map(|e| (e.id.as_str(), e.pack_id.as_str())).collect();
    assert_eq!(ids, vec![("zz:a", "vanilla"), ("minecraft:b", "file/custom")]);
    let mut broken = PackRegistryFiles { pack_id: "p".into(), files: BTreeMap::new() };
    broken.files.insert("a:b".into(), "{".into());
    assert!(load_registry(&[broken]).is_err());
}

fn apply_patch(base: &Value, patch: &Value) -> Option<String> {
    if let Some(text) = patch.get("text") {
        return Some(text.as_str().unwrap().to_owned());
    }
    let mut value = base.clone();
    let path = patch["path"].as_array().unwrap();
    let mut parent = &mut value;
    for step in &path[..path.len() - 1] {
        parent = match step {
            Value::String(key) => parent.get_mut(key.as_str()).unwrap(),
            Value::Number(index) => parent.get_mut(index.as_u64().unwrap() as usize).unwrap(),
            _ => unreachable!(),
        };
    }
    let last = &path[path.len() - 1];
    if patch.get("delete").is_some() {
        parent.as_object_mut().unwrap().remove(last.as_str().unwrap());
    } else {
        match last {
            Value::String(key) => {
                parent.as_object_mut().unwrap().insert(key.clone(), patch["set"].clone());
            }
            Value::Number(index) => parent[index.as_u64().unwrap() as usize] = patch["set"].clone(),
            _ => unreachable!(),
        }
    }
    Some(serde_json::to_string(&value).unwrap())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Every vector of the fixture (or of `DATAPACK_REGISTRY_VECTORS`, a full local run): the port
/// must accept exactly what the original accepts and write the same network NBT. Codecs the port
/// does not cover answer `Unsupported`, which is counted and bounded rather than guessed.
#[test]
fn mutated_elements_match_the_original_codecs() {
    let doc = vanilla();
    let known = known_ids(&doc);
    let text = match std::env::var("DATAPACK_REGISTRY_VECTORS") {
        Ok(path) => std::fs::read_to_string(path).unwrap(),
        Err(_) => include_str!("../../fixtures/datapack-registry-vectors.json").to_owned(),
    };
    let vectors: Value = serde_json::from_str(&text).unwrap();
    let mut failures = Vec::new();
    let mut unsupported = 0usize;
    let rows = vectors["vectors"].as_array().unwrap();
    for (index, row) in rows.iter().enumerate() {
        let registry = row["registry"].as_str().unwrap();
        let id = row["id"].as_str().unwrap();
        let base: Value = serde_json::from_str(doc["registries"][registry][id].as_str().unwrap()).unwrap();
        let text = apply_patch(&base, &row["patch"]).unwrap();
        let ours = match crate::components::lenient::parse(&text) {
            Ok(json) => encode_element(registry, &json, &known).map(|tag| hex(&tag.encode_network_anonymous().unwrap())),
            Err(error) => Err(RegistryError::Codec(error)),
        };
        let expect = &row["expect"];
        match (&ours, expect["ok"].as_bool().unwrap()) {
            (Err(RegistryError::Unsupported(_)), _) => unsupported += 1,
            (Ok(nbt), true) if nbt == expect["nbt"].as_str().unwrap() => {}
            (Err(RegistryError::Codec(_)), false) => {}
            _ => failures.push(format!("#{index} {registry} {id} {}: ours {ours:?}, original {expect}", row["patch"])),
        }
    }
    eprintln!("registry vectors: {} checked, {unsupported} unsupported", rows.len());
    assert!(failures.is_empty(), "{} of {} vectors differ:\n{}", failures.len(), rows.len(), failures[..failures.len().min(25)].join("\n"));
    assert!(unsupported * 100 <= rows.len(), "{unsupported} of {} vectors hit an unported codec", rows.len());
}
