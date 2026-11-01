#!/usr/bin/env python3
"""Actual Java Slot stream-codec acceptance at nested Slot/container boundaries."""
import pathlib,subprocess,json,base64,struct
r=pathlib.Path(__file__).resolve().parent.parent;o=r.parent/'mc121-oracle';j=pathlib.Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin');cp=':'.join([str(o/'server-inner-1.21.jar'),*[str(f) for f in sorted((o/'libraries').rglob('*.jar'))]]);exports=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED'];out=r/'target/crafting-oracle';out.mkdir(parents=True,exist_ok=True)
subprocess.run([str(j/'javac'),*exports,'-proc:none','-cp',cp,'-d',str(out),str(r/'tools/crafting_oracle.java')],check=True)
def varint(n):
 b=bytearray()
 while n>127:b.append((n&127)|128);n>>=7
 b.append(n);return bytes(b)
cases=[]
def case(name,component,data):cases.append({'name':name,'bytes':base64.b64encode(bytes([1,1,1,0,component])+data).decode()})
for comp,name in [(29,'projectiles'),(30,'bundle'),(52,'container')]:
 case(name+'/empty-array',comp,b'\0');case(name+'/empty-item',comp,b'\1\0');case(name+'/stone-item',comp,b'\1\1\1\0\0')
for size in [255,256,257]:case('container/size'+str(size),52,varint(size)+b'\0'*size)
prefix=varint(1)+struct.pack('>f',1.0)+b'\0'+struct.pack('>f',1.6)
case('food/no-conversion',20,prefix+b'\0\0');case('food/empty-conversion',20,prefix+b'\1\0\0');case('food/stone-conversion',20,prefix+b'\1\1\1\0\0\0')
case('potion/no-custom-name',31,b'\0\0\0')
p=subprocess.run([str(j/'java'),*exports,'-cp',str(out)+':'+cp,'oracle.crafting_oracle'],input='\n'.join(json.dumps({'op':'slot_decode','bytes':c['bytes']}) for c in cases)+'\n',capture_output=True,text=True,check=True);results=[json.loads(l.split('ORACLE:',1)[1]) for l in p.stdout.splitlines() if 'ORACLE:'in l];assert len(results)==len(cases),(len(results),len(cases),p.stdout[-1000:],p.stderr[-1000:])
for c,e in zip(cases,results):c['accepted']=e.get('accepted',False);c['java']=e
(r/'tools/container_component_wire_vectors.json').write_text(json.dumps({'minecraft':'1.21','oracle':'actual unchanged cuq.OPTIONAL_STREAM_CODEC + component stream codecs','cases':cases},separators=(',',':'))+'\n');print(json.dumps({'cases':len(cases),'results':[(c['name'],c['accepted']) for c in cases]}))
