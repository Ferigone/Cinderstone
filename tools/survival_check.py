#!/usr/bin/env python3
"""Generate vectors by calling unchanged signed Minecraft1.21 survival classes."""
from pathlib import Path
import argparse,hashlib,json,os,random,re,shutil,struct,subprocess
ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle';JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
def fb(v):return struct.pack('>f',v).hex()
def db(v):return struct.pack('>d',v).hex()
def main():
    p=argparse.ArgumentParser();p.add_argument('--skip-rust',action='store_true');a=p.parse_args();jar=ORACLE/'server-inner-1.21.jar';sha=hashlib.sha256(jar.read_bytes()).hexdigest();assert sha=='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
    runtime=ROOT/'runtime/survival';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True);cp=os.pathsep.join(map(str,[classes,jar,*sorted((ORACLE/'libraries').rglob('*.jar'))]));flags=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED']
    subprocess.run([JDK/'javac',*flags,'-proc:none','-cp',cp,'-d',classes,ROOT/'tools/survival_oracle.java'],check=True,capture_output=True)
    rng=random.Random(0x1217675);queries=[];edge=[-0.,0.,-1.,1e-38,1e-6,1.,3.,4.,4.0000005,5.,6.,20.,40.,100.,float('nan'),float('inf'),-float('inf')]
    def val():return rng.choice(edge)if rng.random()<.4 else rng.uniform(-100,100)
    for i in range(1800):
        queries.append(dict(op='armor',damage=fb(val()),armor=fb(val()),toughness=fb(val())))
        queries.append(dict(op='magic',damage=fb(val()),protection=fb(val())))
        queries.append(dict(op='fall',distance=fb(val()),block_multiplier=fb(val()),safe=db(rng.choice([0.,3.,5.,64.,1024.])),attribute_multiplier=db(rng.choice([0.,.5,1.,2.,10.])),immune=i%7==0))
        if i%3==0:queries.append(dict(op='resistance',damage=fb(val()),amplifier=rng.randrange(8)))
        if i%5==0:queries.append(dict(op='subtract',health=fb(rng.choice([0.,.5,1.,10.,20.,40.])),max=fb(40.),absorption=fb(rng.choice([0.,1.,10.,20.])),max_absorption=fb(20.),damage=fb(val())))
    for i in range(60):
        initial=[rng.choice([-1,0,1,17,18,19,20,21,2147483647,-2147483648]),fb(rng.choice([0.,.5,1.,5.,6.,20.,float('nan')])),fb(rng.choice([0.,4.,4.0000005,40.,float('nan')])),rng.choice([0,9,10,79,80,2147483647,-2147483648]),20]
        steps=[]
        for phase in range(4):
            diff=phase%4;natural=rng.choice([True,False]);health=rng.choice([0.,.5,1.,9.,10.,10.001,19.,20.,float('nan')]);maximum=rng.choice([20.,40.])
            steps += [['tick',diff,natural,fb(health),fb(maximum)]for _ in range(85)]
            steps += [['exhaust',fb(val())],['eat',rng.choice([0,1,4,20,-10,2147483647]),fb(val())],['props',rng.choice([0,1,4,20,-10]),fb(val())],['cause',fb(val()),rng.choice([True,False]),rng.choice([True,False])],['caneat',rng.choice([True,False]),rng.choice([True,False])]]
            steps += [['peace',diff,natural,fb(health),fb(maximum),tick]for tick in [-20,-10,0,1,9,10,19,20,40]]
        queries.append(dict(op='food',initial=initial,steps=steps))
    for i in range(120):
        queries.append(dict(op='health',health=fb(val()),max=fb(rng.choice([1.,20.,40.,1024.])),absorption=fb(val()),max_absorption=fb(rng.choice([0.,20.,100.,2048.])),steps=[[rng.choice(['heal','set','absorb']),fb(val())]for _ in range(20)]))
    for i in range(150):
        entries=[]
        for key in ['foodLevel','foodTickTimer','foodSaturationLevel','foodExhaustionLevel']:
            if rng.random()<.25:continue
            kind=rng.choice(['byte','short','int','long','float','double','string'])
            value=fb(val())if kind=='float'else db(val())if kind=='double'else 'wrong type'if kind=='string'else rng.choice([-9223372036854775808,9223372036854775807,-2147483648,2147483647,-129,256,65536])if kind=='long'else rng.choice([-2147483648,2147483647,-129,256,65536])
            entries.append([key,kind,value])
        queries.append(dict(op='food_nbt',initial=[18,fb(6.),fb(7.),79,17],entries=entries))
    queries += [dict(op='fall_flight_gate',distance=fb(value))for value in [-1.,0.,3.,10.,float('inf'),float('nan')]]
    transcript=''.join(json.dumps(q,separators=(',',':'))+'\n'for q in queries)
    java=subprocess.run([JDK/'java',*flags,'-Xmx1G','-cp',cp,'oracle.survival_oracle'],input=transcript,text=True,capture_output=True);(runtime/'oracle.log').write_text(java.stdout+java.stderr)
    if java.returncode:raise RuntimeError((java.stdout+java.stderr)[-2200:])
    outputs=[json.loads(line.split('ORACLE:',1)[1])for line in java.stdout.splitlines()if line.startswith('ORACLE:')];assert len(outputs)==len(queries),(len(outputs),len(queries))
    vectors={'minecraft':'1.21','oracle_inner_sha256':sha,'cases':[dict(query=q,expected=o)for q,o in zip(queries,outputs)]};vp=ROOT/'fixtures/survival-vectors.json';vp.write_text(json.dumps(vectors,separators=(',',':'))+'\n')
    report=dict(minecraft='1.21',oracle_inner_sha256=sha,queries=len(queries),food_ticks=sum(sum(s[0]=='tick'for s in q.get('steps',[]))for q in queries),observed_states=sum(len(v)if isinstance(v,list)else 1 for v in outputs),vectors_sha256=hashlib.sha256(vp.read_bytes()).hexdigest(),native_source_sha256=hashlib.sha256((ROOT/'src/survival.rs').read_bytes()).hexdigest(),scope='Original FoodData, CombatRules, LivingEntity health/absorption/fall methods and Player numeric damage subtraction with controlled contexts/callbacks; peaceful aiStep prefix stops at missing inventory. Full hurt cooldown, death, equipment/enchantment and world side effects remain caller responsibilities.')
    if not a.skip_rust:
        env=os.environ.copy();env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup');cargo=shutil.which('cargo')or'/root/.local/share/mc-rust-toolchain/cargo/bin/cargo';rust=subprocess.run([cargo,'test','--lib','survival::tests','--','--nocapture'],cwd=ROOT,env=env,text=True,capture_output=True);(runtime/'rust.log').write_text(rust.stdout+rust.stderr)
        if rust.returncode:raise RuntimeError((rust.stdout+rust.stderr)[-6000:])
        report.update(mismatches=0,rust_tests_passed=int(re.search(r'test result: ok\. (\d+) passed',rust.stdout).group(1)))
    (ROOT/'tools/survival_conformance.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
