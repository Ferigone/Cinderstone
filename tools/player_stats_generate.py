#!/usr/bin/env python3
"""Execute original counter arithmetic, dirty/send behavior and packet codecs."""
from pathlib import Path
import hashlib,itertools,json,os,random,subprocess
ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle';JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin');SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def key(kind,value):return dict(type_id=kind,value_id=value)
def main():
    jar=ORACLE/'server-inner-1.21.jar';assert hashlib.sha256(jar.read_bytes()).hexdigest()==SHA
    runtime=ROOT/'runtime/player-stats';runtime.mkdir(parents=True,exist_ok=True);cp=os.pathsep.join(map(str,[runtime,jar,*sorted((ORACLE/'libraries').rglob('*.jar'))]));flags=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED']
    compile=subprocess.run([JDK/'javac',*flags,'-proc:none','-cp',cp,'-d',runtime,ROOT/'tools/player_stats_oracle.java'],text=True,capture_output=True)
    if compile.returncode:raise RuntimeError(compile.stdout+compile.stderr)
    facts=subprocess.run([JDK/'java',*flags,'-Xmx1G','-cp',cp,'oracle.player_stats_oracle','facts'],text=True,capture_output=True)
    if facts.returncode:raise RuntimeError((facts.stdout+facts.stderr)[-5000:])
    registry=[json.loads(s[7:])for s in facts.stdout.splitlines()if s.startswith('ORACLE:')];assert len(registry)==1;registry=registry[0]
    (ROOT/'fixtures/player-stats-registry.json').write_text(json.dumps(registry,separators=(',',':'))+'\n')
    queries=[];extremes=[-2147483648,-2147483647,-100,-1,0,1,100,2147483646,2147483647]
    for server,old,amount in itertools.product([False,True],extremes,extremes):
        operations=[dict(op='get',key=key(4,1)),dict(op='set',key=key(4,1),value=old)]
        if server:operations.append(dict(op='send'))
        operations +=[dict(op='increment',key=key(4,1),amount=amount),dict(op='get',key=key(4,1))]
        if server:operations +=[dict(op='send'),dict(op='send'),dict(op='set',key=key(4,1),value=old),dict(op='set',key=key(4,1),value=old),dict(op='mark_all_dirty'),dict(op='send')]
        queries.append(dict(server=server,operations=operations))
    rng=random.Random(0x767395357a7)
    all_keys=[key(kind['type_id'],v['value_id'])for kind in registry['types']for v in kind['values']]
    for case in range(48):
        chosen=rng.sample(all_keys,12);operations=[]
        for step in range(60):
            k=rng.choice(chosen);op=rng.choice(['set','increment','get','send','mark_all_dirty'])
            value=dict(op=op)
            if op in ['set','increment','get']:value['key']=k
            if op=='set':value['value']=rng.choice(extremes)
            if op=='increment':value['amount']=rng.choice(extremes)
            operations.append(value)
        operations +=[dict(op='mark_all_dirty'),dict(op='send'),dict(op='send')]
        queries.append(dict(server=True,operations=operations))
    for kind in registry['types']:
        for value in [kind['values'][0],kind['values'][-1]]:
            k=key(kind['type_id'],value['value_id'])
            for amount in extremes:queries.append(dict(server=True,operations=[dict(op='set',key=k,value=amount),dict(op='send')]))
    for length in [0,1,2,5,16,64]:
        for repeat in range(8):
            entries=[dict(key=k,value=rng.choice(extremes))for k in rng.sample(all_keys,length)]
            queries.append(dict(server=True,operations=[dict(op='ordered_packet',entries=entries)]))
    queries.append(dict(server=True,operations=[dict(op='ordered_packet',entries=[dict(key=k,value=extremes[index%len(extremes)])for index,k in enumerate(all_keys)])]))
    pickup=json.loads((ROOT/'fixtures/item-pickup-vectors.json').read_text())
    for index,row in enumerate(pickup['cases']):
        operations=[dict(op='pickup_award',item_id=event['stat_item_id'],original_count=event['argument_count'])for event in row['expected']['events']if event['callback']=='stat']
        queries.append(dict(server=True,original_pickup_case=index,operations=[*operations,dict(op='send')]))
    file=dict(DataVersion=3953,stats={'minecraft:picked_up':{'minecraft:stone':17,'minecraft:ender_pearl':-1},'minecraft:custom':{'minecraft:drop':2147483647},'minecraft:mined':{'minecraft:stone':0}})
    queries.append(dict(server=True,operations=[dict(op='set',key=key(4,1),value=3),dict(op='load_current',json=json.dumps(file)),dict(op='get',key=key(4,1)),dict(op='send'),dict(op='mark_all_dirty'),dict(op='send')]))
    java=subprocess.run([JDK/'java',*flags,'-Xmx1G','-cp',cp,'oracle.player_stats_oracle'],input=''.join(json.dumps(q)+'\n'for q in queries),text=True,capture_output=True);(runtime/'oracle.log').write_text(java.stdout+java.stderr)
    if java.returncode:raise RuntimeError((java.stdout+java.stderr)[-7000:])
    rows=[json.loads(s[7:])for s in java.stdout.splitlines()if s.startswith('ORACLE:')];assert len(rows)==len(queries),(len(rows),len(queries))
    output=ROOT/'fixtures/player-stats-vectors.json';output.write_text(json.dumps(dict(minecraft='1.21',oracle_inner_sha256=SHA,scope='Unchanged StatsCounter/ServerStatsCounter methods, actual AwardStats stream codec and current-version parseLocal/toJson. Only network send observed; unordered map entries compared as maps. Ordered-packet input uses actual Object2IntLinkedOpenHashMap.',cases=[dict(query=q,expected=r)for q,r in zip(queries,rows)]),separators=(',',':'))+'\n')
    print(json.dumps(dict(ok=True,cases=len(queries),operation_snapshots=sum(len(r['states'])for r in rows),packets=sum(len(s['packets'])for r in rows for s in r['states']),file=str(output),sha256=hashlib.sha256(output.read_bytes()).hexdigest())))
if __name__=='__main__':main()
