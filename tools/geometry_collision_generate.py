#!/usr/bin/env python3
"""Generate a compact no-runtime-parser lookup from unchanged Minecraft classes."""
import hashlib,json,os,subprocess,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle';JAVA=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin');JAR=ORACLE/'server-inner-1.21.jar';SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
assert hashlib.sha256(JAR.read_bytes()).hexdigest()==SHA
classpath=os.pathsep.join([str(JAR),*map(str,sorted((ORACLE/'libraries').rglob('*.jar')))])
with tempfile.TemporaryDirectory(prefix='mc121-collision-')as temp:
    subprocess.run([str(JAVA/'javac'),'-proc:none','-classpath',classpath,'-d',temp,str(ROOT/'tools/geometry_collision_oracle.java')],check=True)
    result=subprocess.run([str(JAVA/'java'),'-cp',temp+os.pathsep+classpath,'oracle.GeometryCollisionOracle',temp+'/shapes.json',str(ROOT/'fixtures/geometry-sin-1.21.bin')],capture_output=True,text=True)
    if result.returncode:raise RuntimeError(result.stderr+result.stdout[-6000:])
    print(result.stdout.strip());data=json.loads(Path(temp+'/shapes.json').read_text())
metadata={key:value for key,value in data.items()if key not in ['shapes','states','dynamic_rows','special_shapes']}
metadata['states']=len(data['states']);metadata['shapes']=len(data['shapes']);metadata['static_states']=sum(flags==0 for _,flags in data['states']);metadata['generated_fixture']='fixtures/geometry-collisions-1.21.rs';metadata['sin_sha256']=hashlib.sha256((ROOT/'fixtures/geometry-sin-1.21.bin').read_bytes()).hexdigest()
text=['// Generated from original Minecraft1.21 BlockState geometry. Do not edit.','// JAR SHA256: '+SHA,'const COLLISION_SHAPES: &[&[[f64;6]]] = &[']
for shape in data['shapes']:text.append('    &'+str(shape).replace(' ','')+',')
text.append('];\nconst COLLISION_STATES: &[(u16,u8)] = &[')
for start in range(0,len(data['states']),24):text.append('    '+','.join(f'({shape},{flags})'for shape,flags in data['states'][start:start+24])+',')
text.append('];')
text.append('const DYNAMIC_COLLISION_STATES: &[DynamicCollisionState] = &[')
for state,kind,detail,horizontal,vertical,vertical_offset,raw in data['dynamic_rows']:
    text.append(f'    ({state},{kind},{detail},{horizontal}f32,{vertical}f32,{str(vertical_offset).lower()},&'+str(raw).replace(' ','')+'),')
text.append('];')
for name,boxes in data['special_shapes'].items():text.append('const '+name+': &[[f64;6]] = &'+str(boxes).replace(' ','')+';')
fixture=ROOT/'fixtures/geometry-collisions-1.21.rs';fixture.write_text('\n'.join(text)+'\n');metadata['generated_sha256']=hashlib.sha256(fixture.read_bytes()).hexdigest();(ROOT/'evidence/geometry-collision-catalog.json').write_text(json.dumps(metadata,indent=2)+'\n');print(json.dumps({key:value for key,value in metadata.items()if key!='context_dependent'},indent=2))
