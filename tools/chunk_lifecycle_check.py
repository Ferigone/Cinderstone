#!/usr/bin/env python3
"""Strict native replay of real ChunkHolder/CompletableFuture/executor observations."""
from pathlib import Path
import hashlib,json,os,re,subprocess
ROOT=Path(__file__).resolve().parents[1]
PIN='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
 fixture_path=ROOT/'fixtures/chunk-lifecycle-vectors.json';fixture=json.loads(fixture_path.read_text())
 original=json.loads((ROOT/'evidence/chunk-lifecycle-oracle.json').read_text())
 assert sha(ROOT.parent/'mc121-oracle/server-inner-1.21.jar')==PIN
 assert original['status']=='PASS'and original['fixture_sha256']==sha(fixture_path)
 assert original['oracle_source_sha256']==sha(ROOT/'tools/chunk_lifecycle_oracle.java')
 assert original['generator_sha256']==sha(ROOT/'tools/chunk_lifecycle_generate.py')
 env=os.environ.copy();env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup',CARGO_INCREMENTAL='0');env['PATH']=env['CARGO_HOME']+'/bin:'+env['PATH']
 runtime=ROOT/'runtime/chunk-lifecycle';runtime.mkdir(parents=True,exist_ok=True)
 report=dict(ok=False,minecraft='1.21',oracle_inner_sha256=PIN,scenes=len(fixture['scenes']),operations=sum(len(s['query']['operations'])for s in fixture['scenes']),
             vectors_sha256=sha(fixture_path),oracle_report_sha256=sha(ROOT/'evidence/chunk-lifecycle-oracle.json'),
             source_sha256=sha(ROOT/'src/chunk_lifecycle.rs'),tests_sha256=sha(ROOT/'tools/chunk_lifecycle_tests.rs'),
             rust_debug_verified=False,rust_release_verified=False,clippy=False,
             scope='Original constructor levels, per-stage future slots, success-only confirmation, literal cancellation, queued/inline/rejected execution, stale callbacks, immediate demotion, visibility mapping, alias handles, lazy queue notifications and saveSync readiness.',
             not_verified=['Chunk generation and ChunkMap prepare dependency graphs; host supplies actual readiness.','Parallel thread races outside the explicit interleavings.','Save contents, sendSync and broader ChunkHolder block/light logic.','Unbounded retained handle history requires host lifetime management before general runtime use.'])
 try:
  for profile,flags in [('debug',[]),('release',['--release'])]:
   run=subprocess.run(['cargo','test',*flags,'--lib','chunk_lifecycle::tests','--','--nocapture'],cwd=ROOT,env=env,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
   (runtime/f'native-{profile}.log').write_text(run.stdout)
   assert run.returncode==0,f'{profile} native replay failed; see runtime/chunk-lifecycle'
   count=re.findall(r'test result: ok\. (\d+) passed',run.stdout);assert count and int(count[-1])==3
   report[f'rust_{profile}_verified']=True
  run=subprocess.run(['cargo','clippy','--lib','--tests','--','-D','warnings'],cwd=ROOT,env=env,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
  (runtime/'clippy.log').write_text(run.stdout);assert run.returncode==0,'clippy failed; see runtime/chunk-lifecycle/clippy.log'
  report.update(clippy=True,mismatches=0,ok=True)
 finally:(ROOT/'evidence/chunk-lifecycle-conformance.json').write_text(json.dumps(report,indent=2)+'\n')
 print(json.dumps(report,indent=2))
if __name__=='__main__':main()
