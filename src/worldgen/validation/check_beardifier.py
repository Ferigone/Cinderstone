#!/usr/bin/env python3
"""Ordered original Beardifier kernel and retained-target phase comparisons.

This supplies declared rigid/junction data, not a StructureManager or generated
structures. Java runs first; native receives only queries and actual INITIAL.
"""
import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import re
import runpy
import shutil
import tempfile

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
BASE = runpy.run_path(str(HERE / "check_existing_target.py"),
                     run_name="existing_target_validation_helpers")
require, sha, encoded = BASE["require"], BASE["sha"], BASE["encoded"]
equal, write_json = BASE["equal"], BASE["write_json"]
TIMEOUT = 55
BOOTSTRAP = BASE["BOOTSTRAP"].replace("WorldgenExistingTargetBootstrap", "WorldgenBeardifierBootstrap").replace(
    'Class.forName("WorldgenExistingTargetOracle"', 'Class.forName("WorldgenBeardifierOracle"')


def pins():
    production, probes = BASE["source_pins"]()
    for name in ["check_beardifier.py", "WorldgenBeardifierOracle.java"]:
        probes[str((HERE / name).relative_to(ROOT))] = sha(HERE / name)
    return production, probes


def rigid(adjustment, box=None, delta=3):
    return dict(box=box or [2, 60, 2, 13, 72, 13],
                terrain_adjustment=adjustment, ground_level_delta=delta)


def junction(x, y, z):
    return dict(source_x=x, source_ground_y=y, source_z=z)


def cases(work):
    point = [8, 63, 8]
    points = [point, [8, 62, 8], [8, 64, 8], [2, 63, 8], [1, 63, 8],
              [13, 63, 8], [14, 63, 8], [-9, 63, 8], [-10, 63, 8],
              [-11, 63, 8], [24, 63, 8], [25, 63, 8], [26, 63, 8],
              [8, 48, 8], [8, 49, 8], [8, 74, 8], [8, 75, 8],
              [8, 60, 8], [8, 72, 8], [8, 73, 8], [1000, 0, 1000], point]
    adjustments = ["none", "bury", "beard_thin", "beard_box", "encapsulate"]
    kernel = [dict(id="kernel_" + name, mode="beardifier_kernel",
                   beardifier=dict(rigids=[rigid(name)], junctions=[]),
                   beardifier_points=points) for name in adjustments]
    kernel.append(dict(id="kernel_junction", mode="beardifier_kernel",
        beardifier=dict(rigids=[], junctions=[junction(8, 63, 8)]), beardifier_points=points))
    kernel.append(dict(id="kernel_ordered_mixed", mode="beardifier_kernel",
        beardifier=dict(rigids=[rigid(name) for name in adjustments],
                        junctions=[junction(8, 63, 8), junction(-7, 58, 3)]),
        beardifier_points=points))
    controls, declarations, states = BASE["cases"](work)
    control = next(q for q in controls if q["id"] == "aligned_existing")
    declared = next(q for q in declarations if q["query"]["id"] == "aligned_existing")
    rigids = [rigid("bury", [1, 64, 1, 14, 74, 14], 2)]
    junctions = [junction(4, 70, 4), junction(4, 70, 4), junction(12, 68, 12)]
    full, inputs = [], []
    for identity, body in [
        ("empty_control", None),
        ("rigid_only", dict(rigids=rigids, junctions=[])),
        ("junction_only", dict(rigids=[], junctions=junctions)),
        ("combined", dict(rigids=rigids, junctions=junctions)),
    ]:
        query = copy.deepcopy(control)
        query["id"] = identity
        query["beardifier_points"] = [[4, 62, 4], [4, 69, 4], [12, 67, 12],
            [8, 64, 8], [0, 70, 0], [20, 70, 20], [4, 82, 4], [4, 62, 4]]
        if body is not None:
            query["beardifier"] = copy.deepcopy(body)
        full.append(query)
        row = copy.deepcopy(declared)
        row["query"] = query
        inputs.append(row)
    return kernel, full, inputs, states


def scalar_compare(query, original, native):
    identity = query["id"]
    require(equal(original["query"], query) and equal(native["query"], query), identity + ": query changed")
    require(original["oracle_inner_sha256"] == BASE["INNER"], identity + ": wrong original artifact")
    require(original["structure_owner_class"] == "dyl"
            and original["structure_owner_jar_sha256"] == BASE["INNER"],
            identity + ": not the actual original Beardifier")
    expected_kind = "actual_ordered_beardifier" if "beardifier" in query else "actual_empty_beardifier"
    require(original["beardifier_input_kind"] == expected_kind, identity + ": wrong declared owner kind")
    for key in ["beardifier_samples", "beardifier_min_bits", "beardifier_max_bits"]:
        require(equal(original[key], native[key]), identity + ": raw scalar/bound bits differ: " + key)
    samples = original["beardifier_samples"]
    require([row[:3] for row in samples] == query["beardifier_points"], identity + ": point order differs")
    require(samples[0] == samples[-1], identity + ": original iterator rewind did not preserve repeat")
    return dict(id=identity, sample_bits_equal=True, bounds_bits_equal=True,
                repeated_point_equal=True, samples=samples,
                min_bits=original["beardifier_min_bits"], max_bits=original["beardifier_max_bits"])


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
    kernel, full, declared, states = cases(work)
    queries = kernel + full
    write_json(work / "declared-inputs.json", dict(kernel=kernel, pipelines=declared))
    parent, child = work / "parent-classes", work / "child-classes"
    parent.mkdir()
    child.mkdir()
    bootstrap = work / "WorldgenBeardifierBootstrap.java"
    bootstrap.write_text(BOOTSTRAP)
    cp = os.pathsep.join(map(str, [parent, inner, *libraries]))
    _, parent_compile = BASE["bounded"]([javac, "-proc:none", "-cp", cp, "-d", str(parent),
        str(bootstrap), str(HERE / "WorldgenSurfaceOracle.java"),
        str(HERE / "WorldgenBiomeStageOracle.java")], work, "compile-parent", wall=TIMEOUT)
    _, child_compile = BASE["bounded"]([javac, "-proc:none", "-cp", cp, "-d", str(child),
        str(HERE / "WorldgenExistingTargetOracle.java"), str(HERE / "WorldgenBeardifierOracle.java")],
        work, "compile-child", wall=TIMEOUT)
    original_text, original_execution = BASE["bounded"]([
        java, "-Xmx1024m", "-XX:ActiveProcessorCount=2", "-cp", cp,
        "oracle.WorldgenBeardifierBootstrap", str(child), str(work)], work,
        "original-beardifier", stdin="".join(encoded(q) + "\n" for q in queries), wall=TIMEOUT)
    original = [json.loads(line) for line in original_text.splitlines() if line.startswith("{")]
    require(len(original) == len(queries), "incomplete original Beardifier transcript")
    native_input = [dict(query=q) if q.get("mode") == "beardifier_kernel"
                    else dict(query=q, initial=row["initial"]) for q, row in zip(queries, original)]
    require(all(set(row) == ({"query"} if q.get("mode") == "beardifier_kernel"
                             else {"query", "initial"}) for q, row in zip(queries, native_input)),
            "future phase expectations leaked to native")
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
        *extern, "-o", str(executable)], work, "compile-native", wall=TIMEOUT)
    native_text, native_execution = BASE["bounded"]([str(executable), str(work)], work,
        "native-beardifier", stdin="".join(encoded(row) + "\n" for row in native_input), wall=TIMEOUT)
    native = [json.loads(line) for line in native_text.splitlines()]
    require(len(native) == len(queries), "incomplete native Beardifier transcript")
    kernel_rows, pipelines = [], []
    by_id = {}
    for query, old, new in zip(queries, original, native):
        scalars = scalar_compare(query, old, new)
        identity = query["id"]
        if query.get("mode") == "beardifier_kernel":
            if identity == "kernel_none":
                require(all(row[3] == "0" for row in scalars["samples"]), "NONE had an invented contribution")
            else:
                require(any(int(row[3]) & ((1 << 63) - 1) for row in scalars["samples"]),
                        identity + ": no actual nonzero contribution")
            require(scalars["samples"][-2][3] == "0", identity + ": far kernel boundary not zero")
            kernel_rows.append(scalars)
            continue
        result = BASE["compare_case"](query, old, new, work, states)
        for phase in ["biomes", "noise", "surface"]:
            require(old["same_beardifier_after_" + phase] is True,
                    identity + ": original replaced its retained Beardifier at " + phase)
            require(new["native_beardifier_identity_after_" + phase] is True,
                    identity + ": native replaced its captured Beardifier at " + phase)
        require(result["actual_noise_changed_blocks"] > 0
                and result["actual_surface_changed_blocks"] > 0,
                identity + ": complete phases did not execute positive mutations")
        result.update(scalars=scalars, retained_beardifier_identity_each_phase=True)
        pipelines.append(result)
        by_id[identity] = old
    baseline = by_id["empty_control"]
    for result in pipelines:
        if result["id"] == "empty_control":
            continue
        row = by_id[result["id"]]
        for phase in ["noise", "surface"]:
            a = BASE["artifact"](baseline[phase]["blocks_file"], work).read_bytes()
            b = BASE["artifact"](row[phase]["blocks_file"], work).read_bytes()
            count = BASE["changes"](a, b, row[phase]["physical_min_y"])
            require(count > 0, result["id"] + ": selected source has no " + phase + " effect against empty owner")
            result[phase + "_changes_against_empty_owner"] = count
    require(pins() == (production, probes), "source changed during Beardifier observation")
    common = dict(minecraft="1.21", protocol=767, oracle_inner_sha256=BASE["INNER"],
        production_sha256=production, probe_sha256=probes, artifact_sha256=artifacts,
        bootstrap_sha256=hashlib.sha256(BOOTSTRAP.encode()).hexdigest(), java_versions=versions,
        native_rlib_sha256=library_hashes, native_probe_sha256=sha(executable),
        child_timeout_seconds=TIMEOUT, term_grace_seconds=5,
        input_phase_separation="Java first; native kernel gets query only, native pipelines get query and actual original INITIAL only")
    runtime = common | dict(declared_inputs=dict(kernel=kernel, pipelines=declared),
        original=BASE["clean"](original, work), native=BASE["clean"](native, work),
        executions=dict(original=original_execution, native=native_execution,
            compile_parent=parent_compile, compile_child=child_compile, compile_native=native_compile),
        raw_transcript_sha256={name: sha(work / name) for name in [
            "original-beardifier.stdin.jsonl", "original-beardifier.stdout.log",
            "original-beardifier.stderr.log", "native-beardifier.stdin.jsonl",
            "native-beardifier.stdout.log", "native-beardifier.stderr.log"]})
    report = common | dict(success=True, kernel=kernel_rows, cases=pipelines,
        scope="Actual original ordered rigid/junction kernel and four retained existing-target BIOMES/NOISE/SURFACE flows; not generated structures.",
        selection_scope="Input is already ordered accepted data. This runner does not invoke StructureManager/forStructuresInChunk, generate starts, place templates, or prove source-manager selection.",
        junction_scope="Original JigsawJunction objects use explicit fixture deltaY=0 and destination projection RIGID; Beardifier reads only their three supplied source coordinates.",
        storage_scope="Shared full physical arrays/palettes, WG first-available values, counters, status/accessor/unsaved/debug fields and ordered posts/allocation are compared. No unused packed WG bits or global inter-section post order.",
        owner_scope="Original actual dyl and native captured Arc identities are checked after each phase. Cached SURFACE does not replace the retained owner or perform a new Beardifier selection.",
        not_verified=["Complete STRUCTURE_STARTS/REFERENCES or structure placement/processor/jigsaw generation.",
                      "WorldGenRegion/status scheduling, arbitrary custom registries, Main, persistence or FULL publication.",
                      "Positive SURFACE-only postprocessing insertion unless observed in actual selected output."])
    return runtime, report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oracle-dir", type=Path, required=True)
    parser.add_argument("--java-home", type=Path)
    parser.add_argument("--work-dir", type=Path)
    parser.add_argument("--target-dir", type=Path)
    parser.add_argument("--rustc", default="rustc")
    parser.add_argument("--skip-build", action="store_true",
                        help="Required after coordinator builds the current library; this runner never invokes Cargo")
    parser.add_argument("--runtime-report", type=Path)
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    require(os.name == "posix", "bounded process groups require POSIX")
    require(args.skip_build, "coordinator must build current library first, then supply --skip-build")
    if args.work_dir:
        work = BASE["external"](args.work_dir)
        work.mkdir(parents=True, exist_ok=True)
        require(not any(work.iterdir()), "external work must be empty; no implicit retry")
    else:
        work = Path(tempfile.mkdtemp(prefix="cinderstone-beardifier-"))
    runtime, report = compare(args, work)
    for path, value in [(args.runtime_report, runtime), (args.report, report)]:
        if path:
            path.parent.mkdir(parents=True, exist_ok=True)
            write_json(path, value)
    print(encoded(dict(success=True, external_work_directory=str(work), cases=report["cases"])))


if __name__ == "__main__":
    main()
