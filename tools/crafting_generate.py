#!/usr/bin/env python3
"""Extract exact 1.21 recipes/tags; call the unchanged Mojang classes for vectors."""
import argparse, hashlib, json, pathlib, subprocess, zipfile, random
ROOT=pathlib.Path(__file__).resolve().parent.parent
ORACLE=ROOT.parent/'mc121-oracle'
JAVA=pathlib.Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')

def main():
 p=argparse.ArgumentParser();p.add_argument('--no-oracle',action='store_true');args=p.parse_args()
 jar=ORACLE/'server-inner-1.21.jar'; z=zipfile.ZipFile(jar)
 items=json.loads((ROOT/'fixtures/items-1.21.json').read_text())['items']
 ids={'minecraft:'+v['name']:int(k) for k,v in items.items()}
 tags={n[len('data/minecraft/tags/item/'):-5]:json.loads(z.read(n))['values'] for n in z.namelist() if n.startswith('data/minecraft/tags/item/') and n.endswith('.json')}
 expanded={}
 def tag(name,active=()):
  name=name.removeprefix('minecraft:')
  if name in expanded:return expanded[name]
  assert name not in active,(name,active)
  out=set()
  for v in tags[name]:
   if isinstance(v,dict):v=v['id']
   out.update(tag(v[1:],active+(name,)) if v.startswith('#') else [ids[v]])
  expanded[name]=sorted(out);return expanded[name]
 for k in tags:tag(k)
 def ingredient(v):
  if isinstance(v,list):return sorted({i for x in v for i in ingredient(x)})
  if 'item' in v:return [ids[v['item']]]
  return tag(v['tag'])
 recipes={}
 for n in sorted(z.namelist()):
  if not n.startswith('data/minecraft/recipe/') or not n.endswith('.json'):continue
  rid='minecraft:'+n[len('data/minecraft/recipe/'):-5];raw=json.loads(z.read(n));r={'type':raw['type'].removeprefix('minecraft:'),'raw':raw}
  if 'key' in raw:
   r['key']={k:ingredient(v) for k,v in raw['key'].items()}
   pattern=raw['pattern']; occupied=[(x,y) for y,row in enumerate(pattern) for x,c in enumerate(row) if c!=' ']
   x0=min(x for x,y in occupied);x1=max(x for x,y in occupied)+1;y0=min(y for x,y in occupied);y1=max(y for x,y in occupied)+1
   r['pattern']=[row[x0:x1] for row in pattern[y0:y1]]
  for field in ['ingredient','template','base','addition']:
   if field in raw:r[field]=ingredient(raw[field])
  if 'ingredients' in raw:r['ingredients']=[ingredient(v) for v in raw['ingredients']]
  if 'result' in raw:r['result']={'item_id':ids[raw['result']['id']],'count':raw['result'].get('count',1)}
  for field in ['experience','cookingtime']:
   if field in raw:r[field]=raw[field]
  recipes[rid]=r
 remainders={'minecraft:milk_bucket':'minecraft:bucket','minecraft:water_bucket':'minecraft:bucket','minecraft:lava_bucket':'minecraft:bucket','minecraft:dragon_breath':'minecraft:glass_bottle','minecraft:honey_bottle':'minecraft:glass_bottle'}
 fixture={'minecraft':'1.21','protocol':767,'source':{'jar_sha256':hashlib.sha256(jar.read_bytes()).hexdigest(),'recipes':'data/minecraft/recipe/*.json','tags':'data/minecraft/tags/item/*.json'},'item_ids':ids,'tags':expanded,'remainders':{str(ids[k]):ids[v] for k,v in remainders.items()},'recipes':recipes}
 (ROOT/'fixtures/crafting-1.21.json').write_text(json.dumps(fixture,separators=(',',':'))+'\n')
 if args.no_oracle:print(json.dumps({'recipes':len(recipes),'tags':len(tags)}));return
 cp=':'.join([str(jar),*[str(f) for f in sorted((ORACLE/'libraries').rglob('*.jar'))]])
 out=ROOT/'target/crafting-oracle';out.mkdir(parents=True,exist_ok=True)
 subprocess.run([str(JAVA/'javac'),'--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED','-cp',cp,'-d',str(out),str(ROOT/'tools/crafting_oracle.java')],check=True)
 def stack(i,count=1):return {'id':next(k for k,v in ids.items() if v==i),'count':count} if i else None
 def resolve(v):
  if isinstance(v,dict):
   if 'tag' in v:return [{'item':next(k for k,i in ids.items() if i==n)} for n in ingredient(v)]
   return {k:resolve(x) for k,x in v.items()}
  if isinstance(v,list):return [resolve(x) for x in v]
  return v
 vectors=[];rng=random.Random(767)
 for rid,r in recipes.items():
  typ=r['type'];kind='crafting';width=height=0;cells=[]
  if typ=='crafting_shaped':
   width=len(r['pattern'][0]);height=len(r['pattern']);cells=[r['key'][c][0] if c!=' ' else 0 for row in r['pattern'] for c in row]
  elif typ=='crafting_shapeless':
   width=3;height=3;cells=[v[0] for v in r['ingredients']];cells +=[0]*(9-len(cells));rng.shuffle(cells)
  elif typ in ['smelting','smoking','blasting','campfire_cooking','stonecutting']:
   kind='single';cells=[r['ingredient'][0]]
  elif typ=='smithing_transform':kind='smithing';cells=[r[k][0] for k in ['template','base','addition']]
  else:continue
  raw=resolve(r['raw'])
  for name,slots in [('valid',cells),('wrong',([0]*len(cells)))]:
   q={'op':'recipe','recipe':raw,'kind':kind,'width':width,'height':height,'items':[stack(i) for i in slots]}
   vectors.append({'name':rid+'/'+name,'recipe_id':rid,'input':{'kind':kind,'width':width,'height':height,'items':[{'item_id':i,'count':1 if i else 0} for i in slots]},'query':q})
  if typ=='crafting_shaped':
   mirrored=[cells[y*width+x] for y in range(height) for x in reversed(range(width))]
   vectors.append({'name':rid+'/mirrored','recipe_id':rid,'input':{'kind':kind,'width':width,'height':height,'items':[{'item_id':i,'count':1 if i else 0} for i in mirrored]},'query':{'op':'recipe','recipe':raw,'kind':kind,'width':width,'height':height,'items':[stack(i) for i in mirrored]}})
 proc=subprocess.run([str(JAVA/'java'),'--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED','-cp',str(out)+':'+cp,'oracle.crafting_oracle'],input='\n'.join(json.dumps(v['query']) for v in vectors)+'\n',capture_output=True,text=True,check=True)
 outputs=[json.loads(line.split('ORACLE:',1)[1]) for line in proc.stdout.splitlines() if 'ORACLE:' in line]
 assert len(outputs)==len(vectors),(len(outputs),len(vectors),proc.stderr[-1000:])
 errors=[]
 for v,result in zip(vectors,outputs):
  del v['query'];v['expected']=result
  if 'error' in result:errors.append((v['name'],result['error']))
 result={'minecraft':'1.21','oracle':'unchanged actual Minecraft1.21 Recipe classes, reflected names; tag ingredients expanded from exact official data','jar_sha256':fixture['source']['jar_sha256'],'vectors':vectors,'errors':errors}
 (ROOT/'tools/crafting_vectors.json').write_text(json.dumps(result,separators=(',',':'))+'\n')
 print(json.dumps({'recipes':len(recipes),'tags':len(tags),'vectors':len(vectors),'errors':errors[:10],'error_count':len(errors)}))
if __name__=='__main__':main()
