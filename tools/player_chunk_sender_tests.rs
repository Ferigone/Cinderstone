use super::*;
use anyhow::bail;
use serde_json::{json, Value};
use std::collections::BTreeMap;
struct Host<'a> {
    center: ChunkPosition,
    alive: bool,
    ready: BTreeMap<ChunkPosition, ChunkPosition>,
    trace: Vec<Value>,
    fail: &'a str,
}
impl ChunkBatchHost for Host<'_> {
    type Chunk = ChunkPosition;
    fn player_chunk_position(&mut self) -> Result<ChunkPosition> {
        self.trace
            .push(json!(["center", self.center.0, self.center.1]));
        Ok(self.center)
    }
    fn ready_chunk(&mut self, p: ChunkPosition) -> Result<Option<ReadyChunk<Self::Chunk>>> {
        self.trace.push(json!(["ready", p.0, p.1]));
        if self.fail == "ready" {
            bail!("controlled ready failure")
        };
        Ok(self.ready.get(&p).map(|&actual| ReadyChunk {
            handle: actual,
            actual_position: actual,
        }))
    }
    fn begin_batch(&mut self) -> Result<()> {
        self.trace.push(json!(["begin"]));
        if self.fail == "begin" {
            bail!("controlled begin failure")
        };
        Ok(())
    }
    fn send_chunk(&mut self, c: ReadyChunk<Self::Chunk>) -> Result<()> {
        self.trace
            .push(json!(["chunk", c.actual_position.0, c.actual_position.1]));
        if self.fail == "chunk" {
            bail!("controlled chunk failure")
        };
        Ok(())
    }
    fn end_batch(&mut self, n: usize) -> Result<()> {
        self.trace.push(json!(["end", n]));
        if self.fail == "end" {
            bail!("controlled end failure")
        };
        Ok(())
    }
}
impl ChunkDropHost for Host<'_> {
    fn player_alive(&mut self) -> Result<bool> {
        self.trace.push(json!(["alive", self.alive]));
        Ok(self.alive)
    }
    fn unload_chunk(&mut self, p: ChunkPosition) -> Result<()> {
        self.trace.push(json!(["unload", p.0, p.1]));
        Ok(())
    }
}
fn position(v: &Value) -> ChunkPosition {
    (v[0].as_i64().unwrap() as i32, v[1].as_i64().unwrap() as i32)
}
fn f32_bits(v: &Value) -> f32 {
    f32::from_bits(u32::from_str_radix(v.as_str().unwrap(), 16).unwrap())
}
fn bits(v: f32) -> String {
    format!("{:08x}", if v.is_nan() { 0x7fc00000 } else { v.to_bits() })
}
fn state(sender: &PlayerChunkSender, host: &Host<'_>, selected: Vec<Value>, failed: bool) -> Value {
    json!({"h":bits(sender.desired_chunks_per_tick),"i":bits(sender.batch_quota),"j":sender.unacknowledged_batches,"k":sender.max_unacknowledged_batches,"pending":sender.pending_positions().map(|p|json!([p.0,p.1])).collect::<Vec<_>>(),"trace":host.trace,"selected":selected,"failed_transport":failed})
}
#[test]
fn original_arq_pending_ack_collect_drop_and_send_prefix() {
    let fixture: Value = serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/fixtures/player_chunk_sender.json"
        ))
        .unwrap(),
    )
    .unwrap();
    let mut states = 0;
    let mut ready_count = 0;
    let mut failures = 0;
    for case in fixture["cases"].as_array().unwrap() {
        let query = &case["query"];
        let mut sender = PlayerChunkSender::new(query["memory"].as_bool().unwrap());
        for (index, step) in query["steps"].as_array().unwrap().iter().enumerate() {
            let mut host = Host {
                center: step.get("center").map(position).unwrap_or((0, 0)),
                alive: step["alive"].as_bool().unwrap_or(true),
                ready: BTreeMap::new(),
                trace: Vec::new(),
                fail: step["fail"].as_str().unwrap_or("begin"),
            };
            if let Some(ready) = step["ready"].as_array() {
                for row in ready {
                    let a = row.as_array().unwrap();
                    host.ready.insert(
                        (a[0].as_i64().unwrap() as i32, a[1].as_i64().unwrap() as i32),
                        (a[2].as_i64().unwrap() as i32, a[3].as_i64().unwrap() as i32),
                    );
                }
            }
            let mut selected = vec![];
            let mut failed = false;
            match step["op"].as_str().unwrap() {
                "mark" => {
                    for p in step["positions"].as_array().unwrap() {
                        sender.mark_pending(position(p));
                    }
                }
                "ack" => sender.on_ack(f32_bits(&step["desired"])),
                "raw" => {
                    if step.get("h").is_some() {
                        sender.desired_chunks_per_tick = f32_bits(&step["h"]);
                    }
                    if step.get("i").is_some() {
                        sender.batch_quota = f32_bits(&step["i"]);
                    }
                    if let Some(n) = step["j"].as_i64() {
                        sender.unacknowledged_batches = n as i32;
                    }
                    if let Some(n) = step["k"].as_i64() {
                        sender.max_unacknowledged_batches = n as i32;
                    }
                }
                "drop" => sender
                    .drop_chunk(position(&step["position"]), &mut host)
                    .unwrap(),
                "collect" => {
                    selected = sender
                        .collect_chunks(host.center, &mut host)
                        .unwrap()
                        .into_iter()
                        .map(|c| json!([c.actual_position.0, c.actual_position.1]))
                        .collect()
                }
                "send" => {
                    failed = sender.send_next(&mut host).is_err();
                }
                op => panic!("unknown {op}"),
            }
            ready_count += host.trace.iter().filter(|t| t[0] == "ready").count();
            failures += usize::from(failed);
            states += 1;
            assert_eq!(
                state(&sender, &host, selected, failed),
                case["expected"]["steps"][index],
                "{} step{index} {}",
                case["name"],
                step["op"]
            );
        }
    }
    println!("actual arq: {states} states, {ready_count} readiness queries, {failures} transport-failure prefixes");
}
/// Additional host Result error scopes, including readiness errors before Begin.
/// Successful and transport-failure paths also have independent original proof.
#[test]
fn bytecode_derived_success_and_callback_error_scopes() {
    for fail in ["", "ready", "begin", "chunk", "end"] {
        let mut sender = PlayerChunkSender::new(false);
        sender.mark_pending((0, 0));
        let mut host = Host {
            center: (0, 0),
            alive: true,
            ready: BTreeMap::from([((0, 0), (0, 0))]),
            trace: vec![],
            fail,
        };
        let result = sender.send_next(&mut host);
        if fail.is_empty() {
            assert_eq!(result.unwrap(), 1);
            assert_eq!(sender.batch_quota(), 8.0);
            assert_eq!(
                host.trace,
                json!([
                    ["center", 0, 0],
                    ["ready", 0, 0],
                    ["begin"],
                    ["chunk", 0, 0],
                    ["end", 1]
                ])
                .as_array()
                .unwrap()
                .clone()
            );
        } else {
            assert!(result.is_err());
            assert_eq!(sender.batch_quota(), 9.0);
        }
        assert_eq!(sender.pending_count(), usize::from(fail == "ready"));
        assert_eq!(sender.unacknowledged_batches(), i32::from(fail != "ready"));
    }
}
