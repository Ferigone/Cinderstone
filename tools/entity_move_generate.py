#!/usr/bin/env python3
"""Differential fixtures from unchanged Java1.21 Entity.move in dry contexts."""
from pathlib import Path
import hashlib,json,math,os,random,struct,subprocess

ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle'
JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def db(v):return struct.pack('>d',v).hex()
def fb(v):return struct.pack('>f',v).hex()
def dv(*v):return [db(x)for x in v]
def base():return dict(position=dv(0.,64.,0.),velocity=dv(.1,-.3,.2),moves=[dict(movement=dv(1.,-.2,0.))])
def floor():return [dv(-10.,63.,-10.,10.,64.,10.)]
def generate():
    rows=[];r=random.Random(0x121767e1717)
    def add(category,q):rows.append(dict(category=category,query=q))
    moves=[(0.,0.,0.),(-0.,0.,-0.),(1.,2.,3.),(-1.,-2.,-3.),(1e-9,0.,0.),(math.sqrt(1e-7),0.,0.),(math.nextafter(math.sqrt(1e-7),math.inf),0.,0.),(.51,.51,.51)]
    for kind in range(5):
        for movement in moves:
            for stuck in [(0.,0.,0.),(.1,.2,.3),(1.,1.,1.)]:
                q=base();q.update(no_physics=True,ground=True,horizontal_collision=True,vertical_collision=True,vertical_collision_below=True,minor_collision=True,stuck=dv(*stuck),supporting=[0,63,0],on_ground_no_blocks=True,was_on_fire=True,fire_ticks=1,piston_deltas=dv(.2,.3,.4));q['moves']=[dict(kind=kind,movement=dv(*movement))];add('no-physics',q)
    stuck_values=[(0.,0.,0.),(-0.,0.,0.),(.0001,0.,0.),(math.sqrt(1e-7),0.,0.),(math.nextafter(math.sqrt(1e-7),math.inf),0.,0.),(.1,.2,.3),(1.,1.,1.),(-.5,2.,-1.)]
    for stuck in stuck_values:
        for movement in moves:
            for kind in range(5):
                q=base();q['stuck']=dv(*stuck);q['moves']=[dict(kind=kind,movement=dv(*movement)),dict(kind=0,movement=dv(.25,0.,-.25))];add('stuck-speed-reset',q)
    piston_inputs=[(1.,0.,0.),(.51,0.,0.),(.5,0.,0.),(.01,0.,0.),(1e-5,0.,0.),(-.01,0.,0.),(-1.,0.,0.),(0.,1.,0.),(0.,0.,-1.),(1.,1.,1.),(0.,.1,.1),(-0.,0.,-0.)]
    for initial in [(0.,0.,0.),(.5,0.,0.),(-.5,.5,-.5)]:
        for old_time,time in [(0,0),(0,1),(-1,0),(9223372036854775807,-9223372036854775808)]:
            for axis in range(3):
                q=base();q.update(piston_deltas=dv(*initial),piston_time=old_time,game_time=time);q['moves']=[]
                for j,value in enumerate(piston_inputs*2):
                    value=value[axis:]+value[:axis];q['moves'].append(dict(kind=2,movement=dv(*value),game_time=time if j<len(piston_inputs)else time+1))
                add('piston-sequences',q)
    scenes={
        'free':[], 'floor':floor(),
        'wall':[dv(1.,60.,-10.,2.,70.,10.)],
        'corner':[dv(1.,60.,-10.,2.,70.,10.),dv(-10.,60.,1.,10.,70.,2.)],
        'halfstep':floor()+[dv(1.,64.,-.5,2.,64.5,.5)],
        'fullstep':floor()+[dv(1.,64.,-.5,2.,65.,.5)],
        'low-roof':floor()+[dv(-2.,65.5,-2.,2.,66.,2.)],
    }
    for scene,boxes in scenes.items():
        for ground in [False,True]:
            for step in [0.,.5,.6,1.]:
                for movement in [(1.,-.2,0.),(1.,0.,0.),(1.,.5,.5),(-1.,-1.,-1.),(0.,-1.,0.),(0.,1.,0.),(.7,-1e-8,.7),(.700000001,0.,0.)]:
                    q=base();q.update(blocks=boxes,ground=ground,step=fb(step),supports=[[0,63,0]]);q['moves']=[dict(movement=dv(*movement))];add('collision-'+scene,q)
    for support in [None,[0,63,0],[0,62,0],[2,63,2]]:
        for no_blocks in [False,True]:
            for answers in [[],[None],[None,None],[None,[1,63,0]],[[0,63,0]]]:
                q=base();q.update(blocks=floor(),supporting=support,on_ground_no_blocks=no_blocks,supports=answers);q['moves']=[dict(movement=dv(1.,-.2,0.)),dict(movement=dv(0.,.1,0.))];add('supporting-block-query',q)
    # State facts come from the original default-tag-bound registry. State wrappers
    # preserve tag/is(Block)/class predicates and delegate block callbacks to the
    # original actual block implementation.
    blocks=json.loads((ROOT/'fixtures/placement-1.21.json').read_text())['blocks'];ids={b['name'].split(':',1)[1]:b['default']for b in blocks}
    for name in ['air','stone','oak_fence','cobblestone_wall','oak_fence_gate','water','bubble_column','powder_snow']:
        for offset in [0,1,2]:
            for answer in [None,[0,63-offset,0]]:
                q=base();q.update(blocks=floor(),default_state=ids[name],supporting=[0,63-offset,0],supports=[answer],ground=True);q['moves']=[dict(movement=dv(0.,-.2,0.))];add('on-position-'+name,q)
    for foot in [ids['air'],ids['stone'],ids['water'],ids['bubble_column'],ids['soul_sand'],ids['honey_block']]:
        for below in [ids['stone'],ids['soul_sand'],ids['honey_block']]:
            for velocity in [(1.,-.3,-1.),(-0.,-0.,0.)]:
                q=base();q.update(position=dv(.5,64.,.5),velocity=dv(*velocity),default_state=below,world_states={'0,64,0':foot});q['moves']=[dict(movement=dv(0.,0.,0.))];add('block-speed-factor',q)
    for block in ['stone','slime_block','honey_block','soul_sand']:
        for suppress in [False,True]:
            for velocity in [(.5,-.3,-.2),(.01,-1e-5,.01),(1.,2.,3.)]:
                q=base();q.update(blocks=floor(),default_state=ids[block],supports=[[0,63,0]],suppress_bounce=suppress,velocity=dv(*velocity));q['moves']=[dict(movement=dv(.1,-.2,0.))];add('original-block-after-fall-and-step',q)
    for fall in [0.,-0.,1.,10.,float('nan')]:
        for hit in [False,True]:
            for movement in [(1.,0.,0.),(.9,0.,0.),(-1.,-1.,-1.)]:
                q=base();q['fall_distance']=fb(fall);q['moves']=[dict(movement=dv(*movement),ray_hit=hit)];add('fall-reset-ray',q)
    for mask in range(64):
        q=base();q.update(blocks=floor(),supports=[[0,63,0]],wet=bool(mask&1),powder_snow=bool(mask&2),fire_immune=bool(mask&4),fire_ticks=1 if mask&8 else -1,was_on_fire=bool(mask&16),fire_states=[ids['lava']]if mask&32 else[ids['air']]);q['moves']=[dict(movement=dv(.1,-.2,0.))];add('dry-fire-context',q)
    for inside_velocity in [dv(1.,-2.,3.),dv(-0.,0.,-0.)]:
        for remove in [False,True]:
            q=base();q.update(blocks=floor(),supports=[[0,63,0]],default_state=ids['soul_sand']);q['moves']=[dict(movement=dv(1.,-.2,0.),inside_velocity=inside_velocity,inside_stuck=dv(.25,.05,.25),remove_after_fall=remove),dict(movement=dv(.2,0.,.2),removed=False)];add('host-callback-order',q)
    for _ in range(800):
        q=base();q.update(position=dv(r.uniform(-2,2),64.+r.choice([0.,.1,.5]),r.uniform(-2,2)),velocity=dv(*[r.uniform(-1,1)for _ in range(3)]),stuck=dv(*[r.choice([0.,.1,.5,1.])for _ in range(3)]),blocks=r.choice(list(scenes.values())),ground=r.choice([False,True]),step=fb(r.choice([0.,.5,.6,1.])),supports=r.choice([[],[None],[[0,63,0]]]),minor=r.choice([False,True]),fire_ticks=r.choice([-1,0,1]));q['moves']=[dict(kind=r.randrange(5),movement=dv(*[r.uniform(-2,2)for _ in range(3)]))for _ in range(3)];add('random-dry-scenes',q)
    return rows

def main():
    jar=ORACLE/'server-inner-1.21.jar';assert hashlib.sha256(jar.read_bytes()).hexdigest()==SHA
    tags=ROOT/'fixtures/placement-block-tags-1.21.json';tagdata=json.loads(tags.read_text());assert tagdata['jar_sha256']==SHA
    runtime=ROOT/'runtime/entity_move';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True);cp=os.pathsep.join(map(str,[classes,jar,*sorted((ORACLE/'libraries').rglob('*.jar'))]));flags=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED'];sources=[ROOT/'tools'/p for p in ['survival_oracle.java','player_physics_oracle.java','placement_tags.java','entity_move_oracle.java']]
    subprocess.run([JDK/'javac',*flags,'-proc:none','-cp',cp,'-d',classes,*sources],check=True,capture_output=True)
    facts=ROOT/'tools/entity_move_facts.json';java=[JDK/'java',*flags,'-Dmc121.blocktags='+str(tags),'-Xmx1G','-cp',cp,'oracle.entity_move_oracle'];extract=subprocess.run([*java,'extract',facts],text=True,capture_output=True)
    if extract.returncode:raise RuntimeError(extract.stdout+extract.stderr)
    data=json.loads(facts.read_text());assert len(data['rows'])==26684;assert sum(bool(v['flags']&1)for v in data['rows'])==384
    rows=generate();transcript=''.join(json.dumps(row['query'],separators=(',',':'))+'\n'for row in rows)
    run=subprocess.run(java,input=transcript,text=True,capture_output=True);(runtime/'oracle.log').write_text(run.stdout+run.stderr)
    if run.returncode:raise RuntimeError((run.stdout+run.stderr)[-9000:])
    assert 'SOURCE:file:'+str(jar)in run.stdout
    outputs=[json.loads(line.split('ORACLE:',1)[1])for line in run.stdout.splitlines()if line.startswith('ORACLE:')];assert len(outputs)==len(rows),(len(outputs),len(rows))
    for row,value in zip(rows,outputs):row['expected']=value
    path=ROOT/'fixtures/entity_move_vectors.json';path.write_text(json.dumps(dict(minecraft='1.21',oracle_inner_sha256=SHA,cases=rows),separators=(',',':'))+'\n')
    categories={}
    for row in rows:categories[row['category']]=categories.get(row['category'],0)+1
    report=dict(ok=True,minecraft='1.21',oracle_inner_sha256=SHA,actual_method='unchanged bsr.a(bts,exc) / Entity.move invoked through invokespecial bridge',queries=len(rows),move_states=sum(len(v)for v in outputs),categories=categories,block_state_facts=len(data['rows']),vectors_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),facts_sha256=hashlib.sha256(facts.read_bytes()).hexdigest(),block_tags_sha256=hashlib.sha256(tags.read_bytes()).hexdigest(),trace_sha256=hashlib.sha256('\n'.join(json.dumps(v,sort_keys=True,separators=(',',':'))for v in outputs).encode()).hexdigest(),source_sha256={p.name:hashlib.sha256(p.read_bytes()).hexdigest()for p in sources},original_methods=['Entity.move, limitPistonMovement and original per-axis accumulator','private Entity.collide with step heights; original world-query shape intersection filter','Entity.setPos/setPosRaw/rebuild dimensions AABB, onGroundWithMovement and checkSupportingBlock','getOnPosLegacy/getOnPos/getBlockSpeedFactor and fire-state predicate with official default tags','Actual block updateEntityAfterFallOn and stepOn methods called by delegating observer subclasses'],controlled_callbacks=['World block/entity/border collision shapes and supporting-block answers supplied by scene','Player edge backoff supplied callback; no Player.maybeBackOffFromEdge formula asserted','Fall-damage callback captures arguments and may remove the entity; full damage/fall state outside oracle','Inside-block callback may update velocity/stuck multiplier; full block overlap effects outside oracle','Block observer subclasses hold copied immutable state; is(Block) delegates to unchanged original state to preserve identity and official tag membership','MovementEmission.NONE; sound/event movement pipeline excluded','Dry-world fire immune/wet/powder contexts supplied; extinguish sound captured'],not_verified=['Fluid/baseTick environment, travel/acceleration/drag and living/player fall pipeline','Real chunk entity spatial index/support selection; supporting positions are world callbacks','Movement sound/game-event emission beyond NONE','Native Rust comparison is performed by parent harness separately'])
    (ROOT/'evidence/entity_move_oracle.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
