#!/usr/bin/env python3
"""Generate fixtures/component-oracle-vectors.json and fixtures/component-parse-parity.json.

The original `ComponentSerialization.CODEC` (tools/loot_oracle.java `component` mode) decides, for
every input, whether it is accepted, which network NBT bytes it encodes to and which JSON it
re-encodes to. The Rust model in src/components/text.rs must agree.

* vectors: hand-written components that exercise each contents type, style field, number
  narrowing rule, list-collector transition and colour form. Accepted cases record the NBT
  bytes (`nbt`, hex of NbtIo.writeAnyTag) and the stable JSON re-encoding (`json`).
* parity: every vector mutated one step at a time (each member removed, each value replaced by
  nine wrong-typed values, an unknown or foreign `type` added) and accepted/rejected by the
  original. Only the decision is kept.

    python3 tools/component_generate.py --oracle-dir /path/outside/git [--sample 20]
"""
import argparse
import copy
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

COLORS = [
    "black", "dark_blue", "dark_green", "dark_aqua", "dark_red", "dark_purple", "gold", "gray",
    "dark_gray", "blue", "green", "aqua", "red", "light_purple", "yellow", "white",
]
WRONG = ["x", "", 1, 2.5, True, None, [], {}, [1], {"a": 1}]
TYPES = ["text", "translatable", "keybind", "score", "selector", "nbt", "bogus", "minecraft:text", 1, None]


def hand_written():
    cases = [
        "", "hello", "tab\tnew\nline", "unicode ✓ \U0001F600",
        [], ["a"], ["a", "b"], ["a", "b", "c"], [["a", "b"], "c"], ["a", {"text": "b", "bold": True}],
        [{"text": "a", "bold": True}, "b"], [1], [True], [None],
        {"text": ""}, {"text": "x"}, {"text": "x", "extra": []}, {"text": "x", "extra": ["y"]},
        {"text": "x", "extra": ["y", "z"]}, {"text": "x", "extra": [{"text": "y"}, "z"]},
        {"text": "x", "extra": ["y", {"text": "z", "italic": False}]},
        {"text": "x", "extra": [{"text": "y"}, {"text": "z"}]},
        {"text": "x", "extra": [["a", "b"]]},
        {"text": "x", "extra": "y"}, {"text": "x", "extra": None}, {"text": "x", "extra": 1},
        {"text": 1}, {"text": True}, {"text": None}, {"text": ["a"]},
        {"extra": ["a"]}, {}, {"unknown": 1}, {"text": "x", "unknown": 1},
    ]
    for color in COLORS + [
        "#FF00aa", "#000000", "#FFFFFF", "#fff", "#0", "#00000000F", "#1000000", "#-1", "#+FF", "#",
        "#G", "#-0", "#7FFFFFFF", "#80000000", "#FFFFFFFF", "RED", "red ", "", "reset", "#ff 00",
    ]:
        cases.append({"text": "c", "color": color})
    cases += [{"text": "c", "color": 1}, {"text": "c", "color": None}, {"text": "c", "color": True}]
    for flag in ["bold", "italic", "underlined", "strikethrough", "obfuscated"]:
        for value in [True, False, 0, 1, 2, -1, 1.5, 0.4, "true", "x", None, [], {}, 256, 1e30]:
            cases.append({"text": "f", flag: value})
    cases += [
        {"text": "f", "bold": True, "italic": True, "underlined": True, "strikethrough": True,
         "obfuscated": True, "color": "gold", "insertion": "i", "font": "minecraft:uniform"},
        {"text": "f", "insertion": ""}, {"text": "f", "insertion": 1}, {"text": "f", "insertion": None},
    ]
    for font in ["minecraft:default", "uniform", "mod:my/font", "BAD", "a:B", "a b", ":x", "x:", "", "a:b:c", 1, None]:
        cases.append({"text": "f", "font": font})
    for action, value in [
        ("open_url", "https://example.com"), ("open_url", "not a url"), ("run_command", "/say hi"),
        ("suggest_command", "/tp"), ("change_page", "3"), ("change_page", "x"), ("copy_to_clipboard", "text"),
        ("open_file", "C:/x"), ("bogus", "x"), ("OPEN_URL", "https://x"), ("run_command", ""),
    ]:
        cases.append({"text": "c", "clickEvent": {"action": action, "value": value}})
    cases += [
        {"text": "c", "clickEvent": {"action": "open_url"}}, {"text": "c", "clickEvent": {"value": "x"}},
        {"text": "c", "clickEvent": {"action": "open_url", "value": 1}}, {"text": "c", "clickEvent": "x"},
        {"text": "c", "clickEvent": None}, {"text": "c", "clickEvent": {}},
        {"text": "c", "clickEvent": {"action": "run_command", "value": "/a", "extra": 1}},
    ]
    cases += [
        {"text": "h", "hoverEvent": {"action": "show_text", "contents": "tip"}},
        {"text": "h", "hoverEvent": {"action": "show_text", "contents": {"text": "tip", "color": "red"}}},
        {"text": "h", "hoverEvent": {"action": "show_text", "contents": ["a", "b"]}},
        {"text": "h", "hoverEvent": {"action": "show_text", "value": "legacy"}},
        {"text": "h", "hoverEvent": {"action": "show_text", "value": {"text": "legacy"}}},
        {"text": "h", "hoverEvent": {"action": "show_text", "contents": "new", "value": "old"}},
        {"text": "h", "hoverEvent": {"action": "show_text", "contents": None, "value": "old"}},
        {"text": "h", "hoverEvent": {"action": "show_text"}}, {"text": "h", "hoverEvent": {"action": "show_text", "contents": 1}},
        {"text": "h", "hoverEvent": {"action": "show_text", "contents": {"text": "t", "hoverEvent": {"action": "show_text", "contents": "deep"}}}},
        {"text": "h", "hoverEvent": {"action": "show_item", "contents": {"id": "minecraft:stone", "count": 2}}},
        {"text": "h", "hoverEvent": {"action": "show_item", "contents": "minecraft:stone"}},
        {"text": "h", "hoverEvent": {"action": "show_item", "contents": {"id": "minecraft:stone", "count": 2, "components": {"minecraft:damage": 3}}}},
        {"text": "h", "hoverEvent": {"action": "show_item", "value": "{id:\"minecraft:stone\"}"}},
        {"text": "h", "hoverEvent": {"action": "show_item", "contents": {"id": "minecraft:nothing"}}},
        {"text": "h", "hoverEvent": {"action": "show_entity", "contents": {"type": "minecraft:pig", "id": "00000000-0000-0000-0000-000000000001"}}},
        {"text": "h", "hoverEvent": {"action": "show_entity", "contents": {"type": "minecraft:pig", "id": [0, 0, 0, 1], "name": "p"}}},
        {"text": "h", "hoverEvent": {"action": "show_entity", "contents": {"type": "minecraft:pig"}}},
        {"text": "h", "hoverEvent": {"action": "bogus", "contents": "x"}}, {"text": "h", "hoverEvent": {"contents": "x"}},
        {"text": "h", "hoverEvent": "x"}, {"text": "h", "hoverEvent": None}, {"text": "h", "hoverEvent": {}},
    ]
    cases += [
        {"translate": "k"}, {"translate": ""}, {"translate": "k", "fallback": "fb"},
        {"translate": "k", "fallback": 1}, {"translate": "k", "fallback": None}, {"translate": "k", "fallback": ""},
        {"translate": "k", "with": []}, {"translate": "k", "with": None}, {"translate": "k", "with": "a"},
        {"translate": "k", "with": {"text": "a"}}, {"translate": "k", "with": 1},
        {"translate": 1}, {"translate": None}, {"translate": ["a"]},
        {"translate": "k", "with": ["a"]}, {"translate": "k", "with": ["a", "b"]},
        {"translate": "k", "with": [{"text": "q"}]}, {"translate": "k", "with": [{"text": "q", "bold": True}]},
        {"translate": "k", "with": [{"text": "q"}, "a"]}, {"translate": "k", "with": ["a", {"text": "q", "bold": True}]},
        {"translate": "k", "with": [["a", "b"]]}, {"translate": "k", "with": [["a"], "b"]},
        {"translate": "k", "with": [True]}, {"translate": "k", "with": [True, False]}, {"translate": "k", "with": [True, "a"]},
        {"translate": "k", "with": [None]}, {"translate": "k", "with": [[]]}, {"translate": "k", "with": [{}]},
        {"translate": "k", "with": [{"text": "a", "extra": ["b"]}]},
        {"translate": "k", "with": [{"translate": "inner", "with": [1, 2]}]},
        {"translate": "k", "with": [{"translate": "inner", "with": [1.5]}, "x"]},
        {"translate": "k", "with": [{"text": "q"}, {"text": "r", "bold": True}]},
        {"translate": "k", "with": [{"": "wrapped"}]},
        {"translate": "k", "with": [{"text": "q", "extra": [{"": "x"}]}]},
        {"translate": "k", "fallback": "f", "with": ["a"], "extra": ["e"], "color": "red"},
    ]
    for number in [
        0, 1, -1, 127, 128, -128, -129, 255, 256, 32767, 32768, -32768, -32769, 65535, 2147483647, 2147483648,
        -2147483648, -2147483649, 4294967296, 9007199254740992, 9223372036854775807, 9223372036854775808,
        -9223372036854775808, 18446744073709551615, 18446744073709551616,
        0.0, -0.0, 1.0, -1.0, 100.0, 1e2, 1.5, -1.5, 0.5, 0.25, 0.1, 0.2, 0.3, 1 / 3, 3.14159, 1e10, 1.5e300,
        1e-5, 1e-300, 16777216.0, 16777217.0, 3.4028234663852886e38, 3.5e38, 1e39, 1.401298464324817e-45, 123456789.5,
    ]:
        cases.append({"translate": "n", "with": [number]})
    for run in [[1, 2], [1, 2, 3], [100, 200], [1000, 2000], [1, 1000], [1000, 1], [1, 1.5], [1.5, 2.5], [1, "a"], ["a", 1],
                [1, True], [True, 1], [1, 2, "a"], [1.5, 1], [2 ** 40, 1], [1, 2 ** 40], [2 ** 40, 2 ** 40], [1000000, 1000000],
                [1000000, 1], [1, 1000000], [0.5, "a"], [True, True, True], [True, 1000], [1, [1]], [[1], 1]]:
        cases.append({"translate": "n", "with": run})
    cases += [
        {"keybind": "key.jump"}, {"keybind": ""}, {"keybind": 1}, {"keybind": None}, {"keybind": "key.jump", "extra": ["x"]},
        {"score": {"name": "@p", "objective": "o"}}, {"score": {"name": "Steve", "objective": "kills"}},
        {"score": {"name": "@p"}}, {"score": {"objective": "o"}}, {"score": {}}, {"score": "x"}, {"score": None},
        {"score": {"name": "@p", "objective": "o", "value": "7"}}, {"score": {"name": 1, "objective": "o"}},
        {"selector": "@e"}, {"selector": "@e[type=pig]"}, {"selector": ""}, {"selector": 1},
        {"selector": "@e", "separator": {"text": ","}}, {"selector": "@e", "separator": ", "},
        {"selector": "@e", "separator": []}, {"selector": "@e", "separator": None}, {"selector": "@e", "separator": 1},
        {"selector": "@e", "separator": ["a", "b"]},
        {"nbt": "a.b", "entity": "@s"}, {"nbt": "a.b", "block": "1 2 3"}, {"nbt": "a.b", "storage": "minecraft:x"},
        {"nbt": "a.b", "storage": "x"}, {"nbt": "a.b", "storage": "BAD"}, {"nbt": "a.b", "storage": "a:b:c"},
        {"nbt": "a.b", "storage": ""}, {"nbt": "a.b", "storage": 1}, {"nbt": "a.b"}, {"nbt": 1, "entity": "@s"},
        {"nbt": "a", "entity": "@s", "interpret": True}, {"nbt": "a", "entity": "@s", "interpret": False},
        {"nbt": "a", "entity": "@s", "interpret": 1}, {"nbt": "a", "entity": "@s", "interpret": "yes"},
        {"nbt": "a", "entity": "@s", "interpret": None}, {"nbt": "a", "entity": "@s", "interpret": 2.5},
        {"nbt": "a", "entity": "@s", "separator": ","}, {"nbt": "a", "entity": "@s", "separator": {"text": "-", "bold": True}},
        {"nbt": "a", "entity": "@s", "separator": 1}, {"nbt": "a", "entity": "@s", "separator": []},
        {"nbt": "a", "entity": "@s", "separator": {"bad": 1}},
        {"nbt": "a", "entity": "@s", "block": "1 1 1"}, {"nbt": "a", "block": "1 1 1", "storage": "x:y"},
        {"nbt": "a", "storage": "x:y", "source": "storage"}, {"nbt": "a", "entity": "@s", "source": "entity"},
        {"nbt": "a", "block": "1 2 3", "source": "block"}, {"nbt": "a", "entity": "@s", "source": "block"},
        {"nbt": "a", "storage": "x:y", "source": "entity"}, {"nbt": "a", "entity": "@s", "source": "bogus"},
        {"nbt": "a", "entity": "@s", "source": 1}, {"nbt": "a", "entity": "@s", "source": None},
        {"text": "t", "translate": "k"}, {"translate": "k", "text": "t"}, {"text": "t", "keybind": "key.jump"},
        {"keybind": "key.jump", "score": {"name": "a", "objective": "b"}}, {"selector": "@e", "nbt": "a", "entity": "@s"},
        {"score": {"name": "a", "objective": "b"}, "selector": "@e"}, {"translate": "k", "nbt": "a", "entity": "@s"},
        {"text": 1, "translate": "k"}, {"text": None, "translate": "k"}, {"translate": 1, "keybind": "key.jump"},
        {"text": "t", "type": "text"}, {"text": "t", "type": "translatable"}, {"translate": "k", "type": "translatable"},
        {"translate": "k", "type": "text"}, {"keybind": "key.jump", "type": "keybind"}, {"selector": "@e", "type": "selector"},
        {"score": {"name": "a", "objective": "b"}, "type": "score"}, {"nbt": "a", "entity": "@s", "type": "nbt"},
        {"nbt": "a", "entity": "@s", "type": "nbt", "source": "entity"}, {"nbt": "a", "storage": "x:y", "type": "nbt", "source": "storage"},
        {"nbt": "a", "storage": "x:y", "type": "nbt", "source": "entity"}, {"text": "t", "type": "bogus"},
        {"text": "t", "type": "minecraft:text"}, {"text": "t", "type": 1}, {"text": "t", "type": None}, {"text": "t", "type": ["text"]},
        {"type": "text"}, {"type": "translatable"}, {"type": "nbt", "nbt": "a"}, {"type": "object"},
        {"type": "text", "text": "t", "extra": ["x"], "bold": True},
    ]
    return cases


STYLE_MEMBERS = {
    "color": "gold", "bold": True, "italic": False, "underlined": True, "strikethrough": True, "obfuscated": False,
    "clickEvent": {"action": "run_command", "value": "/x"},
    "hoverEvent": {"action": "show_text", "contents": {"text": "tip", "color": "red"}},
    "insertion": "ins", "font": "minecraft:alt",
}
CONTENTS = [
    {"text": "t"}, {"translate": "k"}, {"translate": "k", "fallback": "f"}, {"translate": "k", "with": ["a", 1]},
    {"translate": "k", "fallback": "f", "with": [{"text": "q", "bold": True}, "b"]}, {"keybind": "key.jump"},
    {"score": {"name": "@p", "objective": "o"}}, {"selector": "@e"}, {"selector": "@e", "separator": {"text": ",", "bold": True}},
    {"nbt": "p", "entity": "@s"}, {"nbt": "p", "block": "1 2 3", "interpret": True}, {"nbt": "p", "storage": "a:b"},
    {"nbt": "p", "storage": "a:b", "interpret": True, "separator": {"text": ";", "italic": True}},
]


def combinations_block():
    """Every content type with every pair of style members, and seeded larger subsets, with and
    without `extra`: these are the compounds whose key order depends on HashMap bucket collisions."""
    import itertools
    import random
    rng = random.Random(1021)
    names = list(STYLE_MEMBERS)
    subsets = [()] + [(a,) for a in names] + list(itertools.combinations(names, 2))
    subsets += [tuple(sorted(rng.sample(names, rng.randint(3, 10)), key=names.index)) for _ in range(12)]
    out = []
    for contents in CONTENTS:
        for position, subset in enumerate(subsets):
            # `extra` joins the bucket-collision groups: every size-0/1 subset and the random ones carry it too.
            variants = (None, ["e1", {"text": "e2", "bold": True}]) if len(subset) <= 1 or position >= len(subsets) - 12 else (None,)
            for extra in variants:
                case = dict(contents)
                case.update({name: STYLE_MEMBERS[name] for name in subset})
                if extra:
                    case["extra"] = extra
                out.append(case)
    return out


def walk(node, path=()):
    yield path, node
    if isinstance(node, dict):
        for key, value in node.items():
            yield from walk(value, path + (key,))
    elif isinstance(node, list):
        for index, value in enumerate(node):
            yield from walk(value, path + (index,))


def replace_at(root, path, value, remove=False):
    root = copy.deepcopy(root)
    if not path:
        return value
    parent = root
    for step in path[:-1]:
        parent = parent[step]
    if remove:
        if isinstance(parent, dict):
            del parent[path[-1]]
        else:
            del parent[path[-1]]
    else:
        parent[path[-1]] = value
    return root


def mutants(case):
    out = []
    for path, node in walk(case):
        if path:
            out.append(replace_at(case, path, None, remove=True))
        for wrong in WRONG:
            if wrong != node:
                out.append(replace_at(case, path, wrong))
        if isinstance(node, dict) and "type" not in node:
            for kind in TYPES:
                mutated = copy.deepcopy(node)
                mutated["type"] = kind
                out.append(replace_at(case, path, mutated))
    return out


def run_oracle(oracle_dir, work, cases):
    work.mkdir(parents=True, exist_ok=True)
    (work / "component-in.json").write_text(json.dumps(cases))
    classpath = (oracle_dir / "cp.txt").read_text().strip()
    subprocess.run(
        ["java", "--add-exports", "java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED",
         "-cp", f"{oracle_dir}:{classpath}", "loot_oracle", "component",
         str(work / "component-in.json"), str(work / "component-out.json")],
        check=True,
    )
    return json.loads((work / "component-out.json").read_text())


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--oracle-dir", type=Path, required=True)
    parser.add_argument("--work", type=Path, default=Path("/tmp"))
    parser.add_argument("--sample", type=int, default=20, help="keep every Nth mutant in the committed fixture (1 keeps all)")
    args = parser.parse_args()

    base = []
    seen = set()
    for case in hand_written() + combinations_block():
        key = json.dumps(case, sort_keys=True)
        if key not in seen:
            seen.add(key)
            base.append(case)

    generated = []
    for case in base:
        for mutant in mutants(case):
            key = json.dumps(mutant, sort_keys=True)
            if key not in seen:
                seen.add(key)
                generated.append(mutant)

    results = run_oracle(args.oracle_dir, args.work, base + generated)
    base_results, mutant_results = results[:len(base)], results[len(base):]

    vectors = []
    for case, result in zip(base, base_results):
        entry = {"j": case, "ok": result["ok"]}
        if result["ok"]:
            entry["nbt"] = result["nbt"]
            entry["json"] = result["json"]
        else:
            entry["err"] = result.get("error", "")
        vectors.append(entry)
    source = {
        "server_jar_sha1": "450698d1863ab5180c25d7c804ef0fe6369dd1ba",
        "inner_jar_sha256": "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01",
        "mappings_sha1": "31c77994d96f05ba25a870ada70f47f315330437",
        "oracle": "tools/loot_oracle.java component (ComponentSerialization.CODEC over JsonOps/NbtOps with the vanilla registries)",
    }
    (ROOT / "fixtures/component-oracle-vectors.json").write_text(json.dumps({
        "format": "cinderstone-component-oracle-v1", "minecraft": "1.21", "source": source,
        "result": "ok; nbt = hex of NbtIo.writeAnyTag of the CODEC's NbtOps encoding; json = GsonHelper.toStableString of its JsonOps encoding",
        "cases": vectors,
    }, separators=(",", ":"), ensure_ascii=False))

    kept = [(m, r["ok"], r.get("nbt")) for index, (m, r) in enumerate(zip(generated, mutant_results)) if index % args.sample == 0]
    accepted = sum(1 for r in mutant_results if r["ok"])
    (ROOT / "fixtures/component-parse-parity.json").write_text(json.dumps({
        "format": "cinderstone-component-parse-parity-v1", "minecraft": "1.21", "source": source,
        "sampling": f"every {args.sample}th of {len(generated)} generated mutants",
        "result": "[input, accepted, nbt hex when accepted]",
        "cases": [[m, ok, nbt] if ok else [m, ok] for m, ok, nbt in kept],
    }, separators=(",", ":"), ensure_ascii=False))
    print(f"vectors={len(vectors)} (accepted {sum(1 for v in vectors if v['ok'])}) "
          f"mutants={len(generated)} accepted={accepted} rejected={len(generated) - accepted} kept={len(kept)}")


if __name__ == "__main__":
    main()
