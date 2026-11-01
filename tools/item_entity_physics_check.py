#!/usr/bin/env python3
"""Actual cjh.tick with explicit scalarenvironment and kinematic/floor callbacks.
Entity.baseTick/fluidflow/collisionmovement/merge-neighbors are separate domains.
"""
import json,random,subprocess
from pathlib import Path
from item_entity_check import command,ROOT
rng=random.Random(0x12158)
blocks=json.loads((ROOT/'fixtures/items-1.21.json').read_text())['blocks']
scenes=[]
for index in range(128):
    name=['stone','ice','packed_ice','blue_ice','slime_block','honey_block'][index%6]
    scene={'op':'tick','entity_id':rng.choice([1,58,999,2**31-1]),'tick_count':rng.choice([0,37,2**31-15]),'position':[rng.randrange(-100,100)/8,64+rng.randrange(0,50)/8,rng.randrange(-100,100)/8],'velocity':[rng.randrange(-128,128)/128,rng.randrange(-64,64)/128,rng.randrange(-128,128)/128],'on_ground':index%5==0,'age':rng.choice([0,5998,5999,-32768,-6000]),'pickup_delay':rng.choice([0,1,10,40,32767,-1]),'no_gravity':index%7==0,'water_height':0.,'lava_height':0.,'floor_y':64.0 if index%4 else None,'floor_state':blocks[name]['default_state'],'stack':{'id':'minecraft:stone','count':64 if index%11==0 else 3},'ticks':80,'bounce':index%13==0}
    if index%3==1:scene['water_height']=rng.choice([0.1,0.5,1.0])
    if index%3==2:scene['lava_height']=rng.choice([0.1,0.5,1.0])
    if scene['on_ground']:scene['position'][1]=64.0
    scenes.append(scene)
cmd=command();subprocess.run([*cmd,'facts',ROOT/'tools/item_entity_physics_facts.json'],check=True)
r=subprocess.run(cmd,input=''.join(json.dumps(q)+'\n' for q in scenes),text=True,capture_output=True,check=True)
rows=[json.loads(line.split('ORACLE:',1)[1]) for line in r.stdout.splitlines() if 'ORACLE:' in line]
assert len(rows)==len(scenes),r.stderr
errors=[q for q in rows if 'error' in q];assert not errors,errors[:3]
(ROOT/'tools/item_entity_physics_vectors.json').write_text(json.dumps({'minecraft':'1.21','scope':'Actual cjh.tick scalarphysics/delays/despawn/cadence; baseTick and fluidflow disabled, movement is explicit open-space/floor callback, merging queried with empty-neighborlist','scenes':rows},separators=(',',':'))+'\n')
print(json.dumps({'ok':True,'scenes':len(rows),'post_tick_snapshots':sum(len(q['states']) for q in rows),'scope':'scalar ItemEntityphysics with explicit environmental movement'}))
