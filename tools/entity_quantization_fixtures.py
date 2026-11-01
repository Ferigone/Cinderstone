#!/usr/bin/env python3
"""Capture exact quantization outputs from the unchanged Java 1.21 classes."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import random
import struct
import subprocess
import tempfile

ROOT=Path(__file__).resolve().parents[1]
EXPECTED='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
parser=argparse.ArgumentParser()
parser.add_argument('--oracle-dir',type=Path,default=ROOT.parent/'mc121-oracle')
parser.add_argument('--java-home',type=Path,default=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1'))
args=parser.parse_args()
jar=args.oracle_dir/'server-inner-1.21.jar'
assert hashlib.sha256(jar.read_bytes()).hexdigest()==EXPECTED
cases=[]
def coord(value):
    cases.append({'kind':'coordinate','bits':struct.pack('>d',value).hex()})
for integer in [-122880000000,-32769,-32768,-4096,-2,-1,0,1,2,4096,32767,32768,122879999999]:
    half=(integer+.5)/4096
    for value in [math.nextafter(half,-math.inf),half,math.nextafter(half,math.inf)]:coord(value)
for value in [0.,-0.,30_000_000.,-30_000_000.]:coord(value)
randomizer=random.Random(767121)
for _ in range(1000):coord(randomizer.uniform(-30_000_000,30_000_000))
angles=[0.,-0.,1.,-1.,90.,-90.,180.,-180.,360.,-360.,720.5,-720.5,3.4028234663852886e38,-3.4028234663852886e38]
for _ in range(500):angles.append(randomizer.uniform(-10000,10000))
for value in angles:
    for kind in ['angle','teleport_angle']:
        cases.append({'kind':kind,'bits':struct.pack('>f',value).hex()})
source=ROOT/'tools/entity_quantization_oracle.java'
with tempfile.TemporaryDirectory(prefix='mc121-entity-java-') as classes:
    subprocess.run([str(args.java_home/'bin/javac'),'-d',classes,str(source)],check=True)
    libraries=sorted((args.oracle_dir/'libraries').rglob('*.jar'))
    cp=':'.join([classes,str(jar),*[str(p) for p in libraries]])
    transcript=''.join(case['kind']+'\t'+case['bits']+'\n' for case in cases)
    result=subprocess.run([str(args.java_home/'bin/java'),'-cp',cp,'oracle.entity_quantization_oracle'],input=transcript,text=True,capture_output=True,check=True)
lines=result.stdout.splitlines()
assert len(lines)==len(cases),(len(lines),len(cases),result.stderr)
for case,line in zip(cases,lines):case['expected']=int(line)
output={'oracle':'actual Minecraft1.21 aig(VecDeltaCodec) and ayo(Mth)','jar_sha256':EXPECTED,'cases':cases}
path=ROOT/'tools/entity-quantization.json'
path.write_text(json.dumps(output,indent=2)+'\n')
print(json.dumps({'captured':len(cases),'destination':str(path),'jar_sha256':EXPECTED}))
