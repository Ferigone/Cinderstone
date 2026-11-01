#!/usr/bin/env python3
"""Generate differential stable-light vectors using unchanged signed Java1.21."""
from pathlib import Path
import argparse,hashlib,json,os,random,re,shutil,subprocess
ROOT=Path(__file__).resolve().parents[1]
ORACLE=ROOT.parent/'mc121-oracle'
JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')

def main():
    parser=argparse.ArgumentParser();parser.add_argument("--skip-rust",action="store_true");args=parser.parse_args()
    jar=ORACLE/'server-inner-1.21.jar'; sha=hashlib.sha256(jar.read_bytes()).hexdigest()
    assert sha=='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
    runtime=ROOT/'runtime/lighting';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True)
    cp=os.pathsep.join(map(str,[classes,jar,*sorted((ORACLE/'libraries').rglob('*.jar'))]))
    subprocess.run([JDK/'javac','-proc:none','-cp',cp,'-d',classes,ROOT/'tools/lighting_oracle.java'],check=True,capture_output=True)
    subprocess.run([JDK/'java','-cp',cp,'oracle.lighting_oracle','extract',ROOT/'fixtures/light-1.21.json'],check=True)
    facts=json.loads((ROOT/'fixtures/light-1.21.json').read_text()); metadata=json.loads((ROOT/'fixtures/items-1.21.json').read_text())['blocks']
    ids={k:v['default_state'] for k,v in metadata.items()}
    unique=list(dict.fromkeys(facts['states'])); representatives=[facts['states'].index(v) for v in unique]
    rng=random.Random(0x121767); scenes=[]
    for case in range(11):
        min_y=-16 if case%2 else 0;height=32
        chunks=[(x,z) for z in range(-1,2) for x in range(-1,2)]
        blocks={}
        def add(x,y,z,s):blocks[x,y,z]=s
        if case==0:pass
        elif case==1:add(8,8,8,ids['glowstone'])
        elif case==2:
            for x in range(-8,25):
                for z in range(-8,25):add(x,8,z,ids['stone'])
            add(8,8,8,0);add(8,6,8,ids['torch'])
        elif case==3:
            for y in range(min_y,min_y+height):
                for z in range(-8,25):add(0,y,z,ids['stone'])
            add(-1,0,8,ids['glowstone'])
        elif case==4:
            for x in range(-8,25):
                for z in range(-8,25):add(x,8,z,ids['oak_slab'])
            add(8,6,8,ids['sea_lantern'])
        elif case==5:
            for x in range(-7,24):
                for z in range(-7,24):add(x,8,z,ids['water'])
            add(8,6,8,ids['lava'])
        else:
            for _ in range(220):add(rng.randrange(-7,24),rng.randrange(min_y,min_y+height),rng.randrange(-7,24),rng.choice(representatives))
            for x in range(0,16):
                for z in range(0,16):add(x,4,z,rng.choice([ids['oak_stairs'],ids['stone_slab'],ids['glass'],ids['oak_leaves'],ids['stone']]))
        query=[(x,y,z) for y in range(min_y,min_y+height,3) for z in range(-2,19,2) for x in range(-2,19,2)]
        query+=list(blocks)
        updates=[[]]
        for phase in range(3):
            batch=[]
            for _ in range(16):batch.append([rng.randrange(-2,19),rng.randrange(min_y,min_y+height),rng.randrange(-2,19),rng.choice(representatives+[0]*50)])
            if phase==1:batch+=[[x,y,z,0]for x,y,z in list(blocks)[::max(1,len(blocks)//16)]]
            updates.append(batch)
        y=min_y+height//2
        updates += [[[0,y,0,ids['glowstone']]],[[0,y,0,ids['stone']]],[[0,y,0,0]]]
        if case==0:
            updates += [[[23,y,0,ids['sea_lantern']]],[[7,y,0,ids['stone']]],[[7,y,0,0]],[[23,y,0,0]]]
        scenes.append({'sky_enabled':case!=10,'min_y':min_y,'height':height,'chunks':chunks,'blocks':[[*p,s] for p,s in blocks.items()],'updates':updates,'queries':query})
    inputs=runtime/'scenes.jsonl';inputs.write_text(''.join(json.dumps(q,separators=(',',':'))+'\n'for q in scenes))
    run=subprocess.run([JDK/'java','-Xmx1G','-cp',cp,'oracle.lighting_oracle','scene',inputs],text=True,capture_output=True)
    (runtime/'oracle.log').write_text(run.stdout+run.stderr)
    if run.returncode:raise RuntimeError(run.stderr[-4000:])
    outputs=[json.loads(line.split('ORACLE:',1)[1]) for line in run.stdout.splitlines() if 'ORACLE:'in line]
    assert len(outputs)==len(scenes),(len(outputs),len(scenes))
    vectors={'minecraft':'1.21','oracle_inner_sha256':sha,'scenes':[{**q,'expected':expected}for q,expected in zip(scenes,outputs)]}
    (ROOT/'tools/lighting_vectors.json').write_text(json.dumps(vectors,separators=(',',':'))+'\n')
    report={'minecraft':'1.21','oracle_inner_sha256':sha,'state_count':len(facts['states']),'unique_state_facts':len(facts['rows']),'face_shapes':facts['face_count'],'scenes':len(scenes),'edits':sum(sum(map(len,s['updates']))for s in scenes),'value_pairs':sum(len(q['queries'])*len(q['updates'])for q in scenes),'note':'Vectors generated; run Rust lighting module differential test to establish match. Stable values, full bounded loaded domains; Java section publication ordering not covered.'}
    for field,path in [('facts_sha256',ROOT/'fixtures/light-1.21.json'),('vectors_sha256',ROOT/'tools/lighting_vectors.json'),('native_source_sha256',ROOT/'src/lighting.rs')]:report[field]=hashlib.sha256(path.read_bytes()).hexdigest()
    report['uncached_world_context_checks']=facts['uncached_world_context_checks']
    if not args.skip_rust:
        cargo=shutil.which('cargo') or '/root/.local/share/mc-rust-toolchain/cargo/bin/cargo'
        env=os.environ.copy()
        if Path('/root/.local/share/mc-rust-toolchain/rustup').exists():env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup')
        command=[cargo,'test','--lib','lighting::tests','--','--nocapture']
        checked=subprocess.run(command,cwd=ROOT,env=env,text=True,capture_output=True)
        (runtime/'rust.log').write_text(checked.stdout+checked.stderr)
        if checked.returncode:raise RuntimeError((checked.stdout+checked.stderr)[-6000:])
        report.update(rust_stable_value_matches=True,mismatches=0,rust_module_tests_passed=int(re.search(r'test result: ok\. (\d+) passed',checked.stdout).group(1)),incremental_updates=True,single_interior_edit_column_budget=481,note='All stable values match unchanged Java1.21, including no-skylight dimensions and cached incremental boundary-source preservation. Java asynchronous section publication/scheduling is not covered.')
    (ROOT/'tools/lighting_conformance.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
