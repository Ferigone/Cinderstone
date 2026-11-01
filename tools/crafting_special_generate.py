#!/usr/bin/env python3
"""Differential special-recipe vectors from unchanged Java 1.21 recipe classes."""
import pathlib,subprocess,json,itertools,random
r=pathlib.Path(__file__).resolve().parent.parent;o=r.parent/'mc121-oracle';java='/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin/java';cp=':'.join([str(r/'target/crafting-oracle'),str(o/'server-inner-1.21.jar'),*[str(f) for f in sorted((o/'libraries').rglob('*.jar'))]])
book=json.loads((r/'fixtures/crafting-1.21.json').read_text());meta=json.loads((r/'tools/crafting_metadata.json').read_text());bytype={v['type']:k for k,v in book['recipes'].items()};vectors=[]
def s(name,count=1,components=None):return {'id':'minecraft:'+name,'count':count,**({'components':components} if components else {})}
def case(kind,name,items,width=3,height=3):
 if len(items)<width*height:items=items+[None]*(width*height-len(items))
 rid=bytype.get(kind,kind);vectors.append({'name':name,'recipe_id':rid,'query':{'op':'recipe','recipe':book['recipes'][rid]['raw'],'kind':'smithing' if kind=='smithing_trim' else 'crafting','width':width,'height':height,'items':items},'items':items,'width':width,'height':height})
colors=list(meta['dyes'])
for color in colors:
 for base in ['shulker_box','red_shulker_box']:case('crafting_special_shulkerboxcoloring',base+'/'+color,[s(base,1,{'minecraft:custom_name':'Kept'}),s(color+'_dye')],2,2)
 for count in [1,2,3]:case('crafting_special_firework_rocket','rocket/'+color+'/'+str(count),[s('paper')]+[s('gunpowder')]*count+[s('firework_star',1,{'minecraft:firework_explosion':{'shape':'small_ball','colors':[meta['dyes'][color]['firework']]}})])
 for shape in [None,'fire_charge','gold_nugget','feather','creeper_head','piglin_head']:
  for flags in [[],['diamond'],['glowstone_dust'],['diamond','glowstone_dust']]:case('crafting_special_firework_star','star/'+color+'/'+str(shape)+'/'+str(flags),[s('gunpowder'),s(color+'_dye')]+([s(shape)] if shape else [])+[s(f) for f in flags])
 for existing in [None,{'minecraft:dyed_color':{'rgb':0x002255,'show_in_tooltip':False}}]:case('crafting_special_armordye','armor/'+color+'/'+str(existing),[s('leather_chestplate',1,existing),s(color+'_dye')],2,2)
 case('crafting_special_firework_star_fade','fade/'+color,[s('firework_star',1,{'minecraft:firework_explosion':{'shape':'creeper','colors':[1,2],'has_trail':True}}),s(color+'_dye')],2,2)
 case('crafting_special_bannerduplicate','banner/'+color,[s(color+'_banner',1,{'minecraft:banner_patterns':[{'pattern':'minecraft:stripe_bottom','color':'red'}]}),s(color+'_banner')],2,2)
 case('crafting_special_shielddecoration','shield/'+color,[s('shield',1,{'minecraft:damage':7}),s(color+'_banner',1,{'minecraft:banner_patterns':[{'pattern':'minecraft:stripe_bottom','color':'white'}]})],2,2)
for amount in [1,2,8]:case('crafting_special_mapcloning','map/'+str(amount),[s('filled_map',1,{'minecraft:map_id':123})]+[s('map')]*amount)
for potion in [None,{'minecraft:potion_contents':{'potion':'minecraft:poison'}},{'!minecraft:potion_contents':{}}]:case('crafting_special_tippedarrow','arrow/'+str(potion),[s('arrow')]*4+[s('lingering_potion',1,potion)]+[s('arrow')]*4)
for flower in meta['stew']:case('crafting_special_suspiciousstew','stew/'+flower,[s('bowl'),s('brown_mushroom'),s('red_mushroom'),s(flower.removeprefix('minecraft:'))],2,2)
for n in [1,2,8]:
 for generation in [0,1,2,3]:case('crafting_special_bookcloning','book/'+str(n)+'/'+str(generation),[s('written_book',1,{'minecraft:written_book_content':{'title':'Title','author':'Alice','pages':['Page'],'generation':generation}})]+[s('writable_book')]*n)
for decorations in [['brick']*4,['angler_pottery_sherd','archer_pottery_sherd','arms_up_pottery_sherd','blade_pottery_sherd']]:case('crafting_decorated_pot','pot/'+str(decorations),[None,s(decorations[0]),None,s(decorations[1]),None,s(decorations[2]),None,s(decorations[3]),None])
for damage in [0,1,20,100,250,1560]:case('crafting_special_repairitem','repair/'+str(damage),[s('diamond_sword',1,{'minecraft:damage':damage}),s('diamond_sword',1,{'minecraft:damage':damage})],2,2)
for rid,recipe in book['recipes'].items():
 if recipe['type']=='smithing_trim':
  for addition in recipe['addition']:
   rev={v:k for k,v in book['item_ids'].items()};case('smithing_trim',rid+'/'+str(addition),[s(rev[recipe['template'][0]].removeprefix('minecraft:')),s('diamond_chestplate'),s(rev[addition].removeprefix('minecraft:'))],3,1);vectors[-1]['recipe_id']=rid;vectors[-1]['query']['recipe']=recipe['raw']
for item in ['stone','bow','diamond_sword']:
 for levels in [({'minecraft:binding_curse':1,'minecraft:sharpness':3},{'minecraft:vanishing_curse':2,'minecraft:unbreaking':3}),({'minecraft:binding_curse':2},{'minecraft:binding_curse':1})]:
  case('crafting_special_repairitem','repair/curses/'+item+'/'+str(levels),[s(item,1,{'minecraft:damage':10,'minecraft:custom_name':'Discarded','minecraft:enchantments':{'levels':levels[0]}}),s(item,1,{'minecraft:damage':20,'minecraft:enchantments':{'levels':levels[1]}})],2,2)
for tooltip in [False,True]:
 rid=bytype['smithing_trim'];recipe=book['recipes'][rid]
 template=next(k for k,i in book['item_ids'].items() if i==recipe['template'][0]);pattern=next(k for k,i in meta['registries']['minecraft:trim_pattern'].items() if i==meta['trim_pattern_items'][str(recipe['template'][0])])
 case('smithing_trim','trim/repeated/'+str(tooltip),[s(template.removeprefix('minecraft:')),s('diamond_chestplate',1,{'minecraft:custom_name':'Kept','minecraft:trim':{'material':'minecraft:iron','pattern':pattern,'show_in_tooltip':tooltip}}),s('iron_ingot')],3,1)
for scale in range(6):
 for exploration in [False,True]:
  for missing in [False,True]:
   case('crafting_special_mapextending','mapextend/'+str((scale,exploration,missing)),[s('paper')]*4+[s('filled_map',1,{'minecraft:map_id':123})]+[s('paper')]*4)
   vectors[-1]['map_context']={'scale':scale,'exploration':exploration,'missing':missing};vectors[-1]['query']['map_context']=vectors[-1]['map_context']
# Invalid duplicate/extra ingredients exercise negative paths for every supported special.
for kind in ['crafting_special_shulkerboxcoloring','crafting_special_mapcloning','crafting_decorated_pot','crafting_special_tippedarrow','crafting_special_bannerduplicate','crafting_special_shielddecoration','crafting_special_armordye','crafting_special_firework_rocket','crafting_special_firework_star','crafting_special_firework_star_fade','crafting_special_bookcloning','crafting_special_repairitem','crafting_special_suspiciousstew']:
 for cells in [[],[s('stone')],[s('stone')]*9]:case(kind,kind+'/invalid/'+str(len(cells)),cells)
p=subprocess.run([java,'--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED','-cp',cp,'oracle.crafting_oracle',str(r/'fixtures/crafting-1.21.json')],input='\n'.join(json.dumps(v['query']) for v in vectors)+'\n',capture_output=True,text=True,check=True)
results=[json.loads(l.split('ORACLE:',1)[1]) for l in p.stdout.splitlines() if 'ORACLE:' in l];assert len(results)==len(vectors),(len(results),len(vectors),p.stderr[-1000:])
errors=[];stacks=[]
for v,e in zip(vectors,results):
 del v['query'];v['expected']=e
 if 'error' in e:errors.append((v['name'],e['error']))
 v['input_indices']=list(range(len(stacks),len(stacks)+len(v['items'])));stacks+=v.pop('items')
 if e.get('matches'):
  e.setdefault('result',None)
  v['result_index']=len(stacks)
  if 'map_post_processing' in e:e['result'].setdefault('components',{})['minecraft:map_post_processing']=e['map_post_processing']
  stacks.append(e['result']);v['remaining_indices']=list(range(len(stacks),len(stacks)+len(e['remaining'])));stacks+=e['remaining']
wire=subprocess.run(['node',str(r/'tools/crafting_stack_wire.mjs')],input='\n'.join(map(json.dumps,stacks))+'\n',capture_output=True,text=True,check=True);encoded=[json.loads(l) for l in wire.stdout.splitlines()];assert len(encoded)==len(stacks),(len(encoded),len(stacks),wire.stderr)
wire_errors=[(i,s,x) for i,(s,x) in enumerate(zip(stacks,encoded)) if 'error'in x];assert not wire_errors,wire_errors[:3]
(r/'tools/crafting_special_vectors.json').write_text(json.dumps({'minecraft':'1.21','oracle':'unchanged actual Minecraft1.21 recipe classes + VanillaRegistries + bound official item tags','vectors':vectors,'stacks':[e['slot'] for e in encoded],'errors':errors},separators=(',',':'))+'\n')
print(json.dumps({'vectors':len(vectors),'stacks':len(stacks),'errors':errors[:10],'error_count':len(errors)}))
