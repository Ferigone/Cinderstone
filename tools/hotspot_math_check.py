#!/usr/bin/env python3
"""Generate pinned HotSpot oracle corpus and check the native Rust kernel bits."""
import pathlib,json,subprocess,hashlib,os
r=pathlib.Path(__file__).resolve().parent.parent;j=pathlib.Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin');build=r/'target/hotspot-math-oracle';build.mkdir(parents=True,exist_ok=True)
drops=json.loads((r/'tools/item_entity_drop_vectors.json').read_text())['cases'];seedfile=build/'drop-seeds.txt';seedfile.write_text(''.join(f"{v['seed']},{str(v['around']).lower()}\n" for v in drops))
subprocess.run([str(j/'javac'),'-proc:none','-d',str(build),str(r/'tools/hotspot_math_oracle.java')],check=True)
result=json.loads(subprocess.check_output([str(j/'java'),'-Xmx128M','-cp',str(build),'hotspot_math_oracle',str(r/'fixtures/hotspot_math_vectors.bin'),str(seedfile)],text=True))
assert result['runtime']=='21.0.13.0.2+1-LTS' and result['arch']=='amd64',result
result['jvm_library_sha256']=hashlib.sha256((j.parent/'lib/server/libjvm.so').read_bytes()).hexdigest()
result['source']=json.loads((r/'tools/hotspot_math_sources/source.json').read_text());result['fixture_sha256']=hashlib.sha256((r/'fixtures/hotspot_math_vectors.bin').read_bytes()).hexdigest();result['drop_fixture_sha256']=hashlib.sha256((r/'tools/item_entity_drop_vectors.json').read_bytes()).hexdigest();env=os.environ.copy();env.setdefault('CARGO_HOME','/root/.local/share/mc-rust-toolchain/cargo');env.setdefault('RUSTUP_HOME','/root/.local/share/mc-rust-toolchain/rustup');rustc=pathlib.Path(env['CARGO_HOME'])/'bin/rustc'
for profile,flags in [('debug',[]),('optimized',['-C','opt-level=3','-C','lto=thin'])]:
 binary=build/('native-'+profile);subprocess.run([str(rustc),'--edition','2021','--test',str(r/'src/hotspot_math.rs'),*flags,'-o',str(binary)],check=True,env=env);subprocess.run([str(binary)],check=True)
result.update({'ok':True,'bit_comparisons_per_profile':2*result['cases'],'profiles':['debug','optimized'],'differences':0,'max_ulp_difference':0,'native_source_sha256':hashlib.sha256((r/'src/hotspot_math.rs').read_bytes()).hexdigest(),'domain':'finite[0,f32::TAU] and negative zero; validated against x86_64 pinned HotSpot, not other JVM/math implementations'})
(r/'evidence/hotspot-math-conformance.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
