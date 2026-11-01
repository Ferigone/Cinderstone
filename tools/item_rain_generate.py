#!/usr/bin/env python3
"""Actual complete ItemEntity ticks in genuine-light fresh-fixture rain scenes."""
from pathlib import Path
import hashlib,json,os,struct,subprocess
ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle';JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin');PIN='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def db(v):return struct.pack('>d',v).hex()
def build():
 cases=[]
 for name,fire,old,rain,changes,y in [
  ('wet_fire10',10,'3f800000','3f800000',[],65.),
  ('wet_fire25',25,'3f800000','3f800000',[],65.),
  ('clear_fire10',10,'00000000','00000000',[],65.),
  ('threshold_equal_float_fire25',25,'3e4ccccd','3e4ccccd',[],65.),
  ('threshold_from_one_fire10',10,'3f800000','3e4ccccd',[],65.),
  ('clear_then_wet_fire25',25,'00000000','00000000',[dict(tick=2,old_rain='3f800000',rain='3f800000')],65.),
  ('clear_then_wet_fire10',10,'00000000','00000000',[dict(tick=4,old_rain='3f800000',rain='3f800000')],65.),
  ('only_upper_point_exposed_fire25',25,'3f800000','3f800000',[],63.875),
 ]:
  cases.append(dict(name=name,position=list(map(db,[8.5,y,8.5])),velocity=list(map(db,[0.,0.,0.])),blocks=[],entity_id=400+len(cases),seed=712101+len(cases),tick_count=0,age=0,pickup_delay=40,on_ground=False,fall_distance='00000000',no_gravity=False,ticks=10,drain_initial_metadata=True,clear_has_impulse=True,fire_ticks=fire,weather=dict(old_rain=old,rain=rain),weather_changes=changes))
 return cases
def main():
 jar=ORACLE/'server-inner-1.21.jar';assert sha(jar)==PIN
 source_names=['tools/survival_oracle.java','tools/placement_tags.java','tools/rain_fixture_oracle.java','tools/item_rain_oracle.java'];sources=[ROOT/p for p in source_names];runtime=ROOT/'runtime/item-rain';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True);cp=os.pathsep.join(map(str,[classes,jar,*sorted((ORACLE/'libraries').rglob('*.jar'))]));flags=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED']
 compiled=subprocess.run([JDK/'javac',*flags,'-proc:none','-cp',cp,'-d',classes,*sources],capture_output=True,text=True);assert compiled.returncode==0,compiled.stdout+compiled.stderr
 queries=build();run=subprocess.run([JDK/'java',*flags,'-Xmx1G','-cp',cp,'oracle.item_rain_oracle',ROOT/'fixtures/placement-block-tags-1.21.json'],input=''.join(json.dumps(q,separators=(',',':'))+'\n'for q in queries),capture_output=True,text=True,timeout=180);(runtime/'oracle.log').write_text(run.stdout+run.stderr);assert run.returncode==0,(run.stdout+run.stderr)[-10000:]
 origin=[s[7:]for s in run.stdout.splitlines()if s.startswith('SOURCE:')];assert origin and all(s.endswith('server-inner-1.21.jar')for s in origin)
 results=[json.loads(s[7:])for s in run.stdout.splitlines()if s.startswith('ORACLE:')];assert len(results)==8
 sound_draws=json.loads(next(s[10:]for s in run.stdout.splitlines()if s.startswith('SOUND_RNG:')));assert len(sound_draws)==4
 assert all(s['world_rng_state']==r['world_rng_initial_state']for r in results for s in r['states'])
 fixture=dict(minecraft='1.21',oracle_inner_sha256=PIN,original_classes_modified=False,entity_class='actual original cjh; no Entity/ItemEntity overrides',rng_seeds=dict(level_random=12345,level_thread_safe_random=67890),actual_dzr_draws_before_nextLong_after=sound_draws,scenes=[dict(query=q,expected=r)for q,r in zip(queries,results)]);dest=ROOT/'fixtures/item-rain-vectors.json';dest.write_text(json.dumps(fixture,separators=(',',':'))+'\n')
 sound_events=[event for r in results for s in r['states']for event in s['events']if isinstance(event,dict)and event.get('kind')=='sound'];assert sound_events and all(e['name']=='minecraft:entity.generic.extinguish_fire'for e in sound_events),sound_events
 report=dict(ok=True,minecraft='1.21',oracle_inner_sha256=PIN,original_classes_modified=False,source_sha256={p:sha(ROOT/p)for p in source_names+['tools/item_rain_generate.py']},fixture_sha256=sha(dest),scenes=len(results),states=sum(len(r['states'])for r in results),sound_events=len(sound_events),sound_names=sorted({e['name']for e in sound_events}),level_random_unchanged=True,level_sound_random='Original Level.threadSafeRandom dcw.f = genuine ThreadSafeLegacyRandomSource dzr(67890), separate from Level.random dcw.z = LegacyRandomSource dyz(12345).',scope='Actual complete ItemEntity.tick/baseTick/move/fluid/inside-block execution, genuine canonical LevelChunk Heightmap/ChunkSkyLightSources and settled original SKY LightEngine, original Level.isRainingAt/Entity.isInRain, fire countdown/wet extinguish, actual Level.playSound threadSafeRandom seed draw and required seeded-sound delivery recording.',not_verified=['Rust composite replay until separately run.','General rain generation, edited/imported worlds, complete lighting or non-plains biomes.','Fire damage pulses: fire25 scenes become wet before their first20-tick damage pulse; clear cases begin at10.','Real network sound recipients/packet delivery; lower delivery callback records sound facts after original RNG/pitch arithmetic.','World game-event dispatcher/listener behavior; existing ItemFull environment records emitted events with empty neighbouring-entity/listener input.','Four additional actual dzr.nextLong draws are scalar RNG vectors, not an original multi-item world-scene proof.'])
 (ROOT/'evidence/item-rain-oracle.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
