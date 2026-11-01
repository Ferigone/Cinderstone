#!/usr/bin/env python3
"""Check the concrete native item world against pinned original ItemEntity ticks."""
from pathlib import Path
import hashlib,json,os,re,shutil,subprocess,tempfile
ROOT=Path(__file__).resolve().parents[1]
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def run(args,env,log):
 result=subprocess.run(args,cwd=ROOT,env=env,text=True,capture_output=True)
 log.write_text(result.stdout+result.stderr)
 if result.returncode:raise RuntimeError((result.stdout+result.stderr)[-8500:])
 return result.stdout+result.stderr
def main():
 env=os.environ.copy();env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup',CARGO_INCREMENTAL='0');env.pop('MC121_ITEM_ANVIL_WORLD',None)
 cargo='/root/.local/share/mc-rust-toolchain/cargo/bin/cargo';runtime=ROOT/'runtime/runtime-item-world';runtime.mkdir(parents=True,exist_ok=True)
 fixture=ROOT/'fixtures/item-full-vectors.json';vectors=json.loads(fixture.read_text());assert vectors['oracle_inner_sha256']=='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
 assert len(vectors['scenes'])==64 and sum(len(s['states'])for s in vectors['scenes'])==2616
 water_fixture=ROOT/'fixtures/item-water-vectors.json';water=json.loads(water_fixture.read_text());assert water['oracle_inner_sha256']==vectors['oracle_inner_sha256']
 report=dict(ok=False,minecraft='1.21',oracle_inner_sha256=vectors['oracle_inner_sha256'],original_entity_class=vectors['entity_class'],rust_build_profile='debug',rust_release_verified=False,fixture_scenes=len(vectors['scenes']),fixture_tick_states=sum(len(s['states'])for s in vectors['scenes']),static_full_collision_states_compared=26411,entity_rng_final_values_compared=len(vectors['scenes']),full_tick_vectors_sha256=digest(fixture),full_collision_vectors_sha256=digest(ROOT/'fixtures/runtime_item_world_shapes.json'),not_verified=['Live server/network integration, multi-item merge order and player pickup','Full item ticks in fluids, hazards and custom block callbacks','Vehicles/passengers, portals, leashes, active profiler and damage callbacks','Water ray shape-cache history and uncached block-entity geometry'])
 report.update(water_scenes=len(water['scenes']),water_tick_states=sum(len(s['states'])for s in water['scenes']),water_tick_vectors_sha256=digest(water_fixture),entity_rng_final_values_compared=len(vectors['scenes'])+len(water['scenes']))
 report['not_verified'][1]='Water entry/exit splash, lava/hazards and custom block callbacks'
 report['not_verified'][2]='Vehicles/passengers, portals, leashes, active profiler and live host listener/removal integration'
 report['not_verified'][3]='Unknown fluid shape prehistory and uncached block-entity geometry'
 report['fluid_shape_vectors_sha256']=digest(ROOT/'fixtures/fluid-shape-vectors.json')
 report.update(server_damage_cases=440,base_damage_mapping_cases=62,damage_destruction_states=84,explicitly_unsupported_default_destruction_targets=33,item_hurt_vectors_sha256=digest(ROOT/'fixtures/item-hurt-vectors.json'),item_destruction_facts_sha256=digest(ROOT/'fixtures/item-destroyed-facts.json'))
 try:
  text=run([cargo,'test','--lib','runtime_item_world::','--','--nocapture'],env,runtime/'native-fixture.log');report['rust_fixture_tests_passed']=int(re.search(r'test result: ok\. (\d+) passed',text).group(1))
  assert report['rust_fixture_tests_passed']==9
  source=ROOT.parent/'mc121-oracle/world';original_files=[source/'level.dat',*sorted((source/'region').glob('*.mca'))];assert len(original_files)>=2
  original_hashes={str(p.relative_to(source)):digest(p)for p in original_files}
  directory=Path(tempfile.mkdtemp(prefix='anvil-copy-',dir=runtime));world=directory/'world';world.mkdir();shutil.copyfile(source/'level.dat',world/'level.dat');shutil.copytree(source/'region',world/'region',copy_function=shutil.copyfile)
  env['MC121_ITEM_ANVIL_WORLD']=str(world)
  text=run([cargo,'test','--lib','runtime_item_world::tests::full_original_item_ticks_in_concrete_runtime_world','--','--exact','--nocapture'],env,runtime/'native-anvil.log');assert '1 passed; 0 failed'in text
  assert original_hashes=={str(p.relative_to(source)):digest(p)for p in original_files},'original world changed'
  report.update(anvil_scenes=8,anvil_tick_states=sum(len(s['states'])for s in vectors['scenes'][:8]),anvil_original_files_unchanged=True,anvil_source_files_sha256=original_hashes)
  run([cargo,'clippy','--lib','--','-D','warnings'],env,runtime/'clippy.log');report['clippy']=True
  names=['runtime_item_world','item_entity','item_damage','inventory','entity_base','entity_move','entity_state','runtime_collision','entity_support','fluid_physics','geometry','world','storage']
  report['native_sources_sha256']={name:digest(ROOT/'src'/(name+'.rs'))for name in names};report['ok']=True
 finally:
  (ROOT/'evidence/runtime-item-world-conformance.json').write_text(json.dumps(report,indent=2)+'\n')
 print(json.dumps(report,indent=2))
if __name__=='__main__':main()
