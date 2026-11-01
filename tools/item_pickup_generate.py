#!/usr/bin/env python3
"""Original playerTouch with real original Inventory.add callback states."""
from pathlib import Path
import hashlib,itertools,json,os,subprocess
ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle';JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin');SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def stack(name,count=1,components=None):
    out=dict(id='minecraft:'+name,count=count)
    if components:out['components']=components
    return out
def query(name,count,mode,creative):
    slots=[None]*46
    if mode!='empty':
        for i in [*range(9,45),45]:slots[i]=stack('stone',64)
    if mode=='partial':slots[36]=stack('stone',60)
    if mode=='one_free':slots[36]=None
    components={'minecraft:damage':1,'minecraft:max_stack_size':99}if name=='iron_pickaxe'else None
    return dict(name=f'{name}/{count}/{mode}/{creative}',creative=creative,selected=0,slots=slots,pickup=stack(name,count,components),pickup_delay=0,target='none',client=False,removed=False)
def main():
    jar=ORACLE/'server-inner-1.21.jar';assert hashlib.sha256(jar.read_bytes()).hexdigest()==SHA
    cases=[query(*values)for values in itertools.product(['stone','ender_pearl','iron_pickaxe'],[1,5,64,99],['empty','full','partial','one_free'],[False,True])]
    for delay,target,client,removed in itertools.product([-1,0,1,32767],['none','match','other'],[False,True],[False,True]):
        q=query('stone',5,'empty',False);q.update(pickup_delay=delay,target=target,client=client,removed=removed);cases.append(q)
    for client,removed in itertools.product([False,True],[False,True]):
        q=query('stone',1,'empty',False);q.update(pickup=None,client=client,removed=removed);cases.append(q)
    runtime=ROOT/'runtime/item-pickup';runtime.mkdir(parents=True,exist_ok=True);cp=os.pathsep.join(map(str,[runtime,jar,*sorted((ORACLE/'libraries').rglob('*.jar'))]));flags=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED']
    sources=[ROOT/'tools'/n for n in ['survival_oracle.java','item_hurt_oracle.java','container_oracle.java','item_pickup_oracle.java']]
    compile=subprocess.run([JDK/'javac',*flags,'-proc:none','-cp',cp,'-d',runtime,*sources],text=True,capture_output=True)
    if compile.returncode:raise RuntimeError(compile.stdout+compile.stderr)
    java=subprocess.run([JDK/'java',*flags,'-Xmx1G','-cp',cp,'oracle.item_pickup_oracle'],input=''.join(json.dumps(q)+'\n'for q in cases),text=True,capture_output=True);(runtime/'oracle.log').write_text(java.stdout+java.stderr)
    if java.returncode:raise RuntimeError((java.stdout+java.stderr)[-7000:])
    rows=[json.loads(s[7:])for s in java.stdout.splitlines()if s.startswith('ORACLE:')];assert len(rows)==len(cases),(len(rows),len(cases))
    data=dict(minecraft='1.21',oracle_inner_sha256=SHA,scope='Original cjh.playerTouch calls unchanged real cmw.Inventory.add via observed invokespecial boundary. Outer player collect/stat/onPickup and level onRemove callbacks record source state; actual packet/menu/stat-counter side effects are not executed.',cases=[dict(query=q,expected=r)for q,r in zip(cases,rows)])
    fixture=ROOT/'fixtures/item-pickup-vectors.json';fixture.write_text(json.dumps(data,separators=(',',':'))+'\n')
    print(json.dumps(dict(ok=True,cases=len(cases),callbacks=sum(len(r['events'])for r in rows),file=str(fixture),sha256=hashlib.sha256(fixture.read_bytes()).hexdigest())))
if __name__=='__main__':main()
