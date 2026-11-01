#!/usr/bin/env python3
"""Exhaustive Boolean/collision-mask oracle for unchanged Player.updatePlayerPose."""
import hashlib,json,subprocess,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle';JAVA=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01';jar=ORACLE/'server-inner-1.21.jar';assert hashlib.sha256(jar.read_bytes()).hexdigest()==SHA
cases=[{'flags':flags,'fits':mask,'current':(flags+mask)%18}for flags in range(256)for mask in range(64)]
with tempfile.TemporaryDirectory(prefix='mc121-pose-') as temp:
 temp=Path(temp);target=temp/'target';launch=temp/'launch';target.mkdir();launch.mkdir();cp=':'.join([str(jar),*[str(p)for p in sorted((ORACLE/'libraries').rglob('*.jar'))]])
 subprocess.run([str(JAVA/'javac'),'-proc:none','-cp',cp,'-d',str(target),str(ROOT/'tools/entity_pose_oracle.java')],check=True,capture_output=True)
 subprocess.run([str(JAVA/'javac'),'-d',str(launch),str(ROOT/'tools/entity_motion_launcher.java')],check=True,capture_output=True)
 run=subprocess.run([str(JAVA/'java'),'-cp',str(launch)+':'+cp,'oracle.entity_motion_launcher',str(target),'entity_pose_oracle'],input=''.join(f"{c['flags']}\t{c['fits']}\t{c['current']}\n"for c in cases),text=True,capture_output=True,check=True)
 lines=[line.split('ORACLE:',1)[1]for line in run.stdout.splitlines()if'ORACLE:'in line];assert len(lines)==len(cases)
 for case,line in zip(cases,lines):pose,calls=line.split('|');case.update(expected=int(pose),calls=[int(p)for p in calls.split(',')]if calls else[])
 path=ROOT/'tools/entity-pose.json';path.write_text(json.dumps({'oracle':'unchanged Minecraft1.21 cmx.fR/updatePlayerPose','jar_sha256':SHA,'cases':cases},separators=(',',':'))+'\n');print(json.dumps({'cases':len(cases),'file':str(path)}))
