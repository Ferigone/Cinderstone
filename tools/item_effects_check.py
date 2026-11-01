#!/usr/bin/env python3
"""Native packet/range proof against original Java 1.21 ItemEntity effects."""
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
    fixture_path = ROOT / "fixtures/item-effects-vectors.json"
    fixture = json.loads(fixture_path.read_text())
    assert fixture["oracle_inner_sha256"] == PIN
    original = json.loads((ROOT / "evidence/item-effects-oracle.json").read_text())
    assert original["status"] == "PASS" and original["fixture_sha256"] == sha(fixture_path)
    runtime = ROOT / "runtime/item-effects"
    runtime.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    env.update(CARGO_INCREMENTAL="0", CARGO_HOME="/root/.local/share/mc-rust-toolchain/cargo",
               RUSTUP_HOME="/root/.local/share/mc-rust-toolchain/rustup")
    env["PATH"] = env["CARGO_HOME"] + "/bin:" + env["PATH"]
    report = {
        "ok": False,
        "minecraft": "1.21",
        "oracle_inner_sha256": PIN,
        "rust_build_profile": "debug",
        "rust_release_verified": False,
        "sound_packet_states": len(fixture["sound_cases"]),
        "level_event_packet_states": len(fixture["level_event_cases"]),
        "packet_ids": fixture["packet_ids"],
        "source_name": fixture["source_name"],
        "vectors_sha256": sha(fixture_path),
        "native_source_sha256": sha(ROOT / "src/item_effects.rs"),
        "native_tests_sha256": sha(ROOT / "tools/item_effects_tests.rs"),
        "bytecode_sha256": sha(ROOT / "tools/item_effects_bytecode.txt"),
        "packet_scope": "Actual original packet constructors, STREAM_CODEC payloads, "
                        "bound game-protocol packet IDs and complete packet bodies; "
                        "actual SoundEvent.getRange and registry-reference facts",
        "broadcast_scope": "ServerLevel/PlayerList bytecode plus native boundary checks: "
                           "same dimension; strict double distance; original sound origin; "
                           "f32 sound range widened to double; block-origin LevelEvent radius64",
        "not_verified": [
            "Live connection publication, world RNG seed consumption and triggering "
            "are separate coordinator/host behavior",
            "Actual PlayerList recipient execution is not an executable oracle here",
            "Active game-event listeners and unsupported lifecycle effects",
            "Compression, encryption and transport length prefix are outside these packet-body vectors",
        ],
    }
    try:
        for label, args in [
            ("native", ["cargo", "test", "--lib", "item_effects::tests", "--", "--nocapture"]),
            ("clippy", ["cargo", "clippy", "--lib", "--tests", "--", "-D", "warnings"]),
        ]:
            result = subprocess.run(args, cwd=ROOT, env=env, text=True,
                                    stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=False)
            (runtime / f"{label}.log").write_text(result.stdout)
            assert result.returncode == 0, f"{label} failed; see runtime/item-effects/{label}.log"
            if label == "native":
                matches = re.findall(r"test result: ok\. (\d+) passed", result.stdout)
                assert matches and int(matches[-1]) == 2
                report["native_tests_passed"] = int(matches[-1])
            else:
                report["clippy"] = True
        report["ok"] = True
    finally:
        (ROOT / "evidence/item-effects-conformance.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
