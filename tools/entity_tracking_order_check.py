#!/usr/bin/env python3
"""Verify tracked-entity map ordering against the actual bundled fastutil."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CARGO = "/root/.local/share/mc-rust-toolchain/cargo/bin/cargo"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(command, log, env=None):
    result = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True)
    output = result.stdout + result.stderr
    log.write_text(output)
    if result.returncode:
        raise RuntimeError(f"{command} failed ({result.returncode}):\n{output[-8000:]}")
    return output


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--skip-generation", action="store_true")
    parser.add_argument("--release", action="store_true")
    args = parser.parse_args()
    runtime = ROOT / "runtime/entity_tracking_order"
    runtime.mkdir(parents=True, exist_ok=True)
    if not args.skip_generation:
        run([sys.executable, "tools/entity_tracking_order_generate.py"], runtime / "generation-check.log")
    original = json.loads((ROOT / "evidence/entity-tracking-order-oracle.json").read_text())
    fixture_path = ROOT / "fixtures/entity_tracking_order.json"
    assert digest(fixture_path) == original["fixture_sha256"]
    assert digest(ROOT / "tools/entity_tracking_order_oracle.java") == original["oracle_source_sha256"]
    if "generator_source_sha256" in original:
        assert digest(ROOT / "tools/entity_tracking_order_generate.py") == original["generator_source_sha256"]
    fixture = json.loads(fixture_path.read_text())
    assert fixture["dependency_sha256"] == original["dependency_sha256"]
    states = [operation["expected"] for case in fixture["cases"] for operation in case["ops"]]
    assert len(states) == original["states"]
    entries = sum(len(state["entries"]) for state in states)
    key_slots = sum(len(state["key_slots"]) for state in states)
    env = os.environ.copy()
    env.update(CARGO_HOME="/root/.local/share/mc-rust-toolchain/cargo",
               RUSTUP_HOME="/root/.local/share/mc-rust-toolchain/rustup", CARGO_INCREMENTAL="0")
    profiles = {}
    for profile in ["debug"] + (["release"] if args.release else []):
        command = [CARGO, "test", "--lib", "--offline"]
        if profile == "release":
            command.append("--release")
        command += ["entity_tracking_order::tests", "--", "--nocapture"]
        output = run(command, runtime / ("rust-" + profile + ".log"), env)
        assert re.search(r"test result: ok\. 1 passed; 0 failed", output), output[-3000:]
        assert f'{len(states)} mutation states, {entries} ordered entries' in output
        profiles[profile] = {"tests_passed": 1, "mismatches": 0}
    sources = ["src/entity_tracking_order.rs", "tools/entity_tracking_order_tests.rs",
               "tools/entity_tracking_order_oracle.java", "tools/entity_tracking_order_generate.py",
               "tools/entity_tracking_order_check.py"]
    report = {
        "ok": True, "minecraft": "1.21", "dependency": original["dependency"],
        "dependency_sha256": original["dependency_sha256"],
        "dependency_hash_source": original["dependency_hash_source"],
        "actual_class": original["actual_class"], "construction": original["construction"],
        "cases": original["cases"], "states": len(states), "ordered_entries": entries,
        "key_slots_compared": key_slots, "operations": original["operations"],
        "capacities": original["capacities"], "profiles": profiles,
        "source_sha256": {name: digest(ROOT / name) for name in sources},
        "vectors_sha256": original["fixture_sha256"],
        "scope": [
            "Default ChunkMap Int2ObjectOpenHashMap capacity32/load0.75 and immutable entry/value iteration",
            "put replacements and return values, remove/shiftKeys, growth, shrink floor32, clear retaining capacity",
            "Zero key first, signed/min/max IDs, collisions and wraparound probe clusters",
            "Complete key-table layout, size, current capacity, containsNullKey, minN and maxFill after every mutation",
            "Native get/contains_key verified against actual original entries after each mutation",
        ],
        "not_verified": [
            "Nondefault constructors/load factors, trim, serialization and mutation through live Java iterators",
            "Complete ChunkMap tracked-entity lifecycle and caller scheduling; host must include player keys as well as items",
        ],
        "reproduce": "python3 tools/entity_tracking_order_check.py --release",
    }
    (ROOT / "evidence/entity-tracking-order-conformance.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
