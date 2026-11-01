#!/usr/bin/env python3
"""Actual RandomSupport/factory bytecode with injected nanoTime boundary."""
from pathlib import Path
import argparse,hashlib,json,os,random,re,subprocess,time
from urllib.parse import unquote,urlparse
ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle';JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin');SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def main():
    p=argparse.ArgumentParser();p.add_argument('--check-rust',action='store_true');a=p.parse_args()
    jar=ORACLE/'server-inner-1.21.jar';assert hashlib.sha256(jar.read_bytes()).hexdigest()==SHA
    runtime=ROOT/'runtime/random-seed';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True);cp=os.pathsep.join(map(str,[classes,jar,*sorted((ORACLE/'libraries').rglob('*.jar'))]));flags=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED']
    compile=subprocess.run([JDK/'javac',*flags,'-proc:none','-cp',cp,'-d',classes,ROOT/'tools/random_seed_oracle.java'],text=True,capture_output=True)
    if compile.returncode:raise RuntimeError(compile.stdout+compile.stderr)
    manifest=runtime/'manifest.mf';manifest.write_text('Manifest-Version: 1.0\nPremain-Class: oracle.random_seed_oracle\n\n');agent=runtime/'random-seed-agent.jar'
    subprocess.run([JDK/'jar','--create','--file',agent,'--manifest',manifest,'-C',classes,'.'],check=True,capture_output=True,text=True)
    rng=random.Random(0x7671215eed);initial=[0,1,8682522807148012,2**64-1,2**63-1,2**63,2**32-1,2**32,0x9e3779b97f4a7c15]
    nanos=[-2**63,-1,0,1,2**63-1,1_000_000_000,0x123456789abcdef]
    queries=[]
    for index in range(128):
        operations=['unique','create','thread_safe','level','unique']
        if index%2:rng.shuffle(operations)
        times=[nanos[(index+i)%len(nanos)]if index<16 else rng.randrange(-2**63,2**63)for i in range(7)]
        queries.append(dict(initial_uniquifier_bits=f'{initial[index]if index<len(initial)else rng.randrange(2**64):016x}',nano_times=times,operations=operations))
    before=time.clock_gettime_ns(time.CLOCK_MONOTONIC)
    java=subprocess.run([JDK/'java',*flags,'-javaagent:'+str(agent),'-cp',cp,'oracle.random_seed_oracle'],input=''.join(json.dumps(q)+'\n'for q in queries),text=True,capture_output=True)
    after=time.clock_gettime_ns(time.CLOCK_MONOTONIC);(runtime/'oracle.log').write_text(java.stdout+java.stderr)
    if java.returncode:raise RuntimeError((java.stdout+java.stderr)[-7000:])
    origins=[s[7:]for s in java.stdout.splitlines()if s.startswith('SOURCE:')];assert len(origins)==1 and Path(unquote(urlparse(origins[0]).path)).resolve()==jar.resolve()
    assert 'INSTRUMENTATION:1,1,1'in java.stdout
    live=[int(s.removeprefix('LIVE_NANOTIME:'))for s in java.stdout.splitlines()if s.startswith('LIVE_NANOTIME:')];assert len(live)==1 and before<=live[0]<=after
    rows=[json.loads(s[7:])for s in java.stdout.splitlines()if s.startswith('ORACLE:')];assert len(rows)==len(queries)
    data=dict(minecraft='1.21',oracle_inner_sha256=SHA,scope='Actual original RandomSupport.generateUniqueSeed arithmetic and original RandomSource.create/createThreadSafe/RNG classes. Javaagent replaces only generateUniqueSeed System.nanoTime with injected samples and observes returns. Level RNG initializer composes exact original Level bytecode call order; it does not execute full Level constructor.',instrumentation=dict(clock_call_replacements=1,seed_return_observers=1,classes_transformed=1),cases=[dict(query=q,expected=r)for q,r in zip(queries,rows)])
    fixture=ROOT/'fixtures/random-seed-vectors.json';fixture.write_text(json.dumps(data,separators=(',',':'))+'\n')
    if a.check_rust:
        env=os.environ.copy();env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup',CARGO_INCREMENTAL='0')
        native=subprocess.run(['/root/.local/share/mc-rust-toolchain/cargo/bin/cargo','test','--lib','random_seed::tests','--','--nocapture'],cwd=ROOT,env=env,text=True,capture_output=True);(runtime/'native.log').write_text(native.stdout+native.stderr)
        if native.returncode:raise RuntimeError((native.stdout+native.stderr)[-7000:])
        assert int(re.search(r'test result: ok\. (\d+) passed',native.stdout).group(1))==2
        reports=[json.loads(s.removeprefix('RANDOM_SEED_CONFORMANCE:'))for s in native.stdout.splitlines()if s.startswith('RANDOM_SEED_CONFORMANCE:')];assert len(reports)==1;report=reports[0];assert report['ok']and report['mismatches']==0
        report.update(minecraft='1.21',oracle_inner_sha256=SHA,scope=data['scope'],native_tests_passed=2,original_system_nanotime_bracketed_by_linux_monotonic_clock=True,instrumentation=data['instrumentation'],not_verified=['Full shared process bootstrap unique-seed call order or matching volatile unseeded launches','Full Level constructor world side effects','ThreadSafeLegacyRandomSource concurrent CAS behavior and Gaussian caches','Netty ThreadLocalRandom factory','Post-ServerPlayer transitive constructor RNG state'])
        sources=['src/random_seed.rs','src/random.rs','tools/random_seed_tests.rs','tools/random_seed_generate.py','tools/random_seed_oracle.java','tools/rng_startup_level_bytecode.txt','fixtures/random-seed-vectors.json','Cargo.lock']
        report['source_sha256']={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in sources};(ROOT/'evidence/random-seed-conformance.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(dict(ok=True,cases=len(queries),actual_unique_seed_calls=sum(len(r['observed_seeds'])for r in rows),factory_rng_snapshots=sum(sum('random'in v for v in r['operations'])+sum('thread_safe_random'in v for v in r['operations'])for r in rows),live_clock_bracketed=True,file=str(fixture),sha256=hashlib.sha256(fixture.read_bytes()).hexdigest())))
if __name__=='__main__':main()
