#!/usr/bin/env python3
"""Regenerate original loading/throttler fixtures and compare native profiles."""
from pathlib import Path
import hashlib,json,os,subprocess,sys
ROOT=Path(__file__).resolve().parents[1]
def main():
    subprocess.run([sys.executable,ROOT/'tools/chunk_loading_tickets_generate.py'],cwd=ROOT,check=True,stdout=subprocess.DEVNULL)
    env=os.environ.copy();env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup',CARGO_INCREMENTAL='0');env['PATH']=env['CARGO_HOME']+'/bin:'+env['PATH']
    checks=[];runtime=ROOT/'runtime/chunk_loading_tickets'
    for name,flags in [('debug',[]),('release',['--release'])]:
        command=['cargo','test',*flags,'--lib','chunk_loading_tickets::tests','--','--nocapture'];result=subprocess.run(command,cwd=ROOT,env=env,text=True,capture_output=True);log=result.stdout+result.stderr;(runtime/(name+'-native.log')).write_text(log)
        if result.returncode:raise RuntimeError(log[-12000:])
        assert 'test result: ok. 1 passed; 0 failed;'in log
        checks.append(dict(profile=name,command=command,exit_code=0,log_sha256=hashlib.sha256(log.encode()).hexdigest()))
    original=json.loads((ROOT/'evidence/chunk-loading-tickets-oracle.json').read_text())
    report=dict(ok=True,minecraft='1.21',oracle_inner_sha256=original['oracle_inner_sha256'],cases=original['cases'],states=original['states'],scheduling_events=original['scheduling_events'],max_acquired=original['max_acquired'],fixture_sha256=original['fixture_sha256'],oracle_source_sha256=original['oracle_source_sha256'],generator_source_sha256=original['generator_source_sha256'],native_source_sha256=hashlib.sha256((ROOT/'src/chunk_loading_tickets.rs').read_bytes()).hexdigest(),checker_source_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),audit_sha256=hashlib.sha256((ROOT/'tools/chunk_loading_tickets_audit.md').read_bytes()).hexdigest(),checks=checks,scope=original['scope'],not_verified=[s for s in original['not_verified']if s!='native port conformance until independently compared']+['native ara ticket creation timestamps are not compared; full propagated and pending ara graph is compared; all supported kinds timeout0','future completion/registration conditions remain explicit host callbacks, not a new implicit visibility policy'])
    (ROOT/'evidence/chunk-loading-tickets-conformance.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
