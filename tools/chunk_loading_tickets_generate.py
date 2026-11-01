#!/usr/bin/env python3
"""Record actual DistanceManager loading sources and throttler schedules."""
from pathlib import Path
import hashlib,json,os,subprocess,zipfile,random
ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle';JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
OUTER_SHA1='450698d1863ab5180c25d7c804ef0fe6369dd1ba';INNER_SHA256='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def generate():
    cases=[]
    def add(name,ops):cases.append(dict(name=name,query=dict(simulation_distance=2,points=[[0,0],[1,1],[2,2],[3,3],[4,4],[14,14],[15,15]],ops=ops)))
    def op(name,**args):return dict(op=name,**args)
    def player(name='player_add',ch=(0,0),id=1):return op(name,chunk=list(ch),player=id)
    def spin(n):return [a for _ in range(n)for a in [op('run_sorter'),op('run_main')]]
    start=[op('view',distance=2),player(),op('run_player')]
    add('two-stage-main-admission-and-four-chunk-throttle',start+ [op('run_sorter'),op('run_main',budget=1),op('run_sorter'),op('run_main',budget=1),op('run_sorter'),op('run_main',budget=1),*spin(8),op('run_loading'),op('run_ticking')])
    add('normal-host-future-release-unblocks-next-admission',start+spin(8)+[op('release',chunk=[0,0]),*spin(5),op('run_loading'),op('run_ticking')])
    add('queued-admission-rechecks-published-distance',start+[op('run_sorter'),player('player_remove'),op('run_player'),op('run_main',budget=1),op('run_sorter'),op('run_main'),*spin(4),op('run_loading'),op('run_ticking')])
    add('member-removal-before-graph-drain-preserves-old-predicate',start+[op('run_sorter'),player('player_remove'),op('run_main'),op('run_player'),*spin(4),op('run_loading'),op('run_ticking')])
    add('priority-release-cancels-submit-before-poll',start+[player('player_remove'),op('run_player'),*spin(5),op('run_loading'),op('run_ticking')])
    add('view-shrink-removes-primary-but-not-ticking-source',start+spin(6)+[op('run_ticking'),op('view',distance=-1),*spin(6),op('run_loading'),op('run_ticking')])
    add('view-growth-and-reentrant-source-changes',start+spin(6)+[op('view',distance=3),op('run_sorter',budget=7),player(ch=(1,0),id=2),op('run_player'),op('view',distance=1),*spin(8),op('run_loading'),op('run_ticking')])
    add('duplicate-membership-and-final-player-removal',[op('view',distance=0),player(id=1),player(id=2),player(id=1),op('run_player'),*spin(4),player('player_remove',id=1),op('run_player'),*spin(2),player('player_remove',id=2),op('run_player'),*spin(4),op('run_loading'),op('run_ticking')])
    for kind in ['start','forced','player']:
        add('raw-primary-only-'+kind,[op('raw_add',kind=kind,chunk=[0,0],level=31),op('run_loading',budget=1),op('run_loading'),op('run_ticking'),op('raw_add',kind=kind,chunk=[0,0],level=31),op('purge'),op('raw_remove',kind=kind,chunk=[0,0],level=31),op('run_loading',budget=3),op('run_loading'),op('run_ticking')])
        add('region-transfers-to-ticking-'+kind,[op('region_add',kind=kind,chunk=[0,0],distance=2),op('run_loading'),op('run_ticking'),op('purge'),op('region_add',kind=kind,chunk=[0,0],distance=2),op('region_remove',kind=kind,chunk=[0,0],distance=2),op('run_loading'),op('run_ticking')])
    add('forced-and-start-source-union',[op('forced',chunk=[0,0],enabled=True),op('region_add',kind='start',chunk=[2,0],distance=3),op('run_loading'),op('run_ticking'),op('forced',chunk=[0,0],enabled=False),op('run_loading'),op('run_ticking'),op('region_remove',kind='start',chunk=[2,0],distance=3),op('run_loading'),op('run_ticking')])
    for level in [32,33,43,44,45,46,2147483647]:
        add('main-loading-level-boundary-'+str(level),[op('raw_add',kind='forced',chunk=[0,0],level=level),op('run_loading',budget=0),op('run_loading',budget=1),op('run_loading'),op('raw_remove',kind='forced',chunk=[0,0],level=level),op('run_loading',budget=1),op('run_loading')])
    for state in ['success','failure','unloaded','exceptional','cancelled']:
        add('original-future-registration-'+state,[op('view',distance=0),player(),op('run_player'),*spin(3),op('run_loading'),op('run_ticking'),op('register_releases'),op('future_complete',chunk=[0,0],state=state),*spin(3),op('future_complete',chunk=[0,0],state='failure')])
    for state in ['failure','exceptional']:
        add('future-already-complete-before-registration-'+state,[op('view',distance=0),player(),op('run_player'),*spin(3),op('run_loading'),op('run_ticking'),op('future_complete',chunk=[0,0],state=state),op('register_releases'),*spin(3)])
    for center in [[2147483647,2147483647],[-2147483648,2147483647],[1875066,1875066]]:
        add('loading-coordinate-boundary-'+str(center),[op('raw_add',kind='forced',chunk=center,level=43),op('run_loading',budget=1),op('run_loading'),op('raw_remove',kind='forced',chunk=center,level=43),op('run_loading')])
    for seed in range(10):
        rng=random.Random(0x121767c000+seed);active=set();ops=[op('view',distance=2)];sources=set()
        for _ in range(85):
            chance=rng.random();ch=(rng.randrange(-2,3),rng.randrange(-2,3))
            if chance<.15:
                if active and rng.random()<.45:
                    ch,id=rng.choice(sorted(active));active.remove((ch,id));ops.append(player('player_remove',ch,id))
                else:
                    id=rng.randrange(4);active.add((ch,id));ops.append(player(ch=ch,id=id))
            elif chance<.24:ops.append(op('view',distance=rng.randrange(-1,4)))
            elif chance<.39:ops.append(op('run_player'))
            elif chance<.54:ops.append(op('run_sorter',budget=rng.choice([0,1,2,3,7,2147483647])))
            elif chance<.69:ops.append(op('run_main',budget=rng.choice([0,1,2,3,2147483647])))
            elif chance<.77:ops.append(op('run_loading',budget=rng.choice([0,1,3,19,2147483647])))
            elif chance<.83:ops.append(op('run_ticking'))
            elif chance<.92:
                kind=rng.choice(['start','forced','player']);level=rng.choice([31,32,33,44,45]);sources.add((ch,kind,level));ops.append(op('raw_add',kind=kind,chunk=list(ch),level=level))
            elif sources:
                ch,kind,level=rng.choice(sorted(sources));sources.remove((ch,kind,level));ops.append(op('raw_remove',kind=kind,chunk=list(ch),level=level))
        ops.extend([op('run_player'),*spin(4),op('run_loading'),op('run_ticking')]);add('seeded-executor-interleavings-'+str(seed),ops)
    return cases

def main():
    outer=ORACLE/'server-1.21.jar';inner=ORACLE/'server-inner-1.21.jar';assert hashlib.sha1(outer.read_bytes()).hexdigest()==OUTER_SHA1;assert hashlib.sha256(inner.read_bytes()).hexdigest()==INNER_SHA256
    runtime=ROOT/'runtime/chunk_loading_tickets';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True);cp=os.pathsep.join(map(str,[classes,inner,*sorted((ORACLE/'libraries').rglob('*.jar'))]));exports=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED'];source=ROOT/'tools/chunk_loading_tickets_oracle.java'
    compile=subprocess.run([JDK/'javac',*exports,'-proc:none','-cp',cp,'-d',classes,ROOT/'tools/survival_oracle.java',source],text=True,capture_output=True)
    if compile.returncode:raise RuntimeError(compile.stdout+compile.stderr)
    cases=generate();transcript=''.join(json.dumps(c['query'],separators=(',',':'))+'\n'for c in cases);java=subprocess.run([JDK/'java',*exports,'-Xmx2G','-cp',cp,'oracle.chunk_loading_tickets_oracle'],input=transcript,text=True,capture_output=True);(runtime/'oracle.log').write_text(java.stdout+java.stderr)
    if java.returncode:raise RuntimeError((java.stdout+java.stderr)[-6500:])
    header=next(json.loads(s[7:])for s in java.stdout.splitlines()if s.startswith('HEADER:'));assert header==dict(source='file:'+str(inner),max_loaded_level=44,player_ticket_level=31)
    states=[json.loads(s[7:])for s in java.stdout.splitlines()if s.startswith('ORACLE:')];assert len(cases)==len(states)
    for c,s in zip(cases,states):assert len(s)==len(c['query']['ops']);c['expected']=s
    pool=[];index={}
    for case in cases:
        for step in case['expected']:
            for field in ['player','loading','ticking','queue_levels']:
                value=step['state'][field];encoded=json.dumps(value,separators=(',',':'))
                if encoded not in index:index[encoded]=len(pool);pool.append(value)
                step['state'][field]={'pool':index[encoded]}
    fixture=ROOT/'fixtures/chunk_loading_tickets_vectors.json';fixture.write_text(json.dumps(dict(minecraft='1.21',pool=pool,cases=cases),separators=(',',':'))+'\n')
    flat=[step for s in states for step in s]
    with zipfile.ZipFile(inner)as jar:hashes={n:hashlib.sha256(jar.read(n+'.class')).hexdigest()for n in ['aqk','aqk$a','aqk$b','aqk$c','aqe','aqd','bpk','bpm$a','bpm$b','bpj','aqa','aqs$b','bph','aqb']}
    report=dict(ok=True,minecraft='1.21',official_outer_sha1=OUTER_SHA1,oracle_inner_sha256=INNER_SHA256,class_sha256=hashes,cases=len(cases),states=len(flat),max_acquired=max(len(s['state']['acquired'])for s in flat),scheduling_events=sum(e['event']=='schedule'for s in flat for e in s['events']),fixture_bytes=fixture.stat().st_size,fixture_sha256=hashlib.sha256(fixture.read_bytes()).hexdigest(),oracle_source_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),generator_source_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),scope=['original DistanceManager constructor/source methods and unchanged PlayerTicketTracker/ChunkTicketTracker/aqe/aqd/bpk','exact two-stage main admission, sorter priorities, acquired limit4, cancellation and published-distance recheck','main graph levels/default45/maxLoaded44, raw primary vs region transfer to original ara','original registration pass and CompletableFuture thenAccept behavior for normal/failure/unloaded/exceptional/cancelled completions','production MainThreadExecutor always queues; its poll services distance/future work before each main runnable (bytecode audit in tools/chunk_loading_tickets_audit.md)'],not_verified=['scheduling adapter stores actual ChunkHolder ticket level but does not execute original ChunkMap scheduling/generation/futures','original registration pass is invoked after a controlled host clears chunksToUpdateFutures and supplied loading levels settle; scheduling/generation of real holder futures is delegated to ChunkLifecycle','complete runtime chunk visibility, network chunk sending and host async scheduler interleavings','native port conformance until independently compared'])
    (ROOT/'evidence/chunk-loading-tickets-oracle.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
