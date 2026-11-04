#!/usr/bin/env python3
"""Generate fixtures/commands-brigadier-suggest-vectors.json by EXECUTING the original Brigadier.

    python3 tools/bootstrap_oracle.py --oracle-dir /path/outside/git
    python3 tools/brigadier_suggest_generate.py --oracle-dir /path/outside/git

WP-E3 stage 4 (completion suggestions). Two kinds of evidence, both produced by the unchanged
com.mojang.brigadier 1.2.9 classes bundled with the pinned Java 1.21 server (the same oracle
class as tools/brigadier_dispatch_generate.py, which also builds the random trees used here):

* seeded random command trees whose argument nodes carry custom suggestion providers, with
  inputs (valid paths, truncations, case changes, trailing spaces) completed at several
  cursors, including cursors past the end of the input;
* direct tests of Suggestions.create/merge, SuggestionsBuilder and the Java string semantics
  they depend on (String.compareToIgnoreCase, hashCode, toLowerCase(Locale.ROOT)) plus the
  complete Character.toUpperCase/toLowerCase table of the BMP.

The jar and libraries are proprietary downloads and stay outside git.
"""
import argparse
import json
import random
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import brigadier_dispatch_generate as dispatch  # noqa: E402  (tree generator shared with stage 2)

ROOT = Path(__file__).resolve().parents[1]
SEED = 20261104

# Extra literals: case variants and non-ASCII letters whose lower-casing is not one-to-one.
dispatch.LIT_WORDS = dispatch.LIT_WORDS + ["Foo", "FOO", "Bar", "ÉTÉ", "Été", "İstanbul", "ΑΣ", "ß"]

SUGGEST_WORDS = ["foo", "Foo", "FOO", "foobar", "bar", "Bar", "été", "ÉTÉ", "Été", "a", "A", "ab", "AB", "abc",
                 "x", "İstanbul", "ΑΣ", "ας", "straße", "STRASSE", "😀", "😀a", "a b", "", "0", "10", "9", "Ab-c"]
STRING_POOL = ["", "a", "A", "ab", "AB", "aB", "abc", "ABC", "Abd", "été", "ÉTÉ", "Été", "ß", "SS", "ss", "straße",
               "STRASSE", "İ", "i̇", "I", "i", "ı", "ΑΣ", "ας", "ΑΣΑ", "σ", "Σ", "ς", "ǅ", "ǆ", "Ǆ", "ᾳ", "ᾼ", "ΑΙ",
               "😀", "😀a", "𐐀", "𐐨", "𐐀a", "ǰ", "ﬃ", "FFI", "µ", "Μ", "μ", "K", "k", "K", "ſ", "s", "S",
               "10", "9", "a b", "a  b", "-", "Ab-c", "İ", "ẞ"]
# Capital sigma is left out of the random alphabet on purpose: java.lang.String lower-cases it to
# a final sigma using its legacy word BreakIterator (`AΣ-É` keeps a medial sigma), which differs from
# the Unicode Final_Sigma rule used by the Rust port next to mid-word punctuation. STRING_POOL keeps
# sigma only where both rules agree (end of word, or followed by a letter).
BMP_ALPHABET = "abAB1- éÉİσς"


def decorate(rng, node):
    if node["t"] == "arg" and rng.random() < 0.45:
        kind = rng.choices(["words", "filter", "ctx", "throw"], [20, 50, 20, 10])[0]
        sug = {"kind": kind}
        if kind in ("words", "filter"):
            sug["words"] = rng.sample(SUGGEST_WORDS, rng.randint(1, 7))
        node["sug"] = sug
    for child in node.get("children", []):
        decorate(rng, child)


def utf16_len(text):
    return len(text.encode("utf-16-le")) // 2


def boundaries(text):
    """UTF-16 offsets that do not split a surrogate pair."""
    offsets, position = [0], 0
    for ch in text:
        position += 2 if ord(ch) > 0xFFFF else 1
        offsets.append(position)
    return offsets


def sample_suggest_input(rng, tree):
    text = dispatch.sample_input(rng, tree)
    roll = rng.random()
    if roll < 0.40 and text:
        text = text[: rng.randint(0, len(text))]
    elif roll < 0.55:
        text += " "
    elif roll < 0.67:
        text = text.upper()
    elif roll < 0.72:
        text = text.lower()
    return text


def build_tree_tests(rng):
    tests = []
    for _ in range(70):
        tree = dispatch.gen_tree(rng)
        for node in tree:
            decorate(rng, node)
        inputs = []
        for _ in range(30):
            text = sample_suggest_input(rng, tree)
            offsets = boundaries(text)
            cursors = [offsets[-1]]
            for _ in range(rng.choice([0, 1, 1, 2])):
                cursors.append(rng.choice(offsets))
            if rng.random() < 0.04:
                cursors.append(offsets[-1] + 1)  # past the end: the original throws
            inputs.append({"input": text, "source": rng.choice([2, 3, 4, 4, 1, 0]), "cursors": cursors})
        tests.append({"tree": tree, "inputs": inputs})
    return tests


def rand_text(rng, alphabet, low, high):
    return "".join(rng.choice(alphabet) for _ in range(rng.randint(low, high)))


def suggestion_specs(rng, command, count, same_range=False, integers=False):
    length = len(command)
    shared = sorted(rng.sample(range(length + 1), 2)) if length > 0 and rng.random() < 0.7 else [0, length]
    specs = []
    for _ in range(count):
        if same_range:
            start, end = shared
        else:
            start, end = sorted((rng.randint(0, length), rng.randint(0, length)))
        spec = {"r": [start, end]}
        if integers:
            spec["i"] = rng.choice([0, 1, 9, 10, 11, 100, -1, -10, 2**31 - 1, -2**31, rng.randint(-50, 50)])
            spec["t"] = str(spec["i"])
        else:
            spec["t"] = rng.choice(STRING_POOL[:30] + ["x" + rand_text(rng, "abAB", 0, 3)])
        specs.append(spec)
    return specs


def java_string_hash(text):
    h = 0
    for unit in memoryview(text.encode("utf-16-le")).cast("H"):
        h = (31 * h + unit) & 0xFFFFFFFF
    return h


def suggestion_hash(start, end, text):
    """Suggestion.hashCode() = Objects.hash(range, text, null) with Objects.hash(start, end) for the range."""
    range_hash = (31 * (31 + start) + end) & 0xFFFFFFFF
    h = 1
    h = (31 * h + range_hash) & 0xFFFFFFFF
    h = (31 * h + java_string_hash(text)) & 0xFFFFFFFF
    return (31 * h) & 0xFFFFFFFF


def spread(h):
    return h ^ (h >> 16)


def case_variants(word):
    variants = []
    for mask in range(1 << len(word)):
        variants.append("".join(c.upper() if mask >> i & 1 else c for i, c in enumerate(word)))
    return variants


def tie_cases(rng):
    """Suggestions that all compare equal ignoring case, so their output order is the HashSet's.

    * chains: nine or more variants sharing one bucket of the initial 16-slot table, which makes the
      original grow the table (treeifyBin below 64 slots) before the load factor would;
    * growth: larger sets that pass the 12, 24 and 48 element load-factor thresholds.
    """
    ops = []
    for word in ["abcdefgh", "xyzwvuts", "aeioumnl"]:
        variants = case_variants(word)
        buckets = {}
        for variant in variants:
            buckets.setdefault(spread(suggestion_hash(0, 0, variant)) & 15, []).append(variant)
        for bucket in sorted(buckets.values(), key=len, reverse=True)[:2]:
            for count in (8, 9, 10, 12):
                if len(bucket) >= count:
                    chosen = rng.sample(bucket, count)
                    ops.append({"op": "create", "command": "", "list": [{"r": [0, 0], "t": t} for t in chosen]})
    for count in (13, 14, 20, 25, 30, 40, 55):
        for word in ["abcdefgh", "qrstuvwx"]:
            chosen = rng.sample(case_variants(word), count)
            ops.append({"op": "create", "command": "", "list": [{"r": [0, 0], "t": t} for t in chosen]})
            half = len(chosen) // 2
            ops.append({"op": "merge", "command": "", "groups": [[{"r": [0, 0], "t": t} for t in chosen[:half]],
                                                                  [{"r": [0, 0], "t": t} for t in chosen[half:]]]})
    return ops


def build_direct(rng):
    ops = tie_cases(rng)
    for _ in range(450):
        command = rand_text(rng, BMP_ALPHABET, 0, 9)
        integers = rng.random() < 0.15
        specs = suggestion_specs(rng, command, rng.randint(0, 9), same_range=integers or rng.random() < 0.3, integers=integers)
        ops.append({"op": "create", "command": command, "list": specs})
    for _ in range(250):
        command = rand_text(rng, BMP_ALPHABET, 0, 9)
        groups = [suggestion_specs(rng, command, rng.randint(0, 6)) for _ in range(rng.randint(0, 4))]
        ops.append({"op": "merge", "command": command, "groups": groups})
    for _ in range(350):
        text = rand_text(rng, BMP_ALPHABET, 0, 8)
        start = rng.choice([0, len(text), rng.randint(0, len(text)), rng.randint(0, len(text)), len(text) + 1])
        integers = rng.random() < 0.15
        steps = []
        for _ in range(rng.randint(0, 8)):
            roll = rng.random()
            if roll < 0.08:
                steps.append({"off": rng.randint(0, len(text))})
            elif roll < 0.14:
                steps.append({"restart": True})
            elif integers:
                steps.append({"i": rng.choice([0, 1, 9, 10, -3, 2**31 - 1, -2**31])})
            else:
                steps.append({"s": rng.choice(STRING_POOL[:30] + [text[start:] if start <= len(text) else ""])})
        ops.append({"op": "builder", "input": text, "start": start, "steps": steps})
    for a in STRING_POOL:
        for b in rng.sample(STRING_POOL, 12):
            ops.append({"op": "string", "a": a, "b": b})
    ops.append({"op": "charCase"})
    return ops


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


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--oracle-dir", type=Path, required=True)
    parser.add_argument("--out", type=Path, default=ROOT / "fixtures/commands-brigadier-suggest-vectors.json")
    args = parser.parse_args()
    oracle = args.oracle_dir.resolve()
    libraries = sorted(str(p) for p in (oracle / "libraries").rglob("*.jar"))
    classpath = ":".join([str(oracle / "server-inner-1.21.jar"), *libraries])
    rng = random.Random(SEED)
    tree_tests = build_tree_tests(rng)
    direct = build_direct(rng)
    entries = [*tree_tests, {"direct": direct}]
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        subprocess.run(["javac", "-proc:none", "-Xlint:none", "-cp", classpath, "-d", str(tmp), str(ROOT / "tools/brigadier_dispatch_oracle.java")], check=True)
        (tmp / "cases.json").write_text(json.dumps(entries))
        subprocess.run(["java", "-cp", f"{classpath}:{tmp}", "cinderstone.oracle.brigadier_dispatch_oracle", str(tmp / "cases.json"), str(tmp / "out.json")], check=True, timeout=300)
        outputs = json.loads((tmp / "out.json").read_text())
    assert len(outputs) == len(entries)

    kept_trees, skipped, calls, throws, nonempty = [], 0, 0, 0, 0
    for test, output in zip(tree_tests, outputs):
        cases = []
        for entry, result in zip(test["inputs"], output["results"]):
            suggest = result["suggest"]
            if has_lone_surrogate(suggest):
                skipped += 1
                continue
            cases.append({"input": entry["input"], "source": entry["source"], "suggest": suggest})
            for item in suggest:
                calls += 1
                throws += 1 if "throws" in item else 0
                nonempty += 1 if item.get("result", {}).get("list") else 0
        kept_trees.append({"tree": test["tree"], "cases": cases})
    direct_cases = []
    lone_direct = 0
    for op, result in zip(direct, outputs[-1]["results"]):
        if has_lone_surrogate(result):
            lone_direct += 1
            continue
        direct_cases.append({"op": op, "result": result})
    document = {
        "format": "brigadier-1.2.9-suggestion-vectors-v1",
        "source": "unchanged com.mojang.brigadier 1.2.9 from the pinned 1.21 server, with net.minecraft.commands.BrigadierExceptions installed",
        "seed": SEED,
        "skipped_tree_cases_with_lone_surrogate_output": skipped,
        "skipped_direct_cases_with_lone_surrogate_output": lone_direct,
        "tests": kept_trees,
        "direct": direct_cases,
    }
    args.out.write_text(json.dumps(document, separators=(",", ":"), ensure_ascii=True) + "\n")
    print(f"wrote {args.out} ({len(kept_trees)} trees, {calls} completions of which {nonempty} non-empty and {throws} throwing, "
          f"{len(direct_cases)} direct cases, {args.out.stat().st_size} bytes)")


if __name__ == "__main__":
    sys.exit(main())
