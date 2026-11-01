#!/usr/bin/env python3
"""Invoke unchanged original arq; controlled ready handles and transport prefix."""
import hashlib,json,random,struct,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
ORACLE=ROOT.parent/'mc121-oracle'
JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def bits(n):return struct.pack('>f',n).hex()
def cases():
 out=[]
 def add(name,memory,steps):out.append(dict(name=name,query=dict(memory=memory,steps=steps)))
 add('constructor_and_lazy_gates',False,[dict(op='send'),dict(op='raw',j=1),dict(op='send'),dict(op='ack',desired=bits(.01)),dict(op='send'),dict(op='ack',desired='7fc12345'),dict(op='send')])
 add('fractional_quota_101_ticks',False,[dict(op='ack',desired=bits(.01)),dict(op='mark',positions=[[0,0]])]+[dict(op='send') for _ in range(220)])
 add('initial_window_and_ack_wrapping',False,[dict(op='mark',positions=[[0,0],[1,0]]),dict(op='send',ready=[[0,0,0,0]]),dict(op='send'),dict(op='ack',desired=bits(64)),dict(op='send',ready=[[1,0,1,0]]),dict(op='ack',desired=bits(9)),dict(op='raw',j=-2147483648),dict(op='ack',desired=bits(float('inf'))),dict(op='raw',j=2147483647,k=2147483647),dict(op='send')])
 for raw in ['7fc00000','7fc12345','ffc00000','7f800000','ff800000','80000000','00000000',bits(.009999),bits(.01),bits(64),bits(65)]:
  add('ack_'+raw,False,[dict(op='raw',j=j,i=bits(3.5)) if i%2==0 else dict(op='ack',desired=raw) for i,j in enumerate([1,0,0,0,-1,0,-2147483648,0])])
 add('drop_lazy_alive',False,[dict(op='mark',positions=[[0,0],[1,2]]),dict(op='drop',position=[0,0],alive=True),dict(op='drop',position=[0,0],alive=False),dict(op='drop',position=[0,0],alive=True),dict(op='drop',position=[1,2],alive=False)])
 rng=random.Random(12176708)
 for index in range(240):
  positions=list(dict.fromkeys([(rng.randint(-12,12),rng.randint(-12,12)) for _ in range(180)]))
  if index%5==0:positions += [(2147483647,-2147483648),(-2147483648,2147483647),(46341,0),(0,65536)]
  positions=[list(p) for p in positions]
  ready=[]
  for p in positions:
   if rng.random()<.72:
    actual=p if index%3 else rng.choice(positions)
    ready.append(p+actual)
  center=rng.choice([[0,0],[5,-9],[2147483647,-2147483648],[-2147483648,2147483647]])
  quota=rng.choice([0.,1.,1.9,2.,3.,9.,16.,64.,256.])
  steps=[dict(op='mark',positions=positions),dict(op='raw',i=bits(quota)),dict(op='collect',center=center,ready=ready),dict(op='collect',center=center,ready=ready),dict(op='send',center=center,ready=ready),dict(op='ack',desired=bits(rng.choice([.01,.5,1.,9.,16.,64.]))),dict(op='send',center=center,ready=ready)]
  for _ in range(6):
   steps += [dict(op='mark',positions=rng.sample(positions,15)),dict(op='drop',position=rng.choice(positions),alive=bool(rng.randrange(2))),dict(op='send',center=center,ready=ready),dict(op='ack',desired=bits(rng.choice([.01,.5,1.,9.,16.,64.])))]
  add('selection_'+str(index),bool(index%2),steps)
 for index in range(100):
  positions=[[x,z] for x in range(-4,5) for z in range(-4,5)]
  rng.shuffle(positions)
  steps=[dict(op='mark',positions=positions)]
  for batch in range(15):
   ready=[p+p for p in positions if rng.random()<.8]
   steps += [dict(op='send',center=[index%7-3,index%5-2],ready=ready,fail=rng.choice(['','','','begin','chunk','end'])),dict(op='ack',desired=bits(rng.choice([.01,.5,1.,2.,9.,16.,64.])))]
   if batch%3==0:steps.append(dict(op='mark',positions=rng.sample(positions,18)))
  add('full_send_'+str(index),bool(index%2),steps)
 add('nan_quota_memory',True,[dict(op='mark',positions=[[0,0]]),dict(op='raw',i='7fc12345'),dict(op='send',ready=[[0,0,0,0]])])
 return out

def main():
 jar=ORACLE/'server-inner-1.21.jar';assert hashlib.sha256(jar.read_bytes()).hexdigest()==SHA
 runtime=ROOT/'runtime/player_chunk_sender';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True)
 cp=':'.join(map(str,[classes,jar,*sorted((ORACLE/'libraries').rglob('*.jar'))]));flags=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED']
 sources=['tools/survival_oracle.java','tools/player_chunk_sender_oracle.java']
 subprocess.run([JDK/'javac',*flags,'-proc:none','-cp',cp,'-d',classes,*[ROOT/s for s in sources]],check=True)
 rows=cases();p=subprocess.run([JDK/'java',*flags,'-Xmx1G','-cp',cp,'oracle.player_chunk_sender_oracle'],input=''.join(json.dumps(row['query'])+'\n' for row in rows),text=True,capture_output=True)
 (runtime/'oracle.log').write_text(p.stdout+p.stderr)
 if p.returncode:raise RuntimeError((p.stdout+p.stderr)[-7000:])
 assert [l for l in p.stdout.splitlines() if l.startswith('ACK_ID:')]==['ACK_ID:8']
 actual=[json.loads(l[7:]) for l in p.stdout.splitlines() if l.startswith('ORACLE:')];assert len(actual)==len(rows)
 for row,expected in zip(rows,actual):row['expected']=expected
 fixture=ROOT/'fixtures/player_chunk_sender.json';fixture.write_text(json.dumps(dict(oracle_inner_sha256=SHA,cases=rows),separators=(',',':'))+'\n')
 report=dict(ok=True,minecraft='1.21',oracle_inner_sha256=SHA,original_classes_rewritten=False,original_class_sources=[l[7:] for l in p.stdout.splitlines() if l.startswith('SOURCE:')],histories=len(rows),states=sum(len(row['expected']['steps']) for row in rows),ready_queries=sum(sum(t[0]=='ready' for t in st['trace']) for row in rows for st in row['expected']['steps']),ack_packet_id=8,successful_batches=sum(sum(t[0]=='end' for t in st['trace']) and not st['failed_transport'] for row in rows for st in row['expected']['steps']),transport_failure_prefixes=sum(st['failed_transport'] for row in rows for st in row['expected']['steps']),fixture_sha256=hashlib.sha256(fixture.read_bytes()).hexdigest(),source_sha256={s:hashlib.sha256((ROOT/s).read_bytes()).hexdigest() for s in sources+['tools/player_chunk_sender_generate.py']},original_methods=['arq constructor','arq.markChunkPending','arq.dropChunk','arq.onChunkBatchReceivedByClient','arq.collectChunksToSend','arq.sendNextChunks complete successful/transport-failure paths with actual packet constructors','adg/adf/adk real chunk+light packet constructors','agf.sendChunkPacket original return-only debug method'],not_verified=['Chunk/light payload fidelity in normal worlds: allocated original dvi has empty section/heightmap/BE arrays; original eot uses zero-height context and no light layers','Complete running world and transport; host adapters are controlled'])
 (ROOT/'evidence/player-chunk-sender-oracle.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
