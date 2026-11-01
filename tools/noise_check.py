#!/usr/bin/env python3
"""Exact integer/IEEE-bit differential tests against the official 1.21 classes."""
import argparse, hashlib, itertools, json, math, os, random, struct, subprocess, tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
EXPECTED='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def bits(value):return str(struct.unpack('>Q',struct.pack('>d',value))[0])
def main():
 p=argparse.ArgumentParser();p.add_argument('--oracle-dir',type=Path,default=ROOT.parent/'mc121-oracle');p.add_argument('--java-home',type=Path,default=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1'));p.add_argument('--probe',type=Path);p.add_argument('--rounds',type=int,default=100);p.add_argument('--release',action='store_true');args=p.parse_args()
 if args.rounds<1:p.error('--rounds must be positive')
 jar=args.oracle_dir/'server-inner-1.21.jar'
 if hashlib.sha256(jar.read_bytes()).hexdigest()!=EXPECTED:raise RuntimeError('Wrong official Minecraft 1.21 JAR')
 env=os.environ.copy();env.setdefault('CARGO_HOME','/root/.local/share/mc-rust-toolchain/cargo');env.setdefault('RUSTUP_HOME','/root/.local/share/mc-rust-toolchain/rustup');env['PATH']=env['CARGO_HOME']+'/bin:'+env.get('PATH','');env['CARGO_INCREMENTAL']='0'
 target=Path(env.get('CARGO_TARGET_DIR',ROOT/'target'))
 if not target.is_absolute():target=ROOT/target
 if args.probe:probe=args.probe.resolve()
 else:
  subprocess.run(['cargo','build','--lib',*(['--release']if args.release else[])],cwd=ROOT,env=env,check=True)
  profile='release'if args.release else'debug'
  probe=target/profile/'noise_probe'
  dependencies=target/profile/'deps'
  extern=['--extern','mc121_rust='+str(target/profile/'libmc121_rust.rlib')]
  for name in ['anyhow','serde_json']:
   matches=sorted(dependencies.glob('lib'+name+'-*.rlib'),key=lambda v:v.stat().st_mtime,reverse=True);extern.extend(['--extern',name+'='+str(matches[0])])
  subprocess.run(['rustc','--edition=2021',*(['-O','-C','panic=abort']if args.release else[]),str(ROOT/'tools/noise_probe.rs'),'-L','dependency='+str(dependencies),*extern,'-o',str(probe)],cwd=ROOT,env=env,check=True)
 rng=random.Random(0x121998);commands=[]
 def cmd(op,**fields):commands.append({'op':op,**fields})
 def sample(source,methods=('i32','i64','bool','f32','f64')):
  for method in methods:cmd('rng_sample',source=source,method=method)
 def utf16(s):return list(struct.unpack('<'+'H'*(len(s.encode('utf-16-le'))//2),s.encode('utf-16-le')))
 strings=[[],utf16('minecraft:overworld'),utf16('octave_-7'),utf16('octave_0'),utf16('😀żółć'),[0xd800],[0xdc00],[0xd800,0x61,0xdc00],[0xd800,0xdc00],[0xffff,0,1]]
 bounds=[-2147483648,-1,0,1,2,3,7,16,255,256,257,1073741823,1073741824,1073741825,2147483647]
 seeds=[0,1,-1,-(1<<63),(1<<63)-1,*[rng.randrange(-(1<<63),1<<63)for _ in range(max(0,args.rounds-5))]][:args.rounds]
 for index,seed in enumerate(seeds):
  cmd('mix',seed=seed);cmd('upgrade',seed=seed,mixed=False);cmd('upgrade',seed=seed,mixed=True)
  cmd('coord',x=rng.randrange(-(1<<31),1<<31),y=rng.randrange(-(1<<31),1<<31),z=rng.randrange(-(1<<31),1<<31))
  units=strings[index%len(strings)] if index<len(strings) else [rng.randrange(65536)for _ in range(rng.randrange(30))]
  cmd('hash',units=units)
  for kind in ['legacy','xoro']:
   cmd('rng_new',id='r',kind=kind,seed=seed)
   for _ in range(10):sample('r')
   for bound in bounds:cmd('rng_sample',source='r',method='bounded',bound=bound)
   cmd('rng_state',source='r');cmd('rng_consume',source='r',count=index%13-3);sample('r')
   cmd('rng_fork',source='r',id='child');sample('child');sample('r')
   cmd('rng_factory',source='r',id='factory');sample('r')
   for op,fields in [('factory_at',{'x':rng.randrange(-(1<<31),1<<31),'y':rng.randrange(-(1<<31),1<<31),'z':rng.randrange(-(1<<31),1<<31)}),('factory_hash',{'units':units}),('factory_seed',{'seed':seed})]:
    cmd(op,factory='factory',id='child',**fields);sample('child');cmd('rng_state',source='child')
   cmd('rng_seed',source='r',seed=seed);sample('r')
  cmd('rng_new',id='zero',kind='xoro',lo=0 if index==0 else seed,hi=0 if index==0 else seeds[(index+1)%len(seeds)]);sample('zero');cmd('rng_state',source='zero')
 for seed in seeds[:max(10,args.rounds//2)]:
  for kind in ['legacy','xoro']:
   cmd('rng_new',id='r',kind=kind,seed=seed);cmd('improved_new',source='r',id='imp');cmd('rng_state',source='r');cmd('simplex_new',source='r',id='simplex');cmd('rng_state',source='r')
   for i in range(60):
    coords=rng.choices([0.,-0.,1.,-1.,-64.,319.,16.,-16.,16777216.,-16777216.,29999984.,-29999984.],k=3)if i<10 else [rng.uniform(-30000000,30000000),rng.uniform(-2048,2048),rng.uniform(-30000000,30000000)]
    fields=dict(zip(['x','y','z'],map(bits,coords)))
    scale=rng.choice([0.,0.1,0.125,0.5,-0.5,16.]);ymax=rng.choice([-1.,0.,0.1,0.5,1.,16.])
    cmd('improved',id='imp',**fields,scale=bits(scale),ymax=bits(ymax));cmd('derivative',id='imp',**fields,initial=list(map(bits,[rng.uniform(-1,1)for _ in range(3)])))
    cmd('simplex2',id='simplex',x=fields['x'],y=fields['y']);cmd('simplex3',id='simplex',**fields)
   for legacy in [False,True]:
    for first,amps in [(-7,[1.,1.,0.,1.]),(-3,[0.,1.,0.,0.]),(-3,[1.,0.5,0.25,1.]),(-1,[1.]),(0,[1.]),(-10,[0.,0.]),(0,[]),(-3,[-1.,0.,2.])]:
     parameters={'first':first,'amplitudes':list(map(bits,amps)),'legacy':legacy}
     cmd('perlin_new',source='r',id='p',**parameters);cmd('rng_state',source='r');cmd('normal_new',source='r',id='n',**parameters);cmd('rng_state',source='r')
     for i in range(12):
      fields=dict(zip(['x','y','z'],map(bits,[rng.uniform(-30000000,30000000),rng.uniform(-2048,2048),rng.uniform(-30000000,30000000)])))
      cmd('perlin',id='p',**fields,scale=bits(rng.choice([0.,0.125,0.5])),ymax=bits(rng.choice([-1.,0.,1.,16.])),origin=bool(i%2));cmd('normal',id='n',**fields)
 for index,seed in enumerate(seeds):
  x=rng.randrange(-(1<<31),1<<31);z=rng.randrange(-(1<<31),1<<31)
  for kind in ['legacy','xoro']:
   for method in ['plain','decoration','feature','large','salt']:cmd('worldgen',kind=kind,seed=seed,x=x,z=z,salt=rng.randrange(-(1<<31),1<<31),method=method)
   cmd('rng_new',id='r',kind=kind,seed=seed)
   for octaves in [[-7,-5,-3],[-3,-2,-1,0],[-2,0,2],[0],[1,2],[0,0,-2],[-4],[3]]:
    cmd('perlinsimplex_new',source='r',id='ps',octaves=octaves);cmd('rng_state',source='r')
    for i in range(20):cmd('perlinsimplex',id='ps',x=bits(rng.uniform(-30000000,30000000)),y=bits(rng.uniform(-30000000,30000000)),origin=bool(i%2))
  cmd('slime',x=x,z=z,seed=seed,salt=rng.randrange(-(1<<63),1<<63))
 for seed in seeds:
  for kind in ['legacy','xoro']:
   cmd('rng_new',id='r',kind=kind,seed=seed)
   # Exact built-in overworld/end/nether base_3d_noise parameters from the JAR.
   for xz_scale,y_scale,xz_factor,y_factor,smear in [(0.25,0.125,80.,160.,8.),(0.25,0.25,80.,160.,4.),(0.25,0.375,80.,60.,8.)]:
    cmd('blended_new',id='blend',source='r',xz_scale=bits(xz_scale),y_scale=bits(y_scale),xz_factor=bits(xz_factor),y_factor=bits(y_factor),smear=bits(smear));cmd('rng_state',source='r')
    for i in range(40):
     x,y,z=([0,0,0]if i==0 else [rng.randrange(-30000000,30000000),rng.randrange(-2048,2048),rng.randrange(-30000000,30000000)])
     cmd('blended',id='blend',x=x,y=y,z=z)
 parameters=json.loads((ROOT/'fixtures/noise_parameters-1.21.json').read_text())
 if parameters['source_inner_sha256']!=EXPECTED:raise RuntimeError('Wrong noise parameter data provenance')
 for name,parameter in parameters['noises'].items():
  for kind in ['legacy','xoro']:
   cmd('rng_new',id='r',kind=kind,seed=123456789)
   for legacy in [False,True]:
    cmd('normal_new',source='r',id='builtin',first=parameter['first_octave'],amplitudes=list(map(bits,parameter['amplitudes'])),legacy=legacy);cmd('rng_state',source='r')
    for i in range(10):cmd('normal',id='builtin',x=bits(rng.uniform(-30000000,30000000)),y=bits(rng.uniform(-2048,2048)),z=bits(rng.uniform(-30000000,30000000)))
 boundary=json.loads((ROOT/'fixtures/noise_boundaries.json').read_text())
 if boundary['jar_sha256']!=EXPECTED:raise RuntimeError('Wrong noise-boundary fixture provenance')
 for kind,offsets in boundary['offset_bits'].items():
  offsets=[struct.unpack('>d',struct.pack('>Q',int(value)))[0]for value in offsets]
  cmd('rng_new',id='br',kind=kind,seed=boundary['seed']);cmd('improved_new',id='bimp',source='br')
  for lattice in [0,1,-1,16,-16,29999984,-29999984,2147483647,-2147483648]:
   bases=[float(lattice)-offsets[0],float(lattice%4096)-offsets[1],float(lattice+1)-offsets[2]]
   for perturb in itertools.product([-1,0,1],repeat=3):
    coords=[math.nextafter(base,math.inf if shift>0 else-math.inf)if shift else base for base,shift in zip(bases,perturb)]
    fields=dict(zip(['x','y','z'],map(bits,coords)))
    cmd('derivative',id='bimp',**fields,initial=list(map(bits,[0.,-0.,1.])))
    for scale in [0.,0.125,0.1]:cmd('improved',id='bimp',**fields,scale=bits(scale),ymax=bits(0.125))
 for n in [-1000,-2,-1,0,1,2,1000]:
  value=33554432.0*(n+0.5)
  for coordinate in [math.nextafter(value,-math.inf),value,math.nextafter(value,math.inf)]:cmd('wrap',value=bits(coordinate))
 for value in [0.,-0.,1.,-1.,16777216.,-16777216.,33554432.,-33554432.,float(1<<53),-float(1<<53),*[rng.uniform(-1e15,1e15)for _ in range(100)]]:cmd('wrap',value=bits(value))
 transcript='\n'.join(json.dumps(command,separators=(',',':'))for command in commands)+'\n';libraries=sorted((args.oracle_dir/'libraries').rglob('*.jar'))
 with tempfile.TemporaryDirectory(prefix='mc121-noise-oracle-')as classes:
  cp=os.pathsep.join([classes,str(jar),*map(str,libraries)])
  subprocess.run([str(args.java_home/'bin/javac'),'-proc:none','-cp',cp,'-d',classes,str(ROOT/'tools/noise_Oracle.java')],check=True)
  java=subprocess.run([str(args.java_home/'bin/java'),'-Xmx768m','-cp',cp,'oracle.NoiseOracle'],input=transcript,text=True,capture_output=True)
  if java.returncode:raise RuntimeError(java.stderr[-6000:]+'\n'+java.stdout[-1500:])
  if 'NOISE_ORACLE_CLASSES=dyz,dzy,dzz,dzn,dzx,eof,eok,eoi,eoh,eoj,eoe;SOURCE='not in java.stderr or jar.name not in java.stderr:raise RuntimeError('Wrong oracle provenance')
  rust=subprocess.run([str(probe)],input=transcript,text=True,capture_output=True,check=True)
 expected=[json.loads(line)for line in java.stdout.splitlines()];actual=[json.loads(line)for line in rust.stdout.splitlines()]
 if len(expected)!=len(commands)or len(actual)!=len(commands):raise RuntimeError('Incomplete oracle/probe traces')
 differences=[{'line':i+1,'command':commands[i],'java':a,'rust':b}for i,(a,b)in enumerate(zip(expected,actual))if a!=b]
 result={'success':not differences,'oracle':'actual Minecraft1.21 dyz/dzy/dzz/dzn/dzx/eof/eok/eoi/eoh/eoj/eoe','jar_sha256':EXPECTED,'rounds':args.rounds,'profile':'release'if args.release else'debug','compared_outputs':len(commands),'trace_sha256':hashlib.sha256(java.stdout.encode()).hexdigest(),'covered':['Legacy and Xoroshiro integer/float outputs and rejection sampling','zero seed128 fallback','RandomSupport mix,upgrade and MD5 UTF8 hashing','isolated UTF16 surrogates and coordinate integer overflow','fork and positional factories/state consumption','ImprovedNoise permutations,scaled sampling and derivative accumulation','Simplex2D/3D raw sample bits','modern and legacy Perlin octave initialization,zero amplitudes and coordinate wrapping','NormalNoise max bounds and raw sample bits','WorldgenRandom wrapper consumption count and decoration/feature/largefeature seed mixing','slime chunk mixed int/long arithmetic','PerlinSimplexNoise negative/zero/positive octaves and origin offsets','lattice planes/corners ±1ULP, y-scale epsilon and wrapping boundaries','BlendedNoise bounds/samples with official overworld/end/nether parameters','all60official built-in noise parameter definitions'],'not_covered':['Gaussian sampling','density router,terrain surfaces,carvers,features and structures'],'mismatches':differences[:10]}
 (ROOT/('tools/noise_conformance_release.json'if args.release else'tools/noise_conformance.json')).write_text(json.dumps(result,indent=2)+'\n')
 if differences:(ROOT/'tools/noise_failure.jsonl').write_text(transcript)
 print(json.dumps(result,indent=2));return int(bool(differences))
if __name__=='__main__':raise SystemExit(main())
