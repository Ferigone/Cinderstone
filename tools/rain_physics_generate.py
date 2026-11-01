#!/usr/bin/env python3
"""Replay unchanged Level.isRainingAt and private Entity.isInRain on real Biomes."""
from pathlib import Path
import copy,hashlib,json,math,os,random,struct,subprocess
ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle';JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin');PIN='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def db(v):return struct.pack('>d',v).hex()
def build():
 out=[]
 def add(group,op='level',p=(8,64,8),top=65.25,old='3f800000',rain='3f800000',sky=15,biome='plains',height=None,**extra):
  q=dict(op=op,position=list(p),old_rain=old,rain=rain,default=dict(sky=sky,biome=biome),height=p[1]if height is None else height)
  if op=='entity':q['max_y']=db(top)if isinstance(top,(int,float))else top
  q.update(extra);out.append((group,q));return q
 weather=[('00000000','00000000'),('00000000','3f800000'),('3f800000','00000000'),('3e4ccccc','3e4ccccc'),('3e4ccccd','3e4ccccd'),('3e4cccce','3e4cccce'),('00000000','3e4ccccd'),('3f800000','3e4ccccd'),('3e4ccccd','3e4ccccc'),('3e4ccccc','3e4ccccd'),('80000000','00000000'),('00000000','80000000'),('00000001','00000001'),('7fc01234','3f800000'),('3f800000','7fc05678'),('7f800000','3f800000'),('3f800000','7f800000'),('ff800000','3f800000'),('bf800000','40000000')]
 for old,rain in weather:
  for biome in ('plains','snowy_plains','desert'):
   for sky,max_light in ((15,15),(14,15),(0,0),(-2147483648,2147483647),(2147483647,-2147483648)):
    add('weather_threshold_and_sky',old=old,rain=rain,biome=biome,sky=sky,max_light=max_light)
 coords=(-2147483648,-30000001,-30000000,-16777217,-17,-16,-1,0,15,16,16777217,29999999,30000000,2147483647)
 for x in coords:
  for y in (-2147483648,-80,63,64,80,81,319,320,600,608,4096,2147483647):
   add('coordinates_temperature_and_height',p=(x,y,-x if x!=-2147483648 else 2147483647),height=y)
 for lower_biome in ('plains','snowy_plains','desert'):
  for upper_biome in ('plains','snowy_plains','desert'):
   for lower_sky in (14,15):
    for upper_sky in (14,15):
     for height in (64,65,66):
      add('entity_two_points','entity',height=height,cells={'8,64,8':dict(sky=lower_sky,biome=lower_biome),'8,65,8':dict(sky=upper_sky,biome=upper_biome)})
 for biome in ('plains','snowy_plains','desert'):
  for sky in (14,15):
   for top in (64.,64.25,64.99999999999999,-0.,63.99999999999999):
    add('entity_same_or_lower_top','entity',top=top,sky=sky,biome=biome)
 for op in ('level','entity'):
  for fault in ('rain','sky','max','height','biome'):
   for gate in ('rain_false','sky_false','height_false','desert','plains','second_point'):
    q=add('fault_priority',op,old='00000000'if gate=='rain_false'else'3f800000',rain='00000000'if gate=='rain_false'else'3f800000',sky=14 if gate=='sky_false'else 15,biome='desert'if gate=='desert'else'plains',height=65 if gate in ('height_false','second_point')else 64,fault=fault)
    if gate=='second_point':q['cells']={'8,64,8':dict(sky=14,biome='plains'),'8,65,8':dict(sky=15,biome='plains')}
 tops=[-math.inf,-2147483648.25,-2147483648.,-80.,-1.0000000000000002,-1.,-0.9999999999999999,-5e-324,-0.,0.,5e-324,63.99999999999999,64.,64.00000000000001,65.99999999999999,80.,319.99999999999994,320.,600.,2147483647.,2147483648.,math.inf,'7ff8000000001234','fff8000000004321']
 for bottom in (63,64,-1):
  for top in tops:add('entity_floor_boundaries','entity',p=(-17,bottom,-1),top=top,height=64)
 rng=random.Random(0x121A11)
 while len(out)<1000:
  op=rng.choice(('level','entity'));x=rng.choice(coords);y=rng.choice((-80,63,64,65,79,80,81,319,320,591,600,608,616,624));z=rng.choice(coords);top=y+rng.choice((-1.,-0.25,0.,.25,.99999999999999,1.,1.25,4096.))
  old,rain=rng.choice(weather);q=add('mixed_interleavings',op,p=(x,y,z),top=top,old=old,rain=rain,sky=rng.choice((0,14,15,16)),biome=rng.choice(('plains','snowy_plains','desert')),height=y+rng.choice((-1,0,1)),max_light=rng.choice((0,14,15,16)))
  if rng.randrange(5)==0:q['fault']=rng.choice(('rain','sky','max','height','biome'))
  if rng.randrange(3)==0:q['heights']={str(x)+','+str(z):y+rng.choice((-1,0,1))}
 assert len(out)==1000,len(out)
 return out
def main():
 jar=ORACLE/'server-inner-1.21.jar';assert sha(jar)==PIN
 runtime=ROOT/'runtime/rain-physics';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True);sources=[ROOT/'tools/survival_oracle.java',ROOT/'tools/rain_physics_oracle.java'];cp=os.pathsep.join(map(str,[classes,jar,*sorted((ORACLE/'libraries').rglob('*.jar'))]));flags=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED']
 compiled=subprocess.run([JDK/'javac',*flags,'-proc:none','-cp',cp,'-d',classes,*sources],capture_output=True,text=True);assert compiled.returncode==0,compiled.stdout+compiled.stderr
 entries=build();run=subprocess.run([JDK/'java',*flags,'-Xmx1G','-cp',cp,'oracle.rain_physics_oracle'],input=''.join(json.dumps(q,separators=(',',':'))+'\n'for _,q in entries),capture_output=True,text=True,timeout=180);(runtime/'oracle.log').write_text(run.stdout+run.stderr);assert run.returncode==0,(run.stdout+run.stderr)[-9000:]
 origins=[line[7:]for line in run.stdout.splitlines()if line.startswith('SOURCE:')];assert len(origins)==2 and all(s.endswith('server-inner-1.21.jar')for s in origins)
 cases=[json.loads(line[7:])for line in run.stdout.splitlines()if line.startswith('ORACLE:')];assert len(cases)==len(entries)
 groups={};errors=0;true=0;false=0;entity=0;traces=0
 for (group,q),case in zip(entries,cases):
  assert q==case['query'];case['group']=group;groups[group]=groups.get(group,0)+1;entity+=q['op']=='entity';e=case['expected'];traces+=len(e['trace'])
  if e['accepted']:
   true+=e['result']is True;false+=e['result']is False
  else:
   errors+=1;assert q.get('fault')and e['error_class']=='java.lang.IllegalStateException'and e['error_message']=='controlled/'+q['fault'],case
 fixture=dict(minecraft='1.21',oracle_inner_sha256=PIN,original_classes_modified=False,cases=cases);dest=ROOT/'fixtures/rain-physics-vectors.json';dest.write_text(json.dumps(fixture,separators=(',',':'))+'\n')
 report=dict(ok=True,minecraft='1.21',oracle_inner_sha256=PIN,original_classes_modified=False,source_sha256={str(p.relative_to(ROOT)):sha(p)for p in sources+[Path(__file__)]},fixture_sha256=sha(dest),cases=len(cases),level_cases=len(cases)-entity,entity_cases=entity,accepted_true=true,accepted_false=false,controlled_faults=errors,trace_calls=traces,groups=groups,
  scope='Unchanged Level.isRainingAt and BlockAndTintGetter.canSeeSky, actual ItemEntity inherited private Entity.isInRain, original rain interpolation, registered plains/snowy_plains/desert precipitation and exact lower-world query encounter order/fault priority. Entity cached BlockPos X/Z differs from AABB X/Z.',
  not_verified=['Rust until independent native replay.','Controlled brightness/height callbacks do not establish physical source-world lighting; separate rain-fixture-facts prove the fresh canonical fixture.','Biome temperature cache persists in the controlled JVM; supplied actual world_facts record its current results, not an assumed empty temperature cache per case.','General imported/edited world exposure, arbitrary listener/biome source coverage, and complete server parity.'])
 (ROOT/'evidence/rain-physics-oracle.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
