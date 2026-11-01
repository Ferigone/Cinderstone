#!/usr/bin/env python3
"""Execute the original signed Minecraft1.21 handleMovePlayer on callback scenes.

Entity.move is a controlled displacement callback, not a replacement for geometric
collision proof. Teleport/disconnect/world side effects are captured. The handler
branch decisions, input normalization, counters and callback order are original.
"""
from pathlib import Path
import hashlib,json,math,os,random,struct,subprocess

ROOT=Path(__file__).resolve().parents[1]
ORACLE=ROOT.parent/'mc121-oracle'
JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
JAR_SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def db(v):return struct.pack('>d',v).hex()
def fb(v):return struct.pack('>f',v).hex()
def dv(*v):return [db(x)for x in v]
def fv(*v):return [fb(x)for x in v]
def base():
    p=dv(0.,64.,0.)
    return dict(packet=dict(kind='both',position=dv(1.,64.,0.),rotation=fv(45.,-20.),ground=False),current=p.copy(),current_rotation=fv(10.,5.),first_good=p.copy(),last_good=p.copy(),velocity=dv(0.,0.,0.),runs_normally=True,air=True,old_no_collision=True)

def generate():
    rng=random.Random(0x121767ba5);queries=[]
    def add(category,q):queries.append(dict(category=category,query=q))
    # Invalid position/rotation rules are applied only to fields present in packet.
    position_values=[float('nan'),float('inf'),-float('inf'),-0.,0.,-30000000.,30000000.,-30000000.000001,30000000.000001,-20000000.,20000000.,-1e300,1e300]
    rotation_values=[float('nan'),float('inf'),-float('inf'),-0.,0.,180.,-180.,360.,-360.,36000000.,-36000000.]
    for kind in ['both','position','rotation','ground']:
        for axis in range(3):
            for value in position_values:
                q=base();q['packet']['kind']=kind;q['packet']['position'][axis]=db(value);add('input-position',q)
        for axis in range(2):
            for value in rotation_values:
                q=base();q['packet']['kind']=kind;q['packet']['rotation'][axis]=fb(value);add('input-rotation',q)
    for flag in ['won','passenger','sleep']:
        for kind in ['both','position','rotation','ground']:
            for value in [0.,.5,1.,math.nextafter(1.,math.inf),2.,100.]:
                q=base();q[flag]=True;q['packet'].update(kind=kind,position=dv(value,64.,0.));add('early-'+flag,q)
    for tick in [-2147483648,-1,0,1,19,20,21,22,2147483647]:
        for sent in [-2147483648,-1,0,1,20,2147483647]:
            for pending in [None,dv(123.,67.,-12.)]:
                q=base();q.update(tick=tick,teleport_tick=sent,pending=pending);q['first_good']=dv(-2.,62.,2.);q['last_good']=dv(-1.,63.,1.);add('pending-teleport',q)
    # Exact threshold tests include counter wrapping, frozen-world bypass and
    # f32 multiplication before f64 promotion; speed correction keeps old angles.
    for received,known in [(0,0),(1,0),(4,0),(5,0),(6,0),(100,0),(-1,0),(-2,0),(2147483647,-1),(-2147483648,1),(3,7)]:
        for elytra in [False,True]:
            multiplier=(((received+1+2**31)%2**32)-2**31-known+2**31)%2**32-2**31
            if multiplier>5:multiplier=1
            threshold=(300. if elytra else 100.)*multiplier
            for distance in [0.,1.,math.sqrt(max(threshold,0.)),math.nextafter(math.sqrt(max(threshold,0.)),math.inf),100.]:
                for mode in ['ordinary','owner','changing','elytra-rule','frozen','creative','spectator']:
                    q=base();q.update(received=received,known=known,elytra=elytra,owner=mode=='owner',changing=mode=='changing',elytra_rule=mode=='elytra-rule',runs_normally=mode!='frozen',game_mode=1 if mode=='creative'else 3 if mode=='spectator'else 0,spectator=mode=='spectator');q['packet']['position']=dv(distance,64.,0.);add('speed',q)
    # All boolean floating gates, all eight ground/reset gates and differences
    # between lastGood and current position are observed through original calls.
    gates=['vertical_collision_below','spectator','flight_allowed','may_fly','levitation','elytra','spin','air']
    for mask in range(1<<len(gates)):
        q=base();q.update({name:bool(mask&(1<<i))for i,name in enumerate(gates)});q['game_mode']=3 if q['spectator']else 0;add('floating-gates',q)
    for delta_y in [-100.,-0.03125000000000001,-0.03125,math.nextafter(-.03125,math.inf),-0.,0.,1.,100.]:
        for mask in range(64):
            q=base();q['packet']['position']=dv(0.,64.+delta_y,0.);q['changing']=True
            for i,name in enumerate(['ground','liquid','climb','spectator','elytra','spin']):
                if name=='ground':q['packet']['ground']=bool(mask&(1<<i))
                else:q[name]=bool(mask&(1<<i))
            q['game_mode']=3 if q.get('spectator')else 0;add('ground-reset',q)
    for mask in range(512):
        q=base();q.update(ground=bool(mask&1),vertical_collision_below=bool(mask&2),jump_vertical_collision_below=bool(mask&4),no_physics=bool(mask&8),changing=bool(mask&16),sleep=bool(mask&32),game_mode=1 if mask&64 else 3 if mask&128 else 0,old_no_collision=bool(mask&256));q['spectator']=q['game_mode']==3;q['packet']['position']=dv(1.,65.,0.);q['resolved_delta']=dv(0.,0.,0.);add('rollback-and-jump',q)
    for shape_case in ['empty','new','old','both','only-old-outside-query']:
        for mode in [0,1,3]:
            for no_physics in [False,True]:
                for collide in [False,True]:
                    q=base();q.update(game_mode=mode,spectator=mode==3,no_physics=no_physics);q['resolved_delta']=dv(0. if collide else 1.,0.,0.);q['shapes']=[]
                    if shape_case in ['new','both']:q['shapes'].append(dv(1.,64.,-.3,2.,66.,.3))
                    if shape_case in ['old','both']:q['shapes'].append(dv(-.1,64.,-.3,1.1,66.,.3))
                    if shape_case=='only-old-outside-query':q['shapes'].append(dv(-.2,64.,-.2,.2,65.,.2))
                    add('new-collision',q)
    for residual_y in [-100.,-1.,-.5000001,-.5,0.,.5,.5000001,1.,100.]:
        for residual_x in [0.,.24999999999999997,.25,.25000000000000006,1.]:
            q=base();q['resolved_delta']=dv(1.-residual_x,-residual_y,0.);add('wrong-residual',q)
    for _ in range(1000):
        q=base();q['current']=dv(rng.uniform(-100,100),rng.uniform(0,128),rng.uniform(-100,100));q['first_good']=dv(*[struct.unpack('>d',bytes.fromhex(v))[0]+rng.uniform(-5,5)for v in q['current']]);q['last_good']=dv(*[struct.unpack('>d',bytes.fromhex(v))[0]+rng.uniform(-2,2)for v in q['current']]);q['packet']['position']=dv(*[struct.unpack('>d',bytes.fromhex(v))[0]+rng.uniform(-5,5)for v in q['current']]);q['packet']['rotation']=fv(rng.uniform(-1e7,1e7),rng.uniform(-1e7,1e7));q['velocity']=dv(*[rng.uniform(-3,3)for _ in range(3)]);q['resolved_delta']=dv(*[rng.uniform(-3,3)for _ in range(3)]);q['received']=rng.randint(-10,20);q['known']=rng.randint(-10,20);q['game_mode']=rng.randrange(4);q['spectator']=q['game_mode']==3
        for flag in ['no_physics','owner','changing','elytra','runs_normally','old_no_collision','ground','may_fly','spin','air','vertical_collision_below']:q[flag]=rng.choice([False,True])
        q['packet']['ground']=rng.choice([False,True]);add('random-controlled-move',q)
    return queries

def main():
    jar=ORACLE/'server-inner-1.21.jar';assert hashlib.sha256(jar.read_bytes()).hexdigest()==JAR_SHA
    runtime=ROOT/'runtime/player_movement';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True);cp=os.pathsep.join(map(str,[classes,jar,*sorted((ORACLE/'libraries').rglob('*.jar'))]));flags=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED'];sources=[ROOT/'tools'/n for n in ['survival_oracle.java','player_physics_oracle.java','player_movement_oracle.java']]
    subprocess.run([JDK/'javac',*flags,'-proc:none','-cp',cp,'-d',classes,*sources],check=True,capture_output=True)
    rows=generate();transcript=''.join(json.dumps(r['query'],separators=(',',':'))+'\n'for r in rows)
    java=subprocess.run([JDK/'java',*flags,'-Xmx1G','-cp',cp,'oracle.player_movement_oracle'],input=transcript,text=True,capture_output=True);(runtime/'oracle.log').write_text(java.stdout+java.stderr)
    if java.returncode:raise RuntimeError((java.stdout+java.stderr)[-7000:])
    assert 'SOURCE:file:'+str(jar)in java.stdout
    out=[json.loads(line.split('ORACLE:',1)[1])for line in java.stdout.splitlines()if line.startswith('ORACLE:')];assert len(out)==len(rows),(len(out),len(rows))
    for row,value in zip(rows,out):row['expected']=value
    vectors=dict(minecraft='1.21',oracle_inner_sha256=JAR_SHA,cases=rows);path=ROOT/'tools/player_movement_vectors.json';path.write_text(json.dumps(vectors,separators=(',',':'))+'\n')
    counts={}
    for row in rows:counts[row['category']]=counts.get(row['category'],0)+1
    report=dict(ok=True,minecraft='1.21',oracle_inner_sha256=JAR_SHA,actual_method='unchanged original aru.a(ahg) / ServerGamePacketListenerImpl.handleMovePlayer',queries=len(rows),categories=counts,teleport_callbacks=sum('teleport'in v for v in out),disconnect_callbacks=sum('disconnect'in v for v in out),move_callbacks=sum('move_delta'in v for v in out),trace_sha256=hashlib.sha256('\n'.join(json.dumps(v,sort_keys=True,separators=(',',':'))for v in out).encode()).hexdigest(),vectors_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),source_sha256={p.name:hashlib.sha256(p.read_bytes()).hexdigest()for p in sources},controlled_callbacks=['Entity.move displacement, jump effects and block/entity world shapes','teleport updates position and captures target/angles; original teleport bookkeeping is not invoked','disconnect captures Component text without network effects','world chunks, fall, ground, known movement, stats and reset effects are captured'],not_verified=['original teleport awaiting target/id/time bookkeeping','complete Entity.move block/fluid/landing side effects','running-world movement/network integration','native Rust comparison is performed by the parent harness separately'])
    path=ROOT/'evidence/player-movement-oracle.json';path.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
