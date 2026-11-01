#!/usr/bin/env python3
"""Actual original ItemEntity merge-pair/owner/delay/age checks."""
import json,random,subprocess
from pathlib import Path
from item_entity_check import command,ROOT
rng=random.Random(0x12158c)
queries=[]
for i in range(1200):
    def item():
        max_stack=rng.choice([16,64,99]);name=rng.choice(['minecraft:stone','minecraft:dirt','minecraft:ender_pearl']);count=rng.randint(1,max_stack)
        stack={'id':name,'count':count};components={}
        if i%4==0:components['minecraft:max_stack_size']=max_stack
        if i%17==0:components['minecraft:custom_name']='"'+rng.choice(['A','B'])+'"'
        if components:stack['components']=components
        return {'stack':stack,'age':rng.choice([0,20,5999,6000,-32768,-6000]),'pickup_delay':rng.choice([0,5,10,40,32767,-1]),'target':rng.choice([None,'00000000-0000-4000-8000-000000000001','00000000-0000-4000-8000-000000000002']),'thrower':rng.choice([None,'00000000-0000-4000-8000-000000000001'])}
    a=item();b=item()
    if i%3==0:b['stack']['id']=a['stack']['id']
    if i%5==0:b['target']=a['target']
    queries.append({'op':'merge','name':str(i),'left':a,'right':b})
# Explicit quirks: equalcount choosesother; hard64mergecap withcustommax99.
for a,b,max_stack in [(3,3,64),(20,10,64),(10,20,64),(80,10,99),(10,80,99)]:
    item=lambda n:{'stack':{'id':'minecraft:stone','count':n,'components':{'minecraft:max_stack_size':max_stack}},'age':12,'pickup_delay':0,'target':None,'thrower':None}
    queries.append({'op':'merge','name':f'quirk-{a}-{b}-{max_stack}','left':item(a),'right':item(b)})
r=subprocess.run(command(),input=''.join(json.dumps(q)+'\n' for q in queries),capture_output=True,text=True,check=True)
rows=[json.loads(line.split('ORACLE:',1)[1]) for line in r.stdout.splitlines() if 'ORACLE:' in line]
assert len(rows)==len(queries),r.stderr
assert not [r for r in rows if 'error' in r],[r for r in rows if 'error' in r][:3]
unique=[];ids={}
def wire_id(stack):
    key=json.dumps(stack,separators=(',',':'),sort_keys=True)
    if key not in ids:ids[key]=len(unique);unique.append(stack)
    return ids[key]
for row in rows:
    for key in ['left','right','left_after','right_after']:row[key]['stack']=wire_id(row[key]['stack'])
encoded=subprocess.run(['node',ROOT/'tools/crafting_stack_wire.mjs'],input=''.join(json.dumps(s)+'\n' for s in unique),capture_output=True,text=True,check=True)
wire=[json.loads(line) for line in encoded.stdout.splitlines()]
assert len(wire)==len(unique),encoded.stderr
assert not [v for v in wire if 'error' in v],wire[:3]
report={'minecraft':'1.21','oracle_inner_sha256':'8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01','stacks':[v['slot'] for v in wire],'cases':rows}
(ROOT/'tools/item_entity_merge_vectors.json').write_text(json.dumps(report,separators=(',',':'))+'\n')
print(json.dumps({'ok':True,'actual_java_merge_pairs':len(rows),'unique_stacks':len(unique)}))
