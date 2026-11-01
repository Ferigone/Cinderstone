#!/usr/bin/env python3
"""Actual Java LivingEntity comparison/hand-swap/equipment packet oracle."""
import hashlib,json,subprocess,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle';JAVA=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01';jar=ORACLE/'server-inner-1.21.jar';assert hashlib.sha256(jar.read_bytes()).hexdigest()==SHA
queries=json.loads(subprocess.check_output(['node',str(ROOT/'tools/equipment_inputs.mjs')],text=True,cwd=ROOT))
with tempfile.TemporaryDirectory(prefix='mc121-equipment-') as temp:
 temp=Path(temp);target=temp/'target';launch=temp/'launch';target.mkdir();launch.mkdir();cp=':'.join([str(jar),*[str(p)for p in sorted((ORACLE/'libraries').rglob('*.jar'))]])
 subprocess.run([str(JAVA/'javac'),'-proc:none','-cp',cp,'-d',str(target),str(ROOT/'tools/equipment_oracle.java')],check=True,capture_output=True)
 subprocess.run([str(JAVA/'javac'),'-d',str(launch),str(ROOT/'tools/entity_motion_launcher.java')],check=True,capture_output=True)
 run=subprocess.run([str(JAVA/'java'),'-cp',str(launch)+':'+cp,'oracle.entity_motion_launcher',str(target),'equipment_oracle'],input=''.join(json.dumps(q)+'\n'for q in queries),text=True,capture_output=True)
 if run.returncode:raise RuntimeError(run.stdout[-1500:]+run.stderr[-5000:])
 lines=[json.loads(line.split('ORACLE:',1)[1])for line in run.stdout.splitlines()if'ORACLE:'in line];assert len(lines)==len(queries)
 for query,line in zip(queries,lines):query['expected']=line
 out={'oracle':'unchanged Minecraft1.21 btn.equipmentHasChanged/handleHandSwap/handleEquipmentChanges; attribute callbacks excluded','jar_sha256':SHA,'input_producer':'independent minecraft-protocol1.68.0 Slot serializer','cases':queries};p=ROOT/'tools/equipment-java.json';p.write_text(json.dumps(out,separators=(',',':'))+'\n');print(json.dumps({'cases':len(queries),'packets':sum(len(q['expected']['packets'])for q in queries),'file':str(p)}))
