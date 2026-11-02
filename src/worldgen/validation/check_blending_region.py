#!/usr/bin/env python3
"""Pinned original 1.21 region bootstrap: compile/passive audit by default.

--run-original executes one fresh normal-world observation through the real
server executor. All generated worlds/classes/logs remain outside the checkout.
The exact executed Java source retains its explicit fixed CONTROLLED_ROOT.
This runner neither accepts an EULA nor invokes dedicated-server Main.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import uuid
import zipfile

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
INNER = "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"
AUDIT_MARKER = "All reflected bootstrap/region signatures exist; no Minecraft target initialization or server execution performed."
PASSIVE_AUDIT = "package oracle;\nimport java.lang.reflect.*;\nimport java.util.*;\n/** Passive class metadata lookup only: no target construction/invocation/field get. */\npublic class PassiveApiAudit121 {\n static Class<?> c(String n)throws Exception{return Class.forName(n,false,PassiveApiAudit121.class.getClassLoader());}\n static Class<?>[] types(String...names)throws Exception{Class<?>[]r=new Class<?>[names.length];for(int i=0;i<r.length;i++)r[i]=switch(names[i]){case\"int\"->int.class;case\"long\"->long.class;case\"boolean\"->boolean.class;default->c(names[i]);};return r;}\n static void method(String owner,String name,String...args)throws Exception{c(owner).getMethod(name,types(args));}\n static void ctor(String owner,String...args)throws Exception{c(owner).getDeclaredConstructor(types(args));}\n public static void main(String[]args)throws Exception{\n  method(\"erf\",\"b\",\"java.nio.file.Path\");method(\"erf\",\"d\",\"java.lang.String\");method(\"erf$c\",\"h\");method(\"erf\",\"a\",\"com.mojang.serialization.Dynamic\",\"atp\",\"boolean\");method(\"erf\",\"a\",\"com.mojang.serialization.Dynamic\",\"ddr\",\"jz\",\"ka$b\");\n  ctor(\"alo$d\",\"atp\",\"ddr\",\"boolean\",\"boolean\");ctor(\"alo$c\",\"alo$d\",\"eu$a\",\"int\");method(\"alo\",\"a\",\"alo$c\",\"alo$f\",\"alo$e\",\"java.util.concurrent.Executor\",\"java.util.concurrent.Executor\");method(\"ad\",\"c\",\"java.util.function.Function\");method(\"ad\",\"g\");\n  method(\"dcs\",\"b\");method(\"dcs\",\"a\",\"dcs$e\");method(\"dcs$d\",\"a\",\"int\",\"net.minecraft.server.MinecraftServer\");c(\"dcs\").getField(\"Z\");c(\"tc\").getDeclaredField(\"t\");\n  ctor(\"dda\",\"java.lang.String\",\"dct\",\"boolean\",\"bqo\",\"boolean\",\"dcs\",\"ddr\");ctor(\"dck\",\"java.util.List\",\"java.util.List\");ctor(\"ddr\",\"dck\",\"cpl\");ctor(\"dzw\",\"long\",\"boolean\",\"boolean\");method(\"ejh\",\"a\",\"ka\");method(\"dzt\",\"a\",\"jz\");ctor(\"erj\",\"dda\",\"dzw\",\"erj$a\",\"com.mojang.serialization.Lifecycle\");ctor(\"alo$b\",\"java.lang.Object\",\"ka$b\");ctor(\"alp\",\"atu\",\"alc\",\"jt\",\"erl\");\n  ctor(\"tc\",\"java.lang.Thread\",\"erf$c\",\"atp\",\"alp\",\"java.util.Collection\",\"jd\");ctor(\"tr\",\"java.lang.String\",\"java.lang.String\",\"java.lang.String\",\"int\",\"long\",\"boolean\",\"java.util.function.Consumer\");method(\"sw\",\"a\");method(\"sw\",\"e\");method(\"sw\",\"a\",\"java.lang.String\");method(\"net.minecraft.server.MinecraftServer\",\"a\",\"java.util.function.Function\");\n  method(\"aqs\",\"a\",\"int\",\"int\",\"dvz\",\"boolean\");method(\"aqs\",\"g\");method(\"aqs\",\"i\");method(\"duy\",\"a\",\"java.util.function.Function\");method(\"dzc\",\"d\");method(\"aqm\",\"d\",\"long\");method(\"aqm\",\"a\",\"aqn\");method(\"azi\",\"a\",\"int\",\"int\",\"int\",\"azi$a\");ctor(\"arb\",\"aqu\",\"azi\",\"dwc\",\"duy\");method(\"eaa\",\"a\",\"arb\");method(\"eab\",\"a\",\"arb\",\"int\",\"int\");\n  System.out.println(\"All reflected bootstrap/region signatures exist; no Minecraft target initialization or server execution performed.\");\n }\n}\n"


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def write_json(path, value):
    Path(path).write_text(json.dumps(value, indent=2) + "\n")


def external(path):
    path = Path(path).resolve()
    require(path != ROOT and ROOT not in path.parents, "generated artifacts must stay outside the checkout")
    return path


def kill_group(process, sig):
    try:
        os.killpg(process.pid, sig)
    except ProcessLookupError:
        pass


def bounded(command, cwd, prefix, wall_seconds, grace_seconds=5):
    """One child group, finite wait, TERM/KILL, and unconditional parent reap."""
    start = time.monotonic()
    timed_out = False
    process = None
    code = None
    out_path, err_path = prefix.with_suffix(".stdout.log"), prefix.with_suffix(".stderr.log")
    try:
        with out_path.open("xb") as out, err_path.open("xb") as err:
            process = subprocess.Popen(command, cwd=cwd, stdout=out, stderr=err,
                                       start_new_session=True, stdin=subprocess.DEVNULL)
            try:
                code = process.wait(timeout=max(0.01, wall_seconds - grace_seconds))
            except subprocess.TimeoutExpired:
                timed_out = True
                kill_group(process, signal.SIGTERM)
                try:
                    code = process.wait(timeout=max(0.01, wall_seconds - (time.monotonic() - start)))
                except subprocess.TimeoutExpired:
                    kill_group(process, signal.SIGKILL)
                    code = process.wait()
    finally:
        if process is not None:
            # Kill any surviving helpers in this dedicated group, including
            # when the primary child exited normally or Python is interrupted.
            kill_group(process, signal.SIGKILL)
            process.wait()
            code = process.returncode
            execution = dict(exit_code=code, timed_out=timed_out,
                             elapsed_seconds=round(time.monotonic() - start, 3),
                             reaped=True, max_wall_seconds=wall_seconds)
            write_json(prefix.with_suffix(".execution.json"), execution)
    require(process is not None, "child could not start")
    return execution, out_path, err_path


def checked(command, cwd, prefix, wall_seconds=60):
    execution, out, err = bounded(command, cwd, prefix, wall_seconds)
    require(not execution["timed_out"] and execution["exit_code"] == 0,
            f"{prefix.name} failed; inspect external logs")
    return execution, out, err


def java_tools(args):
    java = str(args.java_home / "bin/java") if args.java_home else shutil.which("java")
    javac = str(args.java_home / "bin/javac") if args.java_home else shutil.which("javac")
    require(java and javac, "Java 21 java and javac are required")
    versions = {}
    for key, tool in [("java", java), ("javac", javac)]:
        version = subprocess.run([tool, "-version"], text=True, capture_output=True,
                                 timeout=10, check=True)
        text = version.stdout + version.stderr
        require(re.search(r"(?:version\s+\"|javac\s+)21(?:[.\"\s]|$)", text),
                f"{key} must be Java 21")
        versions[key] = text.strip()
    return java, javac, versions


def pinned_artifacts(oracle):
    lock = json.loads((ROOT / "fixtures/source-lock.json").read_text())
    artifacts = {}
    for section, filename in [("oracle", "server-1.21.jar"), ("mappings", "server-1.21-mappings.txt")]:
        path = oracle / filename
        raw = path.read_bytes()
        require(len(raw) == lock[section]["size"] and
                hashlib.sha1(raw).hexdigest() == lock[section]["sha1"],
                "incorrect pinned " + filename)
        artifacts[filename] = hashlib.sha256(raw).hexdigest()
    inner = oracle / "server-inner-1.21.jar"
    require(sha(inner) == INNER, "incorrect pinned inner server JAR")
    artifacts[inner.name] = INNER
    libraries = []
    with zipfile.ZipFile(oracle / "server-1.21.jar") as archive:
        versions = [row.split("\t") for row in archive.read("META-INF/versions.list").decode().splitlines()]
        selected = [row for row in versions if row[1] == "1.21"]
        require(len(selected) == 1 and selected[0][0] == INNER, "outer JAR does not pin the expected inner")
        require(hashlib.sha256(archive.read("META-INF/versions/" + selected[0][2])).hexdigest() == INNER,
                "outer bundled inner differs")
        for row in archive.read("META-INF/libraries.list").decode().splitlines():
            digest, coordinate, relative = row.split("\t")
            path = (oracle / "libraries" / relative).resolve()
            require((oracle / "libraries").resolve() in path.parents, "library path escapes oracle")
            require(sha(path) == digest, "incorrect bundled original library " + coordinate)
            libraries.append(path)
            artifacts["libraries/" + relative] = digest
    return inner, libraries, artifacts


def source_audit(source):
    raw = source.read_text()
    match = re.search(r'CONTROLLED_ROOT=Path.of\("([^"]+)"\)', raw)
    require(match, "fixed controlled storage guard is missing")
    controlled = external(Path(match.group(1)))
    require(controlled.is_absolute(), "Java controlled root must be absolute")
    # This complements exact source hashing; it is not a general verifier for
    # arbitrary Java programs or reflection semantics.
    for forbidden in [r"(?:sun\.misc|jdk\.internal\.misc)\.Unsafe", r"\bVarHandle\s*\.", r"\.set(?:Boolean|Byte|Short|Int|Long|Float|Double|Char)?\s*\(",
                      r"acceptEula", r"ServerConnectionListener.*startTcp"]:
        require(not re.search(forbidden, raw), "forbidden harness mutation/API pattern: " + forbidden)
    require('getMethod("execute",Runnable.class)' in raw and
            'getMethod("az").invoke(server)!=Thread.currentThread()' in raw,
            "actual server executor/thread guard is missing")
    return controlled


def expected_fresh(report, config):
    require(isinstance(report, dict) and report.get("phase") != "failed", "original report failed or is unknown")
    expected = dict(minecraft="1.21", protocol=767, oracle_inner_sha256=INNER,
                    task_dispatch="MinecraftServer.execute after original public isReady",
                    real_server_executor_task_executed=True, game_test_callback_executed=False,
                    executor_task_count=1, game_test_callback_count=0, original_development_flag=False,
                    same_original_server_thread=True, world_data_mode="fresh",
                    actual_level_class="aqu", actual_cache_class="aqs", actual_random_state_class="dzm",
                    actual_generator_class="dzb", world_seed=config["seed"], chunk=[0, 0],
                    requested_status="noise", persisted_status="minecraft:noise",
                    highest_generated_status="minecraft:noise", retained_noise_chunk_class="dzc",
                    retained_blender_empty_singleton=True, actual_network_listener_count=0,
                    public_blender_call_counter_observed=False, source_backed_original_chunkstep_path=True,
                    eula_file_written=False)
    for key, value in expected.items():
        require(key in report and type(report[key]) is type(value) and report[key] == value,
                "missing or unexpected original observation: " + key)
    region = report.get("explicit_region_collection")
    require(isinstance(region, dict), "actual public region report missing")
    for key, value in dict(actual_region_class="arb", actual_generation_holders_acquired=289,
                           public_blender_of_executed=True, public_blender_empty_singleton=True,
                           public_get_or_update_executed=True, old_data_absent=True).items():
        require(key in region and type(region[key]) is type(value) and region[key] == value,
                "missing or unexpected public region observation: " + key)


def run(args, work):
    oracle = args.oracle_dir.resolve()
    inner, libraries, artifacts = pinned_artifacts(oracle)
    source = HERE / "RealGameTestServer121.java"
    # Compile a captured copy, never a moving checkout path. A changed harness
    # is a new observation, distinct from the historical executed f0fe source.
    captured = work / source.name
    captured.write_bytes(source.read_bytes())
    captured_sha = sha(captured)
    if args.expected_source_sha256:
        require(captured_sha == args.expected_source_sha256, "unexpected Java source snapshot")
    controlled = source_audit(captured)
    java, javac, versions = java_tools(args)
    classes = work / "classes"
    classes.mkdir()
    audit = work / "PassiveApiAudit121.java"
    audit.write_text(PASSIVE_AUDIT.replace(
        '  System.out.println(',
        '  method("net.minecraft.server.MinecraftServer","execute","java.lang.Runnable");'
        'method("net.minecraft.server.MinecraftServer","aj");'
        'method("net.minecraft.server.MinecraftServer","az");'
        'c("ab").getDeclaredField("aV");\n  System.out.println('
    ))
    cp = os.pathsep.join(map(str, [classes, inner, *libraries]))
    compile_result, compile_out, compile_err = checked([javac, "-proc:none", "-cp", cp, "-d", str(classes),
                                   str(captured), str(audit)], work, work / "compile")
    audit_result, audit_out, audit_err = checked([java, "-Xmx256m", "-cp", cp, "oracle.PassiveApiAudit121"],
                                        work, work / "passive-audit", 30)
    require(audit_out.read_text().strip() == AUDIT_MARKER, "passive API audit marker missing or changed")
    class_hashes = {str(path.relative_to(classes)): sha(path) for path in sorted(classes.rglob("*.class"))}
    require("oracle/RealGameTestServer121.class" in class_hashes and
            "oracle/PassiveApiAudit121.class" in class_hashes, "compiled harness classes are missing")
    require(sha(source) == captured_sha, "Java checkout source changed during compilation")
    build = dict(minecraft="1.21", protocol=767, inner_sha256=INNER, source_sha256=captured_sha,
                 passive_audit_sha256=sha(audit), compiled_class_sha256=class_hashes,
                 compiled_only=True, server_executed=False)
    write_json(work / "build.json", build)
    result = dict(success=True, minecraft="1.21", protocol=767, source_sha256=captured_sha,
                  runner_sha256=sha(Path(__file__)), source_lock_sha256=sha(ROOT / "fixtures/source-lock.json"),
                  artifact_sha256=artifacts, build=build, java_versions=versions,
                  compile=compile_result, passive_api_audit=audit_result,
                  build_record_sha256=sha(work / "build.json"),
                  compile_stdout_sha256=sha(compile_out), compile_stderr_sha256=sha(compile_err),
                  passive_audit_stdout_sha256=sha(audit_out), passive_audit_stderr_sha256=sha(audit_err),
                  server_executed=False,
                  controlled_root_scope="Exact Java source retains its fixed external CONTROLLED_ROOT; arbitrary storage roots are unsupported.",
                  prior_failure_scope="The earlier GameTest-callback attempt timed out and is separate failure history; only an actual server.execute observation can pass this runner.")
    if not args.run_original:
        return result

    # A new unique name is mandatory. No prior save/report is deleted, reused,
    # datafixed by this runner, or replaced with a manufactured metadata file.
    name = "fresh-region-" + uuid.uuid4().hex
    group = "repository-fresh-probes"
    storage = controlled / group
    world = storage / name
    report_path = storage / (name + "-1.21-report.json")
    require(not world.exists() and not report_path.exists(), "fresh world/report already exists")
    config = dict(mode="fresh", storage_group=group, world_name=name, seed=123456789,
                  chunk_x=0, chunk_z=0, target_status="noise", public_region_collection=True,
                  expect_nonempty=False, old_chunk=[0, 0])
    config_path = work / "config.json"
    write_json(config_path, config)
    cwd = work / "run"
    cwd.mkdir()
    marker = dict(max_heap_mib=1536, wall_seconds=120, source_sha256=captured_sha,
                  config_sha256=sha(config_path),
                  target="original normal scheduler NOISE and separately constructed public EMPTY region")
    write_json(work / "launch-marker.json", marker)
    execution, out, err = bounded([java, "-Xmx1536m", "-cp", cp, "oracle.RealGameTestServer121",
                                   "--execute-approved", str(config_path)], cwd, work / "original-server", 120)
    require(execution["reaped"] and not execution["timed_out"] and execution["exit_code"] == 0,
            "original server failed/timed out; no successful region evidence")
    require(report_path.is_file(), "original server produced no report")
    original = json.loads(report_path.read_text())
    expected_fresh(original, config)
    require(sha(source) == captured_sha, "Java checkout source changed during original execution")
    eula_files = [*work.rglob("eula.txt"), *world.rglob("eula.txt")]
    require(not eula_files, "an EULA file appeared")
    require(world.joinpath("level.dat").is_file(), "original shutdown did not save actual world metadata")
    # Preserve original bytes privately; only sanitized observations/hashes are
    # returned. This runner never creates publication fixture JSON itself.
    shutil.copyfile(report_path, work / "runtime-report.json")
    semantic = {key: value for key, value in config.items() if key not in ["world_name", "storage_group"]}
    result.update(server_executed=True, execution=execution, original=original,
                  config_sha256=sha(config_path), semantic_config=semantic,
                  runtime_report_sha256=sha(report_path), stdout_sha256=sha(out), stderr_sha256=sha(err),
                  launch_marker_sha256=sha(work / "launch-marker.json"),
                  original_world_saved=True, eula_files_absent=True,
                  scope="Actual original scheduler NOISE plus separate real BIOMES-descriptor read-region public EMPTY Blender.of/getOrUpdate; no ChunkStep.apply on that read region, nonempty legacy proof, native comparison, Main or FULL claim.")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oracle-dir", required=True, type=Path)
    parser.add_argument("--java-home", type=Path)
    parser.add_argument("--expected-source-sha256", help="Require an explicitly frozen captured Java source digest")
    parser.add_argument("--work-dir", type=Path, help="New/empty directory outside checkout for private artifacts")
    parser.add_argument("--run-original", action="store_true", help="Execute one bounded fresh original server after compile/passive audit")
    parser.add_argument("--report", type=Path, help="Optional external sanitized result; final repository fixtures are coordinator-owned")
    args = parser.parse_args()
    require(os.name == "posix", "process-group isolation requires POSIX")
    if args.work_dir:
        work = external(args.work_dir)
        work.mkdir(parents=True, exist_ok=True)
        require(not any(work.iterdir()), "work directory must be empty; no implicit retry or overwrite")
        result = run(args, work)
    else:
        with tempfile.TemporaryDirectory(prefix="cinderstone-blending-region-") as temporary:
            result = run(args, external(Path(temporary)))
    if args.report:
        report = external(args.report)
        report.parent.mkdir(parents=True, exist_ok=True)
        require(not report.exists(), "external report already exists")
        write_json(report, result)
    print(json.dumps(result))


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(json.dumps(dict(success=False, error=str(error))), file=sys.stderr)
        raise SystemExit(1)
