#!/usr/bin/env python3
import pathlib,subprocess,json,zipfile
r=pathlib.Path(__file__).resolve().parent.parent;o=r.parent/'mc121-oracle';j=pathlib.Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin');cp=':'.join([str(o/'server-inner-1.21.jar'),*[str(f) for f in sorted((o/'libraries').rglob('*.jar'))]])
subprocess.run([str(j/'javac'),'--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED','-proc:none','-cp',cp,'-d',str(r/'target/crafting-oracle'),str(r/'tools/crafting_oracle.java')],check=True)
p=subprocess.run([str(j/'java'),'--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED','-cp',str(r/'target/crafting-oracle')+':'+cp,'oracle.crafting_oracle',str(r/'fixtures/crafting-1.21.json')],input='{"op":"dyes"}\n{"op":"remainders"}\n{"op":"stew"}\n',text=True,capture_output=True,check=True)
out=[json.loads(l.split('ORACLE:',1)[1]) for l in p.stdout.splitlines() if 'ORACLE:' in l];assert len(out)==3,out
metadata={**out[0],**out[1],**out[2],'registries':json.loads(subprocess.check_output(['node',str(r/'tools/crafting_registry_extract.mjs')]))}
z=zipfile.ZipFile(o/'server-inner-1.21.jar');fixture=json.loads((r/'fixtures/crafting-1.21.json').read_text());ids=fixture['item_ids']
metadata['trim_material_items']={};metadata['trim_pattern_items']={}
for kind,field,outkey in [('trim_material','ingredient','trim_material_items'),('trim_pattern','template_item','trim_pattern_items')]:
 prefix='data/minecraft/'+kind+'/'
 for n in z.namelist():
  if n.startswith(prefix) and n.endswith('.json'):
   data=json.loads(z.read(n));metadata[outkey][str(ids[data[field]])]=metadata['registries']['minecraft:'+kind]['minecraft:'+n[len(prefix):-5]]
metadata['mob_effects']=json.loads((o/'reports/reports/registries.json').read_text())['minecraft:mob_effect']['entries']
(r/'tools/crafting_metadata.json').write_text(json.dumps(metadata,separators=(',',':'))+'\n')
print(json.dumps({'dyes':len(metadata['dyes']),'stew':metadata['stew'],'trim_materials':len(metadata['trim_material_items']),'trim_patterns':len(metadata['trim_pattern_items'])}))
