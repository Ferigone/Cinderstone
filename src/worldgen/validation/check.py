#!/usr/bin/env python3
"""Focused NOISE/SURFACE comparison with the unchanged official 1.21 classes.

Generated Java classes, native probes, terrain arrays and thermal observations
stay in the selected external work directory. This is not a production server
or a natural-biome/full-chunk compatibility test.
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


def lines(command, queries):
    output = run(command, input="".join(json.dumps(q) + "\n" for q in queries))
    values = [json.loads(line) for line in output.splitlines()]
    if len(values) != len(queries):
        raise RuntimeError("incomplete original/native transcript")
    return values


def compare(args, work):
    oracle = args.oracle_dir.resolve()
    lock = json.loads((ROOT / "fixtures/source-lock.json").read_text())
    for section, filename in [("oracle", "server-1.21.jar"),
                              ("mappings", "server-1.21-mappings.txt")]:
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
         str(HERE / "WorldgenNoiseOracle.java"), str(HERE / "WorldgenSurfaceOracle.java")])

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
    for probe in ["noise_probe", "surface_probe"]:
        run(["rustc", "--edition=2021", str(HERE / (probe + ".rs")),
             "-L", "dependency=" + str(deps), *extern, "-o", str(work / probe)])

    settings = ["overworld", "nether", "end", "amplified", "large_biomes", "caves", "floating_islands"]
    queries = [dict(seed=123456789, settings="minecraft:" + s, x=0, z=0) for s in settings]
    queries.insert(1, dict(seed=123456789, settings="minecraft:overworld", x=-2, z=3))
    original = lines([java, "-Xmx1024m", "-cp", cp, "oracle.WorldgenNoiseOracle", str(work)], queries)
    native = lines([str(work / "noise_probe"), str(work)], queries)
    noise = []
    for a, b in zip(original, native):
        old = Path(a["blocks_file"]).read_bytes()
        new = Path(b["blocks_file"]).read_bytes()
        equal = old == new
        if not equal:
            raise RuntimeError(f"NOISE mismatch: {a['settings']} chunk {a['x']},{a['z']}")
        noise.append({k: a[k] for k in ["settings", "seed", "x", "z", "min_y", "height"]}
                     | dict(java_sha256=sha(a["blocks_file"]), native_sha256=sha(b["blocks_file"]), equal=equal))

    cases = [("overworld", b) for b in ["plains", "eroded_badlands", "frozen_ocean", "swamp"]]
    cases += [("nether", "nether_wastes"), ("end", "the_end")]
    queries = [dict(seed=123456789, settings="minecraft:" + s, x=0, z=0, biome="minecraft:" + b) for s, b in cases]
    original = lines([java, "-Xmx1024m", "-cp", cp, "oracle.WorldgenSurfaceOracle", str(work)], queries)
    native_queries = [q | dict(thermal=a["context"]) for q, a in zip(queries, original)]
    native = lines([str(work / "surface_probe"), str(work)], native_queries)
    surface = []
    for a, b in zip(original, native):
        equal = Path(a["output"]).read_bytes() == Path(b["output"]).read_bytes()
        heights = a["heightmaps_world_surface_wg_ocean_floor_wg"]
        native_heights = []
        for x in range(16):
            for z in range(16):
                index = z * 16 + x
                native_heights.extend([b["heights"][index] - 1, b["ocean_floor"][index] - 1])
        if not (equal and heights == native_heights and a["postprocess"] == b["surface_postprocess"]
                and b["failed_context_preserves_noise"] and b["duplicate_stage_rejected"]):
            raise RuntimeError(f"SURFACE mismatch: {a['settings']} {a['biome']}")
        surface.append({k: a[k] for k in ["settings", "seed", "x", "z", "biome", "changes"]}
                       | dict(java_sha256=sha(a["output"]), native_sha256=sha(b["output"]), equal=equal,
                              heightmaps_equal=True, surface_postprocessing_equal=True,
                              failed_context_preserves_noise=True, duplicate_stage_rejected=True))
    sources = [*sorted((ROOT / "src/worldgen").rglob("*.rs")), ROOT / "src/density.rs",
               ROOT / "src/density/chunk.rs", ROOT / "src/noise.rs", ROOT / "src/random.rs", ROOT / "src/lib.rs",
               *[ROOT / "fixtures" / name for name in ["source-lock.json", "density_builtin-1.21.json",
                 "noise_parameters-1.21.json", "world-generation-surface-rules.json", "items-1.21.json", "placement-1.21.json"]],
               HERE / "WorldgenNoiseOracle.java",
               HERE / "WorldgenSurfaceOracle.java", Path(__file__).resolve()]
    return dict(minecraft="1.21", protocol=767, oracle_inner_sha256=INNER,
                evidence_kind="actual unchanged Java classes; constructed NoiseChunk and controlled-biome ProtoChunk; native stage calls",
                success=True, noise=noise, surface=surface,
                source_sha256={str(p.relative_to(ROOT)): sha(p) for p in sources},
                not_verified=["Main generation callsites, runtime publication, scheduling and persistence",
                              "Natural multi-biome selection and full dimension generation",
                              "Nonempty Blender/structure beardifier, carving, features and FULL conversion",
                              "Positive eroded-badlands pillar filling; this case has the source water guard",
                              "Every seed, rule branch, height clamp, postprocessing and concurrent-owner context"])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oracle-dir", type=Path, default=ROOT.parent / "mc121-oracle")
    parser.add_argument("--java-home", type=Path)
    parser.add_argument("--work-dir", type=Path)
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    if args.work_dir:
        work = args.work_dir.resolve()
        if work == ROOT or ROOT in work.parents:
            parser.error("generated outputs must be outside the repository")
        work.mkdir(parents=True, exist_ok=True)
        result = compare(args, work)
    else:
        with tempfile.TemporaryDirectory(prefix="cinderstone-worldgen-") as temporary:
            result = compare(args, Path(temporary))
    encoded = json.dumps(result, indent=2) + "\n"
    if args.report:
        args.report.write_text(encoded)
    print(encoded, end="")


if __name__ == "__main__":
    main()
