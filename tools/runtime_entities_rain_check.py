#!/usr/bin/env python3
"""Native coordinator context bridge against actual unchanged rainy ItemEntity."""
from pathlib import Path
import argparse,hashlib,json,os,re,subprocess
ROOT=Path(__file__).resolve().parents[1]
def sha(name):return hashlib.sha256((ROOT/name).read_bytes()).hexdigest()
def main():
 p=argparse.ArgumentParser();p.add_argument('--release',action='store_true');a=p.parse_args()
 oracle=json.loads((ROOT/'evidence/item-rain-oracle.json').read_text());assert oracle['ok'] and not oracle['original_classes_modified']
 assert (oracle['scenes'],oracle['states'],oracle['sound_events'])==(8,80,6)
 assert sha('fixtures/item-rain-vectors.json')==oracle['fixture_sha256']
 for name,digest in oracle['source_sha256'].items():assert sha(name)==digest,name
 names=['src/runtime_entities.rs','tools/runtime_entities_rain_tests.rs','tools/runtime_entities_tests.rs','tools/runtime_entities_rain_check.py','tools/runtime_entities_rain_audit.md','src/runtime_item_world.rs','src/rain_physics.rs','src/rain_fixture.rs','src/random.rs','src/fluid_physics.rs','src/item_entity.rs','src/entity_state.rs','src/entity_base.rs','src/entity_move.rs','src/tick.rs','fixtures/item-rain-vectors.json','fixtures/item-full-vectors.json','fixtures/item-water-vectors.json','evidence/item-rain-oracle.json']
 before={name:sha(name)for name in names};env=os.environ.copy();env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup',CARGO_INCREMENTAL='0')
 profile='release'if a.release else'debug';command=['/root/.local/share/mc-rust-toolchain/cargo/bin/cargo','test','--locked','--offline','--lib']+(['--release']if a.release else[])+['runtime_entities','--','--nocapture']
 process=subprocess.run(command,cwd=ROOT,env=env,text=True,capture_output=True);output=process.stdout+process.stderr;directory=ROOT/'runtime/runtime-entities';directory.mkdir(exist_ok=True);log=directory/('context-'+profile+'.log');log.write_text(output)
 assert before=={name:sha(name)for name in names},'source changed during context proof'
 if process.returncode:raise RuntimeError(output[-9000:])
 assert re.search(r'test result: ok\. 11 passed; 0 failed;',output),output[-6000:]
 assert 'RUNTIME_ENTITIES_RAIN_CONFORMANCE:80 original states,6 sounds,3 independent RNG streams; zero differences'in output
 for marker in ['RUNTIME_ENTITIES_DRY_CONFORMANCE','RUNTIME_ENTITIES_WATER_CONFORMANCE']:
  values=[json.loads(line.split(':',1)[1])for line in output.splitlines()if line.startswith(marker+':')];assert len(values)==1 and values[0]['ok'] and values[0]['mismatches']==0
 dest=ROOT/'evidence/runtime-entities-rain-conformance.json';profiles={}
 if dest.exists():
  old=json.loads(dest.read_text())
  if old.get('source_sha256')==before:profiles=old['profiles']
 profiles[profile]=dict(tests_passed=11,new_context_tests=4,legacy_coordinator_tests=5,legacy_original_corpus_tests=2,differences=0,cargo_target_directory=env.get('CARGO_TARGET_DIR','default'),log_sha256=hashlib.sha256(log.read_bytes()).hexdigest())
 report=dict(ok=True,minecraft='1.21',oracle_inner_sha256=oracle['oracle_inner_sha256'],profiles=profiles,original_rain_scenes=8,original_rain_states=80,original_sound_events=6,old_dry_submerged_states=2976,old_final_entity_rng_states=82,source_sha256=before,source_stability_verified=True,scope=['Complete actual original rainy ItemEntity states and ordered effect facts through authoritative storage/index/tick/provider composition','Separate entity RNG, Level.random and actual Level.threadSafeRandom sound stream; correct f32 pitch and f-stream nextLong order','Two-item context/RNG lifetime and callback-order composition using actual scalar dzr nextLong vectors','Both optional capabilities exercised together: first shallow fluid-shape history reused for second above-water site, while rain extinguishes both items','Absent rain/sound capabilities fail explicitly before invented host facts or Level.random sound draws'],not_verified=['The two-item composition checks are native integration, not an original two-item world-scene proof','Controlled edited-water test does not certify genuine rain exposure in edited worlds','Actual live Main/network sound delivery, listeners, general imported worlds and complete weather effects','Concurrent ThreadSafeLegacyRandomSource CAS semantics; this stream has one serialized host owner','Whole Minecraft server1:1 compatibility'],reproduce='python3 tools/runtime_entities_rain_check.py; CARGO_TARGET_DIR=runtime/post-v5-proof-target python3 tools/runtime_entities_rain_check.py --release')
 dest.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:v for k,v in report.items()if k not in ['source_sha256','scope','not_verified']},indent=2))
if __name__=='__main__':main()
