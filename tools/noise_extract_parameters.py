#!/usr/bin/env python3
"""Extract exact built-in noise parameter data from the pinned 1.21 artifact."""
import argparse,hashlib,json,zipfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
PIN='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def main():
 p=argparse.ArgumentParser();p.add_argument('--jar',type=Path,default=ROOT.parent/'mc121-oracle/server-inner-1.21.jar');args=p.parse_args()
 if hashlib.sha256(args.jar.read_bytes()).hexdigest()!=PIN:raise RuntimeError('Wrong Minecraft1.21 JAR')
 noises={}
 with zipfile.ZipFile(args.jar)as jar:
  prefix='data/minecraft/worldgen/noise/'
  for path in sorted(jar.namelist()):
   if not path.startswith(prefix)or not path.endswith('.json'):continue
   raw=jar.read(path);data=json.loads(raw)
   noises['minecraft:'+path[len(prefix):-5]]={'first_octave':data['firstOctave'],'amplitudes':data['amplitudes'],'source_sha256':hashlib.sha256(raw).hexdigest()}
 result={'minecraft':'1.21','protocol':767,'source_inner_sha256':PIN,'noises':noises}
 target=ROOT/'fixtures/noise_parameters-1.21.json';target.write_text(json.dumps(result,indent=2)+'\n')
 print(json.dumps({'entries':len(noises),'output':str(target),'sha256':hashlib.sha256(target.read_bytes()).hexdigest()}))
if __name__=='__main__':main()
