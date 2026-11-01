#!/usr/bin/env python3
"""Compare the native item sender with unchanged original ServerEntity calls."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
SHA = "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"
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
    runtime = ROOT / "runtime/item_replication"
    runtime.mkdir(parents=True, exist_ok=True)
    if not args.skip_generation:
        run([sys.executable, "tools/item_replication_generate.py"], runtime / "generation.log")
    original = json.loads((ROOT / "evidence/item-replication-oracle.json").read_text())
    assert original["oracle_inner_sha256"] == SHA
    for name, expected in original["files"].items():
        assert digest(ROOT / name) == expected, name
    env = os.environ.copy()
    env.update(CARGO_HOME="/root/.local/share/mc-rust-toolchain/cargo",
               RUSTUP_HOME="/root/.local/share/mc-rust-toolchain/rustup", CARGO_INCREMENTAL="0")
    profiles = {}
    for profile in ["debug"] + (["release"] if args.release else []):
        command = [CARGO, "test", "--lib", "--offline"]
        if profile == "release":
            command.append("--release")
        command += ["item_replication::tests", "--", "--nocapture"]
        output = run(command, runtime / ("rust-" + profile + ".log"), env)
        assert re.search(r"test result: ok\. 1 passed; 0 failed", output), output[-3000:]
        assert f'{original["states"]} snapshots, {original["packets"]} original packets' in output
        regression = [CARGO, "test", "--lib", "--offline"]
        if profile == "release":
            regression.append("--release")
        regression += ["entity::tests::shared_motion_cadence_matches_actual_server_entity_bytecode"]
        output = run(regression, runtime / ("motion-regression-" + profile + ".log"), env)
        assert re.search(r"test result: ok\. 1 passed; 0 failed", output), output[-3000:]
        profiles[profile] = {"tests_passed": 2, "mismatches": 0}
    sources = ["src/item_replication.rs", "src/entity.rs", "src/item_entity.rs",
               "tools/item_replication_oracle.java", "tools/item_replication_generate.py",
               "tools/item_replication_check.py"]
    report = {
        "ok": True, "minecraft": "1.21", "protocol": 767,
        "oracle_inner_sha256": SHA, "actual_original_classes": original["actual_original_classes"],
        "actual_original_methods": ["ServerEntity.sendChanges", "ServerEntity.sendPairingData",
                                    "ItemEntity.playerTouch", "ItemEntity private merge"],
        "cases": original["cases"], "states": original["states"],
        "packets": original["packets"], "packet_payload_bytes": original["packet_payload_bytes"],
        "profiles": profiles,
        "source_sha256": {name: digest(ROOT / name) for name in sources},
        "vectors_sha256": digest(ROOT / "fixtures/item_replication_sender.json"),
        "scope": [
            "Unmounted item sender with empty passenger list; one shared sender per item",
            "Interval 20 and impulse/dirty gates, movement bytes/order and sent position/velocity baselines",
            "Dirty-only metadata packets and full non-default cache refresh on any dirty entry",
            "Canonical EMPTY default reference is omitted from pairing metadata; in-place zero retains Slot8",
            "In-place count remains unsent until a separate dirty refresh captures its new Slot8 value",
            "Hurt velocity after ordinary packets; resets without changing the sent velocity baseline",
            "Late pairing packets from cached metadata and shared movement baselines",
            "Original partial/full pickup inputs replay native try_pickup with controlled inventory capacity",
            "Original private merge inputs replay its destination-copy/source-shrink edits, not eligibility",
            "Snapshots use physical position for visibility and published metadata/velocity for pairing",
        ],
        "not_verified": [
            "Mounted/passenger branches, living attributes and item-frame map handling",
            "Reentrant per-packet callbacks observing intermediate sender state: native API returns complete batches",
            "Complete running-world scheduling/network and inventory implementation; environmental callbacks are controlled",
        ],
        "reproduce": "python3 tools/item_replication_check.py --release",
    }
    (ROOT / "evidence/item-replication-conformance.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
