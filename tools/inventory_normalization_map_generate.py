#!/usr/bin/env python3
"""Bounded original equality proof for seven identity-hash-dependent trim map wire orders."""
from pathlib import Path
import hashlib
import json
import os
import subprocess

from inventory_normalization_generate import ROOT, ORACLE, JDK, SHA


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    source = ROOT / "tools/inventory_normalization_map_oracle.java"
    original_oracle = ROOT / "tools/inventory_normalization_oracle.java"
    jar = ORACLE / "server-inner-1.21.jar"
    assert sha(jar) == SHA
    assert sha(original_oracle) == "d5f6913082d975885e4f248ec0b6cf0e23ea2fbd1f4a67ab75bfc1b0df0a07ca"
    constructor_fixture = ROOT / "fixtures/inventory_normalization_constructor_vectors.json"
    constructor_sha = sha(constructor_fixture)
    assert constructor_sha == "0e4b814d29ca34b011e4fa50b22747a5747a3b2c38ac2e30eb6cfe92dd4f9025"
    cases = [{"name": c["name"], "input": c["query"]["bytes"], "reference": c["expected"]["canonical"]}
             for c in json.loads(constructor_fixture.read_text())["cases"]
             if c["group"] == "trim" and len(c.get("descriptor", {}).get("overrides", [])) == 2]
    assert len(cases) == 7
    runtime = ROOT / "runtime/inventory-normalization-map"
    classes = runtime / "classes"
    classes.mkdir(parents=True, exist_ok=True)
    cp = os.pathsep.join(map(str, [classes, jar, *sorted((ORACLE / "libraries").rglob("*.jar"))]))
    compile_result = subprocess.run([JDK / "javac", "-proc:none", "-cp", cp, "-d", classes, original_oracle, source], capture_output=True, text=True)
    if compile_result.returncode:
        raise RuntimeError(compile_result.stdout + compile_result.stderr)
    observations = []
    for perturb in [0, 2, 10]:
        queries = "".join(json.dumps({"input": c["input"], "reference": c["reference"]}) + "\n" for c in cases)
        run = subprocess.run([JDK / "java", "-cp", cp, "oracle.inventory_normalization_map_oracle",
                              ROOT / "tools/crafting_metadata.json", str(perturb)],
                             input=queries, capture_output=True, text=True, timeout=120)
        (runtime / f"oracle-{perturb}.log").write_text(run.stdout + run.stderr)
        if run.returncode:
            raise RuntimeError((run.stdout + run.stderr)[-5000:])
        rows = [json.loads(line[4:]) for line in run.stdout.splitlines() if line.startswith("MAP:")]
        assert len(rows) == len(cases)
        for case, result in zip(cases, rows):
            assert result["trim_equals"] and result["material_equals"] and result["map_equals"]
            assert result["map_class"] == "it.unimi.dsi.fastutil.objects.Object2ObjectOpenHashMap"
            for key in ["input_roundtrip", "reference_roundtrip"]:
                assert result[key]["accepted"] and result[key]["remaining"] == 0
            assert sorted((e["holder_id"], e["suffix"]) for e in result["input_entries"]) == [(0, "zero"), (1, "one")]
            assert sorted((e["holder_id"], e["suffix"]) for e in result["reference_entries"]) == [(0, "zero"), (1, "one")]
        observations.append({"extra_identity_hashes_before_first_decode": perturb,
                             "cases": [{**case, "expected": row} for case, row in zip(cases, rows)]})
    variants = {c["name"]: sorted({run["cases"][i]["expected"]["input_roundtrip"]["canonical"] for run in observations})
                for i, c in enumerate(cases)}
    changed = sum(len(v) > 1 for v in variants.values())
    assert changed == 7, "perturbation must expose all seven non-canonical order variants"
    header = {"minecraft": "1.21", "protocol": 767, "oracle_inner_sha256": SHA,
              "constructor_fixture_sha256": constructor_sha,
              "scope": "Original cuq.h decode/encode + original cwy.equals and original map.equals; seven inline trim material maps"}
    fixture = ROOT / "fixtures/inventory_normalization_map_vectors.json"
    fixture.write_text(json.dumps({**header, "runs": observations}, separators=(",", ":")) + "\n")
    bytecode = subprocess.run([JDK / "javap", "-classpath", jar, "-p", "-c", "-v", "cwy", "yv$13", "jm$c"],
                              capture_output=True, text=True, check=True).stdout
    (runtime / "map-bytecode.txt").write_text(bytecode)
    report = {"status": "PASS", **header, "fixture_sha256": sha(fixture), "oracle_source_sha256": sha(source),
              "generator_sha256": sha(Path(__file__)), "pairs": 7, "fresh_jvms": 3, "comparisons": 21,
              "original_trim_equal": 21, "original_material_equal": 21, "original_map_equal": 21,
              "case_names_with_multiple_original_wire_orders": list(variants),
              "different_wire_orders_observed": changed,
              "map_factory": "cwy BootstrapMethods #8: REF_newInvokeSpecial Object2ObjectOpenHashMap.<init>(int).",
              "map_codec": "yv$13 decode factory(min(length,65536)) then Map.put; encode writes size then Map.forEach(keyCodec,valueCodec), without sorting.",
              "constructor": "cwy stores incoming map directly; no copy or canonical ordering.",
              "key_hash": "jm$c Holder.Reference inherits Object.equals/hashCode; actual identity hashes recorded per observation.",
              "perturbation": "0/2/10 hashes of unrelated new Object instances before first packet decode, after original registry bootstrap. No original classes or methods modified.",
              "interpretation": "Bytewise order is process/context dependent; the same holder-ID-to-suffix map and full trim material remain equal by original Java semantics.",
              "limits": ["Only these seven pairs; no claim of exhaustive Java hash-table behavior.",
                         "Numeric sorting in a native encoder is not established as original wire behavior.",
                         "Equality checks are original Java calls; any native semantic comparator is a separate implementation."],
              "original_classes_modified": False}
    (ROOT / "evidence/inventory-normalization-map-oracle.json").write_text(json.dumps(report, indent=2) + "\n")
    assert sha(constructor_fixture) == constructor_sha
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
