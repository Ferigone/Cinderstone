#!/usr/bin/env python3
"""Check native NBT/Anvil using actual Java1.21, bundled lz4-java and Node NBT."""
from pathlib import Path
import argparse,hashlib,json,os,subprocess

PROJECT=Path(__file__).resolve().parents[1]

def run(argv,**kwargs):
    result=subprocess.run([str(a) for a in argv],cwd=PROJECT,text=True,capture_output=True,check=True,**kwargs)
    return result.stdout

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--oracle-dir',type=Path,default=PROJECT.parent/'mc121-oracle')
    parser.add_argument('--java-home',type=Path,default=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1'))
    parser.add_argument('--probe',type=Path,default=PROJECT/'target/debug/anvil_probe')
    args=parser.parse_args()
    jar=args.oracle_dir/'server-inner-1.21.jar'
    digest=hashlib.sha256(jar.read_bytes()).hexdigest()
    assert digest=='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01','wrong Java oracle'
    output=PROJECT/'runtime/anvil-conformance'
    output.mkdir(parents=True,exist_ok=True)
    classes=output/'classes';classes.mkdir(exist_ok=True)
    libraries=sorted((args.oracle_dir/'libraries').rglob('*.jar'))
    lz4=next(p for p in libraries if p.name=='lz4-java-1.8.0.jar')
    run([args.java_home/'bin/javac','-cp',lz4,'-d',classes,PROJECT/'tools/anvil_Oracle.java'])
    classpath=os.pathsep.join(map(str,[classes,jar,*libraries]))
    def java(*argv):return run([args.java_home/'bin/java','-cp',classpath,'oracle.anvil_Oracle',*argv])
    records=[]
    for region in sorted((args.oracle_dir/'world/region').glob('r.*.*.mca')):
        parts=region.name.split('.')
        folder=output/f'region.{parts[1]}.{parts[2]}'
        record=json.loads(run([args.probe,'inspect',region,folder]))
        records.append(record)
        java('decode-lz4',folder/'sample.lz4',folder/'java-decoded-lz4.raw')
        assert (folder/'java-decoded-lz4.raw').read_bytes()==(folder/'sample.raw').read_bytes()
        java('encode-lz4',folder/'sample.raw',folder/'java-encoded.lz4')
        run([args.probe,'decode-lz4',folder/'java-encoded.lz4',folder/'rust-decoded-java-lz4.raw'])
        assert (folder/'rust-decoded-java-lz4.raw').read_bytes()==(folder/'sample.raw').read_bytes()
        java('read-region',record['edited_region'],*record['edited_chunk'],folder/'java-read-region.nbt')
    assert records,'no original region files inspected'
    external=json.loads(run([args.probe,'external',output/'external']))
    java('read-region',external['region'],*external['chunk'],output/'java-read-external.nbt')
    (output/'external.json').write_text(json.dumps(external,indent=2)+'\n')
    (output/'records.json').write_text(json.dumps(records,indent=2)+'\n')
    independent=json.loads(run(['node',PROJECT/'tools/anvil_compare.mjs',output]))
    result={**independent,'oracle_inner_jar_sha256':digest,'original_regions_byte_identical':True,
       'untouched_raw_sector_allocations_preserved':sum(r['preserved_unchanged_sector_allocations'] for r in records),
       'lz4_java_to_rust_and_rust_to_java':True,'data_version':3953}
    (PROJECT/'evidence/anvil-conformance.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result,indent=2))

if __name__=='__main__':main()
