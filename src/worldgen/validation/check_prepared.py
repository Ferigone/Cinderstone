#!/usr/bin/env python3
"""Compare actual cachedClimateSampler -> NOISE with native retained preparation.

This runs unchanged official classes in a controlled empty-influence fixture.
It does not select biomes, execute a server scheduler or publish a FULL chunk.
Generated Java classes and native terrain arrays stay outside the repository.
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
         str(HERE / "WorldgenNoiseOracle.java"), str(HERE / "WorldgenPreparedClimateOracle.java")])
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
    probe = work / "prepared_climate"
    run(["rustc", "--edition=2021", str(HERE / "prepared_climate.rs"),
         "-L", "dependency=" + str(deps), *extern, "-o", str(probe)])
    queries = []
    for name, cx, cz in [("overworld", 0, 0), ("overworld", -2, 3),
                         ("nether", 0, 0), ("end", 0, 0)]:
        points = [[cx * 4 + dx, y, cz * 4 + dz] for dx, y, dz in [
            (-1, -16, -1), (0, 0, 0), (1, 16, 1), (3, 79, 3),
            (4, 80, 4), (5, -17, -5), (0, 0, 0)]]
        points += [[-2147483648, 2147483647, -2147483647],
                   [2147483647, -2147483648, 2147483646]]
        queries.append(dict(settings="minecraft:" + name, seed=123456789, x=cx, z=cz, points=points))
    transcript = "".join(json.dumps(query) + "\n" for query in queries)
    original = [json.loads(line) for line in run([
        java, "-Xmx1024m", "-cp", cp, "oracle.WorldgenPreparedClimateOracle", str(work)], input=transcript).splitlines()]
    native = [json.loads(line) for line in run([str(probe), str(work)], input=transcript).splitlines()]
    if len(original) != len(queries) or len(native) != len(queries):
        raise RuntimeError("incomplete original/native prepared transcript")
    cases = []
    for old, new in zip(original, native):
        if old["oracle_inner_sha256"] != INNER:
            raise RuntimeError("wrong original runtime class origin")
        for key in ["settings", "seed", "x", "z", "min_y", "height", "points", "climate", "climate_after", "postprocessing_count"]:
            if old[key] != new[key]:
                raise RuntimeError(f"prepared {key} mismatch: {old['settings']} chunk {old['x']},{old['z']}")
        if not (all(old[key] for key in ["cache_owners_retained", "climate_preserves_counters", "post_climate_preserves_counters", "aquifer_retained"])
                and new["dropped_outer_generator_binding"] and new["moved_prepared_to_worker"]
                and new["stage"] == "Noise"):
            raise RuntimeError("prepared lifetime/cache/stage witness failed")
        if Path(old["blocks_file"]).read_bytes() != Path(new["blocks_file"]).read_bytes():
            raise RuntimeError(f"prepared terrain array mismatch: {old['settings']} chunk {old['x']},{old['z']}")
        cases.append({key: old[key] for key in ["settings", "seed", "x", "z", "min_y", "height",
                      "points", "climate", "climate_after", "cache_owners_retained", "climate_preserves_counters", "post_climate_preserves_counters", "aquifer_retained",
                      "cache_owners", "initial_counters", "postprocessing_count"]}
                     | dict(java_sha256=sha(old["blocks_file"]), native_sha256=sha(new["blocks_file"]),
                            terrain_arrays_equal=True, dropped_outer_generator_binding=True,
                            moved_prepared_to_worker=True, native_stage=new["stage"]))
    paths = ["src/worldgen/generator.rs", "src/worldgen/noise_chunk.rs",
             "src/worldgen/validation/WorldgenNoiseOracle.java",
             "src/worldgen/validation/WorldgenPreparedClimateOracle.java",
             "src/worldgen/validation/prepared_climate.rs",
             "src/worldgen/validation/check_prepared.py"]
    return dict(minecraft="1.21", protocol=767, oracle_inner_sha256=INNER,
                mappings_sha1=lock["mappings"]["sha1"],
                observation="Unchanged original NoiseChunk constructor/cachedClimateSampler/Climate.Sampler and source-ordered NOISE traversal compared with native prepared owner",
                fixture_context="Controlled fresh empty input with actual empty Blender and BeardifierMarker; no biome resolver or biome generation is supplied",
                cases=cases, source_sha256={path: sha(ROOT / path) for path in paths},
                not_verified=["BIOMES selection, biome storage and ChunkStatus completion.",
                              "Main/runtime scheduling, real dependency facts and persistence.",
                              "Nonempty Blender, structures or below-zero retrogen.",
                              "Spawn-target search and complete terrain/FULL generation."])


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
            raise RuntimeError("work directory must be outside the repository")
        work.mkdir(parents=True, exist_ok=True)
        result = compare(args, work)
    else:
        with tempfile.TemporaryDirectory(prefix="cinderstone-prepared-climate-") as temp:
            result = compare(args, Path(temp))
    if args.report:
        args.report.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(dict(success=True, cases=[dict(settings=x["settings"], x=x["x"], z=x["z"],
          terrain_arrays_equal=x["terrain_arrays_equal"]) for x in result["cases"]])))


if __name__ == "__main__":
    main()
