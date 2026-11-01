#!/usr/bin/env python3
"""Record default tracked-entity map order from Mojang's bundled fastutil JAR."""
from pathlib import Path
import hashlib,json,os,random,subprocess,zipfile
ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle'
JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
OUTER_SHA1='450698d1863ab5180c25d7c804ef0fe6369dd1ba'
INNER_SHA256='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def mix(key):
    value=(key*0x9e3779b9)&0xffffffff
    return (value^(value>>16))&0xffffffff
def i32(value):return (value+2**31)%2**32-2**31
def generate():
    cases=[];rng=random.Random(0x1217674a9)
    def add(name,ops):cases.append(dict(name=name,ops=[op.copy()for op in ops]))
    def put(key,value):return dict(op='put',key=key,value=value)
    def remove(key):return dict(op='remove',key=key)
    clear=dict(op='clear')
    edges=[0,-2147483648,2147483647,-1,1,-2,2,-1000,1000]
    add('zero-sign-extremes-replace', [remove(0),clear,*[put(k,i)for i,k in enumerate(edges)],*[put(k,-i)for i,k in enumerate(reversed(edges))],*[remove(k)for k in edges],remove(0),clear])
    add('default-grow-thresholds-and-shrink', [*[put(i,i32(i*1234567))for i in range(190)],*[remove(i)for i in range(190)],clear,put(0,0),put(1,1),clear])
    add('clear-retains-grown-capacity', [*[put(i,-i)for i in range(110)],clear,*[put(-i,i)for i in range(50)],clear,*[put(i,i+1)for i in range(25)],clear])
    # Choose actual fastutil mix buckets near the wrap boundary, including long
    # clusters, removal of first/middle/last, replacement and reinsertion.
    for bucket in [0,1,15,30,31]:
        candidates=[k for k in range(-12000,12001)if k and (mix(k)&31)==bucket][:18]
        rng.shuffle(candidates)
        ops=[put(k,i)for i,k in enumerate(candidates)]
        ops.extend([put(candidates[0],999),remove(candidates[0]),remove(candidates[8]),remove(candidates[-1]),put(0,-1),remove(0)])
        ops.extend(put(k,i+100)for i,k in enumerate(reversed(candidates)))
        ops.extend(remove(k)for k in candidates[::2]);ops.extend(remove(k)for k in candidates[1::2]);add('collision-wrap-bucket-'+str(bucket),ops)
    for seed in range(6):
        local=random.Random(0x1217674a900+seed);pool=edges+[local.randint(-2147483648,2147483647)for _ in range(120)];ops=[]
        for _ in range(350):
            chance=local.random();key=local.choice(pool)
            if chance<.60:ops.append(put(key,local.randint(-2147483648,2147483647)))
            elif chance<.985:ops.append(remove(key))
            else:ops.append(clear.copy())
        add('seeded-operation-history-'+str(seed),ops)
    return cases
def main():
    outer=ORACLE/'server-1.21.jar';assert hashlib.sha1(outer.read_bytes()).hexdigest()==OUTER_SHA1
    inner=ORACLE/'server-inner-1.21.jar';assert hashlib.sha256(inner.read_bytes()).hexdigest()==INNER_SHA256
    with zipfile.ZipFile(outer)as archive:
        line=next(line for line in archive.read('META-INF/libraries.list').decode().splitlines()if '\tit.unimi.dsi:fastutil:8.5.12\t'in line)
    expected_sha,coordinate,path=line.split('\t');jar=ORACLE/'libraries'/path
    assert hashlib.sha256(jar.read_bytes()).hexdigest()==expected_sha
    runtime=ROOT/'runtime/entity_tracking_order';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True);cp=os.pathsep.join(map(str,[classes,jar,*sorted((ORACLE/'libraries').rglob('gson-*.jar'))]));source=ROOT/'tools/entity_tracking_order_oracle.java'
    compile=subprocess.run([JDK/'javac','-proc:none','-cp',cp,'-d',classes,source],text=True,capture_output=True)
    if compile.returncode:raise RuntimeError(compile.stdout+compile.stderr)
    cases=generate();ops=[op for case in cases for op in case['ops']];assert len({id(op)for op in ops})==len(ops),'operation objects must not alias';transcript=''.join(json.dumps(case,separators=(',',':'))+'\n'for case in cases)
    java=subprocess.run([JDK/'java','-Xmx1G','-cp',cp,'oracle.entity_tracking_order_oracle'],input=transcript,text=True,capture_output=True);(runtime/'oracle.log').write_text(java.stdout+java.stderr)
    if java.returncode:raise RuntimeError((java.stdout+java.stderr)[-6000:])
    assert 'SOURCE:file:'+str(jar)in java.stdout
    states=[json.loads(line.split('ORACLE:',1)[1])for line in java.stdout.splitlines()if line.startswith('ORACLE:')];assert len(states)==len(cases)
    for case,result in zip(cases,states):
        assert len(case['ops'])==len(result)
        previous_capacity=32
        for op,value in zip(case['ops'],result):
            if op['op']=='clear':
                assert value['capacity']==previous_capacity and value['size']==0 and value['entries']==[] and value['old']is None
                assert not value['contains_zero'] and not any(value['key_slots'])
            op['expected']=value;previous_capacity=value['capacity']
    fixture=ROOT/'fixtures/entity_tracking_order.json';fixture.write_text(json.dumps(dict(minecraft='1.21',dependency=coordinate,dependency_sha256=expected_sha,cases=cases),separators=(',',':'))+'\n');count=sum(len(case['ops'])for case in cases);all_states=[value for scene in states for value in scene]
    report=dict(ok=True,minecraft='1.21',official_outer_sha1=OUTER_SHA1,oracle_inner_sha256=INNER_SHA256,dependency=coordinate,dependency_sha256=expected_sha,dependency_hash_source='pinned official server META-INF/libraries.list',actual_class='it.unimi.dsi.fastutil.ints.Int2ObjectOpenHashMap',construction='unchanged default constructor; load factor0.75, default expected16',cases=len(cases),states=count,operations={kind:sum(op['op']==kind for case in cases for op in case['ops'])for kind in ['put','remove','clear']},capacities=sorted({value['capacity']for value in all_states}),fixture_bytes=fixture.stat().st_size,fixture_sha256=hashlib.sha256(fixture.read_bytes()).hexdigest(),oracle_source_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),generator_source_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),input_sha256=hashlib.sha256(transcript.encode()).hexdigest(),trace_sha256=hashlib.sha256('\n'.join(json.dumps(value,sort_keys=True,separators=(',',':'))for scene in states for value in scene).encode()).hexdigest(),scope=['put/remove/clear original return values and exact entry/value iteration order','zero-key slot, negative/min/max keys, linear-probe wrap, replacement, grow/shrink thresholds and retained capacity after clear','complete key-slot array, contains-zero flag, current capacity/minN/maxFill after every operation'],not_verified=['nondefault map constructor, trim, iterator mutation, serialization and custom load factors','native Rust comparison owned by TrackingEntityMap implementer separately'])
    (ROOT/'evidence/entity-tracking-order-oracle.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
