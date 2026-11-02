#!/usr/bin/env python3
"""Generate fixtures/commands-brigadier-vectors.json by EXECUTING the original Brigadier.

    python3 tools/bootstrap_oracle.py --oracle-dir /path/outside/git
    python3 tools/brigadier_generate.py --oracle-dir /path/outside/git

The oracle directory holds the pinned, checksum-verified server jar and libraries
(proprietary; never commit them). The harness tools/brigadier_oracle.java runs the
unchanged com.mojang.brigadier 1.2.9 classes with the server's own exception provider.
Inputs are curated edge cases plus seeded random strings, so the file is reproducible.
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
SEED = 20261102


def f32_bits(value):
    return format(struct.unpack(">I", struct.pack(">f", value))[0], "x")


def f64_bits(value):
    return format(struct.unpack(">Q", struct.pack(">d", value))[0], "x")


INT_MIN, INT_MAX = -2**31, 2**31 - 1
LONG_MIN, LONG_MAX = -2**63, 2**63 - 1
FLOAT_MAX = 3.4028234663852886e38

NUMERIC_EDGE = [
    "", "0", "-0", "5", "-5", "007", "-007", "2147483647", "2147483648", "-2147483648", "-2147483649",
    "9223372036854775807", "9223372036854775808", "-9223372036854775808", "-9223372036854775809",
    "1.5", ".5", "5.", "-.5", "-5.", "-", ".", "..", "-.", "1.2.3", "1-2", "--1", "- 1", "1 2", " 12", "12abc",
    "12 ", "+5", "1e5", "NaN", "Infinity", "0x10", "١٢", "1" * 40, "1" * 400, "0." + "0" * 400 + "1",
    "340282350000000000000000000000000000000", "340282360000000000000000000000000000000",
    "340282356779733661637539395458142568447", "340282356779733661637539395458142568448",
    "0.000000000000000000000000000000000000000000001", "0.0000000000000000000000000000000000000000000007",
    "0.00000000000000000000000000000000000000000000070064923216240850",
    "1" + "0" * 308, "17976931348623157" + "0" * 292, "17976931348623159" + "0" * 292,
    "0.1", "0.2", "0.3", "100", "1000000", "10000000", "0.001", "0.0001", "123456789.123456789",
]
STRING_EDGE = [
    "", "a", "abc", "hello world", "hello_world-1.2+3", "Ab9_", "\"quoted\"", "'single'", "\"a\\\"b\"",
    "\"a\\\\b\"", "\"a\\nb\"", "\"unterminated", "'unterminated", "\"\"", "''", "\\", "\"\\", "\"a\\",
    "x\"y", "été", "łódź", "😀", "a😀b", "\"😀\"",
    "tab\tsep", "nb sp", " lead", "trail ", "  two  spaces  ", "+-._", "a=b", "@a[b=c]", "\"a b\" c",
]
ALPHA_NUM = list("0123456789.-") + ["e", "x", " ", "+", "f", "٣", "\t"]
ALPHA_STR = list("abcXYZ019_-.+") + ['"', "'", "\\", " ", "\t", "é", "ł", "😀", "=", "@", " ", "\n", "n"]
WS_CHARS = list(" \t\n\r\x0b\x0c\x1c\x1d\x1e\x1f           　᠎​﻿") + ["a", "1"]


def utf16_len(text):
    return len(text.encode("utf-16-le")) // 2


def rand_text(rng, alphabet, max_len):
    return "".join(rng.choice(alphabet) for _ in range(rng.randint(0, max_len)))


def rand_decimal(rng):
    digits = "".join(rng.choice("0123456789") for _ in range(rng.randint(1, 24)))
    if rng.random() < 0.7:
        point = rng.randint(0, len(digits))
        digits = digits[:point] + "." + digits[point:]
    if digits in (".",):
        digits = "0."
    return ("-" if rng.random() < 0.3 else "") + digits


def build_cases():
    rng = random.Random(SEED)
    cases = []

    def add(op, text, **extra):
        case = {"op": op, "input": text}
        case.update(extra)
        cases.append(case)

    def cursor_for(text):
        n = utf16_len(text)
        # cursors on code point boundaries (never inside a surrogate pair)
        offsets, pos = [0], 0
        for ch in text:
            pos += 2 if ord(ch) > 0xFFFF else 1
            offsets.append(pos)
        return rng.choice(offsets) if rng.random() < 0.35 and n else 0

    numeric_pool = NUMERIC_EDGE + [rand_text(rng, ALPHA_NUM, 12) for _ in range(420)] + [rand_decimal(rng) for _ in range(420)]
    for op in ("readInt", "readLong", "readFloat", "readDouble"):
        for text in numeric_pool:
            add(op, text)
        for _ in range(60):
            text = rand_text(rng, ALPHA_NUM, 12)
            add(op, text, cursor=cursor_for(text))

    string_pool = STRING_EDGE + [rand_text(rng, ALPHA_STR, 14) for _ in range(420)]
    for op in ("readUnquotedString", "readQuotedString", "readString", "arg.bool", "readBoolean"):
        for text in string_pool:
            add(op, text)
        for _ in range(60):
            text = rand_text(rng, ALPHA_STR, 14)
            add(op, text, cursor=cursor_for(text))
    for text in ["true", "false", "True", "FALSE", "truex", "true false", "tru", "t", "\"true\"", "'false'", " true", "trueé"]:
        add("readBoolean", text)
        add("arg.bool", text)
    for terminator in ('"', "'", "x"):
        for text in string_pool[:200]:
            add("readStringUntil", text, a=terminator)
    for text in string_pool[:150]:
        for c in ("a", "=", "\"", " "):
            add("expect", text, a=c)
    for _ in range(150):
        add("skipWhitespace", rand_text(rng, WS_CHARS, 12))
    for text in string_pool[:150]:
        add("read", text)
        add("views", text, cursor=cursor_for(text))
    for text in string_pool:
        add("escapeIfRequired", text)
    for kind in ("word", "string", "greedy"):
        for text in string_pool:
            add("arg.string", text, type=kind)

    int_ranges = [(INT_MIN, INT_MAX), (0, 10), (-5, 5), (5, 5), (-100, -10), (INT_MAX - 1, INT_MAX)]
    long_ranges = [(LONG_MIN, LONG_MAX), (0, 10), (-5, 5), (-10**12, 10**12)]
    float_ranges = [(-FLOAT_MAX, FLOAT_MAX), (0.0, 1.0), (-1.0, 1.0), (1e-5, 100.0), (0.0, 1e7), (1e-3, 5e-3)]
    double_ranges = [(-1.7976931348623157e308, 1.7976931348623157e308), (0.0, 1.0), (-1.0, 1.0), (1e-5, 100.0), (0.0, 1e7), (1e-300, 1e300)]
    for text in numeric_pool[:60] + [rand_decimal(rng) for _ in range(160)]:
        for lo, hi in rng.sample(int_ranges, 3):
            add("arg.int", text, min=lo, max=hi)
        for lo, hi in rng.sample(long_ranges, 2):
            add("arg.long", text, min=str(lo), max=str(hi))
        for lo, hi in rng.sample(float_ranges, 3):
            add("arg.float", text, min=f32_bits(lo), max=f32_bits(hi))
        for lo, hi in rng.sample(double_ranges, 3):
            add("arg.double", text, min=f64_bits(lo), max=f64_bits(hi))

    # Java's Double.toString / Float.toString on seeded random bit patterns, boundaries and ties
    special_doubles = [0.0, -0.0, 1.0, 5e-324, 1e23, 2e23, 9007199254740993.0, 1e7, 9999999.999999998, 1e-3,
                       0.001, 0.00099999999999999, 1.7976931348623157e308, 2.2250738585072014e-308, 0.3, 2 / 3, 1 / 3]
    for value in special_doubles:
        add("d2s", f64_bits(value))
    for _ in range(1500):
        add("d2s", format(rng.getrandbits(64), "x"))
    for _ in range(600):
        base = rng.randint(1, 10**rng.randint(1, 15))
        add("d2s", f64_bits(base + rng.choice([0.25, 0.5, 0.75, 0.125, 0.0])))
        add("d2s", f64_bits(float(base) / 2 ** rng.randint(1, 60)))
    special_floats = [0.0, -0.0, 1.0, 1e-45, 1e7, 9999999.0, 1e-3, 3.4028235e38, 1.17549435e-38, 0.1, 16777216.0, 3308588.25]
    for value in special_floats:
        add("f2s", f32_bits(value))
    for _ in range(1500):
        add("f2s", format(rng.getrandbits(32), "x"))
    for _ in range(600):
        base = rng.randint(1, 10**rng.randint(1, 7))
        add("f2s", f32_bits(base + rng.choice([0.25, 0.5, 0.75, 0.125, 0.0])))
        add("f2s", f32_bits(float(base) / 2 ** rng.randint(1, 30)))

    # exhaustive predicates over every BMP non-surrogate code unit
    for code in range(0x10000):
        if 0xD800 <= code <= 0xDFFF:
            continue
        ch = chr(code)
        add("isWhitespace", ch)
        add("isAllowedNumber", ch)
        add("isAllowedInUnquotedString", ch)
        add("isQuotedStringStart", ch)
    return cases


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--oracle-dir", type=Path, required=True)
    parser.add_argument("--out", type=Path, default=ROOT / "fixtures/commands-brigadier-vectors.json")
    args = parser.parse_args()
    oracle = args.oracle_dir.resolve()
    libraries = sorted(str(p) for p in (oracle / "libraries").rglob("*.jar"))
    classpath = ":".join([str(oracle / "server-inner-1.21.jar"), *libraries])
    cases = build_cases()
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        subprocess.run(["javac", "-proc:none", "-cp", classpath, "-d", str(tmp), str(ROOT / "tools/brigadier_oracle.java")], check=True)
        (tmp / "cases.json").write_text(json.dumps(cases))
        subprocess.run(["java", "-cp", f"{classpath}:{tmp}", "cinderstone.oracle.brigadier_oracle", str(tmp / "cases.json"), str(tmp / "out.json")], check=True)
        results = json.loads((tmp / "out.json").read_text())
    assert len(results) == len(cases)
    predicates = {"isWhitespace": [], "isAllowedNumber": [], "isAllowedInUnquotedString": [], "isQuotedStringStart": []}
    kept = []
    skipped_lone_surrogates = 0

    def has_lone_surrogate(value):
        if isinstance(value, str):
            try:
                value.encode("utf-8")
                return False
            except UnicodeEncodeError:
                return True
        if isinstance(value, dict):
            return any(has_lone_surrogate(v) for v in value.values())
        if isinstance(value, list):
            return any(has_lone_surrogate(v) for v in value)
        return False

    for case, result in zip(cases, results):
        if case["op"] in predicates:
            if result["ok"]:
                predicates[case["op"]].append(ord(case["input"]))
            continue
        if has_lone_surrogate(result):
            skipped_lone_surrogates += 1
            continue
        entry = dict(case)
        if "ok" in result:
            entry["ok"] = result["ok"]
        elif "throws" in result:
            entry["throws"] = result["throws"]
        else:
            err = result["err"]
            assert err["input"] == case["input"], "exception input must equal the reader input"
            err.pop("input")
            entry["err"] = err
        entry["cur"] = result["cursor"]
        kept.append(entry)
    document = {
        "format": "brigadier-1.2.9-vectors-v1",
        "source": "unchanged com.mojang.brigadier 1.2.9 from the pinned 1.21 server, with net.minecraft.commands.BrigadierExceptions installed",
        "seed": SEED,
        "skipped_cases_with_lone_surrogate_output": skipped_lone_surrogates,
        "predicates": predicates,
        "cases": kept,
    }
    args.out.write_text(json.dumps(document, separators=(",", ":"), ensure_ascii=True) + "\n")
    print(f"wrote {args.out} ({len(kept)} cases, {args.out.stat().st_size} bytes)")


if __name__ == "__main__":
    sys.exit(main())
