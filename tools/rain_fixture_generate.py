#!/usr/bin/env python3
"""Extract genuine flat chunk height/skylight and actual plains biome facts."""
from pathlib import Path
import hashlib,json,os,subprocess
ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle';JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin');PIN='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
 jar=ORACLE/'server-inner-1.21.jar';assert sha(jar)==PIN
 blocks=json.loads((ORACLE/'reports/reports/blocks.json').read_text())
 states=[next(s['id']for s in blocks['minecraft:'+name]['states']if s.get('default',False))for name in ('air','stone','dirt','grass_block')];assert states==[0,1,10,9],states
 runtime=ROOT/'runtime/rain-fixture';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True);source=ROOT/'tools/rain_fixture_oracle.java';cp=os.pathsep.join(map(str,[classes,jar,*sorted((ORACLE/'libraries').rglob('*.jar'))]));flags=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED']
 compile=subprocess.run([JDK/'javac',*flags,'-proc:none','-cp',cp,'-d',classes,source],capture_output=True,text=True);assert compile.returncode==0,compile.stdout+compile.stderr
 run=subprocess.run([JDK/'java',*flags,'-Xmx1G','-cp',cp,'oracle.rain_fixture_oracle'],capture_output=True,text=True,timeout=180);(runtime/'oracle.log').write_text(run.stdout+run.stderr);assert run.returncode==0,(run.stdout+run.stderr)[-9000:]
 facts=json.loads(next(line[7:]for line in run.stdout.splitlines()if line.startswith('ORACLE:')));assert all(v.endswith('server-inner-1.21.jar')for v in facts.pop('sources').values());facts['oracle_inner_sha256']=PIN
 assert facts['plains_initial_temperature_cache_size']==0 and facts['light_pending']is False
 assert len(facts['height_columns_x_z_highest_levelheight_lowestskysource'])==256
 assert all(row[2:]==[63,64,64]for row in facts['height_columns_x_z_highest_levelheight_lowestskysource'])
 assert all(v[1]==(15 if v[0]>=64 else 0)and v[2]==(v[0]>=64)for row in facts['sky_rows_y_brightness_canSeeSky']for v in row)
 assert all(row[-1]=='RAIN'for row in facts['plains_temperature_precipitation_probes'])
 assert facts['world_rng_before']==facts['world_rng_after']
 dest=ROOT/'fixtures/rain-fixture-facts.json';dest.write_text(json.dumps(facts,separators=(',',':'))+'\n')
 report=dict(ok=True,minecraft='1.21',oracle_inner_sha256=PIN,original_classes_modified=False,source_sha256=sha(source),generator_sha256=sha(Path(__file__)),facts_sha256=sha(dest),genuine_level_chunks=facts['chunk_count'],height_columns=256,sky_columns=256,sky_y_values_per_column=417,actual_sky_queries=256*417,plains_probes=len(facts['plains_temperature_precipitation_probes']),level_rain_cases=len(facts['actual_level_rain_cases']),scope='Original LevelChunk constructor/sections, Heightmap.primeHeightmaps, ChunkSkyLightSources and actual settled SKY LightEngine; original canSeeSky/isRainingAt, actual bootstrapped plains climate/temperature cache/noise state.',not_verified=['General threaded lighting, edits, arbitrary worlds/biomes or future pending light work.','Uniform precipitation for all possible coordinates/heights; bounded native predicate requires its own noise arithmetic or proof.','Entity.isInRain two-point sampling is a separate original oracle.'])
 (ROOT/'evidence/rain-fixture-oracle.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
