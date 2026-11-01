#!/usr/bin/env python3
"""Run stock item clients against the pinned Java server in a private world copy."""
from pathlib import Path
import argparse,hashlib,json,shutil,subprocess,tempfile,uuid
ROOT=Path(__file__).resolve().parents[1]
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
 p=argparse.ArgumentParser();p.add_argument('--port',type=int,default=25781);p.add_argument('--label',default='java_stock_reference');a=p.parse_args()
 assert a.label.replace('_','').replace('-','').isalnum()
 source=ROOT.parent/'mc121-oracle';world=source/'world'
 assert hashlib.sha1((source/'server-1.21.jar').read_bytes()).hexdigest()=='450698d1863ab5180c25d7c804ef0fe6369dd1ba'
 paths=sorted(path for path in world.rglob('*')if path.is_file());before={str(path.relative_to(world)):digest(path)for path in paths}
 directory=Path(tempfile.mkdtemp(prefix='java-item-stock-',dir=ROOT/'runtime'));shutil.copytree(world,directory/'world',copy_function=shutil.copyfile);shutil.copyfile(source/'eula.txt',directory/'eula.txt')
 updates={'online-mode':'false','gamemode':'creative','force-gamemode':'true','view-distance':'2','simulation-distance':'2','spawn-protection':'0','max-players':'20','level-name':'world','enforce-secure-profile':'false'};lines=[]
 for line in(source/'server.properties').read_text().splitlines():
  key=line.split('=',1)[0]
  if key in updates:line=key+'='+updates.pop(key)
  lines.append(line)
 lines += [key+'='+value for key,value in updates.items()];(directory/'server.properties').write_text('\n'.join(lines)+'\n')
 frozen=ROOT/'runtime'/('item-live-probe-frozen-'+uuid.uuid4().hex+'.mjs');shutil.copyfile(ROOT/'tools/item_live_probe.mjs',frozen)
 cmd=['node',str(frozen),'--scenario','stock','--java-jar',str(source/'server-1.21.jar'),'--java-server-dir',str(directory),'--label',a.label,'--port',str(a.port)]
 result=subprocess.run(cmd,cwd=ROOT,text=True,capture_output=True,timeout=240)
 log=directory/'probe.log';temp=log.with_name(log.name+'.partial');temp.write_text(result.stdout+result.stderr);temp.replace(log)
 assert before=={str(path.relative_to(world)):digest(path)for path in paths},'reference world changed'
 report={'ok':result.returncode==0,'kind':'live stock item probe on original Java server','minecraft':'1.21','probe_source_sha256':digest(frozen),'reference_world_unchanged':True,'runtime_directory':str(directory),'exit_code':result.returncode,'scope':'Two stock protocol clients; creative/Q predictions are separate from forced authoritative menu readback. Stock item creation, motion, pairing and pickup. Controlled merge is a separate scenario.','not_verified':['Native Rust runtime item pipeline','Controlled merge and exact cross-process random trajectory equality']}
 dest=ROOT/'evidence'/f'item-live-{a.label}-wrapper.json';temp=dest.with_name(dest.name+'.partial');temp.write_text(json.dumps(report,indent=2)+'\n');temp.replace(dest)
 print(json.dumps(report));print((result.stdout+result.stderr)[-6500:]);raise SystemExit(result.returncode)
if __name__=='__main__':main()
