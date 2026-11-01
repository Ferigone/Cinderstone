#!/usr/bin/env python3
"""Reproduce original Java graph corpus, then compare native debug/release."""
from pathlib import Path
import hashlib,json,os,subprocess,sys
ROOT=Path(__file__).resolve().parents[1]
def main():
    subprocess.run([sys.executable,ROOT/'tools/entity_ticking_range_generate.py'],cwd=ROOT,check=True,stdout=subprocess.DEVNULL)
    environment=os.environ.copy();environment.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup',CARGO_INCREMENTAL='0');environment['PATH']=environment['CARGO_HOME']+'/bin:'+environment['PATH']
    checks=[];runtime=ROOT/'runtime/entity_ticking_range'
    for profile,flags in [('debug',[]),('release',['--release'])]:
        command=['cargo','test',*flags,'--lib','entity_ticking_range::tests','--','--nocapture']
        result=subprocess.run(command,cwd=ROOT,env=environment,text=True,capture_output=True);log=result.stdout+result.stderr;(runtime/(profile+'-native.log')).write_text(log)
        if result.returncode:raise RuntimeError(log[-10000:])
        assert 'test result: ok. 1 passed; 0 failed;'in log
        checks.append(dict(profile=profile,command=command,exit_code=result.returncode,log_sha256=hashlib.sha256(log.encode()).hexdigest()))
    original=json.loads((ROOT/'evidence/entity-ticking-range-oracle.json').read_text())
    report=dict(ok=True,minecraft='1.21',oracle_inner_sha256=original['oracle_inner_sha256'],cases=original['cases'],states=original['states'],stored_level_rows=original['stored_level_rows'],computed_rows=original['computed_rows'],queued_entries=original['queued_entries'],fixture_sha256=original['fixture_sha256'],oracle_source_sha256=original['oracle_source_sha256'],generator_source_sha256=original['generator_source_sha256'],native_source_sha256=hashlib.sha256((ROOT/'src/entity_ticking_range.rs').read_bytes()).hexdigest(),checker_source_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),checks=checks,scope=original['scope'],not_verified=original['not_verified'])
    (ROOT/'evidence/entity-ticking-range-conformance.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
