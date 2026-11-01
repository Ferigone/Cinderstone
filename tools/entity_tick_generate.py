#!/usr/bin/env python3
"""Call the unchanged1.21 EntityTickList with controlled entity identities."""
import hashlib
import json
import os
from pathlib import Path
import random
import subprocess

ROOT=Path(__file__).resolve().parents[1]
ORACLE=ROOT.parent/'mc121-oracle'
JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'

def op(name,entity=None,**fields):
    result=dict(op=name,**fields)
    if entity is not None:result['entity']=entity
    return result
def iteration(actions=None,at=0,**fields):
    return op('iterate',on_visit=[dict(at=at,actions=actions or [])],**fields)

def scenes():
    pool=[[1,7],[2,2],[3,0],[4,7],[5,-1],[6,-2**31],[7,2**31-1],[8,999]]
    yield dict(name='explicit-copy-on-write-and-identity',entities=pool,operations=[
        iteration(),op('add',1),op('add',2),op('add',3),op('contains',4),
        iteration([op('add',4),op('remove',2),op('add',5),op('remove',8),op('contains',2),iteration()]),
        iteration(),op('contains',1),op('remove',1),op('add',1),iteration(),
        iteration([op('remove',8)]),iteration([op('add',1)]),
        iteration([op('add',6),op('remove',3),op('throw',propagate=True)]),iteration(),
        iteration([op('id',1,value=42),op('contains',1),op('remove',1),op('add',1)]),iteration(),
        iteration([iteration(propagate=True)]),iteration(),op('remove',1),iteration(),
    ])
    yield dict(name='empty-active-map-still-rejects-nested-iteration',entities=[[1,0],[2,1]],operations=[
        op('add',1),iteration([op('remove',1),iteration(),op('contains',1)]),iteration(),
        op('add',2),iteration([op('remove',2),iteration(propagate=True)]),iteration(),
        op('add',1),iteration([op('throw',propagate=True)]),iteration(),
    ])
    rng=random.Random(0x121767E71C)
    edges=[-2**31,-2**31+1,-65536,-17,-1,0,1,7,16,32,65536,2**31-2,2**31-1]
    for index in range(128):
        pool=[[label,rng.choice(edges) if label%3 else rng.randint(-2**31,2**31-1)]for label in range(1,49)]
        operations=[]
        for step in range(40):
            kind=rng.randrange(10);entity=rng.randint(1,48)
            if kind<4:operations.append(op('add',entity))
            elif kind<6:operations.append(op('remove',entity))
            elif kind==6:operations.append(op('contains',entity))
            elif kind==7:operations.append(op('id',entity,value=rng.choice(edges)))
            else:
                callbacks=[]
                for at in rng.sample(range(8),rng.randint(1,3)):
                    actions=[]
                    for _ in range(rng.randint(1,5)):
                        action=rng.choice(['add','remove','contains','id','iterate','throw'])
                        if action=='iterate':actions.append(iteration(propagate=rng.choice([True,False])))
                        elif action=='throw':actions.append(op('throw',propagate=rng.choice([True,False])))
                        elif action=='id':actions.append(op('id',rng.randint(1,48),value=rng.choice(edges)))
                        else:actions.append(op(action,rng.randint(1,48)))
                    callbacks.append(dict(at=at,actions=actions))
                operations.append(op('iterate',on_visit=callbacks))
        operations.append(iteration())
        yield dict(name='random-'+str(index),entities=pool,operations=operations)
    pool=[[label,label-1]for label in range(1,770)]
    operations=[op('add',label)for label in range(1,513)]
    operations += [iteration([op('remove',label)for label in range(2,514,2)]+[op('add',label)for label in range(513,769)]),iteration()]
    operations += [op('add',label)for label in range(2,514,2)]
    operations += [iteration([op('id',769,value=0),op('add',769),op('remove',100),op('add',100)]),iteration()]
    yield dict(name='rehash-large-removal-reinsertion-and-zero-key',entities=pool,operations=operations)

def main():
    jar=ORACLE/'server-inner-1.21.jar'
    assert hashlib.sha256(jar.read_bytes()).hexdigest()==SHA
    runtime=ROOT/'runtime/entity-tick-oracle';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True)
    cp=os.pathsep.join(map(str,[classes,jar,*sorted((ORACLE/'libraries').rglob('*.jar'))]))
    build=subprocess.run([JDK/'javac','-proc:none','-cp',cp,'-d',classes,ROOT/'tools/entity_tick_oracle.java'],capture_output=True,text=True)
    if build.returncode:raise RuntimeError(build.stdout+build.stderr)
    queries=list(scenes())
    result=subprocess.run([JDK/'java','-Xmx1G','-cp',cp,'oracle.entity_tick_oracle'],input=''.join(json.dumps(q,separators=(',',':'))+'\n'for q in queries),capture_output=True,text=True)
    (runtime/'oracle.log').write_text(result.stdout+result.stderr)
    if result.returncode:raise RuntimeError((result.stdout+result.stderr)[-7000:])
    outputs=[json.loads(line.split('ORACLE:',1)[1])for line in result.stdout.splitlines()if line.startswith('ORACLE:')]
    assert len(outputs)==len(queries),(len(outputs),len(queries))
    target=ROOT/'fixtures/entity_tick_vectors.json'
    target.write_text(json.dumps(dict(minecraft='1.21',oracle_inner_sha256=SHA,scenes=[dict(query=q,expected=o)for q,o in zip(queries,outputs)]),separators=(',',':'))+'\n')
    print(json.dumps(dict(ok=True,scenes=len(queries),top_level_operations=sum(len(q['operations'])for q in queries),fixture_sha256=hashlib.sha256(target.read_bytes()).hexdigest(),fixture_bytes=target.stat().st_size)))

if __name__=='__main__':main()
