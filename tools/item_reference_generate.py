#!/usr/bin/env python3
"""Execute original item stack reference/setter/merge dirty-data behavior."""
from pathlib import Path
import argparse,hashlib,json,os,re,subprocess

ROOT=Path(__file__).resolve().parents[1]
ORACLE=ROOT.parent/'mc121-oracle'
JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--check-rust',action='store_true');args=parser.parse_args()
    jar=ORACLE/'server-inner-1.21.jar';assert hashlib.sha256(jar.read_bytes()).hexdigest()==SHA
    runtime=ROOT/'runtime/item-reference';runtime.mkdir(parents=True,exist_ok=True)
    cp=os.pathsep.join(map(str,[runtime,jar,*sorted((ORACLE/'libraries').rglob('*.jar'))]))
    flags=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED']
    subprocess.run([JDK/'javac',*flags,'-proc:none','-cp',cp,'-d',runtime,*[ROOT/'tools'/name for name in ['survival_oracle.java','runtime_collision_oracle.java','item_entity_oracle.java','item_reference_oracle.java']]],check=True,capture_output=True,text=True)
    cases=[]
    for count in [1,2,63,64,70,99]:
        stack={'id':'minecraft:stone','count':count,'components':{'minecraft:max_stack_size':99}}
        for actions in [['same_reference'],['equal_copy'],['count_in_place'],['equal_copy','count_in_place'],['gravity_on','gravity_off'],['same_reference','gravity_on','gravity_off'],['count_in_place','same_reference']]:
            cases.append(dict(op='reference',left=stack,count=3,actions=actions))
    for maximum in [16,64,99]:
        for name in ['minecraft:stone','minecraft:dirt','minecraft:ender_pearl']:
            for left in [1,3,10,16,20,63,64,70,80,98]:
                for right in [1,3,10,16,20,63,64,70,80,98]:
                    if left<=maximum and right<=maximum:
                        stack=lambda count:dict(id=name,count=count,components={'minecraft:max_stack_size':maximum})
                        cases.append(dict(op='merge',left=stack(left),right=stack(right)))
    for left in [1,3,10,16,32,63]:
        for right in [1,3,10,16,32,63]:
            cases.append(dict(op='merge',left=dict(id='minecraft:stone',count=left),right=dict(id='minecraft:dirt',count=right)))
    result=subprocess.run([JDK/'java',*flags,'-cp',cp,'oracle.item_reference_oracle'],input=''.join(json.dumps(q)+'\n'for q in cases),capture_output=True,text=True)
    (runtime/'oracle.log').write_text(result.stdout+result.stderr)
    if result.returncode:raise RuntimeError((result.stdout+result.stderr)[-6000:])
    outputs=[json.loads(line.removeprefix('ORACLE:'))for line in result.stdout.splitlines()if line.startswith('ORACLE:')]
    assert len(outputs)==len(cases)
    stacks={}
    for case in cases:
        for key in ['left','right']:
            if key in case:stacks.setdefault(json.dumps(case[key],sort_keys=True),None)
    encoded=subprocess.run(['node','tools/crafting_stack_wire.mjs'],input=''.join(name+'\n'for name in stacks),capture_output=True,text=True,check=True)
    wire=[json.loads(line)for line in encoded.stdout.splitlines()]
    assert len(wire)==len(stacks)
    stacks=dict(zip(stacks,[row['slot']for row in wire]))
    for case in cases:
        for key in ['left','right']:
            if key in case:case[key+'_wire']=stacks[json.dumps(case[key],sort_keys=True)]
    data=dict(minecraft='1.21',oracle_inner_sha256=SHA,scope='Original SynchedEntityData packDirty: ItemEntity constructor,setItem same reference/equal copy,count in-place,basegravity restore,and merges. Merge contexts use0age/delay andnoowners.',cases=[dict(input=q,expected=a)for q,a in zip(cases,outputs)])
    output=ROOT/'fixtures/item-reference-vectors.json';output.write_text(json.dumps(data,separators=(',',':'))+'\n')
    if args.check_rust:
        env=os.environ.copy();env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup',CARGO_INCREMENTAL='0')
        native=subprocess.run(['/root/.local/share/mc-rust-toolchain/cargo/bin/cargo','test','--lib','item_entity::tests','--','--nocapture'],cwd=ROOT,env=env,text=True,capture_output=True)
        (runtime/'rust.log').write_text(native.stdout+native.stderr)
        if native.returncode:raise RuntimeError((native.stdout+native.stderr)[-7000:])
        passed=int(re.search(r'test result: ok\. (\d+) passed',native.stdout).group(1));assert passed==5
        reports=[json.loads(line.removeprefix('ITEM_REFERENCE_CONFORMANCE:'))for line in native.stdout.splitlines()if line.startswith('ITEM_REFERENCE_CONFORMANCE:')]
        assert len(reports)==1
        report=reports[0]
        factory=[json.loads(line.removeprefix('ITEM_DROP_FACTORY_CONFORMANCE:'))for line in native.stdout.splitlines()if line.startswith('ITEM_DROP_FACTORY_CONFORMANCE:')]
        assert len(factory)==1 and factory[0]['differences']==0
        report['drop_factory']=factory[0]
        report.update(minecraft='1.21',oracle_inner_sha256=SHA,native_tests_passed=passed,kind='original-class differential conformance',not_verified=['Live server publication and cached tracker metadata integration','Player pickup side effects and spatial entity ordering','Reference identity of arbitrary directly mutated component objects'])
        sources=['src/item_entity.rs','src/entity_state.rs','src/inventory.rs','src/metadata.rs','tools/item_reference_oracle.java','tools/item_reference_generate.py','fixtures/item-reference-vectors.json','tools/item_entity_merge_vectors.json','tools/item_entity_physics_vectors.json','tools/item_entity_drop_vectors.json']
        report['source_sha256']={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in sources}
        (ROOT/'evidence/item-reference-conformance.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(dict(ok=True,cases=len(cases),file=str(output),sha256=hashlib.sha256(output.read_bytes()).hexdigest())))

if __name__=='__main__':main()
