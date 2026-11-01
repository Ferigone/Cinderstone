#!/usr/bin/env python3
"""Additional unchanged-cjh tick vectors for continuously submerged items."""
from pathlib import Path
import hashlib,json,os,struct,subprocess
ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle';JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin');SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def db(v):return struct.pack('>d',v).hex()
def fb(v):return struct.pack('>f',v).hex()
def main():
 assert hashlib.sha256((ORACLE/'server-inner-1.21.jar').read_bytes()).hexdigest()==SHA
 f=json.loads((ROOT/'fixtures/fluid_physics_facts.json').read_text());examples={}
 for state,index in enumerate(f['states']):
  row=f['rows'][index]
  if row[0]==1:examples.setdefault((row[2],row[3],row[4]),state)
 source=examples[(8,False,True)];queries=[]
 def add(current,above=False,wall=False,x=.5,z=.5,velocity=(0.,0.,0.),bottom=64.03,blocks=None):
  cells={(bx,by,bz):current for bx in range(-6,7)for bz in range(-6,7)for by in ([64,65]if above else[64])}
  if blocks:
   for bx,by,bz,state in blocks:cells[(bx,by,bz)]=state
  if wall:
   for by in range(64,67):cells[(1,by,0)]=1
  queries.append(dict(position=list(map(db,[x,bottom,z])),velocity=list(map(db,velocity)),blocks=[[*p,s]for p,s in cells.items()],entity_id=501+len(queries),seed=-712100+len(queries),tick_count=0,age=0,pickup_delay=40,on_ground=False,fall_distance=fb(0.),no_gravity=False,ticks=20,drain_initial_metadata=True,clear_has_impulse=True))
 for key,state in list(examples.items()):
  add(state,above=key[0]%2==0,wall=key[1],velocity=(.005,0.,-.005))
  if len(queries)>=16:break
 for above in [False,True]:
  for velocity in [(0.,0.,0.),(.05,0.,0.),(-0.,-.005,-0.)]:add(source,above=above,velocity=velocity,bottom=64.1)
 # Waterlogged bottom slabs keep the item's box above the dry collision slab.
 reports=json.loads((ORACLE/'reports/reports/blocks.json').read_text())
 slabs=[s['id']for s in reports['minecraft:oak_slab']['states']if s.get('properties',{}).get('waterlogged')=='true'and s.get('properties',{}).get('type')=='bottom']
 assert len(slabs)==1
 for above in [False,True]:add(source,above=above,bottom=64.55,blocks=[[0,64,0,slabs[0]]])
 runtime=ROOT/'runtime/item-water';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True);cp=os.pathsep.join(map(str,[classes,ORACLE/'server-inner-1.21.jar',*sorted((ORACLE/'libraries').rglob('*.jar'))]));flags=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED'];sources=[ROOT/'tools'/s for s in ['survival_oracle.java','placement_tags.java','item_full_oracle.java']]
 compiled=subprocess.run([JDK/'javac',*flags,'-proc:none','-cp',cp,'-d',classes,*sources],text=True,capture_output=True)
 if compiled.returncode:raise RuntimeError(compiled.stdout+compiled.stderr)
 java=subprocess.run([JDK/'java',*flags,'-Xmx1G','-cp',cp,'oracle.item_full_oracle',ROOT/'fixtures/placement-block-tags-1.21.json'],input=''.join(json.dumps(q,separators=(',',':'))+'\n'for q in queries),text=True,capture_output=True);(runtime/'oracle.log').write_text(java.stdout+java.stderr)
 if java.returncode:raise RuntimeError((java.stdout+java.stderr)[-6500:])
 outputs=[json.loads(line.removeprefix('ORACLE:'))for line in java.stdout.splitlines()if line.startswith('ORACLE:')];assert len(outputs)==len(queries)
 data=dict(minecraft='1.21',oracle_inner_sha256=SHA,entity_class='cjh exact original class; no entity subclasses or method overrides',scope='Continuously submerged item ticks: all distinct extracted water source/flowing/falling FluidState combinations, same-water above, adjacent stone wall, float eye/depth boundaries and two waterlogged slab contexts; actual inherited base/move/fluid routines.',not_verified=['Entry/exit splash sound/world RNG and particle callbacks','Lava damage, replaced fluid tags, dynamic support faces','Water fall-damage-reset ray cache history'],scenes=[dict(query=q,**out)for q,out in zip(queries,outputs)])
 path=ROOT/'fixtures/item-water-vectors.json';path.write_text(json.dumps(data,separators=(',',':'))+'\n');print(json.dumps(dict(file=str(path),scenes=len(queries),states=sum(len(o['states'])for o in outputs),sha256=hashlib.sha256(path.read_bytes()).hexdigest())))
if __name__=='__main__':main()
