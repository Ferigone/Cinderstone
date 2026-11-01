use super::*;
use serde_json::{json, Value};
fn snapshot(random: &mut LegacyRandomSource, java_class: &str) -> Value {
    let int = random.next_i32();
    let long = random.next_i64();
    let float = random.next_f32();
    let double = random.next_f64();
    json!({"class":java_class,"int":int,"long":long,"float_bits":format!("{:08x}",float.to_bits()),"double_bits":format!("{:016x}",double.to_bits()),"state48_bits":format!("{:016x}",random.state48())})
}
#[test]
fn original_clock_boundary_seed_factory_and_level_sequence() {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/random-seed-vectors.json")).unwrap();
    let mut calls = 0;
    let mut streams = 0;
    let mut levels = 0;
    for (index, row) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let q = &row["query"];
        let expected = &row["expected"];
        let initial =
            u64::from_str_radix(q["initial_uniquifier_bits"].as_str().unwrap(), 16).unwrap();
        let direct = SeedUniquifier::from_state(initial);
        for (time, seed) in q["nano_times"]
            .as_array()
            .unwrap()
            .iter()
            .zip(expected["observed_seeds"].as_array().unwrap())
        {
            assert_eq!(
                direct.next_seed_at(time.as_i64().unwrap()),
                seed.as_i64().unwrap(),
                "actual original full64 seed case{index}"
            );
            calls += 1;
        }
        let uniquifier = SeedUniquifier::from_state(initial);
        let mut clock_events = Vec::new();
        let mut cursor = 0;
        let mut clock = || {
            let nanos = q["nano_times"][cursor].as_i64().unwrap();
            cursor += 1;
            clock_events.push(json!({"updated_uniquifier_bits":format!("{:016x}",uniquifier.state()),"nano_time":nanos}));
            Ok(nanos)
        };
        let mut operations = Vec::new();
        for operation in q["operations"].as_array().unwrap() {
            let name = operation.as_str().unwrap();
            let value = match name {
                "unique" => {
                    json!({"operation":name,"seed":uniquifier.next_seed_with_clock(&mut clock).unwrap()})
                }
                "create" | "thread_safe" => {
                    let mut random = create_random_with_clock(&uniquifier, &mut clock).unwrap();
                    streams += 1;
                    json!({"operation":name,"random":snapshot(&mut random,if name=="create"{"dyz"}else{"dzr"})})
                }
                "level" => {
                    let mut randoms =
                        create_level_randoms_with_clock(&uniquifier, &mut clock).unwrap();
                    levels += 1;
                    streams += 2;
                    json!({"operation":name,"rand_value":randoms.rand_value,"random":snapshot(&mut randoms.random,"dyz"),"thread_safe_random":snapshot(&mut randoms.thread_safe_random,"dzr"),"initial_seeds":randoms.initial_seeds})
                }
                _ => unreachable!(),
            };
            operations.push(value);
        }
        assert_eq!(
            operations,
            *expected["operations"].as_array().unwrap(),
            "actual factory/level state case{index}"
        );
        assert_eq!(
            clock_events,
            *expected["clock_callbacks"].as_array().unwrap(),
            "atomic update must precede clock case{index}"
        );
        assert_eq!(
            format!("{:016x}", uniquifier.state()),
            expected["final_uniquifier_bits"].as_str().unwrap()
        );
        assert_eq!(direct.state(), uniquifier.state());
        assert_eq!(cursor, q["nano_times"].as_array().unwrap().len());
    }
    assert_eq!((calls, streams, levels), (896, 512, 128));
    println!(
        "RANDOM_SEED_CONFORMANCE:{}",
        json!({"ok":true,"original_instrumented_queries":fixture["cases"].as_array().unwrap().len(),"exact_original_seed_returns":calls,"exact_original_sequential_rng_streams":streams,"exact_level_initializer_sequences":levels,"mismatches":0,"scope":"Actual original RandomSupport arithmetic with injected clock and seed-return observer, original factories/RNG samplers; Level initializer order from original bytecode; ThreadSafeLegacy sequential equivalence only."})
    );
}
#[test]
fn linux_monotonic_startup_factories_are_available() {
    let first = system_monotonic_nanos().unwrap();
    let second = system_monotonic_nanos().unwrap();
    assert!(first > 0 && second >= first);
    create_random().unwrap();
    create_level_randoms().unwrap();
}
