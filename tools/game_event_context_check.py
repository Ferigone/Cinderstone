#!/usr/bin/env python3
"""Check the native certified-empty game-event path against original classes."""
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


def digest(name):
    return hashlib.sha256((ROOT / name).read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--skip-generation", action="store_true")
    parser.add_argument("--release", action="store_true")
    args = parser.parse_args()
    runtime = ROOT / "runtime/game_event_context"
    runtime.mkdir(parents=True, exist_ok=True)
    if not args.skip_generation:
        subprocess.run([sys.executable, "tools/game_event_context_generate.py"], cwd=ROOT, check=True)
    oracle = json.loads((ROOT / "evidence/game-event-context-oracle.json").read_text())
    factories = json.loads((ROOT / "evidence/game-event-factories.json").read_text())
    assert digest("fixtures/game_event_context.json") == oracle["fixture_sha256"]
    assert digest("tools/game_event_context_factories.json") == "feb6919538d6a965c141f041ed2d05aadd9d1355d3461a71f92a7898840b2820"
    for source, expected in oracle["source_sha256"].items():
        assert digest(source) == expected, source
    env = os.environ.copy()
    env.update(CARGO_HOME="/root/.local/share/mc-rust-toolchain/cargo",
               RUSTUP_HOME="/root/.local/share/mc-rust-toolchain/rustup", CARGO_INCREMENTAL="0")
    profiles = {}
    for profile in ["debug"] + (["release"] if args.release else []):
        command = [CARGO, "test", "--lib", "--offline"]
        if profile == "release":
            command.append("--release")
        command += ["game_event_context::tests", "--", "--nocapture"]
        result = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True)
        output = result.stdout + result.stderr
        (runtime / (profile + ".log")).write_text(output)
        if result.returncode:
            raise RuntimeError(output[-8000:])
        assert re.search(r"test result: ok\. 3 passed; 0 failed", output), output[-3000:]
        assert "840 posts, 22900 getter calls, 11478 registry visits" in output
        profiles[profile] = {"tests_passed": 3, "differences": 0}
    sources = ["src/game_event_context.rs", "tools/game_event_context_tests.rs",
               "tools/game_event_context_oracle.java", "tools/game_event_context_generate.py",
               "tools/game_event_context_check.py", "tools/game_event_context_bytecode.txt",
               "tools/game_event_context_factories.java", "tools/game_event_context_factories.py"]
    report = {
        "ok": True, "minecraft": "1.21", "oracle_inner_sha256": oracle["oracle_inner_sha256"],
        "profiles": profiles, "original_methods": oracle["original_methods"],
        "posts": oracle["cases"], "world_getter_calls": oracle["getter_calls"],
        "chunk_queries": oracle["chunk_queries"], "section_visits": oracle["section_visits"],
        "materialized_registries": oracle["materialized_registries"],
        "original_world_rng_unchanged": oracle["world_rng_unchanged"],
        "source_sha256": {source: digest(source) for source in sources},
        "vectors_sha256": oracle["fixture_sha256"],
        "factory_facts_sha256": digest("tools/game_event_context_factories.json"),
        "catalogue": {key: factories[key] for key in ["registered_blocks", "block_states", "registered_block_entity_types"]},
        "scope": [
            "Original X/Z/Y loaded-chunk and registry lookup order, including unclamped sectionY and Java numeric boundaries",
            "Actual NOOP and newly created actual Euclidean empty registries; no listener/debug delivery or RNG consumption on this path",
            "Required authoritative registry snapshot; active listeners, pending add/remove, processing or Unknown cause explicit unsupported errors",
            "Actual ItemEntity/ServerPlayer dynamic-listener declarations and all registered BE listener factories are recorded separately",
            "Item ENTITY_DAMAGE/HIT_GROUND/SPLASH registry IDs and notification radii",
        ],
        "not_verified": [
            "Certification of actual native source-world loaded registries and main integration; BE absence alone is insufficient",
            "Nonempty listener dispatch, vibration delivery, game-event listener state machines and mobs",
            "Complete running ServerLevel: the oracle controls only loaded world/chunk-source adapters",
        ],
        "debug_fact_kind": "static original bytecode; debug call is conditional and agf.sendGameEvent itself is return-only",
        "reproduce": "python3 tools/game_event_context_check.py --release",
    }
    (ROOT / "evidence/game-event-context-conformance.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
