#!/usr/bin/env python3
"""Capture exact keepalive decisions from Minecraft 1.21 ServerCommonPacketListenerImpl."""
import hashlib,json,random,subprocess,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle'
JAVA=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
jar=ORACLE/'server-inner-1.21.jar';assert hashlib.sha256(jar.read_bytes()).hexdigest()==SHA
cases=[]
def add(op,now,**kwargs):cases.append(dict(op=op,now=now,**kwargs))
def new(now=0,latency=0,owner=False):add('new',now,latency=latency,owner=owner)
new(123)
for now in [123,15_122,15_123,30_122,30_123]:add('tick',now)
new();add('tick',15000);add('receive',15017,id=15000);add('receive',15018,id=15000);add('tick',29999);add('tick',30000);add('receive',30007,id=1);add('receive',30009,id=30000)
new();add('close',10000);add('tick',15000);add('close',20000);add('tick',24999);add('tick',25000)
new(owner=True,latency=5);add('tick',100000);add('receive',100001,id=123)
r=random.Random(76715000)
for i in range(120):
 start=r.randrange(1_000_000_000);latency=r.choice([0,1,100,2**31-1,-2**31,r.randrange(-2**31,2**31)])
 new(start,latency)
 challenge=start+15000
 for now in [challenge-1,challenge]:add('tick',now)
 if i%3==0:add('receive',challenge+r.randrange(1,200),id=challenge+1)
 rtt=r.choice([0,1,3,4,15,15000,2**31-1,2**31,2**32-1,2**32+100])
 add('receive',challenge+rtt,id=challenge)
 add('receive',challenge+rtt+1,id=challenge)
 add('tick',challenge+15000)
 add('tick',challenge+30000)
with tempfile.TemporaryDirectory(prefix='mc121-session-') as temp:
 temp=Path(temp);target=temp/'target';launch=temp/'launch';target.mkdir();launch.mkdir()
 cp=':'.join([str(jar),*[str(p) for p in sorted((ORACLE/'libraries').rglob('*.jar'))]])
 subprocess.run([str(JAVA/'javac'),'-proc:none','-cp',cp,'-d',str(target),str(ROOT/'tools/session_oracle.java')],check=True,capture_output=True)
 subprocess.run([str(JAVA/'javac'),'-d',str(launch),str(ROOT/'tools/session_launcher.java')],check=True,capture_output=True)
 run=subprocess.run([str(JAVA/'java'),'-cp',str(launch)+':'+cp,'oracle.session_launcher',str(target)],input=''.join(json.dumps(q)+'\n' for q in cases),text=True,capture_output=True,check=True)
 lines=[json.loads(line.split('ORACLE:',1)[1]) for line in run.stdout.splitlines() if 'ORACLE:' in line]
 assert len(lines)==len(cases),(len(lines),len(cases),run.stderr[-4000:])
 for case,line in zip(cases,lines):case.update(expected=line)
 output={'oracle':'actual Minecraft1.21 arr keepConnectionAlive + handleKeepAlive; controlled monotonic clock','jar_sha256':SHA,'cases':cases}
 path=ROOT/'tools/session-keepalive.json';path.write_text(json.dumps(output,separators=(',',':'))+'\n')
 print(json.dumps({'captured':len(cases),'file':str(path)}))
