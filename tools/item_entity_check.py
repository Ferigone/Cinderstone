#!/usr/bin/env python3
"""Reference vectors call the unchanged signed Minecraft1.21 classes."""
import argparse,hashlib,json,os,random,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
JAVA=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
ORACLE=ROOT.parent/'mc121-oracle'
OUT=Path('/tmp/mc121-item-oracle')

def command():
    OUT.mkdir(exist_ok=True);jar=ORACLE/'server-inner-1.21.jar'
    assert hashlib.sha256(jar.read_bytes()).hexdigest()=='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
    cp=os.pathsep.join(map(str,[OUT,jar,*(ORACLE/'libraries').rglob('*.jar')]))
    flags=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED']
    subprocess.run([str(JAVA/'javac'),*flags,'-proc:none','-cp',cp,'-d',OUT,ROOT/'tools/item_entity_oracle.java'],check=True)
    return [str(JAVA/'java'),*flags,'-cp',cp,'oracle.item_entity_oracle']

def main():
    rng=random.Random(12176758);queries=[]
    angles=[(0,0),(90,0),(-90,0),(180,90),(45,-90),(1e6,-1e6),(-360,360)]
    for i in range(1200):
        yaw,pitch=angles[i] if i<len(angles) else (rng.uniform(-180,180),rng.uniform(-90,90))
        for around in [False,True]:
            q={'seed':rng.randrange(-2**63,2**63),'yaw':yaw,'pitch':pitch,'x':rng.choice([8.5,-30e6,30e6,-0.125]),'y':rng.choice([-64,64,319,0.125]),'z':rng.choice([8.5,-30e6,30e6,-0.125]),'eye_height':rng.choice([1.62,1.27,.4,.2]),'around':around,'trace':bool(i%2),'stack':None if i%101==0 else {'id':'minecraft:stone','count':rng.randint(1,64)}};queries.append(q)
    result=subprocess.run(command(),input=''.join(json.dumps(q)+'\n' for q in queries),capture_output=True,text=True,check=True)
    rows=[json.loads(line.split('ORACLE:',1)[1]) for line in result.stdout.splitlines() if 'ORACLE:' in line]
    assert len(rows)==len(queries),result.stderr
    errors=[r for r in rows if 'error' in r];assert not errors,errors[:3]
    path=ROOT/'tools/item_entity_drop_vectors.json';path.write_text(json.dumps({'minecraft':'1.21','oracle_inner_sha256':'8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01','cases':rows},separators=(',',':'))+'\n')
    print(json.dumps({'ok':True,'actual_java_player_drop_cases':len(rows),'empty_stack_cases':sum(r.get('empty',False) for r in rows),'output':str(path)}))
if __name__=='__main__':main()
