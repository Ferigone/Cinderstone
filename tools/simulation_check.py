#!/usr/bin/env python3
"""Observe original signed Minecraft1.21 tick phases with injected callbacks."""
from pathlib import Path
import argparse,hashlib,json,os,random,re,shutil,struct,subprocess
ROOT=Path(__file__).resolve().parents[1];O=ROOT.parent/'mc121-oracle';JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
def bits(value):return struct.pack('>f',value).hex()
def main():
    p=argparse.ArgumentParser();p.add_argument('--skip-rust',action='store_true');a=p.parse_args();jar=O/'server-inner-1.21.jar';sha=hashlib.sha256(jar.read_bytes()).hexdigest();assert sha=='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
    runtime=ROOT/'runtime/simulation';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True);cp=os.pathsep.join(map(str,[classes,jar,*sorted((O/'libraries').rglob('*.jar'))]));flags=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED'];subprocess.run([JDK/'javac',*flags,'-proc:none','-cp',cp,'-d',classes,ROOT/'tools/survival_oracle.java',ROOT/'tools/simulation_oracle.java'],check=True,capture_output=True)
    rng=random.Random(0x121767710);queries=[];gates=['run_game_elements','debug_world','tick_time','daylight_cycle','enough_sleeping','enough_deep_sleeping','weather_cycle','raining','has_players','has_forced_chunks','has_dragon_fight'];times=[0,-1,1,23999,24000,-24001,9223372036854775807,-9223372036854775808];empty=[-2147483648,-1,0,298,299,300,301,2147483647]
    for mask in range(1<<len(gates)):
        q=dict(op='level',game_time=rng.choice(times),day_time=rng.choice(times),empty_time=rng.choice(empty));q.update({gate:bool(mask&(1<<bit))for bit,gate in enumerate(gates)});queries.append(q)
    for i in range(32):
        q=dict(queries[-1]);q.update(daylight_cycle=bool(i&1),daylight_after_functions=bool(i&2),game_time_after_functions=rng.choice(times),day_time_after_functions=rng.choice(times));queries.append(q)
    for i in range(500):queries.append(dict(op='counter',tick_count=rng.choice([0,1,18,19,20,5998,5999,1000000]),until_save=rng.choice([-2147483648,-1,0,1,2,100,6000,2147483647]),rate_bits=bits(rng.choice([0.,1.,10.,20.,10000.,float('nan'),float('inf')])),sprinting=bool(i&1),average_nanos=rng.choice([-1,0,1,1000,1000000,50000000,1000000000])))
    for i in range(500):queries.append(dict(op='sleep',active=rng.choice([-2147483648,-1,0,1,2,5,100,2147483647]),sleeping=rng.choice([-1,0,1,2,5,100,2147483647]),percentage=rng.choice([-2147483648,-1,0,1,50,100,101,1000,2147483647])))
    for i in range(128):
        q=dict(queries[i*16]);q.update(op='children',tick_count=rng.choice([0,1,19,20,39,40,6000]),until_save=6000,rate_bits=bits(20.),sprinting=False,average_nanos=50000000);queries.append(q)
    for i in range(200):queries.append(dict(op='sleep_update',active=rng.randrange(20),sleeping=rng.randrange(20),percentage=rng.choice([-1,0,1,50,100,101,1000]),remove=bool(i&1),players=[[rng.choice([True,False])for _ in range(3)]for _ in range(rng.randrange(9))]))
    for i in range(100):
        steps=[['freeze',True],['tick'],['step',2],['tick'],['tick'],['tick'],['request',3],['check'],['tick'],['check'],['work'],['check'],['work'],['check'],['work'],['spent',rng.choice([0,1,1000,50000000,-1,9223372036854775807])],['check'],['tick']]
        for _ in range(30):steps.append(rng.choice([['freeze',rng.choice([True,False])],['step',rng.choice([-1,0,1,2,100])],['stopstep'],['request',rng.choice([-1,0,1,2,100])],['check'],['tick'],['work'],['stop'],['rate',bits(rng.choice([1.,10.,20.,1000.,float('nan')]))]]))
        queries.append(dict(op='rate',tick_count=0,until_save=6000,rate_bits=bits(20.),sprinting=False,average_nanos=50000000,steps=steps))
    for raw in [0,0x80000000,0x3f800000,0x41a00000,0x7f800000,0xff800000,0x7fc00000,0x7fa12345,0xffc12345]:
        for steps in [-2147483648,-1,0,1,20,2147483647]:queries.append(dict(op='rate_packets',rate_bits=f'{raw:08x}',frozen=bool(steps&1),steps=steps))
    java=subprocess.run([JDK/'java',*flags,'-Xmx1G','-cp',cp,'oracle.simulation_oracle'],input=''.join(json.dumps(q,separators=(',',':'))+'\n'for q in queries),text=True,capture_output=True);(runtime/'oracle.log').write_text(java.stdout+java.stderr)
    if java.returncode:raise RuntimeError((java.stdout+java.stderr)[-3500:])
    outputs=[json.loads(line[7:])for line in java.stdout.splitlines()if line.startswith('ORACLE:')];assert len(outputs)==len(queries),(len(outputs),len(queries));vectors=dict(minecraft='1.21',oracle_inner_sha256=sha,cases=[dict(query=q,expected=value)for q,value in zip(queries,outputs)]);vp=ROOT/'tools/simulation_vectors.json';vp.write_text(json.dumps(vectors,separators=(',',':'))+'\n')
    result=dict(minecraft='1.21',oracle_inner_sha256=sha,queries=len(queries),ticking_packet_vectors=sum(q['op']=='rate_packets'for q in queries),packet_registry_sha256=hashlib.sha256((O/'reports/reports/packets.json').read_bytes()).hexdigest(),sprint_lifecycle_actions=sum(len(q['steps'])for q in queries if q['op']=='rate'),sprint_clock_replay='Actual System.nanoTime readings are observed and injected into Rust for deterministic replay; localized report formatting is not compared.',level_gate_combinations=1<<len(gates),scope='OriginalServerLevel.tick/tickTime, MinecraftServer.tickServer/tickChildren/computeNextAutosaveInterval, SleepStatus and ServerTickRateManager sprint/freeze/step/rate lifecycle. Injectedcallbacksverifyphaseorder/gates; fullchunk/entity/raid/blockmechanicsnotclaimed.',observed_phase_callbacks=sum(len(v.get('phases',[]))for v in outputs if isinstance(v,dict)),vectors_sha256=hashlib.sha256(vp.read_bytes()).hexdigest(),native_source_sha256=hashlib.sha256((ROOT/'src/simulation.rs').read_bytes()).hexdigest())
    if not a.skip_rust:
        env=os.environ.copy();env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup');cargo=shutil.which('cargo')or'/root/.local/share/mc-rust-toolchain/cargo/bin/cargo';rust=subprocess.run([cargo,'test','--lib','simulation::tests','--','--nocapture'],cwd=ROOT,env=env,text=True,capture_output=True);(runtime/'rust.log').write_text(rust.stdout+rust.stderr)
        if rust.returncode:raise RuntimeError((rust.stdout+rust.stderr)[-6000:])
        result.update(mismatches=0,rust_tests_passed=int(re.search(r'test result: ok\. (\d+) passed',rust.stdout).group(1)))
    (ROOT/'tools/simulation_conformance.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
if __name__=='__main__':main()
