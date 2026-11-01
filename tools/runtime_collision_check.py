#!/usr/bin/env python3
"""Compare runtime block/entity/border queries with unchanged Minecraft1.21 classes."""
from pathlib import Path
import argparse,collections,hashlib,json,os,random,re,struct,subprocess

ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle'
JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def db(v):return struct.pack('>d',v).hex()
def fb(v):return struct.pack('>f',v).hex()
def box(values):return list(map(db,values))
def main():
 p=argparse.ArgumentParser();p.add_argument('--skip-rust',action='store_true');a=p.parse_args()
 jar=ORACLE/'server-inner-1.21.jar';assert hashlib.sha256(jar.read_bytes()).hexdigest()==SHA
 runtime=ROOT/'runtime/runtime-collision';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True)
 cp=os.pathsep.join(map(str,[classes,jar,*sorted((ORACLE/'libraries').rglob('*.jar'))]));flags=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED']
 compile=subprocess.run([JDK/'javac',*flags,'-proc:none','-cp',cp,'-d',classes,ROOT/'tools/survival_oracle.java',ROOT/'tools/runtime_collision_oracle.java'],capture_output=True,text=True)
 if compile.returncode:raise RuntimeError(compile.stdout+compile.stderr)
 java=subprocess.run([JDK/'java',*flags,'-Xmx1G','-cp',cp,'oracle.runtime_collision_oracle','extract',ROOT/'fixtures/runtime_collision_shapes-1.21.json'],capture_output=True,text=True)
 if java.returncode:raise RuntimeError((java.stdout+java.stderr)[-4000:])
 catalog=json.loads((ROOT/'fixtures/runtime_collision_shapes-1.21.json').read_text());r=random.Random(0x121767cc55);queries=[]
 def add(blocks,bounds,position=(.5,64.,.5),descending=False,fall=0.,op='blocks'):
  queries.append(dict(op=op,blocks=blocks,bounds=box(bounds),position=box(position),descending=descending,fall=fb(fall)))
 seen=set()
 for state,row in enumerate(catalog['states']):
  shape,kind,*_=row
  if kind in (6,7):continue
  if (shape,kind) not in seen:
   seen.add((shape,kind));add([[0,64,0,state]],[-.1,63.8,-.1,1.1,66.2,1.1])
  if kind in (2,3,4,5):
   for y,down,fall in [(64.,False,0.),(65.,False,0.),(65.,True,0.),(64.8,False,3.)]:
    x,z=r.choice([-20,-2,-1,0,1,13,15,16,18]),r.choice([-20,-2,-1,0,1,13,15,16,18]);add([[x,64,z,state]],[x-.1,63.8,z-.1,x+1.1,66.2,z+1.1],(x+.5,y,z+.5),down,fall)
 valid=[i for i,v in enumerate(catalog['states'])if v[1] not in (6,7)]
 for i in range(1600):
  x,z=r.choice([-1000.,-16.,-1.,0.,1.,15.,16.,1000.]),r.choice([-1000.,-16.,-1.,0.,1.,15.,16.,1000.]);y=64.;blocks=[]
  for j in range(r.randrange(1,18)):
   blocks.append([int(x)+r.randrange(-2,3),int(y)+r.randrange(-2,4),int(z)+r.randrange(-2,3),r.choice(valid)])
  width=r.choice([1e-8,1e-7,.125,.6,1.,2.]);height=r.choice([1e-8,.125,.6,1.5,1.8,2.]);dx,dz=r.choice([-.5,0.,.5,1.]),r.choice([-.5,0.,.5,1.]);feet=(x+dx,y+r.choice([0.,.125,.5,1.]),z+dz)
  add(blocks,[feet[0]-width/2,feet[1],feet[2]-width/2,feet[0]+width/2,feet[1]+height,feet[2]+width/2],feet,bool(i%2),r.choice([0.,2.5,2.5000002,10.]))
 for axis in range(3):
  for sign in [-1,1]:
   for epsilon in [0.,1e-10,1e-8,1e-7,1e-6]:
    lo=[.1,64.1,.1];hi=[.9,64.9,.9];lo[axis]=[1.,65.,1.][axis]-epsilon if sign>0 else [-.8,63.2,-.8][axis];hi[axis]=[1.8,65.8,1.8][axis]if sign>0 else [0.,64.,0.][axis]+epsilon;add([[0,64,0,1]],lo+hi)
 for i in range(700):
  x,z=r.choice([-16.,-1.,0.,1.,16.]),r.choice([-16.,-1.,0.,1.,16.]);y=64.;blocks=[[int(x)+r.randrange(-1,2),64+r.randrange(-1,3),int(z)+r.randrange(-1,2),r.choice([0,1,9,10,2048,2049])]for _ in range(8)]
  add(blocks,[x-.3,y,z-.3,x+.3,y+r.choice([0.,1.5,1.8,2.]),z+.3],op='air')
 for i in range(1000):
  center=[r.choice([0.,.1,-.1,1e6,-1e6,29999980.,-29999980.])for _ in range(2)];size=r.choice([1.,1.5,3.25,64.,256.,59999968.]);cap=29999984.;extent=[max(-cap,min(cap,center[0]-size/2)),max(-cap,min(cap,center[1]-size/2)),max(-cap,min(cap,center[0]+size/2)),max(-cap,min(cap,center[1]+size/2))]
  # Valid levels have a positive extent. Degenerate corrupt border settings
  # are refused by the native adapter instead of approximating a shape.
  if extent[0]>=extent[2]or extent[1]>=extent[3]:continue
  x,z=r.choice([extent[0],extent[2]]),r.choice([extent[1],extent[3]]);x+=r.choice([-3.,-2.,-1.,-.6,-.3,-1e-8,0.,1e-8,.3,.6,1.,2.,3.]);z+=r.choice([-3.,-1.,0.,1.,3.]);w=r.choice([.125,.6,1.,2.,4.]);queries.append(dict(op='border',border=box([*center,size]),position=box([x,64.,z]),bounds=box([x-w/2,64.,z-w/2,x+w/2,65.8,z+w/2])))
 for i in range(1000):
  b=[-.3,64.,-.3,.3,65.8,.3]if i%3 else [0.,64.,0.,1e-8,64.+1e-8,1e-8];entities=[]
  for j in range(r.randrange(0,10)):
   x,y,z=r.choice([-1.,-.3,0.,.3,1.]),r.choice([63.,64.,65.8,66.]),r.choice([-1.,-.3,0.,.3,1.]);width=r.choice([.125,.6,1.]);entities.append(dict(id=j+1,bounds=box([x-width/2,y,z-width/2,x+width/2,y+1.8,z+width/2]),spectator=r.choice([True,False]),collidable=r.choice([True,False])))
  queries.append(dict(op='entities',bounds=box(b),entities=entities))
 transcript=''.join(json.dumps(q,separators=(',',':'))+'\n'for q in queries)
 java=subprocess.run([JDK/'java',*flags,'-Xmx1G','-cp',cp,'oracle.runtime_collision_oracle'],input=transcript,text=True,capture_output=True);(runtime/'oracle.log').write_text(java.stdout+java.stderr)
 if java.returncode:raise RuntimeError((java.stdout+java.stderr)[-6000:])
 outputs=[json.loads(v.split('ORACLE:',1)[1])for v in java.stdout.splitlines()if v.startswith('ORACLE:')];assert len(outputs)==len(queries),(len(outputs),len(queries))
 vectors=ROOT/'fixtures/runtime_collision_vectors.json';vectors.write_text(json.dumps(dict(minecraft='1.21',oracle_inner_sha256=SHA,cases=[dict(query=q,expected=o)for q,o in zip(queries,outputs)]),separators=(',',':'))+'\n')
 report=dict(minecraft='1.21',oracle_inner_sha256=SHA,queries=len(queries),operations=dict(collections.Counter(q['op']for q in queries)),full_voxel_shapes=len(catalog['shapes']),block_states=len(catalog['states']),vectors_sha256=hashlib.sha256(vectors.read_bytes()).hexdigest(),native_source_sha256=hashlib.sha256((ROOT/'src/runtime_collision.rs').read_bytes()).hexdigest(),oracle_source_sha256=hashlib.sha256((ROOT/'tools/runtime_collision_oracle.java').read_bytes()).hexdigest(),scope='Original BlockCollisions cursor/edge rules, state collision shapes and query filtering, inclusive air queries, original EntityGetter collision filtering and static WorldBorder activation/full voxel shape; maps and generic-entity flags supplied by oracle-only subclasses.',not_verified=['Animated moving-piston and shulker block-entity shape generation','Chunk availability/ticking policy and native border lerp progression','Every entity-type canCollideWith override/vehicle relation and interaction pushing','Full Entity.move fluid/block callbacks and ItemWorld integration'])
 if not a.skip_rust:
  env=os.environ.copy();env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup',CARGO_INCREMENTAL='0');cargo='/root/.local/share/mc-rust-toolchain/cargo/bin/cargo';rust=subprocess.run([cargo,'test','--lib','runtime_collision::tests','--','--nocapture'],cwd=ROOT,env=env,text=True,capture_output=True);(runtime/'rust.log').write_text(rust.stdout+rust.stderr)
  if rust.returncode:raise RuntimeError((rust.stdout+rust.stderr)[-6000:])
  report.update(mismatches=0,rust_tests_passed=int(re.search(r'test result: ok\. (\d+) passed',rust.stdout).group(1)))
 (ROOT/'evidence/runtime-collision-conformance.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
