#!/usr/bin/env python3
"""Replay actual onDestroyed registry dispatch and default-stack executions."""
from pathlib import Path
import argparse, collections, hashlib, json, os, re, subprocess
from urllib.parse import urlparse,unquote
ROOT=Path(__file__).resolve().parents[1]; ORACLE=ROOT.parent/'mc121-oracle'
JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
    p=argparse.ArgumentParser();p.add_argument('--write-fixture',action='store_true');a=p.parse_args()
    jar=ORACLE/'server-inner-1.21.jar';assert digest(jar)==SHA
    runtime=ROOT/'runtime/item-destroyed';runtime.mkdir(parents=True,exist_ok=True)
    cp=os.pathsep.join(map(str,[runtime,jar,*sorted((ORACLE/'libraries').rglob('*.jar'))]))
    flags=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED']
    sources=[ROOT/'tools'/n for n in ['survival_oracle.java','item_hurt_oracle.java','item_destroyed_oracle.java']]
    c=subprocess.run([JDK/'javac',*flags,'-proc:none','-cp',cp,'-d',runtime,*sources],text=True,capture_output=True)
    if c.returncode:raise RuntimeError(c.stdout+c.stderr)
    run=subprocess.run([JDK/'java',*flags,'-Xmx512M','-cp',cp,'oracle.item_destroyed_oracle'],text=True,capture_output=True)
    (runtime/'oracle.log').write_text(run.stdout+run.stderr)
    if run.returncode:raise RuntimeError((run.stdout+run.stderr)[-7000:])
    origins=[s[7:]for s in run.stdout.splitlines()if s.startswith('SOURCE:')]
    assert len(origins)==2 and all(Path(unquote(urlparse(s).path)).resolve()==jar.resolve()for s in origins)
    results=[json.loads(s[7:])for s in run.stdout.splitlines()if s.startswith('ORACLE:')];assert len(results)==1
    data=results[0];rows=data['items'];assert len(rows)==1333
    assert sorted(row['item_id']for row in rows)==list(range(1333))
    for row in rows:
        assert row['default_stack_on_destroyed_returned']
        assert row['stack_count_after']==(0 if row['item_id']==0 else 1)
        mutation=row['declaring_class']=='cso' and not row['default_stack_contains_container']
        assert row['patch_size_after']==int(mutation) and not row['removed_after'] and row['game_events']==[]
        assert (row['stack_wire_before']!=row['stack_wire_after'])==mutation
        assert row['entity_rng_next_int']==data['expected_entity_rng_next_int']
        assert row['world_rng_next_int']==data['expected_world_rng_next_int']
    fixture=ROOT/'fixtures/item-destroyed-facts.json'
    if a.write_fixture:fixture.write_text(json.dumps(data,separators=(',',':'))+'\n')
    else:assert data==json.loads(fixture.read_text()),'original dispatch/default execution facts changed'
    snippets=[]
    for cls,signature in [('cul','public void a(cjh);'),('cso','public void a(cjh);'),('cus','public static void a(cjh, java.lang.Iterable<cuq>);')]:
        raw=subprocess.run([JDK/'javap','-p','-c','-classpath',jar,cls],capture_output=True,text=True,check=True).stdout
        start=raw.index('  '+signature);match=re.search(r'\n  (?:public|private|protected|static) ',raw[start+3:]);end=start+3+match.start()if match else len(raw)
        snippet=raw[start:end];snippets.append('CLASS '+cls+'\n'+snippet)
        if cls=='cul':assert re.findall(r'^\s+\d+: (.*)$',snippet,re.M)==['return']
    bytecode=ROOT/'tools/item_destroyed_bytecode.txt';bytecode.write_text('Original inner JAR SHA256 '+SHA+'\n\n'+'\n'.join(snippets))
    counts=dict(collections.Counter(row['declaring_class']for row in rows));assert counts=={'cul':407,'cso':925,'csw':1}
    block_absent=sum(row['declaring_class']=='cso'and not row['default_stack_contains_container']for row in rows)
    report=dict(ok=True,kind='original reflection plus original ItemStack.onDestroyed executions and bytecode',minecraft='1.21',oracle_inner_sha256=SHA,registry_items=1333,declaring_classes=counts,default_stack_executions=1333,base_item_no_op_items=407,block_item_default_absent_container_mutations=block_absent,default_policy_allowed=407+block_absent,policy='Allow exact cul base return; implement cso only when effective CONTAINER component52 absent: set(CONTAINER,EMPTY) in place and skip world drop because previous value null. Present container values, csw Bundle and unknown classes remain unsupported.',native_comparison=False,scope='All1333 actual registry dispatch targets and default-stack hook calls; exact before/after STREAM stack bytes, unchanged count/removal/entity-world RNG and no game events. BlockItem absent-CONTAINER mutates the stack patch; it is not a no-op. Nonempty container/bundle spawning is not proved.',source_sha256={str(f.relative_to(ROOT)):digest(f)for f in [*sources,Path(__file__),fixture,bytecode]})
    (ROOT/'evidence/item-destroyed-audit.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps({k:v for k,v in report.items()if k not in ['source_sha256','scope','policy']}))
if __name__=='__main__':main()
