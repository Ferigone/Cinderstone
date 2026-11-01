use super::*;
use serde_json::{json, Value};

#[derive(Clone)]
struct FixtureHolder {
    reference: HolderRef,
    allowed: Option<ChunkStatus>,
    requests: BTreeMap<u8, u64>,
}
struct FixtureChunk {
    kind: ChunkKind,
    saved: bool,
    ticks: Vec<(i64, i64)>,
}
struct Host {
    holders: BTreeMap<Chunk, FixtureHolder>,
    sources: BTreeMap<u64, SourceSpec>,
    chunks: BTreeMap<ChunkDataId, FixtureChunk>,
    events: Vec<Value>,
    jobs: BTreeMap<FutureId, u64>,
    time: i64,
    time_fault: bool,
}
impl ReadinessHost for Host {
    fn lookup_holder(&mut self, chunk: Chunk) -> Option<HolderRef> {
        self.events
            .push(json!({"kind":"lookup","x":chunk.0,"z":chunk.1}));
        self.holders.get(&chunk).map(|h| h.reference)
    }
    fn get_or_schedule(
        &mut self,
        holder: HolderRef,
        status: ChunkStatus,
    ) -> Result<SourceSpec, ReadinessError> {
        self.events.push(
            json!({"kind":"request","x":holder.chunk.0,"z":holder.chunk.1,"status":status.index()}),
        );
        let h = &self.holders[&holder.chunk];
        if !can_request(h.allowed, status) {
            return Ok(SourceSpec {
                identity: 0,
                initial: ChunkOutcome::Failure("Unloaded chunk".into()),
            });
        }
        Ok(if let Some(source) = h.requests.get(&status.index()) {
            self.sources[source].clone()
        } else {
            SourceSpec {
                identity: 1_000_000 + holder.id * 12 + u64::from(status.index()),
                initial: ChunkOutcome::Pending,
            }
        })
    }
    fn post_process_generation(&mut self, chunk: ChunkDataId) -> Result<(), ReadinessError> {
        // The original producer executes the real method on this exact empty
        // PostProcessing/pending-NBT/UpgradeData.EMPTY fixture domain.
        assert_eq!(self.chunks[&chunk].kind, ChunkKind::Full);
        self.events
            .push(json!({"kind":"postprocess","chunk":chunk}));
        Ok(())
    }
    fn start_ticking_chunk(&mut self, chunk: ChunkDataId) -> Result<(), ReadinessError> {
        self.events.push(json!({"kind":"unpack","chunk":chunk}));
        if self.time_fault {
            return Err(ReadinessError::new(
                "java.lang.IllegalStateException",
                "oracle game time unavailable",
            ));
        }
        self.events
            .push(json!({"kind":"game_time","value":self.time.to_string()}));
        let c = self.chunks.get_mut(&chunk).unwrap();
        if c.saved {
            c.ticks = vec![
                (self.time.wrapping_add(5), -2),
                (self.time.wrapping_sub(1), -1),
            ];
            c.ticks.sort();
            c.saved = false;
        }
        Ok(())
    }
    fn send_chunk(&mut self, chunk: ChunkDataId) -> Result<(), ReadinessError> {
        self.events.push(json!({"kind":"send","chunk":chunk}));
        Ok(())
    }
    fn observe_effect(&mut self, effect: &ReadinessEffect) {
        let queue = |q: &Queue| {
            if matches!(q, Queue::Main) {
                "main"
            } else {
                "priority"
            }
        };
        self.events.push(match effect{
            ReadinessEffect::QueueTask{task,queue:q}=>json!({"kind":"enqueue","queue":queue(q),"task":task}),
            ReadinessEffect::ExecutorRejected{task,queue:q}=>json!({"kind":"reject","queue":queue(q),"task":task}),
            ReadinessEffect::PriorityAcknowledged{task}=>json!({"kind":"priority_ack","task":task}),
            ReadinessEffect::FutureCompleted{observer,future,generated_counter,..}=>json!({"kind":"complete","observer":observer,"job":self.jobs[future],"counter":generated_counter}),
        });
    }
}
fn mode(s: &str) -> ExecutorMode {
    match s {
        "queued" => ExecutorMode::Queued,
        "inline" => ExecutorMode::Inline,
        "reject" => ExecutorMode::Reject,
        _ => panic!("unknown mode"),
    }
}
fn kind(s: &str) -> ReadinessKind {
    match s {
        "accessible" => ReadinessKind::Accessible,
        "block" => ReadinessKind::BlockTicking,
        "entity" => ReadinessKind::EntityTicking,
        _ => panic!("unknown kind"),
    }
}
fn outcome(spec: &Value, chunks: &BTreeMap<ChunkDataId, FixtureChunk>) -> ChunkOutcome {
    match spec["state"].as_str().unwrap() {
        "pending" => ChunkOutcome::Pending,
        "success" => {
            let id = spec["chunk"].as_u64().unwrap();
            ChunkOutcome::Success(ChunkData {
                id,
                kind: chunks[&id].kind,
            })
        }
        "failure" => ChunkOutcome::Failure(
            spec.get("reason")
                .and_then(Value::as_str)
                .unwrap_or("oracle unavailable")
                .into(),
        ),
        "exceptional" => ChunkOutcome::Exceptional(ReadinessError::new(
            "java.lang.IllegalStateException",
            "oracle future failure",
        )),
        "cancelled" => ChunkOutcome::Cancelled,
        _ => panic!("unknown outcome"),
    }
}
fn chunk_result(value: Option<ChunkOutcome>) -> Value {
    match value {
        None | Some(ChunkOutcome::Pending) => json!({"state":"pending"}),
        Some(ChunkOutcome::Success(data)) => json!({"state":"success","chunk":data.id}),
        Some(ChunkOutcome::Failure(reason)) => json!({"state":"failure","reason":reason}),
        Some(ChunkOutcome::Exceptional(error)) => {
            json!({"state":"exceptional","error_class":error.class})
        }
        Some(ChunkOutcome::Cancelled) => json!({"state":"cancelled"}),
    }
}
fn range_result(value: Option<RangeOutcome>) -> Value {
    match value {
        None => json!({"state":"pending"}),
        Some(RangeOutcome::Success(data)) => {
            json!({"state":"success","chunks":data.into_iter().map(|d|d.id).collect::<Vec<_>>()})
        }
        Some(RangeOutcome::Failure(reason)) => json!({"state":"failure","reason":reason}),
        Some(RangeOutcome::Exceptional(error)) => {
            json!({"state":"exceptional","error_class":error.class})
        }
        Some(RangeOutcome::Cancelled) => json!({"state":"cancelled"}),
    }
}
fn snapshot(core: &ChunkReadiness, host: &Host, jobs: &BTreeMap<u64, (FutureId, bool)>) -> Value {
    let jobs: serde_json::Map<_, _> = jobs
        .iter()
        .map(|(id, (f, range))| {
            (
                id.to_string(),
                if *range {
                    range_result(core.range_outcome(*f))
                } else {
                    chunk_result(core.outcome(*f))
                },
            )
        })
        .collect();
    let pending:Vec<_>=core.pending_tasks().into_iter().map(|(task,q)|json!({"task":task,"queue":if matches!(q,Queue::Main){"main"}else{"priority"}})).collect();
    let chunks: serde_json::Map<_, _> = host
        .chunks
        .iter()
        .filter(|(_, c)| c.kind == ChunkKind::Full)
        .map(|(id, c)| {
            let ticks: Vec<_> = c
                .ticks
                .iter()
                .map(|(time, order)| json!({"time":time.to_string(),"order":order.to_string()}))
                .collect();
            (
                id.to_string(),
                json!({"block_ticks":ticks,"fluid_ticks":ticks}),
            )
        })
        .collect();
    json!({"jobs":jobs,"pending":pending,"generated_counter":core.generated_counter(),"chunks":chunks})
}

#[test]
fn original_chunkmap_ranges_futures_actions_and_executor_order() {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/chunk-readiness-vectors.json")).unwrap();
    let scenes = fixture["scenes"].as_array().unwrap();
    assert_eq!(scenes.len(), 189);
    let mut operations = 0;
    for scene in scenes {
        let q = &scene["query"];
        let mut core = ChunkReadiness::new();
        core.set_executor_mode(
            Queue::Main,
            mode(
                q.get("main_mode")
                    .and_then(Value::as_str)
                    .unwrap_or("queued"),
            ),
        );
        core.set_executor_mode(
            Queue::Priority {
                center: HolderRef {
                    id: 0,
                    chunk: (0, 0),
                },
            },
            mode(
                q.get("priority_mode")
                    .and_then(Value::as_str)
                    .unwrap_or("queued"),
            ),
        );
        let mut host = Host {
            holders: BTreeMap::new(),
            sources: BTreeMap::new(),
            chunks: BTreeMap::new(),
            events: Vec::new(),
            jobs: BTreeMap::new(),
            time: q
                .get("game_time")
                .map(|v| v.as_str().unwrap().parse().unwrap())
                .unwrap_or(123),
            time_fault: false,
        };
        for c in q["chunks"].as_array().unwrap() {
            host.chunks.insert(
                c["id"].as_u64().unwrap(),
                FixtureChunk {
                    kind: if c["kind"] == "full" {
                        ChunkKind::Full
                    } else {
                        ChunkKind::Proto
                    },
                    saved: c
                        .get("saved_ticks")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    ticks: Vec::new(),
                },
            );
        }
        for source in q["sources"].as_array().unwrap() {
            let spec = SourceSpec {
                identity: source["id"].as_u64().unwrap(),
                initial: outcome(source, &host.chunks),
            };
            core.register_source(spec.clone());
            host.sources.insert(spec.identity, spec);
        }
        let mut sends = BTreeMap::new();
        for (index, h) in q["holders"].as_array().unwrap().iter().enumerate() {
            let chunk = (
                h["x"].as_i64().unwrap() as i32,
                h["z"].as_i64().unwrap() as i32,
            );
            let reference = HolderRef {
                id: index as u64 + 1,
                chunk,
            };
            let allowed = if let Some(v) = h.get("allowed") {
                v.as_u64().and_then(|v| ChunkStatus::from_index(v as u8))
            } else {
                Some(ChunkStatus::FULL)
            };
            let requests = h["requests"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| (v[0].as_u64().unwrap() as u8, v[1].as_u64().unwrap()))
                .collect();
            host.holders.insert(
                chunk,
                FixtureHolder {
                    reference,
                    allowed,
                    requests,
                },
            );
            if let Some(send) = h.get("send_state") {
                let state = match send.as_str().unwrap() {
                    "pending" => ChunkOutcome::Pending,
                    "success" => ChunkOutcome::Failure("signal only".into()),
                    "exceptional" => ChunkOutcome::Exceptional(ReadinessError::new(
                        "java.lang.IllegalStateException",
                        "oracle send failure",
                    )),
                    "cancelled" => ChunkOutcome::Cancelled,
                    _ => unreachable!(),
                };
                let id = core.register_signal(state);
                core.set_send_sync_future(reference.id, id).unwrap();
                sends.insert(h["send_id"].as_u64().unwrap(), id);
            }
        }
        let mut jobs = BTreeMap::new();
        assert_eq!(
            snapshot(&core, &host, &jobs),
            scene["expected"]["initial"],
            "initial {}",
            q["name"]
        );
        for (index, op) in q["operations"].as_array().unwrap().iter().enumerate() {
            operations += 1;
            host.events.clear();
            let mut result = Value::Null;
            match op["op"].as_str().unwrap() {
                "prepare" | "range" => {
                    let center = host.holders[&(
                        op["x"].as_i64().unwrap() as i32,
                        op["z"].as_i64().unwrap() as i32,
                    )]
                        .reference;
                    let range = op["op"] == "range";
                    let (f, _) = if range {
                        let statuses: Vec<_> = op["statuses"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|v| ChunkStatus::from_index(v.as_u64().unwrap() as u8).unwrap())
                            .collect();
                        core.range_future(
                            center,
                            op["radius"].as_u64().unwrap() as u8,
                            &statuses,
                            &mut host,
                        )
                        .unwrap()
                    } else {
                        core.prepare(kind(op["kind"].as_str().unwrap()), center, &mut host)
                            .unwrap()
                    };
                    let job = op["job"].as_u64().unwrap();
                    jobs.insert(job, (f, range));
                    host.jobs.insert(f, job);
                }
                "observe" => {
                    core.observe(
                        jobs[&op["job"].as_u64().unwrap()].0,
                        op["observer"].as_u64().unwrap(),
                        &mut host,
                    )
                    .unwrap();
                }
                "source" => {
                    let value = outcome(op, &host.chunks);
                    result = json!(
                        core.complete_source(op["id"].as_u64().unwrap(), value, &mut host)
                            .unwrap()
                            .0
                    );
                }
                "job" => {
                    let value = outcome(op, &host.chunks);
                    result = json!(
                        core.complete_future(jobs[&op["id"].as_u64().unwrap()].0, value, &mut host)
                            .unwrap()
                            .0
                    );
                }
                "send" => {
                    let value = match op["state"].as_str().unwrap() {
                        "success" => ChunkOutcome::Failure("normal signal".into()),
                        "cancelled" => ChunkOutcome::Cancelled,
                        _ => ChunkOutcome::Exceptional(ReadinessError::new(
                            "java.lang.IllegalStateException",
                            "oracle send failure",
                        )),
                    };
                    result = json!(
                        core.complete_signal(sends[&op["id"].as_u64().unwrap()], value, &mut host)
                            .unwrap()
                            .0
                    );
                }
                "run" => {
                    core.run_task(op["task"].as_u64().unwrap(), &mut host)
                        .unwrap();
                }
                "drain" => {
                    let queue = op.get("queue").and_then(Value::as_str).unwrap_or("any");
                    let budget = op
                        .get("budget")
                        .and_then(Value::as_u64)
                        .map(|v| v as usize)
                        .unwrap_or(usize::MAX);
                    let mut ran = Vec::new();
                    for _ in 0..budget {
                        let next = core.pending_tasks().into_iter().find(|(_, q)| {
                            queue == "any"
                                || queue
                                    == if matches!(q, Queue::Main) {
                                        "main"
                                    } else {
                                        "priority"
                                    }
                        });
                        let Some((task, _)) = next else {
                            break;
                        };
                        ran.push(task);
                        core.run_task(task, &mut host).unwrap();
                    }
                    result = json!(ran);
                }
                "time" => host.time = op["value"].as_str().unwrap().parse().unwrap(),
                "time_fault" => host.time_fault = op["value"].as_bool().unwrap(),
                "executor" => {
                    let queue = if op["queue"] == "main" {
                        Queue::Main
                    } else {
                        Queue::Priority {
                            center: HolderRef {
                                id: 0,
                                chunk: (0, 0),
                            },
                        }
                    };
                    core.set_executor_mode(queue, mode(op["mode"].as_str().unwrap()));
                }
                unknown => panic!("unknown operation {unknown}"),
            }
            assert_eq!(
                json!({"result":result,"events":host.events,"state":snapshot(&core,&host,&jobs)}),
                scene["expected"]["operations"][index],
                "{} operation {index} {op}",
                q["name"]
            );
        }
    }
    assert_eq!(operations, 4521);
}

#[test]
fn original_generation_status_function_and_explicit_action_failure() {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/chunk-readiness-vectors.json")).unwrap();
    for (level, status) in fixture["allowed_generation_status"].as_object().unwrap() {
        let level = level.parse().unwrap();
        assert_eq!(
            allowed_generation_status(level).map(ChunkStatus::index),
            status.as_u64().map(|v| v as u8)
        );
    }
    for (distance, status) in fixture["status_around_full"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        assert_eq!(
            status_around_full(distance as i32).index(),
            status.as_u64().unwrap() as u8
        );
    }
    assert_eq!(fixture["generation_unloaded_error"], "Unloaded chunk");
    assert!(!can_request(None, ChunkStatus::EMPTY));
    assert!(!can_request(
        allowed_generation_status(34),
        ChunkStatus::FULL
    ));
}

#[test]
fn send_sync_is_done_includes_exceptional_cancelled_and_current_replacement() {
    let mut core = ChunkReadiness::new();
    let holder = |id| HolderRef { id, chunk: (0, 0) };
    assert!(core.send_sync_done(holder(1)));
    for (id, value, done) in [
        (1, ChunkOutcome::Pending, false),
        (2, ChunkOutcome::Failure("normal value".into()), true),
        (
            3,
            ChunkOutcome::Exceptional(ReadinessError::new(
                "java.lang.IllegalStateException",
                "failure",
            )),
            true,
        ),
        (4, ChunkOutcome::Cancelled, true),
        (
            5,
            ChunkOutcome::Success(ChunkData {
                id: 1,
                kind: ChunkKind::Full,
            }),
            true,
        ),
    ] {
        let future = core.register_signal(value);
        core.set_send_sync_future(id, future).unwrap();
        assert_eq!(core.send_sync_done(holder(id)), done);
    }
    let mut host = Host {
        holders: BTreeMap::new(),
        sources: BTreeMap::new(),
        chunks: BTreeMap::new(),
        events: Vec::new(),
        jobs: BTreeMap::new(),
        time: 0,
        time_fault: false,
    };
    let future = core.register_signal(ChunkOutcome::Pending);
    core.set_send_sync_future(3, future).unwrap();
    assert!(!core.send_sync_done(holder(3)));
    core.complete_signal(future, ChunkOutcome::Cancelled, &mut host)
        .unwrap();
    assert!(core.send_sync_done(holder(3)));
    assert!(!core.send_sync_done(holder(1)));
}
