#!/usr/bin/env python3
"""Compare original/native retained BIOMES -> NOISE -> SURFACE -> AIR carvers.

Original source loop executes actual configured carvers over actual ProtoChunks,
mask, cached generation settings and retained NoiseChunk/aquifer/surface owner.
289 available dependencies and stored palette routing are explicit controlled
adapters: no WorldGenRegion, ChunkStep or generated-status completion is claimed.
"""
import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import tempfile
from check_biome_stage import run, sha

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
INNER = "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"


def digest_json(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def compare(args, work):
    oracle = args.oracle_dir.resolve()
    lock = json.loads((ROOT / "fixtures/source-lock.json").read_text())
    for section, name in [("oracle", "server-1.21.jar"), ("mappings", "server-1.21-mappings.txt")]:
        raw = (oracle / name).read_bytes()
        if len(raw) != lock[section]["size"] or hashlib.sha1(raw).hexdigest() != lock[section]["sha1"]:
            raise RuntimeError(f"wrong pinned artifact {name}")
    jar = oracle / "server-inner-1.21.jar"
    if sha(jar) != INNER:
        raise RuntimeError("wrong pinned inner server")
    java = str(args.java_home / "bin/java") if args.java_home else shutil.which("java")
    javac = str(args.java_home / "bin/javac") if args.java_home else shutil.which("javac")
    if not java or not javac:
        raise RuntimeError("Java 21 java/javac required")
    classes = work / "classes"
    classes.mkdir(exist_ok=True)
    cp = os.pathsep.join(map(str, [classes, jar, *sorted((oracle / "libraries").rglob("*.jar"))]))
    java_sources = [HERE / f"Worldgen{name}Oracle.java" for name in
                    ["Surface", "BiomeStage", "BiomeSurface", "Carver"]]
    run([javac, "-proc:none", "-cp", cp, "-d", str(classes), *map(str, java_sources)])
    if not args.skip_build:
        run(["cargo", "build", "--locked", "--lib"], cwd=ROOT)
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    if not target.is_absolute():
        target = ROOT / target
    deps = target / "debug/deps"
    extern = ["--extern", "mc121_rust=" + str(target / "debug/libmc121_rust.rlib")]
    for name in ["anyhow", "serde_json"]:
        matches = sorted(deps.glob(f"lib{name}-*.rlib"), key=lambda x: x.stat().st_mtime, reverse=True)
        if not matches:
            raise RuntimeError(f"missing native dependency {name}")
        extern += ["--extern", f"{name}={matches[0]}"]
    probe = work / "carver_probe"
    run(["rustc", "--edition=2021", str(HERE / "carver_probe.rs"), "-L", "dependency=" + str(deps),
         *extern, "-o", str(probe)])
    # The second source-observed Overworld origin gives actual canyon mask
    # contributions; the first gives extra-underground and cave contributions.
    cases = [("overworld", "overworld", -640, 320, -64, 384),
             ("overworld", "overworld", -638, 324, -64, 384),
             ("nether", "nether", 0, 0, 0, 256), ("end", "end", 65, 0, 0, 256)]
    queries = [dict(settings="minecraft:" + settings, biome_source="minecraft:" + source,
                    seed=123456789, x=x, z=z, level_min_y=y, level_height=height)
               for settings, source, x, z, y, height in cases]
    transcript = "".join(json.dumps(q) + "\n" for q in queries)
    original_lines = run([java, "-Xmx1600m", "-cp", cp, "oracle.WorldgenCarverOracle", str(work)], input=transcript)
    original = [json.loads(line) for line in original_lines.splitlines() if line.startswith("{")]
    native = [json.loads(line) for line in run([str(probe), str(work)], input=transcript).splitlines()]
    if len(original) != len(queries) or len(native) != len(queries):
        raise RuntimeError("incomplete original/native CARVERS transcript")
    results = []
    image_keys = ["world_surface_wg", "ocean_floor_wg", "postprocessing_count",
                  "postprocessing_sections", "postprocessing_positions_by_section"]
    all_contributions = Counter()
    tags = None
    for old, new in zip(original, native):
        identity = f"{old['settings']}({old['x']},{old['z']})"
        if old["oracle_inner_sha256"] != INNER:
            raise RuntimeError("wrong actually loaded original")
        for key in [*queries[0], "chunks", "use_legacy_random_source", "mask", "mask_cardinality"]:
            if old[key] != new[key]:
                raise RuntimeError(f"{identity}: {key} mismatch")
        for phase in ["before", "after"]:
            for key in image_keys:
                if old[phase][key] != new[phase][key]:
                    raise RuntimeError(f"{identity}: {phase}/{key} mismatch")
            if sum(map(len, old[phase]["postprocessing_sections"])) != old[phase]["postprocessing_count"]:
                raise RuntimeError("source section queues/count disagree")
        for key in ["before_file", "after_file", "palettes_file", "resolver_queries_file"]:
            if Path(old[key]).read_bytes() != Path(new[key]).read_bytes():
                raise RuntimeError(f"{identity}: full {key} mismatch")
        for key in ["neighbors", "attempts", "biome_queries", "random_count"]:
            if old["report"][key] != new["report"][key]:
                a, b = old["report"][key], new["report"][key]
                if isinstance(a, list) and isinstance(b, list):
                    first = next(((i, x, y) for i, (x, y) in enumerate(zip(a, b)) if x != y), None)
                    detail = f"lengths {len(a)}/{len(b)}, first {first}"
                else:
                    detail = f"{a}/{b}"
                raise RuntimeError(f"{identity}: report/{key} mismatch: {detail}")
        deterministic_rng = bool(old["report"]["attempts"])
        if deterministic_rng and old["report"]["random_state48"] != new["report"]["random_state48"]:
            raise RuntimeError(f"{identity}: deterministic post-reseed legacy state mismatch")
        checks = ["unique_factory_advances_once", "neighbor_cache_hits_suppress_supplier",
                  "same_noise_cache_aquifer_owners", "biomes_unchanged", "all_289_statuses_remain_empty"]
        if not all(old[k] for k in checks) or not new["biomes_unchanged"] or not new["neighbor_cache_owners_retained"]:
            raise RuntimeError("source/native owner, cache or status witness failed")
        if new["stage"] != "Carvers" or new["retained_prepared_owner_count"] != 9:
            raise RuntimeError("native successful stage/retained prepared owners changed")
        if len(old["report"]["neighbors"]) != 289 or any(n["cached"] for n in old["report"]["neighbors"]):
            raise RuntimeError("expected actual initially absent 289 source neighbor caches")
        before, after = Path(old["before_file"]).read_bytes(), Path(old["after_file"]).read_bytes()
        changed = sum(a != b for a, b in zip(struct.iter_unpack("<I", before), struct.iter_unpack("<I", after)))
        if not deterministic_rng and (changed or old["mask_cardinality"] or old["report"]["random_count"]):
            raise RuntimeError("empty actual AIR registry produced deterministic carving work")
        contributions = old["mask_additions_by_carver"]
        all_contributions.update(contributions)
        if tags is None:
            tags = old["tag_source_sha256"]
        elif tags != old["tag_source_sha256"]:
            raise RuntimeError("source tag binding changed across cases")
        results.append({**queries[len(results)], "material_block_count": len(after) // 4,
                        "before_sha256": hashlib.sha256(before).hexdigest(),
                        "after_sha256": hashlib.sha256(after).hexdigest(), "changed_blocks": changed,
                        "mask_word_count": len(old["mask"]), "mask_cardinality": old["mask_cardinality"],
                        "mask_sha256": digest_json(old["mask"]),
                        "source_mask_additions_by_carver": contributions,
                        "attempts": len(old["report"]["attempts"]),
                        "started_by_carver": dict(Counter(a["key"] for a in old["report"]["attempts"] if a["start"])),
                        "invocation_next_bits_count": old["report"]["random_count"],
                        "post_reseed_state48": old["report"]["random_state48"] if deterministic_rng else None,
                        "empty_rng_clock_value_compared": False,
                        "report_sha256_excluding_clock_state": digest_json({k:v for k,v in old["report"].items() if k != "random_state48"}),
                        "raw_biome_queries": len(old["report"]["biome_queries"]),
                        "raw_top_material_queries": sum(q["kind"] == "top_material" for q in old["report"]["biome_queries"]),
                        "before_postprocessing_count": old["before"]["postprocessing_count"],
                        "after_postprocessing_count": old["after"]["postprocessing_count"],
                        "heightmap_and_queue_before_sha256": digest_json({k:old["before"][k] for k in image_keys}),
                        "heightmap_and_queue_after_sha256": digest_json({k:old["after"][k] for k in image_keys}),
                        "full_arrays_masks_queues_reports_equal": True,
                        "source_unique_factory_advances_once": True,
                        "source_289_statuses_remain_empty": True})
    if any(all_contributions[f"minecraft:{name}"] <= 0 for name in ["cave", "cave_extra_underground", "canyon", "nether_cave"]):
        raise RuntimeError("one actual configured carver lacks nonempty target mask evidence")
    paths = [*java_sources, HERE / "carver_probe.rs", HERE / "check_carvers.py"]
    result = {"scope": "controlled real BIOMES -> NOISE -> SURFACE -> AIR configured carvers, not ChunkStep/WorldGenRegion/status completion",
              "oracle_inner_sha256": INNER, "official_mapping_sha1": lock["mappings"]["sha1"],
              "original_tag_binding": "single pinned builtin block/fluid TagFile CODEC -> TagLoader.build -> Registry.bindTags",
              "source_tag_sha256": tags,
              "validation_source_sha256": {str(p.relative_to(ROOT)):sha(p) for p in paths},
              "cases": results,
              "limitations": ["real dependency availability is controlled; no structures, retrogen, blending or scheduler proof",
                              "selected real source materials exercise builtin debug_mode=false; controlled debug/additional-mask branches require separate evidence",
                              "per-section postprocessing storage order is compared; no global order reconstructed from original section arrays",
                              "unique factory numeric clock outputs differ by process and are not compared; original one-step global advance is witnessed"]}
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oracle-dir", type=Path, required=True)
    parser.add_argument("--java-home", type=Path)
    parser.add_argument("--work-dir", type=Path)
    parser.add_argument("--write-report", type=Path)
    parser.add_argument("--skip-build", action="store_true", help="use parent-built dedicated-worktree library")
    args = parser.parse_args()
    if args.work_dir:
        args.work_dir.mkdir(parents=True, exist_ok=True)
        result = compare(args, args.work_dir.resolve())
    else:
        with tempfile.TemporaryDirectory(prefix="cinderstone-carver-compare-") as d:
            result = compare(args, Path(d))
    encoded = json.dumps(result, indent=2) + "\n"
    if args.write_report:
        args.write_report.write_text(encoded)
    print(encoded)


if __name__ == "__main__":
    main()
