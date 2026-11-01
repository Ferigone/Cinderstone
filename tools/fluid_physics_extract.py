#!/usr/bin/env python3
"""Extract exact original1.21 immutable fluid facts; retain unknown support faces."""
from pathlib import Path
import hashlib,json,os,subprocess

ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle'
JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'

def main():
 jar=ORACLE/'server-inner-1.21.jar';assert hashlib.sha256(jar.read_bytes()).hexdigest()==SHA
 runtime=ROOT/'runtime/fluid-physics-facts';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True)
 cp=os.pathsep.join(map(str,[classes,jar,*sorted((ORACLE/'libraries').rglob('*.jar'))]))
 source=ROOT/'tools/fluid_physics_facts.java';output=ROOT/'fixtures/fluid_physics_facts.json'
 compile=subprocess.run([JDK/'javac','-proc:none','-cp',cp,'-d',classes,source],capture_output=True,text=True)
 if compile.returncode:raise RuntimeError(compile.stdout+compile.stderr)
 java=subprocess.run([JDK/'java','-Xmx1G','-cp',cp,'oracle.fluid_physics_facts',output],capture_output=True,text=True)
 (runtime/'oracle.log').write_text(java.stdout+java.stderr)
 if java.returncode:raise RuntimeError((java.stdout+java.stderr)[-5000:])
 data=json.loads(output.read_text());assert len(data['states'])==26684
 previous=json.loads((ROOT/'tools/item_entity_physics_facts.json').read_text());assert len(previous['states'])==26684
 for state,index in enumerate(data['states']):
  row=data['rows'][index];prior=previous['rows'][previous['states'][state]]
  assert row[0]==prior[1]and row[1]==prior[2],(state,row,prior)
  assert row[8]or row[7]==0,('unknown face bits must not be guessed',state,row)
 print(json.dumps(dict(minecraft='1.21',oracle_inner_sha256=SHA,states=len(data['states']),deduplicated_rows=len(data['rows']),family_state_counts=data['family_state_counts'],cached_face_states=data['cached_face_states'],uncached_face_states=data['uncached_face_states'],ice_block_states=data['ice_block_states'],uncached_blocks=data['uncached_blocks'],uncached_support_classes=data['uncached_support_classes'],existing_item_fluid_fact_comparisons=26684*2,differences=0,fixture_sha256=hashlib.sha256(output.read_bytes()).hexdigest(),oracle_source_sha256=hashlib.sha256(source.read_bytes()).hexdigest()),indent=2))

if __name__=='__main__':main()
