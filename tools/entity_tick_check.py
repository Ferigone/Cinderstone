#!/usr/bin/env python3
"""Verify insertion order and EntityTickList iteration against original1.21."""
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

def records(operations):
    for operation in operations:
        yield operation
        for callback in operation.get('callbacks',[]):
            yield from records(callback['effects'])

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--skip-generate',action='store_true',help='use the pinned Java corpus')
    parser.add_argument('--release',action='store_true',help='also check optimized thinLTO tests')
    args=parser.parse_args()
    if not args.skip_generate:subprocess.run([sys.executable,ROOT/'tools/entity_tick_generate.py'],cwd=ROOT,check=True)
    fixture=json.loads((ROOT/'fixtures/entity_tick_vectors.json').read_text())
    runtime=ROOT/'runtime/entity-tick-oracle';runtime.mkdir(parents=True,exist_ok=True)
    env=os.environ.copy();env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup',CARGO_INCREMENTAL='0')
    env['PATH']=env['CARGO_HOME']+'/bin:'+env.get('PATH','')
    cargo=str(Path(env['CARGO_HOME'])/'bin/cargo')
    tests={}
    required=['actual_java_order_identity_copy_on_write_reentrancy_and_callback_errors','rust_unwind_clears_iteration_marker_and_retains_completed_mutations']
    for profile in ['debug','release']if args.release else ['debug']:
        command=[cargo,'test']+(['--release']if profile=='release'else[])+['--lib','entity_tick::tests','--','--nocapture']
        result=subprocess.run(command,cwd=ROOT,env=env,text=True,capture_output=True)
        output=result.stdout+result.stderr;(runtime/('rust-'+profile+'.log')).write_text(output)
        if result.returncode:raise RuntimeError(output[-7000:])
        for test in required:
            if 'test entity_tick::tests::'+test+' ... ok'not in result.stdout:raise RuntimeError('Required native test did not run: '+test+'\n'+output[-7000:])
        match=re.search(r'test result: ok\. (\d+) passed',result.stdout)
        if not match:raise RuntimeError('Missing native test result: '+output[-7000:])
        tests[profile]=dict(passed=int(match.group(1)),failed=0,mismatches=0)
    observed=[record for scene in fixture['scenes']for record in records(scene['expected'])]
    report=dict(ok=True,minecraft='1.21',oracle_inner_sha256=fixture['oracle_inner_sha256'],
        scenes=len(fixture['scenes']),top_level_operations=sum(len(scene['query']['operations'])for scene in fixture['scenes']),
        recorded_operations=len(observed),operations=dict(collections.Counter(record['op']for record in observed)),
        callback_visits=sum(len(record.get('visits',[]))for record in observed),
        scope='Original EntityTickList and Int2ObjectLinkedOpenHashMap add/remove/contains/forEach; '
              'active/passive map identity, insertion order, stable entity handles, entity-ID mutation, '
              'copy-on-write snapshots, nested iteration and callback failure recovery. '
              'Original ItemEntity objects are allocated without constructors and only their Entity.id '
              'environment field is supplied; no Minecraft methods or classes are rewritten.',
        tests=tests,source_sha256={name:sha(ROOT/name)for name in ['src/entity_tick.rs','tools/entity_tick_oracle.java','tools/entity_tick_generate.py','fixtures/entity_tick_vectors.json']},
        not_verified=['ServerLevel eligibility/removed/passenger/freeze callback behavior','EntitySectionStorage spatial neighbor order','Mixed entity runtime integration'],
        reproduce='python3 tools/entity_tick_check.py --release')
    (ROOT/'evidence/entity-tick-conformance.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))

if __name__=='__main__':main()
