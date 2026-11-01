#!/usr/bin/env python3
"""Differential tests call original signed1.21 classes with supplied world-shape callbacks."""
from pathlib import Path
import argparse,collections,hashlib,json,os,random,re,struct,subprocess
ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle';JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin');SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def fb(v):return struct.pack('>f',v).hex()
def db(v):return struct.pack('>d',v).hex()
def box(v):return list(map(db,v))
def main():
    p=argparse.ArgumentParser();p.add_argument('--skip-rust',action='store_true');p.add_argument('--release',action='store_true',help='also check optimized thinLTO profile');a=p.parse_args();jar=ORACLE/'server-inner-1.21.jar';assert hashlib.sha256(jar.read_bytes()).hexdigest()==SHA
    runtime=ROOT/'runtime/player-physics';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True);cp=os.pathsep.join(map(str,[classes,jar,*sorted((ORACLE/'libraries').rglob('*.jar'))]));flags=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED']
    compile=subprocess.run([JDK/'javac',*flags,'-proc:none','-cp',cp,'-d',classes,ROOT/'tools/survival_oracle.java',ROOT/'tools/player_physics_oracle.java'],capture_output=True,text=True)
    if compile.returncode:raise RuntimeError(compile.stdout+compile.stderr)
    r=random.Random(0x767121face);queries=[];edge=[-0.,0.,-1.,1.,1e-310,-1e-310,1e-8,-1e-8,180.,-180.,360.,-360.,2e7,-2e7,3e7,-3e7,float('nan'),float('inf'),-float('inf')]
    for i in range(1600):
        pos=[r.choice(edge)if i%2 else r.uniform(-1e10,1e10)for _ in range(3)];rot=[r.choice(edge)if i%3 else r.uniform(-1e12,1e12)for _ in range(2)]
        queries.append(dict(op='primitive',position=box(pos),rotation=list(map(fb,rot))))
    for pose in range(18):
        for scale in [-0.,0.,.0625,.1,.5,1.,1.25,2.,4.,16.,float('nan'),float('inf')]:queries.append(dict(op='dimensions',pose=pose,scale=fb(scale)))
    for i in range(1000):
        y=r.choice([-1.,-0.,0.,1e-7,.5,1.,64.]);b=[-.3,y,-.3,.3,y+1.8,.3];shapes=[]
        for j in range(r.randrange(1,8)):
            bottom=y+r.choice([-1.,-1e-7,0.,1e-8,.125,.25,.5,.6,1.,2.]);top=bottom+r.choice([0.,1e-8,.125,.25,.5,1.,2.]);shapes.append(box([-.5,bottom,-.5,.5,top,.5]))
        queries.append(dict(op='step_heights',bounds=box(b),shapes=shapes,step=fb(r.choice([0.,.5,.6,1.,2.,float('nan')])),baseline=fb(r.choice([-0.,0.,.125,.25,.5,-.5]))))
    for i in range(64):
        axes=[]
        for axis in range(3):axes.append(sorted(r.sample([-2.,-1.,0.,.125,.25,.5,.75,1.,2.,64.],r.randrange(1,5))))
        queries.append(dict(op='step_heights',bounds=box([-.3,r.choice([0.,.5,1.]),-.3,.3,2.,.3]),shapes=[],custom_empty=[[box(a)for a in axes]],step=fb(r.choice([.5,.6,1.,2.,float('nan')])),baseline=fb(r.choice([-.5,-0.,0.,.25,.5]))))
    for i in range(2400):
        x,z=r.choice([-2.,-1.,-0.,0.,1.,2.,1e6]),r.choice([-2.,-1.,-0.,0.,1.,2.]);y=r.choice([0.,.5,1.,64.]);width=struct.unpack('>f',struct.pack('>f',r.choice([.2,.6,1.,2.])))[0];height=struct.unpack('>f',struct.pack('>f',r.choice([.6,1.5,1.8])))[0];b=[x-width/2,y,z-width/2,x+width/2,y+height,z+width/2]
        blocks=[box([x-4,y-1,z-4,x+4,y,z+4])];entities=[]
        for j in range(r.randrange(0,7)):
            bx=x+r.choice([-2.,-1.,-.5,0.,.5,1.,2.]);bz=z+r.choice([-2.,-1.,-.5,0.,.5,1.,2.]);by=y+r.choice([-1.,0.,.125,.25,.5,.6,1.,1.5,2.]);ob=box([bx,by,bz,bx+r.choice([.125,.5,1.,2.]),by+r.choice([.125,.25,.5,1.,2.]),bz+r.choice([.125,.5,1.,2.])]);(entities if j%3==0 else blocks).append(ob)
        border=box([x+1,y-2,z-5,x+2,y+5,z+5])if i%7==0 else None
        queries.append(dict(op='collide',bounds=box(b),position=box([x,y,z]),blocks=blocks,entities=entities,border=border,step=fb(r.choice([0.,.5,.6,1.,2.])),ground=bool(i%2),movement=box([r.uniform(-2,2),r.choice([0.,-1.,-.1,-1e-8,.1,.5,1.]),r.uniform(-2,2)])))
    for i in range(1400):
        pos=[r.choice([-1e6,-1.,-0.,0.,1.,1e6]),r.choice([0.,.5,1.,64.]),r.choice([-1.,-0.,0.,1.])];scale=r.choice([.0625,.5,1.,2.,4.]);pose=r.randrange(18);x,y,z=pos;near=r.choice([.2,.6,1.5,1.8])*scale+r.choice([-1e-6,-1e-7,0.,1e-7,1e-6]);ob=box([x-2,y+near,z-2,x+2,y+near+1,z+2]);blocks=[ob]if i%3==0 else [];entities=[ob]if i%3==1 else [];border=ob if i%3==2 else None
        queries.append(dict(op='pose_fits',position=box(pos),scale=fb(scale),pose=pose,blocks=blocks,entities=entities,border=border))
    for i in range(1200):
        x,z=r.choice([-2.,-1.,0.,1.,2.,1e6]),r.choice([-2.,-1.,0.,1.,2.]);y=r.choice([0.,1.,64.]);old=box([x-.3,y,z-.3,x+.3,y+1.8,z+.3]);p=[x+r.uniform(-.3,.3),y+r.uniform(-.3,.3),z+r.uniform(-.3,.3)];b=box([p[0]-.3,p[1],p[2]-.3,p[0]+.3,p[1]+1.8,p[2]+.3]);target=[x+r.uniform(-2.,2.),y+r.uniform(-2.,2.),z+r.uniform(-2.,2.)];blocks=[];entities=[]
        for j in range(r.randrange(0,6)):
            bx=x+r.uniform(-2,2);by=y+r.uniform(-1,2);bz=z+r.uniform(-2,2);ob=box([bx,by,bz,bx+1,by+1,bz+1]);(blocks if j%2 else entities).append(ob)
        queries.append(dict(op='new_collision',position=box(p),bounds=b,old_bounds=old,target=box(target),blocks=blocks,entities=entities,border=None))
    for i in range(500):
        x,y,z=[r.uniform(-1e5,1e5)for _ in range(3)];queries.append(dict(op='floating_bounds',bounds=box([x,y,z,x+.6,y+1.8,z+.6]),air=bool(i%2)))
    transcript=''.join(json.dumps(q,separators=(',',':'))+'\n'for q in queries);java=subprocess.run([JDK/'java',*flags,'-Xmx1G','-cp',cp,'oracle.player_physics_oracle'],input=transcript,text=True,capture_output=True);(runtime/'oracle.log').write_text(java.stdout+java.stderr)
    if java.returncode:raise RuntimeError((java.stdout+java.stderr)[-5000:])
    output=[json.loads(line.split('ORACLE:',1)[1])for line in java.stdout.splitlines()if line.startswith('ORACLE:')];assert len(output)==len(queries),(len(output),len(queries))
    vectors=ROOT/'tools/player_physics_vectors.json';vectors.write_text(json.dumps(dict(minecraft='1.21',oracle_inner_sha256=SHA,cases=[dict(query=q,expected=o)for q,o in zip(queries,output)]),separators=(',',':'))+'\n')
    report=dict(minecraft='1.21',oracle_inner_sha256=SHA,queries=len(queries),operations=dict(collections.Counter(q['op']for q in queries)),vectors_sha256=hashlib.sha256(vectors.read_bytes()).hexdigest(),native_source_sha256=hashlib.sha256((ROOT/'src/player_physics.rs').read_bytes()).hexdigest(),geometry_source_sha256=hashlib.sha256((ROOT/'src/geometry.rs').read_bytes()).hexdigest(),oracle_source_sha256=hashlib.sha256((ROOT/'tools/player_physics_oracle.java').read_bytes()).hexdigest(),scope='Original ServerGamePacketListenerImpl numeric validation/clamps, LivingEntity player pose dimensions and Player pose-fit query, Entity ascending step-height collision search, listener new-collision rollback query and floating-air query bounds. Shape lists and border activation supplied by controlled oracle-only subclasses; original methods and all Minecraft classes unchanged.',not_verified=['Full running-world collision provider and block/entity/border query availability','handleMovePlayer complete callbacks/state machine, speed/fall/damage and server integration','Dynamic moving-piston/shulker shape production and fluid effects'])
    if not a.skip_rust:
        env=os.environ.copy();env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup',CARGO_INCREMENTAL='0');cargo='/root/.local/share/mc-rust-toolchain/cargo/bin/cargo';rust=subprocess.run([cargo,'test','--lib','player_physics::tests','--','--nocapture'],cwd=ROOT,env=env,text=True,capture_output=True);(runtime/'rust.log').write_text(rust.stdout+rust.stderr)
        if rust.returncode:raise RuntimeError((rust.stdout+rust.stderr)[-7000:])
        report.update(mismatches=0,rust_tests_passed=int(re.search(r'test result: ok\. (\d+) passed',rust.stdout).group(1)))
        if a.release:
            optimized=subprocess.run([cargo,'test','--release','--lib','player_physics::tests','--','--nocapture'],cwd=ROOT,env=env,text=True,capture_output=True);(runtime/'rust-release.log').write_text(optimized.stdout+optimized.stderr)
            if optimized.returncode:raise RuntimeError((optimized.stdout+optimized.stderr)[-7000:])
            report.update(optimized_mismatches=0,optimized_rust_tests_passed=int(re.search(r'test result: ok\. (\d+) passed',optimized.stdout).group(1)))
    (ROOT/'evidence/player-physics-conformance.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
