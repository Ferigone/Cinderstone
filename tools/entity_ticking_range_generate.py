#!/usr/bin/env python3
"""Actual Java1.21 TickingTracker and DistanceManager source-bookkeeping oracle."""
from pathlib import Path
import hashlib,json,os,random,subprocess,zipfile
ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle'
JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
OUTER_SHA1='450698d1863ab5180c25d7c804ef0fe6369dd1ba'
INNER_SHA256='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def generate():
    cases=[]
    def add(name,ops,distance=2,points=None,tick=0):
        cases.append(dict(name=name,query=dict(simulation_distance=distance,ticket_tick=tick,points=points or [[0,0],[1,1],[2,2],[3,3],[-1,-1]],ops=[op.copy()for op in ops])))
    def raw(op,ch=(0,0),level=31,kind='player'):return dict(op='raw_'+op,kind=kind,chunk=list(ch),level=level)
    def player(op,ch=(0,0),id=1):return dict(op='player_'+op,chunk=list(ch),player=id)
    def run(budget=None):return dict(op='run',**({}if budget is None else dict(budget=budget)))
    def region(op,ch=(0,0),distance=2,kind='start'):return dict(op='region_'+op,kind=kind,chunk=list(ch),distance=distance)
    add('source-before-drain-and-exact-fifo',[player('add'),run(0),run(-1),run(-2147483648),*[run(1)for _ in range(60)],run(),player('remove'),*[run(1)for _ in range(20)],run()])
    add('shared-player-source-membership',[player('add',id=10),player('add',id=20),player('add',id=10),run(3),player('remove',id=10),run(),player('remove',id=999),player('remove',id=20),run(),player('add',id=-2147483648),player('add',id=2147483647),run(),dict(op='simulation',distance=3),run(1),dict(op='simulation',distance=1),run(),player('remove',id=-2147483648),player('remove',id=2147483647),run()])
    add('source-mutations-without-draining',[raw('add',level=31),raw('add',level=30),raw('remove',level=31),raw('add',level=32),raw('remove',level=30),raw('remove',level=32),run(),raw('remove',ch=(3,-4),level=99),run(),raw('add',level=31),run(2),raw('remove',level=31),raw('add',level=30),run(4),raw('remove',level=30),run()])
    add('ticket-type-union-and-region-clock',[region('add',distance=3),run(),dict(op='purge',count=2),region('add',distance=3),dict(op='forced',chunk=[0,0],enabled=True),player('add'),run(),region('remove',distance=3),run(4),dict(op='forced',chunk=[0,0],enabled=False),run(),player('remove'),run(),region('remove',distance=3),dict(op='purge',count=1),run()])
    add('long-clock-overflow-and-zero-timeout',[region('add',kind='start'),region('add',ch=(1,0),kind='forced'),dict(op='purge',count=1),region('add',kind='start'),dict(op='purge',count=2),region('add',ch=(1,0),kind='forced'),run(),region('remove',kind='start'),region('remove',ch=(1,0),kind='forced'),run()],tick=9223372036854775807)
    for level in [-2147483648,-1,0,1,29,30,31,32,33,34,2147483647]:
        # Deep sources are expensive to serialize; one publication and removal
        # still observe the complete original propagation and invalidation.
        ops=[raw('add',level=level),run(1),run(),raw('add',level=level),raw('remove',level=level),run(3),run()]
        add('raw-level-boundary-'+str(level),ops)
    add('simulation-int-overflow',[player('add'),run(),*[op for d in [-2147483648,2147483647,-1,0,1,2,31,32]for op in [dict(op='simulation',distance=d),run(1),run()]],player('remove'),run()],distance=-2)
    for center in [(2147483647,2147483647),(-2147483648,-2147483648),(2147483647,-2147483648),(1875066,1875066)]:
        points=[[center[0],center[1]],[-2147483648,2147483647],[2147483647,-2147483648],[1875066,1875066]]
        add('wrapping-coordinate-'+str(center),[raw('add',center,30),run(1),run(),raw('add',center,29,'forced'),run(2),run(),raw('remove',center,30),raw('remove',center,29,'forced'),run()],points=points)
    # Source-map growth/shift/shrink order affects replacePlayerTicketsLevel.
    coords=[(i*3,(i*11)%23-11)for i in range(65)]
    ops=[player('add',ch,i)for i,ch in enumerate(coords)]
    ops += [dict(op='simulation',distance=0),run(2),dict(op='simulation',distance=-1),run()]
    ops += [player('remove',ch,i)for i,ch in enumerate(coords[:56])]
    ops += [dict(op='simulation',distance=1),run(7),run()]
    ops += [player('remove',ch,i+56)for i,ch in enumerate(coords[56:])]
    ops += [run()]
    add('source-map-grow-shrink-replacement-order',ops,distance=-1)
    for seed in range(10):
        rng=random.Random(0x121767a000+seed);ops=[];active=set();sources=set()
        for i in range(105):
            chance=rng.random();ch=(rng.randrange(-2,3),rng.randrange(-2,3));kind=rng.choice(['player','start','forced']);level=rng.randrange(29,36)
            if chance<.32:
                ops.append(raw('add',ch,level,kind));sources.add((ch,level,kind))
            elif chance<.54:
                if sources and rng.random()<.85:
                    ch,level,kind=rng.choice(sorted(sources));sources.remove((ch,level,kind))
                ops.append(raw('remove',ch,level,kind))
            elif chance<.69:ops.append(run(rng.choice([0,1,2,3,7,19,-1])))
            elif chance<.78:ops.append(run())
            elif chance<.89:
                if active and rng.random()<.5:
                    ch,id=rng.choice(sorted(active));active.remove((ch,id));ops.append(player('remove',ch,id))
                else:
                    id=rng.randrange(5);active.add((ch,id));ops.append(player('add',ch,id))
            elif chance<.96:ops.append(dict(op='simulation',distance=rng.choice([-1,0,1,2,3])))
            else:ops.append(dict(op='replace_players',level=rng.choice([29,30,31,32,33,34])))
        ops.append(run());add('seeded-source-history-'+str(seed),ops,distance=seed%4-1)
    return cases

def main():
    outer=ORACLE/'server-1.21.jar';inner=ORACLE/'server-inner-1.21.jar'
    assert hashlib.sha1(outer.read_bytes()).hexdigest()==OUTER_SHA1
    assert hashlib.sha256(inner.read_bytes()).hexdigest()==INNER_SHA256
    runtime=ROOT/'runtime/entity_ticking_range';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True)
    cp=os.pathsep.join(map(str,[classes,inner,*sorted((ORACLE/'libraries').rglob('*.jar'))]));exports=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED'];source=ROOT/'tools/entity_ticking_range_oracle.java'
    compile=subprocess.run([JDK/'javac',*exports,'-proc:none','-cp',cp,'-d',classes,ROOT/'tools/survival_oracle.java',source],text=True,capture_output=True)
    if compile.returncode:raise RuntimeError(compile.stdout+compile.stderr)
    cases=generate();transcript=''.join(json.dumps(case['query'],separators=(',',':'))+'\n'for case in cases)
    java=subprocess.run([JDK/'java',*exports,'-Xmx2G','-cp',cp,'oracle.entity_ticking_range_oracle'],input=transcript,text=True,capture_output=True)
    (runtime/'oracle.log').write_text(java.stdout+java.stderr)
    if java.returncode:raise RuntimeError((java.stdout+java.stderr)[-7000:])
    assert 'SOURCE:file:'+str(inner)in java.stdout and 'SOURCE_CHUNK:8053347149716602'in java.stdout
    states=[json.loads(line[7:])for line in java.stdout.splitlines()if line.startswith('ORACLE:')];assert len(states)==len(cases)
    for case,result in zip(cases,states):
        assert len(case['query']['ops'])==len(result)
        case['expected']=result
    fixture=ROOT/'fixtures/entity_ticking_range_vectors.json';fixture.write_text(json.dumps(dict(minecraft='1.21',cases=cases),separators=(',',':'))+'\n')
    with zipfile.ZipFile(inner)as jar:class_hashes={name:hashlib.sha256(jar.read(name+'.class')).hexdigest()for name in ['ara','aqf','eoq','eou','aqk','aqk$a','aqk$b','aqk$c','aqy','aqz','dcd']}
    all_states=[s for scene in states for s in scene]
    report=dict(ok=True,minecraft='1.21',official_outer_sha1=OUTER_SHA1,oracle_inner_sha256=INNER_SHA256,class_sha256=class_hashes,cases=len(cases),states=len(all_states),stored_level_rows=sum(len(s['levels'])for s in all_states),computed_rows=sum(len(s['computed'])for s in all_states),queued_entries=sum(sum(len(q)for q in s['queues'])for s in all_states),operations={kind:sum(op['op']==kind for case in cases for op in case['query']['ops'])for kind in sorted({op['op']for case in cases for op in case['query']['ops']})},fixture_bytes=fixture.stat().st_size,fixture_sha256=hashlib.sha256(fixture.read_bytes()).hexdigest(),oracle_source_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),generator_source_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),helper_source_sha256=hashlib.sha256((ROOT/'tools/survival_oracle.java').read_bytes()).hexdigest(),input_sha256=hashlib.sha256(transcript.encode()).hexdigest(),scope=['unchanged ara/aqf/eoq/eou original methods and actual DistanceManager addPlayer/removePlayer/updateSimulationDistance/addRegionTicket/removeRegionTicket/updateChunkForced/purgeStaleTickets','exact published levels, pending computed levels, FIFO priority queues, partial budgets and source-ticket map iteration during replacement','shared-player membership, PLAYER/START/FORCED source union, zero-timeout duplicate refresh and ticket-clock wrap, wrapping chunk coordinates and source sentinel'],not_verified=['ChunkHolder futures, async promotion confirmation, PersistentEntitySectionManager visibility and integrated main tick ordering','the three auxiliary DistanceManager trackers are initialized original classes but not drained; scheduling callbacks are controlled','nonzero-timeout and arbitrary-key ticket kinds, persistent forced-ticket loading, spawn host startup and natural spawning','full Minecraft runtime or complete1:1 parity'])
    (ROOT/'evidence/entity-ticking-range-oracle.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
