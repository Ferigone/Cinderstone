#!/usr/bin/env python3
"""Check captured dimension fields and their native weather/rule composition."""
from pathlib import Path
import argparse,hashlib,json,os,re,subprocess,uuid
ROOT=Path(__file__).resolve().parents[1]
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
 p=argparse.ArgumentParser();p.add_argument('--release',action='store_true');p.add_argument('--regenerate',action='store_true');a=p.parse_args()
 if a.regenerate:subprocess.run(['node','tools/world_context_extract.mjs'],cwd=ROOT,check=True,capture_output=True)
 original=json.loads((ROOT/'fixtures/dimension-context-vectors.json').read_text())
 assert original['configuration_sha256']==digest(ROOT/'fixtures/configuration.json')
 env=os.environ.copy();env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup',CARGO_INCREMENTAL='0')
 reports=[]
 for profile in (['debug','release']if a.release else['debug']):
  tests={}
  for name,count in [('world_context::',3),('level::',4),('nbt::',13)]:
   cmd=['/root/.local/share/mc-rust-toolchain/cargo/bin/cargo','test','--locked','--lib',name]
   if profile=='release':cmd+=['--release']
   result=subprocess.run(cmd,cwd=ROOT,env=env,text=True,capture_output=True)
   log=ROOT/'runtime/world-context'/f'{profile}-{name.removesuffix("::")}.log';log.parent.mkdir(parents=True,exist_ok=True)
   temp=log.with_name(log.name+'.'+uuid.uuid4().hex);temp.write_text(result.stdout+result.stderr);temp.replace(log)
   if result.returncode:raise RuntimeError((result.stdout+result.stderr)[-6000:])
   passed=int(re.search(r'test result: ok\. (\d+) passed',result.stdout).group(1));assert passed==count,(name,passed,count);tests[name]=passed
  reports.append({'profile':profile,'tests':tests})
 sources=['src/world_context.rs','src/nbt.rs','src/level.rs','src/world_rules.rs','src/tick.rs','fixtures/configuration.json','fixtures/dimension-context-vectors.json','tools/world_context_extract.mjs','tools/world_context_check.py']
 report={'ok':True,'minecraft':'1.21','kind':'native composition against pinned primary registry packet and independent wire decoder','dimension_types':len(original['dimensions']),'runs':reports,'source_sha256':{s:digest(ROOT/s)for s in sources},'scope':'Exact captured dimension fields, bounded embedded NBT prefix, original gamerule semantics in level clock, weather/RNG composition and explicit unavailable rain exposure. Existing NBT/level regressions included.','not_verified':['Complete world-tick order and all world RNG consumption','Rain sky/biome/precipitation exposure','Arbitrary custom registry numeric representations and world layouts','Live chunk/lifecycle and item gameplay integration']}
 dest=ROOT/'evidence/world-context-conformance.json';temp=dest.with_name(dest.name+'.'+uuid.uuid4().hex);temp.write_text(json.dumps(report,indent=2)+'\n');temp.replace(dest)
 print(json.dumps({k:v for k,v in report.items()if k not in ['source_sha256','scope','not_verified']}))
if __name__=='__main__':main()
