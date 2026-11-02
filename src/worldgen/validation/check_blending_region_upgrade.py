#!/usr/bin/env python3
"""Verify a closed genuine 1.17.1 world, then optionally run original 1.21.

The input world is never opened by 1.21: only a new byte-identical complete
copy is used. This bounded profile observes target (6,0) NOISE and public
nonempty Blender collection, not numeric/native/Main/FULL compatibility.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import runpy
import shutil
import sys
import tempfile
from types import SimpleNamespace
import uuid
import zipfile

HERE = Path(__file__).resolve().parent
FRESH_SHA = "43a0a12f2d5bc21d01b24da0f6b35f02fe183ad2e2d5863d10e0240490d4ad2f"
JAVA_SHA = "32ec3d049de352805a613e68a79e72e4293029dfa477481c727826059a00735c"
LEGACY_JAR_SHA = "e8c211b41317a9f5a780c98a89592ecb72eb39a6e475d4ac9657e5bc9ffaf55f"
LEGACY_CLASS_SHA = "4846a20513148027eaf124a51427df69b34560924e6471c8913613648e741f71"
LEGACY_MARKER_SHA = "3dc1c2f7fa2deb3f94ff809d4b7254b76e1f9420ca72b758d50344e1e074a801"
LEGACY_RESULT_SHA = "ef18808ee923dbf0dff377760e3a77993808714754109b5d332191949da2630a"
MARKER = "original-generation-provenance.json"


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def utilities():
    path = HERE / "check_blending_region.py"
    require(sha(path) == FRESH_SHA, "fresh runner changed; upgrade utilities need a new review")
    require(sha(HERE / "RealGameTestServer121.java") == JAVA_SHA, "counted Java source changed")
    # run_path loads guarded utility definitions without starting its CLI or
    # producing a bytecode cache in the source tree.
    return runpy.run_path(str(path), run_name="blending_region_fresh_utilities")


def expect(record, expected, context):
    require(isinstance(record, dict), context + " is missing")
    for key, value in expected.items():
        require(key in record and type(record[key]) is type(value) and record[key] == value,
                context + ": missing or unexpected " + key)


def inventory(root):
    result = {}
    require(root.is_dir() and not root.is_symlink(), "legacy tree must be a real directory")
    for path in sorted(root.rglob("*")):
        require(not path.is_symlink(), "world symlinks are unsupported")
        if path.is_dir():
            continue
        require(path.is_file(), "world contains a non-regular file")
        relative = path.relative_to(root).as_posix()
        result[relative] = dict(file=relative, size=path.stat().st_size, sha256=sha(path))
    return result


def outside_legacy(path, legacy, fresh):
    resolved = fresh["external"](path)
    require(resolved != legacy and legacy not in resolved.parents,
            "output/work paths must stay outside the original legacy world")
    return resolved


def controlled_storage(fresh, legacy):
    source = HERE / "RealGameTestServer121.java"
    controlled = fresh["source_audit"](source)
    literal = re.search(r'CONTROLLED_ROOT=Path.of\("([^"]+)"\)', source.read_text())
    require(literal is not None, "source controlled root missing")
    lexical = Path(literal.group(1)).absolute()
    require(not any(path.is_symlink() for path in [lexical, *lexical.parents]),
            "controlled storage root contains a symlink")
    storage = controlled / "repository-upgrade-probes"
    require(not storage.is_symlink() and storage.resolve().parent == controlled,
            "upgrade storage group is symlinked or escapes controlled root")
    outside_legacy(storage, legacy, fresh)
    return storage


def verify_tree(world, marker):
    require(sha(world / MARKER) == LEGACY_MARKER_SHA, "closed 1173 marker differs")
    rows = marker.get("raw_world_files_before_harness_metadata")
    require(isinstance(rows, list) and len(rows) == 11, "closed legacy inventory must have 11 raw files")
    expected = {}
    for row in rows:
        require(isinstance(row, dict) and isinstance(row.get("file"), str), "invalid legacy inventory row")
        relative = PurePosixPath(row["file"])
        require(not relative.is_absolute() and relative.parts and ".." not in relative.parts
                and str(relative) == row["file"] and "\\" not in row["file"]
                and row["file"] != MARKER, "unsafe legacy relative path")
        require(row["file"] not in expected, "duplicate legacy inventory path")
        require(type(row.get("size")) is int and row["size"] >= 0, "invalid legacy file size")
        expected[row["file"]] = row
    actual = inventory(world)
    require(set(actual) == set(expected) | {MARKER}, "legacy tree has missing or extra files")
    for relative, row in expected.items():
        require(actual[relative] == row, "legacy raw file differs: " + relative)
    require(actual["level.dat"]["sha256"] == marker["level_dat_sha256"], "legacy level metadata differs")
    return actual


def verify_legacy(args):
    require(not args.legacy_world.is_symlink(), "legacy world root must not be a symlink")
    world = args.legacy_world.resolve()
    result_path = args.legacy_result.resolve()
    require(sha(result_path) == LEGACY_RESULT_SHA, "actual closed legacy run-result ledger differs")
    result = json.loads(result_path.read_text())
    marker = json.loads((world / MARKER).read_text())
    raw = args.legacy_server_jar.read_bytes()
    require(len(raw) == 43626592 and hashlib.sha1(raw).hexdigest() == "a16d67e5807f57fc4e550299cf20226194497dc2"
            and hashlib.sha256(raw).hexdigest() == LEGACY_JAR_SHA, "incorrect official 1.17.1 artifact")
    with zipfile.ZipFile(args.legacy_server_jar) as archive:
        require(hashlib.sha256(archive.read("net/minecraft/server/MinecraftServer.class")).hexdigest()
                == LEGACY_CLASS_SHA, "original legacy superclass bytes differ")
        version = json.loads(archive.read("version.json"))
        expect(version, dict(id="1.17.1", world_version=2730, protocol_version=756), "legacy embedded version")
    expect(result, dict(attempt="legacy-normal-1173", exit_code=0, process_reaped=True,
        actual_original_generation_and_postclose_provenance_completed=True,
        provenance_sha256=LEGACY_MARKER_SHA, original_archive_sha256=LEGACY_JAR_SHA,
        source_version="1.17.1", actual_saved_DataVersion=2730, actual_generator="minecraft:noise",
        seed="123456789", raw_world_inventory_entries=11, raw_world_inventory_rechecked=True,
        close_error_lines=[], network_listeners_after_stop=0, eula_absent_before_after=True), "legacy run ledger")
    expect(marker, dict(official_version="1.17.1", official_server_sha256=LEGACY_JAR_SHA,
        runtime_original_superclass_sha256=LEGACY_CLASS_SHA, seed="123456789",
        saved_DataVersion=2730, saved_generator_type="minecraft:noise",
        base_stop_server_returned_before_observation=True, observed_status_counts={"full": 2},
        network_listeners_after_stop=0, eula_file_absent_before=True, eula_file_absent_after=True), "post-close provenance")
    request = dict(chunk=[0, 0], requested_status="minecraft:full", returned_status="minecraft:full",
        requested_serialized_status="full", returned_serialized_status="full", returned_class="cmx",
        generator_class="cpu", network_listeners_before_request=0, network_listeners_after_request=0)
    expect(marker.get("requested"), request, "original legacy request")
    expect(result.get("requested"), request, "legacy result request")
    chunks = marker.get("observed_saved_chunks")
    require(isinstance(chunks, list) and len(chunks) == 2
            and {tuple(row["chunk"]) for row in chunks} == {(0, -1), (0, 0)}, "unexpected legacy saved chunks")
    for row in chunks:
        expect(row, dict(DataVersion=2730, status="full"), "saved original chunk")
    require(result["observed_saved_chunks"] == chunks, "legacy saved-chunk observations differ")
    audit_path = args.legacy_build_audit or result_path.parent / "legacy-build-audit.json"
    require(sha(audit_path) == result["build_audit_sha256"], "legacy build audit differs")
    audit = json.loads(audit_path.read_text())
    expect(audit, dict(compile_exit_code=0, original_jar_sha256=LEGACY_JAR_SHA,
        original_superclass_sha256=LEGACY_CLASS_SHA, header_absent_from_runtime_output=True,
        header_absent_from_planned_runtime_classpath=True), "legacy compile/runtime ABI audit")
    files = verify_tree(world, marker)
    return world, marker, files, dict(result_sha256=sha(result_path), marker_sha256=LEGACY_MARKER_SHA,
        build_audit_sha256=sha(audit_path), official_jar_sha256=LEGACY_JAR_SHA,
        original_superclass_sha256=LEGACY_CLASS_SHA, saved_DataVersion=2730, seed="123456789",
        saved_generator_type="minecraft:noise", saved_chunks=chunks,
        raw_files=[row for relative,row in files.items() if relative != MARKER],
        marker_file=files[MARKER], source_regular_file_count=len(files),
        semantic_scope="Original post-close marker supplies NBT observations; this runner verifies its pinned digest and every raw byte, without rewriting or reinterpreting tags.")


def expected_upgrade(report):
    expect(report, dict(minecraft="1.21", protocol=767,
        oracle_inner_sha256="8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01",
        task_dispatch="MinecraftServer.execute after original public isReady",
        real_server_executor_task_executed=True, game_test_callback_executed=False,
        executor_task_count=1, game_test_callback_count=0, original_development_flag=False,
        same_original_server_thread=True, world_data_mode="upgrade", actual_level_class="aqu",
        actual_cache_class="aqs", actual_random_state_class="dzm", actual_generator_class="dzb",
        world_seed=123456789, chunk=[6, 0], requested_status="noise",
        persisted_status="minecraft:noise", highest_generated_status="minecraft:noise",
        retained_noise_chunk_class="dzc", retained_blender_empty_singleton=False,
        actual_network_listener_count=0, public_blender_call_counter_observed=False,
        source_backed_original_chunkstep_path=True, eula_file_written=False), "actual upgrade report")
    expect(report.get("explicit_region_collection"), dict(actual_region_class="arb",
        actual_generation_holders_acquired=289, public_blender_of_executed=True,
        public_blender_empty_singleton=False, public_get_or_update_executed=True,
        old_data_absent=False, old_data_calculated=True), "actual nonempty public region")


def run(args, work, fresh):
    old_world, marker, old_files, provenance = verify_legacy(args)
    compile_dir = work / "compile"
    compile_dir.mkdir()
    base_args = SimpleNamespace(oracle_dir=args.oracle_dir, java_home=args.java_home,
                               expected_source_sha256=JAVA_SHA, run_original=False)
    result = fresh["run"](base_args, compile_dir)
    result.update(upgrade_runner_sha256=sha(Path(__file__)), legacy_provenance=provenance,
                  input_world_opened_by_121=False, copied_world=False)
    if not args.run_original:
        return result

    group, name = "repository-upgrade-probes", "upgrade-region-" + uuid.uuid4().hex
    storage = controlled_storage(fresh, old_world)
    copied = storage / name
    report_path = storage / (name + "-1.21-report.json")
    require(not copied.exists() and not report_path.exists(), "upgrade copy/report already exists")
    storage.mkdir(parents=True, exist_ok=True)
    shutil.copytree(old_world, copied, copy_function=shutil.copy2, symlinks=False)
    copied_files = verify_tree(copied, marker)
    require(copied_files == old_files and verify_tree(old_world, marker) == old_files,
            "complete copy differs or original closed world changed")
    config = dict(mode="upgrade", storage_group=group, world_name=name, seed=123456789,
                  chunk_x=6, chunk_z=0, target_status="noise", public_region_collection=True,
                  expect_nonempty=True, old_chunk=[0, 0])
    config_path = work / "config.json"
    fresh["write_json"](config_path, config)
    cwd = work / "run"
    cwd.mkdir()
    launch = dict(max_heap_mib=2048, active_processor_count=4, wall_seconds=120,
                  source_sha256=JAVA_SHA, config_sha256=sha(config_path),
                  closed_source_marker_sha256=LEGACY_MARKER_SHA,
                  target="original 1.21 NOISE (6,0), retained/public nonempty Blender and old (0,0)")
    fresh["write_json"](work / "launch-marker.json", launch)
    java, _, _ = fresh["java_tools"](args)
    inner, libraries, _ = fresh["pinned_artifacts"](args.oracle_dir.resolve())
    cp = os.pathsep.join(map(str, [compile_dir / "classes", inner, *libraries]))
    execution, out, err = fresh["bounded"]([java, "-Xmx2048m", "-XX:ActiveProcessorCount=4", "-cp", cp,
        "oracle.RealGameTestServer121", "--execute-approved", str(config_path)], cwd, work / "original-server", 120)
    require(verify_tree(old_world, marker) == old_files, "original closed world changed during upgrade")
    require(execution["reaped"] and not execution["timed_out"] and execution["exit_code"] == 0,
            "original upgrade failed/timed out; preserve this attempt without retry")
    require(report_path.is_file(), "original upgrade report missing")
    original = json.loads(report_path.read_text())
    expected_upgrade(original)
    require(sha(HERE / "RealGameTestServer121.java") == JAVA_SHA and sha(HERE / "check_blending_region.py") == FRESH_SHA,
            "frozen source changed during upgrade")
    require(not list(work.rglob("eula.txt")) and not list(copied.rglob("eula.txt")), "EULA file appeared")
    require(copied.joinpath("level.dat").is_file(), "original upgrade world metadata missing")
    require("All dimensions are saved" in out.read_text(), "original save/close log is missing")
    shutil.copyfile(report_path, work / "runtime-report.json")
    result.update(server_executed=True, copied_world=True, complete_preupgrade_copy_equal=True,
        original_closed_world_unchanged=True, execution=execution, original=original,
        config_sha256=sha(config_path), semantic_config={k:v for k,v in config.items() if k not in ["world_name", "storage_group"]},
        runtime_report_sha256=sha(report_path), stdout_sha256=sha(out), stderr_sha256=sha(err),
        launch_marker_sha256=sha(work / "launch-marker.json"), postupgrade_files=list(inventory(copied).values()),
        eula_files_absent=True, original_save_close_log_observed=True,
        scope="Actual DFU/load of a complete genuine closed legacy copy, scheduler NOISE and separate public nonempty Blender/getOrUpdate observations. No pre-request status, numeric/native/Main/FULL parity or public call counter is observed.",
        old_map_scope="At this target the height/biome map may be nonempty while density-map owners are absent; no map keys, old bounds, retrogen state or numeric blend effect are reported by this Java source.",
        source_profile_scope="Original GameTestServer spawn/weather initialization and any template preparation remain original profile behavior, not harness metadata/rule/status patches.")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oracle-dir", type=Path, required=True)
    parser.add_argument("--java-home", type=Path)
    parser.add_argument("--legacy-world", type=Path, required=True)
    parser.add_argument("--legacy-result", type=Path, required=True)
    parser.add_argument("--legacy-server-jar", type=Path, required=True)
    parser.add_argument("--legacy-build-audit", type=Path)
    parser.add_argument("--work-dir", type=Path)
    parser.add_argument("--run-original", action="store_true")
    parser.add_argument("--report", type=Path, help="External result only; final fixtures remain coordinator-owned")
    args = parser.parse_args()
    require(os.name == "posix", "process-group isolation requires POSIX")
    fresh = utilities()
    legacy = args.legacy_world.resolve()
    # Resolve and reject caller outputs BEFORE mkdir, compilation, copying or
    # writing a report. Even an empty new subdirectory must not alter input.
    if args.work_dir:
        args.work_dir = outside_legacy(args.work_dir, legacy, fresh)
    if args.report:
        args.report = outside_legacy(args.report, legacy, fresh)
        require(not args.report.exists(), "external report already exists")
    controlled_storage(fresh, legacy)
    if args.work_dir:
        work = fresh["external"](args.work_dir)
        work.mkdir(parents=True, exist_ok=True)
        require(not any(work.iterdir()), "work directory must be empty; no implicit retry")
        result = run(args, work, fresh)
    else:
        temporary_root = outside_legacy(Path(tempfile.gettempdir()), legacy, fresh)
        with tempfile.TemporaryDirectory(prefix="cinderstone-region-upgrade-", dir=temporary_root) as temporary:
            result = run(args, fresh["external"](Path(temporary)), fresh)
    if args.report:
        report = fresh["external"](args.report)
        require(not report.exists(), "external report already exists")
        report.parent.mkdir(parents=True, exist_ok=True)
        fresh["write_json"](report, result)
    print(json.dumps(result))


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(json.dumps(dict(success=False, error=str(error))), file=sys.stderr)
        raise SystemExit(1)
