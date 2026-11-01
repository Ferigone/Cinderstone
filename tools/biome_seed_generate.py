#!/usr/bin/env python3
"""Generate exact signed BiomeManager seed hashes from original Java methods."""
from pathlib import Path
import hashlib
import json
import random
import subprocess

ROOT=Path(__file__).resolve().parents[1]
ORACLE=ROOT.parent/'mc121-oracle'
JAVA=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
BUILD=ROOT/'target/biome-seed-oracle'
BUILD.mkdir(parents=True,exist_ok=True)
jar=ORACLE/'server-inner-1.21.jar'
digest=hashlib.sha256(jar.read_bytes()).hexdigest()
assert digest=='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
classpath=':'.join([str(jar),*[str(p) for p in sorted((ORACLE/'libraries').rglob('*.jar'))]])
subprocess.run([str(JAVA/'javac'),'-proc:none','-d',str(BUILD),str(ROOT/'tools/biome_seed_oracle.java')],check=True)
rng=random.Random(7673953)
seeds=[0,1,-1,2**63-1,-2**63,2**31-1,-2**31,2**32]+[rng.randrange(-2**63,2**63) for _ in range(1024)]
process=subprocess.run([str(JAVA/'java'),'-cp',str(BUILD)+':'+classpath,'oracle.biome_seed_oracle'],input=''.join(str(s)+'\n' for s in seeds),capture_output=True,text=True)
if process.returncode:
    raise RuntimeError(process.stderr[-4000:])
values=[[int(v) for v in line.split(',')] for line in process.stdout.splitlines() if ',' in line]
assert [v[0] for v in values]==seeds
path=ROOT/'fixtures/biome-seed-vectors.json'
path.write_text(json.dumps(values,separators=(',',':'))+'\n')
(ROOT/'evidence/biome-seed-oracle.json').write_text(json.dumps({'minecraft':'1.21','queries':len(values),'method':'unchanged BiomeManager.obfuscateSeed via reflection','oracle_inner_sha256':digest,'vectors_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'native_comparison_pending':True},indent=2)+'\n')
print(json.dumps({'original_java_queries':len(values)}))
