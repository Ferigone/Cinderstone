//! Advancement definitions against the original's `Advancement.CODEC`.

use super::definition::{parse, DefinitionError};
use serde_json::Value;

/// Every shipped advancement parses.
#[test]
fn vanilla_advancements_parse() {
    let doc: Value = serde_json::from_str(include_str!("../../fixtures/advancements-vanilla-1.21.json")).unwrap();
    let files = doc["advancement"].as_object().unwrap();
    assert_eq!(files.len(), 1399);
    let mut failures = Vec::new();
    for (id, text) in files {
        let json = crate::components::lenient::parse(text.as_str().unwrap()).unwrap();
        if let Err(error) = parse(&json) {
            failures.push(format!("{id}: {error}"));
        }
    }
    assert!(failures.is_empty(), "{} vanilla advancements fail:\n{}", failures.len(), failures[..failures.len().min(20)].join("\n"));
}

/// One-step mutants (or `ADVANCEMENT_PARSE_VECTORS`, a full local run): accepted and rejected
/// exactly as the original does; parts this port does not read answer `Unsupported`, counted and
/// bounded.
#[test]
fn advancement_mutants_match_the_original_codec() {
    let text = match std::env::var("ADVANCEMENT_PARSE_VECTORS") {
        Ok(path) => std::fs::read_to_string(path).unwrap(),
        Err(_) => include_str!("../../fixtures/advancement-parse-vectors.json").to_owned(),
    };
    let doc: Value = serde_json::from_str(&text).unwrap();
    let cases = doc["cases"].as_array().unwrap();
    let mut failures = Vec::new();
    let mut unsupported = 0usize;
    for case in cases {
        let input = case[0].as_str().unwrap();
        let expected = case[1].as_bool().unwrap();
        let json = crate::components::lenient::parse(input).unwrap();
        match (parse(&json), expected) {
            (Err(DefinitionError::Unsupported(_)), _) => unsupported += 1,
            (Ok(_), true) | (Err(DefinitionError::Codec(_)), false) => {}
            (ours, _) => failures.push(format!("{input}: ours {ours:?}, original accepts: {expected}")),
        }
    }
    eprintln!("advancement mutants: {} checked, {unsupported} unsupported", cases.len());
    if let Ok(path) = std::env::var("ADVANCEMENT_FAILURES") {
        std::fs::write(path, failures.join("\n")).unwrap();
    }
    assert!(failures.is_empty(), "{} of {} differ:\n{}", failures.len(), cases.len(), failures[..failures.len().min(20)].join("\n"));
    assert!(unsupported * 50 <= cases.len(), "{unsupported} of {} hit unported parts", cases.len());
}

/// The vanilla tree, its layout and the Update Advancements packet against the original's loaded
/// tree (`fixtures/advancement-tree-1.21.json`). The original's child order (identity hash) is
/// taken from the fixture; with it the positions and the packet bytes must match exactly.
#[test]
fn vanilla_tree_layout_and_packet_match_the_original() {
    use super::definition::Advancement;
    use super::progress::AdvancementProgress;
    use super::tree::{layout, update_advancements_packet, AdvancementTree, ProgressEntry};
    use std::collections::{BTreeMap, BTreeSet};

    let vanilla: Value = serde_json::from_str(include_str!("../../fixtures/advancements-vanilla-1.21.json")).unwrap();
    let oracle: Value = serde_json::from_str(include_str!("../../fixtures/advancement-tree-1.21.json")).unwrap();
    let advancements: BTreeMap<String, Advancement> = vanilla["advancement"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(id, text)| (id.clone(), parse(&crate::components::lenient::parse(text.as_str().unwrap()).unwrap()).unwrap()))
        .collect();

    let mut tree = AdvancementTree::new();
    let left = tree.add_all(advancements.iter().map(|(id, a)| (id.as_str(), a)));
    assert!(left.is_empty());
    let nodes = oracle["nodes"].as_object().unwrap();
    assert_eq!(tree.len(), nodes.len());
    for (id, node) in nodes {
        assert_eq!(tree.parent(id), node.get("parent").and_then(Value::as_str), "{id}");
        let ours: BTreeSet<&str> = tree.children(id).iter().map(String::as_str).collect();
        let theirs: BTreeSet<&str> = node["children"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
        assert_eq!(ours, theirs, "{id}");
    }
    let roots: BTreeSet<&str> = tree.roots().iter().map(String::as_str).collect();
    let oracle_roots: BTreeSet<&str> = oracle["roots"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    assert_eq!(roots, oracle_roots);

    // Layout of every visible root with the original's child order.
    let has_display = |id: &str| advancements[id].display.is_some();
    let children_of = |id: &str| -> Vec<String> {
        nodes[id]["children"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_owned()).collect()
    };
    let mut positions: BTreeMap<String, (f32, f32)> = BTreeMap::new();
    for root in tree.roots() {
        if has_display(root) {
            positions.extend(layout(root, &has_display, &children_of).unwrap());
        }
    }
    let mut checked = 0;
    for (id, node) in nodes {
        if let (Some(x), Some(y)) = (node.get("x").and_then(Value::as_f64), node.get("y").and_then(Value::as_f64)) {
            let (ox, oy) = positions.get(id).copied().unwrap_or((0.0, 0.0));
            assert_eq!((ox.to_bits(), oy.to_bits()), ((x as f32).to_bits(), (y as f32).to_bits()), "{id}: ours ({ox}, {oy}), original ({x}, {y})");
            checked += 1;
        }
    }
    assert_eq!(checked, 122);

    // Visibility for each set of done advancements.
    let hidden = |id: &str| advancements[id].display.as_ref().is_some_and(|d| d.hidden);
    for case in oracle["visibility"].as_array().unwrap() {
        let done: BTreeSet<&str> = case["done"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
        let mut visible = BTreeSet::new();
        for root in tree.roots() {
            for (id, shown) in super::tree::visibility(&tree, root, &|id| done.contains(id), &has_display, &hidden) {
                if shown {
                    visible.insert(id);
                }
            }
        }
        let expected: BTreeSet<String> = case["visible"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_owned()).collect();
        assert_eq!(visible, expected, "visibility with {} done", done.len());
    }

    // The packet: every holder sorted by id, one removed id, one progress entry.
    let spec = &oracle["progress_spec"];
    let target = spec["advancement"].as_str().unwrap();
    let mut progress = AdvancementProgress::from_json(&spec["progress"]).unwrap();
    progress.update(advancements[target].requirements.clone());
    let granted: Vec<&str> = spec["granted_order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    let added: Vec<(&str, &Advancement, (f32, f32))> = advancements
        .iter()
        .map(|(id, a)| (id.as_str(), a, positions.get(id).copied().unwrap_or((0.0, 0.0))))
        .collect();
    let bytes = update_advancements_packet(
        true,
        &added,
        &[spec["removed"].as_str().unwrap()],
        &[ProgressEntry { id: target, progress: &progress, granted_order: granted }],
    )
    .unwrap();
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let expected = oracle["packet"].as_str().unwrap();
    if hex != expected {
        let at = hex.bytes().zip(expected.bytes()).position(|(a, b)| a != b).unwrap_or(hex.len().min(expected.len()));
        panic!("packet differs at byte {} (ours {} bytes, original {}): ours …{}… original …{}…", at / 2, hex.len() / 2, expected.len() / 2,
            &hex[at.saturating_sub(40)..(at + 40).min(hex.len())], &expected[at.saturating_sub(40)..(at + 40).min(expected.len())]);
    }
}
