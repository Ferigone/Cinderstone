#!/usr/bin/env python3
"""Check bounded runtime chunk composition; no whole-scheduler parity claim."""
from pathlib import Path
import argparse
import hashlib
import json
import os
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
ORIGINAL = "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"


def sha(path):
    return hashlib.sha256((ROOT / path).read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--release", action="store_true")
    args = parser.parse_args()
    dependencies = [
        ("chunk-loading-tickets", "chunk_loading_tickets", "native_source_sha256", "states", 1428),
        ("entity-ticking-range", "entity_ticking_range", "native_source_sha256", "states", 1487),
        ("chunk-lifecycle", "chunk_lifecycle", "source_sha256", "operations", 2653),
        ("chunk-readiness", "chunk_readiness", "source_sha256", "operations", 4521),
    ]
    verified_dependencies = {}
    for name, module, hash_key, count_key, count in dependencies:
        path = f"evidence/{name}-conformance.json"
        report = json.loads((ROOT / path).read_text())
        assert report["ok"] and report["oracle_inner_sha256"] == ORIGINAL, path
        assert report[hash_key] == sha(f"src/{module}.rs"), f"stale source: {path}"
        assert report[count_key] == count, path
        verified_dependencies[name] = {
            "report": path, "report_sha256": sha(path), count_key: count,
            "native_source_sha256": report[hash_key],
        }
    fixture = json.loads((ROOT / "fixtures/item-full-vectors.json").read_text())
    assert fixture["oracle_inner_sha256"] == ORIGINAL
    assert len(fixture["scenes"][0]["states"]) == 80
    env = os.environ.copy()
    env.update(CARGO_HOME="/root/.local/share/mc-rust-toolchain/cargo",
               RUSTUP_HOME="/root/.local/share/mc-rust-toolchain/rustup", CARGO_INCREMENTAL="0")
    cargo = "/root/.local/share/mc-rust-toolchain/cargo/bin/cargo"
    command = [cargo, "test", "--locked", "--lib", "runtime_chunk_manager::tests"]
    if args.release:
        command.append("--release")
    command += ["--", "--nocapture"]
    result = subprocess.run(command, cwd=ROOT, env=env, capture_output=True, text=True)
    profile = "release" if args.release else "debug"
    runtime = ROOT / "runtime/runtime-chunk-manager"
    runtime.mkdir(parents=True, exist_ok=True)
    log = runtime / f"{profile}.log"
    log.write_text(result.stdout + result.stderr)
    if result.returncode:
        raise RuntimeError((result.stdout + result.stderr)[-8000:])
    assert re.search(r"test result: ok\. 9 passed; 0 failed; 0 ignored", result.stdout)
    marker = "RUNTIME_CHUNK_MANAGER_ITEM_CONFORMANCE: "
    comparisons = [json.loads(line.removeprefix(marker)) for line in result.stdout.splitlines()
                   if line.startswith(marker)]
    assert comparisons == [dict(ok=True, original_scenes=1, poststates_motion_and_counters=80,
                                pending_chunk_marks=9, mismatches=0)]
    sources = [
        "src/runtime_chunk_manager.rs", "src/chunk_loading_tickets.rs", "src/entity_ticking_range.rs",
        "src/chunk_lifecycle.rs", "src/chunk_readiness.rs", "src/runtime_world.rs", "src/world.rs",
        "src/storage.rs", "src/anvil.rs", "src/runtime_entities.rs", "src/runtime_entity_index.rs",
        "src/entity_tick.rs", "src/entity_spatial.rs", "src/item_entity.rs", "src/entity_state.rs",
        "src/entity_base.rs", "src/entity_move.rs", "src/entity_support.rs", "src/runtime_item_world.rs",
        "src/runtime_collision.rs", "src/fluid_physics.rs", "src/random.rs", "src/inventory.rs",
        "tools/runtime_chunk_manager_tests.rs", "tools/runtime_chunk_manager_check.py",
        "tools/runtime_chunk_manager_audit.md", "tools/runtime_chunk_manager_bytecode.txt",
        "tools/chunk_lifecycle_bytecode.txt",
        "tools/storage_blockentity_factories.json", "fixtures/item-full-vectors.json",
    ]
    report = dict(
        ok=True, minecraft="1.21", oracle_inner_sha256=ORIGINAL,
        kind="bounded native composition tests over independently original-verified cores",
        profile=profile, native_tests_passed=9, comparisons=comparisons[0],
        dependencies=verified_dependencies,
        command=command, log_sha256=hashlib.sha256(log.read_bytes()).hexdigest(),
        scope="Queued serialized global-main composition; explicit holder order; current-FULL/query-fact guards; fresh native fixture source/actions; one original80tick motion/counter replay.",
        not_verified=[
            "Entire original global scheduler; Java identity HashSet order; complete aqe/mailbox interleavings. Native stable policies are explicit alternatives, not JVM-order parity.",
            "Anvil FULL source conversion/postload, generation, imported source entities, nonempty postprocessing/UpgradeData/block-entity/saved-tick actions. These remain Pending or error.",
            "Asynchronous source loader or automatic completion of already-pending source slots after missing facts change.",
            "Reentrant callbacks, parallel races, full chunk unloading and unbounded holder/future-history reclamation.",
            "ChunkMap visibleChunkMap publication/snapshot history; the send-ready getter uses the retained native holder registry and exact holder predicate.",
            "Whole-world threaded lighting, player tick, live network/client behavior and complete1:1 server parity.",
            "The80tick integration compares position/velocity bits, ground/fall state and item counters only; it does not compare every full-item corpus field.",
        ],
        source_sha256={source: sha(source) for source in sources},
    )
    destination = ROOT / "evidence/runtime-chunk-manager-conformance.json"
    temporary = destination.with_suffix(".json.tmp")
    temporary.write_text(json.dumps(report, indent=2) + "\n")
    temporary.replace(destination)
    print(json.dumps({key: value for key, value in report.items()
                      if key not in {"source_sha256", "not_verified", "dependencies", "scope", "command"}}))


if __name__ == "__main__":
    main()
