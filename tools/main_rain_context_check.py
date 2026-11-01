#!/usr/bin/env python3
"""Check Main's real item/rain composition against unchanged Java tick frames."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def sha(name):
    return hashlib.sha256((ROOT / name).read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--release", action="store_true")
    args = parser.parse_args()
    oracle = json.loads((ROOT / "evidence/item-rain-oracle.json").read_text())
    assert oracle["ok"] and not oracle["original_classes_modified"]
    assert sha("fixtures/item-rain-vectors.json") == oracle["fixture_sha256"]
    for name, digest in oracle["source_sha256"].items():
        assert sha(name) == digest, name
    names = [
        "src/main.rs", "src/main_rain_context.rs", "src/main_chunk_runtime.rs",
        "src/main_item_gameplay.rs", "src/item_host.rs", "src/runtime_entities.rs",
        "src/runtime_item_world.rs", "src/runtime_chunk_manager.rs",
        "src/world_context.rs", "src/random_seed.rs", "src/random.rs",
        "src/rain_physics.rs", "src/rain_fixture.rs", "src/item_entity.rs",
        "src/entity_state.rs", "src/entity_base.rs", "src/item_effects.rs",
        "tools/main_rain_context_tests.rs", "tools/main_rain_context_check.py",
        "tools/main_rain_context_audit.md", "tools/main_item_plumbing_tests.rs",
        "fixtures/item-rain-vectors.json", "fixtures/rain-fixture-facts.json",
        "evidence/item-rain-oracle.json",
    ]
    before = {name: sha(name) for name in names}
    env = os.environ.copy()
    env.update(CARGO_HOME="/root/.local/share/mc-rust-toolchain/cargo",
               RUSTUP_HOME="/root/.local/share/mc-rust-toolchain/rustup",
               CARGO_INCREMENTAL="0")
    profile = "release" if args.release else "debug"
    command = ["/root/.local/share/mc-rust-toolchain/cargo/bin/cargo", "test",
               "--locked", "--offline", "--bin", "mc121-rust"]
    if args.release:
        command.append("--release")
    command.extend(["--", "--nocapture"])
    process = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True)
    output = process.stdout + process.stderr
    directory = ROOT / "runtime/main-rain-context"
    directory.mkdir(parents=True, exist_ok=True)
    log = directory / (profile + ".log")
    log.write_text(output)
    assert before == {name: sha(name) for name in names}, "relevant source changed during proof"
    if process.returncode:
        raise RuntimeError(output[-12000:])
    match = re.search(r"test result: ok\. (\d+) passed; 0 failed;", output)
    assert match, output[-6000:]
    marker = "MAIN_RAIN_CONTEXT_CONFORMANCE:2 actual original first-tick states,1 sound seed,-3561856981251203394,Level.random unchanged,no audience"
    assert marker in output, "actual Main/Java comparison did not execute"
    tests = [
        "main_wet_and_clear_fire10_tick_matches_original_state_sound_and_independent_rngs",
        "main_rain_requires_actual_owner_and_persistent_settled_source_before_tick",
    ]
    assert all(re.search(r"test .*" + name + r" \.\.\. ok", output) for name in tests)
    destination = ROOT / "evidence/main-rain-context-conformance.json"
    profiles = {}
    if destination.exists():
        old = json.loads(destination.read_text())
        if old.get("source_sha256") == before:
            profiles = old["profiles"]
    profiles[profile] = {
        "main_bin_tests_passed": int(match[1]), "new_main_rain_tests": 2,
        "differences": 0, "command": command,
        "cargo_target_directory": env.get("CARGO_TARGET_DIR", "target"),
        "log": str(log.relative_to(ROOT)), "log_sha256": sha(log.relative_to(ROOT)),
    }
    report = {
        "ok": True, "minecraft": "1.21", "profiles": profiles,
        "oracle_inner_sha256": oracle["oracle_inner_sha256"],
        "original_classes_modified": False,
        "original_first_tick_scenes": ["wet_fire10", "clear_fire10"],
        "original_first_tick_states": 2, "original_sound_events": 1,
        "encoded_sound_packets": 1,
        "source_sha256": before, "source_stability_verified": True,
        "tests": tests,
        "scope": [
            "Server.tick_item_world calls the tested LivePlayers.simulate_item_world production helper.",
            "Real forced-ticket manager polling confirms current FULL and TICKING, then the production swept collision/game-event halo guard runs.",
            "Actual WorldContext weather, MainItemGameplay and RuntimeInventoryHost compose the original wet/clear fire10 first-tick state.",
            "Main retains Level.random and Level.threadSafeRandom from one create_level_randoms call; one persistent fresh-fixture rain cache is shared across ticks.",
            "Wet extinguish encodes the original pitch, volume, quantized coordinates and f-stream nextLong seed even with no audience; z remains unchanged.",
            "Missing rain owner, invalidated lighting history and unknown FULL queries fail explicitly; guarded tick attempts preserve item state and level RNG streams.",
        ],
        "not_verified": [
            "The two main scenes compare the first tick only; the separate coordinator proof covers all80 original rainy states.",
            "These tests do not read back the opaque per-item RNG owner; its separate complete coordinator replay verifies the original entity RNG state.",
            "Edited/imported worlds, other biomes, general lighting/weather generation, concurrent ThreadSafeLegacyRandomSource CAS behavior.",
            "Full Player/ServerPlayer.tick, default vanilla advancements/recipe book, game-event listeners, global runtime1:1 compatibility.",
            "The v5 validation tree and released binary are immutable and contain none of this later source change.",
        ],
    }
    destination.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({k: report[k] for k in ["ok", "profiles", "original_first_tick_states", "original_sound_events", "encoded_sound_packets"]}, indent=2))


if __name__ == "__main__":
    main()
