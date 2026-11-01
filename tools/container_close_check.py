#!/usr/bin/env python3
import pathlib,subprocess,json
r=pathlib.Path(__file__).resolve().parent.parent;o=r.parent/'mc121-oracle';j=pathlib.Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin');cp=':'.join([str(o/'server-inner-1.21.jar'),*[str(f) for f in sorted((o/'libraries').rglob('*.jar'))]]);exports=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED'];outdir=r/'target/container-close-oracle';outdir.mkdir(parents=True,exist_ok=True)
subprocess.run([str(j/'javac'),*exports,'-proc:none','-cp',cp,'-d',str(outdir),str(r/'tools/container_oracle.java'),str(r/'tools/container_close_oracle.java')],check=True)
def s(n,c):return {'id':'minecraft:'+n,'count':c}
cases=[]
for creative in [False,True]:
 for full in [False,True]:
  for selected in [0,5]:
   for name,carried,grid in [('empty',None,[]),('cursor',s('stone',10),[]),('grid',None,[s('oak_planks',2)]*4),('both',s('stone',10),[s('oak_planks',2),s('stone',5),None,s('oak_log',3)])]:
    slots=[None]*46
    if full:
     for i in range(9,45):slots[i]=s('stone',64)
     slots[45]=s('stone',60)
    slots[1:5]=grid+[None]*(4-len(grid));slots[0]=s('oak_button',1) if grid else None
    cases.append({'name':name+'/'+str((creative,full,selected)),'creative':creative,'selected':selected,'slots':slots,'carried':carried,'clicks':[]})
p=subprocess.run([str(j/'java'),*exports,'-cp',str(outdir)+':'+cp,'oracle.container_close_oracle'],input='\n'.join(map(json.dumps,cases))+'\n',capture_output=True,text=True,check=True);outputs=[json.loads(l.split('ORACLE:',1)[1]) for l in p.stdout.splitlines() if 'ORACLE:'in l];assert len(outputs)==len(cases),(len(outputs),len(cases),p.stdout[-1000:],p.stderr[-1000:]);errors=[]
for c,v in zip(cases,outputs):c.update(v);errors.extend([(c['name'],v['error'])] if 'error'in v else [])
(r/'tools/container_close_oracle_vectors.json').write_text(json.dumps({'minecraft':'1.21','scope':'actual InventoryMenu.removed normalaliveplayer; environmental equipment/drop hooks; noawards/network','cases':cases},separators=(',',':'))+'\n');print(json.dumps({'cases':len(cases),'errors':errors}))
