#!/usr/bin/env python3
"""Generate fixtures/commands-brigadier-dispatch-vectors.json by EXECUTING the original Brigadier.

    python3 tools/bootstrap_oracle.py --oracle-dir /path/outside/git
    python3 tools/brigadier_dispatch_generate.py --oracle-dir /path/outside/git

Seeded random command trees (literals, typed arguments, requirements, executable nodes,
redirects, forks, merged duplicate registrations) are built with the unchanged
com.mojang.brigadier 1.2.9 builders; random inputs (valid paths plus mutations) are parsed
and executed. The jar and libraries are proprietary downloads and stay outside git.
"""
import argparse
import json
import random
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SEED = 20261103
LIT_WORDS = ["foo", "bar", "baz", "list", "set", "get", "a", "ab", "abc", "x", "123", "exec", "run", "as", "at",
             "if", "été", "a.b", "a-b", "a_b", "😀"]
ARG_NAMES = ["a", "b", "c", "n"]
WORDS = ["hello", "world", "x1", "42", "a.b", "ZZ", "foo", "-", "..", "+1"]


def f32_bits(value):
    return format(struct.unpack(">I", struct.pack(">f", value))[0], "x")


def f64_bits(value):
    return format(struct.unpack(">Q", struct.pack(">d", value))[0], "x")


def gen_type(rng):
    kind = rng.choice(["int", "int", "long", "float", "double", "bool", "word", "string", "greedy"])
    spec = {"k": kind}
    if kind == "int":
        lo, hi = rng.choice([(-2**31, 2**31 - 1), (0, 10), (-5, 5), (1, 100), (5, 5)])
        spec.update(min=lo, max=hi)
    elif kind == "long":
        lo, hi = rng.choice([(-2**63, 2**63 - 1), (0, 10), (-1000, 1000)])
        spec.update(min=str(lo), max=str(hi))
    elif kind == "float":
        lo, hi = rng.choice([(-3.4028234663852886e38, 3.4028234663852886e38), (0.0, 1.0), (-10.0, 10.0)])
        spec.update(min=f32_bits(lo), max=f32_bits(hi))
    elif kind == "double":
        lo, hi = rng.choice([(-1.7976931348623157e308, 1.7976931348623157e308), (0.0, 1.0), (-10.0, 10.0)])
        spec.update(min=f64_bits(lo), max=f64_bits(hi))
    return spec


def gen_node(rng, depth, tops, in_redirect_ok=True):
    is_lit = rng.random() < 0.55
    spec = {"t": "lit" if is_lit else "arg"}
    if is_lit:
        spec["name"] = rng.choice(LIT_WORDS)
    else:
        spec["name"] = rng.choice(ARG_NAMES)
        spec["type"] = gen_type(rng)
    if rng.random() < 0.12:
        spec["req"] = rng.choice([1, 2, 2, 3, 4])
    children = []
    if depth < 3 and rng.random() < 0.7:
        used = set()
        for _ in range(rng.randint(1, 3)):
            child = gen_node(rng, depth + 1, tops)
            if child["name"] in used:
                # same-name sibling: only keep it when it has the same node kind (merge test)
                if any(c["name"] == child["name"] and c["t"] != child["t"] for c in children):
                    continue
            used.add(child["name"])
            children.append(child)
    if children:
        spec["children"] = children
    elif in_redirect_ok and rng.random() < 0.3:
        kind = rng.choice(["plain", "single", "fork", "fork"])
        spec["redirect"] = {"to": rng.choice(["root"] + tops), "kind": kind}
        if kind == "fork":
            spec["redirect"]["mod"] = rng.choice(["id", "dup", "none", "dup", "throw"])
    if rng.random() < (0.25 if children else 0.7):
        spec["cmd"] = "throw" if rng.random() < 0.08 else rng.randint(0, 50)
    return spec


def gen_tree(rng):
    tops = []
    tree = []
    for _ in range(rng.randint(1, 4)):
        spec = gen_node(rng, 0, list(dict.fromkeys(tops)))
        spec["t"] = "lit"  # Brigadier registers literals at the root
        spec["name"] = rng.choice(LIT_WORDS)
        spec.pop("type", None)
        tree.append(spec)
        tops.append(spec["name"])
    return tree


def sample_value(rng, t):
    kind = t["k"]
    if kind == "bool":
        return rng.choice(["true", "false", "maybe", "TRUE"])
    if kind == "int":
        lo, hi = t["min"], t["max"]
        return str(rng.choice([lo, hi, lo - 1, hi + 1, rng.randint(max(lo, -1000), min(hi, 1000)) if lo <= 1000 and hi >= -1000 else lo]))
    if kind == "long":
        lo, hi = int(t["min"]), int(t["max"])
        return str(rng.choice([lo, hi, rng.randint(max(lo, -1000), min(hi, 1000))]))
    if kind in ("float", "double"):
        return rng.choice(["0", "1.5", "-2", ".5", "11", "-11", "1e5", "3.25", "x"])
    if kind == "word":
        return rng.choice(WORDS)
    if kind == "string":
        return rng.choice(WORDS + ['"two words"', "'single q'", '"unterminated', '"a\\"b"'])
    return " ".join(rng.choice(WORDS) for _ in range(rng.randint(1, 3)))


def sample_input(rng, tree):
    tokens = []
    level = tree
    redirects = 0
    while level and len(tokens) < 10:
        node = rng.choice(level)
        tokens.append(node["name"] if node["t"] == "lit" else sample_value(rng, node["type"]))
        if "redirect" in node:
            redirects += 1
            if redirects > 2:
                break  # forks multiply contexts exponentially; keep executions small
            to = node["redirect"]["to"]
            level = tree if to == "root" else [t for t in tree if t["name"] == to][:1]
            level = level if to == "root" else level[0].get("children", [])
        else:
            level = node.get("children", [])
        if "cmd" in node and rng.random() < 0.6:
            break
    text = " ".join(tokens)
    roll = rng.random()
    if roll < 0.06 and tokens:
        text = " ".join(tokens[:-1])
    elif roll < 0.11:
        text += " junk"
    elif roll < 0.14:
        text = text.replace(" ", "  ", 1)
    elif roll < 0.17:
        text += " "
    elif roll < 0.19:
        text = " " + text
    elif roll < 0.23 and text:
        text = text[: rng.randint(0, len(text))]
    return text


def build_tests():
    rng = random.Random(SEED)
    tests = []
    for _ in range(90):
        tree = gen_tree(rng)
        inputs = []
        for _ in range(45):
            inputs.append({"input": sample_input(rng, tree), "source": rng.choice([2, 3, 4, 4, 1, 0])})
        tests.append({"tree": tree, "inputs": inputs})
    return tests


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--oracle-dir", type=Path, required=True)
    parser.add_argument("--out", type=Path, default=ROOT / "fixtures/commands-brigadier-dispatch-vectors.json")
    args = parser.parse_args()
    oracle = args.oracle_dir.resolve()
    libraries = sorted(str(p) for p in (oracle / "libraries").rglob("*.jar"))
    classpath = ":".join([str(oracle / "server-inner-1.21.jar"), *libraries])
    tests = build_tests()
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        subprocess.run(["javac", "-proc:none", "-Xlint:none", "-cp", classpath, "-d", str(tmp), str(ROOT / "tools/brigadier_dispatch_oracle.java")], check=True)
        (tmp / "cases.json").write_text(json.dumps(tests))
        subprocess.run(["java", "-cp", f"{classpath}:{tmp}", "cinderstone.oracle.brigadier_dispatch_oracle", str(tmp / "cases.json"), str(tmp / "out.json")], check=True, timeout=300)
        outputs = json.loads((tmp / "out.json").read_text())
    assert len(outputs) == len(tests)

    def lone(value):
        if isinstance(value, str):
            try:
                value.encode("utf-8")
                return False
            except UnicodeEncodeError:
                return True
        if isinstance(value, dict):
            return any(lone(v) for v in value.values())
        if isinstance(value, list):
            return any(lone(v) for v in value)
        return False

    kept_tests = []
    skipped = 0
    total = 0
    failures = 0
    for test, output in zip(tests, outputs):
        cases = []
        for entry, result in zip(test["inputs"], output["results"]):
            if lone(result):
                skipped += 1
                continue
            case = dict(entry)
            case.update(result)
            cases.append(case)
            total += 1
            failures += 1 if "err" in result else 0
        kept_tests.append({"tree": test["tree"], "cases": cases})
    document = {
        "format": "brigadier-1.2.9-dispatch-vectors-v1",
        "source": "unchanged com.mojang.brigadier 1.2.9 from the pinned 1.21 server, with net.minecraft.commands.BrigadierExceptions installed",
        "seed": SEED,
        "skipped_cases_with_lone_surrogate_output": skipped,
        "tests": kept_tests,
    }
    args.out.write_text(json.dumps(document, separators=(",", ":"), ensure_ascii=True) + "\n")
    print(f"wrote {args.out} ({len(kept_tests)} trees, {total} inputs, {failures} failing executions, {args.out.stat().st_size} bytes)")


if __name__ == "__main__":
    sys.exit(main())
