#!/usr/bin/env python3
"""Original NbtIo with disk DataOutputStream vs Netty ByteBufOutputStream; all values raw hex."""
from pathlib import Path
import hashlib
import json
import os
import random
import subprocess

ROOT = Path(__file__).resolve().parents[1]
ORACLE = ROOT.parent / "mc121-oracle"
JDK = Path("/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin")
PIN = "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def numeric_queries():
    result = []
    rng = random.Random(0x1214E4254)
    for kind, width, exponent, mantissa in [(5, 32, 8, 23), (6, 64, 11, 52)]:
        sign = 1 << (width - 1)
        exp = ((1 << exponent) - 1) << mantissa
        quiet = 1 << (mantissa - 1)
        positive = [0, 1, 2, (1 << mantissa) - 1, 1 << mantissa,
                    (1 << mantissa) + 1, ((1 << (exponent - 1)) - 1) << mantissa,
                    (((1 << (exponent - 1)) - 1) << mantissa) - 1,
                    (((1 << (exponent - 1)) - 1) << mantissa) + 1,
                    exp - 1, exp, exp | 1, exp | 2, exp | (quiet - 1),
                    exp | quiet, exp | quiet | 1, exp | ((1 << mantissa) - 1)]
        values = positive + [v | sign for v in positive]
        values += [rng.getrandbits(width) for _ in range(32)]
        # Explicit random positive/negative quiet and signalling NaN payloads.
        values += [exp | rng.randrange(1, 1 << mantissa) | (sign if i % 2 else 0) for i in range(24)]
        for value in dict.fromkeys(values):
            result.append({"tag_type": kind, "bits": f"{value:0{width // 4}x}"})
    return result


def nested_queries():
    # Every compound has one key so this fixture isolates floating-point semantics,
    # independently of CompoundTag HashMap ordering.
    result = []
    for kind, width, values in [(5, 4, [0x80000000, 0x7FA00001, 0xFFC12345, 1, 0x7F800000]),
                                (6, 8, [0x8000000000000000, 0x7FF0000000000001,
                                         0xFFF8123456789ABC, 1, 0x7FF0000000000000])]:
        payloads = [v.to_bytes(width, "big") for v in values]
        single = bytes([kind]) + payloads[1]
        compound = b"\x0a" + single[:1] + b"\x00\x01x" + single[1:] + b"\x00"
        array = b"\x09" + bytes([kind]) + len(payloads).to_bytes(4, "big") + b"".join(payloads)
        wrapped = b"\x0a\x09\x00\x01x" + array[1:] + b"\x00"
        list_compounds = b"\x09\x0a" + len(payloads).to_bytes(4, "big") + b"".join(
            bytes([kind]) + b"\x00\x01x" + p + b"\x00" for p in payloads)
        deep = compound
        for _ in range(4):
            deep = b"\x0a\x0a\x00\x01x" + deep[1:] + b"\x00"
        for raw in [compound, array, wrapped, list_compounds, deep]:
            result.append({"input_network": raw.hex()})
    return result


def main():
    jar = ORACLE / "server-inner-1.21.jar"
    assert sha(jar) == PIN, "original Minecraft inner JAR changed"
    runtime = ROOT / "runtime/nbt-float"
    classes = runtime / "classes"
    classes.mkdir(parents=True, exist_ok=True)
    classpath = os.pathsep.join(map(str, [classes, jar, *sorted((ORACLE / "libraries").rglob("*.jar"))]))
    source = ROOT / "tools/nbt_float_oracle.java"
    subprocess.run([JDK / "javac", "-proc:none", "-cp", classpath, "-d", classes, source],
                   check=True, capture_output=True, text=True)
    queries = numeric_queries() + nested_queries()
    run = subprocess.run([JDK / "java", "-cp", classpath, "oracle.nbt_float_oracle"],
                         input="".join(json.dumps(q) + "\n" for q in queries),
                         capture_output=True, text=True, check=True)
    (runtime / "original.stdout").write_text(run.stdout)
    (runtime / "original.stderr").write_text(run.stderr)
    rows = [json.loads(line.removeprefix("ORACLE:")) for line in run.stdout.splitlines() if line.startswith("ORACLE:")]
    assert len(rows) == len(queries)
    assert any(line.startswith("SOURCE:") and line.endswith("server-inner-1.21.jar") for line in run.stdout.splitlines())
    for request, row in zip(queries, rows):
        assert all(row[k] == v for k, v in request.items())
    cases = [r for r in rows if "tag_type" in r]
    nested = [r for r in rows if "input_network" in r]
    fixture = {"minecraft": "1.21", "oracle_inner_sha256": PIN,
               "oracle": "Unmodified original uo.b(DataInput,uk), ue/uc.valueOf; uo.a(uy,DataOutput) and uo.b(uy,DataOutput).",
               "disk_output": "java.io.DataOutputStream", "network_output": "io.netty.buffer.ByteBufOutputStream",
               "named_root_name": "", "cases": cases, "nested": nested}
    destination = ROOT / "fixtures/nbt-float-vectors.json"
    destination.write_text(json.dumps(fixture, separators=(",", ":")) + "\n")
    zero_count = sum(r["decoded_bits"] != r["bits"] for r in cases)
    nan_count = sum(r["disk_anonymous"] != r["network_anonymous"] for r in cases)
    report = {"status": "PASS", "minecraft": "1.21", "oracle_inner_sha256": PIN,
              "fixture_sha256": sha(destination), "oracle_source_sha256": sha(source),
              "generator_source_sha256": sha(Path(__file__)), "numeric_cases": len(cases),
              "nested_cases": len(nested), "negative_zero_factory_normalizations": zero_count,
              "disk_vs_network_nan_differences": nan_count,
              "original_classes_modified": False,
              "read_path": "Original NbtIo.readAnyTag with ByteBufInputStream; numeric decoded fields compared with direct original valueOf.",
              "findings": ["Both numeric factories normalize -0 to +0 on decode.",
                           "Netty output preserves NaN sign and payload bits.",
                           "Disk DataOutputStream canonicalizes every NaN to the positive Java canonical quiet NaN.",
                           "Nested lists and one-key compounds use the same recursive original writer semantics."],
              "limits": ["Finite corpus, not exhaustive over all 2^32/2^64 bit patterns.",
                         "Compound ordering is deliberately excluded by using one key per compound.",
                         "Raw signalling-NaN preservation is observed on the pinned JDK/platform."]}
    evidence = ROOT / "evidence/nbt-float-oracle.json"
    evidence.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
