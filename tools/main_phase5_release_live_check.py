#!/usr/bin/env python3
"""Replay all50 native TCP checks against the already-built frozen release binary."""
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor
import hashlib,json,re,subprocess,time,uuid,shutil
ROOT=Path(__file__).resolve().parents[1]
EXPECTED='a7251edcd243c1ad6add1452c79c98f0b38da4548a78b7a158499685f8073bcc'

def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def parsed(path):
 for line in reversed(path.read_text().splitlines()):
  try:row=json.loads(line)
  except json.JSONDecodeError:continue
  if isinstance(row,dict)and'ok'in row:return row
 raise RuntimeError('No report in '+str(path))
def main():
 binary=ROOT/'target/release/mc121-rust';assert sha(binary)==EXPECTED
 validation_path=ROOT/'evidence/phase5-release-validation.json';validation=json.loads(validation_path.read_text())
 assert validation['ok']and validation['server_binary_sha256']==EXPECTED
 inputs=validation['source_sha256']
 harness={'tools/main_phase5_release_live_check.py','tools/item_stats_live_probe.mjs'}
 changes={n:{'build_snapshot_sha256':h,'live_harness_sha256':sha(ROOT/n)}for n,h in inputs.items()if sha(ROOT/n)!=h}
 assert set(changes)<=harness,('compiled/frozen input changed',changes)
 assert all(sha(ROOT/n)==h for n,h in inputs.items()if n not in harness)

 testlog=(ROOT/'evidence/release-tests.log').read_text()
 counts=list(map(int,re.findall(r'test result: ok\. (\d+) passed',testlog)))
 targets=re.findall(r'Running unittests ([^ ]+) ',testlog)
 assert len(counts)==len(targets)==9 and sum(counts)==318
 assert dict(zip(targets,counts))['src/main.rs']==23
 run_id=uuid.uuid4().hex[:12];label='release50_'+run_id
 runtime=ROOT/'runtime/release-live50'/run_id;runtime.mkdir(parents=True)
 evidence=ROOT/'evidence/release-live50';evidence.mkdir(parents=True,exist_ok=True)
 stats_cwd=runtime/'stats-work';stats_cwd.mkdir();(stats_cwd/'fixtures').symlink_to(ROOT/'fixtures',target_is_directory=True)
 (stats_cwd/'runtime/phase5-floor-final').mkdir(parents=True)
 rows=[
  ('stock',7,['tools/item_live_probe.mjs','--scenario','stock','--label',label,'--port','25620'],ROOT,ROOT/f'evidence/item-live-{label}.json'),
  ('entity',12,['tools/entity_live_probe.mjs','--equipment','--tracking-window','--port','25621'],ROOT,None),
  ('container',7,['tools/container_live_probe.mjs','--port','25622'],ROOT,None),
  ('duplicate',7,['tools/duplicate_login_live_probe.mjs','--port','25623','--replacements','3'],ROOT,None),
  ('creative',4,['tools/creative_live_probe.mjs','--port','25624','--label',label],ROOT,None),
  ('placement',4,['tools/placement_live_probe.mjs','--port','25625','--label',label],ROOT,None),
  ('unsupported-guard',5,['tools/phase5_empty_drop_live_probe.mjs','--port','25626','--unsupported-history','--report','evidence/release-live50/unsupported-guard-report.json'],ROOT,None),
  ('stats',4,['tools/item_stats_live_probe.mjs','--port','25627'],stats_cwd,stats_cwd/'evidence/item-stats-live-conformance.json'),
 ]
 def run(row):
  name,expected,args,cwd,full=row;tool=ROOT/args[0]
  command=['node',str(tool),'--server-binary',str(binary),*args[1:]]
  started=time.monotonic();result=subprocess.run(command,cwd=cwd,text=True,capture_output=True,timeout=90)
  log=runtime/(name+'.log');log.write_text(result.stdout+result.stderr)
  if result.returncode:raise RuntimeError(name+': '+(result.stdout+result.stderr)[-10000:])
  report=parsed(log);assert report['ok']and len(report['checks'])==expected and all(report['checks'].values()),(name,report)
  if'server_binary_sha256'in report:assert report['server_binary_sha256']==EXPECTED,(name,'binary mismatch')
  report_path=evidence/(name+'.json');report_path.write_text(json.dumps(report,indent=2)+'\n')
  if full:
   target=evidence/(name+'-full.json');shutil.copyfile(full,target)
  record=dict(command=command,working_directory=str(cwd),exit_code=result.returncode,seconds=round(time.monotonic()-started,3),
   tool_sha256=sha(tool),checks=report['checks'],check_count=expected,
   report=str(report_path.relative_to(ROOT)),report_sha256=sha(report_path),log=str(log.relative_to(ROOT)),log_sha256=sha(log))
  if full:record['full_report']=str(target.relative_to(ROOT));record['full_report_sha256']=sha(target)
  (runtime/(name+'-record.json')).write_text(json.dumps(record,indent=2)+'\n')
  print(json.dumps(dict(probe=name,ok=True,checks=expected)),flush=True)
  return name,record
 with ThreadPoolExecutor(max_workers=2)as pool:runs=dict(pool.map(run,rows))
 assert all(sha(ROOT/n)==h for n,h in inputs.items()if n not in harness),'compiled/frozen input changed during replay'
 assert changes=={n:{'build_snapshot_sha256':h,'live_harness_sha256':sha(ROOT/n)}for n,h in inputs.items()if sha(ROOT/n)!=h}

 assert sha(binary)==EXPECTED,'release binary changed'
 assert sum(run['check_count']for run in runs.values())==50
 stress=[]
 for index in range(1,11):
  work=runtime/f'stats-stress-{index:02d}';work.mkdir();(work/'fixtures').symlink_to(ROOT/'fixtures',target_is_directory=True);(work/'runtime/phase5-floor-final').mkdir(parents=True)
  command=['node',str(ROOT/'tools/item_stats_live_probe.mjs'),'--server-binary',str(binary),'--port','25627']
  started=time.monotonic();result=subprocess.run(command,cwd=work,text=True,capture_output=True,timeout=90)
  log=runtime/f'stats-stress-{index:02d}.log';log.write_text(result.stdout+result.stderr)
  full=work/'evidence/item-stats-live-conformance.json';value=json.loads(full.read_text())
  destination=evidence/f'stats-stress-{index:02d}.json';shutil.copyfile(full,destination)
  record=dict(index=index,command=command,working_directory=str(work),exit_code=result.returncode,seconds=round(time.monotonic()-started,3),tool_sha256=sha(ROOT/'tools/item_stats_live_probe.mjs'),report=str(destination.relative_to(ROOT)),report_sha256=sha(destination),log=str(log.relative_to(ROOT)),log_sha256=sha(log),checks=value['checks'],ok=value['ok'])
  (runtime/f'stats-stress-{index:02d}-record.json').write_text(json.dumps(record,indent=2)+'\n')
  if result.returncode or not value['ok'] or len(value['checks'])!=4 or not all(value['checks'].values()):raise RuntimeError(('stats stress failed',record))
  assert value['server_binary_sha256']==EXPECTED
  stress.append(record);print(json.dumps(dict(stats_stress=index,ok=True,checks=4)),flush=True)
 assert sha(binary)==EXPECTED
 assert all(sha(ROOT/n)==h for n,h in inputs.items()if n not in harness)
 java_report=ROOT/'evidence/item-live-java_stock_reference.json';java_wrapper=ROOT/'evidence/item-live-java_stock_reference-wrapper.json'
 original=json.loads(java_report.read_text());reference=json.loads(java_wrapper.read_text())
 assert original['ok']and reference['ok']and original['checks']==runs['stock']['checks']
 assert reference['probe_source_sha256']==sha(ROOT/'tools/item_live_probe.mjs')
 report=dict(ok=True,minecraft='1.21',protocol=767,build_profile='release',full_runtime_parity=False,
  server_binary=str(binary.relative_to(ROOT)),server_binary_sha256=EXPECTED,release_validation_report=str(validation_path.relative_to(ROOT)),
  release_validation_report_sha256=sha(validation_path),release_all_targets_tests_passed=318,release_test_binaries=9,release_main_tests_passed=23,
  frozen_inputs_checked=len(inputs),compiled_source_and_fixture_inputs_unchanged=True,post_build_harness_changes=changes,
  frozen_main_sha256=inputs['src/main.rs'],source_commit=validation['source_commit'],cargo_invoked=False,live_check_total=50,runs=runs,
  stats_stress_runs=10,stats_stress_checks=40,stats_stress=stress,
  initial_failure_report='evidence/release-live50-initial-failure.json',
  initial_failure_cause='Not directly proven by the initial uninstrumented trace; confirmed observer/chunk snapshot races were independently fixed and five deterministic tracker/overflow regressions passed.',
  entity_ordering_scope='Every accepted observer transition has an immutable producer capture in FIFO; chunk wakes consume exact immutable prefixes. Lost broadcast history disconnects explicitly instead of inventing a recovery snapshot.',
  original_stock_reference=dict(report=str(java_report.relative_to(ROOT)),report_sha256=sha(java_report),
   wrapper=str(java_wrapper.relative_to(ROOT)),wrapper_sha256=sha(java_wrapper),
   original_outer_sha1=original['original_java_outer_sha1'],same_frozen_probe_source=True,identical_seven_predicates=True),
  runner_source_sha256=sha(Path(__file__)),
  setup=dict(unique_release_labels=label,stats_fresh_working_directory=str(stats_cwd.relative_to(ROOT)),
   stats_configuration_source='fixtures -> frozen validation fixtures; new per-run runtime only'),
  limitations=['The same guarded fresh ordinary-floor native scope as the frozen phase5 source; this is not complete vanilla parity',
   'NativeFixtureEmptyCriteria does not load vanilla advancements/recipe unlocks; full Player.tick/doTick and item/stat persistence remain outside scope',
   'Native stable holder/priority policies do not prove original volatile Java global scheduler order',
   'Rain exposure and scheduler-budget fail-stop limitations remain; the separately compiled observer-order fix does not imply complete vanilla overflow/response-order parity',
   'Stock probes use default stone stacks; arbitrary custom component identity graphs and exact cross-process random trajectory equality are not proven'],
  reproduce='python3 tools/main_phase5_release_live_check.py')
 destination=ROOT/'evidence/main-phase5-release-live-conformance.json';destination.write_text(json.dumps(report,indent=2)+'\n')
 print(json.dumps(dict(ok=True,profile='release',binary_sha256=EXPECTED,tests=318,main_tests=23,stats_stress=10,live_checks=50,report=str(destination.relative_to(ROOT)))),flush=True)
if __name__=='__main__':main()
