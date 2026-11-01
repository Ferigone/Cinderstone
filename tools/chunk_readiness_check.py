#!/usr/bin/env python3
"""Replay original ChunkMap ranges/continuations/actions in native debug and release."""
from pathlib import Path
import argparse,hashlib,json,os,re,subprocess
ROOT=Path(__file__).resolve().parents[1]
PIN='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
 args=argparse.ArgumentParser(description=__doc__);args.add_argument('--clippy-only',action='store_true',help='reuse successful debug/release replay only when all recorded proof input hashes still match');args=args.parse_args()
 p=ROOT/'fixtures/chunk-readiness-vectors.json';fixture=json.loads(p.read_text());original=json.loads((ROOT/'evidence/chunk-readiness-oracle.json').read_text())
 assert sha(ROOT.parent/'mc121-oracle/server-inner-1.21.jar')==PIN
 assert original['status']=='PASS'and original['vectors_sha256']==sha(p)
 assert original['oracle_source_sha256']==sha(ROOT/'tools/chunk_readiness_oracle.java')
 assert original['generator_sha256']==sha(ROOT/'tools/chunk_readiness_generate.py')
 env=os.environ.copy();env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup',CARGO_INCREMENTAL='0');env['PATH']=env['CARGO_HOME']+'/bin:'+env['PATH']
 runtime=ROOT/'runtime/chunk-readiness';runtime.mkdir(parents=True,exist_ok=True)
 report=dict(ok=False,minecraft='1.21',oracle_inner_sha256=PIN,scenes=len(fixture['scenes']),operations=sum(len(s['query']['operations'])for s in fixture['scenes']),
             vectors_sha256=sha(p),oracle_report_sha256=sha(ROOT/'evidence/chunk-readiness-oracle.json'),source_sha256=sha(ROOT/'src/chunk_readiness.rs'),tests_sha256=sha(ROOT/'tools/chunk_readiness_tests.rs'),
             rust_debug_verified=False,rust_release_verified=False,clippy=False,
             scope='Actual ChunkMap range dependency order and status function, GenerationHolder allowed status, JDK21 allOf/continuation stack, priority/main queue/rejection/ACK, success-only actions, sendSync timing, exceptional/cancelled/directly completed prepared futures, observer/counter order.',
             actions='Original producer invokes genuine LevelChunk.postProcessGeneration in explicit empty PostProcessing/NBT/upgrade context, real ServerLevel.startTickingChunk with nonempty saved tick containers, and original send loop with empty PlayerMap. Native test host records and checks this guarded action domain; runtime backends must implement required actions or return explicit error.',
             not_verified=['Generation/load conversion and allowed-status update/cancellation machinery outside supplied source futures.','Nonempty PostProcessing, pending block entity NBT or UpgradeData work.','Player recipient selection and real chunk batch transmission (separate sender proof).','General scheduled tick engine; this fixture checks two concrete saved ticks and action timing.','Arbitrary Java int radii: native uses u8; actual preparation radii 1/2 and direct range radii 0..3 are exercised.','Java exception object identity, wrapper chains, stack traces and diagnostic message parity; exceptional outcomes compare the original root-cause class.','Parallel races and reentrant host callbacks outside the explicit serialized interleavings.','Long-lived handle/continuation retention needs host lifetime management before general runtime use.'])
 try:
  if args.clippy_only:
   previous=json.loads((ROOT/'evidence/chunk-readiness-conformance.json').read_text())
   for key in ('minecraft','oracle_inner_sha256','scenes','operations','vectors_sha256','oracle_report_sha256','source_sha256','tests_sha256'):
    assert previous[key]==report[key],f'cannot reuse replay: changed {key}'
   assert previous['rust_debug_verified']and previous['rust_release_verified'],'no successful profile replay to reuse'
   report.update(rust_debug_verified=True,rust_release_verified=True,reused_profile_verification=True)
  else:
   for profile,flags in [('debug',[]),('release',['--release'])]:
    run=subprocess.run(['cargo','test',*flags,'--lib','chunk_readiness::tests','--','--nocapture'],cwd=ROOT,env=env,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
    (runtime/f'native-{profile}.log').write_text(run.stdout);assert run.returncode==0,f'{profile} failed; see runtime/chunk-readiness'
    matches=re.findall(r'test result: ok\. (\d+) passed',run.stdout);assert matches and int(matches[-1])==3
    report[f'rust_{profile}_verified']=True
  run=subprocess.run(['cargo','clippy','--lib','--tests','--','-D','warnings'],cwd=ROOT,env=env,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
  (runtime/'clippy.log').write_text(run.stdout);assert run.returncode==0,'clippy failed; see runtime/chunk-readiness/clippy.log'
  report.update(ok=True,clippy=True,mismatches=0)
 finally:(ROOT/'evidence/chunk-readiness-conformance.json').write_text(json.dumps(report,indent=2)+'\n')
 print(json.dumps(report,indent=2))
if __name__=='__main__':main()
