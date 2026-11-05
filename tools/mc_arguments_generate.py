#!/usr/bin/env python3
"""Generate fixtures/commands-mc-arguments-vectors.json by EXECUTING the original argument classes.

    python3 tools/loot_oracle_setup.py --oracle-dir /path/outside/git     # builds server-named.jar + cp.txt
    python3 tools/mc_arguments_generate.py --oracle-dir /path/outside/git

WP-E3 stages 6-7. Stage 6 (position arguments): Vec3Argument (centred and raw), BlockPosArgument, Vec2Argument
(centred and raw) and ColumnPosArgument of the pinned 1.21 server are run on seeded inputs built from
absolute, relative (`~`) and local (`^`) coordinate tokens, including malformed, mixed and truncated
ones. For every successful parse the harness also resolves the coordinates for a few command sources
(position and rotation), recording the position, rotation and block position the original computes.

Stage 9 appends seeded cases for the int and float RangeArguments (MinMaxBounds, with the squares the distance
checks use) after the stage 7 ones, from their own random stream.

Stage 7 appends seeded cases for TimeArgument, UuidArgument, EntityAnchorArgument, AngleArgument
(resolved for command sources) and SwizzleArgument after the stage 6 cases, from their own random stream, so
the stage 6 part of the fixture does not change.

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
VALUE_SEED = 20261107
RANGE_SEED = 20261108

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


def hexdigits(rng, count):
    return "".join(rng.choice("0123456789abcdefABCDEF") for _ in range(count))


def sample_uuid(rng):
    roll = rng.random()
    if roll < 0.35:
        lengths = [8, 4, 4, 4, 12]
    elif roll < 0.55:
        lengths = [rng.randint(0, 9) for _ in range(5)]
    elif roll < 0.65:
        lengths = [rng.randint(1, 20) for _ in range(5)]
    else:
        lengths = [rng.randint(0, 9) for _ in range(rng.choice([1, 2, 3, 4, 6, 7]))]
    text = "-".join(hexdigits(rng, n) for n in lengths)
    roll = rng.random()
    if roll < 0.10:
        text += rng.choice([" rest", "g", "-", " ", "xyz", "--"])
    elif roll < 0.15:
        text = "-" + text
    elif roll < 0.20 and text:
        text = text[: rng.randint(0, len(text))]
    elif roll < 0.25:
        text = text.upper()
    return text


def sample_uuid_wide_part(rng):
    """Five parts, one of them 16 or more hex digits, in at most 36 characters, so that Long.parseLong's overflow
    (a value above Long.MAX_VALUE, even with leading zeros) decides instead of the length check."""
    wide = rng.choice([
        rng.choice("89abcdefABCDEF") + hexdigits(rng, 15),      # 16 digits >= 2^63: NumberFormatException
        rng.choice("01234567") + hexdigits(rng, 15),            # 16 digits < 2^63: fine, masked
        "0" + rng.choice("89abcdef") + hexdigits(rng, 15),      # 17 digits with a leading zero, >= 2^63
        "0" * rng.randint(1, 10) + hexdigits(rng, rng.randint(1, 6)),  # many leading zeros: fine
        "7fffffffffffffff", "8000000000000000", "0000000000000000", "ffffffffffffffff",
    ])
    parts = [hexdigits(rng, rng.randint(1, 4)) for _ in range(5)]
    parts[rng.randrange(5)] = wide
    text = "-".join(parts)
    return text if len(text) <= 36 else "-".join([wide] + ["1"] * 4)


def sample_time(rng):
    number = rng.choice(["0", "1", "5", "20", "1.5", "0.5", "2.5", "-1", "-0.5", "-2.5", "0.05", "100", "24000", "0.000001", "99999999999",
                         ".5", "5.", "-", "abc", "", "--1", "1.2.3"])
    unit = rng.choice(["", "", "t", "s", "d", "D", "S", "ticks", "m", "dd", "1", "t ", "s5", "é"])
    return number + unit


def sample_swizzle(rng):
    return "".join(rng.choice("xyz") for _ in range(rng.randint(0, 4))) if rng.random() < 0.8 else "".join(rng.choice("xyzXYZ a-") for _ in range(rng.randint(0, 4)))


def sample_angle(rng):
    text = rng.choice(["0", "~", "~-5", "1.5", "-90.5", "~0.5", "180", "360", "-540", "720.25", "~180", "~-720", "0.1",
                       "9" * 45 + ".0", "-" + "9" * 45 + ".0", "~" + "9" * 45, "~x", "x", "-", "~-", "..", "1..2", "--1"])
    if rng.random() < 0.1:
        text = text[: rng.randint(0, len(text))]
    if rng.random() < 0.08:
        text += " 3"
    return text


def build_value_cases(rng):
    cases = []
    for _ in range(450):
        case = {"type": "time", "input": sample_time(rng), "sources": []}
        minimum = rng.choice([None, None, 0, 1, 5, -100])
        if minimum is not None:
            case["min"] = minimum
        cases.append(case)
    for _ in range(500):
        cases.append({"type": "uuid", "input": sample_uuid(rng), "sources": []})
    for _ in range(150):
        cases.append({"type": "uuid", "input": sample_uuid_wide_part(rng), "sources": []})
    names = ["feet", "eyes", "feet", "eyes", "FEET", "Eyes", "fe", "feet2", "", "eyes ", "é", "eyes extra", "feet_", "feet.x", "-eyes"]
    for _ in range(120):
        cases.append({"type": "anchor", "input": rng.choice(names), "sources": []})
    for _ in range(350):
        cases.append({"type": "angle", "input": sample_angle(rng), "sources": sample_sources(rng)})
    for _ in range(250):
        cases.append({"type": "swizzle", "input": sample_swizzle(rng), "sources": []})
    return cases


RANGE_NUMBERS = ["0", "1", "5", "-5", "10", "100", "-100", "2147483647", "-2147483648", "2147483648", "99999999999", "1.5", "-0.25", ".5", "5.",
                 "-.5", "0.0", "00", "3.14159", "1000000.5", "-", "--1", "1-", "1-2", "1.2.3", "..5", "x", "1x"]


def sample_range(rng):
    low, high = rng.choice(RANGE_NUMBERS), rng.choice(RANGE_NUMBERS)
    shape = rng.choices(["single", "both", "min_only", "max_only", "dots", "odd"], [18, 40, 12, 12, 6, 12])[0]
    if shape == "single":
        text = low
    elif shape == "both":
        text = f"{low}..{high}"
    elif shape == "min_only":
        text = f"{low}.."
    elif shape == "max_only":
        text = f"..{high}"
    elif shape == "dots":
        text = rng.choice(["..", ".", "...", "....", ". .", ".. ", " .."])
    else:
        text = rng.choice([f"{low}...{high}", f"{low}..{high}..{rng.choice(RANGE_NUMBERS)}", f"{low}.. {high}", f"{low} ..{high}",
                           f"{low}..{high} junk", f"{low}.{high}", ""])
    if rng.random() < 0.06 and text:
        text = text[: rng.randint(0, len(text))]
    return text


def build_range_cases(rng):
    cases = []
    for kind in ("int_range", "float_range"):
        for _ in range(900):
            cases.append({"type": kind, "input": sample_range(rng), "sources": []})
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
    cases = build_cases(random.Random(SEED)) + build_value_cases(random.Random(VALUE_SEED)) + build_range_cases(random.Random(RANGE_SEED))
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
