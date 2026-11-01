#!/usr/bin/env python3
"""Regenerate exact original GameRules corpus and compare both Rust profiles."""
from pathlib import Path
import hashlib,json,os,subprocess,sys
ROOT=Path(__file__).resolve().parents[1]
def main():
    subprocess.run([sys.executable,ROOT/'tools/world_rules_generate.py'],cwd=ROOT,check=True,stdout=subprocess.DEVNULL)
    env=os.environ.copy();env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup',CARGO_INCREMENTAL='0');env['PATH']=env['CARGO_HOME']+'/bin:'+env['PATH'];checks=[];runtime=ROOT/'runtime/world_rules'
    for profile,flags in [('debug',[]),('release',['--release'])]:
        command=['cargo','test',*flags,'--lib','world_rules::tests','--','--nocapture'];result=subprocess.run(command,cwd=ROOT,env=env,text=True,capture_output=True);log=result.stdout+result.stderr;(runtime/(profile+'-native.log')).write_text(log)
        if result.returncode:raise RuntimeError(log[-10000:])
        assert 'test result: ok. 2 passed; 0 failed;'in log
        checks.append(dict(profile=profile,command=command,exit_code=0,log_sha256=hashlib.sha256(log.encode()).hexdigest()))
    original=json.loads((ROOT/'evidence/world-rules-oracle.json').read_text());report={key:original[key]for key in ['minecraft','oracle_inner_sha256','catalog_size','cases','states','bmp_digit_ranges','bmp_units_exhaustively_checked','fixture_sha256','facts_sha256','oracle_source_sha256','generator_source_sha256','scope','not_verified']};report.update(ok=True,native_source_sha256=hashlib.sha256((ROOT/'src/world_rules.rs').read_bytes()).hexdigest(),checker_source_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),checks=checks)
    (ROOT/'evidence/world-rules-conformance.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
