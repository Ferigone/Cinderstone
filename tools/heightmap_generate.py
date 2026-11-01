#!/usr/bin/env python3
"""Invoke unchanged Java1.21 Heightmap priming/predicates/update on columns."""
from pathlib import Path
import hashlib,json,os,random,subprocess

ROOT=Path(__file__).resolve().parents[1]
ORACLE=ROOT.parent/'mc121-oracle'
JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
FLAGS=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED']

def main():
    jar=ORACLE/'server-inner-1.21.jar'
    assert hashlib.sha256(jar.read_bytes()).hexdigest()==SHA
    runtime=ROOT/'runtime/heightmap';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True)
    cp=os.pathsep.join(map(str,[classes,jar,*sorted((ORACLE/'libraries').rglob('*.jar'))]))
    compile=subprocess.run([JDK/'javac',*FLAGS,'-proc:none','-cp',cp,'-d',classes,ROOT/'tools/survival_oracle.java',ROOT/'tools/heightmap_oracle.java'],capture_output=True,text=True)
    if compile.returncode:raise RuntimeError(compile.stdout+compile.stderr)
    cmd=[JDK/'java',*FLAGS,'-Xmx1G','-cp',cp,'oracle.heightmap_oracle']
    facts=subprocess.run([*cmd,'facts'],capture_output=True,text=True)
    (runtime/'facts.log').write_text(facts.stdout+facts.stderr)
    if facts.returncode:raise RuntimeError((facts.stdout+facts.stderr)[-6000:])
    masks=json.loads(next(line.removeprefix('PREDICATES:')for line in facts.stdout.splitlines()if line.startswith('PREDICATES:')))
    assert len(masks)==26684
    groups={}
    for state,mask in enumerate(masks):groups.setdefault(mask,[]).append(state)
    representatives=[values[len(values)//2]for values in groups.values()]
    levels=[-64,-63,-1,0,63,64,318,319]
    scenes=[]
    edits=[]
    # Every registered state participates as a new state. Insertion, replacement
    # with an equal-predicate state and removal exercise all update paths while
    # the column becomes empty again after each state, including min/max Y.
    for state,mask in enumerate(masks):
        y=levels[state%len(levels)];same=groups[mask][(state+1)%len(groups[mask])]
        edits.extend([[y,state],[y,same],[y,0]])
    scenes.append(dict(x=0,z=0,initial=[0]*384,edits=edits))
    # Every available predicate group pair is tested at each boundary, with a
    # lower blocker to force scan-and-find as well as scan-to-minimum outcomes.
    for x,z in [(15,15),(7,8),(1,14)]:
        edits=[]
        for y in levels:
            for old in representatives:
                for new in representatives:
                    if y>-64:edits.append([y-1,1])
                    edits.extend([[y,old],[y,new],[y,0]])
                    if y>-64:edits.append([y-1,0])
        scenes.append(dict(x=x,z=z,initial=[0]*384,edits=edits))
    rng=random.Random(0x767121d77)
    for index in range(64):
        # Sparse and dense, with varied air/fluid/leaves/nonmotion predicates.
        initial=[rng.choice(representatives)if rng.random()<(index%8)/8 else 0 for _ in range(384)]
        cutoff=rng.choice([0,1,64,128,255,383,384])
        initial[cutoff:]=[0]*(384-cutoff)
        edits=[]
        for step in range(96):
            y=rng.choice(levels)if step%4==0 else rng.randrange(-64,320)
            state=rng.randrange(26684)if step%3==0 else rng.choice(representatives)
            edits.append([y,state])
        scenes.append(dict(x=index%16,z=(index*7+3)%16,initial=initial,edits=edits))
    # Every padded9bit storage index, including the final partial word, receives
    # a nonzero value retained in final_raw for whole-array position proof.
    for z in range(16):
        for x in range(16):
            y=levels[(x+z)%len(levels)]
            scenes.append(dict(x=x,z=z,initial=[0]*384,edits=[[y,1]]))
    transcript=''.join(json.dumps(scene,separators=(',',':'))+'\n'for scene in scenes)
    java=subprocess.run(cmd,input=transcript,capture_output=True,text=True)
    (runtime/'oracle.log').write_text(java.stdout+java.stderr)
    if java.returncode:raise RuntimeError((java.stdout+java.stderr)[-6000:])
    outputs=[json.loads(line.removeprefix('ORACLE:'))for line in java.stdout.splitlines()if line.startswith('ORACLE:')]
    assert len(outputs)==len(scenes),(len(outputs),len(scenes))
    vectors=[]
    for scene,out in zip(scenes,outputs):
        assert len(scene['edits'])==len(out['steps'])
        assert all(op[:2]==expected[:2]for op,expected in zip(scene['edits'],out['steps']))
        vectors.append(dict(x=scene['x'],z=scene['z'],initial=scene['initial'],**out))
    result=dict(minecraft='1.21',oracle_inner_sha256=SHA,names=['WORLD_SURFACE','OCEAN_FLOOR','MOTION_BLOCKING','MOTION_BLOCKING_NO_LEAVES'],predicate_masks=masks,predicate_groups={str(k):len(v)for k,v in groups.items()},scenes=vectors,scope='Original Heightmap.primeHeightmaps, Types.isOpaque predicates and Heightmap.update invoked unchanged. Oracle-only ChunkAccess provides one real384state column and air in other columns. Steps=[y,state,changed_mask,first_available_heights,block_reads_per_map]. Initial/final raw arrays retain all256column entries and padded9bitstorage.',oracle_source_sha256=hashlib.sha256((ROOT/'tools/heightmap_oracle.java').read_bytes()).hexdigest())
    path=ROOT/'fixtures/heightmap-update-vectors.json';path.write_text(json.dumps(result,separators=(',',':'))+'\n')
    print(json.dumps(dict(file=str(path),scenes=len(scenes),edits=sum(len(s['edits'])for s in scenes),heightmap_update_calls=sum(len(s['edits'])*4 for s in scenes),state_predicates=len(masks)*4,predicate_groups=result['predicate_groups'],bytes=path.stat().st_size,sha256=hashlib.sha256(path.read_bytes()).hexdigest())))

if __name__=='__main__':main()
