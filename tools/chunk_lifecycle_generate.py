#!/usr/bin/env python3
"""Real ChunkHolder updateFutures with controlled CompletableFutures/executor, no original rewrites."""
from pathlib import Path
import hashlib,itertools,json,os,random,subprocess
ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle';JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin');SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def rank(level):return 3 if level<=31 else 2 if level<=32 else 1 if level<=33 else 0
class Scene:
 def __init__(self,name,initial=45,executor='queued',apply=True):self.q=dict(name=name,initial_level=initial,executor=executor,apply_queue_inline=apply,futures=[],operations=[]);self.old=45;self.ticket=initial;self.id=0
 def future(self,state):self.id+=1;self.q['futures'].append(dict(id=self.id,state=state));return self.id
 def op(self,name,**fields):self.q['operations'].append(dict(op=name,**fields))
 def update(self,level,state='pending',alias=None):
  plans=[]
  for r,stage in [(1,'full'),(2,'block'),(3,'entity')]:
   if rank(self.old)<r<=rank(level):plans.append(dict(stage=stage,future=alias if alias is not None else self.future(state)))
  self.op('update',level=level,prepares=plans);self.old=level;self.ticket=level;return [p['future']for p in plans]
def build():
 scenes=[]
 # Every status transition with every readiness/result category.
 for old,new in itertools.product([45,34,33,32,31],repeat=2):
  for state in ['pending','success','failure','unloaded','exceptional','cancelled']:
   s=Scene(f'transition/{old}/{new}/{state}');s.update(old,'success');s.op('drain');ids=s.update(new,state)
   for id in reversed(ids):s.op('complete',future=id,state='success')
   s.op('drain');s.op('refresh');s.update(new,state);scenes.append(s.q)
 for mode in ['queued','inline','reject']:
  for states in itertools.product(['pending','success','failure'],repeat=3):
   s=Scene('three_stages/'+mode+'/'+'/'.join(states),executor=mode);ids=[s.future(v)for v in states]
   s.op('update',level=31,prepares=[dict(stage=k,future=id)for k,id in zip(['full','block','entity'],ids)]);s.old=31
   s.update(45);s.op('drain')
   for id in ids:s.op('complete',future=id,state='success')
   s.op('drain');scenes.append(s.q)
 # A queued thenRunAsync is not removed by cancelling its already-completed source confirmation.
 s=Scene('queued_success_then_demote_then_reverse_tasks');s.update(31,'success');s.update(45)
 for id in [3,1,2,1]:s.op('run',task=id)
 s.op('drain');scenes.append(s.q)
 for state in ['pending','success','failure','cancelled']:
  s=Scene('aliased_prepare_future/'+state);id=s.future(state);s.update(31,alias=id);s.op('complete',future=id,state='success');s.update(32);s.update(31,alias=id);s.op('drain');s.update(45);s.op('complete',future=id,state='cancelled');s.op('complete',future=id,state='cancelled');scenes.append(s.q)
 s=Scene('shared_unloaded_prepare_sentinel');s.update(31,alias=0);s.update(45);s.update(31,alias=0);s.op('drain');scenes.append(s.q)
 s=Scene('lazy_queue_level_getters',apply=False);s.update(33,'success');s.update(31,'success');s.update(34);s.op('apply_queue',notification=1);s.op('apply_queue',notification=0);s.op('apply_queue',notification=2);s.op('drain');scenes.append(s.q)
 rng=random.Random(0x121C4A11)
 for index in range(40):
  s=Scene(f'interleaved/{index}',executor=['queued','inline','reject'][index%3],apply=index%2==0)
  for _ in range(24):
   choice=rng.randrange(6)
   if choice<3:s.update(rng.choice([45,34,33,32,31,0,-2147483648,2147483647]),rng.choice(['pending','success','failure','exceptional','cancelled']))
   elif choice==3 and s.id:s.op('complete',future=rng.randint(1,s.id),state=rng.choice(['success','failure','unloaded','exceptional','cancelled']))
   elif choice==4:s.op('drain',budget=rng.randrange(4))
   else:s.op('executor',mode=rng.choice(['queued','inline','reject']))
  s.op('drain');s.op('refresh');scenes.append(s.q)
 return scenes
def main():
 jar=ORACLE/'server-inner-1.21.jar';assert sha(jar)==SHA
 runtime=ROOT/'runtime/chunk-lifecycle';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True)
 cp=os.pathsep.join(map(str,[classes,jar,*sorted((ORACLE/'libraries').rglob('*.jar'))]));source=ROOT/'tools/chunk_lifecycle_oracle.java';flags=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED']
 compile=subprocess.run([JDK/'javac',*flags,'-proc:none','-cp',cp,'-d',classes,source],capture_output=True,text=True)
 if compile.returncode:raise RuntimeError(compile.stdout+compile.stderr)
 scenes=build();run=subprocess.run([JDK/'java',*flags,'-cp',cp,'oracle.chunk_lifecycle_oracle'],input=''.join(json.dumps(q)+'\n'for q in scenes),capture_output=True,text=True,timeout=180);(runtime/'oracle.log').write_text(run.stdout+run.stderr)
 if run.returncode:raise RuntimeError((run.stdout+run.stderr)[-6500:])
 header=json.loads(next(l[7:]for l in run.stdout.splitlines()if l.startswith('HEADER:')));assert header['source'].endswith('server-inner-1.21.jar')
 rows=[json.loads(l[7:])for l in run.stdout.splitlines()if l.startswith('ORACLE:')];assert len(rows)==len(scenes)
 fixture=dict(minecraft='1.21',oracle_inner_sha256=SHA,max_level=header['max_level'],scenes=[dict(query=q,expected=r)for q,r in zip(scenes,rows)])
 destination=ROOT/'fixtures/chunk-lifecycle-vectors.json';destination.write_text(json.dumps(fixture,separators=(',',':'))+'\n')
 bytecode=subprocess.run([JDK/'javap','-classpath',jar,'-p','-c','apz','aqa','aql','dxt'],capture_output=True,text=True,check=True).stdout;(ROOT/'tools/chunk_lifecycle_bytecode.txt').write_text(bytecode)
 report=dict(status='PASS',minecraft='1.21',oracle_inner_sha256=SHA,fixture_sha256=sha(destination),oracle_source_sha256=sha(source),generator_sha256=sha(Path(__file__)),max_level=header['max_level'],scenes=len(scenes),operations=sum(len(q['operations'])for q in scenes),
             original_classes_modified=False,original_chunkholder_constructor=True,original_completable_future=True,
             callbacks='Only separate ChunkMap adapter overrides prepareAccessible/Ticking/EntityTicking and status callback; same loader/runtime package via MethodHandles lookup for package-private method.',
             executor_modes=['queued','inline','reject'],proof=['Immediate success and pending/failure/exception/cancellation source states.','Previously queued completed-confirmation callbacks survive later demotion/cancellation.','Demotion completes pending requests with original unloaded sentinel, replaces slots, immediately publishes status.','Shared future aliasing, repeated cancellation, arbitrary executor task order, mutable ticket levels and lazy queue-level callbacks.'],
             not_verified=['Native Rust implementation until independent fixture replay.','Chunk generation, save contents and the ChunkMap prepare methods themselves.','Thread races beyond explicit single-thread operation/executor interleavings.'])
 (ROOT/'evidence/chunk-lifecycle-oracle.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
