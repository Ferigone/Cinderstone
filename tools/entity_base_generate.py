#!/usr/bin/env python3
"""Execute unchanged original Entity.baseTick with explicit lower world hooks."""
from pathlib import Path
import argparse,collections,hashlib,itertools,json,os,random,re,struct,subprocess

ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle'
JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
FLAGS=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED']
def fb(v):return struct.pack('>f',v).hex()
def db(v):return struct.pack('>d',v).hex()
def initial():
    return dict(op="base",pre_dirty=0,pose=0,removed=False,spectator=False,boarding_cooldown=0,fire_ticks=0,tick_count=0,portal_cooldown=0,walk_dist_o=fb(-1),walk_dist=fb(2),yaw_o=fb(-2),pitch_o=fb(3),fall_distance=fb(7),yaw=fb(34),pitch=fb(-56),in_powder_snow=False,was_in_powder_snow=True,first_tick=False,touching_water=False,was_eye_in_water=False,visual_fire=False,client=False,fire_immune=False,sprint_particles=False,portal_active=False,server_level=True,leashable=False,vehicle_present=False,vehicle_removed=False,boat=False,boat_underwater=False,hurt_result=True,silent=False,ultrawarm=False,y=db(64),eye_height=fb(.2125),min_y=-64,water_depth=db(0),lava_depth=db(0),eye_fluid=0,eye_tags=[],flags=0,frozen_ticks=0,boat_min_y=db(63),boat_max_y=db(65),seed=12345)

def main():
    p=argparse.ArgumentParser();p.add_argument('--smoke',action='store_true');p.add_argument('--check-rust',action='store_true');a=p.parse_args()
    jar=ORACLE/'server-inner-1.21.jar';assert hashlib.sha256(jar.read_bytes()).hexdigest()==SHA
    runtime=ROOT/'runtime/entity-base';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True)
    cp=os.pathsep.join(map(str,[classes,jar,*sorted((ORACLE/'libraries').rglob('*.jar'))]))
    compile=subprocess.run([JDK/'javac',*FLAGS,'-proc:none','-cp',cp,'-d',classes,ROOT/'tools/survival_oracle.java',ROOT/'tools/entity_base_oracle.java'],capture_output=True,text=True)
    if compile.returncode:raise RuntimeError(compile.stdout+compile.stderr)
    queries=[]
    fire=[-2147483648,-1,0,1,2,3,4,19,20,21,39,40,299,300,301,2147483647]
    for client,immune,first,water,lava,frozen,ticks in itertools.product([False,True],[False,True],[False,True],[0.,.5],[0.,.5],[-1,0,1,140],fire):
        q=initial();q.update(client=client,fire_immune=immune,first_tick=first,water_depth=db(water),lava_depth=db(lava),frozen_ticks=frozen,fire_ticks=ticks,flags=(ticks^frozen)&255);queries.append(q)
    r=random.Random(0x767121ba5e)
    for i in range(2500):
        q=initial();q.update(boarding_cooldown=r.choice([-2147483648,-1,0,1,2,2147483647]),fire_ticks=r.choice(fire),tick_count=r.choice([-2147483648,-1,0,1,2147483647]),portal_cooldown=r.randrange(-3,301),walk_dist=fb(r.choice([-0.,0.,.1,1.,100.,1e30])),walk_dist_o=fb(r.uniform(-100,100)),yaw=fb(r.uniform(-1e6,1e6)),pitch=fb(r.uniform(-1e6,1e6)),yaw_o=fb(r.uniform(-100,100)),pitch_o=fb(r.uniform(-100,100)),fall_distance=fb(r.choice([-0.,0.,-1.,1.,100.,1e30])),y=db(r.choice([-129.,-128.00000000000003,-128.,-127.99999999999999,-64.,0.,63.,64.,64.5,320.])),eye_height=fb(r.choice([0.,.2125,1.62,2.])),min_y=r.choice([-2032,-64,0,2032]),water_depth=db(r.choice([0.,.0001,.1,.5,1.])),lava_depth=db(r.choice([0.,.0001,.1,.5,1.])),eye_fluid=r.choice([0,80,81,96,97]),eye_tags=r.choice([[],['water'],['lava'],['water','lava']]),flags=r.randrange(256),frozen_ticks=r.choice([-2147483648,-1,0,1,140,2147483647]),seed=r.randrange(-2**63,2**63))
        q['pre_dirty']=r.choice([0,1,128,129])
        for name in ['client','fire_immune','first_tick','touching_water','was_eye_in_water','in_powder_snow','was_in_powder_snow','visual_fire','sprint_particles','portal_active','server_level','leashable','vehicle_present','vehicle_removed','boat','boat_underwater','hurt_result','silent','ultrawarm']:q[name]=bool(r.randrange(2))
        queries.append(q)
    # Vehicle shielding and eye sample boundaries; state IDs are extracted from
    # the exact original block registry, and will be fixed below before launch.
    for boat,submerged,removed,first,touch,fluid in itertools.product([False,True],[False,True],[False,True],[False,True],[False,True],[0,80,96]):
        q=initial();q.update(vehicle_present=True,boat=boat,boat_underwater=submerged,vehicle_removed=removed,first_tick=first,touching_water=touch,water_depth=db(.5),lava_depth=db(.5),eye_fluid=fluid,eye_tags=['water'],fire_ticks=20,frozen_ticks=1);queries.append(q)
    # Independently invoke original default br and bl via invokespecial, not
    # the controlled baseTick virtual overrides, to verify the host defaults.
    for sprint,touch,spectator,pose,lava,first,removed in itertools.product([False,True],[False,True],[False,True],range(18),[0.,.5],[False,True],[False,True]):
        q=initial();q.update(op='defaults',flags=8 if sprint else 0,touching_water=touch,spectator=spectator,pose=pose,lava_depth=db(lava),first_tick=first,removed=removed);queries.append(q)
    for flags,touch,eye,vehicle,fluid in itertools.product([0,8,16,24],[False,True],[False,True],[False,True],[0,80,96]):
        q=initial();q.update(op='defaults',flags=flags,touching_water=touch,was_eye_in_water=eye,vehicle_present=vehicle,eye_fluid=fluid);queries.append(q)
    for i in range(400):
        q=initial();q.update(pre_dirty=r.choice([0,1,128,129]),op='defaults',flags=r.randrange(256),touching_water=bool(r.randrange(2)),was_eye_in_water=bool(r.randrange(2)),vehicle_present=bool(r.randrange(2)),pose=r.randrange(18),spectator=bool(r.randrange(2)),removed=bool(r.randrange(2)),first_tick=bool(r.randrange(2)),lava_depth=db(r.choice([0.,.5])),eye_fluid=r.choice([0,80,81,96]));queries.append(q)
    if a.smoke:queries=[initial(),dict(initial(),fire_ticks=20,frozen_ticks=1),dict(initial(),lava_depth=db(.5)),dict(initial(),water_depth=db(.5),eye_fluid=80),dict(initial(),eye_fluid=96),dict(initial(),leashable=True),dict(initial(),boat=True,vehicle_present=True,water_depth=db(.5))]
    transcript=''.join(json.dumps(q,separators=(',',':'))+'\n'for q in queries)
    java=subprocess.run([JDK/'java',*FLAGS,'-Xmx1G','-cp',cp,'oracle.entity_base_oracle'],input=transcript,text=True,capture_output=True)
    (runtime/'oracle.log').write_text(java.stdout+java.stderr)
    if java.returncode:raise RuntimeError((java.stdout+java.stderr)[-7000:])
    outputs=[json.loads(line.removeprefix('ORACLE:'))for line in java.stdout.splitlines()if line.startswith('ORACLE:')]
    assert len(outputs)==len(queries),(len(outputs),len(queries))
    if a.smoke:print(json.dumps(outputs,indent=2));return
    result=dict(minecraft='1.21',oracle_inner_sha256=SHA,cases=[dict(query=q,expected=o)for q,o in zip(queries,outputs)],scope='Original Entity.baseTick, water/lava wrapper, water entry/fire clearing/firstTick, private fluid-on-eyes, lavaHurt, fire/frozen/sharedflags plus independent original default sprint-particle and swimming decisions. Oracle-only entity/world callbacks control lower fluid results, particles, portal/swimming hook, hurt/sound/vehicle detach/void and empty leash data. No Minecraft methods rewritten. Full real-world flow, portals, swimming decisions, health/removal side effects and leash physics are not proved.',oracle_source_sha256=hashlib.sha256((ROOT/'tools/entity_base_oracle.java').read_bytes()).hexdigest())
    path=ROOT/'fixtures/entity-base-vectors.json';path.write_text(json.dumps(result,separators=(',',':'))+'\n')
    if a.check_rust:
        env=os.environ.copy();env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup',CARGO_INCREMENTAL='0')
        cargo='/root/.local/share/mc-rust-toolchain/cargo/bin/cargo'
        native=subprocess.run([cargo,'test','--lib','entity_base::tests','--','--nocapture'],cwd=ROOT,env=env,text=True,capture_output=True)
        (runtime/'rust.log').write_text(native.stdout+native.stderr)
        if native.returncode:raise RuntimeError((native.stdout+native.stderr)[-7000:])
        passed=int(re.search(r'test result: ok\. (\d+) passed',native.stdout).group(1));assert passed==2
        counts=collections.Counter(q['op']for q in queries)
        source_paths=['src/entity_base.rs','src/entity_state.rs','tools/entity_base_oracle.java','tools/entity_base_tests.rs','tools/entity_base_generate.py','fixtures/entity-base-vectors.json']
        report=dict(ok=True,kind='controlled-world differential conformance',minecraft='1.21',oracle_inner_sha256=SHA,queries=len(queries),original_base_tick_calls=counts['base'],original_default_sprint_calls=counts['defaults'],original_default_swimming_calls=counts['defaults'],ordered_callbacks=sum(len(out.get('events',[]))for out in outputs),metadata_dirty_snapshots=len(queries),state_and_rng_snapshots=counts['base'],native_tests_passed=passed,mismatches=0,scope=result['scope'],not_verified=['Full world fluid flow and fluid query availability','Portal transitions, particles, swimming host override, damage health effects and sound delivery','Nonempty leash state and actual vehicle-detach/removal side effects','Complete running-server entity tick and network phase integration'],source_sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in source_paths})
        (ROOT/'evidence/entity-base-conformance.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(dict(file=str(path),queries=len(queries),operations=dict(collections.Counter(q['op']for q in queries)),events=dict(collections.Counter(e for out in outputs for e in out.get('events',[]))),bytes=path.stat().st_size,sha256=hashlib.sha256(path.read_bytes()).hexdigest())))

if __name__=='__main__':main()
