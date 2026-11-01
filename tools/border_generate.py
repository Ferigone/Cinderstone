#!/usr/bin/env python3
"""Generate border vectors by invoking unchanged signed Minecraft1.21 code."""
from pathlib import Path
import base64,hashlib,json,os,random,struct,subprocess

ROOT=Path(__file__).resolve().parents[1]
ORACLE=ROOT.parent/'mc121-oracle'
JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
NAMES=['BorderCenterX','BorderCenterZ','BorderDamagePerBlock','BorderSafeZone','BorderWarningBlocks','BorderWarningTime','BorderSize','BorderSizeLerpTime','BorderSizeLerpTarget']

def db(v):return struct.pack('>d',v).hex()
def defaults():return [db(0),db(0),db(.2),db(5),5,15,db(59999968),0,db(0)]
def text(s):
    raw=s.encode('utf-8');return struct.pack('>H',len(raw))+raw
def payload(kind,value):
    if kind in range(1,7):return struct.pack({1:'>b',2:'>h',3:'>i',4:'>q',5:'>f',6:'>d'}[kind],value)
    if kind==8:return text(value)
    if kind==7:return struct.pack('>i',len(value))+value
    if kind==10:return b'\0'
    if kind==9:return bytes([3])+struct.pack('>i',2)+struct.pack('>ii',13,-19)
    raise ValueError(kind)
def compound(entries):
    raw=bytes([10,0,0])
    for name,kind,value in entries:raw+=bytes([kind])+text(name)+payload(kind,value)
    return base64.b64encode(raw+b'\0').decode('ascii')
def read_query(entries,base=None):
    return dict(op='read',input=compound([('OpaqueProbe',8,'retained'),*entries]),defaults=base or defaults())

def queries():
    r=random.Random(0x121767B04DE4)
    reads=[dict(read_query([]),official_defaults=True)]
    values={1:[-128,-127,-1,0,1,127],2:[-32768,-30000,-1,0,1,32767],3:[-2**31,-30000000,-1,0,1,29999984,30000000,2**31-1],4:[-2**63,-2**63+1,-2**32-1,-2**31-1,-30000000,-1,0,1,2**31,2**32+17,2**63-1],5:[-float('inf'),-3.4e38,-30000000.,-2.9,-.9,-0.,0.,.9,2.9,30000000.,3.4e38,float('inf'),float('nan')],6:[-float('inf'),-1e308,-float(2**63),-30000000.,-2.9,-.9,-0.,0.,.9,2.9,30000000.,float(2**63),1e308,float('inf'),float('nan')]}
    for name in NAMES:
        for kind,items in values.items():
            for value in items:reads.append(read_query([(name,kind,value)]))
        for kind,value in [(7,b'\x00\x01\xff'),(8,'123.75'),(9,None),(10,None)]:reads.append(read_query([(name,kind,value)]))
    for index in range(1050):
        base=[db(r.choice([-30000001.,-7.25,-0.,0.,21.5,30000001.])),db(r.uniform(-4e7,4e7)),db(r.uniform(-10,10)),db(r.uniform(-100,100)),r.randint(-2**31,2**31-1),r.randint(-2**31,2**31-1),db(r.uniform(-1e8,1e8)),r.choice([-2**63,-1,0,1,2**63-1]),db(r.uniform(-1e8,1e8))]
        entries=[]
        for name in NAMES:
            if r.randrange(5)==0:continue
            kind=r.choice([1,2,3,4,5,6,7,8,9,10])
            value=r.choice(values[kind])if kind in values else {7:b'\0\1\2',8:'7',9:None,10:None}[kind]
            entries.append((name,kind,value))
        reads.append(read_query(entries,base))
    worlds=[]
    for index in range(120):
        start=r.choice([-9000000000000,-1000000000,-10,0,10,1000,9000000000000])
        size=r.choice([0.,.125,1.,2.,5.,16.,1000.,59999968.,6e7,1e100])
        target=r.choice([0.,.25,2.,100.,1e7,59999968.])
        duration=r.choice([-1,0,1,2,20,1000,100000,2**63-1])
        setting=[db(r.choice([-3e7,-29999984.,-1.5,0.,1.5,29999984.,3e7])),db(r.uniform(-3e7,3e7)),db(r.uniform(-1,2)),db(r.uniform(-10,10)),r.randint(-20,20),r.randint(-20,20),db(size),duration,db(target)]
        actions=[]
        def action(op,offset=0,**fields):actions.append(dict(op=op,now_ms=start+offset,**fields))
        # Clock values stay representable as nanoseconds. Huge duration values
        # still exercise Java signed-long end-time arithmetic and subtraction.
        for offset in [-1,0,1,2,19,20,21,999,1000,1001]:action('snapshot',offset)
        action('tick',1001)
        action('center',1002,x=db(r.uniform(-4e7,4e7)),z=db(r.uniform(-4e7,4e7)))
        action('absolute',1003,value=r.choice([1,16,128,1000000,29999984,2**31-1]))
        action('size',1004,size=db(r.choice([0.,.5,1.,9.,100000.,59999968.])))
        frm=r.choice([0.,.125,1.,2.,128.,1e7,59999968.]);to=frm if index%7==0 else r.choice([0.,.25,2.,64.,4096.,59999968.])
        action('lerp',1005,**{'from':db(frm),'to':db(to),'duration':r.choice([-2**63,-1,0,1,3,1000,2**63-1])})
        for offset in [1004,1005,1006,1007,2005,2006]:action('snapshot',offset)
        action('tick',2006)
        worlds.append(dict(op='world',settings=setting,start_ms=start,absolute_max=r.choice([16,1000,29999984]),actions=actions))
    return reads,worlds

def main():
    jar=ORACLE/'server-inner-1.21.jar';assert hashlib.sha256(jar.read_bytes()).hexdigest()==SHA
    run=ROOT/'runtime/border-oracle';classes=run/'classes';classes.mkdir(parents=True,exist_ok=True)
    cp=os.pathsep.join(map(str,[classes,jar,*sorted((ORACLE/'libraries').rglob('*.jar'))]))
    compile=subprocess.run([JDK/'javac','-proc:none','-cp',cp,'-d',classes,ROOT/'tools/border_oracle.java'],text=True,capture_output=True)
    if compile.returncode:raise RuntimeError(compile.stdout+compile.stderr)
    reads,worlds=queries();all_queries=reads+worlds
    result=subprocess.run([JDK/'java','-Xmx1G','-cp',cp,'oracle.border_oracle'],input=''.join(json.dumps(q,separators=(',',':'))+'\n'for q in all_queries),capture_output=True,text=True)
    (run/'oracle.log').write_text(result.stdout+result.stderr)
    if result.returncode:raise RuntimeError((result.stdout+result.stderr)[-7000:])
    output=[json.loads(line.split('ORACLE:',1)[1])for line in result.stdout.splitlines()if line.startswith('ORACLE:')]
    assert len(output)==len(all_queries),(len(output),len(all_queries))
    fixture=dict(minecraft='1.21',oracle_inner_sha256=SHA,clock='originalUtil.getMillis with injected NanoTimeSource.getAsLong; no original methods rewritten',settings_cases=[dict(query=q,expected=o)for q,o in zip(reads,output[:len(reads)])],world_cases=[dict(query=q,expected=o)for q,o in zip(worlds,output[len(reads):])])
    target=ROOT/'fixtures/border_vectors.json';target.write_text(json.dumps(fixture,separators=(',',':'))+'\n')
    print(json.dumps(dict(ok=True,settings_cases=len(reads),world_cases=len(worlds),world_states=sum(len(q['actions'])for q in worlds),point_queries=sum(len(state['queries'])for result in output[len(reads):]for state in result),fixture_sha256=hashlib.sha256(target.read_bytes()).hexdigest(),fixture_bytes=target.stat().st_size)))

if __name__=='__main__':main()
