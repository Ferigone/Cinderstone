#!/usr/bin/env python3
"""Observe real stock ServerPlayer constructors through offline protocol clients."""
from pathlib import Path
import argparse,collections,hashlib,json,os,re,shutil,subprocess,tempfile,time
ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle';JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01';OUTER_SHA1='450698d1863ab5180c25d7c804ef0fe6369dd1ba'
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def wait_log(process,path,marker,timeout=120):
    deadline=time.monotonic()+timeout
    while marker not in path.read_text(errors='replace'):
        if process.poll()is not None:raise RuntimeError('stock server exited: '+path.read_text(errors='replace')[-7000:])
        if time.monotonic()>deadline:raise TimeoutError('stock server wait '+marker+': '+path.read_text(errors='replace')[-7000:])
        time.sleep(.1)
def main():
    parser=argparse.ArgumentParser();parser.add_argument('--check-rust',action='store_true');args=parser.parse_args()
    assert digest(ORACLE/'server-inner-1.21.jar')==SHA
    assert hashlib.sha1((ORACLE/'server-1.21.jar').read_bytes()).hexdigest()==OUTER_SHA1
    source_files=[ORACLE/'world/level.dat',*sorted((ORACLE/'world').rglob('*.mca'))];before={str(p.relative_to(ORACLE/'world')):digest(p)for p in source_files}
    runtime=ROOT/'runtime/player-rng-constructor';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True)
    flags=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED']
    sources=[ROOT/'tools/player_rng_constructor_agent.java',ROOT/'tools/player_rng_constructor_observer.java']
    compile=subprocess.run([JDK/'javac',*flags,'-proc:none','-d',classes,*sources],text=True,capture_output=True)
    if compile.returncode:raise RuntimeError(compile.stdout+compile.stderr)
    manifest=runtime/'manifest.mf';manifest.write_text('Manifest-Version: 1.0\nPremain-Class: oracle.player_rng_constructor_agent\n\n')
    agent=runtime/'constructor-agent.jar';observer=runtime/'constructor-observer.jar'
    command=[JDK/'jar','--create','--file',agent,'--manifest',manifest]
    for path in sorted((classes/'oracle').glob('player_rng_constructor_agent*.class')):command +=['-C',classes,str(path.relative_to(classes))]
    subprocess.run(command,check=True,capture_output=True,text=True)
    subprocess.run([JDK/'jar','--create','--file',observer,'-C',classes,'oracle/player_rng_constructor_observer.class'],check=True,capture_output=True,text=True)
    trace=runtime/'trace.jsonl';trace.unlink(missing_ok=True);log_path=runtime/'server.log';clients=[];port=25582
    with tempfile.TemporaryDirectory(prefix='world-',dir=runtime)as temporary:
        work=Path(temporary);shutil.copytree(ORACLE/'world',work/'world',copy_function=shutil.copyfile)
        (work/'eula.txt').write_text('eula=true\n')
        properties={'server-ip':'127.0.0.1','server-port':str(port),'online-mode':'false','enforce-secure-profile':'false','max-players':'4','gamemode':'creative','force-gamemode':'true','view-distance':'2','simulation-distance':'2','spawn-protection':'0','generate-structures':'false','spawn-animals':'false','spawn-npcs':'false','spawn-monsters':'false','level-type':'minecraft:flat','max-tick-time':'60000'}
        (work/'server.properties').write_text(''.join(k+'='+v+'\n'for k,v in properties.items()))
        with log_path.open('w')as log:
            process=subprocess.Popen([JDK/'java',*flags,'-Dmc121.player_rng_trace='+str(trace),'-javaagent:'+str(agent)+'='+str(observer),'-Xms128M','-Xmx768M','-jar',ORACLE/'server-1.21.jar','nogui'],cwd=work,stdin=subprocess.PIPE,stdout=log,stderr=subprocess.STDOUT,text=True)
            try:
                wait_log(process,log_path,'Done (')
                for mode,radius,prefix in [('creative',0,'CtorRngC'),('survival',10,'CtorRngS'),('adventure',0,'CtorRngA')]:
                    marker='CTOR_MODE_'+mode.upper()+'_READY';commands=f'gamerule doMobSpawning false\ngamerule randomTickSpeed 0\ngamerule doDaylightCycle false\ngamerule doWeatherCycle false\ndefaultgamemode {mode}\ngamerule spawnRadius {radius}\nsay {marker}\n';process.stdin.write(commands);process.stdin.flush();wait_log(process,log_path,marker,30)
                    names=[prefix+str(i)for i in range(3)]
                    run=subprocess.run(['node','tools/player_rng_constructor_probe.mjs',str(port),*names],cwd=ROOT,text=True,capture_output=True,timeout=100)
                    (runtime/(mode+'-client.log')).write_text(run.stdout+run.stderr)
                    if run.returncode:raise RuntimeError(run.stdout+run.stderr)
                    for line in run.stdout.splitlines():clients.append(dict(mode=mode,radius=radius,**json.loads(line)))
                inner=work/'versions/1.21/server-1.21.jar';assert inner.exists()and digest(inner)==SHA
            finally:
                if process.poll()is None:
                    try:process.stdin.write('stop\n');process.stdin.flush();process.wait(timeout=40)
                    except (BrokenPipeError,subprocess.TimeoutExpired):process.kill();process.wait(timeout=10)
    assert {str(p.relative_to(ORACLE/'world')):digest(p)for p in source_files}==before,'reference oracle world changed'
    rows=[json.loads(line)for line in trace.read_text().splitlines()];assert not any('error'in row for row in rows),rows
    groups=collections.defaultdict(list)
    for row in rows:groups[row['entity_identity']].append(row)
    assert len(groups)==len(clients)==9,(len(groups),len(clients))
    cases=[]
    for group in groups.values():
        assert [row['stage']for row in group]==['entity_rng_initialized','entity_exit','server_player_exit']
        initial,entity,player=group;assert all(r['random_class']=='dyz'for r in group)
        assert len({r['random_identity']for r in group})==1
        assert entity['state48']==player['state48'],'inherited RNG changed after Entity constructor'
        assert initial['uuid']is None and entity['uuid']is not None and player['uuid']is not None
        client=next(row for row in clients if row['uuid']==player['uuid'])
        cases.append(dict(client=client,observations=group))
    data=dict(minecraft='1.21',oracle_inner_sha256=SHA,scope='Real unchanged stock server and plain ServerPlayer through three protocol client modes; javaagent only reads RNG state/identity at initialization, Entity-return and ServerPlayer-return. No RNG methods, seeds, arithmetic or environment methods replaced.',cases=cases)
    fixture=ROOT/'fixtures/player-rng-constructor-vectors.json';fixture.write_text(json.dumps(data,separators=(',',':'))+'\n')
    source_paths=[*sources,ROOT/'tools/player_rng_constructor_check.py',ROOT/'tools/player_rng_constructor_probe.mjs',fixture]
    report=dict(ok=True,kind='live original server constructor observer',minecraft='1.21',oracle_inner_sha256=SHA,outer_sha1=OUTER_SHA1,protocol767_clients=len(clients),observations=len(rows),inherited_rng_unchanged_between_entity_and_server_player=9,random_object_identity_unchanged=9,reference_world_unchanged=True,reference_world_source_sha256=before,native_comparison=False,not_verified=['Native initialization helper until captured-state replay','Subsequent login/gameplay RNG draws after ServerPlayer constructor','Global Math.random values and separate fresh spawn-location RNG'],source_sha256={str(p.relative_to(ROOT)):digest(p)for p in source_paths})
    (ROOT/'evidence/player-rng-constructor-observer.json').write_text(json.dumps(report,indent=2)+'\n')
    if args.check_rust:
        env=os.environ.copy();env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup',CARGO_INCREMENTAL='0')
        native=subprocess.run(['/root/.local/share/mc-rust-toolchain/cargo/bin/cargo','test','--lib','random_seed::player_constructor_tests','--','--nocapture'],cwd=ROOT,env=env,text=True,capture_output=True);(runtime/'native.log').write_text(native.stdout+native.stderr)
        if native.returncode:raise RuntimeError((native.stdout+native.stderr)[-7000:])
        assert int(re.search(r'test result: ok\. (\d+) passed',native.stdout).group(1))==1
        results=[json.loads(s.removeprefix('PLAYER_CONSTRUCTOR_RANDOM_CONFORMANCE:'))for s in native.stdout.splitlines()if s.startswith('PLAYER_CONSTRUCTOR_RANDOM_CONFORMANCE:')];assert len(results)==1;result=results[0];assert result['ok']and result['mismatches']==0
        result.update(minecraft='1.21',oracle_inner_sha256=SHA,native_tests_passed=1,kind='live stock constructor state differential conformance',not_verified=['Subsequent login/gameplay RNG draws after constructor','Global Math.random and separate spawn-location random sources','Full global unique-seed bootstrap call order'])
        paths=[*source_paths,ROOT/'src/random_seed.rs',ROOT/'src/random.rs',ROOT/'tools/player_rng_constructor_tests.rs'];result['source_sha256']={str(p.relative_to(ROOT)):digest(p)for p in paths};(ROOT/'evidence/player-rng-constructor-conformance.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps({k:v for k,v in report.items()if k not in ['source_sha256','reference_world_source_sha256','not_verified']}))
if __name__=='__main__':main()
