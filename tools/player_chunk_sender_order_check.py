#!/usr/bin/env python3
"""Regenerate original fastutil/Guava corpus and compare the sibling Rust core."""
from pathlib import Path
import argparse,hashlib,json,os,subprocess,sys
ROOT=Path(__file__).resolve().parents[1]
def main():
    parser=argparse.ArgumentParser();parser.add_argument('--standalone',action='store_true');args=parser.parse_args()
    subprocess.run([sys.executable,ROOT/'tools/player_chunk_sender_order_generate.py'],cwd=ROOT,check=True,stdout=subprocess.DEVNULL)
    env=os.environ.copy();env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup',CARGO_INCREMENTAL='0');env['PATH']=env['CARGO_HOME']+'/bin:'+env['PATH'];runtime=ROOT/'runtime/player_chunk_sender_order';checks=[]
    for profile,flags in [('debug',[]),('release',['--release'])]:
        if args.standalone:
            deps=ROOT/'target/debug/deps';serde=max(deps.glob('libserde_json-*.rlib'),key=lambda p:p.stat().st_mtime);binary=runtime/('native-'+profile+'-tests');compile=['rustc','--edition=2021','--test','tools/player_chunk_sender_order_tests.rs','--extern','serde_json='+str(serde),'-L','dependency='+str(deps),'-o',str(binary),*(['-C','opt-level=3']if profile=='release'else[])]
            built=subprocess.run(compile,cwd=ROOT,env=env,text=True,capture_output=True)
            if built.returncode:raise RuntimeError(built.stdout+built.stderr)
            command=[str(binary),'--nocapture']
        else:command=['cargo','test',*flags,'--lib','player_chunk_sender::order::tests','--','--nocapture']
        result=subprocess.run(command,cwd=ROOT,env=env,text=True,capture_output=True);log=result.stdout+result.stderr;(runtime/(profile+'-native.log')).write_text(log)
        if result.returncode:raise RuntimeError(log[-15000:])
        assert 'test result: ok. 2 passed; 0 failed;'in log
        checks.append(dict(profile=profile,standalone=args.standalone,command=command,exit_code=0,log_sha256=hashlib.sha256(log.encode()).hexdigest()))
    original=json.loads((ROOT/'evidence/player-chunk-sender-order-oracle.json').read_text());report={k:original[k]for k in ['minecraft','oracle_inner_sha256','dependencies','cases','set_states','selection_cases','selection_items','actual_trim_calls','actual_fallback_cases','topk_tie_orders_different_from_stable_full_sort','stream_iterator_order_differences','fixture_sha256','oracle_source_sha256','generator_source_sha256','scope','not_verified']};report.update(ok=True,native_source_sha256=hashlib.sha256((ROOT/'src/player_chunk_sender_order.rs').read_bytes()).hexdigest(),checker_source_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),checks=checks)
    (ROOT/'evidence/player-chunk-sender-order-conformance.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
