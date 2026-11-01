#!/usr/bin/env python3
"""Record original OPTIONAL_STREAM_CODEC canonical Slot bytes and bounded policy."""
from pathlib import Path
import base64,collections,hashlib,json,os,random,struct,subprocess

ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle';JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def vi(n):
 n&=0xffffffff;out=bytearray()
 while n>127:out.append((n&127)|128);n>>=7
 out.append(n);return bytes(out)
def b64(b):return base64.b64encode(b).decode()
def unb64(b):return base64.b64decode(b)
def slot(item=1,added=(),removed=(),count=1):return vi(count)+vi(item)+vi(len(added))+vi(len(removed))+b''.join(vi(c)+payload for c,payload in added)+b''.join(vi(c)for c in removed)
def readvi(raw,i):
 value=0
 for shift in range(0,35,7):
  b=raw[i];i+=1;value|=(b&127)<<shift
  if b<128:return value,i
 raise ValueError('varint')
def build_cases():
 prototype=json.loads((ROOT/'fixtures/item-components-1.21.json').read_text())['prototypes'];vectors=json.loads((ROOT/'tools/inventory_vectors.json').read_text())['vectors'];cases=[]
 def add(name,group,raw,item=None,component=None,payload=None):
  query=dict(bytes=b64(raw))
  if component is not None and payload is not None:query.update(component_id=component,component_payload=b64(payload))
  cases.append(dict(name=name,group=group,query=query,**({'item_id':item,'component_id':component}if item is not None and component is not None else{})))
 add('ender_pearl/redundant_max_stack_size16','smoke',unb64('AeEHAQABEA=='),993,1,vi(16))
 for item,components in sorted(prototype.items(),key=lambda p:int(p[0])):
  if int(item)==0:continue
  for component,payload in sorted(components.items(),key=lambda p:int(p[0])):
   i,c=int(item),int(component);p=unb64(payload);add(f'prototype/{i}/{c}','prototype',slot(i,[(c,p)]),i,c,p)
 payloads=collections.defaultdict(list)
 for v in vectors:
  raw=unb64(v['slot_base64']);add(v['name'],'independent117',raw)
  try:
   count,p=readvi(raw,0)
   if count<=0:continue
   item,p=readvi(raw,p);added,p=readvi(raw,p);removed,p=readvi(raw,p)
   if added==1 and removed==0:
    c,p=readvi(raw,p);payloads[c].append(raw[p:])
  except(IndexError,ValueError):pass
 for item in [1,993,443,625,748,835,1043,1230,1332]:
  for c in range(57):add(f'remove/{item}/{c}','removal',slot(item,removed=[c]))
 for c,variants in sorted(payloads.items()):
  p0,p1=variants[0],variants[-1]
  add(f'duplicate/{c}/same','duplicate',slot(1,[(c,p0),(c,p0)]))
  add(f'duplicate/{c}/last','duplicate',slot(1,[(c,p0),(c,p1)]))
  add(f'duplicate/{c}/remove','duplicate',slot(1,[(c,p0),(c,p1)],removed=[c,c]))
 for item in [1,993,443,748]:
  adds=[(int(c),unb64(p))for c,p in list(prototype[str(item)].items())[:4]]
  add(f'ordering/{item}/forward','ordering',slot(item,adds))
  add(f'ordering/{item}/reverse','ordering',slot(item,list(reversed(adds))))
  add(f'ordering/{item}/removals','ordering',slot(item,adds,[c for c,_ in reversed(adds)]))
 child=unb64('AeEHAQABEA==');canonical_child=unb64('AeEHAAA=')
 for c in [29,30,52]:
  add(f'nested/{c}/redundant_child','nested',slot(1,[(c,vi(1)+child)]))
  add(f'nested/{c}/canonical_child','nested',slot(1,[(c,vi(1)+canonical_child)]))
  if c==52:
   add('nested/container/trailing_empty','nested',slot(1,[(c,vi(3)+child+b'\0\0')]))
   add('nested/container/all_empty','nested',slot(1,[(c,vi(3)+b'\0\0\0')]))
 food=vi(1)+struct.pack('>f',1.0)+b'\0'+struct.pack('>f',1.6)+b'\1'+child+vi(0)
 add('nested/food/redundant_conversion','nested',slot(1,[(20,food)]))
 # Anonymous NBT with original FloatTag/DoubleTag factory normalization.
 for kind,number in [(5,struct.pack('>I',0x80000000)),(6,struct.pack('>Q',0x8000000000000000)),(5,struct.pack('>I',0x7fa00001)),(6,struct.pack('>Q',0x7ff0000000000001))]:
  nbt=bytes([10,kind])+b'\0\1x'+number+b'\0';add(f'nbt/custom_data/{kind}/{number.hex()}','nbt',slot(1,[(0,nbt)]))
 for c in [5,6]:
  text=b'\10\0\5Hello';add(f'component/{c}/literal_string','text',slot(1,[(c,text)]))
  compound=b'\12\10\0\4text\0\5Hello\0';add(f'component/{c}/literal_compound','text',slot(1,[(c,compound)]))
 return cases
def main():
 jar=ORACLE/'server-inner-1.21.jar';assert hashlib.sha256(jar.read_bytes()).hexdigest()==SHA
 runtime=ROOT/'runtime/inventory-normalization';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True)
 cp=os.pathsep.join(map(str,[classes,jar,*sorted((ORACLE/'libraries').rglob('*.jar'))]));source=ROOT/'tools/inventory_normalization_oracle.java'
 compile=subprocess.run([JDK/'javac','-proc:none','-cp',cp,'-d',classes,source],capture_output=True,text=True)
 if compile.returncode:raise RuntimeError(compile.stdout+compile.stderr)
 cases=build_cases();transcript=''.join(json.dumps(c['query'],separators=(',',':'))+'\n'for c in cases)
 java=subprocess.run([JDK/'java','-Xmx1G','-cp',cp,'oracle.inventory_normalization_oracle',ROOT/'tools/crafting_metadata.json'],input=transcript,text=True,capture_output=True,timeout=180);(runtime/'oracle.log').write_text(java.stdout+java.stderr)
 if java.returncode:raise RuntimeError((java.stdout+java.stderr)[-6000:])
 assert 'SOURCE:'in java.stdout and jar.name in java.stdout and'ACCESS:originalpd'in java.stdout
 expected=[json.loads(line.split('ORACLE:',1)[1])for line in java.stdout.splitlines()if line.startswith('ORACLE:')];assert len(expected)==len(cases),(len(expected),len(cases))
 for c,e in zip(cases,expected):c['expected']=e
 golden=[c for c in cases if c['group']=='prototype'];assert len(golden)==8314,len(golden)
 errors=[(c['name'],c['expected'])for c in golden if not c['expected']['accepted']];assert not errors,errors[:3]
 for c in golden:assert c['expected']['component_consumed']==len(unb64(c['query']['component_payload'])),c
 header=dict(minecraft='1.21',protocol=767,oracle_inner_sha256=SHA,codec='cuq.OPTIONAL_STREAM_CODEC (cuq.h) originaldecode->originalencode; never ItemStack.CODEC',registry_context='Original pd.a bootstrap values assigned exact capturedconfiguration IDs in original MappedRegistry, merged with original built-in registries',configuration_sha256=hashlib.sha256((ROOT/'fixtures/configuration.json').read_bytes()).hexdigest(),registry_metadata_sha256=hashlib.sha256((ROOT/'tools/crafting_metadata.json').read_bytes()).hexdigest())
 vectors=ROOT/'fixtures/inventory_normalization_vectors.json';vectors.write_text(json.dumps(dict(**header,cases=cases),separators=(',',':'))+'\n')
 removable=[[c['item_id'],c['component_id']]for c in golden if c['expected']['redundant_removed']];retained=[[c['item_id'],c['component_id']]for c in golden if not c['expected']['redundant_removed']]
 policy=dict(minecraft='1.21',oracle_inner_sha256=SHA,source_fixture_sha256=hashlib.sha256((ROOT/'fixtures/item-components-1.21.json').read_bytes()).hexdigest(),oracle_source_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),vectors_sha256=hashlib.sha256(vectors.read_bytes()).hexdigest(),removable_prototypes=removable,retained_prototypes=retained)
 (ROOT/'fixtures/inventory_normalization_policy.json').write_text(json.dumps(policy,separators=(',',':'))+'\n')
 independent=[c for c in cases if c['group']=='independent117'];rejected=[dict(name=c['name'],error_class=c['expected'].get('error_class'),error=c['expected'].get('error'))for c in independent if not c['expected']['accepted']]
 # "Unknown registry key ... minecraft:probe" means an invalid value inside
 # an available registry. It must not be mistaken for a missing registry.
 registry_errors=[c for c in cases if not c['expected']['accepted'] and any(word in(c['expected'].get('error')or'').lower()for word in['missing registry:','missingregistry'])]
 changed=[dict(name=c['name'],input=c['query']['bytes'],canonical=c['expected']['canonical'])for c in independent if c['expected']['accepted']and c['query']['bytes']!=c['expected']['canonical']]
 groups={}
 for group in sorted({c['group']for c in cases}):
  rows=[c for c in cases if c['group']==group];groups[group]=dict(cases=len(rows),accepted=sum(c['expected']['accepted']for c in rows),canonical_changed=sum(c['expected']['accepted']and c['query']['bytes']!=c['expected']['canonical']for c in rows))
 report=dict(ok=True,**header,cases=len(cases),groups=groups,prototype_overrides_removed=len(removable),prototype_overrides_retained=len(retained),retained_by_component=dict(collections.Counter(c['component_id']for c in golden if not c['expected']['redundant_removed'])),registry_context_errors=len(registry_errors),independent117_exact_accepted=sum(c['expected']['accepted']and c['expected']['remaining']==0 for c in independent),independent117_prefix_only=[dict(name=c['name'],consumed=c['expected']['consumed'],remaining=c['expected']['remaining'])for c in independent if c['expected']['accepted']and c['expected']['remaining']!=0],independent117_rejections=rejected,independent117_canonical_changes=changed,vectors_sha256=hashlib.sha256(vectors.read_bytes()).hexdigest(),vectors_bytes=vectors.stat().st_size,policy_sha256=hashlib.sha256((ROOT/'fixtures/inventory_normalization_policy.json').read_bytes()).hexdigest(),not_verified=['Native Rust normalization; implementation agent must consume these STREAM vectors','All possible nondefault component values and arbitrary nested component combinations','Persisted ItemStack.CODEC and DataComponentPatch CODEC behavior'])
 (ROOT/'evidence/inventory-normalization-oracle.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
