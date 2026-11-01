#!/usr/bin/env python3
"""Generate compact actualJava Inventory.add state vectors; no native expectations."""
import base64,json,pathlib,random,subprocess
root=pathlib.Path(__file__).resolve().parent.parent
oracle=root.parent/'mc121-oracle';jdk=pathlib.Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
cp=':'.join([str(oracle/'server-inner-1.21.jar'),*[str(p) for p in sorted((oracle/'libraries').rglob('*.jar'))]])
exports=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED']
out=root/'target/player-inventory-oracle';out.mkdir(parents=True,exist_ok=True)
subprocess.run([str(jdk/'javac'),*exports,'-proc:none','-cp',cp,'-d',str(out),str(root/'tools/container_oracle.java'),str(root/'tools/player_inventory_oracle.java')],check=True)
def stack(name,count=1,components=None):
 s={'id':'minecraft:'+name,'count':count}
 if components:s['components']=components
 return s
catalog=json.loads((root/'fixtures/items-1.21.json').read_text())
cases=[]
for item in catalog['items'].values():
 if item['name']=='air':continue
 for creative in [False,True]:
  # Count99 covers ItemStack.CODEC overstack acceptance independently of its
  # max-stack component, including nonstackable undamaged tools and armor.
  cases.append({'name':item['name']+'/'+str(creative),'creative':creative,'selected':0,'slots':[None]*46,'pickup':stack(item['name'],99)})
variants=[None,stack('stone',1),stack('stone',64),stack('stone',99,{'minecraft:max_stack_size':99}),stack('stone',10,{'minecraft:custom_name':'"named"'}),stack('iron_pickaxe',99,{'minecraft:max_stack_size':99,'minecraft:damage':1}),stack('iron_pickaxe',99),stack('iron_pickaxe',50,{'minecraft:max_stack_size':99,'minecraft:damage':1}),stack('oak_planks',31),stack('snowball',16)]
rng=random.Random(7673953)
for index in range(420):
 creative=bool(index%2);slots=[None]*46
 mode=(index//2)%5
 for i in [*range(9,45),45]:
  if mode==0:slots[i]=stack('stone',64)
  elif mode==1:slots[i]=stack('stone',64)
  elif mode==2:slots[i]=rng.choice(variants)
  elif mode==3:slots[i]=stack('stone',99,{'minecraft:max_stack_size':99})
  else:slots[i]=rng.choice([None,None,None,*variants])
 if mode==1:slots[36+rng.randrange(9)]=stack('stone',60)
 cases.append({'name':'inventory'+str(index),'creative':creative,'selected':rng.randrange(9),'slots':slots,'pickup':variants[index%len(variants)]})
process=subprocess.run([str(jdk/'java'),*exports,'-cp',str(out)+':'+cp,'oracle.player_inventory_oracle'],input='\n'.join(map(json.dumps,cases))+'\n',capture_output=True,text=True,check=True)
results=[json.loads(line.split('ORACLE:',1)[1]) for line in process.stdout.splitlines() if 'ORACLE:' in line]
assert len(results)==len(cases),(len(results),len(cases),process.stderr[-1000:])
errors=[(c['name'],r['error']) for c,r in zip(cases,results) if 'error' in r]
assert not errors,errors[:10]
for case,result in zip(cases,results):case.update(result)
stacks=[];indices={}
def intern(value):
 key=json.dumps(value,sort_keys=True,separators=(',',':'))
 if key not in indices:indices[key]=len(stacks);stacks.append(value)
 return indices[key]
for c in cases:
 c['slots']=[intern(s) for s in c['slots']];c['pickup']=intern(c['pickup']);c['remaining']=intern(c['remaining']);c['state']=[intern(s) for s in c['state']['slots']]
wireprocess=subprocess.run(['node',str(root/'tools/crafting_stack_wire.mjs')],input='\n'.join(map(json.dumps,stacks))+'\n',capture_output=True,text=True,check=True)
wire=[json.loads(line) for line in wireprocess.stdout.splitlines()]
assert len(wire)==len(stacks)
assert all('slot' in s for s in wire),[s for s in wire if 'error' in s]
data={'minecraft':'1.21','oracle':'unchanged Inventory.add(ItemStack), environmental player hooks only','scope':'inventory stack placement, input remainder and acceptance; excludes item animation timers, playerTouch statistics, sounds and network timing','cases':cases,'stacks':[s['slot'] for s in wire],'wire_producer':'independent Prismarine corrected protocol767 Slot codec'}
(root/'fixtures/player_inventory_pickup.json').write_text(json.dumps(data,separators=(',',':'))+'\n')
print(json.dumps({'cases':len(cases),'item_types':len(catalog['items'])-1,'unique_stacks':len(stacks),'errors':0}))
