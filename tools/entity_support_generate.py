#!/usr/bin/env python3
"""Generate original supporting-block selection and candidate-order fixtures."""
from pathlib import Path
import hashlib,json,math,os,random,struct,subprocess
ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle'
JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin');SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def db(v):return struct.pack('>d',v).hex()
def fb(v):return struct.pack('>f',v).hex()
def dv(*v):return [db(x)for x in v]
def generate():
    rows=[];rng=random.Random(0x1217675a770);catalog=json.loads((ROOT/'fixtures/runtime_collision_shapes-1.21.json').read_text());blocks=json.loads((ROOT/'fixtures/placement-1.21.json').read_text())['blocks'];ids={b['name'].split(':',1)[1]:b['default']for b in blocks}
    def add(category,blocks,bounds,position=(.5,64.,.5),descending=False,fall=0.):
        legal=[b for b in blocks if -64<=b[1]<=319 and abs(b[0])<=30000000 and abs(b[2])<=30000000]
        rows.append(dict(category=category,query=dict(blocks=legal,bounds=dv(*bounds),position=dv(*position),descending=descending,fall=fb(fall))))
    # Every static full-voxel class and context-dependent shape class supported by
    # RuntimeCollision, excluding animated block-entity piston/shulker shapes.
    seen=set()
    for state,row in enumerate(catalog['states']):
        shape,kind,*_=row
        if kind in (6,7):continue
        if (shape,kind)not in seen:
            seen.add((shape,kind));add('collision-shape-class',[[0,64,0,state]],[-.1,63.9,-.1,1.1,66.1,1.1],(.5,65.,.5))
    for name in ['powder_snow','scaffolding']:
        block=next(b for b in blocks if b['name']=='minecraft:'+name)
        for state in range(block['min'],block['max']+1):
            for feet,down,fall in [(64.,False,0.),(65.,False,0.),(65.,True,0.),(64.8,False,2.5),(64.8,False,2.5000002),(65.,True,10.)]:
                add('context-'+name,[[0,64,0,state]],[-.1,63.99,-.1,1.1,65.01,1.1],(.5,feet,.5),down,fall)
    # Exact symmetric ties over all three comparator axes, shuffled maps and
    # several slabs of query bounds. The original iterator owns visitation order.
    cube=[[x,y,z,1]for x in [-1,0]for y in [63,64]for z in [-1,0]]
    for subset in [[p for p in cube if p[1]==63],[p for p in cube if p[2]==0],[p for p in cube if p[0]==0],cube,[cube[0],cube[-1]]]:
        for position in [(0.,64.,0.),(0.,64.,.5),(.5,64.,0.),(0.,64.5,0.),(-.5,63.5,-.5)]:
            for _ in range(5):
                shuffled=subset.copy();rng.shuffle(shuffled);add('exact-distance-ties',shuffled,[-1.1,62.99,-1.1,1.1,65.01,1.1],position)
    for y in [-65,-64,-1,0,63,64,319,320]:
        for epsilon in [-1e-6,-1e-7,-1e-8,-0.,0.,1e-8,1e-7,1e-6]:
            add('thin-support-boundaries',[[0,y-1,0,1],[1,y-1,0,1]],[-.3,y-1e-6,.2,.7+epsilon,float(y),.8],(.7,y,.5))
            add('top-face-boundaries',[[0,y,0,1]],[-.3,y+1.+epsilon,.2,.7,y+1.+epsilon+1e-6,.8],(.2,y+1.+epsilon,.5))
    for x,z in [(-30000000,-30000000),(-30000000,30000000),(30000000,-30000000),(30000000,30000000),(-17,-17),(-16,-16),(-1,-1),(15,15),(16,16)]:
        add('signed-world-boundaries',[[x,63,z,1],[x+1,63,z,1]],[x-.3,63.999999,z-.3,x+1.3,64.,z+.3],(x+.5,64.,z))
    for position in [(1e154,64.,.5),(1e155,64.,.5),(-1e308,64.,.5),(1e308,1e308,1e308),(float('nan'),64.,.5),(float('inf'),64.,.5),(-float('inf'),64.,.5),(0.,float('nan'),0.)]:
        add('method-level-extreme-distance',[[0,63,0,1],[1,63,0,1]],[-.3,63.999999,-.3,1.3,64.,.3],position)
    valid=[i for i,row in enumerate(catalog['states'])if row[1]not in (6,7)]
    for _ in range(1000):
        x,y,z=rng.choice([-1000,-17,-16,-1,0,15,16,1000]),rng.choice([-64,0,64,319]),rng.choice([-1000,-17,-16,-1,0,15,16,1000]);blocks=[[x+rng.randrange(-2,3),y+rng.randrange(-2,3),z+rng.randrange(-2,3),rng.choice(valid)]for _ in range(rng.randrange(0,22))]
        px,py,pz=x+rng.choice([-.5,0.,.5,1.]),y+rng.choice([0.,.125,.5,1.]),z+rng.choice([-.5,0.,.5,1.]);width=rng.choice([1e-8,1e-7,.125,.6,1.,2.,4.]);height=rng.choice([1e-8,1e-6,.125,.5,1.,2.]);add('random-scenes',blocks,[px-width/2,py-height,pz-width/2,px+width/2,py,pz+width/2],(px,py,pz),rng.choice([False,True]),rng.choice([0.,2.5,2.5000002,10.]))
    add('empty-world',[],[-.3,63.999999,-.3,.3,64.,.3]);add('no-hit',[[0,61,0,1]],[-.3,63.999999,-.3,.3,64.,.3])
    return rows
def main():
    jar=ORACLE/'server-inner-1.21.jar';assert hashlib.sha256(jar.read_bytes()).hexdigest()==SHA
    tags=ROOT/'fixtures/placement-block-tags-1.21.json';assert json.loads(tags.read_text())['jar_sha256']==SHA
    runtime=ROOT/'runtime/entity_support';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True);cp=os.pathsep.join(map(str,[classes,jar,*sorted((ORACLE/'libraries').rglob('*.jar'))]));flags=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED'];sources=[ROOT/'tools'/p for p in ['survival_oracle.java','runtime_collision_oracle.java','placement_tags.java','entity_support_oracle.java']]
    subprocess.run([JDK/'javac',*flags,'-proc:none','-cp',cp,'-d',classes,*sources],check=True,capture_output=True)
    rows=generate();transcript=''.join(json.dumps(r['query'],separators=(',',':'))+'\n'for r in rows)
    run=subprocess.run([JDK/'java',*flags,'-Dmc121.blocktags='+str(tags),'-Xmx1G','-cp',cp,'oracle.entity_support_oracle'],input=transcript,text=True,capture_output=True);(runtime/'oracle.log').write_text(run.stdout+run.stderr)
    if run.returncode:raise RuntimeError((run.stdout+run.stderr)[-6500:])
    assert 'SOURCE:file:'+str(jar)in run.stdout
    outputs=[json.loads(line.split('ORACLE:',1)[1])for line in run.stdout.splitlines()if line.startswith('ORACLE:')];assert len(outputs)==len(rows),(len(outputs),len(rows))
    for row,value in zip(rows,outputs):row['expected']=value
    path=ROOT/'fixtures/entity_support_vectors.json';path.write_text(json.dumps(dict(minecraft='1.21',oracle_inner_sha256=SHA,cases=rows),separators=(',',':'))+'\n');counts={}
    for row in rows:counts[row['category']]=counts.get(row['category'],0)+1
    report=dict(ok=True,minecraft='1.21',oracle_inner_sha256=SHA,actual_methods=['dcg.g(bsr,ewx): unchanged CollisionGetter.findSupportingBlock','new dca(...,BiFunction copyPosition): unchanged BlockCollisions candidate positions and order'],queries=len(rows),candidate_positions=sum(len(o['positions'])for o in outputs),selected_support=sum(o['support']is not None for o in outputs),categories=counts,vectors_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),block_tags_sha256=hashlib.sha256(tags.read_bytes()).hexdigest(),trace_sha256=hashlib.sha256('\n'.join(json.dumps(v,sort_keys=True,separators=(',',':'))for v in outputs).encode()).hexdigest(),source_sha256={p.name:hashlib.sha256(p.read_bytes()).hexdigest()for p in sources},scope='Original candidate scan, collision context, closest block-center distance and exact Y/Z/X tie selection; generic entity context and finite map-backed world with all chunks available.',not_verified=['Missing/unloaded collision chunks and live entity/world integration','Animated moving-piston and shulker block-entity shapes','Native Rust comparison performed by entity_support owner separately'])
    (ROOT/'evidence/entity-support-oracle.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
