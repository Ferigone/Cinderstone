#!/usr/bin/env python3
"""Check selected/creative drops against unchanged original Minecraft1.21."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT=Path(__file__).resolve().parents[1]


def sha(name):return hashlib.sha256((ROOT/name).read_bytes()).hexdigest()


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--regenerate',action='store_true')
    parser.add_argument('--release',action='store_true')
    args=parser.parse_args()
    if args.regenerate:subprocess.run([sys.executable,ROOT/'tools/selected_drop_generate.py'],cwd=ROOT,check=True)
    oracle={kind:json.loads((ROOT/f'evidence/{kind}-drop-oracle.json').read_text())for kind in ['selected','creative']}
    for kind,proof in oracle.items():
        assert proof['ok']and not proof['original_classes_modified']
        assert proof['fixture_sha256']==sha(f'fixtures/{kind}_drop_vectors.json')
        for name,digest in proof['source_sha256'].items():assert sha(name)==digest,(kind,name)
    env=os.environ.copy();env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup',CARGO_INCREMENTAL='0')
    env['PATH']=env['CARGO_HOME']+'/bin:'+env.get('PATH','')
    runtime=ROOT/'runtime/selected-drop';runtime.mkdir(parents=True,exist_ok=True)
    checks={}
    for profile in ['debug','release']if args.release else ['debug']:
        checks[profile]={}
        for module,required in [
            ('container','actual_java_selected_drop_updates_prediction_before_drop_without_ack'),
            ('creative_drop','actual_java_creative_negative_slot_spam_gate_and_full_listener_tick'),
        ]:
            command=[str(Path(env['CARGO_HOME'])/'bin/cargo'),'test']+(['--release']if profile=='release'else[])+['--lib',module+'::tests','--','--nocapture']
            result=subprocess.run(command,cwd=ROOT,env=env,text=True,capture_output=True)
            output=result.stdout+result.stderr;(runtime/f'native-{module}-{profile}.log').write_text(output)
            if result.returncode:raise RuntimeError(output[-10000:])
            assert f'test {module}::tests::{required} ... ok'in result.stdout
            count=re.search(r'test result: ok\. (\d+) passed',result.stdout);assert count
            checks[profile][module]=dict(passed=int(count.group(1)),failed=0)
    sources=['src/container.rs','src/creative_drop.rs','src/inventory.rs','src/protocol.rs',
             'tools/selected_drop_tests.rs','tools/selected_drop_oracle.java','tools/selected_drop_generate.py',
             'tools/selected_drop_check.py','tools/container_oracle.java','tools/inventory_normalization_oracle.java',
             'fixtures/selected_drop_vectors.json','fixtures/creative_drop_vectors.json']
    hashes={name:sha(name)for name in sources}
    for kind,proof in oracle.items():
        common=dict(ok=True,minecraft='1.21',protocol=767,oracle_inner_sha256=proof['oracle_inner_sha256'],
            cases=proof['cases'],operations=proof['operations'],tests=checks,mismatches=0,source_sha256=hashes,
            reproduce='python3 tools/selected_drop_check.py --regenerate --release')
        if kind=='selected':
            report=dict(**common,prediction_before_drop_callbacks=proof['prediction_before_drop_callbacks'],
                empty_callbacks=proof['empty_callbacks'],nonempty_drops=proof['dropped_nonempty'],deferred_packets=proof['deferred_packets'],
                scope='Original ServerPlayer.drop(boolean)/Inventory.removeFromSelected, InventoryMenu selected-slot mapping and setRemoteSlot before final environmental Player.drop. Compares every native slot/remote slot/cursor/state ID, actual drop arguments, no immediate ack, later original broadcastChanges + original ServerPlayer synchronizer payload bytes. Covers all nine selected hotbar positions, Q/CtrlQ, empty calls, unrelated pending slot/cursor changes and state-ID wrap.',
                not_verified=['Other active menu layouts outside window0','Final ItemEntity creation, trajectory, stats and advancement callbacks','Live player-action dispatch integration'])
        else:
            report=dict(**common,drop_callbacks=proof['drop_callbacks'],empty_callbacks=proof['empty_callbacks'],
                tick_operations=proof['tick_operations'],disabled_item_attempts=proof['disabled_item_attempts'],
                scope='Original creative packet handler and full listener tick execute without rewriting. Native gate compares strict spam<200, +20 and positive-only -1 decay after every operation, all negative i16 sentinels, zero/positive boundaries, nonempty count/maxStackSize, empty attempts, and around=false/traceItem=true wrapper arguments. Creative and per-stack feature checks are explicit host preconditions; empty stacks remain enabled in the original empty-feature world. Negative drops send no ack.',
                not_verified=['World block-entity coordinate/component replacement before gate','Native positive creative slot assignment is tested separately','Final ItemEntity creation, trajectory, stats and advancement callbacks','Live session tick and packet dispatch integration'])
        (ROOT/f'evidence/{kind}-drop-conformance.json').write_text(json.dumps(report,indent=2)+'\n')
        print(json.dumps({key:value for key,value in report.items()if key!='source_sha256'},indent=2))


if __name__=='__main__':main()
