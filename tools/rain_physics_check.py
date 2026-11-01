#!/usr/bin/env python3
"""Verify exact rain predicates and bounded genuine-source fixture capability."""
from pathlib import Path
import argparse, hashlib, json, os, re, subprocess

ROOT=Path(__file__).resolve().parents[1]
PIN='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def sha(path):return hashlib.sha256((ROOT/path).read_bytes()).hexdigest()
def main():
    parser=argparse.ArgumentParser();parser.add_argument('--release',action='store_true');args=parser.parse_args()
    oracle=json.loads((ROOT/'evidence/rain-physics-oracle.json').read_text())
    physical=json.loads((ROOT/'evidence/rain-fixture-oracle.json').read_text())
    for report in (oracle,physical):assert report['ok']and report['oracle_inner_sha256']==PIN and not report['original_classes_modified']
    assert oracle['cases']==1000 and oracle['trace_calls']==4011
    assert oracle['fixture_sha256']==sha('fixtures/rain-physics-vectors.json')
    assert physical['facts_sha256']==sha('fixtures/rain-fixture-facts.json')
    for path,digest in oracle['source_sha256'].items():assert sha(path)==digest,path
    assert physical['source_sha256']==sha('tools/rain_fixture_oracle.java')
    assert physical['generator_sha256']==sha('tools/rain_fixture_generate.py')
    env=os.environ.copy();env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup',CARGO_INCREMENTAL='0')
    cargo='/root/.local/share/mc-rust-toolchain/cargo/bin/cargo';runtime=ROOT/'runtime/rain-physics';runtime.mkdir(parents=True,exist_ok=True)
    profile='release'if args.release else'debug';checks=[];comparisons={}
    for name,pattern,count in [('rain','rain_',8),('item-regression','runtime_item_world::',11)]:
        command=[cargo,'test','--locked','--lib',pattern]
        if args.release:command.append('--release')
        command+=['--','--nocapture'];result=subprocess.run(command,cwd=ROOT,env=env,capture_output=True,text=True)
        log=runtime/f'{profile}-{name}.log';log.write_text(result.stdout+result.stderr)
        if result.returncode:raise RuntimeError((result.stdout+result.stderr)[-8000:])
        assert re.search(rf'test result: ok\. {count} passed; 0 failed; 0 ignored',result.stdout)
        checks.append(dict(name=name,command=command,exit_code=result.returncode,log_sha256=hashlib.sha256(log.read_bytes()).hexdigest(),passed=count))
        if name=='rain':
            for line in result.stdout.splitlines():
                for marker in ('RAIN_PHYSICS_CONFORMANCE','RAIN_FIXTURE_TEMPERATURE_CONFORMANCE','RAIN_FIXTURE_LEVEL_CONFORMANCE'):
                    if line.startswith(marker+': '):comparisons[marker]=json.loads(line.split(': ',1)[1])
    assert len(comparisons)==3 and all(v['ok']and v['mismatches']==0 for v in comparisons.values())
    command=[cargo,'clippy','--locked','--lib','--tests','--','-D','warnings'];result=subprocess.run(command,cwd=ROOT,env=env,capture_output=True,text=True)
    log=runtime/f'{profile}-clippy.log';log.write_text(result.stdout+result.stderr)
    if result.returncode:raise RuntimeError((result.stdout+result.stderr)[-8000:])
    checks.append(dict(name='clippy',command=command,exit_code=0,log_sha256=hashlib.sha256(log.read_bytes()).hexdigest()))
    paths=['src/rain_physics.rs','src/rain_fixture.rs','src/runtime_item_world.rs','src/runtime_world.rs','src/world.rs','src/entity_state.rs','src/tick.rs','tools/rain_physics_tests.rs','tools/rain_fixture_tests.rs','tools/runtime_item_rain_tests.rs','tools/rain_physics_check.py','tools/rain_physics_audit.md','fixtures/rain-physics-vectors.json','fixtures/rain-fixture-facts.json','evidence/rain-physics-oracle.json','evidence/rain-fixture-oracle.json']
    paths += ['tools/rain_'+name+'_bytecode.txt'for name in ('entity','level','lightgetter','biome','perlin_simplex','simplex')]
    report=dict(ok=True,minecraft='1.21',oracle_inner_sha256=PIN,profile=profile,release_verified=args.release,clippy=True,checks=checks,comparisons=comparisons,genuine_source_evidence=dict(level_chunks=physical['genuine_level_chunks'],sky_evaluations=physical['actual_sky_queries'],plains_temperatures=physical['plains_probes'],level_rain_calls=physical['level_rain_cases']),scope='Original Level/Entity predicates, exact query/fault order, actual settled canonical fixture sky/height and literal plains temperature/cache arithmetic. Additive item-provider capability preserves water/rain/bubble ordering.',not_verified=['Main/coordinator rain wiring and live-client behavior are separate integration work.','General imported/edited sources, unknown/shared biome cache prehistory or multiple-thread/multiple-world ownership.','Concrete source sky queries outside measured Y−80..336 or absent current FULL/light-history witnesses.','Complete weather precipitation side effects, complete Player.tick or full1:1 server parity.'],source_sha256={path:sha(path)for path in paths})
    dest=ROOT/'evidence/rain-physics-conformance.json';temp=dest.with_suffix('.json.tmp');temp.write_text(json.dumps(report,indent=2)+'\n');temp.replace(dest)
    print(json.dumps({key:val for key,val in report.items()if key not in ('source_sha256','checks','not_verified','scope')}))
if __name__=='__main__':main()
