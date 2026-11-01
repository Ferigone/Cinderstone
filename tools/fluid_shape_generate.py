#!/usr/bin/env python3
"""Original per-fluid IdentityHashMap cache history; each scene runs in a new JVM, without clearing caches."""
from pathlib import Path
import hashlib
import json
import os
import struct
import subprocess

ROOT=Path(__file__).resolve().parents[1]
ORACLE=ROOT.parent/'mc121-oracle'
JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'


def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def key(kind=1,source=True,amount=8,falling=False): return dict(kind=kind,source=source,amount=amount,falling=falling)
def keys():
    return [key(kind,source,amount,falling) for kind in [1,2] for source in [True,False]
            for amount in ([8] if source else range(1,9)) for falling in [False,True]]
def q(state=None,above=None,**other):
    result=dict(position=[3,-12,7]);result.update(other)
    if state is not None: result['key']=state
    if above is not None: result['above_key']=above
    return result


def scenes():
    states=keys()
    alone=[];above=[]
    for i,state in enumerate(states):
        same=key(state['kind'],i%2==0,8 if i%2==0 else i%8+1,i%3==0)
        alone += [q(state),q(state,same)]
        above += [q(state,same),q(state)]
    aliases=[q(key()),q(state_alias='waterlogged_block_state',above_alias='water_source_block_state'),
             q(state_alias='water_source_block_state',above=key()),q(key(1,True,8,True),key()),
             q(state_alias='waterlogged_block_state'),q(key(2)),q(state_alias='lava_source_block_state',above=key(2))]
    aliases_reverse=[q(state_alias='waterlogged_block_state',above_alias='water_source_block_state'),q(key()),
                     q(state_alias='water_source_block_state'),q(key(2),key(2)),q(state_alias='lava_source_block_state')]
    faults=[q(key(),fault_above=True),q(key(),key()),q(key(),fault_above=True),
            q(key(2),fault_above=True),q(key(2)),q(key(2),fault_above=True),
            q(key(1,False,1,False),fault_above=True),q(key(1,False,1,False)),
            q(key(1,False,1,False),fault_above=True)]
    mixed=[]
    for i,state in enumerate(states):
        mixed.append(q(state,key(3-state['kind']),position=[i,64,-i]))
    for i,state in enumerate(reversed(states)):
        mixed.append(q(state,key(state['kind']),position=[-i,-64,i]))
    result=[('all36_alone_then_same_above',alone),('all36_same_above_then_alone',above),
            ('waterlogged_canonical_identity_alone_first',aliases),('waterlogged_canonical_identity_above_first',aliases_reverse),
            ('query_fault_then_retry_and_hit_bypass',faults),('cross_family_owner_and_position_independence',mixed)]
    for name,queries in result:
        for i,query in enumerate(queries[:6 if name=='query_fault_then_retry_and_hit_bypass'else 4]):
            x,y,z=query['position']
            start,end=([x+.5,y+2,z+.5],[x+.5,y-1,z+.5])
            if i==2: start,end=([x-1,y+.95,z+.5],[x+2,y+.95,z+.5])
            if i==3: start,end=([x+.5,y+.1,z+.5],[x+2,y+.1,z+.5])
            query['clip_from_bits']=[struct.pack('>d',v).hex()for v in start]
            query['clip_to_bits']=[struct.pack('>d',v).hex()for v in end]
    return result


def main():
    jar=ORACLE/'server-inner-1.21.jar';assert sha(jar)==SHA
    runtime=ROOT/'runtime/fluid-shape';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True)
    cp=os.pathsep.join(map(str,[classes,jar,*sorted((ORACLE/'libraries').rglob('*.jar'))]))
    source=ROOT/'tools/fluid_shape_oracle.java'
    compile_result=subprocess.run([JDK/'javac','-proc:none','-cp',cp,'-d',classes,source],capture_output=True,text=True)
    if compile_result.returncode: raise RuntimeError(compile_result.stdout+compile_result.stderr)
    result=[];state_metadata=None;aliases=None;alias_descriptions=None
    for name,queries in scenes():
        run=subprocess.run([JDK/'java','-cp',cp,'oracle.fluid_shape_oracle'],input=''.join(json.dumps(q)+'\n'for q in queries),capture_output=True,text=True,timeout=120)
        (runtime/(name+'.log')).write_text(run.stdout+run.stderr)
        if run.returncode: raise RuntimeError((run.stdout+run.stderr)[-6000:])
        header=json.loads(next(line[7:]for line in run.stdout.splitlines()if line.startswith('HEADER:')))
        assert header['source'].endswith('server-inner-1.21.jar')
        rows=[json.loads(line[7:])for line in run.stdout.splitlines()if line.startswith('ORACLE:')]
        assert len(rows)==len(queries)
        assert len(header['states'])==36 and len(header['initial_cache'])==4
        if state_metadata is None: state_metadata=header['states'];aliases=header['aliases'];alias_descriptions=header['alias_descriptions']
        else: assert state_metadata==header['states'] and aliases==header['aliases'] and alias_descriptions==header['alias_descriptions']
        assert all(q.get('fault_above')for q in rows if not q['expected']['accepted']), 'unexpected original query/clip failure'
        result.append(dict(name=name,initial_cache=header['initial_cache'],queries=rows))
        print(json.dumps(dict(scene=name,queries=len(rows),initial_entries=sum(len(o['entries'])for o in header['initial_cache']))),flush=True)
    block_report=ORACLE/'reports/reports/blocks.json';blocks=json.loads(block_report.read_text())
    assert any(s['id']==aliases['waterlogged_block_state'] and s.get('properties',{}).get('waterlogged')=='true'and s.get('properties',{}).get('type')=='bottom'for s in blocks['minecraft:oak_slab']['states'])
    fixture=dict(minecraft='1.21',oracle_inner_sha256=SHA,states=state_metadata,aliases=aliases,alias_descriptions=alias_descriptions,scenes=result)
    destination=ROOT/'fixtures/fluid-shape-vectors.json';destination.write_text(json.dumps(fixture,separators=(',',':'))+'\n')
    all_queries=[q for s in result for q in s['queries']]
    assert sum(not q['expected']['accepted']for q in all_queries)==3
    assert sum('clip_hit'in q['expected']for q in all_queries)==24
    report=dict(status='PASS',minecraft='1.21',oracle_inner_sha256=SHA,fixture_sha256=sha(destination),
                oracle_source_sha256=sha(source),generator_sha256=sha(Path(__file__)),scenes=len(result),fresh_jvms=len(result),
                canonical_states=len(state_metadata),queries=len(all_queries),
                block_report_sha256=sha(block_report),alias_descriptions=alias_descriptions,
                bootstrap_cache_entries=[sum(len(o['entries'])for o in s['initial_cache'])for s in result],
                failed_queries=sum(not q['expected']['accepted']for q in all_queries),
                original_shape_clip_queries=sum('clip_hit'in q['expected']for q in all_queries),
                world_reads=sum(len(q['expected']['trace'])for q in all_queries),
                original_classes_modified=False,cache_cleared_or_replaced=False,native_implementation_verified=False,
                contract=['epc.g is a separate IdentityHashMap per canonical fluid owner singleton, constructed by Maps.newIdentityHashMap.',
                          'FlowingFluid.getShape computes one box on first query per FluidState identity using getHeight(world,pos) f32 promoted to f64.',
                          'Cached hits bypass world reads, independent of position and above-block context.',
                          'Optional clip_hit is produced by the actual returned VoxelShape.clip(from,to,BlockPos), without a second getShape query or cache mutation.',
                          'source/flowing/falling/amount keys are distinct; waterlogged and legacy water source blocks share canonical source FluidState.',
                          'Canonical source amount is8 and flowing amount1..8. The amount==9 full-shape early branch is outside all36 canonical states.'],
                limits=['Only original getShape cache/query semantics; no random fluid tick or Entity callbacks.',
                        'Explicit world query faults are injected provider errors, not normal Minecraft missing-chunk behavior.',
                        'Initial cache snapshots are observed separately for each fresh JVM; no assumed empty prehistory.',
                        'No custom FluidState with noncanonical amount9 or rewritten original fluid methods.'])
    (ROOT/'evidence/fluid-shape-oracle.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))


if __name__=='__main__':main()
