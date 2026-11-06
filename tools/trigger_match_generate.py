#!/usr/bin/env python3
"""Generate fixtures/trigger-match-vectors.json: entity-free advancement trigger matching.

Every distinct vanilla criterion of the item, enchanting, durability, beacon, ender-eye, potion,
dimension, block, bee-nest and recipe-crafted triggers, plus hand-written conditions, is matched
by the unchanged TriggerInstance.matches (tools/loot_oracle.java `trigger-match`) against seeded
events. Block states are global state ids.

    python3 tools/trigger_match_generate.py --oracle-dir /path/outside/git
"""
import argparse
import json
import random
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
VANILLA = ROOT / "fixtures" / "advancements-vanilla-1.21.json"
OUT = ROOT / "fixtures" / "trigger-match-vectors.json"
ITEM_TRIGGERS = ["consume_item", "filled_bucket", "used_totem", "using_item", "shot_crossbow"]
ITEMS = ["minecraft:apple", "minecraft:golden_apple", "minecraft:water_bucket", "minecraft:lava_bucket", "minecraft:totem_of_undying",
         "minecraft:spyglass", "minecraft:crossbow", "minecraft:diamond_sword", "minecraft:elytra", "minecraft:honey_bottle",
         "minecraft:milk_bucket", "minecraft:potion", "minecraft:cod_bucket", "minecraft:stick", "minecraft:axolotl_bucket"]
EXTRA = {
    "consume_item": [{}, {"item": {"items": "#minecraft:fishes"}}],
    "enchanted_item": [{}, {"levels": {"min": 30}}, {"item": {"items": "minecraft:book"}, "levels": 1}],
    "item_durability_changed": [{"durability": {"max": 10}}, {"delta": {"min": 2}}, {"item": {"items": "minecraft:elytra"}, "durability": 0}],
    "construct_beacon": [{"level": 4}, {"level": {"min": 2, "max": 3}}],
    "used_ender_eye": [{"distance": {"max": 100}}, {"distance": {"min": -50, "max": 10}}, {"distance": 5.5}],
    "brewed_potion": [{}, {"potion": "minecraft:swiftness"}],
    "changed_dimension": [{"from": "minecraft:overworld"}, {"to": "minecraft:the_end"}, {"from": "minecraft:the_nether", "to": "minecraft:overworld"}],
    "enter_block": [{"block": "minecraft:wheat", "state": {"age": "7"}}, {"block": "minecraft:oak_stairs", "state": {"facing": "north", "half": {"min": "bottom"}}}],
    "slide_down_block": [{"block": "minecraft:honey_block"}],
    "bee_nest_destroyed": [{"block": "minecraft:beehive", "num_bees_inside": {"min": 1}}, {"item": {"items": "minecraft:shears"}}],
    "recipe_crafted": [{"recipe_id": "minecraft:stick", "ingredients": [{"items": "#minecraft:planks"}, {"items": "#minecraft:planks"}]},
                       {"recipe_id": "minecraft:stick"}],
}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--oracle-dir", type=Path, required=True)
    parser.add_argument("--work", type=Path, default=Path("/tmp/trigger-match-work"))
    parser.add_argument("--events", type=int, default=40, help="events per criterion")
    args = parser.parse_args()
    rng = random.Random(55)
    triggers = set(ITEM_TRIGGERS) | set(EXTRA) | {"crafter_recipe_crafted"}
    criteria = {}
    for text in json.loads(VANILLA.read_text())["advancement"].values():
        for criterion in json.loads(text)["criteria"].values():
            name = criterion["trigger"].split(":")[1]
            if name in triggers:
                key = json.dumps(criterion, sort_keys=True)
                criteria.setdefault(key, {"trigger": criterion["trigger"], "conditions": criterion.get("conditions", {})})
    for name, extra in EXTRA.items():
        for conditions in extra:
            criteria.setdefault(json.dumps([name, conditions], sort_keys=True), {"trigger": "minecraft:" + name, "conditions": conditions})
    states = {"wheat": list(range(4466, 4474)), "stairs": list(range(2874, 2954)), "honey": [21479], "nests": list(range(21412, 21460)), "stone": [1]}
    all_states = sum(states.values(), [])

    def stack(item=None):
        item = item or rng.choice(ITEMS)
        out = {"id": item, "count": 1}
        if item in ("minecraft:elytra", "minecraft:diamond_sword", "minecraft:crossbow") and rng.random() < 0.7:
            out["components"] = {"minecraft:damage": rng.choice([0, 1, 5, 400, 431])}
        return out

    def event(name):
        if name in ITEM_TRIGGERS:
            return {"stack": stack()}
        if name == "enchanted_item":
            return {"stack": stack(rng.choice(["minecraft:book", "minecraft:diamond_sword", "minecraft:bow"])), "value": rng.choice([1, 3, 15, 30, 31])}
        if name == "item_durability_changed":
            return {"stack": stack(rng.choice(["minecraft:elytra", "minecraft:diamond_sword"])), "value": rng.choice([0, 1, 5, 400, 431, 432])}
        if name == "construct_beacon":
            return {"value": rng.randint(0, 5)}
        if name == "used_ender_eye":
            return {"value": rng.choice([0.0, 24.0, 2500.0, 9999.0, 10001.0, 30.25])}
        if name == "brewed_potion":
            return {"potion": rng.choice(["minecraft:swiftness", "minecraft:water", "minecraft:awkward", "minecraft:long_swiftness"])}
        if name == "changed_dimension":
            dims = ["minecraft:overworld", "minecraft:the_nether", "minecraft:the_end"]
            return {"from": rng.choice(dims), "to": rng.choice(dims)}
        if name in ("enter_block", "slide_down_block"):
            return {"state": rng.choice(all_states)}
        if name == "bee_nest_destroyed":
            return {"state": rng.choice(states["nests"] + [1]), "stack": stack(rng.choice(["minecraft:shears", "minecraft:stick"])), "value": rng.randint(0, 3)}
        recipes = ["minecraft:stick", "minecraft:crafter", "minecraft:oak_planks"]
        pool = ["minecraft:oak_planks", "minecraft:birch_planks", "minecraft:stick", "minecraft:iron_ingot", "minecraft:redstone", "minecraft:dropper", "minecraft:crafting_table"]
        return {"recipe": rng.choice(recipes), "ingredients": [{"id": rng.choice(pool), "count": 1} for _ in range(rng.randint(0, 5))]}

    specs = []
    for criterion in criteria.values():
        name = criterion["trigger"].split(":")[1]
        for _ in range(args.events):
            specs.append({**criterion, "event": event(name)})
    args.work.mkdir(parents=True, exist_ok=True)
    (args.work / "in.json").write_text(json.dumps(specs))
    classpath = (args.oracle_dir / "cp.txt").read_text().strip()
    subprocess.run(["java", "--add-exports", "java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED", "-cp", f"{args.oracle_dir}:{classpath}", "loot_oracle",
                    "trigger-match", str(args.work / "in.json"), str(args.work / "out.json")], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    results = json.loads((args.work / "out.json").read_text())
    errors = [r for r in results if not isinstance(r, bool)]
    if errors:
        raise SystemExit(f"oracle errors: {errors[:3]}")
    for spec, result in zip(specs, results):
        spec["matches"] = result
    OUT.write_text(json.dumps({"format": "trigger-match-vectors-v1", "generator": "tools/trigger_match_generate.py",
                               "oracle": "tools/loot_oracle.java trigger-match (unchanged TriggerInstance.matches)", "cases": specs}, separators=(",", ":")) + "\n")
    print(f"{len(criteria)} criteria, {len(specs)} cases, {sum(results)} matching -> {OUT}")


if __name__ == "__main__":
    main()
