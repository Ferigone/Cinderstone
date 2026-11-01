#!/usr/bin/env python3
"""Check the native disk/Netty distinction against original NbtIo byte vectors."""
from pathlib import Path
import hashlib
import json
import os
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
JAR = ROOT.parent / "mc121-oracle/server-inner-1.21.jar"
JAR_SHA = "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    assert sha(JAR) == JAR_SHA
    fixture_path = ROOT / "fixtures/nbt-float-vectors.json"
    fixture = json.loads(fixture_path.read_text())
    runtime = ROOT / "runtime/nbt-float"
    runtime.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    env.update(CARGO_INCREMENTAL="0",
               CARGO_HOME="/root/.local/share/mc-rust-toolchain/cargo",
               RUSTUP_HOME="/root/.local/share/mc-rust-toolchain/rustup")
    env["PATH"] = env["CARGO_HOME"] + "/bin:" + env["PATH"]
    report = {
        "ok": False,
        "minecraft": "1.21",
        "oracle_inner_sha256": JAR_SHA,
        "rust_build_profile": "debug",
        "rust_release_verified": False,
        "float_cases": len(fixture["cases"]),
        "nested_cases": len(fixture.get("nested", [])),
        "vectors_sha256": sha(fixture_path),
        "native_sources_sha256": {
            name: sha(ROOT / f"src/{name}.rs")
            for name in ["nbt", "metadata", "storage"]
        },
        "scope": "Original NbtIo and Netty ByteBufOutputStream floating NBT; "
                 "signed-zero factories; anonymous network and disk output; "
                 "named disk output; network budgets and metadata/block-entity callsites",
    }
    destination = ROOT / "evidence/nbt-float-conformance.json"
    try:
        counts = {}
        for name in ["nbt", "metadata", "storage"]:
            result = subprocess.run(
                ["cargo", "test", "--lib", f"{name}::tests", "--", "--nocapture"],
                cwd=ROOT, env=env, text=True, stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT, check=False,
            )
            (runtime / f"native-{name}.log").write_text(result.stdout)
            assert result.returncode == 0, f"{name} tests failed; see runtime/nbt-float"
            matches = re.findall(r"test result: ok\. (\d+) passed", result.stdout)
            assert matches and int(matches[-1]) > 0, f"no {name} tests executed"
            counts[name] = int(matches[-1])
        report["native_tests_passed"] = counts
        result = subprocess.run(
            ["cargo", "clippy", "--lib", "--tests", "--", "-D", "warnings"],
            cwd=ROOT, env=env, text=True, stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT, check=False,
        )
        (runtime / "clippy.log").write_text(result.stdout)
        assert result.returncode == 0, "clippy failed; see runtime/nbt-float/clippy.log"
        report["clippy"] = True
        report["ok"] = True
    finally:
        destination.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
