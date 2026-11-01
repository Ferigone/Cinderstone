#!/usr/bin/env python3
"""ActualJava sendPairingData vectors with unsent physical/metadata changes."""
import json,pathlib,subprocess,random
r=pathlib.Path(__file__).resolve().parent.parent;o=r.parent/'mc121-oracle';jdk=pathlib.Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin');cp=':'.join([str(o/'server-inner-1.21.jar'),*[str(f) for f in sorted((o/'libraries').rglob('*.jar'))]]);exports=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED'];out=r/'target/item-pairing-oracle';out.mkdir(parents=True,exist_ok=True)
subprocess.run([str(jdk/'javac'),*exports,'-proc:none','-cp',cp,'-d',str(out),str(r/'tools/item_entity_oracle.java'),str(r/'tools/item_pairing_oracle.java')],check=True)
rng=random.Random(3953767);cases=[]
velocities=[[0.,0.,0.],[.1,-.5,1.],[-3.9,3.9,-4.],[-0.000124999,0.000125,0.000125001],[1e-9,-1e-9,1e-300]]
for index in range(240):
 p={'x':rng.uniform(-64,64),'y':rng.uniform(-64,320),'z':rng.uniform(-64,64),'yaw':rng.uniform(-720,720),'pitch':rng.uniform(-180,180),'on_ground':False}
 q={'name':'pair'+str(index),'stack':{'id':'minecraft:stone' if index%2 else 'minecraft:iron_pickaxe','count':1+index%64},'position':p,'velocity':velocities[index%len(velocities)],'no_gravity':bool(index%3==0)}
 if index%4:q['physical_velocity']=[9.,8.,7.];q['physical_position']={**p,'x':p['x']+8.25,'yaw':p['yaw']+90.,'pitch':p['pitch']+90.}
 if index%5:q['unsent_stack']={'id':'minecraft:dirt','count':99}
 cases.append(q)
process=subprocess.run([str(jdk/'java'),*exports,'-cp',str(out)+':'+cp,'oracle.item_pairing_oracle'],input='\n'.join(map(json.dumps,cases))+'\n',capture_output=True,text=True,check=True)
results=[json.loads(line.split('ORACLE:',1)[1]) for line in process.stdout.splitlines() if 'ORACLE:' in line]
assert len(results)==len(cases),(len(results),len(cases),process.stderr[-2000:])
errors=[(case['name'],result['error']) for case,result in zip(cases,results) if 'error' in result]
assert not errors,errors[:10]
for c,v in zip(cases,results):c.update(v)
(r/'fixtures/item_pairing.json').write_text(json.dumps({'minecraft':'1.21','oracle':'actual unchanged ServerEntity.sendPairingData on actual ItemEntity, controlled environmental level only','cases':cases},separators=(',',':'))+'\n')
print(json.dumps({'cases':len(cases),'packets':sum(len(c['packets']) for c in cases),'errors':0}))
