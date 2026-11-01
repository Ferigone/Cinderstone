#!/usr/bin/env python3
import hashlib,json,os,subprocess,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle';JAVA=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin');JAR=ORACLE/'server-inner-1.21.jar';SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
assert hashlib.sha256(JAR.read_bytes()).hexdigest()==SHA
classpath=os.pathsep.join([str(JAR),*map(str,sorted((ORACLE/'libraries').rglob('*.jar')))])
from zipfile import ZipFile
blocks=json.loads((ROOT/'fixtures/items-1.21.json').read_text())['blocks']
with ZipFile(JAR)as jar:
 files={name:jar.read(name)for name in jar.namelist()if name.startswith('data/minecraft/tags/block/')and name.endswith('.json')}
 raw={'minecraft:'+name[len('data/minecraft/tags/block/'):-5]:json.loads(data)for name,data in files.items()}
 def resolve(name,stack=()):
  if name in stack:raise ValueError('cyclic block tag '+name)
  result=[]
  for value in raw[name]['values']:
   required=True
   if isinstance(value,dict):required=value.get('required',True);value=value['id']
   if value.startswith('#'):
    if value[1:]not in raw:
     if required:raise ValueError(value)
     continue
    nested=resolve(value[1:],(*stack,name))
   else:
    short=value.removeprefix('minecraft:')
    if short not in blocks:
     if required:raise ValueError(value)
     continue
    nested=[blocks[short]['default_state']]
   for state in nested:
    if state not in result:result.append(state)
  return result
 tags={name:resolve(name)for name in raw}
 tagdata={'minecraft':'1.21','jar_sha256':SHA,'source_files':{name:hashlib.sha256(data).hexdigest()for name,data in files.items()},'tags':tags}
 (ROOT/'fixtures/placement-block-tags-1.21.json').write_text(json.dumps(tagdata,separators=(',',':'))+'\n')

with tempfile.TemporaryDirectory(prefix='mc121-placement-')as temp:
 subprocess.run([str(JAVA/'javac'),'-proc:none','-cp',classpath,'-d',temp,str(ROOT/'tools/placement_catalog.java'),str(ROOT/'tools/placement_tags.java')],check=True)
 r=subprocess.run([str(JAVA/'java'),'-cp',temp+os.pathsep+classpath,'oracle.PlacementCatalog',temp+'/catalog.json',str(ROOT/'fixtures/placement-block-tags-1.21.json')],capture_output=True,text=True)
 if r.returncode:raise RuntimeError(r.stderr+r.stdout[-5000:])
 data=json.loads(Path(temp+'/catalog.json').read_text())
(ROOT/'tools/placement_catalog.json').write_text(json.dumps(data,indent=2)+'\n')
(ROOT/'fixtures/placement-1.21.json').write_text(json.dumps(data,separators=(',',':'))+'\n')
print(json.dumps({'blocks':len(data['blocks']),'states':len(data['fluids']),'sha256':hashlib.sha256((ROOT/'fixtures/placement-1.21.json').read_bytes()).hexdigest()}))
