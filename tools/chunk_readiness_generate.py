#!/usr/bin/env python3
"""Original ChunkMap prepare/range methods with genuine empty-context postprocess and saved-tick unpack."""
from pathlib import Path
import copy,hashlib,json,os,random,subprocess
ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle';JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin');PIN='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def base(name,state='success',radius=2,alias=False):
 q=dict(name=name,game_time='123',chunks=[],sources=[],holders=[],operations=[]);center=(2*radius+1)**2//2+1
 for z in range(-radius,radius+1):
  for x in range(-radius,radius+1):
   id=len(q['chunks'])+1;q['chunks'].append(dict(id=id,x=x,z=z,kind='full',saved_ticks=id==center))
   source=1 if alias else id
   if not alias or id==1:q['sources'].append(dict(id=source,state=state,chunk=center if alias else id))
   q['holders'].append(dict(x=x,z=z,requests=[[s,source]for s in range(12)]))
 return q
def op(q,name,**fields):q['operations'].append(dict(op=name,**fields))
def prepare(q,kind='block',job=1):op(q,'prepare',kind=kind,job=job,x=0,z=0);op(q,'observe',job=job,observer=job)
def finish_sources(q,order=None,state='success'):
 rows=q['sources']if order is None else order
 for s in rows:op(q,'source',id=s['id'],state=state,chunk=s['chunk'])
def build():
 scenes=[];modes=[('queued','queued'),('inline','queued'),('queued','inline'),('inline','inline'),('reject','queued'),('queued','reject')]
 for kind in ['accessible','block','entity']:
  for state in ['success','pending','failure','exceptional','cancelled']:
   for main,priority in modes:
    q=base(f'prepare/{kind}/{state}/{main}/{priority}',state);q.update(main_mode=main,priority_mode=priority);prepare(q,kind)
    finish_sources(q,list(reversed(q['sources'])));op(q,'drain');scenes.append(q)
 for alias in [False,True]:
  for terminal in ['success','failure','exceptional','cancelled']:
   for reverse in [False,True]:
    q=base(f'shared_sources/{alias}/{terminal}/{reverse}','pending',alias=alias)
    for i,kind in enumerate(['accessible','block','entity'],1):prepare(q,kind,i);op(q,'observe',job=i,observer=i+10)
    finish_sources(q,list(reversed(q['sources']))if reverse else None,terminal);op(q,'drain',queue='main');op(q,'drain',queue='priority');op(q,'drain');scenes.append(q)
 for send_state in ['pending','success','exceptional','cancelled']:
  for completion in ['success','exceptional','cancelled']:
   q=base(f'send_sync/{send_state}/{completion}');h=next(h for h in q['holders']if h['x']==0 and h['z']==0);h.update(send_id=1,send_state=send_state);prepare(q);op(q,'drain');op(q,'send',id=1,state=completion);op(q,'drain');scenes.append(q)
 for kind in ['accessible','block','entity']:
  for missing in [(-1,-1),(0,-1),(1,1)]:
   q=base(f'missing_holder/{kind}/{missing}','pending');q['holders']=[h for h in q['holders']if(h['x'],h['z'])!=missing];prepare(q,kind);op(q,'drain');finish_sources(q);op(q,'drain');scenes.append(q)
  for allowed in [None,0,8,11]:
   q=base(f'allowed/{kind}/{allowed}');next(h for h in q['holders']if h['x']==0 and h['z']==0)['allowed']=allowed;prepare(q,kind);op(q,'drain');scenes.append(q)
  for terminal in ['failure','exceptional','cancelled']:
   q=base(f'complete_prepared_before_tasks/{kind}/{terminal}');prepare(q,kind);op(q,'job',id=1,state=terminal,reason='Unloaded level chunk');op(q,'drain');scenes.append(q)
 for radius in [0,1,2,3]:
  for state in ['success','pending','failure','exceptional','cancelled']:
   q=base(f'direct_range/{radius}/{state}',state,max(2,radius));op(q,'range',job=1,x=0,z=0,radius=radius,statuses=[11,8,6,3][:radius+1]);finish_sources(q);op(q,'drain');scenes.append(q)
 for kind in ['accessible','block','entity']:
  q=base('center_proto/'+kind);next(c for c in q['chunks']if c['x']==0 and c['z']==0)['kind']='proto';prepare(q,kind);op(q,'drain');scenes.append(q)
 q=base('game_time_fault_is_real_action_failure');op(q,'time_fault',value=True);prepare(q);op(q,'drain');op(q,'time_fault',value=False);prepare(q,job=2);op(q,'time',value='456');op(q,'drain');scenes.append(q)
 q=base('tick_unpack_not_rebased_on_second_start');prepare(q);op(q,'drain');op(q,'time',value='999');prepare(q,job=2);op(q,'drain');scenes.append(q)
 rng=random.Random(0x1214EAD)
 for index in range(16):
  q=base(f'interleaved/{index}','pending',alias=index%3==0);q['main_mode'],q['priority_mode']=modes[index%len(modes)];jobs=0
  for _ in range(12):
   choice=rng.randrange(6)
   if choice<2:jobs+=1;prepare(q,rng.choice(['accessible','block','entity']),jobs)
   elif choice==2:
    s=rng.choice(q['sources']);op(q,'source',id=s['id'],state=rng.choice(['success','failure','exceptional','cancelled']),chunk=s['chunk'])
   elif choice==3:op(q,'drain',queue=rng.choice(['main','priority','any']),budget=rng.randrange(3))
   elif choice==4 and jobs:op(q,'job',id=rng.randint(1,jobs),state=rng.choice(['failure','exceptional','cancelled']),reason='Unloaded level chunk')
   else:op(q,'time',value=str(rng.randrange(-100,500)))
  finish_sources(q);op(q,'drain');scenes.append(q)
 return scenes
def main():
 jar=ORACLE/'server-inner-1.21.jar';assert sha(jar)==PIN
 runtime=ROOT/'runtime/chunk-readiness';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True);cp=os.pathsep.join(map(str,[classes,jar,*sorted((ORACLE/'libraries').rglob('*.jar'))]));flags=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED'];source=ROOT/'tools/chunk_readiness_oracle.java'
 compile=subprocess.run([JDK/'javac',*flags,'-proc:none','-cp',cp,'-d',classes,source],capture_output=True,text=True)
 if compile.returncode:raise RuntimeError(compile.stdout+compile.stderr)
 queries=build();run=subprocess.run([JDK/'java',*flags,'-Xmx1G','-cp',cp,'oracle.chunk_readiness_oracle'],input=''.join(json.dumps(q)+'\n'for q in queries),capture_output=True,text=True,timeout=180);(runtime/'oracle.log').write_text(run.stdout+run.stderr)
 if run.returncode:raise RuntimeError((run.stdout+run.stderr)[-6000:])
 header=json.loads(next(s[7:]for s in run.stdout.splitlines()if s.startswith('HEADER:')));assert header['source'].endswith('server-inner-1.21.jar')
 expected=[json.loads(s[7:])for s in run.stdout.splitlines()if s.startswith('ORACLE:')];assert len(expected)==len(queries)
 fixture=dict(minecraft='1.21',oracle_inner_sha256=PIN,**{k:v for k,v in header.items()if k!='source'},scenes=[dict(query=q,expected=e)for q,e in zip(queries,expected)])
 dest=ROOT/'fixtures/chunk-readiness-vectors.json';dest.write_text(json.dumps(fixture,separators=(',',':'))+'\n')
 report=dict(status='PASS',minecraft='1.21',oracle_inner_sha256=PIN,oracle_source_sha256=sha(source),generator_sha256=sha(Path(__file__)),vectors_sha256=sha(dest),scenes=len(queries),operations=sum(len(q['operations'])for q in queries),vectors_bytes=dest.stat().st_size,original_classes_modified=False,
             actual_methods=['ChunkMap.prepareAccessibleChunk','ChunkMap.prepareTickingChunk','ChunkMap.prepareEntityTickingChunk','ChunkMap.getChunkRangeFuture','GenerationChunkHolder.getOrScheduleFuture (observer calls super)','LevelChunk.postProcessGeneration (observer calls super)','ServerLevel.startTickingChunk (observer calls super)','LevelChunkTicks.unpack with nonempty SavedTicks','ChunkMap.sendChunkToPlayers (actual empty PlayerMap loop)','CompletableFuture/Util.sequence and original priority Runnable acknowledgement wrapper'],
             action_domain='Empty postprocessing arrays and pending block-entity NBT, original UpgradeData.EMPTY, no previous nonempty upgrade fixer work, concrete genuine saved tick containers. No block/entity/world mutation method is replaced by a no-op.',
             source_domain='Injected canonical status future slots in original GenerationChunkHolder, explicit highest allowed status and an existing original generation-task target FULL; generation is not executed or faked.',
             not_verified=['Rust core until independent replay.','Nonempty postprocessing/upgrade/block-entity conversion and actual generation scheduling.','Nonempty player recipients and actual chunk batch transmission.','General parallel-thread races outside controlled serialized executor/source interleavings.'])
 (ROOT/'evidence/chunk-readiness-oracle.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
