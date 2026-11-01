#!/usr/bin/env python3
"""Replay frozen unchanged-original full item ticks through RuntimeEntities."""
from pathlib import Path
import argparse,hashlib,json,os,re,subprocess
ROOT=Path(__file__).resolve().parents[1]
def main():
    p=argparse.ArgumentParser();p.add_argument('--release',action='store_true');args=p.parse_args()
    sha='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
    inputs=['fixtures/item-full-vectors.json','fixtures/item-water-vectors.json']
    for name in inputs:assert json.loads((ROOT/name).read_text())['oracle_inner_sha256']==sha
    env=os.environ.copy();env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup',CARGO_INCREMENTAL='0')
    cmd=['/root/.local/share/mc-rust-toolchain/cargo/bin/cargo','test','--lib','runtime_entities::original_corpus_tests']
    if args.release:cmd.append('--release')
    cmd+=['--','--nocapture']
    native=subprocess.run(cmd,cwd=ROOT,env=env,text=True,capture_output=True)
    runtime=ROOT/'runtime/runtime-entities';runtime.mkdir(parents=True,exist_ok=True);profile='release'if args.release else'debug';(runtime/(profile+'.log')).write_text(native.stdout+native.stderr)
    if native.returncode:raise RuntimeError((native.stdout+native.stderr)[-7000:])
    assert int(re.search(r'test result: ok\. (\d+) passed',native.stdout).group(1))==2
    comparisons={}
    for line in native.stdout.splitlines():
        for name in ['RUNTIME_ENTITIES_DRY_CONFORMANCE','RUNTIME_ENTITIES_WATER_CONFORMANCE']:
            if line.startswith(name+':'):comparisons[name]=json.loads(line.removeprefix(name+':'))
    assert len(comparisons)==2 and all(r['ok']and r['mismatches']==0 for r in comparisons.values())
    sources=[*inputs,'tools/runtime_entities_tests.rs','tools/runtime_entities_check.py',*['src/'+name+'.rs'for name in ['runtime_entities','runtime_entity_index','entity_tick','entity_spatial','item_entity','item_replication','entity_base','entity_state','entity_move','runtime_item_world','runtime_collision','fluid_physics','entity_support','runtime_world','world','storage','inventory','random']]]
    report=dict(ok=True,kind='native coordinator integration against frozen original-class runtime states',minecraft='1.21',oracle_inner_sha256=sha,profile=profile,native_tests_passed=2,comparisons=comparisons,scope='All2976 original dry/submerged item poststates and82final entity RNG states through authoritative item storage, real ticklist, synchronous section index callbacks and RuntimeItemWorld. Includes30 exact removal ticks. Original harness clearhasImpulse/packDirty actions are explicit; sender tracking is not called by this oracle corpus.',not_verified=['Live client/network and actual server ticket lifecycle','Tracking/publication phase timing in this corpus','Multiple items, player query/collection or inventory effects in this corpus','Water entry/exit, lava/fire ticks and unsupported block callbacks'],source_sha256={s:hashlib.sha256((ROOT/s).read_bytes()).hexdigest()for s in sources})
    (ROOT/'evidence'/('runtime-entities-'+profile+'-conformance.json')).write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps({k:v for k,v in report.items()if k not in ['source_sha256','not_verified','scope']}))
if __name__=='__main__':main()
