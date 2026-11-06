#!/usr/bin/env python3
"""Generate the advancement definition fixtures.

* fixtures/advancements-vanilla-1.21.json: every shipped `data/minecraft/advancement/**.json`
  (tools/loot_oracle.java `export-data advancement`), verbatim;
* fixtures/advancement-parse-vectors.json: accept/reject of `Advancement.CODEC` over
  `RegistryOps<JsonOps>` (tools/loot_oracle.java `advancement-parse`) for every shipped file and for
  one-step mutants of every distinct criterion, display, rewards and requirements fragment (each
  key removed, each value replaced by a wrong-typed value, an unknown trigger, one nested level),
  each mutant placed into a minimal advancement.

    python3 tools/advancement_parse_generate.py --oracle-dir /path/outside/git [--sample 4]
"""
import argparse
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
VANILLA = ROOT / "fixtures" / "advancements-vanilla-1.21.json"
OUT = ROOT / "fixtures" / "advancement-parse-vectors.json"
WRONG = ["x", 1, 2.5, True, None, [], {}, [1], {"a": 1}, "minecraft:stone", "#minecraft:logs", -1]
ICON = {"id": "minecraft:stone"}


def oracle(oracle_dir, mode, args):
    classpath = (oracle_dir / "cp.txt").read_text().strip()
    subprocess.run(
        ["java", "--add-exports", "java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED", "-cp", f"{oracle_dir}:{classpath}", "loot_oracle", mode, *args],
        check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )


def mutants(node, depth=0):
    out = []
    if not isinstance(node, dict):
        return out
    for key in node:
        out.append({k: v for k, v in node.items() if k != key})
        for wrong in WRONG:
            if wrong != node[key]:
                out.append({**node, key: wrong})
    for key, value in node.items():
        if isinstance(value, dict) and depth < 2:
            for inner in mutants(value, depth + 1)[:14]:
                out.append({**node, key: inner})
        if isinstance(value, list) and value and isinstance(value[0], dict) and depth < 2:
            for inner in mutants(value[0], depth + 1)[:8]:
                out.append({**node, key: [inner] + value[1:]})
    return out


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--oracle-dir", type=Path, required=True)
    parser.add_argument("--work", type=Path, default=Path("/tmp/advancement-parse-work"))
    parser.add_argument("--sample", type=int, default=4, help="keep every Nth mutant (1 keeps all)")
    parser.add_argument("--out", type=Path, default=OUT)
    args = parser.parse_args()
    args.work.mkdir(parents=True, exist_ok=True)
    oracle(args.oracle_dir, "export-data", [str(args.work / "data.json"), "advancement"])
    vanilla = json.loads((args.work / "data.json").read_text())["advancement"]
    VANILLA.write_text(json.dumps({"format": "advancements-vanilla-v1", "advancement": vanilla}, sort_keys=True, separators=(",", ":")) + "\n")

    seen = set()
    cases = []

    def add(document):
        text = json.dumps(document, sort_keys=True)
        if text not in seen:
            seen.add(text)
            cases.append(text)

    fragments = {"criterion": {}, "display": {}, "rewards": {}, "requirements": {}}
    for text in vanilla.values():
        doc = json.loads(text)
        for criterion in doc["criteria"].values():
            fragments["criterion"].setdefault(json.dumps(criterion, sort_keys=True), criterion)
        if "display" in doc:
            fragments["display"].setdefault(json.dumps(doc["display"], sort_keys=True), doc["display"])
        if "rewards" in doc:
            fragments["rewards"].setdefault(json.dumps(doc["rewards"], sort_keys=True), doc["rewards"])
    base = {"criteria": {"c": {"trigger": "minecraft:tick"}}}
    for criterion in fragments["criterion"].values():
        for mutant in mutants(criterion) + [{**criterion, "trigger": "minecraft:no_such"}, {**criterion, "trigger": "tick"}]:
            add({"criteria": {"c": mutant}})
        conditions = criterion.get("conditions")
        if isinstance(conditions, dict):
            for mutant in mutants(conditions):
                add({"criteria": {"c": {**criterion, "conditions": mutant}}})
    for display in fragments["display"].values():
        for mutant in mutants(display):
            add({**base, "display": mutant})
    for rewards in fragments["rewards"].values():
        for mutant in mutants(rewards):
            add({**base, "rewards": mutant})
    for extra in [
        {"criteria": {}},
        {"criteria": {"a": {"trigger": "minecraft:tick"}, "b": {"trigger": "minecraft:tick"}}, "requirements": [["a"]]},
        {"criteria": {"a": {"trigger": "minecraft:tick"}}, "requirements": [["a"], []]},
        {"criteria": {"a": {"trigger": "minecraft:tick"}}, "requirements": [["a", "z"]]},
        {"criteria": {"a": {"trigger": "minecraft:tick"}}, "requirements": [["a"], ["a"]]},
        {"criteria": {"a": {"trigger": "minecraft:tick", "conditions": None}}},
        {"criteria": {"a": {"trigger": "minecraft:tick", "conditions": []}}},
        {"criteria": {"a": {"trigger": "minecraft:impossible", "conditions": {"x": 1}}}},
        {**base, "parent": "Bad Parent"},
        {**base, "sends_telemetry_event": 1},
        {**base, "display": {"icon": {"id": "minecraft:air"}, "title": "t", "description": "d"}},
        {**base, "display": {"icon": {"id": "minecraft:stone", "count": 100}, "title": "t", "description": "d"}},
        {**base, "display": {"icon": {"id": "minecraft:stone", "components": {"minecraft:no_such": 1}}, "title": "t", "description": "d"}},
        {**base, "display": {"icon": ICON, "title": "t", "description": "d", "frame": "epic"}},
    ]:
        add(extra)
    mutant_count = len(cases)
    texts = list(vanilla.values()) + cases
    (args.work / "in.json").write_text(json.dumps(texts))
    oracle(args.oracle_dir, "advancement-parse", [str(args.work / "in.json"), str(args.work / "out.json")])
    results = json.loads((args.work / "out.json").read_text())
    vanilla_ok = all(r["ok"] for r in results[: len(vanilla)])
    rows = [[text, result["ok"]] for index, (text, result) in enumerate(zip(cases, results[len(vanilla):])) if index % args.sample == 0]
    doc = {
        "format": "advancement-parse-vectors-v1",
        "oracle": "tools/loot_oracle.java advancement-parse (Advancement.CODEC over RegistryOps<JsonOps>, vanilla registries and tags)",
        "generator": "tools/advancement_parse_generate.py",
        "sampling": f"every {args.sample}th of {mutant_count} mutants",
        "vanilla_all_accepted": vanilla_ok,
        "cases": rows,
    }
    args.out.write_text(json.dumps(doc, separators=(",", ":")) + "\n")
    accepted = sum(1 for _, ok in rows if ok)
    print(f"vanilla {len(vanilla)} (all accepted: {vanilla_ok}); mutants {mutant_count}, kept {len(rows)} ({accepted} accepted) -> {args.out}")


if __name__ == "__main__":
    main()
