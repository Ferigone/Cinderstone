#!/usr/bin/env python3
"""Compare spatial sections and class caches with unchanged Minecraft1.21."""
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
    parser.add_argument('--skip-generate',action='store_true',help='use the pinned original Java corpus')
    parser.add_argument('--release',action='store_true',help='also check optimized thinLTO tests')
    args=parser.parse_args()
    if not args.skip_generate:subprocess.run([sys.executable,ROOT/'tools/entity_spatial_generate.py'],cwd=ROOT,check=True)
    fixture=json.loads((ROOT/'fixtures/entity_spatial_vectors.json').read_text())
    runtime=ROOT/'runtime/entity_spatial';runtime.mkdir(parents=True,exist_ok=True)
    env=os.environ.copy();env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup',CARGO_INCREMENTAL='0')
    env['PATH']=env['CARGO_HOME']+'/bin:'+env.get('PATH','')
    cargo=str(Path(env['CARGO_HOME'])/'bin/cargo');tests={}
    for profile in ['debug','release']if args.release else ['debug']:
        command=[cargo,'test']+(['--release']if profile=='release'else[])+['--lib','entity_spatial::tests','--','--nocapture']
        result=subprocess.run(command,cwd=ROOT,env=env,text=True,capture_output=True)
        output=result.stdout+result.stderr;(runtime/('rust-'+profile+'.log')).write_text(output)
        if result.returncode:raise RuntimeError(output[-7000:])
        required='test entity_spatial::tests::actual_java_sections_class_caches_order_visibility_moves_and_queries ... ok'
        if required not in result.stdout:raise RuntimeError('Required spatial test did not execute\n'+output[-7000:])
        match=re.search(r'test result: ok\. (\d+) passed',result.stdout)
        if not match:raise RuntimeError('Missing native test result\n'+output[-7000:])
        tests[profile]=dict(passed=int(match.group(1)),failed=0,mismatches=0)
    operations=[(op,expected)for case in fixture['cases']for op,expected in zip(case['scene']['operations'],case['expected'])]
    report=dict(ok=True,minecraft='1.21',oracle_inner_sha256=fixture['oracle_inner_sha256'],scenes=len(fixture['cases']),
        operations=len(operations),operation_counts=dict(collections.Counter(op['op']for op,_ in operations)),
        query_results=sum(len(result['result']['handles'])for op,result in operations if op['op']=='query'),
        visited_sections=sum(len(result['result']['sections'])for op,result in operations if op['op']=='query'),
        tests=tests,
        scope='Original EntitySectionStorage/EntitySection ordered queries and ClassInstanceMultiMap '
              'lazy class lists, compared after every operation. Includes X/Z±2 and Y−4 broadphase, '
              'signed packed-long section/chunk order, visibility, strict AABB/type tests, aborts, '
              'moves/removal/re-add, duplicate handles and cross-class equal-ID cache divergence. '
              'Opaque handles reference host entity metadata; full entity state is not cloned.',
        source_sha256={name:sha(ROOT/name)for name in [
            'src/entity_spatial.rs','src/geometry.rs','tools/entity_spatial_oracle.java',
            'tools/entity_spatial_generate.py','fixtures/entity_spatial_vectors.json',
        ]},
        not_verified=['Index mutation inside query-consumer callbacks',
                      'Invalid Java class keys outside the declared entity base hierarchy',
                      'Nonfinite AABB queries are explicitly refused; numeric position conversion is tested separately',
                      'Persistent entity manager and live mixed-entity integration'],
        reproduce='python3 tools/entity_spatial_check.py --release')
    (ROOT/'evidence/entity-spatial-conformance.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))

if __name__=='__main__':main()
