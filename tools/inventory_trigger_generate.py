#!/usr/bin/env python3
"""Generate fixtures/inventory-trigger-vectors.json: InventoryChangeTrigger matching.

Every distinct `inventory_changed` criterion of the shipped advancements
(fixtures/advancements-vanilla-1.21.json), plus hand-written slot-count and multi-item ones, is
matched by the unchanged `InventoryChangeTrigger.TriggerInstance.matches`
(tools/loot_oracle.java `inventory-trigger`) against seeded inventories of the 41 player slots and
a changed stack. Each case stores one character per criterion (1 matches).

    python3 tools/inventory_trigger_generate.py --oracle-dir /path/outside/git
"""
import argparse
import json
import random
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
VANILLA = ROOT / "fixtures" / "advancements-vanilla-1.21.json"
OUT = ROOT / "fixtures" / "inventory-trigger-vectors.json"
EXTRA = [
    {},
    {"slots": {"occupied": {"min": 3}}},
    {"slots": {"empty": 0}},
    {"slots": {"full": {"min": 1}, "occupied": {"max": 5}}},
    {"items": [{"items": "minecraft:diamond"}, {"items": "minecraft:stick"}]},
    {"items": [{"items": "minecraft:diamond", "count": {"min": 2}}, {"items": "#minecraft:logs"}, {"items": "minecraft:apple"}]},
    {"items": [{"items": "minecraft:diamond_sword", "predicates": {"minecraft:damage": {"damage": {"min": 1}}}}]},
    {"items": [{"items": "minecraft:enchanted_book", "predicates": {"minecraft:stored_enchantments": [{"enchantments": "minecraft:mending"}]}}]},
    {"items": [{"count": 64}]},
    {"items": [{"items": "minecraft:stick"}, {"items": "minecraft:stick"}]},
]
FILLER = [
    "minecraft:dirt", "minecraft:stone", "minecraft:oak_log", "minecraft:birch_planks", "minecraft:diamond", "minecraft:stick",
    "minecraft:apple", "minecraft:iron_ingot", "minecraft:ender_pearl", "minecraft:diamond_sword", "minecraft:water_bucket",
    "minecraft:wheat_seeds", "minecraft:cobblestone", "minecraft:crafting_table", "minecraft:iron_pickaxe", "minecraft:enchanted_book",
]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--oracle-dir", type=Path, required=True)
    parser.add_argument("--work", type=Path, default=Path("/tmp/inventory-trigger-work"))
    parser.add_argument("--cases", type=int, default=160)
    parser.add_argument("--out", type=Path, default=OUT)
    args = parser.parse_args()
    rng = random.Random(27)
    vanilla = json.loads(VANILLA.read_text())["advancement"]
    criteria = {}
    items = set(FILLER)
    for text in vanilla.values():
        for criterion in json.loads(text)["criteria"].values():
            if criterion["trigger"] == "minecraft:inventory_changed":
                conditions = criterion.get("conditions", {})
                criteria.setdefault(json.dumps(conditions, sort_keys=True), conditions)
                for predicate in conditions.get("items", []):
                    named = predicate.get("items")
                    if isinstance(named, str) and not named.startswith("#"):
                        items.add(named)
                    elif isinstance(named, list):
                        items.update(named)
    criteria = list(criteria.values()) + EXTRA
    items = sorted(items)

    def stack(name, count=None):
        out = {"id": name, "count": count or rng.choice([1, 1, 2, 16, 63, 64])}
        if name == "minecraft:diamond_sword" and rng.random() < 0.5:
            out["components"] = {"minecraft:damage": rng.randint(0, 3)}
        if name == "minecraft:enchanted_book" and rng.random() < 0.6:
            out["components"] = {"minecraft:stored_enchantments": {"levels": {rng.choice(["minecraft:mending", "minecraft:unbreaking"]): 1}}}
        if name in ("minecraft:diamond_sword", "minecraft:water_bucket", "minecraft:iron_pickaxe", "minecraft:enchanted_book"):
            out["count"] = 1
        if name in ("minecraft:ender_pearl",):
            out["count"] = min(out["count"], 16)
        return out

    cases = []
    for k in range(args.cases):
        filled = rng.choice([0, 1, 2, 3, 5, 10, 20, 36, 41])
        slots = rng.sample(range(41), filled)
        inventory = [{"slot": slot, "item": stack(rng.choice(items))} for slot in slots]
        if inventory and rng.random() < 0.8:
            changed = rng.choice(inventory)["item"]
        else:
            changed = {"id": "minecraft:air", "count": 1} if rng.random() < 0.3 else stack(rng.choice(items))
        cases.append({"inventory": inventory, "changed": changed})

    args.work.mkdir(parents=True, exist_ok=True)
    (args.work / "in.json").write_text(json.dumps({"criteria": criteria, "cases": cases}))
    classpath = (args.oracle_dir / "cp.txt").read_text().strip()
    subprocess.run(
        ["java", "--add-exports", "java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED", "-cp", f"{args.oracle_dir}:{classpath}", "loot_oracle",
         "inventory-trigger", str(args.work / "in.json"), str(args.work / "out.json")],
        check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    results = json.loads((args.work / "out.json").read_text())
    for case, bits in zip(cases, results):
        case["matches"] = bits
    doc = {
        "format": "inventory-trigger-vectors-v1",
        "oracle": "tools/loot_oracle.java inventory-trigger (unchanged InventoryChangeTrigger.TriggerInstance.matches over a real Inventory)",
        "generator": "tools/inventory_trigger_generate.py",
        "criteria": criteria,
        "cases": cases,
    }
    args.out.write_text(json.dumps(doc, separators=(",", ":")) + "\n")
    ones = sum(bits.count("1") for bits in results)
    print(f"{len(criteria)} criteria x {len(cases)} cases, {ones} matches -> {args.out}")


if __name__ == "__main__":
    main()
