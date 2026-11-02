#!/usr/bin/env python3
"""Generate fixtures/commands-mc-arguments-vectors.json by EXECUTING the original argument classes.

    python3 tools/loot_oracle_setup.py --oracle-dir /path/outside/git     # builds server-named.jar + cp.txt
    python3 tools/mc_arguments_generate.py --oracle-dir /path/outside/git

WP-E3 stage 6 (position arguments): Vec3Argument (centred and raw), BlockPosArgument, Vec2Argument
(centred and raw) and ColumnPosArgument of the pinned 1.21 server are run on seeded inputs built from
absolute, relative (`~`) and local (`^`) coordinate tokens, including malformed, mixed and truncated
ones. For every successful parse the harness also resolves the coordinates for a few command sources
(position and rotation), recording the position, rotation and block position the original computes.

The jar and libraries are proprietary downloads and stay outside git.
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
SEED = 20261106

TYPES = ["vec3", "vec3_raw", "blockpos", "vec2", "vec2_raw", "columnpos"]
ABSOLUTE = ["0", "1", "-1", "7", "-12", "1.5", "-0.25", ".5", "-.5", "5.", "100.75", "-30000000", "29999999.5",
            "2147483647", "-2147483648", "99999999999", "1e3", "00", "+1", "--1", "1x", "0.0", "-0", "-0.0", "3.0",
            "1.99999999999999999", "123456789.123456789", "-2147483649", "-99999999999", "-3000000000.5"]
RELATIVE = ["~", "~1", "~-2.5", "~.5", "~0", "~-0.0", "~5.", "~100", "~~", "~x", "~1x", "~-", "~+1"]
LOCAL = ["^", "^1", "^-3.25", "^.5", "^0", "^2.", "^x", "^^", "^-"]


def f64_bits(value):
    return format(struct.unpack(">Q", struct.pack(">d", value))[0], "x")


def f32_bits(value):
    return format(struct.unpack(">I", struct.pack(">f", value))[0], "x")


CLEAN = {
    "absolute": ["0", "1", "-1", "7", "-12", "1.5", "-0.25", ".5", "100.75", "-30000000", "29999999.5", "3.0", "2147483647"],
    "relative": ["~", "~1", "~-2.5", "~.5", "~0", "~-7", "~100"],
    "local": ["^", "^1", "^-3.25", "^.5", "^0", "^2", "^-7"],
}
CLEAN_INT = {
    "absolute": ["0", "1", "-1", "7", "-12", "100", "-30000000", "2147483647", "-2147483648"],
    "relative": CLEAN["relative"],
    "local": CLEAN["local"],
}


def sample_token(rng, style, integer):
    if rng.random() < 0.8:
        return rng.choice((CLEAN_INT if integer else CLEAN)[style])
    return rng.choice({"absolute": ABSOLUTE, "relative": RELATIVE, "local": LOCAL}[style])


def sample_input(rng, arity, integer):
    style = rng.choices(["absolute", "relative", "local", "mixed"], [35, 30, 20, 15])[0]
    tokens = []
    for _ in range(arity):
        tokens.append(sample_token(rng, rng.choice(["absolute", "relative", "local"]) if style == "mixed" else style, integer))
    roll = rng.random()
    if roll < 0.08 and tokens:
        tokens = tokens[: rng.randint(0, len(tokens))]
    elif roll < 0.20:
        tokens.append(sample_token(rng, style if style != "mixed" else "absolute", integer))
    separator = rng.choices([" ", "  ", "\t"], [90, 7, 3])[0]
    text = separator.join(tokens)
    roll = rng.random()
    if roll < 0.07:
        text += " "
    elif roll < 0.10:
        text = " " + text
    elif roll < 0.14 and text:
        text = text[: rng.randint(0, len(text))]
    return text


def sample_sources(rng):
    positions = [0.0, 0.5, -3.25, 100.75, 1000000.5, -30000000.5, 12.34567, -64.0, 3.0e9, -3.0e9]
    sources = []
    for _ in range(2):
        position = [rng.choice(positions) for _ in range(3)]
        rotation = [rng.choice([0.0, 90.0, -90.0, 45.5, -17.25, 0.1]), rng.choice([0.0, 90.0, 180.0, -180.0, 270.0, -540.5, 33.3, 720.0])]
        sources.append({"pos": [f64_bits(v) for v in position], "rot": [f32_bits(v) for v in rotation]})
    return sources


def build_cases(rng):
    cases = []
    for type_name in TYPES:
        arity = 2 if type_name in ("vec2", "vec2_raw", "columnpos") else 3
        for _ in range(700):
            case = {"type": type_name, "input": sample_input(rng, arity, type_name in ("blockpos", "columnpos")), "sources": sample_sources(rng)}
            if rng.random() < 0.05:
                case["input"] = "ab " + case["input"]
                case["cursor"] = 3
            cases.append(case)
    return cases


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--oracle-dir", type=Path, required=True)
    parser.add_argument("--out", type=Path, default=ROOT / "fixtures/commands-mc-arguments-vectors.json")
    args = parser.parse_args()
    oracle = args.oracle_dir.resolve()
    classpath_file = oracle / "cp.txt"
    if not classpath_file.exists():
        sys.exit("run tools/loot_oracle_setup.py --oracle-dir <dir> first (it builds server-named.jar and cp.txt)")
    classpath = classpath_file.read_text().strip()
    cases = build_cases(random.Random(SEED))
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        subprocess.run(["javac", "-proc:none", "-Xlint:none", "-cp", classpath, "-d", str(tmp), str(ROOT / "tools/mc_arguments_oracle.java")], check=True)
        (tmp / "cases.json").write_text(json.dumps(cases))
        subprocess.run(["java", "-cp", f"{classpath}:{tmp}", "cinderstone.oracle.mc_arguments_oracle", str(tmp / "cases.json"), str(tmp / "out.json")], check=True, timeout=300)
        outputs = json.loads((tmp / "out.json").read_text())
    assert len(outputs) == len(cases)
    kept, failures = [], 0
    for case, output in zip(cases, outputs):
        # the case's optional "cursor" is the start position; the oracle's "cursor" is the position afterwards
        entry = {key: value for key, value in case.items() if key != "cursor"}
        if "cursor" in case:
            entry["start"] = case["cursor"]
        entry.update(output)
        failures += 1 if "err" in output else 0
        kept.append(entry)
    document = {
        "format": "mc-arguments-vectors-v1",
        "source": "unchanged position argument classes of the pinned 1.21 server (official names), BrigadierExceptions installed",
        "seed": SEED,
        "cases": kept,
    }
    args.out.write_text(json.dumps(document, separators=(",", ":"), ensure_ascii=True) + "\n")
    print(f"wrote {args.out} ({len(kept)} cases, {failures} failures, {args.out.stat().st_size} bytes)")


if __name__ == "__main__":
    sys.exit(main())
