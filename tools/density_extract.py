#!/usr/bin/env python3
"""Extract the official 1.21 density definitions and seven noise routers."""
import argparse,hashlib,json,zipfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
PIN='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def main():
 p=argparse.ArgumentParser();p.add_argument('--jar',type=Path,default=ROOT.parent/'mc121-oracle/server-inner-1.21.jar');args=p.parse_args()
 if hashlib.sha256(args.jar.read_bytes()).hexdigest()!=PIN:raise RuntimeError('Wrong official Minecraft1.21 artifact')
 definitions={};settings={};sources={}
 with zipfile.ZipFile(args.jar)as jar:
  for path in sorted(jar.namelist()):
   if not path.endswith('.json'):continue
   for prefix,kind in [('data/minecraft/worldgen/density_function/','function'),('data/minecraft/worldgen/noise_settings/','settings')]:
    if not path.startswith(prefix):continue
    raw=jar.read(path);value=json.loads(raw);name='minecraft:'+path[len(prefix):-5];sources[name+':'+kind]=hashlib.sha256(raw).hexdigest()
    if kind=='function':definitions[name]=value
    else:settings[name]={'legacy_random_source':value['legacy_random_source'],'noise':value['noise'],'noise_router':value['noise_router']}
 result={'minecraft':'1.21','source_inner_sha256':PIN,'definitions':definitions,'settings':settings,'source_hashes':sources}
 path=ROOT/'fixtures/density_builtin-1.21.json';path.write_text(json.dumps(result,indent=2)+'\n')
 print(json.dumps({'definitions':len(definitions),'routers':len(settings),'bytes':path.stat().st_size,'output':str(path)}))
if __name__=='__main__':main()
