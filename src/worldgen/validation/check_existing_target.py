#!/usr/bin/env python3
"""Actual original existing-ProtoChunk phases versus native live-target phases.

Declared palettes are controlled input, not historical saves. Java runs first;
native stdin contains only query plus original INITIAL, never future phases.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import struct
import subprocess
import tempfile
import time
import zipfile

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
INNER = "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"
BOOTSTRAP = """package oracle;
import java.net.*;import java.nio.file.*;
public final class WorldgenExistingTargetBootstrap {
 public static void main(String[] a)throws Exception {
  if(a.length!=2)throw new IllegalArgumentException("child-classes output-directory required");
  try(URLClassLoader c=new URLClassLoader(new URL[]{Path.of(a[0]).toUri().toURL()},WorldgenExistingTargetBootstrap.class.getClassLoader())) {
   Class.forName("WorldgenExistingTargetOracle",true,c).getMethod("main",String[].class).invoke(null,(Object)new String[]{a[1]});
  }
 }
}
"""
FIELDS = [
    "physical_min_y", "physical_height", "world_surface_wg", "ocean_floor_wg",
    "posts", "post_count", "post_allocated", "section_counts", "persisted_status",
    "highest_generated_status", "retrogen_present", "generation_min_y",
    "generation_height", "unsaved", "debug_void_terrain",
]


def require(value, message):
    if not value:
        raise RuntimeError(message)


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def encoded(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"))


def equal(a, b):
    return encoded(a) == encoded(b)  # Preserve JSON bool/integer distinctions.


def write_json(path, value):
    Path(path).write_text(json.dumps(value, indent=2) + "\n")


def external(path):
    path = Path(path).resolve()
    require(path != ROOT and ROOT not in path.parents, "work artifacts must stay outside checkout")
    require(not any((p / "AGENTS.md").is_file() and (p / "Cargo.toml").is_file()
                    for p in [path, *path.parents]), "work must not enter another checkout")
    return path


def bounded(command, work, name, stdin="", wall=180, cwd=None):
    """Preserve raw transcripts; TERM/KILL and reap a dedicated process group."""
    prefix = work / name
    prefix.with_suffix(".stdin.jsonl").write_text(stdin)
    out_path, err_path = prefix.with_suffix(".stdout.log"), prefix.with_suffix(".stderr.log")
    start, timed_out, process = time.monotonic(), False, None
    try:
        with out_path.open("xb") as out, err_path.open("xb") as err:
            process = subprocess.Popen(command, cwd=cwd or work, stdin=subprocess.PIPE,
                                       stdout=out, stderr=err, start_new_session=True)
            try:
                process.communicate(stdin.encode(), timeout=wall)
            except subprocess.TimeoutExpired:
                timed_out = True
                try:
                    os.killpg(process.pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
                try:
                    process.communicate(timeout=5)
                except subprocess.TimeoutExpired:
                    try:
                        os.killpg(process.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                    process.communicate()
    finally:
        if process is not None:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            process.wait()
            execution = dict(exit_code=process.returncode, timed_out=timed_out,
                             elapsed_seconds=round(time.monotonic() - start, 3),
                             process_reaped=True, max_wall_seconds=wall)
            write_json(prefix.with_suffix(".execution.json"), execution)
    require(process is not None, name + ": process did not start")
    require(not timed_out and process.returncode == 0,
            name + ": failed; preserve external stdout/stderr/execution records")
    return out_path.read_text(), execution


def pinned_artifacts(oracle):
    lock = json.loads((ROOT / "fixtures/source-lock.json").read_text())
    require(lock["minecraft"] == "1.21" and lock["protocol"] == 767, "wrong source lock")
    artifacts = {}
    for section, name in [("oracle", "server-1.21.jar"), ("mappings", "server-1.21-mappings.txt")]:
        data = (oracle / name).read_bytes()
        require(len(data) == lock[section]["size"]
                and hashlib.sha1(data).hexdigest() == lock[section]["sha1"], "wrong pinned " + name)
        artifacts[name] = hashlib.sha256(data).hexdigest()
    inner = oracle / "server-inner-1.21.jar"
    require(sha(inner) == INNER, "wrong pinned inner JAR")
    artifacts[inner.name] = INNER
    libraries = []
    with zipfile.ZipFile(oracle / "server-1.21.jar") as archive:
        versions = [line.split("\t") for line in archive.read("META-INF/versions.list").decode().splitlines()]
        selected = [row for row in versions if row[1] == "1.21"]
        require(len(selected) == 1 and selected[0][0] == INNER, "outer does not pin inner")
        require(hashlib.sha256(archive.read("META-INF/versions/" + selected[0][2])).hexdigest() == INNER,
                "outer bundled inner differs")
        for line in archive.read("META-INF/libraries.list").decode().splitlines():
            digest, coordinate, relative = line.split("\t")
            path = (oracle / "libraries" / relative).resolve()
            require((oracle / "libraries").resolve() in path.parents, "library escapes oracle")
            require(sha(path) == digest, "wrong original library " + coordinate)
            libraries.append(path)
            artifacts["libraries/" + relative] = digest
    return inner, libraries, artifacts


def source_pins():
    paths = {p for p in (ROOT / "src/worldgen").rglob("*.rs") if "validation" not in p.parts}
    paths.update(ROOT / p for p in [
        "src/density.rs", "src/density/chunk.rs", "src/random.rs", "src/noise.rs",
        "src/hotspot_math.rs", "src/geometry.rs", "src/lighting.rs", "src/inventory.rs",
        "src/placement.rs", "src/nbt.rs", "src/chunk_readiness.rs", "fixtures/source-lock.json",
        "fixtures/placement-1.21.json", "fixtures/items-1.21.json", "fixtures/item-components-1.21.json",
        "fixtures/density_builtin-1.21.json", "fixtures/noise_parameters-1.21.json",
        "fixtures/world-generation-biome-climate.json", "fixtures/world-generation-surface-rules.json",
        "fixtures/world-generation-multinoise-parameters.json",
        "fixtures/world-generation-target-state-facts.json",
    ])
    probes = [HERE / name for name in [
        "check_existing_target.py", "WorldgenSurfaceOracle.java", "WorldgenBiomeStageOracle.java",
        "WorldgenExistingTargetOracle.java", "existing_target_probe.rs",
    ]]
    return ({str(p.relative_to(ROOT)): sha(p) for p in sorted(paths)},
            {str(p.relative_to(ROOT)): sha(p) for p in probes})


def geometry(minimum, height):
    start = (minimum // 16) * 16
    end = ((minimum + height - 1) // 16 + 1) * 16
    return start, end - start


def palette(minimum, height, cx, cz):
    start, span = geometry(minimum, height)
    return ["minecraft:ocean" if (cx * 4 + x + cz * 4 + z + section * 4 + y) % 3 == 0
            else "minecraft:plains"
            for section in range(start // 16, (start + span) // 16)
            for y in range(4) for z in range(4) for x in range(4)]


def cases(work):
    catalog = json.loads((ROOT / "fixtures/placement-1.21.json").read_text())
    require(catalog["minecraft"] == "1.21" and catalog["jar_sha256"] == INNER, "wrong canonical catalog")
    defaults = {entry["name"]: entry["default"] for entry in catalog["blocks"]}
    state = {name: defaults["minecraft:" + name]
             for name in ["air", "stone", "gold_block", "bedrock", "water", "deepslate"]}
    definitions = [
        ("aligned_existing", "overworld", 0, 0, -64, 384, False, False),
        ("partial_positive", "overworld", -2, 3, 1, 127, False, False),
        ("partial_negative", "overworld", -2, 3, -63, 127, False, False),
        ("retrogen_active", "overworld", 0, 0, -64, 384, True, False),
        ("no_cells_retrogen", "nether", 0, 0, -64, 384, True, True),
    ]
    queries, declarations = [], []
    for identity, setting, cx, cz, minimum, height, retrogen, skip in definitions:
        start, span = geometry(minimum, height)
        end, marker = minimum + height, min(minimum + height - 2, 180)
        raw = bytearray()
        for y in range(start, start + span):
            for x in range(16):
                for z in range(16):
                    material = state["air"]
                    if y < minimum or y >= end:
                        material = state["gold_block"]
                    elif y < minimum + 3:
                        material = state["stone"]
                    elif (y == marker or y >= end - 3) and (x * 3 + z) % 11 == 0:
                        material = state["gold_block"]
                    elif y == 63 and (x + z) % 7 == 0:
                        material = state["water"]
                    if retrogen and 0 <= y <= 4:
                        material = state["bedrock"]
                    raw.extend(struct.pack("<I", material))
        blocks, biomes = "input-" + identity + ".bin", "input-" + identity + "-biomes.json"
        (work / blocks).write_bytes(raw)
        (work / biomes).write_text(encoded(palette(minimum, height, cx, cz)))
        posts = [None if i % 3 == 0 else [] if i % 3 == 1 else [0, 0, 4095]
                 for i in range(span // 16)]
        query = dict(id=identity, settings="minecraft:" + setting, seed="123456789",
                     x=cx, z=cz, level_min_y=minimum, level_height=height,
                     blocks_file=blocks, biomes_file=biomes, posts=posts)
        if retrogen:
            query["retrogen"] = dict(target_status="minecraft:spawn",
                                     missing_bedrock=["5", "0", "0", "0", "8"])
        if skip:
            query["skip_biomes"] = True  # No biome-source owner is supplied.
        else:
            query["biome_source"] = "minecraft:" + setting
            neighbors = []
            for dx in range(-1, 2):
                for dz in range(-1, 2):
                    if dx == dz == 0:
                        continue
                    name = f"input-{identity}-neighbor-{dx}-{dz}-biomes.json"
                    (work / name).write_text(encoded(palette(minimum, height, cx + dx, cz + dz)))
                    neighbors.append(dict(x=cx + dx, z=cz + dz, level_min_y=minimum,
                                          level_height=height, biomes_file=name))
            query["surface_neighbors"] = neighbors
        queries.append(query)
        declarations.append(dict(query=query, physical_min_y=start, physical_height=span,
            blocks_sha256=sha(work / blocks), biomes_sha256=sha(work / biomes),
            canonical_default_states=state,
            input_kind="Declared controlled physical palettes and queues; not saved/generated terrain"))
    return queries, declarations, state


def artifact(name, work):
    path = Path(name)
    path = (path if path.is_absolute() else work / path).resolve()
    require(work in path.parents, "phase artifact escapes external work directory")
    return path


def snapshot(row, work, label):
    require(isinstance(row, dict), label + ": missing snapshot")
    raw = artifact(row["blocks_file"], work).read_bytes()
    require(hashlib.sha256(raw).hexdigest() == row["blocks_sha256"], label + ": stale blocks")
    require(len(raw) == row["physical_height"] * 256 * 4, label + ": incomplete physical image")
    packed = artifact(row["biomes_file"], work).read_bytes()
    require(hashlib.sha256(packed).hexdigest() == row["biomes_sha256"], label + ": stale biomes")
    biomes, count = json.loads(packed), row["physical_height"] // 16
    require(len(biomes) == count * 64 and all(type(x) is str and x for x in biomes),
            label + ": incomplete physical biome image")
    for key in ["world_surface_wg", "ocean_floor_wg"]:
        require(len(row[key]) == 256 and all(type(x) is int for x in row[key]), label + ": incomplete WG values")
    require(len(row["posts"]) == len(row["post_allocated"]) == len(row["section_counts"]) == count,
            label + ": incomplete section metadata")
    require(all(type(x) is bool for x in row["post_allocated"]), label + ": unknown queue allocation")
    require(all(isinstance(xs, list) and all(type(x) is int and -32768 <= x <= 32767 for x in xs)
                for xs in row["posts"]), label + ": invalid signed packed queues")
    require(sum(map(len, row["posts"])) == row["post_count"], label + ": post count differs")
    require(all(flag or not xs for flag, xs in zip(row["post_allocated"], row["posts"])),
            label + ": unallocated queue contains data")
    for key in ["retrogen_present", "unsaved", "debug_void_terrain"]:
        require(type(row[key]) is bool, label + ": unknown " + key)
    return raw, biomes


def preserve_posts(before, after, label):
    for old, new, existed, exists in zip(before["posts"], after["posts"],
                                       before["post_allocated"], after["post_allocated"]):
        require(new[:len(old)] == old, label + ": queue prefix/order/duplicates changed")
        require(not existed or exists, label + ": allocated source queue lost")


def values(raw):
    return [x[0] for x in struct.iter_unpack("<I", raw)]


def changes(a, b, minimum, predicate=lambda y, x, z, old, new: True):
    return sum(old != new and predicate(minimum + index // 256, (index % 256) // 16,
                                       index % 16, old, new)
               for index, (old, new) in enumerate(zip(values(a), values(b))))


def compare_phase(query, old, new, work, phase):
    label = query["id"] + ": " + phase
    a, b = old[phase], new[phase]
    raw, biomes = snapshot(a, work, label + " original")
    other, other_biomes = snapshot(b, work, label + " native")
    require(raw == other, label + ": full physical block images differ")
    require(equal(biomes, other_biomes), label + ": full physical biome palettes differ")
    for key in FIELDS:
        require(equal(a[key], b[key]), label + ": " + key + " differs")
    require(b["all_section_leases_released"] is True, label + ": native lease remains acquired")
    return raw, biomes


def surface_queries(query, source, native, center, work):
    trace = source["surface_biome_queries"]
    require(equal(trace, native["surface_biome_queries"]), query["id"] + ": full ordered quart trace differs")
    require(trace, query["id"] + ": no actual SURFACE quart query")
    palettes = {(query["x"], query["z"]): (query["level_min_y"], query["level_height"], center)}
    for neighbor in query["surface_neighbors"]:
        palettes[neighbor["x"], neighbor["z"]] = (neighbor["level_min_y"], neighbor["level_height"],
                                                  json.loads((work / neighbor["biomes_file"]).read_text()))
    foreign = 0
    for x, y, z, biome in trace:
        chunk = (x >> 2, z >> 2)
        require(chunk in palettes, query["id"] + ": missing supplied neighbor")
        minimum, height, data = palettes[chunk]
        low, high = minimum >> 2, (minimum >> 2) + (height >> 2) - 1
        y = min(max(y, low), high)
        index = ((y // 4) - minimum // 16) * 64 + (y & 3) * 16 + (z & 3) * 4 + (x & 3)
        require(data[index] == biome, query["id"] + ": source callback differs from stored palette")
        foreign += chunk != (query["x"], query["z"])
    return len(trace), foreign


def compare_case(query, old, new, work, state):
    identity = query["id"]
    require(equal(old["query"], query) and equal(new["query"], query), identity + ": query changed")
    require(old["oracle_inner_sha256"] == INNER, identity + ": wrong actually loaded original")
    require("error" not in new and new["native_poisoned"] is False, identity + ": native phase failed")
    require(new["same_target_storage_owners"] is True, identity + ": native storage owners changed")
    require(new["native_private_aquifer_pointer_observed"] is False, identity + ": unsupported native AQ identity claim")
    require(new["native_cache_initially_absent"] is True, identity + ": native imported a future cache")
    for key in ["biomes_ran", "surface_ran", "fill_cell_count_y", "persisted_status_unchanged"]:
        require(equal(old[key], new[key]), identity + ": " + key + " differs")
    require(old["persisted_status_unchanged"] is True, identity + ": original status advanced")
    require(old["original_noise_task_callback_executed"] is True
            and new["native_noise_task_callback_executed"] is True, identity + ": NOISE continuation missing")
    require(old["debug_void_terrain"] is False, identity + ": debug void invalidates material guards")
    phases = ["initial"] + (["biomes"] if old["biomes_ran"] else []) + ["noise_before_retrogen", "noise"]
    if old["surface_ran"]:
        phases.append("surface")
    data = {phase: compare_phase(query, old, new, work, phase) for phase in phases}
    initial, initial_biomes = data["initial"]
    require(initial == (work / query["blocks_file"]).read_bytes(), identity + ": INITIAL differs from declared blocks")
    require(equal(initial_biomes, json.loads((work / query["biomes_file"]).read_text())),
            identity + ": INITIAL differs from declared biomes")
    require(old["initial"]["posts"] == [row or [] for row in query["posts"]]
            and old["initial"]["post_allocated"] == [row is not None for row in query["posts"]],
            identity + ": INITIAL queue allocation/data installation differs")
    for before, after in zip(phases, phases[1:]):
        preserve_posts(old[before], old[after], identity + ": " + after)
    require(equal(old["biome_queries"], new["biome_queries"]), identity + ": ordered natural BIOMES queries differ")
    minimum = old["initial"]["physical_min_y"]
    before = data["biomes" if old["biomes_ran"] else "initial"][0]
    filled, fixed = data["noise_before_retrogen"][0], data["noise"][0]
    settings = json.loads((ROOT / "fixtures/world-generation-surface-rules.json").read_text())["settings"][query["settings"]]["noise"]
    cell_height = settings["size_vertical"] * 4
    lower = (max(settings["min_y"], old["initial"]["generation_min_y"]) // cell_height) * cell_height
    upper = lower + old["fill_cell_count_y"] * cell_height
    require(changes(before, filled, minimum, lambda y,x,z,a,b: not lower <= y < upper) == 0,
            identity + ": NOISE changed an unwritten tail/padding cell")
    fill_changes = changes(before, filled, minimum)
    below = changes(before, filled, minimum, lambda y,x,z,a,b: y < query["level_min_y"])
    pairs = list(zip(values(before), values(filled)))
    gold = sum(a == b == state["gold_block"] and lower <= minimum + i // 256 < upper
               for i, (a, b) in enumerate(pairs))
    tail = sum(a == b == state["gold_block"] and not lower <= minimum + i // 256 < upper
               for i, (a, b) in enumerate(pairs))
    bedrock = changes(filled, fixed, minimum,
        lambda y,x,z,a,b: 0 <= y <= 4 and a == state["bedrock"] and b == state["deepslate"])
    masked = changes(filled, fixed, minimum,
        lambda y,x,z,a,b: -64 <= y < 0 and z == 0 and x in [0, 2] and b == state["air"])
    surface_changed, added_posts, quart_queries, foreign_queries = 0, 0, 0, 0
    if old["biomes_ran"]:
        for key in ["same_noise_chunk", "same_aquifer", "same_blender", "same_cache_owners", "biomes_same_sampler"]:
            require(old[key] is True, identity + ": original " + key + " failed")
        require(new["biomes_same_sampler"] is True and new["native_cache_present_before_noise"] is True
                and new["native_cached_noise_chunk_present"] is True, identity + ": native cache/sampler missing")
        require(equal(old["source_use_legacy_random_source"], new["native_use_legacy_random_source"]),
                identity + ": settings legacy flag differs")
    if old["surface_ran"]:
        require(new["native_phase"] == "Surface", identity + ": native SURFACE phase missing")
        require(old["surface_stored_biomes_unchanged"] is True
                and new["surface_stored_biomes_unchanged"] is True, identity + ": SURFACE changed BIOMES")
        surface_changed = changes(fixed, data["surface"][0], minimum)
        added_posts = old["surface"]["post_count"] - old["noise"]["post_count"]
        quart_queries, foreign_queries = surface_queries(query, old, new, data["noise"][1], work)
        if "retrogen" in query:
            require(changes(fixed, data["surface"][0], minimum, lambda y,x,z,a,b: y >= 0) == 0,
                    identity + ": SURFACE wrote above retrogen generation range")
    else:
        require(new["native_phase"] == "NoiseTaskFinished", identity + ": skipped task not finished")
    if "retrogen" in query:
        require(bedrock > 0 and masked > 0, identity + ": positive bedrock/mask branch not observed")
        if old["biomes_ran"]:
            upper_section = (0 - minimum) // 16
            require(data["initial"][1][upper_section * 64:] == data["biomes"][1][upper_section * 64:],
                    identity + ": retrogen rewrote upper palettes")
    if query.get("skip_biomes"):
        require(old["fill_cell_count_y"] <= 0 and old["noise_chunk_absent"] is True
                and new["noise_chunk_absent"] is True and new["native_cache_present_before_noise"] is False
                and new["native_cached_noise_chunk_present"] is False, identity + ": NoCells manufactured cache")
        require(initial == filled and initial_biomes == data["noise"][1] and not old["biome_queries"],
                identity + ": NoCells fill changed input or queried biome source")
        require(old["source_biome_source_absent"] is True,
                identity + ": NoCells did not demonstrate absent biome-source dependency")
    return dict(id=identity, complete_phase_arrays_palettes_metadata_equal=True, phases=phases,
        actual_noise_changed_blocks=fill_changes, actual_below_logical_noise_writes=below,
        actual_noise_added_posts=old["noise_before_retrogen"]["post_count"] - old["initial"]["post_count"],
        actual_preserved_gold_inside_noise_span=gold, actual_preserved_tail_gold=tail,
        actual_bedrock_to_deepslate_changes=bedrock, actual_masked_to_air_changes=masked,
        actual_surface_changed_blocks=surface_changed, actual_surface_added_posts=added_posts,
        surface_quart_queries=quart_queries, surface_foreign_quart_queries=foreign_queries,
        native_phase=new["native_phase"], native_box_section_state_map_postarray_identity=True,
        native_private_aquifer_identity_observed=False,
        phase_sha256={phase: dict(blocks=old[phase]["blocks_sha256"], biomes=old[phase]["biomes_sha256"])
                      for phase in phases})


def clean(value, work):
    if isinstance(value, dict):
        return {key: (str(artifact(item, work).relative_to(work)) if key in ["blocks_file", "biomes_file"]
                      else clean(item, work)) for key, item in value.items()}
    if isinstance(value, list):
        return [clean(item, work) for item in value]
    return value


def compare(args, work):
    production, probes = source_pins()
    inner, libraries, artifacts = pinned_artifacts(args.oracle_dir.resolve())
    java = str(args.java_home / "bin/java") if args.java_home else shutil.which("java")
    javac = str(args.java_home / "bin/javac") if args.java_home else shutil.which("javac")
    require(java and javac, "Java 21 java/javac required")
    versions = {}
    for name, tool in [("java", java), ("javac", javac)]:
        stdout, _ = bounded([tool, "-version"], work, name + "-version", wall=15)
        versions[name] = stdout + (work / (name + "-version.stderr.log")).read_text()
        require(re.search(r'(?:version\s+"|javac\s+)21(?:[."\s]|$)', versions[name]),
                name + ": Java 21 required")
    queries, declared, states = cases(work)
    write_json(work / "declared-inputs.json", declared)
    parent, child = work / "parent-classes", work / "child-classes"
    parent.mkdir()
    child.mkdir()
    bootstrap = work / "WorldgenExistingTargetBootstrap.java"
    bootstrap.write_text(BOOTSTRAP)
    cp = os.pathsep.join(map(str, [parent, inner, *libraries]))
    _, parent_compile = bounded([javac, "-proc:none", "-cp", cp, "-d", str(parent), str(bootstrap),
        str(HERE / "WorldgenSurfaceOracle.java"), str(HERE / "WorldgenBiomeStageOracle.java")],
        work, "compile-parent", wall=60)
    _, child_compile = bounded([javac, "-proc:none", "-cp", cp, "-d", str(child),
        str(HERE / "WorldgenExistingTargetOracle.java")], work, "compile-child", wall=60)
    transcript = "".join(encoded(query) + "\n" for query in queries)
    original_stdout, original_execution = bounded([java, "-Xmx1600m", "-cp", cp,
        "oracle.WorldgenExistingTargetBootstrap", str(child), str(work)],
        work, "original-existing-target", stdin=transcript, wall=args.wall_seconds)
    original = [json.loads(line) for line in original_stdout.splitlines() if line.startswith("{")]
    require(len(original) == len(queries), "incomplete original transcript")
    native_input = [dict(query=query, initial=row["initial"]) for query, row in zip(queries, original)]
    require(all(set(row) == {"query", "initial"} for row in native_input), "future expected state leaked to native")
    target = args.target_dir or Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    target = target if target.is_absolute() else ROOT / target
    if not args.skip_build:
        bounded(["cargo", "build", "--locked", "--lib", "--target-dir", str(target)],
                work, "cargo-lib", wall=240, cwd=ROOT)
    deps, library = target / "debug/deps", target / "debug/libmc121_rust.rlib"
    require(library.is_file(), "missing explicitly built native library")
    extern, library_hashes = ["--extern", "mc121_rust=" + str(library)], {"mc121_rust": sha(library)}
    for name in ["anyhow", "serde_json", "openssl"]:
        choices = sorted(deps.glob("lib" + name + "-*.rlib"), key=lambda p: p.stat().st_mtime, reverse=True)
        require(choices, "missing dependency " + name)
        extern.extend(["--extern", name + "=" + str(choices[0])])
        library_hashes[name] = sha(choices[0])
    executable = work / "existing-target-probe"
    _, native_compile = bounded([args.rustc, "--edition=2021", str(HERE / "existing_target_probe.rs"),
        "-L", "dependency=" + str(deps), *extern, "-o", str(executable)],
        work, "compile-native", wall=90)
    native_stdout, native_execution = bounded([str(executable), str(work)], work,
        "native-existing-target", stdin="".join(encoded(row) + "\n" for row in native_input), wall=args.wall_seconds)
    native = [json.loads(line) for line in native_stdout.splitlines()]
    require(len(native) == len(queries), "incomplete native transcript")
    results = [compare_case(q, old, new, work, states) for q, old, new in zip(queries, original, native)]
    by_id = {row["id"]: row for row in results}
    require(any(row["actual_preserved_gold_inside_noise_span"] > 0 for row in results
                if row["id"] != "no_cells_retrogen"), "positive old-block preservation not observed")
    require(all(by_id[name]["actual_preserved_tail_gold"] > 0
                and by_id[name]["actual_below_logical_noise_writes"] > 0
                for name in ["partial_positive", "partial_negative"]),
            "selected source did not exercise both partial tail and below-logical writes")
    require(any(row["actual_surface_changed_blocks"] > 0 for row in results), "no positive SURFACE block mutation")
    require(any(row["surface_foreign_quart_queries"] > 0 for row in results), "no actual foreign SURFACE palette query")
    require(source_pins() == (production, probes), "source changed during paired observation")
    common = dict(minecraft="1.21", protocol=767, oracle_inner_sha256=INNER,
        production_sha256=production, probe_sha256=probes, artifact_sha256=artifacts,
        bootstrap_sha256=hashlib.sha256(BOOTSTRAP.encode()).hexdigest(),
        native_rlib_sha256=library_hashes, native_probe_sha256=sha(executable), java_versions=versions,
        input_phase_separation="Java runs first; native stdin contains exactly query and original INITIAL, no future phase output",
        evidence_kind="Actual unchanged original methods and native live-target replay over declared controlled input")
    runtime = common | dict(declared_inputs=declared, original=clean(original, work), native=clean(native, work),
        executions=dict(original=original_execution, native=native_execution,
                        compile_parent=parent_compile, compile_child=child_compile, compile_native=native_compile),
        raw_transcript_sha256={name: sha(work / name) for name in [
            "original-existing-target.stdin.jsonl", "original-existing-target.stdout.log", "original-existing-target.stderr.log",
            "native-existing-target.stdin.jsonl", "native-existing-target.stdout.log", "native-existing-target.stderr.log"]})
    report = common | dict(success=True, cases=results,
        controlled_context="Declared canonical block/registered biome palettes, queues and actual CODEC retrogen payload. Original ProtoChunk persists EMPTY; no saved-world, WorldGenRegion, ChunkStep completion, beardifier or FULL owner is fabricated.",
        source_owner_scope="Original NoiseChunk/aquifer/Blender/cache and controlled BIOMES sampler identities are observed. Native Box/section/state/map/post-array identity and released leases are separate observations. Native private AQ pointer and runtime lock/acquire order are not observed.",
        surface_scope="Actual SurfaceSystem uses full chunk build bounds and live WG heights, with generation-accessor write clipping. Seeded zoom reads stored center/foreign palettes. At EMPTY the probes expose a declared base-ChunkAccess view without falsifying ProtoChunk status.",
        layout="Physical canonical u32 LE Y/X/Z; palettes sectionY/quartY/Z/X; first-available WG Z/X; signed packed-short queue order per section, preserving null/allocated-empty/duplicates.",
        heightmap_scope="WG first-available values are imported and compared; unused original packed-word bits are not measured or claimed preserved.",
        surface_postprocessing_scope="Complete queues/allocation/prefixes are compared. Zero selected SURFACE-only additions prove no positive insertion branch; no global inter-section order is inferred.",
        not_verified=["Actual scheduler/ChunkStep status completion or native Main integration.",
                      "Saved-world import/DFU/persistence/FULL publication or concrete WorldGenRegion dependencies.",
                      "Nonempty beardifier, every retrogen target status, datapack/custom registry overrides.",
                      "Native private AQ/cache allocation identity or original runtime lock/acquire order."])
    return runtime, report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oracle-dir", type=Path, required=True)
    parser.add_argument("--java-home", type=Path)
    parser.add_argument("--work-dir", type=Path)
    parser.add_argument("--target-dir", type=Path)
    parser.add_argument("--rustc", default="rustc")
    parser.add_argument("--skip-build", action="store_true", help="Only after explicit lib build of the same source/target")
    parser.add_argument("--wall-seconds", type=int, default=180)
    parser.add_argument("--runtime-report", type=Path)
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    require(os.name == "posix", "bounded process groups require POSIX")
    require(1 <= args.wall_seconds <= 600, "wall-seconds must be between 1 and 600")
    if args.work_dir:
        work = external(args.work_dir)
        work.mkdir(parents=True, exist_ok=True)
        require(not any(work.iterdir()), "external work directory must be empty; no implicit retry")
    else:
        work = Path(tempfile.mkdtemp(prefix="cinderstone-existing-target-"))  # Retain transcripts.
    runtime, report = compare(args, work)
    for path, value in [(args.runtime_report, runtime), (args.report, report)]:
        if path:
            path.parent.mkdir(parents=True, exist_ok=True)
            write_json(path, value)
    print(encoded(dict(success=True, external_work_directory=str(work), cases=report["cases"])))


if __name__ == "__main__":
    main()
