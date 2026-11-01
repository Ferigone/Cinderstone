#!/usr/bin/env python3
"""Execute unchanged GameEventDispatcher with actual empty loaded registries."""
import hashlib
import json
import math
from pathlib import Path
import random
import struct
import subprocess

ROOT = Path(__file__).resolve().parents[1]
ORACLE = ROOT.parent / "mc121-oracle"
JDK = Path("/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin")
SHA = "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"


def i32(value):
    return (value + 2**31) % 2**32 - 2**31


def floor_java(value):
    if math.isnan(value):
        return 0
    truncated = max(-2**31, min(2**31 - 1, int(value))) if math.isfinite(value) else (2**31 - 1 if value > 0 else -2**31)
    return i32(truncated - 1) if value < truncated else truncated


def cases():
    result = []
    def add(position, radius, loaded, registry, event=None):
        block = list(map(floor_java, position))
        minimum = [i32(axis - radius) >> 4 for axis in block]
        maximum = [i32(axis + radius) >> 4 for axis in block]
        lengths = [max(0, high - low + 1) for low, high in zip(minimum, maximum)]
        if lengths[0] * lengths[2] > 1000 or math.prod(lengths) > 10000:
            return
        query = dict(position_bits=[struct.pack(">d", value).hex() for value in position], radius=radius, loaded=loaded, registry=registry)
        if event:
            query["event"] = event
        result.append(dict(name=f"empty{len(result)}", query=query))
    positions = [[0., 64., 0.], [-0., -64., -0.], [-0.001, 15.999, -16.],
                 [15.999999999999998, -65., 16.], [16., 319., 32.],
                 [-16.000000000000004, 10000., -32.], [29999999.99, -10000., -29999999.99],
                 [float("nan"), 0., 0.], [float("inf"), 0., 0.], [-float("inf"), 0., 0.],
                 [2147483647., 2147483647., 2147483647.], [-2147483649., -2147483649., -2147483649.]]
    for index, position in enumerate(positions):
        for radius in [-1, 0, 1, 8, 16, 32]:
            for loaded in ["all", "none", "checker"]:
                for registry in ["noop", "euclidean"]:
                    add(position, radius, loaded, registry,
                        ["hit_ground", "splash", "entity_damage"][index % 3] if radius == 16 else None)
    rng = random.Random(76700121)
    for index in range(420):
        position = [rng.randint(-2048, 2048) + rng.choice([0., -.001, .5, .9999999999999]) for _ in range(3)]
        radius = rng.choice([-1, 0, 1, 7, 8, 15, 16, 17, 31, 32])
        add(position, radius, ["all", "none", "checker"][index % 3], ["noop", "euclidean"][index % 2],
            ["hit_ground", "splash", "entity_damage"][index % 3] if radius == 16 else None)
    return result


def main():
    jar = ORACLE / "server-inner-1.21.jar"
    assert hashlib.sha256(jar.read_bytes()).hexdigest() == SHA
    runtime = ROOT / "runtime/game_event_context"
    classes = runtime / "classes"
    classes.mkdir(parents=True, exist_ok=True)
    classpath = ":".join(map(str, [classes, jar, *sorted((ORACLE / "libraries").rglob("*.jar"))]))
    flags = ["--add-exports", "java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED"]
    sources = ["tools/survival_oracle.java", "tools/game_event_context_oracle.java"]
    subprocess.run([JDK / "javac", *flags, "-proc:none", "-cp", classpath, "-d", classes,
                    *[ROOT / source for source in sources]], check=True)
    inputs = cases()
    process = subprocess.run([JDK / "java", *flags, "-Xmx1G", "-cp", classpath, "oracle.game_event_context_oracle"],
                             input="".join(json.dumps(row["query"]) + "\n" for row in inputs), text=True, capture_output=True)
    (runtime / "oracle.log").write_text(process.stdout + process.stderr)
    if process.returncode:
        raise RuntimeError((process.stdout + process.stderr)[-6000:])
    sources_seen = [line[7:] for line in process.stdout.splitlines() if line.startswith("SOURCE:")]
    assert len(sources_seen) == 5 and all(str(jar) in value for value in sources_seen)
    outputs = [json.loads(line[7:]) for line in process.stdout.splitlines() if line.startswith("ORACLE:")]
    assert len(outputs) == len(inputs)
    for row, output in zip(inputs, outputs):
        assert output["world_rng_before"] == output["world_rng_after"]
        for registry in output["registry_states"]:
            assert registry["active"] == registry["pending_add"] == registry["pending_remove"] == 0
            assert not registry["processing"]
        row["expected"] = output
    fixture = ROOT / "fixtures/game_event_context.json"
    fixture.write_text(json.dumps(dict(minecraft="1.21", oracle_inner_sha256=SHA, cases=inputs), separators=(",", ":")) + "\n")
    report = dict(ok=True, minecraft="1.21", oracle_inner_sha256=SHA, original_class_sources=sources_seen,
                  original_methods=["GameEventDispatcher.post", "LevelChunk.getListenerRegistry", "EuclideanGameEventListenerRegistry.visitInRangeListeners", "GameEventListenerRegistry.NOOP.visitInRangeListeners"],
                  cases=len(inputs), getter_calls=sum(len(row["trace"]) for row in outputs),
                  chunk_queries=sum(sum(call[0] == "chunk" for call in row["trace"]) for row in outputs),
                  section_visits=sum(sum(call[0] == "registry" for call in row["trace"]) for row in outputs),
                  materialized_registries=sum(row["materialized_registries"] for row in outputs),
                  world_rng_unchanged=True, original_classes_rewritten=False,
                  fixture_sha256=hashlib.sha256(fixture.read_bytes()).hexdigest(),
                  source_sha256={source: hashlib.sha256((ROOT / source).read_bytes()).hexdigest() for source in sources + ["tools/game_event_context_generate.py"]},
                  scope=["actual empty Euclidean/NOOP registries, controlled loaded chunk lookup", "X/Z/Y getter order, unclamped sectionY, signed wrap and Java floor edge inputs", "actual HIT_GROUND/SPLASH/ENTITY_DAMAGE holders and custom radius definitions", "actual empty registry creation and post-visit quiescent state, original level RNG seed unchanged"],
                  not_verified=["nonempty listener dispatch and debug event emission", "source-world registry certification or main integration", "a complete running ServerLevel; world/chunk-source adapters are controlled"],
                  static_debug_fact="Original agf.sendGameEvent is return-only; dya calls it only when visit returned true. Empty original visits return false.")
    (ROOT / "evidence/game-event-context-oracle.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
