#!/usr/bin/env python3
"""Generate fixtures/loot-parse-parity.json: codec accept/reject parity with the original.

Every distinct condition, function, entry and number-provider fragment of the shipped
tables (plus the hand-written ones from loot_generate.py) is mutated one step at a time:
each key removed, each value replaced by a wrong-typed value, and an unknown `type`
substituted. The original LootTable.DIRECT_CODEC decides whether each mutant is accepted;
the Rust parser must decide identically. Usage mirrors loot_generate.py:

    python3 tools/loot_parse_parity.py --oracle-dir /path/outside/git
"""
import argparse
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools"))
import loot_generate as generator  # noqa: E402

TABLES = generator.TABLES
WRONG = ["x", 1, 2.5, True, None, [], {}, [1], {"a": 1}]


def condition_table(condition):
    return {"type": "minecraft:chest", "pools": [{"rolls": 1, "entries": [], "conditions": [condition]}]}


# Hand-written edge cases (registry validation, tag existence, inline forms, id syntax).
HAND_WRITTEN = [
    condition_table(c) for c in [
        {"condition": "minecraft:block_state_property", "block": "minecraft:wheat", "properties": {"age": "7"}},
        {"condition": "minecraft:block_state_property", "block": "minecraft:wheat", "properties": {"nonexistent": "1"}},
        {"condition": "minecraft:block_state_property", "block": "minecraft:wheat", "properties": {"age": {"min": "2"}}},
        {"condition": "minecraft:block_state_property", "block": "minecraft:oak_stairs", "properties": {"half": "top", "facing": "west"}},
        {"condition": "minecraft:block_state_property", "block": "minecraft:oak_stairs", "properties": {"age": "1"}},
        {"condition": "minecraft:match_tool", "predicate": {"items": "minecraft:not_an_item"}},
        {"condition": "minecraft:match_tool", "predicate": {"items": ["minecraft:stick", "#minecraft:swords"]}},
        {"condition": "minecraft:match_tool", "predicate": {"items": "#minecraft:no_such_tag"}},
        {"condition": "minecraft:match_tool", "predicate": {"items": "#minecraft:swords"}},
        {"condition": "minecraft:match_tool", "predicate": {"predicates": {"minecraft:no_such": []}}},
        {"condition": "minecraft:entity_properties", "entity": "this", "predicate": {"type": "minecraft:not_an_entity"}},
        {"condition": "minecraft:location_check", "predicate": {"biomes": "minecraft:not_a_biome"}},
        {"condition": "minecraft:location_check", "predicate": {"biomes": "#minecraft:is_ocean"}},
        {"condition": "minecraft:location_check", "predicate": {"dimension": "Bad Dim"}},
        {"condition": "minecraft:entity_properties", "entity": "nobody"},
        {"condition": "minecraft:random_chance", "chance": {"type": "minecraft:score", "target": "this", "score": "s"}},
        {"condition": "minecraft:random_chance", "chance": {"type": "minecraft:storage", "storage": "a:b", "path": "x.y"}},
        {"condition": "minecraft:random_chance", "chance": {"type": "minecraft:score", "target": {"type": "minecraft:fixed", "name": "n"}, "score": "s", "scale": "bad"}},
    ]
] + [
    {"type": "minecraft:chest", "random_sequence": "Bad Seq", "pools": []},
    {"type": "minecraft:no_such_type", "pools": []},
    {"pools": [{"rolls": 1, "entries": [{"type": "minecraft:item", "name": "minecraft:stick", "weight": 1.5}]}]},
    {"pools": [{"rolls": 1, "entries": [{"type": "minecraft:item", "name": "stick"}]}]},
    {"pools": [{"rolls": 1, "entries": [{"type": "item", "name": "minecraft:stick"}]}]},
    {"pools": [{"rolls": 1, "entries": [{"type": "minecraft:tag", "name": "minecraft:logs"}]}]},
    {"pools": [{"rolls": 1, "entries": [{"type": "minecraft:tag", "name": "logs", "expand": True}]}]},
]


def pool(**extra):
    return {"type": "minecraft:chest", "pools": [{"rolls": 1, "entries": [], **extra}]}


WRAPPERS = {
    "pool": lambda f: {"type": "minecraft:chest", "pools": [f]},
    "table": lambda f: f,
    "condition": lambda f: pool(conditions=[f]),
    "function": lambda f: pool(functions=[f]),
    "entry": lambda f: {"type": "minecraft:chest", "pools": [{"rolls": 1, "entries": [f]}]},
    "provider": lambda f: {"type": "minecraft:chest", "pools": [{"rolls": f, "entries": []}]},
}


def fragments():
    seen = {}

    def add(kind, node):
        key = (kind, json.dumps(node, sort_keys=True))
        seen.setdefault(key, node)

    def walk(node, kind=None):
        if isinstance(node, dict):
            if "condition" in node:
                add("condition", node)
            if "function" in node:
                add("function", node)
            if isinstance(node.get("type"), str) and node["type"].startswith("minecraft:") and kind == "entries":
                add("entry", node)
            for key, value in node.items():
                walk(value, "entries" if key in ("entries", "children") else None)
        elif isinstance(node, list):
            for item in node:
                walk(item, kind)

    generator.synthetic_cases()
    for table in list(TABLES.values()) + generator.CUSTOM_TABLES:
        walk(table)
        # table and pool skeletons: the keys that are not conditions/functions/entries
        add("table", {k: v for k, v in table.items() if k not in ("pools", "functions")} | {"pools": [], "functions": []})
        for pool_node in table.get("pools", []):
            add("pool", {k: v for k, v in pool_node.items() if k not in ("entries", "conditions", "functions")}
                | {"entries": [], "conditions": [], "functions": []})
    return list(seen.items())


def mutants(node):
    """One-step mutations of a JSON object, recursing one nested object deep."""
    out = []
    if not isinstance(node, dict):
        return out
    for key in node:
        reduced = {k: v for k, v in node.items() if k != key}
        out.append(reduced)
        for wrong in WRONG:
            if wrong != node[key]:
                out.append({**node, key: wrong})
    for key in ("type", "condition", "function"):
        if key in node:
            out.append({**node, key: "minecraft:no_such_type"})
            out.append({**node, key: "no_such"})
    for key, value in node.items():
        if isinstance(value, dict):
            for inner in mutants(value)[:12]:
                out.append({**node, key: inner})
    return out


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--oracle-dir", type=Path, required=True)
    parser.add_argument("--out", type=Path, default=ROOT / "fixtures/loot-parse-parity.json")
    parser.add_argument("--work", type=Path, default=Path("/tmp"))
    parser.add_argument("--sample", type=int, default=20,
                        help="keep every Nth mutant in the committed fixture (1 keeps all)")
    args = parser.parse_args()
    cases, seen = [], set()
    for (kind, _), node in fragments():
        for mutant in mutants(node):
            table = WRAPPERS[kind](mutant)
            key = json.dumps(table, sort_keys=True)
            if key not in seen:
                seen.add(key)
                cases.append(table)
    hand_count = len(HAND_WRITTEN)
    cases = HAND_WRITTEN + cases
    (args.work / "loot-parse-in.json").write_text(json.dumps(cases))
    classpath = (args.oracle_dir / "cp.txt").read_text().strip()
    subprocess.run(
        ["java", "--add-exports", "java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED",
         "-cp", f"{args.oracle_dir}:{classpath}", "loot_oracle", "parse",
         str(args.work / "loot-parse-in.json"), str(args.work / "loot-parse-out.json")],
        check=True,
    )
    results = json.loads((args.work / "loot-parse-out.json").read_text())
    accepted = sum(1 for r in results if r["ok"])
    out = {
        "format": "cinderstone-loot-parse-parity-v1",
        "minecraft": "1.21",
        "oracle": "tools/loot_oracle.java parse (LootTable.DIRECT_CODEC.parse over JsonOps with the vanilla registries)",
        "sampling": f"every {args.sample}th of {len(cases)} generated mutants",
        "cases": [[table, r["ok"]] for index, (table, r) in enumerate(zip(cases, results)) if index < hand_count or index % args.sample == 0],
    }
    args.out.write_text(json.dumps(out, separators=(",", ":")))
    print(f"cases={len(cases)} accepted={accepted} rejected={len(cases) - accepted}")


if __name__ == "__main__":
    main()
