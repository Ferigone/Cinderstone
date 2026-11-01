#!/usr/bin/env python3
"""Pin original sender order plus TCP quota/ACK/view and existing regressions."""
import argparse,hashlib,json,os,re,shutil,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
RUNTIME=ROOT/'runtime/phase5-chunk-transport'
GOLDEN=ROOT/'fixtures/main_chunk_transport_golden.json'
PROBES=[
 ('entity',['tools/entity_live_probe.mjs','--equipment','--tracking-window','--port','25611']),
 ('container',['tools/container_live_probe.mjs','--port','25612']),
 ('duplicate',['tools/duplicate_login_live_probe.mjs','--port','25613','--replacements','3']),
 ('creative',['tools/creative_live_probe.mjs','--port','25614','--label','phase5transport']),
 ('placement',['tools/placement_live_probe.mjs','--port','25615','--label','phase5transport']),
 ('drop-safety',['tools/phase5_empty_drop_live_probe.mjs','--port','25616']),
]
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def run(args,log,**kwargs):
 result=subprocess.run(args,cwd=ROOT,text=True,capture_output=True,**kwargs);log.write_text(result.stdout+result.stderr)
 if result.returncode:raise RuntimeError((result.stdout+result.stderr)[-10000:])
 return result.stdout+result.stderr
def report(path):
 for line in reversed(path.read_text().splitlines()):
  try:value=json.loads(line)
  except json.JSONDecodeError:continue
  if isinstance(value,dict)and'ok'in value:return value
 raise RuntimeError('Missing report '+str(path))
def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--run',action='store_true');args=parser.parse_args()
 RUNTIME.mkdir(parents=True,exist_ok=True);binary=RUNTIME/'mc121-rust'
 names=['src/main.rs','src/main_chunk_transport.rs','tools/main_item_plumbing_tests.rs','tools/main_chunk_transport_live_probe.mjs']
 before={name:sha(ROOT/name)for name in names}
 if args.run:
  env=os.environ.copy();env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup',CARGO_INCREMENTAL='0');env['PATH']=env['CARGO_HOME']+'/bin:'+env.get('PATH','')
  cargo=str(Path(env['CARGO_HOME'])/'bin/cargo')
  run([cargo,'test','--bin','mc121-rust','phase5','--','--nocapture'],ROOT/'runtime/main-chunk-transport-tests.log',env=env)
  run([cargo,'build','--bin','mc121-rust'],ROOT/'runtime/main-chunk-transport-build.log',env=env)
  temporary=RUNTIME/'mc121-rust.new';shutil.copyfile(ROOT/'target/debug/mc121-rust',temporary);temporary.chmod(0o755);temporary.replace(binary)
  for name,arguments in PROBES:run(['node',arguments[0],'--server-binary',binary,*arguments[1:]],RUNTIME/(name+'-probe.log'),timeout=90)
  run([cargo,'clippy','--bin','mc121-rust','--tests','--','-D','warnings'],ROOT/'runtime/main-chunk-transport-clippy.log',env=env)
 assert GOLDEN.exists(),'Requires actual original successful arq view45 golden'
 golden=json.loads(GOLDEN.read_text());assert golden['batches']and[list(map(len,golden['batches']))]==[[9,16,16,4]]
 run(['node','tools/main_chunk_transport_live_probe.mjs','--server-binary',binary,'--golden',GOLDEN],RUNTIME/'probe.log',timeout=90)
 live=report(RUNTIME/'probe.log');assert live['ok']and all(live['checks'].values()),live
 regressions={}
 for name,_ in PROBES:
  value=report(RUNTIME/(name+'-probe.log'));assert value['ok']and all(value['checks'].values()),value
  if'server_binary_sha256'in value:assert value['server_binary_sha256']==sha(binary)
  regressions[name]=value
 tests=(ROOT/'runtime/main-chunk-transport-tests.log').read_text();match=re.search(r'test result: ok\. (\d+) passed',tests);assert match and int(match.group(1))>=8
 assert 'Finished'in(ROOT/'runtime/main-chunk-transport-clippy.log').read_text()
 assert before=={name:sha(ROOT/name)for name in names},'sources changed during verification'
 evidence=ROOT/'evidence/phase5-chunk-transport';evidence.mkdir(exist_ok=True)
 for name,value in regressions.items():(evidence/(name+'.json')).write_text(json.dumps(value,indent=2)+'\n')
 result=dict(ok=True,minecraft='1.21',protocol=767,full_runtime_parity=False,active_item_runtime=False,
  glue_tests=int(match.group(1)),clippy=True,live_checks=live['checks'],
  regression_checks={name:len(value['checks'])for name,value in regressions.items()},
  actual_original_batch_sizes=[len(b)for b in golden['batches']],actual_original_ordered_chunk_positions=sum(map(len,golden['batches'])),
  golden_sha256=sha(GOLDEN),server_binary_sha256=sha(binary),source_sha256=before,
  dependency_source_sha256={name:sha(ROOT/name)for name in ['src/player_chunk_sender.rs','src/player_chunk_sender_order.rs','src/entity_tracking.rs','src/replication.rs']},
  established=['Initial original9-chunk quota; no further batch before ACK; protocol767 ACK0x08 then16/16/4 chunks',
   'All45 actual TCP chunk positions match unchanged original arq encounter/tie order',
   'Rounded view difference callback order feeds the retained fastutil pending set',
   'Global chunk-send phase follows world/connection phases; session reader runs while waiting for ACK',
   'Private transport outbox retains actual staged packets across dropped broadcast wake-ups',
   'Entity pairing uses actual completed chunk writes and pending state, separately from world visibility'],
  not_verified=['Active ItemEntity world ticks, pickup and drop commit remain gated by separate runtime integration',
   'Original generation/readiness scheduling is a separate manager integration; transport does not confer FULL or TICKING',
   'Actual forced network broadcast-lag/slow-writer fault: retained queue and recovery are covered by glue tests',
   'Complete threaded light engine and imported Anvil generation/conversion'],
  reproduce='python3 tools/main_chunk_transport_check.py --run')
 (ROOT/'evidence/main-chunk-transport-conformance.json').write_text(json.dumps(result,indent=2)+'\n')
 for name in names:shutil.copyfile(ROOT/name,RUNTIME/Path(name).name)
 print(json.dumps(result,indent=2))
if __name__=='__main__':main()
