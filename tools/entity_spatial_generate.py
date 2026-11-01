#!/usr/bin/env python3
"""Record original signed1.21 spatial sections, class caches and ordered queries."""
from pathlib import Path
import collections,hashlib,json,math,os,random,struct,subprocess

ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle';JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def db(v):return struct.pack('>d',v).hex()
def box(v):return list(map(db,v))
def entity(handle,id,kind,position=(.5,.0,.5),width=.25,height=.25):
 x,y,z=position;return dict(handle=handle,id=id,kind=kind,bounds=box([x-width/2,y,z-width/2,x+width/2,y+height,z+width/2]))
def query(bounds=(-1.,-1.,-1.,2.,2.,2.),class_key=None,exact=False,abort_after=0):return dict(op='query',bounds=box(bounds),class_key=class_key,exact=exact,abort_after=abort_after)
def scene(entities,ops,visibility=2,**extra):return dict(entities=entities,initial_visibility=visibility,operations=ops,**extra)
def directed():
 cases=[];s=[0,0,0]
 for reverse in [False,True]:
  entities=[entity(1,7,1),entity(2,7,2),entity(3,8,1),entity(4,8,2),entity(5,9,1)]
  ops=[dict(op='create_section',section=s),dict(op='create_section',section=s)]+[dict(op='add',section=s,entity=h)for h in ([2,1,4,3]if reverse else[1,2,3,4])]
  ops += [query(class_key=k,exact=e)for k,e in [(None,False),(0,False),(1,False),(2,False),(3,False),(4,False),(3,True),(4,True),(0,True)]]
  ops += [dict(op='remove',section=s,entity=1 if reverse else 2)]
  ops += [query(class_key=k)for k in [None,0,1,2,3,4]]
  ops += [dict(op='id',entity=5,id=8),dict(op='remove',section=s,entity=5)]
  ops += [query(class_key=k)for k in [None,1,2,3,4]]
  ops += [dict(op='move',entity=3,**{'from':s,'to':s},remove_empty=True),dict(op='visibility',section=s,visibility=0),query(class_key=1),dict(op='visibility',section=s,visibility=1),query(class_key=1,abort_after=1),dict(op='remove_section',section=s),dict(op='remove_section',section=s)]
  cases.append(scene(entities,ops))
 entities=[entity(1,1,1),entity(2,1,2),entity(3,3,1)]
 ops=[dict(op='add',section=s,entity=1),query(class_key=1),query(class_key=2),query(class_key=3),query(class_key=4),dict(op='remove',section=s,entity=2)]
 # Base empty + retained item cache must still skip the whole section.
 ops += [query(class_key=1),dict(op='add',section=s,entity=3),query(class_key=1),query(),dict(op='bounds',entity=1,bounds=box([16.,0.,16.,17.,1.,17.])),query(class_key=1),dict(op='bounds',entity=3,bounds=box([1.,0.,0.,2.,1.,1.])),query(bounds=[0.,0.,0.,1.,1.,1.]),query(bounds=[0.,0.,0.,math.nextafter(1.,math.inf),1.,1.])]
 cases.append(scene(entities,ops))
 ops=[dict(op='add',section=s,entity=1),dict(op='add',section=s,entity=1),query(class_key=1),dict(op='add',section=s,entity=1),dict(op='remove',section=s,entity=1),dict(op='move',entity=1,**{'from':s,'to':s},remove_empty=True),query(class_key=1),dict(op='remove',section=s,entity=1),dict(op='remove',section=s,entity=1),dict(op='remove',section=s,entity=1),dict(op='move',entity=1,**{'from':s,'to':[0,-1,0]},remove_empty=False),dict(op='chunk_sections',chunk=[0,0]),dict(op='move',entity=1,**{'from':[0,-1,0],'to':s},remove_empty=True),dict(op='chunk_sections',chunk=[0,0])]
 cases.append(scene([entity(1,4,1)],ops))
 # Long-sorted section order, coarse accessibility, empty sections, aborts,
 # inherited/exact type caches and initial per-chunk visibility.
 entities=[entity(i+1,i+1,1 if i%2==0 else 2,(-8.,-8.,-8.),width=.6,height=1.8)for i in range(12)]
 sections=[[-1,-1,-1],[-1,0,-1],[-1,1,-1],[0,-1,0],[0,0,0],[0,1,0],[0,-2,-1],[0,2,1],[1,0,0],[1,1,0],[1,-1,0],[1,2,0]]
 ops=[dict(op='add',section=p,entity=i+1)for i,p in enumerate(sections)]+[dict(op='create_section',section=[1,-2,0])]
 for chunk in [[-1,-1],[0,0],[1,0],[0,-1],[0,1],[8,8]]:ops.append(dict(op='chunk_sections',chunk=chunk))
 broad=[-33.,-33.,-33.,34.,50.,34.]
 for k in [None,0,1,2,3,4]:
  for abort in [1,2,4,0]:ops.append(query(bounds=broad,class_key=k,abort_after=abort))
 ops += [dict(op='visibility',section=[-1,-1,-1],visibility=0),query(bounds=broad),dict(op='visibility',section=[-1,0,-1],visibility=2),query(bounds=broad,class_key=3,exact=True)]
 cases.append(scene(entities,ops,visibility=2,chunk_visibility=[[-1,-1,1],[1,0,0]]))
 # Packed-coordinate aliasing/signed order without huge query spans.
 coords=[[-2097152,-524288,-2097152],[-1,-1,-1],[0,0,0],[2097150,524287,2097151],[2147483647,2147483647,2147483647],[-2147483648,-2147483648,-2147483648]]
 ops=[]
 for p in coords:ops.append(dict(op='create_section',section=p))
 for p in coords:ops.append(dict(op='visibility',section=p,visibility=1))
 for p in coords:ops.append(dict(op='remove_section',section=p))
 cases.append(scene([],ops))
 # Original numeric helper, including values outside the running world's
 # finite bounds. Nonfinite inputs are confined to this pure conversion op.
 edges=[-0.,0.,-1e-310,1e-310,-16.,16.,-32.,32.,-64.,64.,-30000000.,30000000.,-20000000.,20000000.,-2147483648.,2147483647.,float('nan'),float('inf'),-float('inf')]
 values=[]
 for v in edges:
  values.append(v)
  if math.isfinite(v):values += [math.nextafter(v,-math.inf),math.nextafter(v,math.inf)]
 ops=[dict(op='position_key',position=box([x,y,z]))for x in values for y,z in [(0.,0.),(x,x),(16.,-16.),(-20000000.,30000000.)]]
 cases.append(scene([],ops))
 return cases
def randomized():
 r=random.Random(0x1217675ec710);cases=[]
 for case in range(90):
  sections=[r.choice([[0,0,0],[-1,0,0],[0,-1,0],[0,0,-1],[1,1,1],[-1,-1,-1],[0,1,0],[-2,4,2],[2,-2,-2]])for _ in range(8)]
  entities=[]
  for h in range(1,21):
   p=r.choice(sections);x,z=p[0]*16+r.choice([0.,.125,8.,15.875,16.]),p[2]*16+r.choice([0.,.125,8.,15.875,16.]);y=p[1]*16+r.choice([0.,.125,8.,15.875,16.]);kind=r.choice([1,2]);entities.append(entity(h,r.choice([-2147483648,-1,0,1,2,3,4,2147483647])if case%2 else h,kind,(x,y,z),width=.25 if kind==1 else .6,height=.25 if kind==1 else 1.8))
  ops=[]
  for p in sections:ops.append(dict(op='create_section',section=p))
  for h in range(1,13):ops.append(dict(op='add',section=sections[(h-1)%len(sections)],entity=h))
  broad=[-40.,-40.,-40.,50.,90.,50.]
  for k in [1,2,3,4]:ops.append(query(bounds=broad,class_key=k,abort_after=r.choice([0,1,3])))
  for step in range(70):
   op=r.choices(['add','remove','move','visibility','id','bounds','query','chunk_sections','create_section','remove_section'],[10,12,12,7,5,6,30,6,6,4])[0];h=r.randrange(1,21);p=r.choice(sections)
   if op=='add'or op=='remove':ops.append(dict(op=op,section=p,entity=h))
   elif op=='move':ops.append(dict(op=op,entity=h,**{'from':p,'to':r.choice(sections)},remove_empty=r.choice([True,False])))
   elif op=='visibility':ops.append(dict(op=op,section=p,visibility=r.randrange(3)))
   elif op=='id':ops.append(dict(op=op,entity=h,id=r.choice([-2147483648,-1,0,1,2,3,4,2147483647])))
   elif op=='bounds':
    x,y,z=p[0]*16+r.choice([0.,.5,8.,16.]),p[1]*16+r.choice([0.,.5,8.,16.]),p[2]*16+r.choice([0.,.5,8.,16.]);ops.append(dict(op=op,entity=h,bounds=box([x,y,z,x+.25,y+.25,z+.25])))
   elif op=='query':
    if r.random()<.6:b=broad
    else:
     x,y,z=p[0]*16,p[1]*16,p[2]*16;b=[x-2.,y-4.,z-2.,x+18.,y+16.,z+18.]
    ops.append(query(bounds=b,class_key=r.choice([None,0,1,2,3,4]),exact=r.choice([True,False]),abort_after=r.choice([0,0,1,2,5])))
   elif op=='chunk_sections':ops.append(dict(op=op,chunk=[p[0],p[2]]))
   else:ops.append(dict(op=op,section=p))
  cases.append(scene(entities,ops,visibility=r.randrange(3),chunk_visibility=[[0,0,r.randrange(3)],[-1,-1,r.randrange(3)]]))
 return cases
def main():
 jar=ORACLE/'server-inner-1.21.jar';assert hashlib.sha256(jar.read_bytes()).hexdigest()==SHA
 runtime=ROOT/'runtime/entity-spatial';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True)
 source=ROOT/'tools/entity_spatial_oracle.java';cp=os.pathsep.join(map(str,[classes,jar,*sorted((ORACLE/'libraries').rglob('*.jar'))]))
 compile=subprocess.run([JDK/'javac','-proc:none','-cp',cp,'-d',classes,source],capture_output=True,text=True)
 if compile.returncode:raise RuntimeError(compile.stdout+compile.stderr)
 scenes=directed()+randomized();transcript=''.join(json.dumps(q,separators=(',',':'))+'\n'for q in scenes)
 java=subprocess.run([JDK/'java','-Xmx1G','-cp',cp,'oracle.entity_spatial_oracle'],input=transcript,text=True,capture_output=True,timeout=240);(runtime/'oracle.log').write_text(java.stdout+java.stderr)
 if java.returncode:raise RuntimeError((java.stdout+java.stderr)[-7000:])
 assert 'SOURCE:'in java.stdout and jar.name in java.stdout
 outputs=[json.loads(l.split('ORACLE:',1)[1])for l in java.stdout.splitlines()if l.startswith('ORACLE:')];assert len(outputs)==len(scenes)
 for q,expected in zip(scenes,outputs):assert len(q['operations'])==len(expected)
 data=dict(minecraft='1.21',oracle_inner_sha256=SHA,schema='entity-spatial-v1',cases=[dict(scene=q,expected=expected)for q,expected in zip(scenes,outputs)])
 vectors=ROOT/'fixtures/entity_spatial_vectors.json';tmp=vectors.with_suffix('.json.partial');tmp.write_text(json.dumps(data,separators=(',',':'))+'\n');tmp.replace(vectors)
 counts=collections.Counter(op['op']for q in scenes for op in q['operations']);report=dict(ok=True,minecraft='1.21',oracle_inner_sha256=SHA,scenes=len(scenes),operations=sum(counts.values()),operation_counts=dict(counts),query_results=sum(len(v['result']['handles'])for rows in outputs for v in rows if isinstance(v['result'],dict)and'handles'in v['result']),visited_sections=sum(len(v['result']['sections'])for rows in outputs for v in rows if isinstance(v['result'],dict)and'handles'in v['result']),snapshots=sum(len(rows)for rows in outputs),vectors_bytes=vectors.stat().st_size,vectors_sha256=hashlib.sha256(vectors.read_bytes()).hexdigest(),oracle_source_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),scope='Original EntitySectionStorage ordered accessible-section traversal, EntitySection AABB/type/abort queries, ClassInstanceMultiMap lazy typed cache mutation including cross-class duplicateEntityID removal, section/chunk key packing and position conversion. Original ItemEntity and ServerPlayer instances; only entityID/AABB fields are injected; visibility callback is controlled.',not_verified=['Native Rust parity: parent spatial module tests must consume these vectors','Entity index mutation during query-consumer callbacks','Persistent entity manager, chunk ticking transitions, entity tick list and query predicate callbacks beyond type/abort'])
 (ROOT/'evidence/entity-spatial-oracle.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
