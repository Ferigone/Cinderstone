#!/usr/bin/env python3
"""ActualServerEntity interval20/velocity-threshold and metadata-vs-impulse oracle."""
import hashlib,json,math,struct,subprocess,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle';JAVA=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin');SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01';jar=ORACLE/'server-inner-1.21.jar';assert hashlib.sha256(jar.read_bytes()).hexdigest()==SHA
cases=[]
def p(x=8.5,yaw=0.,ground=True):return dict(x=x,y=65.,z=8.5,yaw=yaw,pitch=0.,on_ground=ground)
def add(op,pos,vel,**kwargs):cases.append(dict(op=op,position=pos,velocity=vel,dirty=False,impulse=False,**kwargs))
add('new',p(),[.1,.2,-.1],interval=20,track_delta=True,track_head=False)
for i in range(1250):
 vel=[(i%7)*.0001,-(i%17)*.00001,.1-i*.00001]
 if i%113==0:vel=[4.,-4.,0.]
 if i%127==0:vel=[0.,0.,0.]
 add('tick',p(x=8.5+i*.001,yaw=i*.05,ground=i%53<26),vel)
 cases[-1].update(dirty=i%31==0,impulse=i%47==0)
add('new',p(),[0.,0.,0.],interval=2,track_delta=False,track_head=True)
for i in range(400):
 add('tick',p(yaw=i*.125),[i*.01,0.,0.]);cases[-1].update(dirty=i%3==0,impulse=i%19==0)
add('new',p(),[.001,0.,0.],interval=20,track_delta=True,track_head=False)
for value in [math.sqrt(1e-7),math.nextafter(math.sqrt(1e-7),0.),math.nextafter(math.sqrt(1e-7),math.inf),0.,-0.,3.9,-3.9,4.,-4.,.00001,0.]:
 add('tick',p(),[value,0.,0.]);cases[-1]['impulse']=True
with tempfile.TemporaryDirectory(prefix='mc121-velocity-') as temp:
 temp=Path(temp);target=temp/'target';launch=temp/'launch';target.mkdir();launch.mkdir();cp=':'.join([str(jar),*[str(p)for p in sorted((ORACLE/'libraries').rglob('*.jar'))]])
 subprocess.run([str(JAVA/'javac'),'-proc:none','-cp',cp,'-d',str(target),str(ROOT/'tools/entity_motion_oracle.java')],check=True,capture_output=True)
 subprocess.run([str(JAVA/'javac'),'-d',str(launch),str(ROOT/'tools/entity_motion_launcher.java')],check=True,capture_output=True)
 run=subprocess.run([str(JAVA/'java'),'-cp',str(launch)+':'+cp,'oracle.entity_motion_launcher',str(target)],input=''.join(json.dumps(q)+'\n'for q in cases),text=True,capture_output=True,check=True)
 lines=[json.loads(line.split('ORACLE:',1)[1])for line in run.stdout.splitlines()if'ORACLE:'in line];assert len(lines)==len(cases)
 for query,line in zip(cases,lines):
  query['expected']=line;query['bits']={k:struct.pack('>f'if k in ['yaw','pitch']else'>d',v).hex()for k,v in query['position'].items()if k!='on_ground'};query['velocity_bits']=[struct.pack('>d',v).hex()for v in query['velocity']]
 p=ROOT/'tools/entity-velocity.json';p.write_text(json.dumps({'oracle':'actual Minecraft1.21 aqt.sendChanges; unmounted ITEM/player intervals, velocity and metadata/impulse gates','jar_sha256':SHA,'cases':cases},separators=(',',':'))+'\n');print(json.dumps({'cases':len(cases),'packets':sum(len(q['expected']['packets'])for q in cases),'file':str(p)}))
