#!/usr/bin/env python3
"""Compare actual source BIOMES palette filling then same-NoiseChunk public NOISE.

A controlled original fixture supplies explicit empty blending/beardifier,
actual built-in sources and full dimension height accessors. No ChunkStep,
server scheduling, synthetic biome or FULL runtime completion is supplied.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
INNER = "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def run(command, **kwargs):
    result = subprocess.run(command, text=True, capture_output=True, **kwargs)
    if result.returncode:
        raise RuntimeError(f"{command[0]} failed ({result.returncode}):\n"
                           + result.stdout[-3000:] + result.stderr[-6000:])
    return result.stdout


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
         str(HERE / "WorldgenSurfaceOracle.java"), str(HERE / "WorldgenBiomeStageOracle.java")])
    run(["cargo", "build", "--locked", "--lib"], cwd=ROOT)
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    if not target.is_absolute():
        target = ROOT / target
    deps = target / "debug/deps"
    extern = ["--extern", "mc121_rust=" + str(target / "debug/libmc121_rust.rlib")]
    for name in ["anyhow", "serde_json"]:
        libraries = sorted(deps.glob(f"lib{name}-*.rlib"), key=lambda p: p.stat().st_mtime, reverse=True)
        if not libraries:
            raise RuntimeError(f"missing built dependency {name}")
        extern += ["--extern", name + "=" + str(libraries[0])]
    probe = work / "biome_stage"
    run(["rustc", "--edition=2021", str(HERE / "biome_stage.rs"), "-L", "dependency=" + str(deps),
         *extern, "-o", str(probe)])
    cases = [("overworld", "overworld", 0, 0, -64, 384),
             ("overworld", "overworld", -2, 3, -64, 384),
             ("overworld", "overworld", -640, 320, -64, 384),
             ("nether", "nether", 0, 0, 0, 256),
             ("end", "end", 0, 0, 0, 256), ("end", "end", 65, 0, 0, 256)]
    queries = [dict(settings="minecraft:" + s, biome_source="minecraft:" + b, seed=123456789,
                    x=x, z=z, level_min_y=y, level_height=h) for s, b, x, z, y, h in cases]
    transcript = "".join(json.dumps(query) + "\n" for query in queries)
    original = [json.loads(line) for line in run([
        java, "-Xmx1024m", "-cp", cp, "oracle.WorldgenBiomeStageOracle", str(work)], input=transcript).splitlines()]
    native = [json.loads(line) for line in run([str(probe), str(work)], input=transcript).splitlines()]
    if len(original) != len(queries) or len(native) != len(queries):
        raise RuntimeError("incomplete original/native BIOMES stage transcript")
    results = []
    for old, new in zip(original, native):
        if old["oracle_inner_sha256"] != INNER:
            raise RuntimeError("wrong loaded original NoiseChunk artifact")
        if old["initial_status"] != "minecraft:empty":
            raise RuntimeError("original fixture did not start with actual EMPTY status")
        for key in ["settings", "biome_source", "seed", "x", "z", "level_min_y", "level_height",
                    "biome_counts", "world_surface_wg", "ocean_floor_wg", "postprocessing_count",
                    "postprocessing_min_section_y", "postprocessing_sections", "postprocessing_positions_by_section"]:
            if old[key] != new[key]:
                raise RuntimeError(f"BIOMES stage {key} mismatch: {old['settings']} at {old['x']},{old['z']}")
        for key in ["biomes_file", "queries_file", "blocks_file"]:
            if Path(old[key]).read_bytes() != Path(new[key]).read_bytes():
                raise RuntimeError(f"BIOMES stage {key} mismatch: {old['settings']} at {old['x']},{old['z']}")
        source_checks = ["same_noise_chunk", "same_aquifer", "same_cache_owners", "same_sampler",
                         "persisted_status_unchanged", "noise_future_same_chunk", "source_pack_roundtrip"]
        native_checks = ["biomes_retained", "dropped_outer_generator_binding", "owned_noise_worker"]
        if not all(old[key] for key in source_checks) or not all(new[key] for key in native_checks) or new["stage"] != "Noise":
            raise RuntimeError("BIOMES owner/lifetime/status witness failed")
        if sum(map(len, old["postprocessing_sections"])) != old["postprocessing_count"]:
            raise RuntimeError("original postprocessing section contents/count disagree")
        if len(old["source_postprocessing_allocated"]) != len(old["postprocessing_sections"]):
            raise RuntimeError("original postprocessing allocation span is incomplete")
        nonempty_sections = [index for index, values in enumerate(old["postprocessing_sections"]) if values]
        palette = json.loads(Path(old["biomes_file"]).read_text())
        queries = json.loads(Path(old["queries_file"]).read_text())
        results.append({key: old[key] for key in ["settings", "biome_source", "seed", "x", "z",
                        "level_min_y", "level_height", "biome_counts", "postprocessing_count", "initial_status", *source_checks]}
                       | {key: new[key] for key in native_checks}
                       | dict(biomes_equal=True, query_order_equal=True, noise_arrays_equal=True,
                              heightmaps_equal=True,
                              noise_postprocessing_positions_and_per_section_order_equal=True,
                              postprocessing_min_section_y=old["postprocessing_min_section_y"],
                              postprocessing_sections=old["postprocessing_sections"],
                              source_postprocessing_allocated=old["source_postprocessing_allocated"],
                              actual_nonempty_postprocessing_sections=nonempty_sections,
                              biome_sha256=sha(old["biomes_file"]),
                              queries_sha256=sha(old["queries_file"]), noise_sha256=sha(old["blocks_file"]),
                              biome_entries=len(palette), query_entries=len(queries),
                              first_query=queries[0], last_query=queries[-1], native_stage=new["stage"]))
    if not any(case["actual_nonempty_postprocessing_sections"] for case in results):
        raise RuntimeError("no actual nonempty NOISE postprocessing section-order witness")
    sources = ["src/worldgen/biomes.rs", "src/worldgen/generator.rs", "src/worldgen/noise_chunk.rs",
               "src/worldgen/biome_source.rs", "src/worldgen/climate.rs",
               "src/worldgen/validation/WorldgenSurfaceOracle.java",
               "src/worldgen/validation/WorldgenBiomeStageOracle.java",
               "src/worldgen/validation/biome_stage.rs", "src/worldgen/validation/check_biome_stage.py"]
    return dict(minecraft="1.21", protocol=767, oracle_inner_sha256=INNER,
                official_mappings_sha1=lock["mappings"]["sha1"],
                evidence="Actual unchanged original ChunkAccess.fillBiomesFromNoise/LevelChunkSection palettes and seeded MultiNoise/End sources, followed by actual public NoiseBasedChunkGenerator.fillFromNoise on the same installed NoiseChunk/ProtoChunk, compared with native owned BIOMES data -> NOISE",
                controlled_context="Fresh ProtoChunk at actual level height; actual empty Blender and BeardifierMarker; no retrogen. StructureManager is absent but source-cache-branch-proved unused because the actual NoiseChunk is already installed. No actual WorldGenRegion or server dependency scheduler is claimed.",
                biome_layout="section Y ascending, local quart Y/Z/X (X fastest), 64 canonical names per section",
                query_layout="actual original resolver calls: section Y ascending, local quart X/Y/Z (Z fastest)",
                worker_history="One persistent original/native RTree search history per multi-noise preset on the BIOMES worker, retained across the recorded chunk sequence; source cursors are not moved with NOISE jobs",
                noise_postprocessing_layout="All actual source section lists ascending section Y; packed short X&15 | (Y&15)<<4 | (Z&15)<<8. Native global callback vector is grouped by actual section without sorting/deduplication. Null and allocated-empty source lists normalize only to zero queued entries; actual allocation flags are retained separately.",
                postprocessing_order_scope="Exact NOISE positions and insertion order within each source section list; source section storage does not expose global inter-section callback order. This result does not compare SURFACE postprocessing.",
                cases=results, source_sha256={path: sha(ROOT / path) for path in sources},
                not_verified=["Actual ChunkStep/status completion, WorldGenRegion dependencies or Main scheduling.",
                              "Nonempty blending/structure adjustments, retrogen or datapack registry overrides.",
                              "Neighbor BIOMES-driven SURFACE, carvers/features, lighting, persistence or FULL runtime chunks."])


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--oracle-dir", type=Path, required=True)
    parser.add_argument("--java-home", type=Path)
    parser.add_argument("--work-dir", type=Path)
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    if args.work_dir:
        work = args.work_dir.resolve()
        if work == ROOT or ROOT in work.parents:
            raise RuntimeError("work directory must be outside the checkout")
        work.mkdir(parents=True, exist_ok=True)
        result = compare(args, work)
    else:
        with tempfile.TemporaryDirectory(prefix="cinderstone-biome-stage-") as temporary:
            result = compare(args, Path(temporary))
    if args.report:
        args.report.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(dict(success=True, cases=[dict(settings=x["settings"], x=x["x"], z=x["z"],
          biomes_equal=x["biomes_equal"], query_order_equal=x["query_order_equal"],
          noise_arrays_equal=x["noise_arrays_equal"]) for x in result["cases"]])))


if __name__ == "__main__":
    main()
