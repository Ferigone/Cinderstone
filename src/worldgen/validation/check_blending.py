#!/usr/bin/env python3
"""Actual old snapshots, Blender wrappers and retained BIOMES/NOISE/SURFACE.

The original controlled ChunkAccess snapshot uses real section/height/biome
owners. No WorldGenRegion, fake ProtoChunk status, scheduler or FULL is supplied.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile
import zipfile
from check_biome_stage import run, sha

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
INNER = "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"
BOOTSTRAP = """package oracle;
import java.net.*;import java.nio.file.*;
public class WorldgenBlendingBootstrap {
 public static void main(String[] a)throws Exception {
  try(URLClassLoader c=new URLClassLoader(new URL[]{Path.of(a[0]).toUri().toURL()},WorldgenBlendingBootstrap.class.getClassLoader())) {
   Class.forName("WorldgenBlendingOracle",true,c).getMethod("main",String[].class).invoke(null,(Object)new String[]{a[1]});
  }
 }
}
"""
PRODUCTION = ["src/density.rs", "src/density/chunk.rs", "src/worldgen/biomes.rs",
              "src/worldgen/generator.rs", "src/worldgen/noise_chunk.rs", "src/worldgen/mod.rs",
              "src/worldgen/biome_context.rs", "src/worldgen/biome_temperature.rs",
              "src/worldgen/biome_zoom.rs", "src/worldgen/surface/mod.rs",
              "src/worldgen/surface/rules.rs"] + [
                  "src/worldgen/blending/" + name + ".rs" for name in
                  ["mod", "data", "blender", "density", "biome", "ordered_map"]]
DEPENDENCIES = ["src/random.rs", "src/noise.rs", "src/hotspot_math.rs", "src/geometry.rs",
                "src/lighting.rs", "src/worldgen/aquifer.rs", "src/worldgen/random_state.rs",
                "src/worldgen/biome_source.rs", "src/worldgen/climate.rs", "src/placement.rs",
                "src/inventory.rs", "fixtures/source-lock.json", "fixtures/geometry-collisions-1.21.rs",
                "fixtures/placement-block-tags-1.21.json", "fixtures/world-generation-multinoise-parameters.json",
                "fixtures/world-generation-biome-climate.json", "fixtures/world-generation-surface-rules.json",
                "fixtures/density_builtin-1.21.json", "fixtures/noise_parameters-1.21.json",
                "fixtures/placement-1.21.json", "fixtures/items-1.21.json",
                "fixtures/item-components-1.21.json"]
PROBES = ["src/worldgen/validation/" + name for name in [
    "WorldgenBlendingOracle.java", "blending_probe.rs", "check_blending.py", "check_biome_stage.py",
    "WorldgenOreOracle.java", "WorldgenSurfaceOracle.java", "WorldgenBiomeStageOracle.java",
    "WorldgenBiomeSurfaceOracle.java", "WorldgenCarverOracle.java"]]


def require(value, message):
    if not value:
        raise RuntimeError(message)


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def image_bytes(image, identity, phase):
    raw = Path(image["file"]).read_bytes()
    require(len(raw) == 384 * 256 * 4, identity + ": incomplete " + phase + " array")
    if "sha256" in image:
        require(hashlib.sha256(raw).hexdigest() == image["sha256"], identity + ": stale " + phase + " digest")
    for key in ["world_surface_wg", "ocean_floor_wg"]:
        require(len(image[key]) == 256, identity + ": incomplete " + phase + " " + key)
    require(len(image["posts"]) == 24 and sum(map(len, image["posts"])) == image["post_count"],
            identity + ": " + phase + " section queues/count disagree")
    return raw


def compare_surface(query, old, new, before):
    identity = query["id"]
    source, native = old["surface"], new["surface"]
    require(source["status"] == "minecraft:empty" and native["stage"] == "Surface",
            identity + ": SURFACE runtime/status fabricated")
    require(source["same_noise_aquifer_blender"], identity + ": SURFACE replaced the original center owner")
    require(source["stored_biomes_unchanged"] and native["stored_biomes_unchanged"],
            identity + ": SURFACE changed retained BIOMES palettes")
    for key in ["world_surface_wg", "ocean_floor_wg", "posts", "post_count", "changed_blocks_from_noise"]:
        require(source[key] == native[key], identity + ": SURFACE " + key + " mismatch")
    after = image_bytes(source, identity, "original SURFACE")
    require(after == image_bytes(native, identity, "native SURFACE"), identity + ": complete SURFACE arrays differ")
    changed = sum(a[0] != b[0] for a, b in zip(struct.iter_unpack("<I", before), struct.iter_unpack("<I", after)))
    require(changed == source["changed_blocks_from_noise"], identity + ": SURFACE change count differs from actual arrays")
    delta = source["post_count"] - old["noise"]["post_count"]
    require(delta >= 0, identity + ": SURFACE removed prior NOISE postprocessing")
    for prior, current in zip(old["noise"]["posts"], source["posts"]):
        require(current[:len(prior)] == prior, identity + ": SURFACE reordered prior NOISE section postprocessing")

    cx, cz = query["chunk"]
    expected_chunks = [[cx + dx, cz + dz] for dx in range(-1, 2) for dz in range(-1, 2)]
    old_chunks = {tuple(entry["chunk"]) for entry in query["old"]}
    palettes, neighbors = {}, []
    require(len(old["surface_neighbors"]) == len(new["surface_neighbors"]) == 9,
            identity + ": incomplete SURFACE neighborhood")
    for chunk, original, actual in zip(expected_chunks, old["surface_neighbors"], new["surface_neighbors"]):
        owner = "center" if chunk == [cx, cz] else "old" if tuple(chunk) in old_chunks else "new"
        require(original["chunk"] == actual["chunk"] == chunk and original["owner"] == actual["owner"] == owner,
                identity + ": SURFACE neighbor order/owner mismatch")
        raw = Path(original["biome_file"]).read_bytes()
        require(raw == Path(actual["biome_file"]).read_bytes(), identity + ": complete SURFACE neighbor palettes differ")
        palette = json.loads(raw)
        require(len(palette) == 96 * 16 and all(isinstance(b, str) and b for b in palette),
                identity + ": incomplete SURFACE neighbor palette")
        if owner == "center":
            require(raw == Path(old["biome_file"]).read_bytes(), identity + ": SURFACE replaced center BIOMES")
        palettes[tuple(chunk)] = palette
        neighbors.append(dict(chunk=chunk, owner=owner, biome_sha256=hashlib.sha256(raw).hexdigest()))

    complete_queries = old["surface_source_all_biome_queries"]
    require(complete_queries, identity + ": original SURFACE made no quart biome query")
    for row in complete_queries:
        require(len(row) == 4, identity + ": malformed original SURFACE quart query")
        x, y, z, biome = row
        chunk = (x >> 2, z >> 2)
        require(chunk in palettes, identity + ": original SURFACE queried outside supplied neighborhood")
        # All fixture ChunkAccess owners expose the actual full -64..320
        # accessor, independently of their old-generation BlendingData bounds.
        index = (max(-16, min(79, y)) + 16) * 16 + (z & 3) * 4 + (x & 3)
        require(palettes[chunk][index] == biome, identity + ": SURFACE query differs from actual clamped palette")
    foreign_queries = [row for row in complete_queries if (row[0] >> 2, row[2] >> 2) != (cx, cz)]
    require(old["surface_biome_queries"] == new["surface_biome_queries"] == foreign_queries,
            identity + ": ordered foreign SURFACE quart callbacks differ")
    return dict(complete_surface_arrays_equal=True, surface_heightmaps_posts_equal=True,
        actual_surface_changed_blocks=changed, surface_added_postprocessing=delta,
        surface_sha256=hashlib.sha256(after).hexdigest(), surface_native_stage="Surface",
        surface_original_status="minecraft:empty", surface_same_original_noise_aquifer_blender=True,
        surface_stored_biomes_unchanged=True,
        surface_neighbors=neighbors, surface_foreign_quart_queries=len(foreign_queries),
        surface_foreign_quart_queries_sha256=digest(foreign_queries),
        surface_source_all_quart_queries=len(complete_queries),
        surface_source_all_quart_queries_sha256=digest(complete_queries),
        source_surface_heightmap_post_sha256=digest({k:source[k] for k in
            ["world_surface_wg", "ocean_floor_wg", "posts", "post_count"]}))


def roots():
    alpha, offset = dict(type="minecraft:blend_alpha"), dict(type="minecraft:blend_offset")
    return [dict(name="alpha", function=alpha), dict(name="offset", function=offset),
            dict(name="density", function=dict(type="minecraft:blend_density", argument=0.375)),
            dict(name="min", function=dict(type="minecraft:min", argument1=0.5, argument2=alpha)),
            dict(name="max", function=dict(type="minecraft:max", argument1=0.5, argument2=offset)),
            dict(name="spline", function=dict(type="minecraft:spline", spline=dict(coordinate=alpha,
                 points=[dict(location=0.5, value=2.0, derivative=1.0)])))]


def cases():
    def old(x, z, profile, biome=None):
        result = dict(chunk=[x, z], profile=profile, min_section=0, max_section=16)
        if biome:
            result["biome"] = "minecraft:" + biome
        return result
    entries = [("empty_singleton", [0, 0], [], True), ("constructed_zero_maps", [0, 0], [], False),
               ("west_old", [0, 0], [old(-1, 0, 0)], False),
               ("high_old", [0, 0], [old(-1, 0, 8, "badlands")], False),
               ("corner_ties", [0, 0], [old(-1, 0, 0, "plains"), old(0, -1, 8, "badlands"), old(1, 1, 3, "desert")], False),
               ("negative_chunk", [-2, 3], [old(-3, 3, 1)], False),
               ("initial_cache_sentinel", [117190, 117190], [old(117191, 117191, 4, "plains")], False)]
    result = []
    for identity, chunk, data, singleton in entries:
        x, z = [c * 16 for c in chunk]
        qx, qz = [c * 4 for c in chunk]
        result.append(dict(id=identity, settings="minecraft:overworld", seed="123456789", chunk=chunk,
            old=data, singleton_empty=singleton, custom_roots=roots(),
            points=[[x, 0, z], [x, 64, z], [x+7, 70, z+7], [x+40, -7, z+40],
                    [x-1, 8, z-1], [x+111, 0, z], [x+112, 0, z], [x+32, 257, z+32]],
            biome_points=[[qx, -1, qz], [qx, 0, qz], [qx+1, 32, qz+1], [qx, 63, qz],
                          [qx, 64, qz], [qx+8, 32, qz+8], [qx+27, 32, qz], [qx+28, 32, qz]],
            cache_points=[[1875066, 0, 1875066], [x, 16, z], [x+16, 0, z+16], [x+17, 64, z+17],
                          [x+20, 0, z+20], [x-1, 0, z-1], [1875066, 0, 1875066]]))
    return result


def compare(args, work):
    composed = run(["git", "rev-parse", "HEAD"], cwd=ROOT).strip()
    if args.native_source_commit:
        for path in PRODUCTION + DEPENDENCIES:
            raw = subprocess.run(["git", "show", args.native_source_commit + ":" + path], cwd=ROOT,
                                 capture_output=True, check=True).stdout
            require(hashlib.sha256(raw).hexdigest() == sha(ROOT / path), "production/dependency changed after source commit: " + path)
    oracle = args.oracle_dir.resolve()
    lock = json.loads((ROOT / "fixtures/source-lock.json").read_text())
    for section, name in [("oracle", "server-1.21.jar"), ("mappings", "server-1.21-mappings.txt")]:
        raw = (oracle / name).read_bytes()
        require(len(raw) == lock[section]["size"] and hashlib.sha1(raw).hexdigest() == lock[section]["sha1"], "wrong pinned " + name)
    jar = oracle / "server-inner-1.21.jar"
    require(sha(jar) == INNER, "wrong pinned inner JAR")
    java = str(args.java_home / "bin/java") if args.java_home else shutil.which("java")
    javac = str(args.java_home / "bin/javac") if args.java_home else shutil.which("javac")
    require(java and javac, "Java 21 java/javac required")
    parent, child = work / "parent-classes", work / "child-classes"
    parent.mkdir(exist_ok=True)
    child.mkdir(exist_ok=True)
    bootstrap = work / "WorldgenBlendingBootstrap.java"
    bootstrap.write_text(BOOTSTRAP)
    cp = os.pathsep.join(map(str, [parent, jar, *sorted((oracle / "libraries").rglob("*.jar"))]))
    helpers = [HERE / f"Worldgen{name}Oracle.java" for name in ["Surface", "BiomeStage", "BiomeSurface", "Carver"]]
    run([javac, "-proc:none", "-cp", cp, "-d", str(parent), str(bootstrap), *map(str, helpers)])
    run([javac, "-proc:none", "-cp", cp, "-d", str(child), str(HERE / "WorldgenOreOracle.java"), str(HERE / "WorldgenBlendingOracle.java")])
    if not args.skip_build:
        run(["cargo", "build", "--locked", "--lib"], cwd=ROOT)
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    if not target.is_absolute():
        target = ROOT / target
    deps = target / "debug/deps"
    extern = ["--extern", "mc121_rust=" + str(target / "debug/libmc121_rust.rlib")]
    for name in ["anyhow", "serde_json"]:
        choices = sorted(deps.glob(f"lib{name}-*.rlib"), key=lambda p: p.stat().st_mtime, reverse=True)
        require(choices, "missing dependency " + name)
        extern += ["--extern", f"{name}={choices[0]}"]
    probe = work / "blending_probe"
    run(["rustc", "--edition=2021", str(HERE / "blending_probe.rs"), "-L", "dependency=" + str(deps), *extern, "-o", str(probe)])
    queries = cases()
    transcript = "".join(json.dumps(q) + "\n" for q in queries)
    original = [json.loads(line) for line in run([java, "-Xmx1600m", "-cp", cp, "oracle.WorldgenBlendingBootstrap",
                str(child), str(work)], input=json.dumps(dict(mode="maps")) + "\n" + transcript).splitlines() if line.startswith("{")]
    require(original, "original map transcript missing")
    map_input = dict(mode="maps", collision_keys=[row[:2] for row in original[0]["collision_keys"]])
    native = [json.loads(line) for line in run([str(probe), str(work)], input=json.dumps(map_input) + "\n" + transcript).splitlines()]
    (work / "original-blending.json").write_text(json.dumps(original) + "\n")
    (work / "native-blending.json").write_text(json.dumps(native) + "\n")
    maps = original.pop(0)
    with zipfile.ZipFile(oracle / "server-1.21.jar") as archive:
        libraries = archive.read("META-INF/libraries.list").decode().splitlines()
    fastutil = [line.split()[0] for line in libraries if "it.unimi.dsi:fastutil:" in line]
    require(len(fastutil) == 1 and maps["fastutil_jar_sha256"] == fastutil[0], "loaded Fastutil differs from pinned original library")
    native_maps = native.pop(0)
    source_orders = [{"size":row["size"], "order":row["order"]} for row in maps["stencil"]]
    require(source_orders == native_maps["stencil"], "actual native source-map order/value mismatch across growth thresholds")
    for key in ["replacement_preserves_order", "replacement_returns_old_owner", "replacement_retains_new_owner", "collision_order"]:
        require(maps[key] == native_maps[key], "native actual map " + key + " mismatch")
    require(len(original) == len(native) == len(queries), "incomplete paired blending transcript")
    require([row["capacity"] for row in maps["stencil"]] == [32, 32, 32, 64, 64, 128, 128, 256, 256, 512], "actual full-stencil map growth changed")
    require(all(maps[key] for key in ["replacement_preserves_order", "replacement_returns_old_owner", "replacement_retains_new_owner"]), "original map owner replacement failed")
    compared = ["query", "data", "age_queries", "second_calculation_skipped", "height_map_order", "density_map_order",
                "height_to_offset", "shift_noise", "scalars", "resolved_biomes", "resolver_calls", "prefill", "cache_samples",
                "density_contexts", "custom", "biome_fallback"]
    observations, results, by_id = [], [], {}
    for query, old, new in zip(queries, original, native):
        identity = query["id"]
        require(old["oracle_inner_sha256"] == INNER, identity + ": wrong actually loaded original")
        require(old["query"] == new["query"] == query, identity + ": paired case query changed")
        for key in compared:
            if old[key] != new[key]:
                detail = ""
                if isinstance(old[key], list) and isinstance(new[key], list):
                    first = next(((i, a, b) for i, (a, b) in enumerate(zip(old[key], new[key])) if a != b), None)
                    detail = f" lengths {len(old[key])}/{len(new[key])}; first={first}"
                raise RuntimeError(identity + ": " + key + " mismatch" + detail)
        require(old["second_calculation_skipped"] and all(d["calculated"] and d["source_status"] == "minecraft:empty" for d in old["data"]), identity + ": snapshot one-time/status witness failed")
        require(old["same_noise_aquifer_blender"] and new["prepared_blender_identity"], identity + ": retained job owner witness failed")
        require(old["noise"]["status"] == "minecraft:empty" and new["noise"]["stage"] == "Noise", identity + ": runtime/status fabricated")
        for key in ["world_surface_wg", "ocean_floor_wg", "posts", "post_count"]:
            require(old["noise"][key] == new["noise"][key], identity + ": NOISE " + key + " mismatch")
        a, b = image_bytes(old["noise"], identity, "original NOISE"), image_bytes(new["noise"], identity, "native NOISE")
        require(a == b, identity + ": complete NOISE arrays differ")
        require(Path(old["biome_file"]).read_bytes() == Path(new["biome_file"]).read_bytes(), identity + ": complete BIOMES palettes differ")
        require(all(call[3] for call in old["biome_fallback"] + old["resolver_calls"]), identity + ": original fallback sampler identity changed")
        surface = compare_surface(query, old, new, a)
        snapshot = {key: old[key] for key in compared if key not in ["biome_fallback"]}
        snapshot |= dict(noise_sha256=sha(old["noise"]["file"]), biome_sha256=sha(old["biome_file"]),
            biome_fallback_count=len(old["biome_fallback"]), biome_fallback_sha256=digest(old["biome_fallback"]),
            noise_heightmap_post_sha256=digest({k:old["noise"][k] for k in ["world_surface_wg", "ocean_floor_wg", "posts", "post_count"]}),
            original_status="minecraft:empty", original_same_noise_aquifer_blender=True,
            source_last_cache_after_samples=old["last_cache_after_samples"])
        snapshot |= surface
        observations.append(snapshot)
        results.append(dict(id=identity, data_and_iterator_bits_equal=True, function_bits_equal=True,
            lazy_biome_resolution_equal=True, wrapped_bounds_scalar_fill_equal=True, provider_index_calls_equal=True,
            complete_biome_palettes_equal=True, complete_noise_arrays_equal=True, noise_heightmaps_posts_equal=True,
            original_status_remains_empty=True, native_stage="Noise", noise_sha256=sha(old["noise"]["file"]),
            biome_sha256=sha(old["biome_file"]), biome_fallback_count=len(old["biome_fallback"])) | surface)
        by_id[identity] = (old, new)
    empty = by_id["empty_singleton"][0]
    zero = by_id["constructed_zero_maps"][0]
    require(empty["noise"]["sha256"] == zero["noise"]["sha256"], "zero-map constructor changed baseline terrain")
    for identity in ["west_old", "high_old", "corner_ties"]:
        row = by_id[identity][0]
        require(row["noise"]["sha256"] != empty["noise"]["sha256"], identity + ": no actual nonempty NOISE effect")
        require(any(s[3] != "4607182418800017408" and s[4] != "0" for s in row["scalars"]), identity + ": no nonempty alpha/offset effect")
        require(any(s[5] != "4600427019358961664" for s in row["scalars"]), identity + ": no density effect")
        require(0 < len(row["biome_fallback"]) < 96 * 16, identity + ": no lazy old/new biome branch coverage")
    require(any(row["actual_surface_changed_blocks"] > 0 and row["surface_foreign_quart_queries"] > 0
                for row in results if row["id"] in ["west_old", "high_old", "corner_ties"]),
            "selected nonempty original SURFACE did not change actual NOISE blocks and query adjacent BIOMES")
    require(by_id["west_old"][0]["density_contexts"][0][-1] != by_id["west_old"][0]["density_contexts"][1][-1], "owner/external density contexts did not differ")
    sentinel = by_id["initial_cache_sentinel"][0]["cache_samples"]
    require(sentinel[0][3:] == ["4607182418800017408", "0"] and sentinel[-1][3:] != sentinel[0][3:], "initial shared-last sentinel behavior absent")
    custom_empty = {row["name"]:row for row in empty["custom"]}
    custom_zero = {row["name"]:row for row in zero["custom"]}
    require(custom_empty["alpha"]["wrapped_bounds"] != custom_zero["alpha"]["wrapped_bounds"] and
            custom_empty["alpha"]["for_index_calls"] == [] and custom_zero["alpha"]["for_index_calls"], "EMPTY versus constructed zero-map wrapper identity absent")
    # The original rows retain exact ties; distinct old fixed-source palettes
    # are explicit test inputs, never expected native output supplied to Java.
    tie = by_id["corner_ties"][0]
    qx, qy, qz = tie["query"]["biome_points"][2]
    candidates = []
    for chunk in tie["height_map_order"]:
        entry = next(d for d in tie["data"] if d["chunk"] == chunk)
        row = next(r for r in entry["iterate_biomes"] if r["quart_y"] == qy)
        for x, z, biome in row["values"]:
            candidates.append(((x-qx)**2 + (z-qz)**2, biome))
    nearest = min(distance for distance, _ in candidates)
    tied = [biome for distance, biome in candidates if distance == nearest]
    require(len(set(tied)) >= 2 and tie["resolved_biomes"][2][3] == tied[0], "observable equal-distance original biome tie absent")
    hashes = {path:sha(ROOT / path) for path in PRODUCTION + DEPENDENCIES}
    probes = {path:sha(ROOT / path) for path in PROBES}
    common = dict(minecraft="1.21", protocol=767, oracle_inner_sha256=INNER,
        native_source_commit=args.native_source_commit, native_composed_commit=composed,
        production_sha256=hashes, probe_sha256=probes, bootstrap_sha256=hashlib.sha256(BOOTSTRAP.encode()).hexdigest())
    runtime = common | dict(evidence="unchanged original calculated-data/function/cache/BIOMES/NOISE/SURFACE runtime observations",
                            map_witnesses=maps, cases=observations)
    report = common | dict(success=True, cases=results,
        scope="Actual calculated old-data snapshots and private Blender construction; coherent retained BIOMES then public NOISE then actual SURFACE over explicit empty target/beardifier. No actual WorldGenRegion or ChunkStep.",
        snapshot_scope="Controlled original ChunkAccess subclass with actual block/height/biome section storage and base getNoiseBiome. All snapshot and target statuses remain EMPTY. Explicit old fixed-biome sources are fixture inputs; remaining old palettes use the retained raw multi-noise worker.",
        collection_scope="Source-reviewed X-major/Z-minor region scan adapter over supplied calculated snapshots, not execution of Blender.of(WorldGenRegion)/getOrUpdateBlendingData. Actual sideByGenerationAge executes against a rejecting WorldGenLevel interface proxy.",
        owner_scope="Component wrapper probes mutate an independent real ChunkDensity/NoiseChunk sharing the same immutable Blender. Separate full-job BIOMES then NOISE then SURFACE retain the same center job; original center NoiseChunk/aquifer/Blender identity is checked before and after. Probing a sentinel cannot warm that job's cache. Neighbor BIOMES owners are constructed only after capturing the existing center NOISE image, in delta-X outer/delta-Z inner order; the center is reused.",
        surface_scope="SURFACE reads actual ordered 3x3 section palette owners through a controlled callback. Old snapshot palettes retain the actual full level accessor and source quart-Y clamp, independently of their narrower BlendingData generation bounds. This does not execute or recollect through WorldGenRegion. All original targets remain EMPTY; native center data advances to Surface only.",
        surface_trace_scope="The complete original SURFACE quart callback trace is observed. Only foreign-center callbacks are compared with native, because the existing typed neighborhood resolves center cells internally; the source-only center trace is not claimed as a native trace comparison. Every original callback value is checked against the actual clamped palette.",
        surface_postprocessing_scope="Complete before/after section queues and insertion order are compared, preserving duplicates and each NOISE prefix. An observed zero SURFACE-added count proves retained NOISE queues, not a positive SURFACE-only insertion branch; no global inter-section call order is inferred.",
        layout="NOISE and SURFACE canonical u32 LE: Y, local X, local Z. BIOMES: section Y, local quart Y/Z/X. Data/iterator/map arrays retain original encounter order.",
        map_scope="The native probe compiles the exact production ordered_map.rs by path and compares primitive Arc value-owner order/replacement/collisions over193source-stencil keys at growth thresholds. Original capacities are observed; native capacity is not inspected. Actual calculated-data Blender functions are also paired independently; map primitive values never stand in for terrain data.",
        native_map_order_growth_replacement_equal=True,
        not_verified=["Actual concrete ServerLevel/WorldGenRegion construction, Blender.of/getOrUpdate public region entry points or storage old-chunk scan.",
                      "Structure-beardifier effects, BelowZeroRetrogen, existing target block preservation, old/new CARVERS masks or border ticks.",
                      "Actual WorldGenRegion SURFACE/status prerequisites, complete FEATURES, lighting, persistence, Main integration or FULL publication.",
                      "Every SURFACE biome/rule branch or a positive SURFACE-only postprocessing insertion unless a selected case actually adds entries.",
                      "Arbitrary out-of-contract protected getter coordinates, datapack registries or natural historical terrain generation."])
    return runtime, report


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--oracle-dir", type=Path, required=True)
    parser.add_argument("--java-home", type=Path)
    parser.add_argument("--work-dir", type=Path)
    parser.add_argument("--native-source-commit", help="Verified immutable production-source commit; all production blobs must match")
    parser.add_argument("--skip-build", action="store_true", help="Only after separate explicit cargo build --lib of the same source/target")
    parser.add_argument("--runtime-report", type=Path)
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    if args.work_dir:
        work = args.work_dir.resolve()
        require(work != ROOT and ROOT not in work.parents, "work directory must be outside checkout")
        work.mkdir(parents=True, exist_ok=True)
        runtime, report = compare(args, work)
    else:
        with tempfile.TemporaryDirectory(prefix="cinderstone-blending-") as temporary:
            runtime, report = compare(args, Path(temporary))
    for path, value in [(args.runtime_report, runtime), (args.report, report)]:
        if path:
            path.write_text(json.dumps(value, indent=2) + "\n")
    print(json.dumps(dict(success=True, scope="controlled old-data Blender and retained BIOMES/NOISE/SURFACE",
        cases=[dict(id=r["id"], noise_equal=r["complete_noise_arrays_equal"], biomes_equal=r["complete_biome_palettes_equal"],
                    surface_equal=r["complete_surface_arrays_equal"], surface_changed_blocks=r["actual_surface_changed_blocks"],
                    surface_added_postprocessing=r["surface_added_postprocessing"]) for r in report["cases"]])))


if __name__ == "__main__":
    main()
