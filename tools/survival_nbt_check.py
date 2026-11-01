#!/usr/bin/env python3
"""Actual signed Minecraft NbtIo read/write, including numeric-tag zero factories."""
from pathlib import Path
import hashlib,json,os,struct,subprocess
ROOT=Path(__file__).resolve().parents[1];O=ROOT.parent/'mc121-oracle';JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
def main():
    jar=O/'server-inner-1.21.jar';sha=hashlib.sha256(jar.read_bytes()).hexdigest();assert sha=='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
    runtime=ROOT/'runtime/survival-nbt-factory';classes=runtime/'classes';classes.mkdir(parents=True,exist_ok=True);cp=os.pathsep.join(map(str,[classes,jar,*sorted((O/'libraries').rglob('*.jar'))]));subprocess.run([JDK/'javac','-proc:none','-cp',cp,'-d',classes,ROOT/'tools/anvil_Oracle.java'],check=True,capture_output=True)
    vectors=[]
    for kind,width,values in [(5,4,[0,0x80000000,1,0x80000001,0x007fffff,0x00800000,0x3f800000,0xbf800000,0x7f7fffff,0x7f800000,0xff800000,0x7fc00000,0x7fa12345,0xffc12345]),(6,8,[0,0x8000000000000000,1,0x8000000000000001,0x000fffffffffffff,0x0010000000000000,0x3ff0000000000000,0xbff0000000000000,0x7fefffffffffffff,0x7ff0000000000000,0xfff0000000000000,0x7ff8000000000000,0x7ff123456789abcd,0xfff8123456789abc])]:
        for bits in values:
            raw=bytes([10,0,0,kind,0,5])+b'value'+bits.to_bytes(width,'big')+bytes([0]);inp=runtime/'input.nbt';out=runtime/'output.nbt';inp.write_bytes(raw);subprocess.run([JDK/'java','-cp',cp,'oracle.anvil_Oracle','read-nbt',inp,out],check=True,capture_output=True);vectors.append(dict(input=raw.hex(),expected=out.read_bytes().hex()))
    result=dict(minecraft='1.21',oracle_inner_sha256=sha,oracle='unchanged uo.read/write and FloatTag/DoubleTag.valueOf via tools/anvil_Oracle.java read-nbt',cases=vectors)
    p=ROOT/'fixtures/survival-nbt-factory-vectors.json';p.write_text(json.dumps(result,separators=(',',':'))+'\n');print(json.dumps(dict(cases=len(vectors),negative_zero_float_output=vectors[1]['expected'],negative_zero_double_output=vectors[15]['expected'],fixture_sha256=hashlib.sha256(p.read_bytes()).hexdigest()),indent=2))
if __name__=='__main__':main()
