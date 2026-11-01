#!/usr/bin/env python3
"""Compare Rust geometry against actual classes from the pinned 1.21 JAR."""
import argparse,hashlib,json,os,random,struct,subprocess,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
JAR_SHA256='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def bits(n):return struct.pack('>d',n).hex()
def fbits(n):return struct.pack('>f',n).hex()
def main():
    p=argparse.ArgumentParser();p.add_argument('--oracle-dir',type=Path,default=ROOT.parent/'mc121-oracle');p.add_argument('--rounds',type=int,default=2000);p.add_argument('--java-home',type=Path,default=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1'));a=p.parse_args()
    jar=a.oracle_dir/'server-inner-1.21.jar'
    assert hashlib.sha256(jar.read_bytes()).hexdigest()==JAR_SHA256
    r=random.Random(0x121767abc);commands=[]
    def cmd(kind,*numbers):commands.append(kind+'\t'+'\t'.join(bits(n)for n in numbers))
    edge=[-0.0,0.0,1e-8,-1e-8,1e-7,-1e-7,1e-4,-1e-4,1.0,-1.0,0.5,-0.5,1.00000005,-0.00000005]
    def n():return r.choice(edge)if r.random()<.4 else r.uniform(-8,8)
    for i in range(a.rounds):
        cmd('vec',*[n()for _ in range(7)])
        commands.append('look\t'+fbits(r.uniform(-180,180))+'\t'+fbits(r.uniform(-36000,36000)))
        cmd('box',*[n()for _ in range(15)])
        if i%2==0:a_box=[0.,0.,0.,1.,1.,1.];b_box=[1.+r.choice([-1.,0.,1.])*10**r.randrange(-9,-5),0.,0.,2.,1.,1.]
        else:a_box=[n()for _ in range(6)];b_box=[n()for _ in range(6)]
        cmd('obstruct',*a_box,*b_box)
        commands.append('reach\t'+'\t'.join([*[bits(n())for _ in range(3)],fbits(r.choice([1.62,1.27,.4])),*[str(r.randrange(-8,9))for _ in range(3)],bits(r.choice([4.5,5.0,0.,64.])),bits(r.choice([0.,1.]))]))
        if i%2:box=[0.,0.,0.,1.,1.,1.]
        else:box=[n()for _ in range(6)]
        cmd('clip',*box,*[n()for _ in range(6)])
        start=[n()for _ in range(3)];end=[n()for _ in range(3)]
        if i%5==0:end=start.copy()
        if i%7==0:end=[v+1 for v in start]
        cmd('dda',*start,*end)
        commands.append('player\t'+'\t'.join([*[bits(n())for _ in range(3)],fbits(r.choice([0.6,0.2,1.0,3.4])),fbits(r.choice([1.8,1.5,0.6,2.9]))]))
        coords=[]
        for _ in range(3):coords.append(sorted(r.sample([-1.,-.5,0.,.0625,.125,.5,.75,1.,1.25,1.5,2.],r.randrange(2,6))))
        count=(len(coords[0])-1)*(len(coords[1])-1)*(len(coords[2])-1)
        occupancy=''.join(r.choice('01')for _ in range(count))
        shape_prefix='\t'+str(i%3)+'\t'+'\t'.join(','.join(map(bits,co))for co in coords)+'\t'+occupancy+'\t'
        collide=shape_prefix+'\t'.join(bits(n())for _ in range(7))
        commands.extend(['collide'+collide,'boxes'+collide,'motion'+collide+'\t'+'\t'.join(bits(n())for _ in range(3))])
        commands.append('shapeclip'+shape_prefix+'\t'.join([*[bits(n())for _ in range(6)],*[str(r.randrange(-2,3))for _ in range(3)]]))
    commands.extend('static\t'+str(state)for state in range(26684))
    catalog=json.loads((ROOT/'evidence/geometry-collision-catalog.json').read_text())
    for row in catalog['context_dependent']:
        state=int(row.split(':',1)[0])
        for i in range(16):
            x,z=r.choice([-30000000,-1,0,1,30000000]),r.choice([-30000000,-1,0,1,30000000]);y=r.randrange(-64,320)
            commands.append('dynamic\t'+'\t'.join(map(str,[state,x,y,z,str(i==0).lower(),bits(y+r.choice([-1.,0.,1e-6,.5,1.-1e-6,1.,2.])),str(i%3==0).lower(),fbits(r.choice([0.,2.5,2.50001,4.])),str(i%5==0).lower(),str(i%2==0).lower()])))

    for v in [-0.,0.,float('nan'),float('inf'),-float('inf')]:
        cmd('vec',v,0.,0.,1.,2.,3.,.5)
        cmd('box',v,0.,0.,1.,1.,1.,0.,0.,0.,2.,2.,2.,0.,0.,0.)
    for edge_from,edge_to in [([0.,0.,0.],[1.,1.,1.]),([1.,1.,1.],[0.,0.,0.]),([0.,0.,0.],[-0.,0.,0.]),([-1.,-1.,-1.],[1.,1.,1.])]:cmd('dda',*edge_from,*edge_to)
    transcript='\n'.join(commands)+'\n';rustc=Path('/root/.local/share/mc-rust-toolchain/rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc')
    with tempfile.TemporaryDirectory(prefix='mc121-geometry-')as tmp:
        subprocess.run([str(rustc),'--edition=2021','-O',str(ROOT/'tools/geometry_probe.rs'),'-o',tmp+'/probe'],check=True)
        subprocess.run([str(a.java_home/'bin/javac'),'-d',tmp,str(ROOT/'tools/geometry_Oracle.java')],check=True)
        classpath=os.pathsep.join([tmp,str(jar),*map(str,sorted((a.oracle_dir/'libraries').rglob('*.jar')))])
        java=subprocess.run([str(a.java_home/'bin/java'),'-cp',classpath,'oracle.GeometryOracle'],input=transcript,text=True,capture_output=True)
        if java.returncode:raise RuntimeError(java.stderr)
        assert 'ORACLE_CLASS=exc,ewx,dcc,exv,bsu;SOURCE='in java.stderr and jar.name in java.stderr
        rust=subprocess.run([tmp+'/probe'],input=transcript,text=True,capture_output=True,check=True)
    expected,actual=java.stdout.splitlines(),rust.stdout.splitlines();assert len(expected)==len(commands)==len(actual)
    mismatches=[{'line':i+1,'command':commands[i],'java':x,'rust':y}for i,(x,y)in enumerate(zip(expected,actual))if x!=y]
    result={'success':not mismatches,'oracle':'actual Minecraft1.21 exc/ewx/dcc/exv/bsu/bsr/cmx/exs/dfy','jar_sha256':JAR_SHA256,'compared_outputs':len(commands),'trace_sha256':hashlib.sha256(java.stdout.encode()).hexdigest(),'covered':['Vec3 normalize/dot/cross/distance/lerp exact canonical f64 bits','Direction orderedByNearest and horizontal yaw facing using original65536entry Mth sine table','AABB normalization/intersection/union/containment/contract/expand/inflate/distance/center','AABB clipping with open segment endpoints and epsilon face bounds','BlockGetter DDA endpoint nudges and tie order','EntityDimensions float width and height promotion','ArrayVoxelShape axis collision against random occupied coordinate grids','BitSetDiscreteVoxelShape greedy box merging order','VoxelShape ray clipping including inside hit direction and float narrowing','Entity.collideWithShapes axis order and multiple shapes','Player.canInteractWithBlock actual eye-position/attribute-range/verification-buffer strict bounds','Shapes.joinIsNotEmpty AND AABB obstruction with epsilon and near-grid snapping','All26684state collision catalog compared with original Java, including273explicitdynamic states','Dynamic state collisions: actual EntityCollisionContext, scaffolding, powder-snow boots/fall, bamboo/dripstone offsets and absent shulker/piston block entities'],'not_covered':['animated shulker/piston block-entity shape generation','VoxelShape boolean joins','complete entity movement/step-up/fluids'],'mismatches':mismatches[:8]}
    (ROOT/'evidence/geometry-conformance.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2));return int(bool(mismatches))
if __name__=='__main__':raise SystemExit(main())
