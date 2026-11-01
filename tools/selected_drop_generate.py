#!/usr/bin/env python3
"""Execute unchanged Q/CtrlQ and creative-negative handlers on controlled hosts."""
from pathlib import Path
import collections
import hashlib
import itertools
import json
import os
import random
import subprocess

ROOT=Path(__file__).resolve().parents[1]
JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
ORACLE=ROOT.parent/'mc121-oracle'
SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'


def stack(name='stone',count=1,components=None):
    return dict(id='minecraft:'+name,count=count,**(dict(components=components)if components else{}))


def selected_cases():
    pool=[None,stack(count=1),stack(count=2),stack(count=64),stack('ender_pearl',16),
          stack('diamond_sword'),stack(count=65),stack(count=8,components={'minecraft:max_stack_size':8}),
          stack(count=5,components={'minecraft:custom_name':'"Hello"'})]
    rows=[]
    for index,(selected,value,all_)in enumerate(itertools.product(range(9),pool,[False,True])):
        slots=[None]*46;slots[36+selected]=value;slots[9]=stack('dirt',3);slots[1]=stack('oak_planks',2)
        remote=slots.copy()
        if index%3==0:remote[36+selected]=None;remote[9]=None
        if index%3==1:remote[36+selected]=stack('stone',4)
        carried=stack('arrow',3)if index%4==0 else None
        rows.append(dict(name=f'selected/{selected}/{index}',selected=selected,creative=index%2==0,
            slots=slots,remote_slots=remote,carried=carried,remote_carried=None if index%8==0 else carried,
            initial_state_id=[0,1,32766,32767][index%4],operations=[
                dict(all=all_,broadcast=index%2==0),dict(all=False,broadcast=True),
                dict(all=True,broadcast=True),dict(all=False,broadcast=True)]))
    rng=random.Random(121767)
    for index in range(64):
        slots=[None]*46
        for i in range(9,46):
            if rng.random()<.28:slots[i]=rng.choice(pool)
        remote=slots.copy()
        for _ in range(3):remote[rng.randrange(9,46)]=rng.choice(pool)
        rows.append(dict(name=f'selected/random/{index}',selected=rng.randrange(9),creative=bool(index%2),
            slots=slots,remote_slots=remote,carried=None,remote_carried=None,
            initial_state_id=rng.choice([0,32766,32767]),operations=[
                dict(all=bool(rng.randrange(2)),broadcast=bool(rng.randrange(2)))for _ in range(7)]))
    return rows


def creative_cases():
    pool=[None,stack(count=64),stack(count=65),stack('ender_pearl',16),stack('ender_pearl',17),
          stack('diamond_sword'),stack('diamond_sword',2),
          stack(count=8,components={'minecraft:max_stack_size':8}),
          stack(count=9,components={'minecraft:max_stack_size':8}),stack('bundle')]
    rows=[]
    for index,(spam,slot,value)in enumerate(itertools.product(
            [-2147483648,-1,0,1,179,180,199,200,219,2147483647],
            [-32768,-999,-1,0,1,45,46,32767],pool)):
        op=dict(op='drop',slot=slot,stack=value)
        rows.append(dict(name=f'creative/boundary/{index}',mode='creative',creative=True,spam=spam,
            operations=[op,dict(op='tick'),op,dict(op='tick')]))
    for spam in [0,199,200]:
        for value in [None,stack(count=1),stack('bundle')]:
            rows.append(dict(name=f'creative/not_creative/{spam}/{value}',mode='creative',creative=False,spam=spam,
                operations=[dict(op='drop',slot=-1,stack=value),dict(op='tick')]))
    for spam in [0,199,200]:
        for value in [None,stack(count=1),stack('bundle')]:
            rows.append(dict(name=f'creative/no_features/{spam}/{value}',mode='creative',features_enabled=False,spam=spam,
                operations=[dict(op='drop',slot=-1,stack=value),dict(op='tick')]))
    for value in [None,stack(count=1)]:
        rows.append(dict(name=f'creative/spam_sequence/{value}',mode='creative',spam=0,
            operations=[dict(op='drop',slot=-1,stack=value)for _ in range(12)]+
                       [dict(op='tick')for _ in range(10)]+[dict(op='drop',slot=-999,stack=value)]))
    return rows


def main():
    jar=ORACLE/'server-inner-1.21.jar';assert hashlib.sha256(jar.read_bytes()).hexdigest()==SHA
    runtime=ROOT/'runtime/selected-drop';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True)
    cp=os.pathsep.join(map(str,[classes,jar,*sorted((ORACLE/'libraries').rglob('*.jar'))]))
    exports=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED']
    sources=['tools/container_oracle.java','tools/inventory_normalization_oracle.java','tools/selected_drop_oracle.java']
    subprocess.run([JDK/'javac',*exports,'-proc:none','-cp',cp,'-d',classes,*[ROOT/name for name in sources]],cwd=ROOT,check=True)
    selected,creative=selected_cases(),creative_cases();queries=selected+creative
    result=subprocess.run([JDK/'java',*exports,'-Xmx1G','-cp',cp,'oracle.selected_drop_oracle'],cwd=ROOT,
        input=''.join(json.dumps(query,separators=(',',':'))+'\n'for query in queries),text=True,capture_output=True,timeout=180)
    (runtime/'oracle.log').write_text(result.stdout+result.stderr)
    result.check_returncode()
    results=[json.loads(line.split('ORACLE:',1)[1])for line in result.stdout.splitlines()if line.startswith('ORACLE:')]
    assert len(results)==len(queries),(len(results),len(queries))
    assert not any('error'in row for row in results),[row for row in results if'error'in row][:3]
    assert all('server-inner-1.21.jar'in line for line in result.stdout.splitlines()if line.startswith('SOURCE:'))
    header=dict(minecraft='1.21',protocol=767,oracle_inner_sha256=SHA,
        original_classes_modified=False,environment='ASM subclasses supply world/server, network sink and final item creation; original selected-drop/creative handlers, Inventory removal, menu prediction/broadcast and aqv$1 synchronizer execute unchanged')
    for kind,cases,offset in [('selected',selected,0),('creative',creative,len(selected))]:
        rows=[dict(query=query,expected=results[offset+i]['expected'])for i,query in enumerate(cases)]
        fixture=ROOT/f'fixtures/{kind}_drop_vectors.json';fixture.write_text(json.dumps(dict(**header,cases=rows),separators=(',',':'))+'\n')
        states=[state for row in rows for state in row['expected']['states']]
        report=dict(ok=True,**header,cases=len(rows),operations=len(states),fixture_sha256=hashlib.sha256(fixture.read_bytes()).hexdigest(),
            source_sha256={name:hashlib.sha256((ROOT/name).read_bytes()).hexdigest()for name in sources+['tools/selected_drop_generate.py']})
        if kind=='selected':
            callbacks=[attempt for state in states for attempt in state['before_broadcast']['attempts']]
            assert all(attempt['inventory_selected']==attempt['remote_selected']for attempt in callbacks)
            assert all(not attempt['around']and attempt['trace_item']for attempt in callbacks)
            assert all(not state['before_broadcast']['packets']for state in states)
            report.update(prediction_before_drop_callbacks=len(callbacks),empty_callbacks=sum(attempt['stack']=='AA=='for attempt in callbacks),
                dropped_nonempty=sum(len(state['before_broadcast']['drops'])for state in states),
                deferred_packets=sum(len(state['after_broadcast']['packets'])for state in states),
                scope='Original ServerPlayer.drop(boolean), Inventory.removeFromSelected, remote prediction before drop, flags false/true, unchanged unrelated slots/cursor, no immediate ack, deferred original menu/ServerPlayer synchronizer packet bytes and 15-bit state IDs')
        else:
            callbacks=[attempt for state in states for attempt in state['attempts']]
            assert all(not attempt['around']and attempt['trace_item']for attempt in callbacks)
            report.update(drop_callbacks=len(callbacks),empty_callbacks=sum(attempt['stack']=='AA=='for attempt in callbacks),
                tick_operations=sum(op['op']=='tick'for row in rows for op in row['query']['operations']),
                disabled_item_attempts=sum(op['op']=='drop'and inp is not None and not inp['enabled']for row in rows for op,inp in zip(row['query']['operations'],row['expected']['inputs'])),
                scope='Original creative packet handler and full listener tick on controlled host: any negative i16 slot, empty/count maximum, creative/features preconditions, strict spam<200, +20 wrapping arithmetic, positive-only -1 tick, empty attempts and Player.drop wrapper false/true flags',
                not_verified=['World block-entity coordinate replacement','Actual item entity creation, trajectory and gameplay callback effects','Native positive-slot assignment is covered by separate inventory tests'])
        (ROOT/f'evidence/{kind}-drop-oracle.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))


if __name__=='__main__':main()
