use super::*;
use crate::nbt::NbtRoot;
use base64::Engine;
use serde_json::{json, Value};

fn ids(values: &BTreeSet<RecipeId>) -> Value {
    serde_json::to_value(values).unwrap()
}
fn state(book: &ServerRecipeBook) -> Value {
    json!({"known":ids(book.known()),"highlight":ids(book.highlights()),"settings":book.settings().booleans()})
}
fn text(value: &Value) -> JavaString {
    if let Some(units) = value.get("utf16") {
        JavaString::from_utf16_lossless(
            units
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u16)
                .collect(),
        )
    } else {
        value.as_str().unwrap().into()
    }
}
fn id(value: &Value) -> Result<RecipeId> {
    RecipeId::from_java(&text(value))
}
fn descriptor(value: &Value) -> Result<RecipeDescriptor> {
    if value.get("template").is_some() {
        let template = VanillaRecipeCatalogue::get()
            .descriptor(&id(&value["template"])?)
            .context("oracle template missing")?;
        Ok(RecipeDescriptor {
            id: id(&value["id"])?,
            is_special: template.is_special,
        })
    } else {
        VanillaRecipeCatalogue::get()
            .descriptor(&id(value)?)
            .context("oracle holder missing")
    }
}
fn descriptors(values: &Value) -> Result<Vec<RecipeDescriptor>> {
    values.as_array().unwrap().iter().map(descriptor).collect()
}
fn settings(value: &Value) -> RecipeBookSettings {
    RecipeBookSettings::from_booleans(serde_json::from_value(value.clone()).unwrap())
}
fn base64_bytes(value: &Value) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD
        .decode(value.as_str().unwrap())
        .unwrap()
}
fn typed_nbt(value: &Value) -> Tag {
    let payload = &value["value"];
    let integer = || {
        payload
            .as_i64()
            .unwrap_or_else(|| payload.as_str().unwrap().parse().unwrap())
    };
    match value["type"].as_u64().unwrap() {
        1 => Tag::Byte(integer() as i8),
        2 => Tag::Short(integer() as i16),
        3 => Tag::Int(integer() as i32),
        4 => Tag::Long(integer()),
        5 => Tag::Float(f32::from_bits(
            u32::from_str_radix(value["bits"].as_str().unwrap(), 16).unwrap(),
        )),
        6 => Tag::Double(f64::from_bits(
            u64::from_str_radix(value["bits"].as_str().unwrap(), 16).unwrap(),
        )),
        7 => Tag::ByteArray(
            payload
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap() as i8)
                .collect(),
        ),
        8 => Tag::String(if value.get("utf16").is_some() {
            text(value)
        } else {
            text(payload)
        }),
        9 => {
            let values = payload
                .as_array()
                .unwrap()
                .iter()
                .map(typed_nbt)
                .collect::<Vec<_>>();
            Tag::List {
                element_type: values.first().map_or(0, Tag::tag_id),
                values,
            }
        }
        10 => Tag::Compound(
            payload
                .as_object()
                .unwrap()
                .iter()
                .map(|(name, v)| (name.as_str().into(), typed_nbt(v)))
                .collect(),
        ),
        11 => Tag::IntArray(
            payload
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap() as i32)
                .collect(),
        ),
        12 => Tag::LongArray(
            payload
                .as_array()
                .unwrap()
                .iter()
                .map(|v| {
                    v.as_i64()
                        .unwrap_or_else(|| v.as_str().unwrap().parse().unwrap())
                })
                .collect(),
        ),
        _ => panic!("unknown typed NBT"),
    }
}
fn logical_nbt(book: &ServerRecipeBook) -> Value {
    let tag = book.to_nbt();
    let settings = SETTING_NAMES
        .into_iter()
        .flatten()
        .map(|name| (name, nbt_boolean(tag.get(name))))
        .collect::<BTreeMap<_, _>>();
    json!({"settings":settings,"recipes":ids(book.known()),"toBeDisplayed":ids(book.highlights())})
}
#[derive(Default)]
struct Host {
    events: Vec<Value>,
    packets: Vec<(RecipeBookPacket, bool)>,
    saved: Vec<(RecipeBookPacket, bool)>,
    registered: Vec<(RecipeId, String)>,
    callback: Option<Value>,
    consumed: bool,
    issues: Vec<RecipeLoadIssue>,
    record_load_logs: bool,
}
impl Host {
    fn reset(&mut self, operation: &Value) {
        self.events.clear();
        self.packets.clear();
        self.callback = operation.get("callback").cloned();
        self.consumed = false;
    }
    fn event(&mut self, book: &ServerRecipeBook, callback: &str, extra: Value) {
        let mut value = state(book);
        value["callback"] = json!(callback);
        for (k, v) in extra.as_object().unwrap() {
            value[k] = v.clone();
        }
        self.events.push(value);
    }
    fn mutate(&mut self, book: &mut ServerRecipeBook, phase: &str) -> Result<()> {
        let Some(callback) = self.callback.clone() else {
            return Ok(());
        };
        if self.consumed || callback["phase"] != phase {
            return Ok(());
        }
        self.consumed = true;
        let mutation = callback["mutation"].as_str().unwrap();
        self.event(
            book,
            "mutation_begin",
            json!({"phase":phase,"mutation":mutation}),
        );
        match mutation {
            "remove" => {
                for value in callback["recipes"].as_array().unwrap() {
                    book.remove_id(&id(value)?);
                }
            }
            "highlight" => {
                for value in callback["recipes"].as_array().unwrap() {
                    book.add_highlight(id(value)?);
                }
            }
            "settings" => book.set_settings(&settings(&callback["settings"])),
            "reentrant_add" => {
                book.add_recipes(&descriptors(&callback["recipes"])?, self)?;
            }
            "throw" => bail!("controlled recipe callback failure"),
            _ => panic!("unknown controlled callback"),
        }
        self.event(book, "mutation_end", json!({"phase":phase}));
        Ok(())
    }
    fn emit(&mut self, book: &ServerRecipeBook, packet: RecipeBookPacket, ordered: bool) {
        self.event(book, "send", json!({"packet_index":self.saved.len()}));
        self.saved.push((packet.clone(), ordered));
        self.packets.push((packet, ordered));
    }
}
impl RecipeBookHost for Host {
    fn recipe_unlocked(
        &mut self,
        book: &mut ServerRecipeBook,
        recipe: &RecipeDescriptor,
    ) -> Result<()> {
        self.event(book, "recipe_unlocked", json!({"recipe":recipe.id}));
        self.mutate(book, "criterion")?;
        self.event(book, "advancements", json!({}));
        if !self.registered.is_empty() {
            for callback in [
                "level_context",
                "world_server",
                "world_random",
                "server_lookup",
            ] {
                self.event(book, callback, json!({}));
            }
            // Controlled outside-core criterion environment: optional player
            // predicate absent, one actual recipe-ID equality listener. The
            // real original dt/dv and outer award run in the Java oracle.
            for (id, name) in self.registered.clone() {
                if id == recipe.id {
                    self.event(book, "criterion_award", json!({"criterion":name}));
                    self.mutate(book, "award")?;
                }
            }
        }
        Ok(())
    }
    fn send_packet(&mut self, book: &mut ServerRecipeBook, packet: RecipeBookPacket) -> Result<()> {
        let ordered = packet.state != RecipePacketState::Init;
        self.emit(book, packet, ordered);
        Ok(())
    }
}
impl RecipeLoadLog for Host {
    fn issue(&mut self, book: &mut ServerRecipeBook, issue: RecipeLoadIssue) -> Result<()> {
        if self.record_load_logs {
            self.event(
                book,
                "load_log",
                json!({"message_template":issue.message_template(),"argument_utf16":issue.argument().units().as_ref()}),
            );
        }
        self.issues.push(issue);
        Ok(())
    }
}

#[test]
fn original_recipe_book_transitions_settings_packets_nbt_and_callbacks() {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/server-recipe-book-vectors.json")).unwrap();
    let mut snapshots = 0;
    let mut packets = 0;
    let mut exact_packets = 0;
    let mut exact_nbt = 0;
    let mut callbacks = 0;
    let mut errors = 0;
    let mut excluded = 0;
    let mut load_log_issues = 0;
    for (case_index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        if case["query"]["java_only"].as_bool().unwrap_or(false) {
            excluded += 1;
            continue;
        }
        let mut book = ServerRecipeBook::default();
        let mut external = RecipeBookSettings::default();
        let mut host = Host::default();
        assert_eq!(
            case["query"]["operations"].as_array().unwrap().len(),
            case["expected"]["states"].as_array().unwrap().len(),
            "every original operation must have an expected snapshot"
        );
        for (step, (op, expected)) in case["query"]["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(case["expected"]["states"].as_array().unwrap())
            .enumerate()
        {
            host.reset(op);
            let kind = op["op"].as_str().unwrap();
            let result = (|| -> Result<Value> {
                let mut value = Value::Null;
                match kind {
                    "add_recipes" => {
                        value = json!(book.add_recipes(&descriptors(&op["recipes"])?, &mut host)?)
                    }
                    "remove_recipes" => {
                        value =
                            json!(book.remove_recipes(&descriptors(&op["recipes"])?, &mut host)?)
                    }
                    "direct_add" => book.add_known(&descriptor(&op["recipe"])?),
                    "direct_remove" => book.remove_id(&descriptor(&op["recipe"])?.id),
                    "add_highlight" => book.add_highlight(descriptor(&op["recipe"])?.id),
                    "remove_highlight" => book.remove_highlight(&descriptor(&op["recipe"])?.id),
                    "contains" => {
                        value = json!(if op["recipe"].is_null() {
                            book.contains_holder(None)
                        } else {
                            book.contains_holder(Some(&descriptor(&op["recipe"])?))
                        })
                    }
                    "contains_id" => value = json!(book.contains(&id(&op["id"])?)),
                    "will_highlight" => {
                        value = json!(book.will_highlight(&descriptor(&op["recipe"])?.id))
                    }
                    "parse_id" => value = json!(id(&op["id"])?.as_str()),
                    "set_open" => book.set_open(
                        RecipeBookType::from_ordinal(op["type"].as_i64().unwrap() as i32)?,
                        op["value"].as_bool().unwrap(),
                    ),
                    "set_filter" => book.set_filtering(
                        RecipeBookType::from_ordinal(op["type"].as_i64().unwrap() as i32)?,
                        op["value"].as_bool().unwrap(),
                    ),
                    "set_settings" => book.set_settings(&settings(&op["settings"])),
                    "get_settings_copy" => external = book.settings(),
                    "external_set" => external = settings(&op["settings"]),
                    "set_external" => book.set_settings(&external),
                    "settings_wire_decode" => {
                        let bytes = base64_bytes(&op["wire"]);
                        let mut input = Reader::new(&bytes);
                        book.set_settings(&RecipeBookSettings::read_network(&mut input)?);
                        value = json!(input.remaining().len());
                    }
                    "load_nbt" => {
                        let mut lookup = VanillaRecipeCatalogue::get();
                        book.load_nbt(&typed_nbt(&op["nbt"]), &mut lookup, &mut host)?;
                    }
                    "copy_from" => {
                        let mut source = ServerRecipeBook::default();
                        for recipe in descriptors(&op["known"])? {
                            source.add_known(&recipe);
                        }
                        for recipe in descriptors(&op["highlight"])? {
                            source.add_highlight(recipe.id);
                        }
                        source.set_settings(&settings(&op["settings"]));
                        book.copy_from(&source);
                    }
                    "init" => book.send_initial(&mut host)?,
                    "register_criterion" => host
                        .registered
                        .push((id(&op["recipe"])?, op["name"].as_str().unwrap().into())),
                    "clear_criteria" => host.registered.clear(),
                    "packet_ordered" => {
                        let packet = RecipeBookPacket {
                            state: RecipePacketState::from_ordinal(
                                op["state"].as_i64().unwrap() as i32
                            )?,
                            recipes: op["recipes"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(id)
                                .collect::<Result<_>>()?,
                            highlights: op["highlights"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(id)
                                .collect::<Result<_>>()?,
                            settings: book.settings(),
                        };
                        host.emit(&book, packet, true);
                    }
                    "saved_packet" => host
                        .packets
                        .push(host.saved[op["index"].as_u64().unwrap() as usize].clone()),
                    "packet_decode" => {
                        let bytes = base64_bytes(&op["wire"]);
                        let mut input = Reader::new(&bytes);
                        host.packets
                            .push((RecipeBookPacket::read(&mut input)?, true));
                        value = json!(input.remaining().len());
                    }
                    _ => panic!("unknown oracle operation {kind}"),
                }
                Ok(value)
            })();
            assert_eq!(
                result.is_err(),
                !expected["error"].is_null(),
                "original acceptance case{case_index}step{step}op{kind}: {result:?}"
            );
            if let Ok(result) = result {
                assert_eq!(
                    result, expected["result"],
                    "original result case{case_index}step{step}"
                );
            } else {
                errors += 1;
            }
            let actual = state(&book);
            for name in ["known", "highlight", "settings"] {
                assert_eq!(
                    actual[name], expected[name],
                    "original {name} case{case_index}step{step}"
                );
            }
            assert_eq!(
                logical_nbt(&book),
                expected["nbt"],
                "logical original NBT case{case_index}step{step}"
            );
            assert_eq!(
                external.booleans().as_slice(),
                serde_json::from_value::<Vec<bool>>(expected["external_settings"].clone()).unwrap()
            );
            assert_eq!(
                host.events,
                *expected["events"].as_array().unwrap(),
                "literal callback state/order case{case_index}step{step}"
            );
            callbacks += host.events.len();
            assert_eq!(
                host.packets.len(),
                expected["packets"].as_array().unwrap().len()
            );
            for ((packet, ordered), original) in host
                .packets
                .iter()
                .zip(expected["packets"].as_array().unwrap())
            {
                assert_eq!(
                    packet.state as i32,
                    original["state"].as_i64().unwrap() as i32
                );
                assert_eq!(
                    packet.settings.booleans().to_vec(),
                    serde_json::from_value::<Vec<bool>>(original["settings"].clone()).unwrap()
                );
                let mut actual_recipes = serde_json::to_value(&packet.recipes).unwrap();
                let mut actual_highlights = serde_json::to_value(&packet.highlights).unwrap();
                let mut expected_recipes = original["recipes"].clone();
                let mut expected_highlights = original["highlights"].clone();
                if !ordered {
                    for value in [
                        &mut actual_recipes,
                        &mut actual_highlights,
                        &mut expected_recipes,
                        &mut expected_highlights,
                    ] {
                        value
                            .as_array_mut()
                            .unwrap()
                            .sort_by(|a, b| a.as_str().unwrap().cmp(b.as_str().unwrap()));
                    }
                }
                assert_eq!(
                    actual_recipes, expected_recipes,
                    "original packet recipe list case{case_index}step{step}"
                );
                assert_eq!(
                    actual_highlights, expected_highlights,
                    "original packet highlight list"
                );
                if *ordered || (packet.recipes.len() <= 1 && packet.highlights.len() <= 1) {
                    assert_eq!(
                        packet.encode().unwrap().payload,
                        base64_bytes(&original["wire"]),
                        "literal original packet case{case_index}step{step}"
                    );
                    exact_packets += 1;
                }
                packets += 1;
            }
            if book.known.len() <= 1 && book.highlight.len() <= 1 {
                let bytes = NbtRoot {
                    name: "".into(),
                    tag: book.to_nbt(),
                }
                .encode()
                .unwrap();
                assert_eq!(
                    bytes,
                    base64_bytes(&expected["nbt_wire"]),
                    "literal original NBT case{case_index}step{step}"
                );
                exact_nbt += 1;
            }
            assert_eq!(expected["world_rng_before"], expected["world_rng_after"]);
            snapshots += 1;
        }
        load_log_issues += host.issues.len();
    }
    println!(
        "SERVER_RECIPE_BOOK_CONFORMANCE:{}",
        json!({"ok":true,"original_histories":fixture["cases"].as_array().unwrap().len(),"native_snapshots":snapshots,"packet_snapshots":packets,"exact_ordered_or_singleton_packet_bytes":exact_packets,"exact_zero_or_singleton_set_nbt_bytes":exact_nbt,"callback_observations":callbacks,"controlled_or_invalid_input_errors":errors,"java_only_self_alias_histories_excluded":excluded,"native_load_log_hook_issues":load_log_issues,"mismatches":0,"scope":"Original per-player recipe-book state/settings/packet/NBT transitions; actual synchronous original criterion gates and controlled outer award/reentrant callbacks. INIT multi-ID sets and NBT multi-ID list order compare logically. Ordered ADD/REMOVE/direct packet collections compare bytes. Default advancement registration and dynamic datapack loading remain separate."})
    );
}

#[test]
fn original_recipe_packet_malformed_varints_utf_and_collection_edges() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../fixtures/server-recipe-book-codec-edges.json"
    ))
    .unwrap();
    let mut accepted = 0;
    let mut rejected = 0;
    for case in fixture["cases"].as_array().unwrap() {
        let query = &case["query"];
        let expected = &case["expected"]["states"][0];
        let bytes = base64_bytes(&query["operations"][0]["wire"]);
        let mut input = Reader::new(&bytes);
        let result = RecipeBookPacket::read(&mut input);
        assert_eq!(
            result.is_err(),
            !expected["error"].is_null(),
            "original codec acceptance {}: {result:?}",
            query["name"]
        );
        match result {
            Ok(packet) => {
                accepted += 1;
                let original = &expected["packets"][0];
                assert_eq!(
                    input.remaining().len(),
                    expected["result"].as_u64().unwrap() as usize
                );
                assert_eq!(
                    packet.state as i32,
                    original["state"].as_i64().unwrap() as i32
                );
                assert_eq!(
                    serde_json::to_value(&packet.recipes).unwrap(),
                    original["recipes"]
                );
                assert_eq!(
                    serde_json::to_value(&packet.highlights).unwrap(),
                    original["highlights"]
                );
                assert_eq!(
                    serde_json::to_value(packet.settings.booleans()).unwrap(),
                    original["settings"]
                );
                assert_eq!(
                    packet.encode().unwrap().payload,
                    base64_bytes(&original["wire"])
                );
            }
            Err(_) => rejected += 1,
        }
    }
    assert_eq!((accepted, rejected), (14, 17));
    println!(
        "SERVER_RECIPE_BOOK_CODEC_CONFORMANCE:{}",
        json!({"ok":true,"cases":accepted+rejected,"accepted":accepted,"rejected":rejected,"accepted_exact_reencoded_packets":accepted,"mismatches":0,"scope":"Actual original packet codec acceptance and accepted decoded values/remaining bytes/reencoded bytes; native errors are Result errors, not Java exception class emulation."})
    );
}

#[test]
fn original_vanilla_catalogue_and_natural_login_declarations() {
    let facts: Value =
        serde_json::from_str(include_str!("../fixtures/server-recipe-book-facts.json")).unwrap();
    let decoded: Value = serde_json::from_str(include_str!(
        "../fixtures/server-recipe-book-declarations-decoded.json"
    ))
    .unwrap();
    let capture: Value = serde_json::from_str(include_str!(
        "../fixtures/server-recipe-book-natural-declarations.json"
    ))
    .unwrap();
    let catalogue = VanillaRecipeCatalogue::get();
    assert_eq!(catalogue.len(), 1290);
    assert_eq!(
        catalogue.iter().filter(|recipe| recipe.is_special).count(),
        14
    );
    for original in facts["recipes"].as_array().unwrap() {
        let native = catalogue.descriptor(&id(&original["id"]).unwrap()).unwrap();
        assert_eq!(native.is_special, original["is_special"].as_bool().unwrap());
    }
    let declaration = catalogue.declarations_packet();
    assert_eq!(declaration.id, 119);
    assert_eq!(declaration.payload, base64_bytes(&capture["payload"]));
    assert_eq!(declaration.payload.len(), 108270);
    assert_eq!(decoded["natural"]["roundtrip_equal"], true);
    assert_eq!(decoded["natural"]["remaining_bytes"], 0);
    assert_eq!(decoded["natural"]["decoded_count"], 1290);
    let mut input = Reader::new(&declaration.payload);
    assert_eq!(java_varint(&mut input).unwrap(), 1290);
    println!(
        "SERVER_RECIPE_BOOK_CATALOGUE_CONFORMANCE:{}",
        json!({"ok":true,"default_recipes":catalogue.len(),"special_recipes":14,"natural_declaration_packet_id":declaration.id,"natural_declaration_payload_bytes":declaration.payload.len(),"mismatches":0,"scope":"Pinned original vanilla data and static normal-login declaration packet; actual original agc/czb STREAM roundtrip is independently rerun by the original checker. Native dynamic datapack recipe serialization is not implemented."})
    );
}

#[test]
fn original_recipe_load_warning_arguments_and_synchronous_state() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../fixtures/server-recipe-book-load-logs.json"
    ))
    .unwrap();
    let mut snapshots = 0;
    let mut warning_callbacks = 0;
    for case in fixture["cases"].as_array().unwrap() {
        let mut book = ServerRecipeBook::default();
        let mut host = Host {
            record_load_logs: true,
            ..Host::default()
        };
        let operations = case["query"]["operations"].as_array().unwrap();
        let states = case["expected"]["states"].as_array().unwrap();
        assert_eq!(operations.len(), states.len());
        for (op, expected) in operations.iter().zip(states) {
            host.reset(op);
            match op["op"].as_str().unwrap() {
                "direct_add" => book.add_known(&descriptor(&op["recipe"]).unwrap()),
                "add_highlight" => book.add_highlight(descriptor(&op["recipe"]).unwrap().id),
                "set_settings" => book.set_settings(&settings(&op["settings"])),
                "load_nbt" => {
                    let mut lookup = VanillaRecipeCatalogue::get();
                    book.load_nbt(&typed_nbt(&op["nbt"]), &mut lookup, &mut host)
                        .unwrap();
                }
                "init" => book.send_initial(&mut host).unwrap(),
                operation => panic!("unknown original log operation {operation}"),
            }
            assert!(expected["error"].is_null());
            for name in ["known", "highlight", "settings"] {
                assert_eq!(state(&book)[name], expected[name]);
            }
            assert_eq!(logical_nbt(&book), expected["nbt"]);
            let mut original_events = expected["events"].clone();
            for event in original_events.as_array_mut().unwrap() {
                // The original stdout encoder replaces isolated surrogates;
                // compare its separately captured authoritative UTF-16 units.
                event.as_object_mut().unwrap().remove("argument");
            }
            assert_eq!(serde_json::to_value(&host.events).unwrap(), original_events);
            assert_eq!(
                host.packets.len(),
                expected["packets"].as_array().unwrap().len()
            );
            for ((packet, _), original) in host
                .packets
                .iter()
                .zip(expected["packets"].as_array().unwrap())
            {
                assert_eq!(
                    packet.state as i32,
                    original["state"].as_i64().unwrap() as i32
                );
                assert_eq!(
                    serde_json::to_value(packet.settings.booleans()).unwrap(),
                    original["settings"]
                );
                for (actual, field) in [
                    (&packet.recipes, "recipes"),
                    (&packet.highlights, "highlights"),
                ] {
                    let mut actual = serde_json::to_value(actual).unwrap();
                    let mut original = original[field].clone();
                    for list in [&mut actual, &mut original] {
                        list.as_array_mut()
                            .unwrap()
                            .sort_by(|a, b| a.as_str().unwrap().cmp(b.as_str().unwrap()));
                    }
                    assert_eq!(actual, original);
                }
            }
            snapshots += 1;
        }
        warning_callbacks += host.issues.len();
    }
    assert_eq!(warning_callbacks, 19);
    println!(
        "SERVER_RECIPE_BOOK_LOAD_LOG_CONFORMANCE:{}",
        json!({"ok":true,"histories":fixture["cases"].as_array().unwrap().len(),"snapshots":snapshots,"synchronous_warning_callbacks":warning_callbacks,"mismatches":0,"scope":"Original logger-boundary message templates and lossless UTF-16 arguments at exact known/highlight/settings mutation points. Native host logging policy remains explicit."})
    );
}
