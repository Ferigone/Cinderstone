#!/usr/bin/env python3
"""Original arq dependency order: fastutil streams and Guava least/trim ties."""
from pathlib import Path
import hashlib,json,os,random,subprocess,zipfile,math
ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle';JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin');PIN='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def i64(v):return (v+2**63)%2**64-2**63
def i32(v):return (v+2**31)%2**32-2**31
def pack(x,z):return i64((x&0xffffffff)|((z&0xffffffff)<<32))
def mix(v):
    h=(v*0x9e3779b97f4a7c15)&0xffffffffffffffff;h^=h>>32;return h^(h>>16)
def generate():
    cases=[];rng=random.Random(0x121767a7aa)
    def op(name,key):return dict(op=name,key=str(key))
    def setcase(name,ops):cases.append(dict(name=name,query=dict(kind='set',ops=ops)))
    def select(name,values,k,center=(0,0),ops=None):cases.append(dict(name=name,query=dict(kind='select',k=k,center=list(center),**(dict(input=list(map(str,values)))if ops is None else dict(ops=ops)))))
    edges=[0,-1,1,-2,2,-2**63,2**63-1,pack(-2**31,2**31-1),pack(2**31-1,-2**31)]
    setcase('zero-sign-extremes',[*[op('remove',k)for k in edges],*[op('insert',k)for k in edges],*[op('insert',k)for k in reversed(edges)],*[op('remove',k)for k in edges]])
    setcase('grow-shrink-reinsert',[*[op('insert',pack(i,i*3))for i in range(210)],*[op('remove',pack(i,i*3))for i in range(210)],*[op('insert',-i)for i in range(27)],*[op('remove',-i)for i in range(27)]])
    for bucket in [0,1,15,30,31]:
        keys=[k for k in range(-20000,20001)if k and mix(k)&31==bucket][:20];rng.shuffle(keys);ops=[op('insert',k)for k in keys];ops += [op('remove',keys[0]),op('remove',keys[9]),op('remove',keys[-1]),op('insert',0),op('remove',0)];ops += [op('insert',k)for k in reversed(keys)];ops += [op('remove',k)for k in keys];setcase('collision-wrap-'+str(bucket),ops)
    for seed in range(8):
        r=random.Random(0x121767a700+seed);pool=edges+[r.randrange(-2**63,2**63)for _ in range(70)];ops=[]
        for _ in range(175):ops.append(op('insert'if r.random()<.57 else'remove',r.choice(pool)))
        setcase('seeded-set-history-'+str(seed),ops)
    rings=[]
    for radius in range(1,15):
        for x in range(-radius,radius+1):
            for z in range(-radius,radius+1):
                if max(abs(x),abs(z))==radius:rings.append(pack(x,z))
    r=random.Random(0x767a7);r.shuffle(rings)
    for k in [0,1,2,3,5,9,16,31,64,127,256,len(rings),len(rings)+1]:select('ties-grid-stream-'+str(k),rings,k)
    for order,values in [('descending',list(reversed(rings))),('ascending',sorted(rings,key=lambda p:(i32(p)**2+i32(p>>32)**2))),('equal-rings',[pack(x,z)for x,z in [(1,0),(-1,0),(0,1),(0,-1),(1,0),(-1,0),(0,1),(0,-1)]*300])]:
        for k in [1,2,3,9,32,64,257]:select('topk-'+order+'-'+str(k),values,k)
    # Large >2k inputs exercise repeated partition/trimming; adversarial equal
    # bands also drive Guava's bounded quickselect fallback sorting subranges.
    for seed in range(12):
        r=random.Random(0x121767b000+seed);center=(r.randrange(-2**31,2**31),r.randrange(-2**31,2**31));values=[]
        for _ in range(2500+seed*13):
            if r.random()<.6:x,z=r.randrange(-60,61),r.randrange(-60,61)
            else:x,z=r.randrange(-2**31,2**31),r.randrange(-2**31,2**31)
            values.append(pack(x,z))
        for k in [1,2,9,32,64,257]:select('large-wrapping-'+str(seed)+'-'+str(k),values,k,center)
    for seed in range(20):
        r=random.Random(0x121767c000+seed);values=rings[:r.randrange(20,300)];ops=[op('insert',v)for v in values];ops += [op('remove',v)for v in values[::3]];ops += [op('insert',v)for v in reversed(values[::4])]
        for k in [1,9,64]:select('actual-set-stream-'+str(seed)+'-'+str(k),None,k,(r.randrange(-10,11),r.randrange(-10,11)),ops=ops)
    for norm in [1105,32045]:
        near=[]
        for x in range(-math.isqrt(norm),math.isqrt(norm)+1):
            z=math.isqrt(norm-x*x)
            if x*x+z*z==norm:
                near.append(pack(x,z))
                if z:near.append(pack(x,-z))
        far=[pack(1000+i,1000)for i in range(len(near))];select('actual-quickselect-fallback-'+str(norm),far+near,len(near));cases[-1]['query']['trace_fallback']=True
    overflow=[pack(x,z)for x,z in [(0,0),(46340,0),(46341,0),(32768,32768),(-32768,-32768),(-2**31,0),(2**31-1,0),(65536,0),(1,0),(0,1),(2**31-1,-2**31)]]
    for center in [(0,0),(-2**31,2**31-1),(2**31-1,-2**31)]:
        for k in [1,2,3,5,10,11,12]:select('signed-distance-overflow-'+str(center)+'-'+str(k),overflow,k,center)
    return cases

def main():
    inner=ORACLE/'server-inner-1.21.jar';outer=ORACLE/'server-1.21.jar';assert hashlib.sha256(inner.read_bytes()).hexdigest()==PIN;assert hashlib.sha1(outer.read_bytes()).hexdigest()=='450698d1863ab5180c25d7c804ef0fe6369dd1ba'
    dependencies={}
    with zipfile.ZipFile(outer)as jar:
        for line in jar.read('META-INF/libraries.list').decode().splitlines():
            if '\tit.unimi.dsi:fastutil:8.5.12\t'in line or'\tcom.google.guava:guava:32.1.2-jre\t'in line:
                sha,coordinate,path=line.split('\t');file=ORACLE/'libraries'/path;assert hashlib.sha256(file.read_bytes()).hexdigest()==sha;dependencies[coordinate]=dict(path=str(file),sha256=sha)
    runtime=ROOT/'runtime/player_chunk_sender_order';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True);cp=os.pathsep.join(map(str,[classes,inner,*sorted((ORACLE/'libraries').rglob('*.jar'))]));source=ROOT/'tools/player_chunk_sender_order_oracle.java';compile=subprocess.run([JDK/'javac','-proc:none','-cp',cp,'-d',classes,source],text=True,capture_output=True)
    if compile.returncode:raise RuntimeError(compile.stdout+compile.stderr)
    cases=generate();transcript=''.join(json.dumps(c['query'],separators=(',',':'))+'\n'for c in cases);run=subprocess.run([JDK/'java','-Xmx2G','-cp',cp,'oracle.player_chunk_sender_order_oracle'],input=transcript,text=True,capture_output=True);(runtime/'oracle.log').write_text(run.stdout+run.stderr)
    if run.returncode:raise RuntimeError((run.stdout+run.stderr)[-6500:])
    for dep in dependencies.values():assert str(dep['path'])in run.stdout
    assert 'SOURCE:minecraft:file:'+str(inner)in run.stdout
    results=[json.loads(s[7:])for s in run.stdout.splitlines()if s.startswith('ORACLE:')];assert len(results)==len(cases)
    setstates=0;different_ties=0;stream_iterator_differences=0
    for case,result in zip(cases,results):
        case['expected']=result
        if case['query']['kind']=='set':
            assert len(result)==len(case['query']['ops']);setstates+=len(result);stream_iterator_differences+=sum(s['stream']!=s['iterator']for s in result);assert all(s['stream']==s['long_stream']for s in result)
        else:
            different_ties+=result['least']!=result['full_sorted'][:case['query']['k']]
            if case['query'].get('trace_fallback'):assert result['fallback_observed']is True
    selection_items=sum(len(r['input'])for c,r in zip(cases,results)if c['query']['kind']=='select');trim_count=sum(r['trim_count']for c,r in zip(cases,results)if c['query']['kind']=='select')
    pool=[];indices={}
    def intern(obj,key):
        if key not in obj:return
        value=obj[key];serialized=json.dumps(value,separators=(',',':'))
        if serialized not in indices:indices[serialized]=len(pool);pool.append(value)
        obj[key]={'pool':indices[serialized]}
    for case in cases:
        if case['query']['kind']=='set':
            for state in case['expected']:
                for key in ['key_slots','iterator','stream','long_stream']:intern(state,key)
        else:
            for key in ['input','distances','least','full_sorted']:intern(case['expected'],key)
            intern(case['query'],'input')
    fixture=ROOT/'fixtures/player_chunk_sender_order.json';fixture.write_text(json.dumps(dict(minecraft='1.21',dependencies=dependencies,pool=pool,cases=cases),separators=(',',':'))+'\n')
    report=dict(ok=True,minecraft='1.21',oracle_inner_sha256=PIN,dependencies=dependencies,cases=len(cases),set_states=setstates,selection_cases=sum(c['query']['kind']=='select'for c in cases),selection_items=selection_items,actual_trim_calls=trim_count,actual_fallback_cases=sum(c['query'].get('trace_fallback',False)for c in cases),topk_tie_orders_different_from_stable_full_sort=different_ties,stream_iterator_order_differences=stream_iterator_differences,fixture_bytes=fixture.stat().st_size,fixture_sha256=hashlib.sha256(fixture.read_bytes()).hexdigest(),oracle_source_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),generator_source_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),scope=['unchanged shipped LongOpenHashSet default mutations, complete key layout and zero state, iterator versus both stream orders','actual Guava Comparators.least sequential collector with original ChunkPos.distanceSquared comparator; ties, repeated trim, >2k input and signed int overflow','full-sort actual Java reference retained to demonstrate that least tie order is not a stable full-sort prefix'],not_verified=['arq public quota/ACK/readiness and chunk delivery pipeline owned by separate sender oracle','parallel stream combining, iterator mutation/serialization/trim/nondefault set constructor or alternate load factors','invalid Guava k arguments beyond its required0..INT_MAX/2 precondition, allocation failure and unrelated custom comparators'])
    (ROOT/'evidence/player-chunk-sender-order-oracle.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
