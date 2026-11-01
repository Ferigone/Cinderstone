#!/usr/bin/env python3
"""Compare controlled real 3x3 BIOMES palettes -> same-owner NOISE -> SURFACE.

The original quart provider reads actual section palettes through an explicit
snapshot adapter. This is not WorldGenRegion, ChunkStep, persisted BIOMES status,
server scheduling or FULL chunk publication. Generated artifacts stay external.
"""
import argparse
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


def compare(args, work):
    oracle = args.oracle_dir.resolve()
    lock = json.loads((ROOT / "fixtures/source-lock.json").read_text())
    for section, filename in [("oracle", "server-1.21.jar"), ("mappings", "server-1.21-mappings.txt")]:
        data = (oracle / filename).read_bytes()
        if len(data) != lock[section]["size"] or hashlib.sha1(data).hexdigest() != lock[section]["sha1"]:
            raise RuntimeError(f"wrong pinned artifact: {filename}")
    jar = oracle / "server-inner-1.21.jar"
    if sha(jar) != INNER:
        raise RuntimeError("wrong pinned inner server")
    java = str(args.java_home / "bin/java") if args.java_home else shutil.which("java")
    javac = str(args.java_home / "bin/javac") if args.java_home else shutil.which("javac")
    if not java or not javac:
        raise RuntimeError("Java 21 java/javac must be available")
    classes = work / "classes"
    classes.mkdir(exist_ok=True)
    cp = os.pathsep.join(map(str, [classes, jar, *sorted((oracle / "libraries").rglob("*.jar"))]))
    run([javac, "-proc:none", "-cp", cp, "-d", str(classes),
         str(HERE / "WorldgenSurfaceOracle.java"), str(HERE / "WorldgenBiomeStageOracle.java"),
         str(HERE / "WorldgenBiomeSurfaceOracle.java")])
    run(["cargo", "build", "--locked", "--lib"], cwd=ROOT)
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    if not target.is_absolute():
        target = ROOT / target
    deps = target / "debug/deps"
    extern = ["--extern", "mc121_rust=" + str(target / "debug/libmc121_rust.rlib")]
    for name in ["anyhow", "serde_json"]:
        candidates = sorted(deps.glob(f"lib{name}-*.rlib"), key=lambda p: p.stat().st_mtime, reverse=True)
        if not candidates:
            raise RuntimeError(f"missing built dependency {name}")
        extern += ["--extern", name + "=" + str(candidates[0])]
    probe = work / "biome_surface"
    run(["rustc", "--edition=2021", str(HERE / "biome_surface.rs"), "-L", "dependency=" + str(deps),
         *extern, "-o", str(probe)])
    cases = [("overworld", "overworld", -640, 320, -64, 384),
             ("nether", "nether", 0, 0, 0, 256),
             ("end", "end", 65, 0, 0, 256)]
    queries = [dict(settings="minecraft:" + settings, biome_source="minecraft:" + source,
                    seed=123456789, x=x, z=z, level_min_y=y, level_height=height)
               for settings, source, x, z, y, height in cases]
    transcript = "".join(json.dumps(query) + "\n" for query in queries)
    original = [json.loads(line) for line in run([
        java, "-Xmx1024m", "-cp", cp, "oracle.WorldgenBiomeSurfaceOracle", str(work)], input=transcript).splitlines()]
    native = [json.loads(line) for line in run([str(probe), str(work)], input=transcript).splitlines()]
    if len(original) != len(queries) or len(native) != len(queries):
        raise RuntimeError("incomplete original/native BIOMES -> NOISE -> SURFACE transcript")
    results = []
    image_keys = ["world_surface_wg", "ocean_floor_wg", "postprocessing_count",
                  "postprocessing_sections", "postprocessing_positions_by_section"]
    for old, new in zip(original, native):
        if old["oracle_inner_sha256"] != INNER:
            raise RuntimeError("wrong loaded original NoiseChunk artifact")
        for key in ["settings", "biome_source", "seed", "x", "z", "level_min_y", "level_height",
                    "chunks", "center_biome_counts", "use_legacy_random_source"]:
            if old[key] != new[key]:
                raise RuntimeError(f"natural SURFACE {key} mismatch: {old['settings']}")
        for phase in ["before", "after"]:
            for key in image_keys:
                if old[phase][key] != new[phase][key]:
                    raise RuntimeError(f"natural SURFACE {phase}/{key} mismatch: {old['settings']}")
            if sum(map(len, old[phase]["postprocessing_sections"])) != old[phase]["postprocessing_count"]:
                raise RuntimeError("actual source section queue contents/count disagree")
        for key in ["palettes_file", "resolver_queries_file", "neighbor_queries_file", "before_file", "after_file"]:
            if Path(old[key]).read_bytes() != Path(new[key]).read_bytes():
                raise RuntimeError(f"natural SURFACE {key} mismatch: {old['settings']} at {old['x']},{old['z']}")
        checks = ["same_center_noise_chunk", "same_cache_and_aquifer_owners", "stored_biomes_unchanged",
                  "persisted_statuses_remain_empty", "noise_future_same_chunk"]
        if not all(old[key] for key in checks) or not new["stored_biomes_unchanged"] or new["stage"] != "Surface":
            raise RuntimeError("natural SURFACE owner or status witness failed")
        if not new["dropped_outer_generator_binding"] or new["retained_prepared_owner_count"] != 9:
            raise RuntimeError("native natural SURFACE lost prepared owners")
        before_bytes = Path(old["before_file"]).read_bytes()
        after_bytes = Path(old["after_file"]).read_bytes()
        changed = sum(a[0] != b[0] for a, b in zip(struct.iter_unpack("<I", before_bytes),
                                                  struct.iter_unpack("<I", after_bytes)))
        neighbors = json.loads(Path(old["neighbor_queries_file"]).read_text())
        if not neighbors:
            raise RuntimeError("fixture did not actually read adjacent BIOMES images")
        if old["biome_source"] == "minecraft:overworld" and len(old["center_biome_counts"]) < 2:
            raise RuntimeError("selected overworld fixture was not a mixed natural biome image")
        delta = old["after"]["postprocessing_count"] - old["before"]["postprocessing_count"]
        if delta < 0:
            raise RuntimeError("SURFACE removed queued postprocessing")
        for before, after in zip(old["before"]["postprocessing_sections"], old["after"]["postprocessing_sections"]):
            if after[:len(before)] != before:
                raise RuntimeError("SURFACE reordered prior NOISE section postprocessing")
        results.append({key: old[key] for key in ["settings", "biome_source", "seed", "x", "z",
                        "level_min_y", "level_height", "chunks", "center_biome_counts",
                        "use_legacy_random_source", "first_surface_quart_query", "surface_quart_query_count", *checks]}
                       | dict(biome_palettes_equal=True, resolver_query_order_equal=True,
                              actual_neighbor_query_order_equal=True, neighbor_queries=len(neighbors),
                              before_blocks_equal=True, after_blocks_equal=True,
                              before_after_wg_heightmaps_equal=True,
                              before_after_postprocessing_positions_and_section_order_equal=True,
                              source_before=old["before"], source_after=old["after"],
                              surface_added_postprocessing=delta,
                              actual_surface_changed_blocks=changed,
                              palettes_sha256=sha(old["palettes_file"]),
                              resolver_queries_sha256=sha(old["resolver_queries_file"]),
                              neighbor_queries_sha256=sha(old["neighbor_queries_file"]),
                              before_blocks_sha256=sha(old["before_file"]),
                              after_blocks_sha256=sha(old["after_file"]), native_stage=new["stage"]))
    if not any(case["actual_surface_changed_blocks"] > 0 for case in results):
        raise RuntimeError("selected original/native SURFACE did not change any actual blocks")
    sources = ["src/worldgen/biomes.rs", "src/worldgen/generator.rs", "src/worldgen/noise_chunk.rs",
               "src/worldgen/biome_context.rs", "src/worldgen/biome_source.rs", "src/worldgen/climate.rs",
               "src/worldgen/biome_temperature.rs", "src/worldgen/biome_zoom.rs",
               "src/worldgen/surface/mod.rs", "src/worldgen/surface/rules.rs",
               "src/worldgen/validation/WorldgenSurfaceOracle.java",
               "src/worldgen/validation/WorldgenBiomeStageOracle.java",
               "src/worldgen/validation/WorldgenBiomeSurfaceOracle.java",
               "src/worldgen/validation/biome_surface.rs", "src/worldgen/validation/check_biome_surface.py",
               "src/worldgen/validation/check_biome_stage.py",
               "fixtures/world-generation-biome-climate.json", "fixtures/world-generation-surface-rules.json"]
    return dict(minecraft="1.21", protocol=767, oracle_inner_sha256=INNER,
                official_mappings_sha1=lock["mappings"]["sha1"],
                evidence="Actual unchanged original natural biome sources and real 3x3 section palettes, same-center public fillFromNoise, then actual SurfaceSystem with original zoom and thermal predicates; compared with native PreparedBiomes -> BiomeTerrain NOISE -> SURFACE and actual neighbor images",
                controlled_context="Actual initially EMPTY ProtoChunks with explicit empty Blender/Beardifier and absent retrogen. SURFACE quart lookup is a controlled read adapter over actual section palette snapshots using source ChunkAccess Y clamp. This does not construct WorldGenRegion or certify ChunkStep/status prerequisites; actual persisted statuses stay EMPTY.",
                chunk_order="delta X -1..1 outer, delta Z -1..1 inner; center is fifth image",
                worker_history="Same worker/preset search cursor across neighbor/case palette fills; one native worker-local thermal owner matches original built-in biome ThreadLocal history",
                postprocessing_scope="Exact actual before/after positions and insertion order inside each section queue, preserving duplicates. Source allocation flags are recorded separately; source section storage does not expose global inter-section call order.",
                cases=results, source_sha256={path: sha(ROOT / path) for path in sources},
                not_verified=["Actual WorldGenRegion/ChunkStep dependencies, persisted BIOMES/NOISE/SURFACE status transitions or Main scheduling.",
                              "Nonempty blending/structure adjustments, retrogen, datapack overrides or arbitrary registry identities.",
                              "Every natural biome/rule branch, carvers/features, lighting, persistence or FULL runtime chunks.",
                              "A positive SURFACE-only fluid postprocessing branch unless a recorded selected case actually adds entries."])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oracle-dir", type=Path, required=True)
    parser.add_argument("--java-home", type=Path)
    parser.add_argument("--work-dir", type=Path)
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    if args.work_dir:
        work = args.work_dir.resolve()
        if work == ROOT or ROOT in work.parents:
            parser.error("generated work directory must be outside the checkout")
        work.mkdir(parents=True, exist_ok=True)
        result = compare(args, work)
    else:
        with tempfile.TemporaryDirectory(prefix="cinderstone-biome-surface-") as temporary:
            result = compare(args, Path(temporary))
    if args.report:
        args.report.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(dict(success=True, cases=[dict(settings=x["settings"], x=x["x"], z=x["z"],
          biome_palettes_equal=x["biome_palettes_equal"], after_blocks_equal=x["after_blocks_equal"],
          actual_neighbor_query_order_equal=x["actual_neighbor_query_order_equal"]) for x in result["cases"]])))


if __name__ == "__main__":
    main()
