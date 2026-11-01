#!/usr/bin/env python3
"""Original STREAM semantic prototype equality for overlong numeric and noncanonical true payloads."""
import hashlib
import json
import os
import subprocess

from inventory_normalization_generate import ROOT, ORACLE, JDK, SHA, slot, b64, unb64


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def overlong(canonical, width):
    assert len(canonical) < width <= 5 and canonical[-1] < 128
    return canonical[:-1] + bytes([canonical[-1] | 128]) + b"\x80" * (width - len(canonical) - 1) + b"\0"


def main():
    source = ROOT / "tools/inventory_normalization_oracle.java"
    jar = ORACLE / "server-inner-1.21.jar"
    assert sha(jar) == SHA
    assert sha(source) == "d5f6913082d975885e4f248ec0b6cf0e23ea2fbd1f4a67ab75bfc1b0df0a07ca"
    prototype_path = ROOT / "fixtures/item-components-1.21.json"
    prototype = json.loads(prototype_path.read_text())["prototypes"]
    cases = []

    def add(name, group, item, component, payload, canonical):
        cases.append({"name": name, "group": group, "item_id": item, "component_id": component,
                      "prototype_payload_hex": canonical.hex(), "variant_payload_hex": payload.hex(),
                      "query": {"bytes": b64(slot(item, [(component, payload)])),
                                "component_id": component, "component_payload": b64(payload)}})

    for item, component in [(1, 1), (771, 2), (771, 3), (1, 8), (1, 16), (1331, 41)]:
        canonical = unb64(prototype[str(item)][str(component)])
        for width in range(len(canonical) + 1, 6):
            add(f"scalar/{component}/item{item}/width{width}", "overlong_varint", item, component,
                overlong(canonical, width), canonical)
    for item, values in prototype.items():
        if int(item) > 0 and "18" in values:
            canonical = unb64(values["18"])
            assert canonical == b"\1"
            for variant in [2, 127, 255]:
                add(f"scalar/18/item{item}/true{variant}", "noncanonical_true", int(item), 18,
                    bytes([variant]), canonical)
    # Map color is fixed-width i32, so no alternate overlong VarInt representation exists.
    canonical = unb64(prototype["982"]["25"])
    assert len(canonical) == 4
    add("scalar/25/fixed_i32_control", "fixed_i32_control", 982, 25, canonical, canonical)
    assert len(cases) == 46
    runtime = ROOT / "runtime/inventory-normalization-scalar"
    classes = runtime / "classes"
    classes.mkdir(parents=True, exist_ok=True)
    cp = os.pathsep.join(map(str, [classes, jar, *sorted((ORACLE / "libraries").rglob("*.jar"))]))
    compile_result = subprocess.run([JDK / "javac", "-proc:none", "-cp", cp, "-d", classes, source], capture_output=True, text=True)
    if compile_result.returncode:
        raise RuntimeError(compile_result.stdout + compile_result.stderr)
    run = subprocess.run([JDK / "java", "-cp", cp, "oracle.inventory_normalization_oracle", ROOT / "tools/crafting_metadata.json"],
                         input="".join(json.dumps(c["query"]) + "\n" for c in cases), capture_output=True, text=True, timeout=120)
    (runtime / "oracle.log").write_text(run.stdout + run.stderr)
    if run.returncode:
        raise RuntimeError((run.stdout + run.stderr)[-5000:])
    expected = [json.loads(line[7:]) for line in run.stdout.splitlines() if line.startswith("ORACLE:")]
    assert len(expected) == len(cases)
    for case, result in zip(cases, expected):
        assert result["accepted"] and result["remaining"] == 0
        assert result["prototype_equals_decoded"] and result["redundant_removed"]
        assert result["component_consumed"] == len(bytes.fromhex(case["variant_payload_hex"]))
        assert result["patch_count"] == 0
        case["expected"] = result
    header = {"minecraft": "1.21", "protocol": 767, "oracle_inner_sha256": SHA,
              "oracle_source_sha256": sha(source), "prototype_fixture_sha256": sha(prototype_path),
              "codec": "Unchanged original cuq.h and original DataComponentType STREAM decode with original Objects.equals/prototype patched map"}
    fixture = ROOT / "fixtures/inventory_normalization_scalar_vectors.json"
    fixture.write_text(json.dumps({**header, "cases": cases}, separators=(",", ":")) + "\n")
    report = {"status": "PASS", **header, "cases": 46, "overlong_varints": 24,
              "noncanonical_true": 21, "fixed_i32_controls": 1,
              "variant_payload_differs_from_prototype_bytes": 45,
              "original_prototype_equals_decoded": 46, "original_redundant_overrides_removed": 46,
              "fixture_sha256": sha(fixture), "generator_sha256": sha(__import__("pathlib").Path(__file__)),
              "component_contracts": {"varint": [1, 2, 3, 8, 16, 41], "bool_nonzero_true": [18], "fixed_i32": [25]},
              "original_classes_modified": False, "native_implementation_verified": False,
              "limits": ["Only selected scalar defaults; not a generic equality proof for arbitrary components.",
                         "Overlong VarInts are limited to valid representations of at most five bytes.",
                         "Map color25 is fixed i32; no fake VarInt variants are generated."]}
    (ROOT / "evidence/inventory-normalization-scalar-oracle.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
