use super::*;
use serde_json::{json, Value};

fn future_state(value: &Value) -> FutureState {
    match value.as_str().unwrap() {
        "pending" => FutureState::Pending,
        "success" => FutureState::Success,
        "failure" => FutureState::Failure,
        "unloaded" => FutureState::Unloaded,
        "exceptional" => FutureState::Exceptional,
        "cancelled" => FutureState::Cancelled,
        name => panic!("unexpected source future {name}"),
    }
}
fn mode(value: &Value) -> ExecutorMode {
    match value.as_str().unwrap() {
        "queued" => ExecutorMode::Queued,
        "inline" => ExecutorMode::Inline,
        "reject" => ExecutorMode::Reject,
        name => panic!("unexpected executor mode {name}"),
    }
}
fn stage(value: &Value) -> Stage {
    match value.as_str().unwrap() {
        "full" => Stage::Full,
        "block" => Stage::Block,
        "entity" => Stage::Entity,
        name => panic!("unexpected prepare stage {name}"),
    }
}
fn event(effect: LifecycleEffect) -> Value {
    match effect {
        LifecycleEffect::Prepare { stage, future } => {
            json!({"kind":"prepare","stage":stage.name(),"future":future})
        }
        LifecycleEffect::Enqueue { task } => json!({"kind":"enqueue","task":task}),
        LifecycleEffect::ExecutorRejected { task } => json!({"kind":"reject","task":task}),
        LifecycleEffect::FullStatusChange { status, visibility } => {
            json!({"kind":"status","status":status as u8,"visibility":visibility as u8})
        }
        LifecycleEffect::LevelChange {
            queue_level,
            new_level,
        } => json!({"kind":"level","queue_level":queue_level,"new_level":new_level}),
    }
}
fn snapshot(state: &ChunkLifecycle) -> Value {
    let s = state.snapshot();
    json!({
        "old_level":s.old_level,"ticket_level":s.ticket_level,"queue_level":s.queue_level,
        "was_accessible":s.was_accessible,"confirmation":s.confirmation.name(),
        "output_status":s.output_status.map(|v|v as i32).unwrap_or(-1),"slots":s.slots,
        "futures":s.futures.into_iter().map(|(id,state)|json!([id,state.name()])).collect::<Vec<_>>(),
        "pending_tasks":s.pending_tasks,"save_done":s.save_done,"save_exceptional":s.save_exceptional,
        "level_getter_values":s.level_getter_values,
    })
}

#[test]
fn actual_chunkholder_futures_executor_and_visibility_interleavings() {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/chunk-lifecycle-vectors.json")).unwrap();
    assert_eq!(fixture["max_level"], MAX_LEVEL);
    let scenes = fixture["scenes"].as_array().unwrap();
    assert_eq!(scenes.len(), 278);
    let mut operations = 0;
    for scene in scenes {
        let query = &scene["query"];
        let mut state = ChunkLifecycle::new(query["initial_level"].as_i64().unwrap() as i32);
        state.set_executor_mode(mode(&query["executor"]));
        state.set_apply_queue_inline(query["apply_queue_inline"].as_bool().unwrap());
        for f in query["futures"].as_array().unwrap() {
            state
                .register_future(f["id"].as_u64().unwrap(), future_state(&f["state"]))
                .unwrap();
        }
        assert_eq!(
            snapshot(&state),
            scene["expected"]["initial"],
            "initial {}",
            query["name"]
        );
        for (index, op) in query["operations"].as_array().unwrap().iter().enumerate() {
            operations += 1;
            let mut result = Value::Null;
            let effects = match op["op"].as_str().unwrap() {
                "set_level" => {
                    state.set_ticket_level(op["level"].as_i64().unwrap() as i32);
                    Vec::new()
                }
                "update" => {
                    if let Some(level) = op.get("level") {
                        state.set_ticket_level(level.as_i64().unwrap() as i32);
                    }
                    let plans: Vec<_> = op["prepares"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|p| Preparation {
                            stage: stage(&p["stage"]),
                            future: p["future"].as_u64().unwrap(),
                        })
                        .collect();
                    state.update_futures(&plans).unwrap()
                }
                "complete" => {
                    let (completed, effects) = state
                        .complete_future(op["future"].as_u64().unwrap(), future_state(&op["state"]))
                        .unwrap();
                    result = json!(completed);
                    effects
                }
                "run" => state
                    .run_executor_task(op["task"].as_u64().unwrap())
                    .unwrap(),
                "drain" => {
                    let (ran, effects) = state.drain_executor(
                        op.get("budget")
                            .map(|v| v.as_u64().unwrap() as usize)
                            .unwrap_or(usize::MAX),
                    );
                    result = json!(ran);
                    effects
                }
                "executor" => {
                    state.set_executor_mode(mode(&op["mode"]));
                    Vec::new()
                }
                "apply_queue" => {
                    state
                        .apply_level_change(op["notification"].as_u64().unwrap() as usize)
                        .unwrap();
                    Vec::new()
                }
                "refresh" => {
                    state.refresh_accessibility();
                    Vec::new()
                }
                name => panic!("unexpected original operation {name}"),
            };
            let actual = json!({"result":result,"events":effects.into_iter().map(event).collect::<Vec<_>>(),"state":snapshot(&state)});
            assert_eq!(
                actual, scene["expected"]["operations"][index],
                "{} operation {index} {op}",
                query["name"]
            );
        }
    }
    assert_eq!(operations, 2653);
}

#[test]
fn completed_promotion_remains_executable_after_immediate_demotion() {
    let mut holder = ChunkLifecycle::new(33);
    holder.register_future(1, FutureState::Success).unwrap();
    let effects = holder
        .update_futures(&[Preparation {
            stage: Stage::Full,
            future: 1,
        }])
        .unwrap();
    assert!(effects.contains(&LifecycleEffect::Enqueue { task: 1 }));
    assert_eq!(holder.output_status(), None);
    holder.set_ticket_level(34);
    holder.update_futures(&[]).unwrap();
    assert_eq!(holder.output_visibility(), Some(Visibility::Hidden));
    holder.run_executor_task(1).unwrap();
    assert_eq!(holder.output_visibility(), Some(Visibility::Tracked));
    assert_eq!(holder.snapshot().ticket_level, 34);
    assert!(holder.run_executor_task(1).unwrap().is_empty());
}

#[test]
fn synchronous_host_preserves_prepare_queue_interleaving_and_demotion_observers() {
    #[derive(Default)]
    struct Host {
        trace: Vec<String>,
        effects: Vec<LifecycleEffect>,
    }
    impl ChunkLifecycleHost for Host {
        fn prepare(&mut self, stage: Stage) -> Result<PreparedFuture, String> {
            self.trace.push(format!("prepare:{}", stage.name()));
            // A later prepare has its own synchronous executor side effect.
            if stage == Stage::Entity {
                self.trace.push("readiness-main".into());
            }
            Ok(PreparedFuture {
                future: stage.index() as u64 + 1,
                initial_state: if stage == Stage::Full {
                    FutureState::Success
                } else {
                    FutureState::Pending
                },
            })
        }
        fn emit(&mut self, effect: LifecycleEffect) {
            match effect {
                LifecycleEffect::Enqueue { task } => {
                    self.trace.push(format!("lifecycle-main:{task}"))
                }
                LifecycleEffect::FullStatusChange { status, .. } => {
                    self.trace.push(format!("status:{}", status as u8))
                }
                _ => {}
            }
            self.effects.push(effect);
        }
        fn complete_prepared_future(&mut self, future: FutureId, state: FutureState) {
            assert_eq!(state, FutureState::Unloaded);
            self.trace.push(format!("complete-provider:{future}"));
        }
    }
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/chunk-lifecycle-vectors.json")).unwrap();
    let scene = fixture["scenes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["query"]["name"] == "three_stages/queued/success/pending/pending")
        .unwrap();
    let mut holder = ChunkLifecycle::new(31);
    let mut host = Host::default();
    holder.update_futures_with_host(&mut host).unwrap();
    assert_eq!(
        host.effects.iter().cloned().map(event).collect::<Vec<_>>(),
        scene["expected"]["operations"][0]["events"]
            .as_array()
            .unwrap()
            .clone()
    );
    assert_eq!(
        host.trace,
        vec![
            "prepare:full",
            "lifecycle-main:1",
            "prepare:block",
            "prepare:entity",
            "readiness-main"
        ]
    );
    host.trace.clear();
    holder.set_ticket_level(45);
    holder.update_futures_with_host(&mut host).unwrap();
    assert_eq!(
        host.trace,
        vec!["complete-provider:2", "complete-provider:3", "status:0"]
    );
    assert_eq!(holder.queue_level(), 45);
    assert_eq!(holder.future_state(3), Some(FutureState::Unloaded));
}
