#!/usr/bin/env python3
"""Derive the pinned vanilla 1.21 cooking catalogue and furnace fuel map.

Inputs are the original server JAR, Mojang mappings, and the existing pinned
protocol item-ID catalogue. javap output stays in memory and is never published.
The getFuel instruction order and default item tags determine the fuel map;
there is no hand-maintained approximation of fuel values.
"""
import argparse
import hashlib
import json
import re
import subprocess
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PINNED_JAR_SHA256 = "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--jar", type=Path, required=True)
    p.add_argument("--mappings", type=Path, required=True)
    p.add_argument("--javap", required=True)
    p.add_argument("--output", type=Path, default=ROOT / "fixtures/smelting-1.21.json")
    args = p.parse_args()
    digest = hashlib.sha256(args.jar.read_bytes()).hexdigest()
    if digest != PINNED_JAR_SHA256:
        raise ValueError("This exporter accepts only the pinned Java 1.21 server artifact")
    original = json.loads((ROOT / "fixtures/crafting-1.21.json").read_text())
    if original["source"]["jar_sha256"] != digest:
        raise ValueError("Item IDs and cooking source must have the same artifact identity")
    item_ids = original["item_ids"]

    classes, fields, methods = {}, {}, {}
    owner = None
    for line in args.mappings.read_text().splitlines():
        if line and not line[0].isspace() and not line.startswith("#"):
            name, owner = line.removesuffix(":").split(" -> ")
            classes[name] = owner
        elif owner is not None:
            field = re.match(r"^    [\w.$<>\[\]]+ ([A-Z_\d]+) -> (\w+)$", line)
            if field:
                fields[owner, field[2]] = field[1]
            method = re.match(r"^    (?:\d+:\d+:)?[^ ]+ (\w+)\(\) -> (\w+)$", line)
            if method:
                methods[owner, method[1]] = method[2]
    furnace = classes["net.minecraft.world.level.block.entity.AbstractFurnaceBlockEntity"]
    item = classes["net.minecraft.world.item.Items"]
    block = classes["net.minecraft.world.level.block.Blocks"]
    tags = classes["net.minecraft.tags.ItemTags"]
    item_class = classes["net.minecraft.world.item.Item"]
    get_fuel = methods[furnace, "getFuel"]
    bytecode = subprocess.check_output(
        [args.javap, "-classpath", str(args.jar), "-c", "-p", furnace], text=True
    )
    body = bytecode.split(
        f"public static java.util.Map<{item_class}, java.lang.Integer> {get_fuel}();", 1
    )[1].split("private static boolean", 1)[0]

    with zipfile.ZipFile(args.jar) as jar:
        tag_cache = {}

        def normalized(name):
            return name if ":" in name else "minecraft:" + name

        def tag_items(name, visiting=()):
            name = normalized(name)
            if name in tag_cache:
                return tag_cache[name]
            if name in visiting:
                raise ValueError("Cyclic item tag")
            namespace, path = name.split(":", 1)
            raw = json.loads(jar.read(f"data/{namespace}/tags/item/{path}.json"))
            values = []
            for value in raw["values"]:
                required = True
                if isinstance(value, dict):
                    required = value.get("required", True)
                    value = value["id"]
                try:
                    expanded = tag_items(value[1:], (*visiting, name)) if value.startswith("#") else [item_ids[normalized(value)]]
                except KeyError:
                    if required:
                        raise
                    expanded = []
                values.extend(v for v in expanded if v not in values)
            tag_cache[name] = values
            return values

        excluded = set(tag_items("minecraft:non_flammable_wood"))
        fuel_map, additions = {}, []
        reference, duration = None, None
        for line in body.splitlines():
            match = re.search(r"getstatic.*// Field (\w+)\.(\w+):", line)
            if match and match[1] in {item, block, tags}:
                reference, duration = (match[1], match[2]), None
            match = re.search(r"\b(?:sipush|bipush)\s+(\d+)", line)
            if match:
                duration = int(match[1])
            if "invokestatic" not in line or "// Method a:" not in line:
                continue
            if reference is None or duration is None:
                raise ValueError("Unrecognized getFuel addition")
            name = "minecraft:" + fields[reference].lower()
            is_tag = reference[0] == tags
            members = tag_items(name) if is_tag else [item_ids[name]]
            for item_id in members:
                if item_id not in excluded:
                    fuel_map[item_id] = duration
            additions.append({"kind": "tag" if is_tag else "item", "id": name, "duration": duration})
            reference, duration = None, None
        if len(additions) != 59 or not fuel_map:
            raise ValueError("The pinned getFuel instruction sequence changed")

        def ingredient(value):
            if isinstance(value, list):
                out = []
                for alternative in value:
                    out.extend(v for v in ingredient(alternative) if v not in out)
                return out
            if "tag" in value:
                return tag_items(value["tag"])
            return [item_ids[normalized(value["item"])]]

        recipes, matching = [], {}
        supported = {"minecraft:smelting": "Smelting", "minecraft:blasting": "Blasting", "minecraft:smoking": "Smoking"}
        for entry in sorted(jar.namelist()):
            if not entry.startswith("data/minecraft/recipe/") or not entry.endswith(".json"):
                continue
            raw = json.loads(jar.read(entry))
            if raw["type"] not in supported:
                continue
            kind = supported[raw["type"]]
            recipe_id = "minecraft:" + entry.removeprefix("data/minecraft/recipe/").removesuffix(".json")
            inputs = ingredient(raw["ingredient"])
            result = raw["result"]
            if set(result) - {"id", "count"}:
                raise ValueError("Cooking result has additional component semantics")
            record = {
                "id": recipe_id, "kind": kind, "ingredient": inputs,
                "result": {"item_id": item_ids[normalized(result["id"])], "count": result.get("count", 1), "components": [], "removed_components": []},
                "cooking_time": raw.get("cookingtime", 200 if kind == "Smelting" else 100),
                "experience": raw.get("experience", 0.0),
            }
            # Vanilla default data has no ambiguous matching cooking recipes.
            # CachedCheck's last-recipe preference therefore has the same result
            # as indexed lookup; custom datapack ordering is not asserted here.
            for input_id in inputs:
                if (kind, input_id) in matching:
                    raise ValueError("Default cooking recipes became ambiguous")
                matching[kind, input_id] = recipe_id
            recipes.append(record)
        if len(recipes) != 103:
            raise ValueError("Unexpected default cooking recipe count")

    fixture = {
        "minecraft": "1.21", "protocol": 767,
        "source": {"jar_sha256": digest, "recipes": "data/minecraft/recipe/*.json", "item_tags": "data/minecraft/tags/item/*.json", "fuel_method": "AbstractFurnaceBlockEntity.getFuel", "mapping_sha256": hashlib.sha256(args.mappings.read_bytes()).hexdigest(), "fuel_derivation": "Ordered original bytecode additions expanded against default item tags; non_flammable_wood omitted"},
        "scope": "Pinned vanilla default recipes and item tags; datapack reload and custom recipe priority are not represented",
        "fuel_additions": additions,
        "fuels": {str(k): v for k, v in sorted(fuel_map.items())},
        "remainders": original["remainders"],
        "recipes": recipes,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(fixture, indent=2) + "\n")
    print(json.dumps({"recipes": len(recipes), "fuels": len(fuel_map), "fuel_additions": len(additions), "output": str(args.output)}))


if __name__ == "__main__":
    main()
