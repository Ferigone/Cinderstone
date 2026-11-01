#!/usr/bin/env python3
"""Verify the native biome seed hash against actual Java vectors."""
from pathlib import Path
import hashlib,json,os,subprocess

root=Path(__file__).resolve().parents[1]
env=os.environ.copy()
env.setdefault('CARGO_HOME','/root/.local/share/mc-rust-toolchain/cargo')
env.setdefault('RUSTUP_HOME','/root/.local/share/mc-rust-toolchain/rustup')
env['PATH']=env['CARGO_HOME']+'/bin:'+env['PATH']
process=subprocess.run(['cargo','test','--lib','world::tests::biome_seed_hashes_match_original_java_biome_manager'],cwd=root,env=env,capture_output=True,text=True)
assert process.returncode==0,process.stdout+process.stderr
assert 'world::tests::biome_seed_hashes_match_original_java_biome_manager ... ok' in process.stdout
assert '1 passed; 0 failed' in process.stdout
oracle=json.loads((root/'evidence/biome-seed-oracle.json').read_text())
report={**oracle,'native_comparison_pending':False,'ok':True,'mismatches':0,
        'native_source_sha256':hashlib.sha256((root/'src/world.rs').read_bytes()).hexdigest(),
        'reproduce':'python3 tools/biome_seed_generate.py && python3 tools/biome_seed_check.py'}
(root/'evidence/biome-seed-conformance.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(report))
