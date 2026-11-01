#!/usr/bin/env python3
"""Original ChunkTrackingView.forEach → arq successful sendNext golden."""
import hashlib,json,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle'
JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def main():
 jar=ORACLE/'server-inner-1.21.jar';assert hashlib.sha256(jar.read_bytes()).hexdigest()==SHA
 runtime=ROOT/'runtime/main_chunk_transport_golden';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True)
 cp=':'.join(map(str,[classes,jar,*sorted((ORACLE/'libraries').rglob('*.jar'))]));flags=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED']
 sources=['tools/survival_oracle.java','tools/player_chunk_sender_oracle.java','tools/main_chunk_transport_golden.java','tools/main_chunk_transport_golden_generate.py']
 subprocess.run([JDK/'javac',*flags,'-proc:none','-cp',cp,'-d',classes,*[ROOT/s for s in sources if s.endswith('.java')]],check=True)
 p=subprocess.run([JDK/'java',*flags,'-cp',cp,'oracle.main_chunk_transport_golden'],cwd=ROOT,text=True,capture_output=True);(runtime/'oracle.log').write_text(p.stdout+p.stderr)
 if p.returncode:raise RuntimeError((p.stdout+p.stderr)[-5000:])
 found=[json.loads(l[7:]) for l in p.stdout.splitlines() if l.startswith('GOLDEN:')];assert len(found)==1;row=found[0]
 classes_seen=[l[7:] for l in p.stdout.splitlines() if l.startswith('SOURCE:')];assert len(classes_seen)==8 and all(str(jar) in source for source in classes_seen)
 assert list(map(len,row['batches']))==[9,16,16,4]
 assert len({tuple(pos) for batch in row['batches'] for pos in batch})==45
 for step in row['expected']['steps'][2:8]:assert step['trace']==[] and step['j']==step['k']==1 and step['i']=='00000000'
 assert row['expected']['steps'][-1]['pending']==[]
 original=ROOT/'fixtures/main_chunk_transport_original45.json';original.write_text(json.dumps(row,separators=(',',':'))+'\n')
 golden=ROOT/'fixtures/main_chunk_transport_golden.json';golden.write_text(json.dumps(dict(batches=row['batches']),indent=2)+'\n')
 report=dict(ok=True,minecraft='1.21',oracle_inner_sha256=SHA,original_classes_rewritten=False,original_class_sources=classes_seen,chunk_view=dict(center=[0,0],requested_view_distance=2,chunks=45,mark_order='actual aqg.Positioned.forEach, X outer / Z inner'),initial_sender=dict(memory=False,desired=9,quota=0,unacknowledged=0,max_unacknowledged=1),sequence=['mark original45 positions','send successful','six send calls blocked by outstanding initial batch','ACK16','send successful','send successful','send successful'],batch_sizes=list(map(len,row['batches'])),all45_positions_once=True,golden_sha256=hashlib.sha256(golden.read_bytes()).hexdigest(),original_history_sha256=hashlib.sha256(original.read_bytes()).hexdigest(),source_sha256={s:hashlib.sha256((ROOT/s).read_bytes()).hexdigest() for s in sources},scope=['Actual original ChunkTrackingView.forEach insertion order','Actual original arq scheduling, dependency order, readiness queries, Begin/Chunk/End, pending removals and counter/quota states','Successful original adg/adf/adk constructors in controlled empty-section chunk/no-light-layer context'],not_verified=['Normal-world chunk/light payload equality','Native main integration; liveprobe consumes the separate golden'],reproduce='python3 tools/main_chunk_transport_golden_generate.py')
 (ROOT/'evidence/main-chunk-transport-original45.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
