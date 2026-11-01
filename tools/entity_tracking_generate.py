#!/usr/bin/env python3
"""Actual TrackedEntity.updatePlayer and related original policy methods."""
from pathlib import Path
import collections,hashlib,json,math,os,random,struct,subprocess
ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle';JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin');SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def db(v):return struct.pack('>d',v).hex()
def dv(x,y,z):return [db(x),db(y),db(z)]
def base():return dict(observer=dv(0,64,0),entity=dv(1,64,0),center=[0,0],requested=10,server_view=10,view_radius=10,range=96,percent=100,broadcast=True,camera_self=True)
def generate(facts):
 rows=[];r=random.Random(0x7671217ac)
 def add(category,q):rows.append(dict(category=category,query=q))
 for radius in [0,1,16,32,64,96,128,256,512]:
  for requested in [0,1,2,4,6,10,32,127]:
   for previous in [False,True]:
    q=base();q.update(range=radius,requested=requested,previous=previous,view_radius=32,server_view=32)
    limit=min(radius,max(2,min(requested,32))*16);q['steps']=[]
    for x,z in [(limit,0),(math.nextafter(float(limit),math.inf),0),(-limit,0),(0,limit),(limit/math.sqrt(2),limit/math.sqrt(2)),(limit*.8,limit*.8),(0,0)]:q['steps'].append(dict(entity=dv(x,1e10,z)))
    add('distance-inclusive-xz-and-transitions',q)
 for percent in [-2147483648,-1000,-1,0,1,10,100,250,1000,2147483647]:
  for range_ in [-2147483648,-1,0,1,16,96,512,2147483647]:
   q=base();q.update(percent=percent,range=range_,previous=True);add('dedicated-i32-scale',q)
 for requested in [-2147483648,-128,-1,0,1,2,3,10,127,2147483647]:
  for server in [-1,0,1,2,6,32,2147483647]:
   q=base();q.update(requested=requested,server_view=server);add('player-view-clamp',q)
 for t in facts['rows']:
  q=base();q.update(range=0,passengers=[dict(type=t['id'])]);add('passenger-type-effective-range',q)
  q=base();q.update(range=0,passengers=[dict(type=58,passengers=[dict(type=t['id'])])]);add('nested-passenger-effective-range',q)
 for mask in range(128):
  q=base();q.update(previous=bool(mask&1),broadcast=bool(mask&2),pending=bool(mask&4),range=0 if mask&8 else 96,view_radius=0 if mask&16 else 10,same=bool(mask&32),player_target=bool(mask&64));add('lazy-gates-and-self',q)
 for mask in range(16):
  for previous in [False,True]:
   q=base();q.update(player_target=True,observer_spectator=bool(mask&1),target_spectator=bool(mask&2),camera_self=bool(mask&4),previous=previous,pending=bool(mask&8));add('original-player-broadcast-policy',q)
 for radius in [2,3,6,10,32]:
  for cx in range(-radius-3,radius+4):
   for cz in range(-radius-3,radius+4):
    if (cx+cz)%3:continue
    q=base();q.update(range=512,requested=32,server_view=32,view_radius=radius,entity=dv(cx*16+.5,64,cz*16+.5));add('original-rounded-chunk-view',q)
 for i in range(1200):
  q=base();q.update(observer=dv(r.uniform(-500,500),r.uniform(-1e5,1e5),r.uniform(-500,500)),entity=dv(r.uniform(-500,500),r.uniform(-1e5,1e5),r.uniform(-500,500)),range=r.choice([32,64,96,128,512]),percent=r.choice([10,50,100,200]),requested=r.choice([0,2,6,10,32]),server_view=r.choice([2,6,10,32]),view_radius=r.choice([2,6,10,32]),center=[r.randrange(-32,33),r.randrange(-32,33)],previous=bool(i%2),pending=i%7==0,broadcast=i%5!=0,passengers=[dict(type=r.choice(facts['rows'])['id'])]if i%3==0 else[]);add('random-policy-contexts',q)
 return rows

def main():
 jar=ORACLE/'server-inner-1.21.jar';assert hashlib.sha256(jar.read_bytes()).hexdigest()==SHA
 runtime=ROOT/'runtime/entity_tracking';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True);cp=os.pathsep.join(map(str,[classes,jar,*sorted((ORACLE/'libraries').rglob('*.jar'))]));flags=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED'];sources=[ROOT/'tools/survival_oracle.java',ROOT/'tools/entity_tracking_oracle.java'];subprocess.run([JDK/'javac',*flags,'-proc:none','-cp',cp,'-d',classes,*sources],check=True,capture_output=True)
 java=[JDK/'java',*flags,'-Xmx1G','-cp',cp,'oracle.entity_tracking_oracle'];facts=ROOT/'tools/entity_tracking_facts.json';extract=subprocess.run([*java,'extract',facts],capture_output=True,text=True)
 if extract.returncode:raise RuntimeError(extract.stdout+extract.stderr)
 data=json.loads(facts.read_text());assert data['default_requested_view_distance']==2
 views=ROOT/'fixtures/entity_tracking_views.json';view_result=subprocess.run([*java,'views',views],capture_output=True,text=True)
 if view_result.returncode:raise RuntimeError(view_result.stdout+view_result.stderr)
 view_data=json.loads(views.read_text());assert len(view_data['iterations'][6]['expected'])==45
 rows=generate(data);p=subprocess.run(java,input=''.join(json.dumps(x['query'],separators=(',',':'))+'\n'for x in rows),capture_output=True,text=True);(runtime/'oracle.log').write_text(p.stdout+p.stderr)
 if p.returncode:raise RuntimeError((p.stdout+p.stderr)[-7000:])
 assert 'SOURCE:file:'+str(jar)in p.stdout;out=[json.loads(l.split('ORACLE:',1)[1])for l in p.stdout.splitlines()if l.startswith('ORACLE:')];assert len(out)==len(rows)
 for row,result in zip(rows,out):row['expected']=result
 path=ROOT/'fixtures/entity_tracking_vectors.json';path.write_text(json.dumps(dict(minecraft='1.21',oracle_inner_sha256=SHA,cases=rows),separators=(',',':'))+'\n')
 report=dict(ok=True,minecraft='1.21',oracle_inner_sha256=SHA,queries=len(rows),states=sum(map(len,out)),categories=dict(collections.Counter(x['category']for x in rows)),type_facts=len(data['rows']),view_iterations=len(view_data['iterations']),view_differences=len(view_data['differences']),view_vectors_sha256=hashlib.sha256(views.read_bytes()).hexdigest(),default_requested_view_distance=2,vectors_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),facts_sha256=hashlib.sha256(facts.read_bytes()).hexdigest(),original_methods=['aqb$b.b(aqv) updatePlayer','aqb$b.b() getEffectiveRange; original indirect-passenger traversal','apn.b(int) DedicatedServer int scaling','aqb.b(aqv) per-player view clamp','aqb.a(aqv,int,int) view+pending gate with original arq.isPending','aqg.Positioned.contains and original rounded integer view-distance shape','aqv.a(aqv) original player spectator/camera broadcast conditions','aqh.createDefault','EntityType tracking range/updateInterval/trackDeltas getters'],controlled_callbacks=['ServerEntity.add/removePairing are captured; packet assembly not part of this oracle','Observer position/requested view/spectator and generic-entity broadcast result are supplied contexts'],not_verified=['Actual chunk scheduling/send pending transitions and ChunkMap update call scheduling','Full pairing packet content','Native Rust comparison is performed separately'],source_sha256={p.name:hashlib.sha256(p.read_bytes()).hexdigest()for p in sources});(ROOT/'evidence/entity-tracking-oracle.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
