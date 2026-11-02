#!/usr/bin/env python3
"""Pair two zero-overlap flows plus one aligned positive-NOISE control.

Reuse the existing-target oracle, native probe and comparison helpers. Java
runs first; native receives only each query and actual original INITIAL.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import runpy
import shutil
import struct
import tempfile

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
BASE = runpy.run_path(str(HERE / "check_existing_target.py"),
                     run_name="existing_target_validation_helpers")
require, sha, encoded = BASE["require"], BASE["sha"], BASE["encoded"]
equal, write_json = BASE["equal"], BASE["write_json"]
# The shared helper adds five seconds of TERM grace before KILL/reap.
CHILD_TIMEOUT = 55


def pins():
    production, probes = BASE["source_pins"]()
    probes[str(Path(__file__).resolve().relative_to(ROOT))] = sha(__file__)
    return production, probes


def cases(work):
    catalog = json.loads((ROOT / "fixtures/placement-1.21.json").read_text())
    require(catalog["minecraft"] == "1.21" and catalog["jar_sha256"] == BASE["INNER"],
            "wrong canonical input catalog")
    defaults = {row["name"]: row["default"] for row in catalog["blocks"]}
    queries, declarations = [], []
    for identity, setting, minimum, material in [
        ("zero_nether", "nether", -64, "netherrack"),
        ("zero_overworld", "overworld", 320, "stone"),
    ]:
        height = 64
        blocks, biomes = "input-" + identity + ".bin", "input-" + identity + "-biomes.json"
        fill, gold = defaults["minecraft:" + material], defaults["minecraft:gold_block"]
        data = b"".join(struct.pack("<I", gold if y == minimum + height - 2
                                   and (x * 3 + z) % 11 == 0 else fill)
                        for y in range(minimum, minimum + height)
                        for x in range(16) for z in range(16))
        (work / blocks).write_bytes(data)
        (work / biomes).write_text(encoded(BASE["palette"](minimum, height, 0, 0)))
        neighbors = []
        for dx in range(-1, 2):
            for dz in range(-1, 2):
                if dx == dz == 0:
                    continue
                name = f"input-{identity}-neighbor-{dx}-{dz}-biomes.json"
                (work / name).write_text(encoded(BASE["palette"](minimum, height, dx, dz)))
                neighbors.append(dict(x=dx, z=dz, level_min_y=minimum,
                                      level_height=height, biomes_file=name))
        query = dict(id=identity, settings="minecraft:" + setting,
                     biome_source="minecraft:" + setting, seed="123456789",
                     x=0, z=0, level_min_y=minimum, level_height=height,
                     blocks_file=blocks, biomes_file=biomes,
                     actual_empty_beardifier=True,
                     posts=[None, [], [0, 0, 4095], None], surface_neighbors=neighbors)
        queries.append(query)
        declarations.append(dict(query=query, blocks_sha256=sha(work / blocks),
            biomes_sha256=sha(work / biomes), default_material=material,
            default_state=fill, marker_state=gold,
            input_kind="Declared controlled 64-layer physical palettes; not generated terrain or saved worlds"))
    controls, control_declarations, _ = BASE["cases"](work)
    control = next(row for row in controls if row["id"] == "aligned_existing")
    control["actual_empty_beardifier"] = True
    queries.append(control)
    declarations.append(next(row for row in control_declarations
                             if row["query"]["id"] == "aligned_existing"))
    return queries, declarations, {
        name: defaults["minecraft:" + name]
        for name in ["air", "stone", "netherrack", "gold_block", "bedrock", "water", "deepslate"]
    }


def validate_case(query, original, native, work, states):
    result = BASE["compare_case"](query, original, native, work, states)
    identity = query["id"]
    require(original["beardifier_input_kind"] == "actual_empty_beardifier",
            identity + ": source did not use the actual empty Beardifier owner")
    require(original["structure_owner_class"] == "dyl",
            identity + ": source structure owner is not the original Beardifier")
    require(original["biomes_ran"] is True and original["surface_ran"] is True,
            identity + ": actual source phases did not execute")
    if identity == "aligned_existing":
        require(original["fill_cell_count_y"] > 0
                and result["actual_noise_changed_blocks"] > 0,
                identity + ": aligned control did not enter a positive material loop")
        require(result["actual_surface_changed_blocks"] > 0,
                identity + ": aligned control did not execute positive SURFACE")
        result.update(profile="aligned_positive_noise_control",
                      actual_biome_query_count=len(original["biome_queries"]),
                      original_beardifier_input_kind=original["beardifier_input_kind"])
        return result
    require(original["fill_cell_count_y"] == native["fill_cell_count_y"] == 0,
            identity + ": expected actual zero-cell material fill")
    require(len(original["biome_queries"]) == len(native["biome_queries"]) == 256,
            identity + ": incomplete actual BIOMES traversal")
    expected_min = 0 if identity == "zero_nether" else 320
    require(original["noise_grid_min_y"] == expected_min and original["noise_grid_height"] == 0,
            identity + ": original retained NoiseChunk grid is not zero-height")
    require(native["native_cache_present_before_noise"] is True
            and native["native_cached_noise_chunk_present"] is True
            and native["noise_chunk_absent"] is False,
            identity + ": skipped fill discarded the retained BIOMES cache")
    for phase in ["noise_before_retrogen", "noise"]:
        before, after = original["biomes"], original[phase]
        for key in ["blocks_file", "biomes_file"]:
            a = BASE["artifact"](before[key], work).read_bytes()
            b = BASE["artifact"](after[key], work).read_bytes()
            require(a == b, identity + ": zero-cell NOISE changed " + key)
        for key in BASE["FIELDS"]:
            require(equal(before[key], after[key]), identity + ": zero-cell NOISE changed " + key)
    require(result["actual_noise_changed_blocks"] == result["actual_noise_added_posts"] == 0,
            identity + ": zero-cell NOISE manufactured material/postprocessing work")
    require(result["actual_surface_changed_blocks"] > 0,
            identity + ": original SURFACE made no positive block changes")
    require(native["native_phase"] == "Surface", identity + ": native final phase is not SURFACE")
    result.update(actual_biome_query_count=256, original_noise_grid_min_y=expected_min,
        original_noise_grid_height=0, native_private_noise_grid_observed=False,
        profile="zero_overlap_retained_cache",
        noise_target_data_unchanged=True, retained_biomes_cache_after_skipped_fill=True,
        original_beardifier_input_kind=original["beardifier_input_kind"])
    return result


def compare(args, work):
    production, probes = pins()
    inner, libraries, artifacts = BASE["pinned_artifacts"](args.oracle_dir.resolve())
    java = str(args.java_home / "bin/java") if args.java_home else shutil.which("java")
    javac = str(args.java_home / "bin/javac") if args.java_home else shutil.which("javac")
    require(java and javac, "Java 21 java/javac required")
    versions = {}
    for name, tool in [("java", java), ("javac", javac)]:
        text, _ = BASE["bounded"]([tool, "-version"], work, name + "-version", wall=15)
        versions[name] = text + (work / (name + "-version.stderr.log")).read_text()
        require(re.search(r'(?:version\s+"|javac\s+)21(?:[."\s]|$)', versions[name]),
                name + ": Java 21 required")
    queries, declared, states = cases(work)
    write_json(work / "declared-inputs.json", declared)
    parent, child = work / "parent-classes", work / "child-classes"
    parent.mkdir()
    child.mkdir()
    bootstrap = work / "WorldgenExistingTargetBootstrap.java"
    bootstrap.write_text(BASE["BOOTSTRAP"])
    cp = os.pathsep.join(map(str, [parent, inner, *libraries]))
    _, parent_compile = BASE["bounded"]([javac, "-proc:none", "-cp", cp, "-d", str(parent),
        str(bootstrap), str(HERE / "WorldgenSurfaceOracle.java"),
        str(HERE / "WorldgenBiomeStageOracle.java")], work, "compile-parent", wall=CHILD_TIMEOUT)
    _, child_compile = BASE["bounded"]([javac, "-proc:none", "-cp", cp, "-d", str(child),
        str(HERE / "WorldgenExistingTargetOracle.java")], work, "compile-child", wall=CHILD_TIMEOUT)
    original_stdout, original_execution = BASE["bounded"]([
        java, "-Xmx768m", "-XX:ActiveProcessorCount=2", "-cp", cp,
        "oracle.WorldgenExistingTargetBootstrap", str(child), str(work)], work,
        "original-zero-overlap", stdin="".join(encoded(q) + "\n" for q in queries), wall=CHILD_TIMEOUT)
    original = [json.loads(line) for line in original_stdout.splitlines() if line.startswith("{")]
    require(len(original) == len(queries), "incomplete original zero-overlap transcript")
    native_input = [dict(query=q, initial=row["initial"]) for q, row in zip(queries, original)]
    require(all(set(row) == {"query", "initial"} for row in native_input),
            "future expected phase supplied to native")
    target = args.target_dir or Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    target = target if target.is_absolute() else ROOT / target
    deps, library = target / "debug/deps", target / "debug/libmc121_rust.rlib"
    require(library.is_file(), "coordinator must explicitly build the current native library first")
    extern, library_hashes = ["--extern", "mc121_rust=" + str(library)], {"mc121_rust": sha(library)}
    for name in ["anyhow", "serde_json", "openssl"]:
        choices = sorted(deps.glob("lib" + name + "-*.rlib"), key=lambda p: p.stat().st_mtime, reverse=True)
        require(choices, "missing native dependency " + name)
        extern.extend(["--extern", name + "=" + str(choices[0])])
        library_hashes[name] = sha(choices[0])
    executable = work / "existing-target-probe"
    _, native_compile = BASE["bounded"]([args.rustc, "--edition=2021",
        str(HERE / "existing_target_probe.rs"), "-L", "dependency=" + str(deps),
        *extern, "-o", str(executable)], work, "compile-native", wall=CHILD_TIMEOUT)
    native_stdout, native_execution = BASE["bounded"]([str(executable), str(work)], work,
        "native-zero-overlap", stdin="".join(encoded(row) + "\n" for row in native_input), wall=CHILD_TIMEOUT)
    native = [json.loads(line) for line in native_stdout.splitlines()]
    require(len(native) == len(queries), "incomplete native zero-overlap transcript")
    results = [validate_case(q, old, new, work, states)
               for q, old, new in zip(queries, original, native)]
    require(pins() == (production, probes), "source changed during zero-overlap observation")
    common = dict(minecraft="1.21", protocol=767, oracle_inner_sha256=BASE["INNER"],
        production_sha256=production, probe_sha256=probes, artifact_sha256=artifacts,
        bootstrap_sha256=hashlib.sha256(BASE["BOOTSTRAP"].encode()).hexdigest(),
        native_rlib_sha256=library_hashes, native_probe_sha256=sha(executable), java_versions=versions,
        input_phase_separation="Java first; native receives only query plus actual original INITIAL",
        child_timeout_seconds=CHILD_TIMEOUT, term_grace_seconds=5)
    runtime = common | dict(declared_inputs=declared, original=BASE["clean"](original, work),
        native=BASE["clean"](native, work), executions=dict(original=original_execution,
            native=native_execution, compile_parent=parent_compile,
            compile_child=child_compile, compile_native=native_compile),
        raw_transcript_sha256={name: sha(work / name) for name in [
            "original-zero-overlap.stdin.jsonl", "original-zero-overlap.stdout.log",
            "original-zero-overlap.stderr.log", "native-zero-overlap.stdin.jsonl",
            "native-zero-overlap.stdout.log", "native-zero-overlap.stderr.log"]})
    report = common | dict(success=True, cases=results,
        scope="Two controlled zero-overlap retained-cache flows and one aligned positive-NOISE control, each original BIOMES/task continuation/SURFACE paired with native live target.",
        reuse_scope="Existing-target oracle/probe and shared artifact/process/snapshot/comparison/sanitization helpers; no duplicate generator.",
        beardifier_scope="All three queries require the actual original empty Beardifier owner, class dyl. Historical RAW BeardifierMarker observations are not this run's acceptance evidence.",
        grid_scope="Zero-height retained grid fields are observed in original only. Native private grid/AQ/cache pointers and runtime acquisition order are not observed; cache-presence/storage/lease guards are separate.",
        storage_scope="Complete physical states/palettes, WG first-available values, counters, status/generation/unsaved/debug fields and ordered posts/allocation are compared. Unused WG bits and global inter-section post order are not measured.",
        context_scope="Original targets retain persisted EMPTY. SURFACE uses declared registered stored-center/foreign palettes through the explicit base-ChunkAccess view; no WorldGenRegion, scheduler or historical save.",
        not_verified=["Negative-height overlap or arbitrary dimension/registry inputs.",
                      "Nonempty structures, full status scheduling, Main, persistence or FULL publication.",
                      "Native private aquifer/cache/grid identities or runtime acquisition order.",
                      "Positive SURFACE-only fluid insertion unless observed in actual output."])
    return runtime, report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oracle-dir", type=Path, required=True)
    parser.add_argument("--java-home", type=Path)
    parser.add_argument("--work-dir", type=Path)
    parser.add_argument("--target-dir", type=Path)
    parser.add_argument("--rustc", default="rustc")
    parser.add_argument("--skip-build", action="store_true",
                        help="Required: coordinator separately built current library; this runner never invokes Cargo")
    parser.add_argument("--runtime-report", type=Path)
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    require(os.name == "posix", "bounded process groups require POSIX")
    require(args.skip_build, "coordinator must build the library first, then supply --skip-build")
    if args.work_dir:
        work = BASE["external"](args.work_dir)
        work.mkdir(parents=True, exist_ok=True)
        require(not any(work.iterdir()), "external work must be empty; no implicit retry")
    else:
        work = Path(tempfile.mkdtemp(prefix="cinderstone-zero-overlap-"))
    runtime, report = compare(args, work)
    for path, value in [(args.runtime_report, runtime), (args.report, report)]:
        if path:
            path.parent.mkdir(parents=True, exist_ok=True)
            write_json(path, value)
    print(encoded(dict(success=True, external_work_directory=str(work), cases=report["cases"])))


if __name__ == "__main__":
    main()
