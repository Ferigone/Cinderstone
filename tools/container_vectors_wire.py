#!/usr/bin/env python3
"""Encode actual Java menu snapshots using independent protocol Slot writer."""
import pathlib,json,subprocess,argparse
r=pathlib.Path(__file__).resolve().parent.parent
p=argparse.ArgumentParser();p.add_argument('--input',default='tools/container_oracle_vectors.json');p.add_argument('--output',default='tools/container_vectors.json');args=p.parse_args()
source=json.loads((r/args.input).read_text());stacks=[];indices={}
def stack(value):
 key=json.dumps(value,sort_keys=True,separators=(',',':'))
 if key not in indices:indices[key]=len(stacks);stacks.append(value)
 return indices[key]
for case in source['cases']:
 case['slots']=[stack(s) for s in case['slots']];case['carried']=stack(case['carried'])
 for state in case['states']:
  state['slots']=[stack(s) for s in state['slots']];state['carried']=stack(state['carried']);state['drops']=[stack(s) for s in state['drops']]
p=subprocess.run(['node',str(r/'tools/crafting_stack_wire.mjs')],input='\n'.join(map(json.dumps,stacks))+'\n',capture_output=True,text=True,check=True);wire=[json.loads(l) for l in p.stdout.splitlines()];assert len(wire)==len(stacks),(len(wire),len(stacks),p.stderr);assert all('slot' in v for v in wire),[v for v in wire if 'error'in v]
source['stacks']=[v['slot'] for v in wire];source['wire_producer']='independent Prismarine Slot encoder with exact1.21 Mojang schema corrections'
(r/args.output).write_text(json.dumps(source,separators=(',',':'))+'\n');print(json.dumps({'cases':len(source['cases']),'steps':sum(len(c['states']) for c in source['cases']),'unique_stacks':len(stacks)}))
