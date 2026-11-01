#!/usr/bin/env python3
"""Run actual ItemEntity.tick and inherited Entity methods in a dry generated world."""
from pathlib import Path
import argparse,hashlib,json,os,random,struct,subprocess,zipfile
ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle'
JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
FLAGS=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED']
def fb(v):return struct.pack('>f',v).hex()
def db(v):return struct.pack('>d',v).hex()
def vec(v):return list(map(db,v))
def main():
    p=argparse.ArgumentParser();p.add_argument('--smoke',action='store_true');a=p.parse_args()
    jar=ORACLE/'server-inner-1.21.jar';assert hashlib.sha256(jar.read_bytes()).hexdigest()==SHA
    tags=ROOT/'fixtures/placement-block-tags-1.21.json';tagdata=json.loads(tags.read_text());assert tagdata['jar_sha256']==SHA
    with zipfile.ZipFile(jar)as archive:
        for name,digest in tagdata['source_files'].items():assert hashlib.sha256(archive.read(name)).hexdigest()==digest,name
    runtime=ROOT/'runtime/item-full';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True)
    cp=os.pathsep.join(map(str,[classes,jar,*sorted((ORACLE/'libraries').rglob('*.jar'))]))
    compile=subprocess.run([JDK/'javac',*FLAGS,'-proc:none','-cp',cp,'-d',classes,ROOT/'tools/survival_oracle.java',ROOT/'tools/placement_tags.java',ROOT/'tools/item_full_oracle.java'],capture_output=True,text=True)
    if compile.returncode:raise RuntimeError(compile.stdout+compile.stderr)
    queries=[]
    def add(position,velocity=(0.,0.,0.),blocks=(),**kw):
        q=dict(position=vec(position),velocity=vec(velocity),blocks=list(blocks),entity_id=len(queries)+41,seed=12345+len(queries),tick_count=0,age=0,pickup_delay=40,on_ground=False,fall_distance=fb(0),no_gravity=False,ticks=80,drain_initial_metadata=True,clear_has_impulse=True);q.update(kw);queries.append(q)
    add((8.5,65.,8.5))
    add((8.5,64.,8.5),(.2,0.,0.),on_ground=True)
    add((-.001,64.,-.001),(-.1,.1,-.1))
    add((-16.,80.,15.999),(1.,-.2,-.4))
    add((8.5,63.,8.5))
    add((.5,319.125,.5),(.03,-.5,.03))
    add((.5,-128.00000000000003,.5))
    add((8.5,64.,8.5),(.3,.1,.02),blocks=[[10,y,z,1]for y in range(64,67)for z in range(6,12)])
    # Constructor ItemStack and noGravity setter dirtiness are independently
    # visible when the host has not drained an initial tracker snapshot yet.
    add((8.5,65.,8.5),drain_initial_metadata=False,ticks=4)
    add((8.5,64.5,8.5),no_gravity=True,drain_initial_metadata=False,ticks=4)
    r=random.Random(0x121767f011)
    while len(queries)<64:
        i=len(queries);x,z=r.choice([-100.,-16.001,-16.,-15.999,-1.,-.5,-.001,0.,.001,.5,1.,15.999,16.,100.]),r.choice([-16.,-1.,-.001,0.,.5,1.,15.999,16.]);y=r.choice([63.,63.5,63.999,64.,64.001,65.,70.,80.,319.]);velocity=[r.choice([-1.,-.2,-.01,-1e-7,-0.,0.,1e-7,.01,.2,1.]),r.choice([-1.,-.1,-1e-7,-0.,0.,.1,.3,1.]),r.choice([-.2,-.01,-0.,0.,.01,.2])];blocks=[]
        if i%3==0:
            bx=int(x//1)+r.choice([-1,0,1]);bz=int(z//1)+r.choice([-1,0,1]);blocks=[[bx,by,bz,1]for by in range(64,r.choice([65,66,67]))]
        add((x,y,z),velocity,blocks,seed=r.randrange(-2**63,2**63),tick_count=r.choice([-2147483648,-1,0,1,3,39,40,2147483647]),age=r.choice([-32768,-6000,0,5998,5999]),pickup_delay=r.choice([-1,0,1,40,32767]),on_ground=bool(i%2),fall_distance=fb(r.choice([-.0,0.,.1,2.,100.])),no_gravity=i%7==0)
    if a.smoke:queries=queries[:8]
    java=subprocess.run([JDK/'java',*FLAGS,'-Xmx1G','-cp',cp,'oracle.item_full_oracle',tags],input=''.join(json.dumps(q,separators=(',',':'))+'\n'for q in queries),text=True,capture_output=True)
    (runtime/'oracle.log').write_text(java.stdout+java.stderr)
    if java.returncode:raise RuntimeError((java.stdout+java.stderr)[-9000:])
    outputs=[json.loads(line.removeprefix('ORACLE:'))for line in java.stdout.splitlines()if line.startswith('ORACLE:')];assert len(outputs)==len(queries),(len(outputs),len(queries))
    result=dict(minecraft='1.21',oracle_inner_sha256=SHA,terrain='FlatWorld exact fixture: stone−64..60,dirt61..62,grass63,air elsewhere; sparse block overrides.',entity_class='cjh (exact original class; no subclasses or method overrides)',scope='Original ItemEntity.tick, Entity.baseTick, Entity.move, water/lava scans, collision shapes, support selection, landing/inside-block/speed effects on dry generated flat chunks and sparse ordinary stone obstacles. Empty neighbouring entity and game-listener world. Host advances oldposition/tickCount and clears hasImpulse before each tick; dirtymetadata drained after each tick. Creation RNG separately proven; entity RNG deliberately injected after actual constructor.',not_verified=['Full running ServerLevel/ChunkMap network and entity ordering','Multiple-item merging query order, player pickup, entities and vehicles','Fluid scenes, hazards, special block callbacks, portals, dynamic block entities','Player/world/item creation RNG sequence before injected per-entity seed'],scenes=[dict(query=q,**o)for q,o in zip(queries,outputs)],oracle_source_sha256=hashlib.sha256((ROOT/'tools/item_full_oracle.java').read_bytes()).hexdigest(),block_tags_sha256=hashlib.sha256(tags.read_bytes()).hexdigest())
    path=ROOT/'fixtures/item-full-vectors.json';path.write_text(json.dumps(result,separators=(',',':'))+'\n')
    print(json.dumps(dict(file=str(path),scenes=len(queries),states=sum(len(out['states'])for out in outputs),bytes=path.stat().st_size,sha256=hashlib.sha256(path.read_bytes()).hexdigest())))
if __name__=='__main__':main()
