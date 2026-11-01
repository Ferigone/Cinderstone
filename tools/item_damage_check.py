#!/usr/bin/env python3
"""Original hurt/destruction replay followed by bounded native comparison."""
from pathlib import Path
import hashlib,json,os,re,subprocess,sys
ROOT=Path(__file__).resolve().parents[1]
def run(command,env=None):
    result=subprocess.run(command,cwd=ROOT,env=env,text=True,capture_output=True)
    if result.returncode:raise RuntimeError((result.stdout+result.stderr)[-7000:])
    return result
def main():
    original=run([sys.executable,'tools/item_hurt_check.py'])
    destruction=run([sys.executable,'tools/item_destroyed_check.py'])
    env=os.environ.copy();env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup',CARGO_INCREMENTAL='0')
    cargo='/root/.local/share/mc-rust-toolchain/cargo/bin/cargo'
    native=run([cargo,'test','--lib','item_damage::tests','--','--nocapture'],env)
    runtime=ROOT/'runtime/item-damage';runtime.mkdir(parents=True,exist_ok=True)
    (runtime/'native.log').write_text(native.stdout+native.stderr)
    (runtime/'original.log').write_text(original.stdout+destruction.stdout)
    assert int(re.search(r'test result: ok\. (\d+) passed',native.stdout).group(1))==3
    reports={}
    for line in native.stdout.splitlines():
        for name in ['ITEM_DAMAGE_CONFORMANCE','ITEM_DESTRUCTION_CONFORMANCE']:
            if line.startswith(name+':'):reports[name]=json.loads(line.removeprefix(name+':'))
    assert len(reports)==2 and all(row['ok']and row['mismatches']==0 for row in reports.values())
    report=dict(ok=True,minecraft='1.21',kind='bounded native differential conformance',oracle_inner_sha256='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01',native_tests_passed=3,damage=reports['ITEM_DAMAGE_CONFORMANCE'],destruction=reports['ITEM_DESTRUCTION_CONFORMANCE'],not_verified=['Live server damage source registry and game-event listener integration','Present-CONTAINER destruction/spawning, including shulker defaults and custom contents','BundleItem destruction','Vehicles/passengers and complete generic entity removal lifecycle','Full lava/fire item tick and tracker/network integration'])
    sources=['src/item_damage.rs','src/inventory.rs','src/entity_state.rs','tools/item_damage_tests.rs','tools/item_damage_check.py','tools/item_hurt_oracle.java','tools/item_hurt_check.py','tools/item_destroyed_oracle.java','tools/item_destroyed_check.py','tools/item_destroyed_bytecode.txt','fixtures/item-hurt-vectors.json','fixtures/item-destroyed-facts.json']
    report['source_sha256']={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in sources}
    (ROOT/'evidence/item-damage-conformance.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps({k:v for k,v in report.items()if k not in ['source_sha256','not_verified']}))
if __name__=='__main__':main()
