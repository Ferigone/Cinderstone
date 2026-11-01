#!/usr/bin/env python3
"""Record provenance for the frozen fresh-floor runtime; optionally repeat TCP probes."""
from pathlib import Path
import argparse, hashlib, json, re, subprocess
ROOT=Path(__file__).resolve().parents[1]
RUNTIME=ROOT/'runtime/phase5-floor-final'
PROBES=[
 ('stock',['tools/item_live_probe.mjs','--scenario','stock','--label','rust-phase5-final','--port','25620']),
 ('entity',['tools/entity_live_probe.mjs','--equipment','--tracking-window','--port','25621']),
 ('container',['tools/container_live_probe.mjs','--port','25622']),
 ('duplicate',['tools/duplicate_login_live_probe.mjs','--port','25623','--replacements','3']),
 ('creative',['tools/creative_live_probe.mjs','--port','25624','--label','phase5final']),
 ('placement',['tools/placement_live_probe.mjs','--port','25625','--label','phase5final']),
 ('unsupported-guard',['tools/phase5_empty_drop_live_probe.mjs','--port','25626','--unsupported-history','--report','evidence/main-runtime-unsupported-conformance.json']),
 ('stats',['tools/item_stats_live_probe.mjs','--port','25627']),
]
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def parse(path):
 for line in reversed(path.read_text().splitlines()):
  try:value=json.loads(line)
  except json.JSONDecodeError:continue
  if isinstance(value,dict)and'ok'in value:return value
 raise RuntimeError('Missing report '+str(path))
def main():
 parser=argparse.ArgumentParser(description=__doc__)
 parser.add_argument('--run-live',action='store_true',help='repeat all TCP probes using the pinned binary; does not rebuild')
 args=parser.parse_args();binary=RUNTIME/'mc121-rust';build=json.loads((RUNTIME/'build-snapshot.json').read_text())
 assert sha(binary)==build['binary_sha256'],'pinned build changed'
 assert build['source_sha256']=={name:sha(ROOT/name)for name in build['source_sha256']},'built source changed after pin'
 if args.run_live:
  for name,arguments in PROBES:
   result=subprocess.run(['node',arguments[0],'--server-binary',binary,*arguments[1:]],cwd=ROOT,text=True,capture_output=True,timeout=90)
   (RUNTIME/(name+'.log')).write_text(result.stdout+result.stderr)
   if result.returncode:raise RuntimeError((result.stdout+result.stderr)[-10000:])
 reports={name:parse(RUNTIME/(name+'.log'))for name,_ in PROBES}
 for name,value in reports.items():
  assert value['ok']and all(value['checks'].values()),(name,value)
  if'server_binary_sha256'in value:assert value['server_binary_sha256']==sha(binary),(name,'wrong binary')
 tests=(ROOT/'runtime/main-phase5-runtime-tests.log').read_text();match=re.search(r'test result: ok\. (\d+) passed',tests)
 assert match and int(match.group(1))>=18
 assert 'Finished'in(ROOT/'runtime/main-phase5-runtime-clippy.log').read_text()
 java_path=ROOT/'evidence/item-live-java_stock_reference.json';java_wrapper=ROOT/'evidence/item-live-java_stock_reference-wrapper.json'
 java=json.loads(java_path.read_text());provenance=json.loads(java_wrapper.read_text())
 assert java['ok']and java['scenario']=='stock'and java['original_java_outer_sha1']=='450698d1863ab5180c25d7c804ef0fe6369dd1ba'
 assert provenance['ok']and provenance['reference_world_unchanged']and provenance['probe_source_sha256']==sha(ROOT/'tools/item_live_probe.mjs')
 assert reports['stock']['checks']==java['checks'],'shared stock predicates differ'
 evidence=ROOT/'evidence/phase5-runtime';evidence.mkdir(exist_ok=True)
 for name,value in reports.items():(evidence/(name+'.json')).write_text(json.dumps(value,indent=2)+'\n')
 dependencies=['src/runtime_entity_index.rs','src/entity_tick.rs','src/entity_spatial.rs','src/item_entity.rs','src/item_damage.rs',
  'src/entity_base.rs','src/entity_state.rs','src/entity_move.rs','src/entity_support.rs','src/runtime_item_world.rs','src/runtime_collision.rs',
  'src/runtime_chunk_manager.rs','src/chunk_readiness.rs','src/chunk_lifecycle.rs','src/chunk_loading_tickets.rs','src/entity_ticking_range.rs',
  'src/player_chunk_sender.rs','src/player_chunk_sender_order.rs','src/metadata.rs','src/replication.rs','src/entity_tracking.rs',
  'src/entity_tracking_order.rs','src/entity.rs','src/world_context.rs','src/world_rules.rs','src/level.rs','src/border.rs','src/tick.rs',
  'src/protocol.rs','src/game_event_context.rs','src/player_stats.rs','src/random.rs','src/random_seed.rs','src/world.rs','src/storage.rs',
  'src/geometry.rs','src/fluid_physics.rs','src/inventory.rs','Cargo.toml','Cargo.lock']
 report=dict(ok=True,minecraft='1.21',protocol=767,milestone='fresh native ordinary-floor item runtime',
  full_runtime_parity=False,full_server_player_tick=False,default_vanilla_advancements=False,
  world_item_simulation_active=True,connection_phase_pickup_active=True,
  build_profile='debug',server_binary=str(binary.relative_to(ROOT)),server_binary_sha256=sha(binary),
  built_source_sha256=build['source_sha256'],dependency_sources_sha256_at_recording={name:sha(ROOT/name)for name in dependencies},
  native_bin_tests=int(match.group(1)),clippy_warnings_denied=True,
  native_test_source_sha256={name:sha(ROOT/name)for name in ['tools/main_item_plumbing_tests.rs','tools/main_drop_commit_tests.rs','tools/main_item_gameplay_tests.rs']},
  live_checks={name:len(value['checks'])for name,value in reports.items()},live_check_total=sum(len(v['checks'])for v in reports.values()),
  stock_reference=dict(original_outer_sha1=java['original_java_outer_sha1'],reference_world_unchanged=True,
   identical_shared_probe_predicates=True,checks=reports['stock']['checks'],
   probe_source_sha256=sha(ROOT/'tools/item_live_probe.mjs'),
   original_report=str(java_path.relative_to(ROOT)),original_report_sha256=sha(java_path),
   original_wrapper=str(java_wrapper.relative_to(ROOT)),original_wrapper_sha256=sha(java_wrapper),
   native_report='evidence/item-live-rust-phase5-final.json',native_report_sha256=sha(ROOT/'evidence/item-live-rust-phase5-final.json')),
  probe_sha256={arguments[0]:sha(ROOT/arguments[0])for _,arguments in PROBES},
  evidence_sha256={str((evidence/(name+'.json')).relative_to(ROOT)):sha(evidence/(name+'.json'))for name in reports},
  validation_logs_sha256={name:sha(ROOT/name)for name in ['runtime/main-phase5-runtime-tests.log','runtime/main-phase5-runtime-build.log','runtime/main-phase5-runtime-clippy.log']},
  established=['Real retained world/player/entity random streams and atomic menu/drop ID/RNG commit',
   'Q/CtrlQ shared item spawn, Slot8 and velocity pairing, gravity/ordinary-floor collision, stationary leave/re-pair and delayed pickup',
   'Mixed entity sender and entity tick order; pickup after levels in connection admission order',
   'Actual chunk lifecycle callbacks, current Block/sendSync send predicate, ACK0x08 and retained private transport outbox',
   'Every actual scheduled item query has a conservative swept/event halo certificate independent of published visibility',
   'Source-exact local menu observer baseline; staged empty-criteria observations before Menu publish and normal level-phase menu sync',
   'Actual request_stats wire action, drop/custom-drop/pickup dirty deltas and repeated empty response',
   'Uncertified saved/imported source refuses nonempty drops before inventory/RNG/ID consumption'],
  supported_source_scope='Fresh newly constructed, unedited native flat source; observed native empty lighting work, owned empty criterion/scoreboard/game-event registries; dry ordinary item queries and velocity envelope4. This fixture does not load vanilla advancements/recipe unlocks.',
  ordering_scope='NativeStablePolicy holder order, NativeStablePriorityScope and retained native holder lookup are explicit policies; original volatile identity HashSet/global mailbox/visible-map iteration are not claimed.',
  live_stock_item_scope='Default minecraft:stone stacks; the stock probe is not an exhaustive arbitrary custom-component/identity-graph proof',
  not_verified=['Complete vanilla1.21 behavior, all player ticks/travel/attributes and complete connection doTick',
   'Arbitrary custom component identity/equality graphs beyond the separately documented inventory conformance scope',
   'Original default advancement catalogue/recipe unlocks or nonempty scoreboard/listener/criterion execution',
   'Imported Anvil generation/source conversion, inherited entity/BE/dynamic listener state',
   'Moving item hazards/fluids/passengers and arbitrary callbacks outside the guarded ordinary-floor source',
   'Natural rain/exposure: active item ticks fail explicitly until the concrete rain provider is integrated',
   'A nonquiescent16384-turn scheduler budget can fail the item certificate; this is a fail-stop checkpoint, not a long-lived ready server',
   'Unsupported world edits are refused while items exist; edits/load invalidate the fresh lighting source scope',
   'Runtime item/stat persistence and restart parity; source scope deliberately becomes unknown on saved-world load',
   'Controlled live merge harness, exact cross-process random trajectory bytes, performance superiority and complete Java parity'],
  reproduce='python3 tools/main_phase5_runtime_check.py --run-live (uses pinned binary/build-snapshot plus recorded bin-test/Clippy logs)')
 (ROOT/'evidence/main-phase5-runtime-conformance.json').write_text(json.dumps(report,indent=2)+'\n')
 print(json.dumps({k:report[k]for k in ['ok','milestone','server_binary_sha256','native_bin_tests','live_checks','live_check_total','full_runtime_parity']},indent=2))
if __name__=='__main__':main()
