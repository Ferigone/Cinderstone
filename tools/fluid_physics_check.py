#!/usr/bin/env python3
"""Differential fluid/current checks against unchanged Minecraft1.21 classes."""
from pathlib import Path
import argparse,collections,hashlib,json,os,random,re,struct,subprocess,zipfile
ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle';JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin');SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def db(v):return struct.pack('>d',v).hex()
def vec(values):return list(map(db,values))
def main():
 p=argparse.ArgumentParser();p.add_argument('--skip-rust',action='store_true');a=p.parse_args();jar=ORACLE/'server-inner-1.21.jar';assert hashlib.sha256(jar.read_bytes()).hexdigest()==SHA
 with zipfile.ZipFile(jar)as archive:
  tags={name:json.loads(archive.read('data/minecraft/tags/fluid/'+name+'.json'))for name in ['water','lava']}
 assert set(tags['water']['values'])=={'minecraft:water','minecraft:flowing_water'}
 assert set(tags['lava']['values'])=={'minecraft:lava','minecraft:flowing_lava'}
 data=json.loads((ROOT/'fixtures/fluid_physics_facts.json').read_text());rows=data['rows'];states=data['states'];r=random.Random(0x767f1d121);queries=[]
 cached=[i for i,row in enumerate(states)if rows[row][8]];fluids=[i for i,row in enumerate(states)if rows[row][0]];water=[i for i,row in enumerate(states)if rows[row][0]==1];lava=[i for i,row in enumerate(states)if rows[row][0]==2];fluid_examples={};fact_examples={}
 for state,row in enumerate(states):
  fact_examples.setdefault(row,state)
  if rows[row][0]:fluid_examples.setdefault(tuple(rows[row][:5]),state)
 pools=[0,1,9,10,*fluid_examples.values(),*r.sample(cached,50)]
 def add_flow(p,blocks):queries.append(dict(op='flow',position=list(p),blocks=blocks))
 # Every unique fluid representation, including waterlogged forms, source,
 # all flowing amounts, falling flags and empty-fluid behavior.
 for state in [0,*fluid_examples.values()]:
  for dx,dz in [(0,-1),(1,0),(0,1),(-1,0)]:
   for neighbor in [0,1,r.choice(water),r.choice(lava)]:
    add_flow((0,64,0),[[0,64,0,state],[dx,64,dz,neighbor],[dx,63,dz,r.choice([0,*fluid_examples.values()])]])
 for i in range(1700):
  x,y,z=r.choice([-33,-16,-1,0,1,15,16,10000]),r.choice([-63,0,63,64,319]),r.choice([-33,-16,-1,0,1,15,16,10000]);current=r.choice([0,*fluid_examples.values(),r.choice(fluids)]);blocks=[[x,y,z,current]]
  for dx,dz in [(0,-1),(1,0),(0,1),(-1,0)]:
   for dy in [-1,0,1]:blocks.append([x+dx,y+dy,z+dz,r.choice(pools)])
  add_flow((x,y,z),blocks)
 for state in [0,*fluid_examples.values(),*r.sample(water,100)]:
  for above in [0,1,r.choice(water),r.choice(lava)]:queries.append(dict(op='height',position=[0,64,0],blocks=[[0,64,0,state],[0,65,0,above]]))
 for row,state in fact_examples.items():
  if not rows[row][8]:continue
  for tag in [1,2]:
   for direction in range(6):queries.append(dict(op='face',position=[0,64,0],blocks=[[0,64,0,state]],tag=tag,direction=direction))
 for i in range(1800):
  x,y,z=r.choice([-16.,-1.,0.,1.,15.,16.,1000.,30000000.]),r.choice([-63.,0.,64.,319.,20000000.]),r.choice([-16.,-1.,0.,1.,16.]);w,h=r.choice([.001,.125,.25,.6,1.,1.8,2.]),r.choice([.001,.125,.25,.6,1.5,1.8,2.]);x+=r.choice([-.1,0.,.1,.5]);y+=r.choice([-.001,0.,.001,.1,.4,.5,.9]);z+=r.choice([-.1,0.,.1,.5]);blocks=[]
  for bx in range(int(x)-2,int(x)+3):
   for by in range(int(y)-1,int(y)+4):
    for bz in range(int(z)-2,int(z)+3):
     if r.random()<.2:blocks.append([bx,by,bz,r.choice(pools)])
  vx,vz=r.choice([-0.,0.,.002999999,.003,.01,-.01]),r.choice([-0.,0.,.002999999,.003,.01,-.01]);vy=r.choice([-1.,-0.,0.,.001,.01,1.]);queries.append(dict(op='push',blocks=blocks,bounds=vec([x-w/2,y,z-w/2,x+w/2,y+h,z+w/2]),velocity=vec([vx,vy,vz]),tag=r.choice([1,2]),factor=db(r.choice([0.,.0023333333333333335,.007,.014,.0045,1.,-.014])),player=bool(i%2),pushed=i%5!=0,loaded=i%7!=0,previous_height=db(r.choice([0.,.25,1.,10.]))))
 # Running depth must weight later currents, not each cell's own submersion.
 for player in [False,True]:
  for depth in [.05,.1,.399999,.4,.5,.9]:
   queries.append(dict(op='push',blocks=[[0,64,0,water[0]],[1,64,0,water[-1]],[0,64,-1,water[-1]],[2,64,0,1]],bounds=vec([0.,64.+8/9-depth,0.,2.,66.,1.]),velocity=vec([0.,0.,0.]),tag=1,factor=db(.014),player=player,pushed=True,loaded=True,previous_height=db(7.)))
 runtime=ROOT/'runtime/fluid-physics';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True);cp=os.pathsep.join(map(str,[classes,jar,*sorted((ORACLE/'libraries').rglob('*.jar'))]));flags=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED']
 compiled=subprocess.run([JDK/'javac',*flags,'-proc:none','-cp',cp,'-d',classes,ROOT/'tools/survival_oracle.java',ROOT/'tools/fluid_physics_oracle.java'],capture_output=True,text=True)
 if compiled.returncode:raise RuntimeError(compiled.stdout+compiled.stderr)
 transcript=''.join(json.dumps(q,separators=(',',':'))+'\n'for q in queries);java=subprocess.run([JDK/'java',*flags,'-Xmx1G','-cp',cp,'oracle.fluid_physics_oracle'],input=transcript,capture_output=True,text=True);(runtime/'oracle.log').write_text(java.stdout+java.stderr)
 if java.returncode:raise RuntimeError((java.stdout+java.stderr)[-6000:])
 outputs=[json.loads(line.split('ORACLE:',1)[1])for line in java.stdout.splitlines()if line.startswith('ORACLE:')];assert len(outputs)==len(queries),(len(outputs),len(queries));vectors=ROOT/'fixtures/fluid_physics_vectors.json';vectors.write_text(json.dumps(dict(minecraft='1.21',oracle_inner_sha256=SHA,cases=[dict(query=q,expected=e)for q,e in zip(queries,outputs)]),separators=(',',':'))+'\n')
 report=dict(minecraft='1.21',oracle_inner_sha256=SHA,queries=len(queries),operations=dict(collections.Counter(q['op']for q in queries)),block_states=len(states),fact_rows=len(rows),cached_support_face_states=data['cached_face_states'],explicit_unknown_support_face_states=data['uncached_face_states'],default_fluid_tags=tags,world_query_order_verified=False,vectors_sha256=hashlib.sha256(vectors.read_bytes()).hexdigest(),facts_sha256=hashlib.sha256((ROOT/'fixtures/fluid_physics_facts.json').read_bytes()).hexdigest(),native_source_sha256=hashlib.sha256((ROOT/'src/fluid_physics.rs').read_bytes()).hexdigest(),oracle_source_sha256=hashlib.sha256((ROOT/'tools/fluid_physics_oracle.java').read_bytes()).hexdigest(),scope='Original Entity.updateFluidHeightAndDoFluidPushing/touchingUnloadedChunk, FluidState.getHeight/getFlow and FlowingFluid.isSolidFace. Original methods unchanged; controlled world maps, availability and isPushedByFluid callbacks; exact shipped default fluid tags bound through original Holder.Reference.',not_verified=['Full baseTick/swimming/eye-fluid/fire/lava lifecycle and gameplay integration','Fluid random ticks/source creation/block replacement','Live uncached support faces including animated shulker/piston state','Replacement datapack fluid tags require host matches_tag override'])
 if not a.skip_rust:
  env=os.environ.copy();env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup',CARGO_INCREMENTAL='0');rust=subprocess.run(['/root/.local/share/mc-rust-toolchain/cargo/bin/cargo','test','--lib','fluid_physics::tests','--','--nocapture'],cwd=ROOT,env=env,text=True,capture_output=True);(runtime/'rust.log').write_text(rust.stdout+rust.stderr)
  if rust.returncode:raise RuntimeError((rust.stdout+rust.stderr)[-6000:])
  report.update(mismatches=0,world_query_order_verified=True,rust_build_profile='debug',rust_release_verified=False,rust_tests_passed=int(re.search(r'test result: ok\. (\d+) passed',rust.stdout).group(1)))
 (ROOT/'evidence/fluid-physics-conformance.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
