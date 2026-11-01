#!/usr/bin/env python3
"""Verify the thin item host's native composition and publisher ordering."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
CARGO = "/root/.local/share/mc-rust-toolchain/cargo/bin/cargo"


def digest(name):
    return hashlib.sha256((ROOT / name).read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--release", action="store_true")
    args = parser.parse_args()
    effects = json.loads((ROOT / "evidence/item-effects-oracle.json").read_text())
    assert digest("fixtures/item-effects-vectors.json") == effects["fixture_sha256"]
    pickup = json.loads((ROOT / "evidence/item-pickup-conformance.json").read_text())
    assert pickup["ok"] and pickup["mismatches"] == 0
    # The menu/drop modules can grow unrelated APIs after the original proof.
    # Pin its immutable corpus, then rerun that original differential test on
    # the current sources in both profiles rather than borrowing stale hashes.
    assert digest("fixtures/item-pickup-vectors.json") == pickup["source_sha256"]["fixtures/item-pickup-vectors.json"]
    local = json.loads((ROOT / "evidence/container-local-listener-conformance.json").read_text())
    assert local["ok"] and local["native_cases"] == 102 and local["native_operations"] == 1985
    for source in ["src/container.rs", "src/inventory.rs", "fixtures/container_local_listener.json"]:
        assert digest(source) == local["source_sha256"][source], source
    env = os.environ.copy()
    env.update(CARGO_HOME="/root/.local/share/mc-rust-toolchain/cargo",
               RUSTUP_HOME="/root/.local/share/mc-rust-toolchain/rustup", CARGO_INCREMENTAL="0")
    runtime = ROOT / "runtime/item_host"
    runtime.mkdir(parents=True, exist_ok=True)
    sources = ["src/item_host.rs", "tools/item_host_tests.rs", "tools/item_host_check.py",
               "src/runtime_entities.rs", "src/player_inventory.rs", "src/item_effects.rs",
               "src/item_entity.rs", "src/container.rs", "src/inventory.rs"]
    frozen_sources = {source: digest(source) for source in sources}
    def assert_sources_stable():
        for source, expected in frozen_sources.items():
            assert digest(source) == expected, "source changed during host proof: " + source
    profiles = {}
    for profile in ["debug"] + (["release"] if args.release else []):
        command = [CARGO, "test", "--lib", "--offline"]
        if profile == "release":
            command.append("--release")
        command += ["item_host::tests", "--", "--nocapture"]
        assert_sources_stable()
        result = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True)
        assert_sources_stable()
        output = result.stdout + result.stderr
        (runtime / (profile + ".log")).write_text(output)
        if result.returncode:
            raise RuntimeError(output[-8000:])
        assert re.search(r"test result: ok\. 5 passed; 0 failed", output), output[-3000:]
        command[-3:] = ["item_entity::pickup_tests", "--", "--nocapture"]
        assert_sources_stable()
        result = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True)
        assert_sources_stable()
        output = result.stdout + result.stderr
        (runtime / (profile + "-original-pickup.log")).write_text(output)
        if result.returncode:
            raise RuntimeError(output[-8000:])
        assert re.search(r"test result: ok\. 1 passed; 0 failed", output), output[-3000:]
        assert '"mismatches":0' in output
        profiles[profile] = {"tests_passed": {"item_host": 5, "original_pickup_core": 1}, "failures": 0}
    report = {
        "ok": True, "minecraft": "1.21", "profiles": profiles,
        "verification_kind": "native adapter composition; not an independent whole-host Java oracle",
        "cargo_target_directory": env.get("CARGO_TARGET_DIR", "default"),
        "original_packet_fixture_sha256": effects["fixture_sha256"],
        "related_original_proofs": ["evidence/item-effects-oracle.json", "evidence/item-pickup-conformance.json", "evidence/container-local-listener-conformance.json"],
        "source_sha256": frozen_sources,
        "source_stability_verified_during_all_runs": True,
        "verified": [
            "Actual RuntimeEntities + SharedContainer publisher: Collect, required immutable inventory_slot_changed callbacks from the local menu baseline, private menu synchronization, removal, empty discarded callback, restored-count statistics and onPickup",
            "Local InventoryMenu callback 5..45 identity gate is independent of remote prediction; host drains local changes before publishing the private menu packet batch",
            "Current registered gamemode1 enables creative overflow; survival partial transfer returns false without collect/stat/menu publication",
            "Rejected partial transfer keeps authoritative inventory changes and leaves published item count/cache unchanged; the next bounded level-player phase executes its required hook, local inventory listener, then Slot packet using the same menu helper as Collect",
            "Sound and level-event bytes from actual original fixture, supplied seed and captured callback position/block origin",
            "Fresh registry audience, required overworld fact, strict 3D sound radius16 and level-event radius64, spectators included",
            "Required synchronous player tick/profile/game-event/damage/stat/local-inventory-listener/onPickup hooks and explicit hook/identity/container failures",
        ],
        "not_verified": [
            "Full ServerPlayer.tick/doTick: the proved level-player path is only required host callback plus menu broadcast/local-listener seam; statistic/advancement, game-event listener implementations and multidimension runtime remain separate",
            "Actual live socket dispatch and shared-publisher installation in main",
            "Reentrant gameplay callbacks modifying the authoritative item store or menu",
            "Exact original menu baseline state after a required local callback throws: SharedContainer rolls back its transaction while earlier Collect/hook effects remain",
            "Per-slot callback/packet interleaving for listeners that emit advancement/recipe packets: this host drains all immutable callbacks before the returned menu batch; the original menu alternates each local callback with its remote slot synchronization",
            "Nonempty advancement/reward listener execution; the current fixture hook certificate is separate",
        ],
        "reproduce": "python3 tools/item_host_check.py --release",
    }
    (ROOT / "evidence/item-host-integration.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
