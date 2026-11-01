#!/usr/bin/env python3
"""Capture byte-exact motion from actual unmodified vanilla ServerEntity."""
import hashlib,json,math,random,struct,subprocess,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
ORACLE=ROOT.parent/'mc121-oracle'
JAVA=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
jar=ORACLE/'server-inner-1.21.jar';assert hashlib.sha256(jar.read_bytes()).hexdigest()==SHA
cases=[]
def pos(x=8.5,y=65.,z=8.5,yaw=0.,pitch=0.,on_ground=True):
 return dict(x=x,y=y,z=z,yaw=yaw,pitch=pitch,on_ground=on_ground)
def add(op,p,force=False):cases.append(dict(op=op,position=p,force=force))
add('new',pos())
for _ in range(825):add('tick',pos())
add('new',pos(x=-.5,yaw=-1.,pitch=1.))
for i in range(1,251):add('tick',pos(x=-.5+i*.00025,yaw=-1+i*.05,pitch=1-i*.03))
add('new',pos())
for i in range(180):
 add('tick',pos(x=8.5+(9 if i>=45 else 0)+(-18 if i>=90 else 0),yaw=-1 if i%15<8 else 91.,on_ground=i%13<7),i%17==0)
r=random.Random(767121)
p=pos();add('new',p)
for i in range(1250):
 p={**p,'x':p['x']+r.uniform(-.004,.004),'y':p['y']+r.uniform(-.001,.001),'z':p['z']+r.uniform(-.004,.004),'yaw':p['yaw']+r.uniform(-1.,1.),'pitch':p['pitch']+r.uniform(-.1,.1)}
 if i%113==0:p['x']+=r.choice([-8.,8.,-9.,9.])
 if i%127==0:p['on_ground']=not p['on_ground']
 add('tick',p,i%31==0)
for yaw in [-360.,-90.,-1.,-.5,0.,1.,90.,360.,720.5,-720.5]:
 p=pos(x=-29_999_900.,z=29_999_900.,yaw=yaw,pitch=-yaw);add('new',p)
 for i in range(4):add('tick',{**p,'x':p['x']+i*math.sqrt(7.62939453125E-6)})
with tempfile.TemporaryDirectory(prefix='mc121-motion-') as temp:
 temp=Path(temp);target=temp/'target';launch=temp/'launch';target.mkdir();launch.mkdir()
 cp=':'.join([str(jar),*[str(p) for p in sorted((ORACLE/'libraries').rglob('*.jar'))]])
 subprocess.run([str(JAVA/'javac'),'-proc:none','-cp',cp,'-d',str(target),str(ROOT/'tools/entity_motion_oracle.java')],check=True,capture_output=True)
 subprocess.run([str(JAVA/'javac'),'-d',str(launch),str(ROOT/'tools/entity_motion_launcher.java')],check=True,capture_output=True)
 run=subprocess.run([str(JAVA/'java'),'-cp',str(launch)+':'+cp,'oracle.entity_motion_launcher',str(target)],input=''.join(json.dumps(q)+'\n' for q in cases),text=True,capture_output=True,check=True)
 lines=[json.loads(line.split('ORACLE:',1)[1]) for line in run.stdout.splitlines() if 'ORACLE:' in line]
 assert len(lines)==len(cases),(len(lines),len(cases),run.stderr[-4000:])
 for case,line in zip(cases,lines):
  case.update(expected=line)
  case['bits']={key:struct.pack('>f' if key in ['yaw','pitch'] else '>d',value).hex() for key,value in case['position'].items() if key!='on_ground'}
 output={'oracle':'actual Minecraft 1.21 aqt.sendChanges; unmounted entity, player interval2, zero velocity','jar_sha256':SHA,'cases':cases}
 path=ROOT/'tools/entity-motion.json';path.write_text(json.dumps(output,separators=(',',':'))+'\n')
 print(json.dumps({'captured':len(cases),'packets':sum(len(c['expected']['packets']) for c in cases),'bytes':path.stat().st_size,'file':str(path)}))
