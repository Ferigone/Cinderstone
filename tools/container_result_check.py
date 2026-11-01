#!/usr/bin/env python3
import pathlib,subprocess,json
r=pathlib.Path(__file__).resolve().parent.parent;o=r.parent/'mc121-oracle';j=pathlib.Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin');cp=':'.join([str(o/'server-inner-1.21.jar'),*[str(f) for f in sorted((o/'libraries').rglob('*.jar'))]])
exports=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED'];outdir=r/'target/container-result-oracle';outdir.mkdir(parents=True,exist_ok=True)
subprocess.run([str(j/'javac'),*exports,'-proc:none','-cp',cp,'-d',str(outdir),str(r/'tools/container_oracle.java'),str(r/'tools/container_result_oracle.java')],check=True)
book=json.loads((r/'fixtures/crafting-1.21.json').read_text());reverse={v:k for k,v in book['item_ids'].items()};cases=[]
def s(n,c=1,components=None):return {'id':'minecraft:'+n,'count':c,**({'components':components} if components else {})}
def resolve(v):
 if isinstance(v,dict):
  if 'tag'in v:return [{'item':reverse[i]} for i in book['tags'][v['tag'].removeprefix('minecraft:')]]
  return {k:resolve(x) for k,x in v.items()}
 if isinstance(v,list):return [resolve(x) for x in v]
 return v
def case(name,recipe,grid,clicks,inventory=None,carried=None):
 slots=[None]*46;slots[1:5]=grid
 for index,value in (inventory or {}).items():slots[index]=value
 cases.append({'name':name,'recipe_id':'minecraft:'+recipe,'recipe':resolve(book['recipes']['minecraft:'+recipe]['raw']),'creative':False,'slots':slots,'carried':carried,'clicks':[{'slot':slot,'button':button,'type':kind} for slot,button,kind in clicks]})
for count in [1,2,16,64]:
 for button in [0,1]:case('planks/pickup/'+str(count)+'/'+str(button),'oak_planks',[s('oak_log',count),None,None,None],[(0,button,'PICKUP')])
 case('planks/shift/'+str(count),'oak_planks',[s('oak_log',count),None,None,None],[(0,0,'QUICK_MOVE')])
 case('planks/throw/'+str(count),'oak_planks',[s('oak_log',count),None,None,None],[(0,0,'THROW')])
 case('planks/swap/'+str(count),'oak_planks',[s('oak_log',count),None,None,None],[(0,0,'SWAP')])
case('sticks/translated','stick',[None,s('oak_planks',3),None,s('oak_planks',3)],[(0,0,'QUICK_MOVE')])
case('honey/remainders-empty-grid','honey_block',[s('honey_bottle')]*4,[(0,0,'PICKUP')])
case('honey/remainders-to-inventory','honey_block',[s('honey_bottle',2)]*4,[(0,0,'PICKUP')])
full={i:s('stone',64) for i in range(9,45)}
case('planks/partial-shift-fit-drop','oak_planks',[s('oak_log',2),None,None,None],[(0,0,'QUICK_MOVE')],{**full,44:s('oak_planks',63)})
case('planks/no-shift-capacity','oak_planks',[s('oak_log',2),None,None,None],[(0,0,'QUICK_MOVE')],full)
case('planks/cursor-insufficient-capacity','oak_planks',[s('oak_log',2),None,None,None],[(0,0,'PICKUP')],carried=s('oak_planks',63))
case('planks/cursor-merge-full-output','oak_planks',[s('oak_log',2),None,None,None],[(0,1,'PICKUP')],carried=s('oak_planks',60))
case('honey/remainders-full-inventory-drop','honey_block',[s('honey_bottle',2)]*4,[(0,0,'PICKUP')],full)
# Component-preserving recipes with original items returned as remainders.
book_id=next(k.removeprefix('minecraft:') for k,v in book['recipes'].items() if v['type']=='crafting_special_bookcloning')
case('book/original-returned',book_id,[s('written_book',1,{'minecraft:written_book_content':{'title':'Title','author':'Alice','pages':['Page'],'generation':0}}),s('writable_book',2),None,None],[(0,0,'PICKUP')])
banner_id=next(k.removeprefix('minecraft:') for k,v in book['recipes'].items() if v['type']=='crafting_special_bannerduplicate')
case('banner/original-returned',banner_id,[s('red_banner',1,{'minecraft:banner_patterns':[{'pattern':'minecraft:stripe_bottom','color':'white'}]}),s('red_banner',2),None,None],[(0,0,'PICKUP')])
p=subprocess.run([str(j/'java'),*exports,'-cp',str(outdir)+':'+cp,'oracle.container_result_oracle'],input='\n'.join(map(json.dumps,cases))+'\n',capture_output=True,text=True,check=True)
outputs=[json.loads(line.split('ORACLE:',1)[1]) for line in p.stdout.splitlines() if 'ORACLE:'in line];assert len(outputs)==len(cases),(len(outputs),len(cases),p.stdout[-1000:],p.stderr[-1000:]);errors=[]
for case,out in zip(cases,outputs):
 if 'error'in out:errors.append((case['name'],out['error']))
 case.update(out);del case['recipe']
data={'minecraft':'1.21','scope':'actual cpu/cqw/crm result clicks+RecipeManager/Recipe; environment recomputation, awards/network omitted','cases':cases};(r/'tools/container_result_oracle_vectors.json').write_text(json.dumps(data,separators=(',',':'))+'\n');print(json.dumps({'cases':len(cases),'errors':errors}))
