#!/usr/bin/env python3
"""Compare the runtime support provider with original findSupportingBlock."""
import argparse
import collections
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT=Path(__file__).resolve().parents[1]

def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--skip-generate',action='store_true',help='use the pinned Java corpus')
    parser.add_argument('--release',action='store_true',help='also check optimized thinLTO tests')
    args=parser.parse_args()
    if not args.skip_generate:subprocess.run([sys.executable,ROOT/'tools/entity_support_generate.py'],cwd=ROOT,check=True)
    fixture=json.loads((ROOT/'fixtures/entity_support_vectors.json').read_text())
    runtime=ROOT/'runtime/entity_support';runtime.mkdir(parents=True,exist_ok=True)
    env=os.environ.copy();env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup',CARGO_INCREMENTAL='0')
    env['PATH']=env['CARGO_HOME']+'/bin:'+env.get('PATH','')
    cargo=str(Path(env['CARGO_HOME'])/'bin/cargo');tests={}
    for profile in ['debug','release']if args.release else ['debug']:
        command=[cargo,'test']+(['--release']if profile=='release'else[])+['--lib','entity_support::tests','--','--nocapture']
        result=subprocess.run(command,cwd=ROOT,env=env,text=True,capture_output=True)
        output=result.stdout+result.stderr;(runtime/('rust-'+profile+'.log')).write_text(output)
        if result.returncode:raise RuntimeError(output[-7000:])
        required='test entity_support::tests::actual_java_candidates_and_support_selection ... ok'
        if required not in result.stdout:raise RuntimeError('Required support test did not execute\n'+output[-7000:])
        match=re.search(r'test result: ok\. (\d+) passed',result.stdout)
        if not match:raise RuntimeError('Missing native test result\n'+output[-7000:])
        tests[profile]=dict(passed=int(match.group(1)),failed=0,mismatches=0)
    cases=fixture['cases']
    report=dict(ok=True,minecraft='1.21',oracle_inner_sha256=fixture['oracle_inner_sha256'],
        queries=len(cases),candidate_positions=sum(len(case['expected']['positions'])for case in cases),
        selected_support=sum(case['expected']['support']is not None for case in cases),
        categories=dict(collections.Counter(case['category']for case in cases)),tests=tests,
        scope='Original CollisionGetter.findSupportingBlock and BlockCollisions candidate positions/order '
              'compared end-to-end with RuntimeCollision over a finite empty fixture plus supplied block states. '
              'Selection uses exact block-center squared distance, Double.MAX_VALUE initialization and '
              'Vec3i Y/Z/X tie comparison; the original Java methods/classes are unchanged.',
        source_sha256={name:sha(ROOT/name)for name in [
            'src/entity_support.rs','src/runtime_collision.rs','src/geometry.rs',
            'tools/entity_support_oracle.java','tools/entity_support_generate.py',
            'fixtures/entity_support_vectors.json','fixtures/runtime_collision_shapes-1.21.json',
        ]},
        not_verified=['Live Entity.move/support fallback integration',
                      'Animated moving-piston and shulker block-entity shapes',
                      'Vanilla missing/unloaded chunk policy; native unavailable-world errors remain explicit'],
        reproduce='python3 tools/entity_support_check.py --release')
    (ROOT/'evidence/entity-support-conformance.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))

if __name__=='__main__':main()
