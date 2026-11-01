#!/usr/bin/env python3
"""Original listener teleport, accept-teleport, reset and tick state transitions."""
from pathlib import Path
import hashlib,json,math,os,random,subprocess
from player_movement_generate import ROOT,ORACLE,JDK,JAR_SHA,base,db,dv,fv

def generate():
    rows=[];rng=random.Random(0x1217677e1e)
    def add(category,q):q['op']='bookkeeping';rows.append(dict(category=category,query=q))
    ids=[-2147483648,-2147483647,-2,-1,0,1,2147483645,2147483646,2147483647]
    ticks=[-2147483648,-1,0,1,20,2147483646,2147483647]
    for old_id in ids:
        next_id=(old_id+1+2**31)%2**32-2**31
        if next_id==2147483647:next_id=0
        for tick in ticks:
            for changing in [False,True]:
                q=base();q.update(teleport_id=old_id,tick=tick,changing=changing,teleport_tick=12345);q['steps']=[dict(op='teleport',position=dv(12.25,-64.,-123.5),rotation=fv(190.,-91.)),dict(op='ack',id=next_id^1),dict(op='ack',id=next_id),dict(op='ack',id=next_id),dict(op='reset'),dict(op='tick')];add('absolute-teleport-ack-reset-tick',q)
    for mask in range(32):
        for _ in range(12):
            q=base();q['current']=dv(rng.uniform(-3e7,3e7),rng.uniform(-64,320),rng.uniform(-3e7,3e7));q['current_rotation']=fv(rng.uniform(-3600,3600),rng.uniform(-3600,3600));q['teleport_id']=rng.randint(-2147483648,2147483647);q['steps']=[dict(op='teleport',position=dv(rng.uniform(-3e7,3e7),rng.uniform(-64,320),rng.uniform(-3e7,3e7)),rotation=fv(rng.uniform(-3600,3600),rng.uniform(-3600,3600)),relative_mask=mask)];add('relative-packet-projection',q)
    for pending in [None,dv(-0.,0.,-0.),dv(14.,256.,-123.)]:
        for changing in [False,True]:
            for old_id in ids:
                for ack in [old_id,old_id^1]:
                    q=base();q.update(pending=pending,changing=changing,teleport_id=old_id);q['steps']=[dict(op='ack',id=ack)];add('ack-initial-state',q)
    for tick in ticks:
        for received in [-2147483648,-1,0,1,2147483647]:
            for known in [-2147483648,-1,0,2147483647]:
                q=base();q.update(tick=tick,received=received,known=known,block_ack=123,tick_player_position=dv(50.,55.,60.),tick_player_rotation=fv(390.,-110.));q['first_good']=dv(-12.,-13.,-14.);q['last_good']=dv(23.,24.,25.);q['steps']=[dict(op='tick'),dict(op='tick')];add('listener-tick-counters-and-player-callback',q)
    gravities=[0.,-0.,.08,.04,.16,9.999999747378752e-6,math.nextafter(9.999999747378752e-6,0.),float('nan'),float('inf'),-float('inf'),-1.]
    for gravity in gravities:
        for above in [-2147483648,-1,0,39,40,79,80,81,159,160,161,2147483647]:
            for mask in range(16):
                q=base();q.update(gravity=db(gravity),above_ground_ticks=above,floating=bool(mask&1),sleep=bool(mask&2),passenger=bool(mask&4),dead=bool(mask&8));q['steps']=[dict(op='tick')];add('floating-tick-gravity-limit',q)
    for value in [-0.,0.,float('inf'),-float('inf'),float('nan'),1e300,-1e300]:
        q=base();q['steps']=[dict(op='teleport',position=dv(value,value,value),rotation=fv(-0.,0.)),dict(op='ack',id=1),dict(op='reset')];add('method-level-extreme-numeric-values',q)
    return rows

def main():
    jar=ORACLE/'server-inner-1.21.jar';assert hashlib.sha256(jar.read_bytes()).hexdigest()==JAR_SHA
    runtime=ROOT/'runtime/player_movement';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True);cp=os.pathsep.join(map(str,[classes,jar,*sorted((ORACLE/'libraries').rglob('*.jar'))]));flags=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED'];sources=[ROOT/'tools'/n for n in ['survival_oracle.java','player_physics_oracle.java','player_movement_oracle.java']]
    subprocess.run([JDK/'javac',*flags,'-proc:none','-cp',cp,'-d',classes,*sources],check=True,capture_output=True)
    rows=generate();transcript=''.join(json.dumps(r['query'],separators=(',',':'))+'\n'for r in rows)
    java=subprocess.run([JDK/'java',*flags,'-Xmx1G','-cp',cp,'oracle.player_movement_oracle'],input=transcript,text=True,capture_output=True);(runtime/'bookkeeping-oracle.log').write_text(java.stdout+java.stderr)
    if java.returncode:raise RuntimeError((java.stdout+java.stderr)[-7000:])
    assert 'SOURCE:file:'+str(jar)in java.stdout
    out=[json.loads(line.split('ORACLE:',1)[1])for line in java.stdout.splitlines()if line.startswith('ORACLE:')];assert len(out)==len(rows),(len(out),len(rows))
    for row,value in zip(rows,out):row['expected']=value
    vectors=dict(minecraft='1.21',oracle_inner_sha256=JAR_SHA,cases=rows);vp=ROOT/'tools/player_movement_bookkeeping_vectors.json';vp.write_text(json.dumps(vectors,separators=(',',':'))+'\n')
    counts={}
    for row in rows:counts[row['category']]=counts.get(row['category'],0)+1
    states=[s for sequence in out for s in sequence];report=dict(ok=True,minecraft='1.21',oracle_inner_sha256=JAR_SHA,actual_methods=['aru.a(DDDFFSet): original teleport, including awaiting target/id/time and ClientboundPlayerPositionPacket construction','aru.a(agk): original accept teleport and original aqv.P/Q changing-dimension accessors','aru.l(): original resetPosition','aru.d(): original entire game-listener tick, with controlled player and common-listener callbacks'],queries=len(rows),observed_states=len(states),categories=counts,disconnect_callbacks=sum('disconnect'in s for s in states),packet_callbacks=sum(len(s.get('packets',[]))for s in states),trace_sha256=hashlib.sha256('\n'.join(json.dumps(v,sort_keys=True,separators=(',',':'))for v in out).encode()).hexdigest(),vectors_sha256=hashlib.sha256(vp.read_bytes()).hexdigest(),source_sha256={p.name:hashlib.sha256(p.read_bytes()).hexdigest()for p in sources},controlled_callbacks=['ServerPlayer.moveTo records position/rotation without the full Entity bookkeeping','ServerPlayer.doTick is controlled and may change position and rotation','Root vehicle is the player; vehicle-floating branch not covered','Common-listener keepalive tick and packet/disconnect sends are captured','ServerPlayer gravity base and dead/sleep/passenger context are supplied; original final Entity.getGravity and listener flying threshold execute unchanged'],not_verified=['actual network writes and common-listener keepalive','full player/entity physics inside player.doTick','vehicle-floating branch','native Rust comparison is performed by the parent harness separately'])
    path=ROOT/'evidence/player-movement-bookkeeping-oracle.json';path.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
