#!/usr/bin/env python3
"""Generate fixtures/recipe-unlock-vectors.json: recipe unlocks after scripted item pickups.

Each script picks up items one at a time into an empty 41-slot inventory. After every pickup the
unchanged `InventoryChangeTrigger.TriggerInstance.matches` (tools/loot_oracle.java
`inventory-trigger`) answers every distinct `inventory_changed` criterion of the shipped
`recipes/**` advancements against the inventory and the picked-up stack. The expected unlock set
then follows the original award path: every recipe advancement has a single any-of requirement
group, so an advancement completes the first time one of its listening `inventory_changed`
criteria matches, and its `rewards.recipes` are added to the recipe book.

    python3 tools/recipe_unlock_generate.py --oracle-dir /path/outside/git
"""
import argparse
import json
import random
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
VANILLA = ROOT / "fixtures" / "advancements-vanilla-1.21.json"
OUT = ROOT / "fixtures" / "recipe-unlock-vectors.json"
COMMON = ["minecraft:oak_log", "minecraft:birch_log", "minecraft:cobblestone", "minecraft:iron_ingot", "minecraft:stick",
          "minecraft:oak_planks", "minecraft:diamond", "minecraft:white_wool", "minecraft:string", "minecraft:coal",
          "minecraft:redstone", "minecraft:gold_ingot", "minecraft:copper_ingot", "minecraft:sand", "minecraft:wheat",
          "minecraft:leather", "minecraft:feather", "minecraft:flint", "minecraft:stone", "minecraft:crimson_stem"]
MAX_STACK_1 = {"minecraft:water_bucket", "minecraft:lava_bucket", "minecraft:milk_bucket"}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--oracle-dir", type=Path, required=True)
    parser.add_argument("--work", type=Path, default=Path("/tmp/recipe-unlock-work"))
    parser.add_argument("--scripts", type=int, default=60)
    args = parser.parse_args()
    rng = random.Random(2706)
    files = json.loads(VANILLA.read_text())["advancement"]
    recipes = {}
    criteria = {}
    items = set(COMMON)
    for aid, text in sorted(files.items()):
        if not aid.startswith("minecraft:recipes/"):
            continue
        adv = json.loads(text)
        assert len(adv.get("requirements", [[0]])) == 1, aid
        keys = []
        for criterion in adv["criteria"].values():
            if criterion["trigger"] != "minecraft:inventory_changed":
                continue
            conditions = criterion.get("conditions", {})
            key = json.dumps(conditions, sort_keys=True)
            criteria.setdefault(key, conditions)
            keys.append(key)
            for predicate in conditions.get("items", []):
                named = predicate.get("items")
                if isinstance(named, str) and not named.startswith("#"):
                    items.add(named)
                elif isinstance(named, list):
                    items.update(named)
        recipes[aid] = (keys, adv.get("rewards", {}).get("recipes", []))
    keys = list(criteria)
    index = {key: i for i, key in enumerate(keys)}
    items = sorted(items)

    scripts = [[("minecraft:oak_log", 1)], [("minecraft:oak_log", 4), ("minecraft:cobblestone", 3)]]
    for _ in range(args.scripts):
        steps = []
        for _ in range(rng.randint(1, 10)):
            name = rng.choice(COMMON if rng.random() < 0.6 else items)
            steps.append((name, 1 if name in MAX_STACK_1 else rng.choice([1, 1, 2, 5, 16])))
        scripts.append(steps)

    cases = []
    for steps in scripts:
        inventory = {}
        for name, count in steps:
            slot = next((s for s, st in sorted(inventory.items()) if st["id"] == name and st["count"] + count <= 64 and name not in MAX_STACK_1), None)
            if slot is None:
                slot = next(s for s in range(36) if s not in inventory)
                inventory[slot] = {"id": name, "count": count}
            else:
                inventory[slot]["count"] += count
            cases.append({"inventory": [{"slot": s, "item": dict(st)} for s, st in sorted(inventory.items())], "changed": dict(inventory[slot])})

    args.work.mkdir(parents=True, exist_ok=True)
    (args.work / "in.json").write_text(json.dumps({"criteria": [criteria[k] for k in keys], "cases": cases}))
    classpath = (args.oracle_dir / "cp.txt").read_text().strip()
    subprocess.run(["java", "--add-exports", "java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED", "-cp", f"{args.oracle_dir}:{classpath}",
                    "loot_oracle", "inventory-trigger", str(args.work / "in.json"), str(args.work / "out.json")],
                   check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    bits = json.loads((args.work / "out.json").read_text())

    out_scripts = []
    position = 0
    for steps in scripts:
        done = set()
        unlocked = []
        expect = []
        for _ in steps:
            row = bits[position]
            case = cases[position]
            position += 1
            newly = []
            for aid, (criterion_keys, rewards) in recipes.items():
                if aid not in done and any(row[index[k]] == "1" for k in criterion_keys):
                    done.add(aid)
                    newly.append(aid)
                    for recipe in rewards:
                        if recipe not in unlocked:
                            unlocked.append(recipe)
            expect.append({"inventory": case["inventory"], "changed": case["changed"], "completed": newly})
        out_scripts.append({"steps": expect, "recipes": sorted(unlocked)})
    OUT.write_text(json.dumps({
        "format": "recipe-unlock-vectors-v1",
        "generator": "tools/recipe_unlock_generate.py",
        "oracle": "tools/loot_oracle.java inventory-trigger (unchanged InventoryChangeTrigger.TriggerInstance.matches) over the shipped recipes/** criteria",
        "scripts": out_scripts,
    }, separators=(",", ":")) + "\n")
    total = sum(len(s["recipes"]) for s in out_scripts)
    print(f"{len(scripts)} scripts, {len(cases)} pickups, {len(keys)} criteria, {total} unlocks -> {OUT}")


if __name__ == "__main__":
    main()
