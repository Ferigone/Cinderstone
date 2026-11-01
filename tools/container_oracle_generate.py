#!/usr/bin/env python3
"""Deterministic inputs; expectedresults come only from the originalJava menu."""
import json,random
from pathlib import Path
R=random.Random(0x121767)
CASES=[]
def stack(name,count=1,components=None):
    s={'id':'minecraft:'+name,'count':count}
    if components:s['components']=components
    return s

def case(name,items=None,carried=None,creative=False,clicks=()):
    slots=[None]*46
    for i,item in (items or {}).items():slots[i]=item
    c={'name':name,'creative':creative,'crafting_disabled':True,'slots':slots,'carried':carried,'clicks':list(clicks)};CASES.append(c);return c

def click(slot,button=0,kind='PICKUP'):return {'slot':slot,'button':button,'type':kind}
def drag(slots,mode=0):return [click(-999,mode<<2,'QUICK_CRAFT'),*[click(i,1|(mode<<2),'QUICK_CRAFT') for i in slots],click(-999,2|(mode<<2),'QUICK_CRAFT')]
stone=stack('stone',17)
case('left-right-pickup-merge-outside',{9:stone,10:stack('stone',60),11:stack('dirt',8)},clicks=[click(9,1),click(10),click(11),click(12,1),click(-999,1),click(-999),click(9),click(10)])
for count in [1,2,3,15,16,17,63,64]:
 for button in [0,1]:
  case(f'pickup-count-{count}-{button}',{9:stack('stone',count),10:stack('stone',63)},clicks=[click(9,button),click(10,button),click(11,button),click(-999,button)])
for name in ['stone','ender_pearl','iron_sword','diamond_helmet','diamond_chestplate','diamond_leggings','diamond_boots','elytra','shield','carved_pumpkin','skeleton_skull','wolf_armor']:
 case('quickmove-'+name,{9:stack(name,1)},clicks=[click(9,0,'QUICK_MOVE'),click(36,0,'QUICK_MOVE'),click(5,0,'QUICK_MOVE'),click(6,0,'QUICK_MOVE'),click(45,0,'QUICK_MOVE')])
case('quickmove-merge-first',{9:stack('stone',50),36:stack('stone',60),37:stack('dirt',64),38:stack('stone',63)},clicks=[click(9,0,'QUICK_MOVE'),click(36,0,'QUICK_MOVE')])
for creative in [False,True]:
 for armor,slot in [('diamond_helmet',5),('diamond_chestplate',6),('diamond_leggings',7),('diamond_boots',8)]:
  item=stack(armor,1,{'minecraft:enchantments':{'levels':{'minecraft:binding_curse':1}}})
  case(f'binding-{armor}-{creative}',{slot:item,36:stack('stone',12)},creative=creative,clicks=[click(slot),click(slot,0,'QUICK_MOVE'),click(slot,0,'SWAP'),click(slot,1,'THROW'),click(slot,0,'CLONE')])
for target in [5,6,7,8,9,36,45]:
 for button in [0,1,8,40,9,39]:
  case(f'swap-{target}-{button}',{target:stack('stone',32),36:stack('dirt',20),37:stack('iron_sword'),44:stack('ender_pearl',16),45:stack('diamond_helmet')},clicks=[click(target,button,'SWAP')])
for creative in [False,True]:
 for carried in [None,stack('dirt',3)]:
  case(f'clone-{creative}-{bool(carried)}',{9:stack('ender_pearl',7),10:stack('iron_sword')},creative=creative,carried=carried,clicks=[click(9,2,'CLONE'),click(10,2,'CLONE'),click(-999,0),click(10,2,'CLONE')])
for kind in ['stone','iron_sword','ender_pearl']:
 for button in [0,1]:
  case(f'throw-{kind}-{button}',{9:stack(kind,1 if kind=='iron_sword' else 12)},clicks=[click(9,button,'THROW'),click(9,button,'THROW')])
for mode in [0,1,2,3]:
 for creative in [False,True]:
  for count in [1,2,7,64]:
   case(f'drag-{mode}-{creative}-{count}',{9:stack('stone',63),10:stack('dirt',2)},carried=stack('stone',count),creative=creative,clicks=drag([9,10,11,12,13],mode))
case('drag-armor-slot-limits',{9:stack('diamond_helmet')},carried=stack('diamond_helmet'),clicks=drag([5,6,7,8,11],0))
case('drag-reset-then-click',carried=stack('stone',12),clicks=[click(-999,0,'QUICK_CRAFT'),click(9,1,'QUICK_CRAFT'),click(10),click(11,1,'QUICK_CRAFT'),click(-999,2,'QUICK_CRAFT')])
for button in [0,1]:
 case(f'pickupall-order-{button}',{9:stack('stone',64),10:stack('stone',16),11:stack('stone',30),12:stack('dirt',20)},carried=stack('stone',1),clicks=[click(13,button,'PICKUP_ALL')])
case('pickupall-distinct-components',{9:stack('stone',20,{'minecraft:custom_name':'"A"'}),10:stack('stone',30,{'minecraft:custom_name':'"B"'}),11:stack('stone',5,{'minecraft:custom_name':'"A"'})},carried=stack('stone',1,{'minecraft:custom_name':'"A"'}),clicks=[click(12,0,'PICKUP_ALL')])
case('custom-stack-limit',{9:stack('stone',7,{'minecraft:max_stack_size':8}),10:stack('stone',5,{'minecraft:max_stack_size':8})},clicks=[click(10),click(9),click(11),click(9,0,'QUICK_MOVE')])
items=['stone','dirt','ender_pearl','snowball','iron_sword','shield','diamond_helmet','diamond_chestplate','diamond_leggings','diamond_boots','elytra','carved_pumpkin','wolf_armor']
maxes={n:(1 if n not in ['stone','dirt','ender_pearl','snowball'] else 16 if n in ['ender_pearl','snowball'] else 64) for n in items}
for sequence in range(48):
 initial={}
 for slot in list(range(9,45))+[45]:
  if R.randrange(3)==0:
   item=R.choice(items);initial[slot]=stack(item,R.randint(1,maxes[item]))
 carried=stack('stone',R.randint(1,64)) if sequence%3==0 else None
 actions=[]
 for step in range(80):
  kind=R.choice(['PICKUP','PICKUP','PICKUP','QUICK_MOVE','SWAP','CLONE','THROW','PICKUP_ALL','QUICK_CRAFT'])
  slot=R.choice([-999,*range(1,46)]) if kind=='PICKUP' else R.randrange(1,46)
  button=R.choice([0,1,8,40]) if kind=='SWAP' else 2 if kind=='CLONE' else R.randrange(2)
  if kind=='QUICK_CRAFT':actions.extend(drag(R.sample(list(range(1,46)),R.randint(1,6)),R.randrange(3)))
  else:actions.append(click(slot,button,kind))
 case(f'random-{sequence}',initial,carried,creative=sequence%2==0,clicks=actions)
output=Path(__file__).resolve().parent/'container_oracle_queries.jsonl'
output.write_text(''.join(json.dumps(c,separators=(',',':'))+'\n' for c in CASES))
print(json.dumps({'cases':len(CASES),'clicks':sum(len(c['clicks']) for c in CASES),'seed':'0x121767'}))
