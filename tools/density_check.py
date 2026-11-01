#!/usr/bin/env python3
"""Compare typed Rust density graphs with actual 1.21 RandomState routers."""
import argparse,hashlib,json,os,random,subprocess,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
PIN='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def main():
 p=argparse.ArgumentParser();p.add_argument('--oracle-dir',type=Path,default=ROOT.parent/'mc121-oracle');p.add_argument('--java-home',type=Path,default=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1'));p.add_argument('--rounds',type=int,default=10);p.add_argument('--release',action='store_true');p.add_argument('--probe',type=Path);args=p.parse_args()
 jar=args.oracle_dir/'server-inner-1.21.jar'
 if hashlib.sha256(jar.read_bytes()).hexdigest()!=PIN:raise RuntimeError('Wrong Minecraft1.21 oracle artifact')
 env=os.environ.copy();env.setdefault('CARGO_HOME','/root/.local/share/mc-rust-toolchain/cargo');env.setdefault('RUSTUP_HOME','/root/.local/share/mc-rust-toolchain/rustup');env['PATH']=env['CARGO_HOME']+'/bin:'+env.get('PATH','');env['CARGO_INCREMENTAL']='0';profile='release'if args.release else'debug';target=Path(env.get('CARGO_TARGET_DIR',ROOT/'target'));target=target if target.is_absolute()else ROOT/target
 if args.probe:probe=args.probe.resolve()
 else:
  subprocess.run(['cargo','build','--lib',*(['--release']if args.release else[])],cwd=ROOT,env=env,check=True);probe=target/profile/'density_probe';deps=target/profile/'deps';extern=['--extern','mc121_rust='+str(target/profile/'libmc121_rust.rlib')]
  for name in ['anyhow','serde_json']:
   candidates=sorted(deps.glob('lib'+name+'-*.rlib'),key=lambda p:p.stat().st_mtime,reverse=True);extern.extend(['--extern',name+'='+str(candidates[0])])
  subprocess.run(['rustc','--edition=2021',*(['-O','-C','panic=abort']if args.release else[]),str(ROOT/'tools/density_probe.rs'),'-L','dependency='+str(deps),*extern,'-o',str(probe)],env=env,cwd=ROOT,check=True)
 data=json.loads((ROOT/'fixtures/density_builtin-1.21.json').read_text());rng=random.Random(0x121767551);commands=[]
 def cmd(op,**fields):commands.append({'op':op,**fields})
 seeds=[0,1,-1,-(1<<63),(1<<63)-1,*[rng.randrange(-(1<<63),1<<63)for _ in range(max(0,args.rounds-5))]][:args.rounds]
 settings=list(data['settings'])
 for seed in seeds:
  for setting in settings:
   cmd('world',seed=seed,settings=setting);cmd('router')
   points=[[0,0,0],[0,64,0],[-1,-64,-1],[15,319,16],[-16,320,-17],[29999984,100,-29999984]]+[[rng.randrange(-30000000,30000000),rng.randrange(-2048,2048),rng.randrange(-30000000,30000000)]for _ in range(10)]
   cmd('climate',points=[[p[0]//4,p[1]//4,p[2]//4]for p in points]+[[rng.randrange(-(1<<31),1<<31),rng.randrange(-(1<<31),1<<31),rng.randrange(-(1<<31),1<<31)]for _ in range(4)])
   for node in data['settings'][setting]['noise_router']:
    cmd('eval',id=node,points=points);cmd('fill',id=node,points=points)
 for setting in ['minecraft:overworld','minecraft:nether']:
  cmd('world',seed=123456789,settings=setting)
  for name in data['definitions']:
   cmd('load',id='definition',function=name)
   points=[[rng.randrange(-100000,100000),rng.randrange(-128,384),rng.randrange(-100000,100000)]for _ in range(20)]
   cmd('eval',id='definition',points=points);cmd('fill',id='definition',points=points)
  gradient={'type':'minecraft:y_clamped_gradient','from_y':-64,'to_y':320,'from_value':-1.0,'to_value':1.0}
  cases=[0.0,-0.0,0.37,*[rng.uniform(-999999,999999)for _ in range(128)],{'type':'minecraft:clamp','input':gradient,'min':-0.5,'max':0.5}]
  for kind in ['abs','square','cube','half_negative','quarter_negative','squeeze','interpolated','flat_cache','cache_2d','cache_once','cache_all_in_cell','blend_density']:cases.append({'type':'minecraft:'+kind,'argument':gradient})
  for kind in ['add','mul','min','max']:
   for left,right in [(gradient,0.0),(0.0,gradient),(gradient,-0.0),(-0.0,gradient),(gradient,gradient)]:cases.append({'type':'minecraft:'+kind,'argument1':left,'argument2':right})
  cases.extend([{'type':'minecraft:range_choice','input':gradient,'min_inclusive':-0.5,'max_exclusive':0.5,'when_in_range':1.0,'when_out_of_range':-1.0},{'type':'minecraft:spline','spline':{'coordinate':gradient,'points':[{'location':-1.0,'value':-0.5,'derivative':0.25},{'location':0.0,'value':1.0,'derivative':-1.0},{'location':1.0,'value':0.0,'derivative':0.5}]}}])
  for function in cases:
   cmd('load',id='custom',function=function);points=[[0,y,0]for y in [-128,-65,-64,-63,0,31,32,63,64,65,127,128,129,255,256,319,320,321,384]];cmd('eval',id='custom',points=points);cmd('fill',id='custom',points=points)
 transcript='\n'.join(json.dumps(c,separators=(',',':'))for c in commands)+'\n';libraries=sorted((args.oracle_dir/'libraries').rglob('*.jar'))
 with tempfile.TemporaryDirectory(prefix='mc121-density-oracle-')as classes:
  cp=os.pathsep.join([classes,str(jar),*map(str,libraries)]);subprocess.run([str(args.java_home/'bin/javac'),'-proc:none','-cp',cp,'-d',classes,str(ROOT/'tools/density_Oracle.java')],check=True)
  java=subprocess.run([str(args.java_home/'bin/java'),'-Xmx1024m','-cp',cp,'oracle.DensityOracle'],input=transcript,text=True,capture_output=True)
  if java.returncode:raise RuntimeError(java.stderr[-5000:]+'\n'+java.stdout[-2000:])
  if 'DENSITY_ORACLE_CLASS=dyr,dys,dzm,axp;SOURCE='not in java.stderr or jar.name not in java.stderr:raise RuntimeError('Wrong density oracle provenance')
  rust=subprocess.run([str(probe)],input=transcript,text=True,capture_output=True,check=True)
 expected=list(map(json.loads,java.stdout.splitlines()));actual=list(map(json.loads,rust.stdout.splitlines()))
 if len(expected)!=len(commands)or len(actual)!=len(commands):raise RuntimeError('Incomplete traces')
 differences=[{'line':i+1,'command':commands[i],'java':a,'rust':b}for i,(a,b)in enumerate(zip(expected,actual))if a!=b]
 values=sum(len(c['points'])for c in commands if c['op']in['eval','fill']);result={'success':not differences,'oracle':'actual Minecraft1.21 DensityFunction/RandomState/CubicSpline classes','jar_sha256':PIN,'profile':profile,'seeds':len(seeds),'routers':len(settings),'compared_outputs':len(commands),'sampled_scalar_values':values,'trace_sha256':hashlib.sha256(java.stdout.encode()).hexdigest(),'covered':['typed graph loading and 7official routers','legacy/modern RandomState noise and terrain bindings','all35official named density definitions','raw scalar and fillArray sampling','bounds and cubic float32 spline evaluation','arithmetic,clamp,mapped,range,ygradient,noise,shift,rarity,end-islands nodes','marker pass-through before NoiseChunk wrapping','Climate.Sampler quart conversion/float32 quantization/parameter arrays'],'not_covered':['NoiseChunk cache/interpolation state machines','nonempty Blender/structure beardifier','chunk terrain/biome generation'],'mismatches':differences[:20]}
 (ROOT/('tools/density_conformance_release.json'if args.release else'tools/density_conformance.json')).write_text(json.dumps(result,indent=2)+'\n')
 if differences:(ROOT/'tools/density_failure.jsonl').write_text(transcript)
 print(json.dumps(result,indent=2));return int(bool(differences))
if __name__=='__main__':raise SystemExit(main())
