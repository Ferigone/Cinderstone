#!/usr/bin/env python3
"""Pair original configured ore leaves with native caller-owned leaf execution.

Actual original ProtoChunks/sections and a rejecting controlled WorldGenLevel
proxy are used. This does not construct WorldGenRegion, a decorator pipeline,
FEATURES status, Main integration, POI hooks or a completed runtime chunk.
"""
import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
from check_biome_stage import run, sha

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
INNER = "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"
SOURCE_COMMIT = "2de9240eae9ffddeeab68e234ef2d542e788e411"
BOOTSTRAP = """package oracle;
import java.net.*;
import java.nio.file.*;
public class WorldgenOreBootstrap {
 public static void main(String[] args)throws Exception {
  try(URLClassLoader child=new URLClassLoader(new URL[]{Path.of(args[0]).toUri().toURL()},WorldgenOreBootstrap.class.getClassLoader())) {
   Class<?> probe=Class.forName("WorldgenOreOracle",true,child);
   probe.getMethod("main",String[].class).invoke(null,(Object)new String[]{args[1]});
  }
 }
}
"""
PRODUCTION = ["src/worldgen/feature/" + x for x in
              ["config.rs", "host.rs", "mod.rs", "ore.rs", "scattered_ore.rs"]] + [
                  "src/worldgen/mod.rs", "fixtures/world-generation-ore-configs.json"]
DEPENDENCIES = ["src/random.rs", "src/geometry.rs", "src/hotspot_math.rs",
                "src/placement.rs", "src/inventory.rs", "src/lighting.rs",
                "src/worldgen/generator.rs", "fixtures/source-lock.json",
                "fixtures/light-1.21.json", "fixtures/placement-1.21.json",
                "fixtures/placement-block-tags-1.21.json", "fixtures/items-1.21.json",
                "fixtures/geometry-collisions-1.21.rs"]
PROBES = ["src/worldgen/validation/" + x for x in [
    "WorldgenOreOracle.java", "ore_probe.rs", "check_ores.py", "check_biome_stage.py",
    "WorldgenSurfaceOracle.java", "WorldgenBiomeStageOracle.java",
    "WorldgenBiomeSurfaceOracle.java", "WorldgenCarverOracle.java"]]


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def cases():
    base = dict(rng_seed="123456789", origin=[15, -20, 15], min_y=-64,
                height=384, top_y=95, profile="layered", available_radius=1, write_radius=1)
    result = []

    def add(identity, feature, **fields):
        result.append(base | dict(id=identity, feature="minecraft:" + feature) | fields)

    add("iron_layered", "ore_iron")
    add("iron_stone", "ore_iron", origin=[15, 40, 15])
    add("copper_cross", "ore_copper_large", origin=[14, 42, 14], profile="stone", rng_seed="0")
    add("diamond_partial_air", "ore_diamond_large", profile="air_plane")
    add("diamond_buried_air", "ore_diamond_buried", profile="air_plane")
    add("diamond_bottom_air", "ore_diamond_buried", origin=[7, -62, 7])
    nether = dict(origin=[7, 32, 7], min_y=0, height=128, top_y=127,
                  profile="netherrack", rng_seed="1")
    add("ancient_large", "ore_ancient_debris_large", **nether)
    add("ancient_small", "ore_ancient_debris_small", **(nether | dict(rng_seed="4")))
    add("ancient_radius_veto", "ore_ancient_debris_large",
        **(nether | dict(origin=[15, 32, 15], write_radius=0, rng_seed="3")))
    add("scattered_zero_attempts", "ore_ancient_debris_small", **(nether | dict(rng_seed="2")))
    add("below_height_guard", "ore_iron", origin=[7, -64, 7])
    add("above_height_preflight", "ore_iron", origin=[7, 330, 7], top_y=319)
    add("origin_rejected", "ore_iron", write_radius=-1)
    add("scattered_origin_rejected", "ore_ancient_debris_small", write_radius=-1)
    add("regular_neighbor_write_guard", "ore_copper_large", origin=[14, 42, 14],
        profile="stone", rng_seed="0", write_radius=0)
    add("missing_after_acquire", "ore_copper_large", origin=[14, 42, 14],
        profile="stone", rng_seed="1", available_radius=0)
    return result


def compare(args, work):
    composed_commit = run(["git", "rev-parse", "HEAD"], cwd=ROOT).strip()
    for path in PRODUCTION:
        raw = subprocess.run(["git", "show", SOURCE_COMMIT + ":" + path], cwd=ROOT,
                             capture_output=True, check=True).stdout
        require(hashlib.sha256(raw).hexdigest() == sha(ROOT / path),
                "ore source changed after pinned implementation commit: " + path)
    oracle = args.oracle_dir.resolve()
    lock = json.loads((ROOT / "fixtures/source-lock.json").read_text())
    for section, name in [("oracle", "server-1.21.jar"), ("mappings", "server-1.21-mappings.txt")]:
        raw = (oracle / name).read_bytes()
        require(len(raw) == lock[section]["size"] and hashlib.sha1(raw).hexdigest() == lock[section]["sha1"],
                "wrong pinned artifact " + name)
    jar = oracle / "server-inner-1.21.jar"
    require(sha(jar) == INNER, "wrong pinned inner server")
    java = str(args.java_home / "bin/java") if args.java_home else shutil.which("java")
    javac = str(args.java_home / "bin/javac") if args.java_home else shutil.which("javac")
    require(java and javac, "Java 21 java/javac required")
    parent, child = work / "parent-classes", work / "ore-classes"
    parent.mkdir(exist_ok=True)
    child.mkdir(exist_ok=True)
    bootstrap = work / "WorldgenOreBootstrap.java"
    bootstrap.write_text(BOOTSTRAP)
    cp = os.pathsep.join(map(str, [parent, jar, *sorted((oracle / "libraries").rglob("*.jar"))]))
    # Minecraft's signed default-package classes stay in the parent loader;
    # unsigned observing subclasses use a child loader, without rewriting JARs.
    helper_sources = [HERE / f"Worldgen{x}Oracle.java" for x in ["Surface", "BiomeStage", "BiomeSurface", "Carver"]]
    run([javac, "-proc:none", "-cp", cp, "-d", str(parent), str(bootstrap), *map(str, helper_sources)])
    run([javac, "-proc:none", "-cp", cp, "-d", str(child), str(HERE / "WorldgenOreOracle.java")])
    if not args.skip_build:
        run(["cargo", "build", "--locked", "--lib"], cwd=ROOT)
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    if not target.is_absolute():
        target = ROOT / target
    deps = target / "debug/deps"
    extern = ["--extern", "mc121_rust=" + str(target / "debug/libmc121_rust.rlib")]
    for name in ["anyhow", "serde_json"]:
        choices = sorted(deps.glob(f"lib{name}-*.rlib"), key=lambda p: p.stat().st_mtime, reverse=True)
        require(choices, "missing native dependency " + name)
        extern += ["--extern", f"{name}={choices[0]}"]
    probe = work / "ore_probe"
    run(["rustc", "--edition=2021", str(HERE / "ore_probe.rs"), "-L", "dependency=" + str(deps),
         *extern, "-o", str(probe)])
    queries = cases()
    transcript = "".join(json.dumps(q) + "\n" for q in [dict(mode="configs"), *queries])
    original = [json.loads(line) for line in run(
        [java, "-Xmx1500m", "-cp", cp, "oracle.WorldgenOreBootstrap", str(child), str(work)],
        input=transcript).splitlines() if line.startswith("{")]
    native = [json.loads(line) for line in run([str(probe), str(work)], input=transcript).splitlines()]
    require(len(original) == len(native) == len(queries) + 1, "incomplete paired ore transcript")
    configs, native_configs = original.pop(0), native.pop(0)
    require(configs["oracle_inner_sha256"] == INNER, "wrong actual configured-feature source")
    require(configs["configured_ores"] == native_configs["configured_ores"], "typed configured-ore catalog mismatch")
    results, observations = [], []
    compared = ["query", "placed", "error", "events", "random_count", "random_state128",
                "config_size", "config_discard_bits", "feature_type", "changed", "heightmaps",
                "section_acquisitions", "section_releases"]
    for query, old, new in zip(queries, original, native):
        identity = query["id"]
        require(old["oracle_inner_sha256"] == INNER, identity + ": wrong actually loaded source")
        for key in compared:
            if old[key] != new[key]:
                detail = ""
                if isinstance(old[key], list) and isinstance(new[key], list):
                    first = next(((i, a, b) for i, (a, b) in enumerate(zip(old[key], new[key])) if a != b), None)
                    detail = f" lengths {len(old[key])}/{len(new[key])}, first={first}"
                raise RuntimeError(identity + ": " + key + " mismatch" + detail)
        original_image, native_image = Path(old["image_file"]).read_bytes(), Path(new["image_file"]).read_bytes()
        expected_size = (2 * query["available_radius"] + 1) ** 2 * query["height"] * 256 * 4
        require(len(original_image) == len(native_image) == expected_size, identity + ": incomplete controlled region image")
        require(original_image == native_image, identity + ": full block-array mismatch")
        require(old["image_sha256"] == sha(new["image_file"]), identity + ": output digest mismatch")
        require(old["statuses_remain_empty"] and old["section_locks_restored"] and new["section_locks_restored"],
                identity + ": actual status/lock invariant failed")
        require(new["bulk_sessions_balanced"], identity + ": native bulk lifecycle leaked")
        if old["feature_type"] == "edh":
            require(new["bulk_begins"] == 0, identity + ": scattered leaf entered bulk path")
        event_counts = dict(sorted(Counter(event[0] for event in old["events"]).items()))
        snapshot = {key: old[key] for key in ["query", "placed", "error", "random_count", "random_state128",
                    "config_size", "config_discard_bits", "feature_type", "changed", "image_sha256",
                    "section_acquisitions", "section_releases", "statuses_remain_empty", "section_locks_restored"]}
        snapshot |= dict(events_count=len(old["events"]), event_counts=event_counts,
                         events_sha256=digest(old["events"]), heightmaps_sha256=digest(old["heightmaps"]))
        observations.append(snapshot)
        results.append(dict(id=identity, feature=query["feature"], changed_blocks=len(old["changed"]),
                            event_counts=event_counts, random_count=old["random_count"],
                            outcome=old["error"] or old["placed"], block_arrays_equal=True,
                            host_event_order_equal=True, caller_rng_equal=True, heightmaps_equal=True,
                            source_sections_released=True, native_bulk_begins=new["bulk_begins"],
                            native_bulk_closes=new["bulk_closes"]))
    by_id = {row["query"]["id"]: row for row in original}
    require(by_id["iron_layered"]["changed"] and by_id["iron_stone"]["changed"], "both original ordered iron targets must actually replace material")
    require(any(e[0] == "section_set" for e in by_id["copper_cross"]["events"]), "regular direct write path was not exercised")
    require(any(e[0] == "region_set" and e[-1] for e in by_id["ancient_large"]["events"]), "scattered accepted region write was not exercised")
    veto = by_id["ancient_radius_veto"]
    require(veto["placed"] is True and any(e[0] == "region_set" and not e[-1] for e in veto["events"]), "actual radius veto/ignored boolean path absent")
    zero = by_id["scattered_zero_attempts"]
    require(zero["placed"] is True and zero["random_count"] == 1 and len(zero["events"]) == 1 and not zero["changed"], "actual zero-attempt witness changed")
    for identity in ["origin_rejected", "scattered_origin_rejected"]:
        row = by_id[identity]
        require(row["placed"] is False and row["random_count"] == 0 and len(row["events"]) == 1, "origin rejection consumed randomness or queried material")
    require(any(e[0] == "outside_y" and e[-1] for e in by_id["below_height_guard"]["events"]), "height guard was not exercised")
    above = by_id["above_height_preflight"]
    require(above["placed"] is False and above["random_count"] == 3 and above["section_acquisitions"] == 0, "height preflight witness changed")
    missing = by_id["missing_after_acquire"]
    require(missing["error"] == "missing_neighbor" and missing["changed"] and missing["section_acquisitions"] > 0
            and missing["section_acquisitions"] == missing["section_releases"], "missing neighbor did not fail after real mutations/acquisition and cleanup")
    require(by_id["diamond_partial_air"]["config_discard_bits"] not in [0, 1065353216], "partial air-discard branch absent")
    require(by_id["diamond_buried_air"]["config_discard_bits"] == 1065353216, "full air-discard branch absent")
    require(any(e[0] == "bulk_null_y" for e in by_id["diamond_bottom_air"]["events"]), "actual bulk null-section AIR path absent")
    guarded = by_id["regular_neighbor_write_guard"]
    require(len(guarded["changed"]) < len(by_id["copper_cross"]["changed"]) and any(e[0] == "ensure" and not e[-1] for e in guarded["events"]), "regular candidate write guard was not exercised")
    require(any(e[0] == "bulk_get" for e in by_id["diamond_buried_air"]["events"]), "regular air reads were not bulk reads")
    require(any(e[0] == "world_get" for e in by_id["ancient_large"]["events"]), "scattered air reads were not region reads")
    production_hashes = {path: sha(ROOT / path) for path in PRODUCTION + DEPENDENCIES}
    probe_hashes = {path: sha(ROOT / path) for path in PROBES}
    common = dict(minecraft="1.21", protocol=767, oracle_inner_sha256=INNER,
                  native_source_commit=SOURCE_COMMIT, native_composed_commit=composed_commit, production_sha256=production_hashes,
                  probe_sha256=probe_hashes, generated_bootstrap_sha256=hashlib.sha256(BOOTSTRAP.encode()).hexdigest())
    runtime = common | dict(evidence="unchanged original runtime observations", configured_ores=configs["configured_ores"], cases=observations)
    report = common | dict(success=True,
        scope="Configured regular/scattered ore leaves only; controlled actual ProtoChunk sections and rejecting region adapter, not WorldGenRegion or FEATURES delivery",
        caller_rng="One explicit actual caller WorldgenRandom<XoroshiroRandomSource(seed)> per query, no decorator factory/reseed. Count and source128 state compared after all successful/error paths.",
        layout="Chunk X/Z ascending, block Y then local Z then X; canonical u32 little-endian. WG arrays use local Z/X.",
        trace="Exact ensureWrite/height/outsideY/bulk-nullY/chunk-acquire/bulk-read/direct-section-write/world-read/region-write event order. Actual section release balance compared; Java BulkSectionAccess internal close iteration order is not represented.",
        controlled_host="Fresh real original EMPTY ProtoChunks initialized with explicit solid/air material profiles; actual bound builtin tags. Native arrays reproduce only these fixture owners and fail if region writes change WG predicates. Java region-write calls delegate to actual ProtoChunk; no ServerLevel POI or generalized block-entity/postprocessing callbacks are supplied.",
        class_loading="Signed unmodified original JAR in parent loader; unsigned observing section subclasses in child loader. Actual OreFeature CodeSource SHA is checked.",
        configured_catalog_equal=True, cases=results,
        not_verified=["Placed-feature modifiers, decorator seeds/history, feature ordering or complete FEATURES stage.",
                      "WorldGenRegion/ChunkStep dependencies, Main integration, persistence, scheduler completion or FULL chunks.",
                      "Generic region write callbacks, POI, block entities, postprocessing/tick side effects or height predicate transitions.",
                      "Datapack overrides, random RuleTests, arbitrary material profiles or size-zero configured overrides."])
    return runtime, report


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--oracle-dir", type=Path, required=True)
    parser.add_argument("--java-home", type=Path)
    parser.add_argument("--work-dir", type=Path)
    parser.add_argument("--skip-build", action="store_true", help="Only after a separate explicit cargo build --lib for this source")
    parser.add_argument("--runtime-report", type=Path)
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    if args.work_dir:
        work = args.work_dir.resolve()
        require(work != ROOT and ROOT not in work.parents, "work directory must be outside checkout")
        work.mkdir(parents=True, exist_ok=True)
        runtime, report = compare(args, work)
    else:
        with tempfile.TemporaryDirectory(prefix="cinderstone-ore-") as temporary:
            runtime, report = compare(args, Path(temporary))
    for target, value in [(args.runtime_report, runtime), (args.report, report)]:
        if target:
            target.write_text(json.dumps(value, indent=2) + "\n")
    print(json.dumps(dict(success=True, scope="controlled configured ore leaves", cases=[
        {key: row[key] for key in ["id", "changed_blocks", "outcome", "caller_rng_equal"]} for row in report["cases"]])))


if __name__ == "__main__":
    main()
