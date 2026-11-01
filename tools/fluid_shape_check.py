#!/usr/bin/env python3
"""Verify explicit native fluid-shape history against fresh original JVMs."""
from pathlib import Path
import hashlib
import json
import os
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
PIN = "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    assert sha(ROOT.parent / "mc121-oracle/server-inner-1.21.jar") == PIN
    fixture_path = ROOT / "fixtures/fluid-shape-vectors.json"
    fixture = json.loads(fixture_path.read_text())
    original = json.loads((ROOT / "evidence/fluid-shape-oracle.json").read_text())
    assert fixture["oracle_inner_sha256"] == PIN
    assert original["status"] == "PASS" and original["fixture_sha256"] == sha(fixture_path)
    queries = [q for scene in fixture["scenes"] for q in scene["queries"]]
    env = os.environ.copy()
    env.update(CARGO_INCREMENTAL="0", CARGO_HOME="/root/.local/share/mc-rust-toolchain/cargo",
               RUSTUP_HOME="/root/.local/share/mc-rust-toolchain/rustup")
    env["PATH"] = env["CARGO_HOME"] + "/bin:" + env["PATH"]
    runtime = ROOT / "runtime/fluid-shape"
    runtime.mkdir(parents=True, exist_ok=True)
    report = {
        "ok": False,
        "minecraft": "1.21",
        "oracle_inner_sha256": PIN,
        "rust_build_profile": "debug",
        "rust_release_verified": False,
        "canonical_fluid_states": len(fixture["states"]),
        "original_fresh_jvms": original["fresh_jvms"],
        "original_shape_queries": len(queries),
        "original_world_reads": sum(len(q["expected"]["trace"]) for q in queries),
        "original_query_faults": sum(not q["expected"]["accepted"] for q in queries),
        "original_shape_clip_booleans": sum("clip_hit" in q["expected"] for q in queries),
        "cached_key_states_compared": len(fixture["states"]) * len(queries),
        "vectors_sha256": sha(fixture_path),
        "oracle_report_sha256": sha(ROOT / "evidence/fluid-shape-oracle.json"),
        "native_sources_sha256": {name: sha(ROOT / f"src/{name}.rs") for name in ["fluid_physics", "runtime_item_world"]},
        "native_test_sources_sha256": {name: sha(ROOT / f"tools/{name}.rs") for name in ["fluid_shape_tests", "runtime_item_world_fluid_shape_tests"]},
        "scope": "Original process-global per-owner IdentityHashMap semantics under canonical "
                 "source/flowing/amount/falling identity; first query vs cached hit; "
                 "world read order; failure leaves absent entry; observed bootstrap snapshots; "
                 "guarded RuntimeItemWorld ray integration across worlds and waterlogged aliases",
        "not_verified": [
            "Full BlockGetter.clip DDA execution is separate from this original returned-shape clip oracle",
            "Only canonical vanilla fluid states; original noncanonical amount9 branch is unreachable in this domain",
            "Arbitrary runtime/bootstrap prehistory remains unknown unless explicitly supplied",
            "Fluid random ticks/source creation/block replacement and replaced datapack fluid tags",
        ],
    }
    try:
        tests = 0
        for label, pattern, expected in [
            ("native-shapes", "fluid_physics::shape_tests", 2),
            ("native-provider", "runtime_item_world::fluid_shape_tests", 1),
        ]:
            result = subprocess.run(["cargo", "test", "--lib", pattern, "--", "--nocapture"],
                                    cwd=ROOT, env=env, text=True, stdout=subprocess.PIPE,
                                    stderr=subprocess.STDOUT, check=False)
            (runtime / f"{label}.log").write_text(result.stdout)
            assert result.returncode == 0, f"{label} failed; see runtime/fluid-shape"
            matches = re.findall(r"test result: ok\. (\d+) passed", result.stdout)
            assert matches and int(matches[-1]) == expected
            tests += int(matches[-1])
        report["native_tests_passed"] = tests
        result = subprocess.run(["cargo", "clippy", "--lib", "--tests", "--", "-D", "warnings"],
                                cwd=ROOT, env=env, text=True, stdout=subprocess.PIPE,
                                stderr=subprocess.STDOUT, check=False)
        (runtime / "clippy.log").write_text(result.stdout)
        assert result.returncode == 0, "clippy failed; see runtime/fluid-shape/clippy.log"
        report.update(clippy=True, mismatches=0, ok=True)
    finally:
        (ROOT / "evidence/fluid-shape-conformance.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
