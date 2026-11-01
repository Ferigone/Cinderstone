#!/usr/bin/env python3
"""Reproduce actual-Java fixtures, then native local-listener debug/release replay."""
from pathlib import Path
import argparse
import hashlib
import json
import os
import subprocess
import sys

ROOT=Path(__file__).resolve().parents[1]


def main():
    parser=argparse.ArgumentParser();parser.add_argument("--skip-oracle",action="store_true")
    args=parser.parse_args()
    if not args.skip_oracle:
        subprocess.run([sys.executable,ROOT/"tools/container_local_listener_generate.py"],cwd=ROOT,check=True)
    env=os.environ.copy();env.update(CARGO_HOME="/root/.local/share/mc-rust-toolchain/cargo",RUSTUP_HOME="/root/.local/share/mc-rust-toolchain/rustup",CARGO_INCREMENTAL="0")
    env["PATH"]=env["CARGO_HOME"]+"/bin:"+env["PATH"]
    runtime=ROOT/"runtime/container-local-listener";runtime.mkdir(parents=True,exist_ok=True)
    runs=[]
    for profile in ["debug","release"]:
        command=["cargo","test","--lib"]+(["--release"] if profile=="release" else [])+["actual_java_local_slot_listener_baselines_and_packet_order","--","--nocapture"]
        result=subprocess.run(command,cwd=ROOT,env=env,text=True,capture_output=True)
        (runtime/f"native-{profile}.log").write_text(result.stdout+result.stderr)
        assert result.returncode==0 and "1 passed" in result.stdout, (profile,result.stdout[-4000:]+result.stderr[-4000:])
        runs.append(dict(profile=profile,passed=1,failed=0))
    paths=["src/container.rs","src/inventory.rs","tools/container_local_listener_oracle.java","tools/container_local_listener_generate.py","tools/container_local_listener_tests.rs","tools/container_local_listener_audit.md","tools/container_local_listener_check.py","fixtures/container_local_listener.json"]
    oracle=json.loads((ROOT/"evidence/container-local-listener-oracle.json").read_text())
    report=dict(ok=True,minecraft="1.21",protocol=767,oracle_inner_sha256=oracle["oracle_inner_sha256"],
                actual_oracle_cases=oracle["cases"],actual_oracle_operations=oracle["operations"],
                native_cases=oracle["native_cases"],native_operations=oracle["native_operations"],runs=runs,
                comparison="All native-subset snapshots, local/remote/physical baselines, state IDs, original aqv$1 packet payloads, callback values/backing indices and callback/packet order. Expected values never drive native transitions.",
                not_verified=oracle["not_verified"],source_sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in paths})
    (ROOT/"evidence/container-local-listener-conformance.json").write_text(json.dumps(report,indent=2)+"\n")
    print(json.dumps(report,indent=2))


if __name__=="__main__":main()
