#!/usr/bin/env python3
import pathlib,subprocess,json,hashlib
r=pathlib.Path(__file__).resolve().parent.parent;o=r.parent/'mc121-oracle';j=pathlib.Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin');cp=':'.join([str(o/'server-inner-1.21.jar'),*[str(f) for f in sorted((o/'libraries').rglob('*.jar'))]])
exports=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED']
subprocess.run([str(j/'javac'),*exports,'-proc:none','-cp',cp,'-d',str(r/'target/crafting-oracle'),str(r/'tools/crafting_oracle.java')],check=True)
p=subprocess.run([str(j/'java'),*exports,'-cp',str(r/'target/crafting-oracle')+':'+cp,'oracle.crafting_oracle'],input='{"op":"prototype_wire"}\n',capture_output=True,text=True,check=True)
out=[json.loads(l.split('ORACLE:',1)[1]) for l in p.stdout.splitlines() if 'ORACLE:' in l];assert len(out)==1,(p.stdout[-2000:],p.stderr[-1000:]);out=out[0];print(json.dumps({'items':len(out['prototypes']),'components':sum(map(len,out['prototypes'].values())),'errors':out['errors']}))
assert not out['errors'];data={'minecraft':'1.21','protocol':767,'source':'actual unchanged Minecraft1.21 DataComponentType.streamCodec encode(Item.getComponents), built-in RegistryAccess','jar_sha256':hashlib.sha256((o/'server-inner-1.21.jar').read_bytes()).hexdigest(),'prototypes':out['prototypes']}
(r/'fixtures/item-components-1.21.json').write_text(json.dumps(data,separators=(',',':'))+'\n')
