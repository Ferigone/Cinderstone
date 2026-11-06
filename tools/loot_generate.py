#!/usr/bin/env python3
"""Generate fixtures/loot-oracle-vectors.json from the unchanged original loot classes.

The scenario set is derived from the shipped tables themselves: every table is rolled
under the loot parameters its parameter set allows, with tools, block states, killers,
fire state, luck and seeds chosen from what its own conditions test. The expected
result of each case (produced stacks as Slot wire bytes, plus the next RNG draw after
the roll) is computed by tools/loot_oracle.java running the original server classes.

Usage (the oracle directory is built by tools/loot_remap.java; nothing in it is committed):

    python3 tools/loot_generate.py --oracle-dir /path/to/oracle [--out fixtures/loot-oracle-vectors.json]

where the oracle directory contains `cp.txt` (a class path with the remapped server jar and
its libraries) and the compiled `loot_oracle` class.
"""
import argparse
import json
import subprocess
import sys
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TABLES = json.loads((ROOT / "fixtures/loot-tables-1.21.json").read_text())["tables"]
FACTS = json.loads((ROOT / "fixtures/loot-registry-facts-1.21.json").read_text())


def stack(item, count=1, enchantments=None, damage=None):
    out = {"id": "minecraft:" + item, "count": count}
    components = {}
    if enchantments:
        components["minecraft:enchantments"] = {"levels": {"minecraft:" + k: v for k, v in enchantments.items()}}
    if damage is not None:
        components["minecraft:damage"] = damage
    if components:
        out["components"] = components
    return out


TOOLS = [
    stack("air"),                                                  # 0 empty hand
    stack("diamond_pickaxe"),                                      # 1
    stack("diamond_pickaxe", enchantments={"silk_touch": 1}),      # 2
    stack("diamond_pickaxe", enchantments={"fortune": 1}),         # 3
    stack("diamond_pickaxe", enchantments={"fortune": 2}),         # 4
    stack("diamond_pickaxe", enchantments={"fortune": 3}),         # 5
    stack("shears"),                                               # 6
    stack("diamond_axe"),                                          # 7
    stack("diamond_hoe", enchantments={"silk_touch": 1}),          # 8
    stack("iron_pickaxe", enchantments={"silk_touch": 1, "fortune": 3}),  # 9
    stack("stick"),                                                # 10
    stack("fishing_rod"),                                          # 11
    stack("fishing_rod", enchantments={"luck_of_the_sea": 3}),     # 12
    stack("diamond_pickaxe", damage=100),                          # 13
    {"id": "minecraft:enchanted_book", "count": 1,
     "components": {"minecraft:stored_enchantments": {"levels": {"minecraft:fortune": 2}}}},  # 14
    stack("iron_pickaxe", count=3),                                # 15
]

ENTITIES = []
ENTITY_INDEX = {}


def entity(**fields):
    key = json.dumps(fields, sort_keys=True)
    if key not in ENTITY_INDEX:
        ENTITY_INDEX[key] = len(ENTITIES)
        ENTITIES.append(fields)
    return ENTITY_INDEX[key]


DAMAGE_SOURCES = []
DS_INDEX = {}


def damage_source(type_, source=None, direct=None):
    fields = {"type": "minecraft:" + type_}
    if source is not None:
        fields["source_entity"] = ENTITIES[source]
    if direct is not None:
        fields["direct_entity"] = ENTITIES[direct]
    key = json.dumps(fields, sort_keys=True)
    if key not in DS_INDEX:
        DS_INDEX[key] = len(DAMAGE_SOURCES)
        DAMAGE_SOURCES.append(fields)
    return DS_INDEX[key]


LEVELS = [{}]
LEVEL_INDEX = {"{}": 0}


def level(**fields):
    key = json.dumps(fields, sort_keys=True)
    if key not in LEVEL_INDEX:
        LEVEL_INDEX[key] = len(LEVELS)
        LEVELS.append(fields)
    return LEVEL_INDEX[key]


BLOCK_STATES = []
BS_INDEX = {}


def block_state(name, properties=None):
    fields = {"name": name}
    if properties:
        fields["properties"] = properties
    key = json.dumps(fields, sort_keys=True)
    if key not in BS_INDEX:
        BS_INDEX[key] = len(BLOCK_STATES)
        BLOCK_STATES.append(fields)
    return BS_INDEX[key]


def seed_for(name, n):
    value = zlib.crc32(f"{name}#{n}".encode()) * 2654435761 % (1 << 63)
    return value or 1


def walk(node, visit):
    if isinstance(node, dict):
        visit(node)
        for value in node.values():
            walk(value, visit)
    elif isinstance(node, list):
        for value in node:
            walk(value, visit)


def block_conditions(table):
    found = []

    def visit(node):
        if node.get("condition") == "minecraft:block_state_property" and "block" in node:
            found.append((node["block"], node.get("properties") or {}))

    walk(table, visit)
    return found


def mentions(table, text):
    return text in json.dumps(table)


def block_cases():
    cases = []
    for key, table in TABLES.items():
        if table.get("type") != "minecraft:block":
            continue
        block = "minecraft:" + key.split("/", 1)[1]
        if block not in FACTS["registries"]["block"]:
            continue
        states = [block_state(block)]
        seen = {json.dumps({})}
        for name, props in block_conditions(table):
            if name != block:
                continue
            marker = json.dumps(props, sort_keys=True)
            if marker in seen:
                continue
            seen.add(marker)
            states.append(block_state(name, props))
        tools = [0, 1]
        if mentions(table, "fortune") or mentions(table, "apply_bonus"):
            tools.append(5)
        if mentions(table, "silk_touch"):
            tools.append(2)
        if mentions(table, "shears"):
            tools.append(6)
        if mentions(table, "cluster_max_harvestables"):
            tools.append(1)
        tools = sorted(set(tools))
        bare = key[len("minecraft:"):]
        n = 0
        for state in states[:4]:
            for tool in tools:
                n += 1
                cases.append({"t": bare, "s": seed_for(bare, n), "k": tool, "b": state})
        if mentions(table, "survives_explosion") or mentions(table, "explosion_decay"):
            for radius in (2.0, 5.0):
                n += 1
                cases.append({"t": bare, "s": seed_for(bare, n), "k": tools[-1], "b": states[0], "x": radius})
        n += 1
        cases.append({"t": bare, "s": seed_for(bare, n), "k": 5, "b": states[-1]})
        if mentions(table, "copy_components") or mentions(table, "copy_state"):
            for be in ({"custom_name": "Named"}, {"lock": "secret"}, {"custom_name": "N", "lock": "k"}):
                n += 1
                cases.append({"t": bare, "s": seed_for(bare, n), "k": 1, "b": states[0], "be": be})
    return cases


def entity_cases():
    cases = []
    attackers = {
        "none": None,
        "plain": entity(type="minecraft:zombie", mainhand={"id": "minecraft:diamond_sword", "count": 1}),
        "looting1": entity(type="minecraft:zombie", mainhand=stack("diamond_sword", enchantments={"looting": 1})),
        "looting3": entity(type="minecraft:zombie", mainhand=stack("diamond_sword", enchantments={"looting": 3})),
        "fire": entity(type="minecraft:zombie", mainhand=stack("diamond_sword", enchantments={"fire_aspect": 2})),
    }
    sources = {
        "mob": damage_source("mob_attack", source=entity(type="minecraft:zombie")),
        "player": damage_source("player_attack"),
        "skeleton": damage_source("arrow", source=entity(type="minecraft:skeleton"),
                                  direct=entity(type="minecraft:arrow")),
        "creeper": damage_source("explosion", source=entity(type="minecraft:creeper")),
        "frog_temperate": damage_source("mob_attack", source=entity(type="minecraft:frog", variant="minecraft:temperate")),
        "frog_warm": damage_source("mob_attack", source=entity(type="minecraft:frog", variant="minecraft:warm")),
        "frog_cold": damage_source("mob_attack", source=entity(type="minecraft:frog", variant="minecraft:cold")),
        "generic": damage_source("generic"),
        "explosion_tag": damage_source("explosion"),
    }
    for key, table in TABLES.items():
        if table.get("type") != "minecraft:entity":
            continue
        bare = key[len("minecraft:"):]
        entity_type = "minecraft:" + bare.split("/", 1)[1].split("/")[0]
        n = 0
        sizes = [None]
        if entity_type in ("minecraft:slime", "minecraft:magma_cube"):
            sizes = [1, 2, 4]
        fires = [False, True] if mentions(table, "is_on_fire") else [False]
        for size in sizes:
            for fire in fires:
                fields = {"type": entity_type}
                if size is not None:
                    fields["size"] = size
                if fire:
                    fields["on_fire"] = True
                this_entity = entity(**fields)
                for attacker_name in ("none", "plain", "looting1", "looting3", "fire"):
                    if attacker_name == "fire" and not fire:
                        continue
                    for player in (False, True):
                        for source_name in ("player", "mob"):
                            if source_name == "mob" and attacker_name != "none":
                                continue
                            n += 1
                            case = {"t": bare, "s": seed_for(bare, n), "te": this_entity, "ds": sources[source_name]}
                            if attackers[attacker_name] is not None:
                                case["ae"] = attackers[attacker_name]
                            if player:
                                case["lp"] = 1
                            cases.append(case)
        if mentions(table, "damage_source_properties"):
            for source_name in ("skeleton", "creeper", "frog_temperate", "frog_warm", "frog_cold", "generic", "explosion_tag"):
                n += 1
                cases.append({"t": bare, "s": seed_for(bare, n), "te": entity(type=entity_type),
                              "ds": sources[source_name], "lp": 1})
    return cases


def other_cases():
    cases = []
    villager = entity(type="minecraft:villager")
    piglin = entity(type="minecraft:piglin")
    for key, table in TABLES.items():
        kind = table.get("type")
        bare = key[len("minecraft:"):]
        if kind == "minecraft:chest":
            for n in range(1, 7):
                cases.append({"t": bare, "s": seed_for(bare, n)})
            cases.append({"t": bare, "s": seed_for(bare, 9), "l": 1.0})
        elif kind == "minecraft:archaeology":
            for n in range(1, 5):
                cases.append({"t": bare, "s": seed_for(bare, n)})
        elif kind == "minecraft:gift":
            for n in range(1, 5):
                cases.append({"t": bare, "s": seed_for(bare, n), "te": villager})
        elif kind == "minecraft:barter":
            for n in range(1, 25):
                cases.append({"t": bare, "s": seed_for(bare, n), "te": piglin})
        elif kind == "minecraft:equipment":
            for n in range(1, 5):
                cases.append({"t": bare, "s": seed_for(bare, n), "te": entity(type="minecraft:zombie")})
        elif kind == "minecraft:shearing":
            for n in range(1, 5):
                cases.append({"t": bare, "s": seed_for(bare, n), "te": entity(type="minecraft:bogged")})
        elif kind == "minecraft:fishing":
            for rod in (11, 12):
                for luck in (0.0, 2.0):
                    for biome in ("minecraft:plains", "minecraft:jungle", "minecraft:ocean"):
                        for open_water in (False, True):
                            n = len(cases) + 1
                            hook = entity(type="minecraft:fishing_bobber", open_water=open_water)
                            cases.append({"t": bare, "s": seed_for(bare, n), "k": rod, "l": luck, "te": hook,
                                          "lv": level(biome=biome)})
    return cases



CUSTOM_TABLES = []


def custom(table):
    CUSTOM_TABLES.append(table)
    return len(CUSTOM_TABLES) - 1


def item(name, **extra):
    return {"type": "minecraft:item", "name": "minecraft:" + name, **extra}


def const_count(n, **extra):
    return {"function": "minecraft:set_count", "count": n, **extra}


def synthetic_cases():
    """Hand-written tables for behaviour the shipped data pack never exercises."""
    cases = []
    zombie = entity(type="minecraft:zombie")
    burning = entity(type="minecraft:zombie", on_fire=True)
    sword = entity(type="minecraft:zombie", mainhand=stack("diamond_sword", enchantments={"looting": 2}))
    player_hit = damage_source("player_attack")

    def add(table, n=8, **fields):
        index = custom(table)
        for seed in range(1, n + 1):
            cases.append({"ct": index, "s": seed_for(f"custom{index}", seed), **fields})

    # pool structure: rolls, bonus rolls with luck, pool and table functions
    add({"type": "minecraft:chest", "pools": [
        {"rolls": {"type": "minecraft:uniform", "min": 1, "max": 4}, "bonus_rolls": 1.5,
         "entries": [item("apple", weight=3), item("bread", weight=2, quality=2), item("stick", quality=-1)],
         "functions": [const_count({"type": "minecraft:uniform", "min": 1, "max": 6})]}],
         "functions": [{"function": "minecraft:limit_count", "limit": {"min": 2, "max": 4}}]}, 24, l=0.0)
    add({"type": "minecraft:chest", "pools": [
        {"rolls": 3, "bonus_rolls": 2.0,
         "entries": [item("apple", weight=3), item("bread", weight=2, quality=2), item("stick", quality=-1)]}]}, 24, l=2.5)
    add({"type": "minecraft:chest", "pools": [
        {"rolls": {"type": "minecraft:binomial", "n": 6, "p": 0.4},
         "entries": [item("gold_ingot"), item("iron_ingot"), item("diamond")]}]}, 12)
    add({"type": "minecraft:chest", "pools": [
        {"rolls": 2, "conditions": [{"condition": "minecraft:random_chance", "chance": 0.5}],
         "entries": [item("gold_ingot")]},
        {"rolls": 1, "entries": [item("coal", weight=0), item("stone", weight=0)]}]}, 12)

    # entry containers
    add({"type": "minecraft:chest", "pools": [{"rolls": 2, "entries": [
        {"type": "minecraft:alternatives", "children": [
            item("diamond", conditions=[{"condition": "minecraft:random_chance", "chance": 0.3}]),
            item("emerald", conditions=[{"condition": "minecraft:random_chance", "chance": 0.5}]),
            item("coal")]},
        {"type": "minecraft:group", "children": [item("stick"), item("string", weight=5)]},
        {"type": "minecraft:sequence", "children": [
            item("bone", conditions=[{"condition": "minecraft:random_chance", "chance": 0.6}]),
            item("flint")]},
        {"type": "minecraft:empty", "weight": 4}]}]}, 24)
    add({"type": "minecraft:chest", "pools": [{"rolls": 3, "entries": [
        {"type": "minecraft:tag", "name": "minecraft:flowers", "expand": True,
         "functions": [const_count(7)]},
        {"type": "minecraft:tag", "name": "minecraft:logs", "expand": False, "weight": 6,
         "functions": [const_count(2)]}]}]}, 12)
    add({"type": "minecraft:chest", "pools": [{"rolls": 1, "entries": [
        {"type": "minecraft:loot_table", "weight": 2, "value": {"type": "minecraft:chest", "pools": [
            {"rolls": 2, "entries": [item("arrow"), item("feather")]}],
            "functions": [const_count(3)]},
         "functions": [{"function": "minecraft:limit_count", "limit": 4}]},
        {"type": "minecraft:loot_table", "value": "minecraft:chests/simple_dungeon"},
        {"type": "minecraft:loot_table", "value": "minecraft:chests/missing_table"},
        {"type": "minecraft:dynamic", "name": "minecraft:contents"}]}]}, 16)

    # conditions on the chest context
    add({"type": "minecraft:chest", "pools": [{"rolls": 4, "entries": [
        {"type": "minecraft:item", "name": "minecraft:apple", "conditions": [
            {"condition": "minecraft:inverted", "term": {"condition": "minecraft:random_chance", "chance": {"type": "minecraft:uniform", "min": 0.2, "max": 0.7}}}]},
        {"type": "minecraft:item", "name": "minecraft:bread", "conditions": [
            [{"condition": "minecraft:random_chance", "chance": 0.5}, {"condition": "minecraft:random_chance", "chance": 0.5}]]},
        {"type": "minecraft:item", "name": "minecraft:cake", "conditions": [
            {"condition": "minecraft:any_of", "terms": [{"condition": "minecraft:random_chance", "chance": 0.1}, {"condition": "minecraft:random_chance", "chance": 0.4}]}]},
        {"type": "minecraft:item", "name": "minecraft:cookie", "conditions": [
            {"condition": "minecraft:value_check", "value": {"type": "minecraft:uniform", "min": 0, "max": 10}, "range": {"min": 3, "max": {"type": "minecraft:uniform", "min": 4, "max": 9}}}]}]}]}, 16)
    for day_time, raining, thundering in ((0, False, False), (6000, True, False), (13000, True, True), (30000, False, True)):
        add({"type": "minecraft:chest", "pools": [{"rolls": 1, "entries": [
            item("clock", conditions=[{"condition": "minecraft:time_check", "value": {"min": 12000, "max": 24000}, "period": 24000}]),
            item("water_bucket", conditions=[{"condition": "minecraft:weather_check", "raining": True}]),
            item("lightning_rod", conditions=[{"condition": "minecraft:weather_check", "thundering": True, "raining": True}]),
            item("sunflower", conditions=[{"condition": "minecraft:weather_check", "raining": False}])]}]}, 4,
            lv=level(day_time=day_time, raining=raining, thundering=thundering))
    for biome in ("minecraft:plains", "minecraft:desert", "minecraft:swamp"):
        add({"type": "minecraft:chest", "pools": [{"rolls": 1, "entries": [
            item("cactus", conditions=[{"condition": "minecraft:location_check", "predicate": {"biomes": ["minecraft:desert", "minecraft:badlands"]}}]),
            item("lily_pad", conditions=[{"condition": "minecraft:location_check", "predicate": {"biomes": "#minecraft:is_overworld"}, "offsetY": 3}]),
            item("grass_block", conditions=[{"condition": "minecraft:location_check", "predicate": {"position": {"y": {"min": 60, "max": 70}}}}]),
            item("stone", conditions=[{"condition": "minecraft:location_check", "predicate": {"dimension": "minecraft:the_nether"}}])]}]}, 2,
            lv=level(biome=biome))

    # functions on items
    add({"type": "minecraft:chest", "pools": [{"rolls": 1, "entries": [
        item("stick", functions=[
            const_count({"type": "minecraft:binomial", "n": 10, "p": 0.3}),
            const_count(2, add=True),
            {"function": "minecraft:limit_count", "limit": {"max": 5}},
            {"function": "minecraft:limit_count", "limit": {"min": 2}}])]}]}, 12)
    add({"type": "minecraft:chest", "pools": [{"rolls": 1, "entries": [
        item("diamond_pickaxe", functions=[
            {"function": "minecraft:set_damage", "damage": {"type": "minecraft:uniform", "min": 0.1, "max": 0.9}},
            {"function": "minecraft:set_damage", "damage": 0.25, "add": True}])]}]}, 8)
    add({"type": "minecraft:chest", "pools": [{"rolls": 1, "entries": [
        item("stone", functions=[{"function": "minecraft:set_item", "item": "minecraft:diamond"}, const_count(3)]),
        item("book", functions=[{"function": "minecraft:enchant_randomly"}]),
        item("book", functions=[{"function": "minecraft:enchant_randomly", "only_compatible": False}]),
        item("diamond_sword", functions=[{"function": "minecraft:enchant_randomly"}]),
        item("bow", functions=[{"function": "minecraft:enchant_randomly", "options": ["minecraft:power", "minecraft:punch", "minecraft:unbreaking"]}]),
        item("bow", functions=[{"function": "minecraft:enchant_randomly", "options": ["minecraft:sharpness"]}]),
        item("book", functions=[{"function": "minecraft:enchant_with_levels", "levels": {"type": "minecraft:uniform", "min": 5, "max": 30}}]),
        item("iron_chestplate", functions=[{"function": "minecraft:enchant_with_levels", "levels": 20, "options": "#minecraft:on_random_loot"}]),
        item("fishing_rod", functions=[{"function": "minecraft:enchant_with_levels", "levels": 30}]),
        item("stick", functions=[{"function": "minecraft:enchant_with_levels", "levels": 30}]),
        item("book", functions=[{"function": "minecraft:set_enchantments", "enchantments": {"minecraft:sharpness": 3, "minecraft:efficiency": {"type": "minecraft:uniform", "min": 1, "max": 5}}}]),
        item("diamond_sword", functions=[
            {"function": "minecraft:set_enchantments", "enchantments": {"minecraft:sharpness": 2}},
            {"function": "minecraft:set_enchantments", "enchantments": {"minecraft:sharpness": 2, "minecraft:unbreaking": 1}, "add": True}])]}]}, 40)
    add({"type": "minecraft:chest", "pools": [{"rolls": 1, "entries": [
        item("potion", functions=[{"function": "minecraft:set_potion", "id": "minecraft:strong_healing"}]),
        item("splash_potion", functions=[{"function": "minecraft:set_potion", "id": "minecraft:long_night_vision"}]),
        item("suspicious_stew", functions=[{"function": "minecraft:set_stew_effect", "effects": [
            {"type": "minecraft:night_vision", "duration": {"type": "minecraft:uniform", "min": 4, "max": 8}},
            {"type": "minecraft:saturation", "duration": 7}]}, {"function": "minecraft:set_stew_effect", "effects": [{"type": "minecraft:poison", "duration": 5}]}]),
        item("ominous_bottle", functions=[{"function": "minecraft:set_ominous_bottle_amplifier", "amplifier": {"type": "minecraft:uniform", "min": -2, "max": 9}}]),
        item("goat_horn", functions=[{"function": "minecraft:set_instrument", "options": "#minecraft:screaming_goat_horns"}]),
        item("stick", functions=[{"function": "minecraft:set_name", "name": {"translate": "item.test"}, "target": "item_name"}]),
        item("beef", functions=[{"function": "minecraft:furnace_smelt"}, const_count(3)]),
        item("potato", functions=[{"function": "minecraft:furnace_smelt"}]),
        item("apple", functions=[{"function": "minecraft:furnace_smelt"}])]}]}, 24)
    # item modifiers that write lore, tooltip flags, model data, container loot; filtered; styled names
    lore = lambda lines, **mode: {"function": "minecraft:set_lore", "lore": lines, **(mode or {"mode": "append"})}
    styled = {"text": "Gold", "color": "gold", "bold": True, "extra": [{"translate": "item.x", "with": ["a", 1]}]}
    add({"type": "minecraft:chest", "pools": [{"rolls": 1, "entries": [
        item("stick", functions=[lore(["one", {"text": "two", "italic": False}])]),
        item("stick", functions=[lore(["a", "b", "c"]), lore(["X"], mode="insert", offset=1)]),
        item("stick", functions=[lore(["a", "b", "c"]), lore(["X", "Y"], mode="replace_section", offset=1, size=1)]),
        item("stick", functions=[lore(["a", "b", "c"]), lore(["X"], mode="replace_section", offset=2)]),
        item("stick", functions=[lore(["a", "b"]), lore(["X"], mode="insert", offset=5)]),
        item("stick", functions=[lore(["a", "b"]), lore(["X"], mode="replace_all")]),
        item("stick", functions=[lore(["a"]), lore([], mode="replace_all")]),
        item("stick", functions=[lore(["line"] * 200), lore(["more"] * 100)]),
        item("stick", functions=[lore([styled])]),
        item("stick", functions=[{"function": "minecraft:set_name", "name": styled}]),
        item("stick", functions=[{"function": "minecraft:set_name", "name": {"text": "", "extra": ["x", {"keybind": "key.jump"}]}, "target": "item_name"}]),
        item("diamond_sword", functions=[{"function": "minecraft:toggle_tooltips", "toggles": {"minecraft:attribute_modifiers": False, "minecraft:enchantments": False}}]),
        item("diamond_sword", functions=[{"function": "minecraft:set_enchantments", "enchantments": {"minecraft:sharpness": 2}},
                                         {"function": "minecraft:toggle_tooltips", "toggles": {"minecraft:enchantments": False, "minecraft:unbreakable": False, "minecraft:trim": True}}]),
        item("stick", functions=[{"function": "minecraft:toggle_tooltips", "toggles": {"minecraft:enchantments": True}}]),
        item("music_disc_cat", functions=[{"function": "minecraft:toggle_tooltips", "toggles": {"minecraft:jukebox_playable": False}}]),
        item("iron_chestplate", functions=[{"function": "minecraft:set_components", "components": {"minecraft:trim": {"material": "minecraft:gold", "pattern": "minecraft:coast"}}},
                                           {"function": "minecraft:toggle_tooltips", "toggles": {"minecraft:trim": False}}]),
        item("stick", functions=[{"function": "minecraft:set_custom_model_data", "value": {"type": "minecraft:uniform", "min": 0, "max": 100}}]),
        item("chest", functions=[{"function": "minecraft:set_loot_table", "name": "minecraft:chests/simple_dungeon", "type": "minecraft:chest"}]),
        item("barrel", functions=[{"function": "minecraft:set_loot_table", "name": "minecraft:chests/igloo_chest", "seed": -42, "type": "minecraft:barrel"}]),
        item("stick", functions=[{"function": "minecraft:filtered", "item_filter": {"items": "minecraft:stick"}, "modifier": const_count(7)}]),
        item("apple", functions=[{"function": "minecraft:filtered", "item_filter": {"items": "minecraft:stick"}, "modifier": const_count(7)}]),
        item("stick", functions=[{"function": "minecraft:filtered", "item_filter": {"count": {"min": 2}},
                                  "modifier": [const_count({"type": "minecraft:uniform", "min": 1, "max": 3}), lore(["filtered"])]}])]}]}, 48)
    add({"type": "minecraft:chest", "pools": [{"rolls": 3, "entries": [
        item("arrow", functions=[[const_count({"type": "minecraft:uniform", "min": 1, "max": 3}), {"function": "minecraft:limit_count", "limit": 2}], const_count(5, add=True)])]}]}, 12)

    # entity conditions
    for fire in (zombie, burning):
        for attacker in (None, sword):
            fields = {"te": fire, "ds": player_hit}
            if attacker is not None:
                fields["ae"] = attacker
            add({"type": "minecraft:entity", "pools": [{"rolls": 2, "entries": [
                item("blaze_rod", conditions=[{"condition": "minecraft:entity_properties", "entity": "this", "predicate": {"flags": {"is_on_fire": True}}}]),
                item("diamond_sword", conditions=[{"condition": "minecraft:entity_properties", "entity": "attacker",
                    "predicate": {"equipment": {"mainhand": {"items": "#minecraft:swords", "predicates": {"minecraft:enchantments": [{"enchantments": "minecraft:looting", "levels": {"min": 2}}]}}}}}]),
                item("bone", conditions=[{"condition": "minecraft:random_chance_with_enchanted_bonus", "unenchanted_chance": 0.2,
                    "enchanted_chance": {"type": "minecraft:linear", "base": 0.3, "per_level_above_first": 0.2}, "enchantment": "minecraft:looting"}]),
                item("leather", conditions=[{"condition": "minecraft:entity_properties", "entity": "this", "predicate": {"type": ["minecraft:zombie", "minecraft:husk"]}}],
                    functions=[{"function": "minecraft:enchanted_count_increase", "enchantment": "minecraft:looting", "count": {"type": "minecraft:uniform", "min": 0, "max": 1}, "limit": 3}]),
                item("rotten_flesh", conditions=[{"condition": "minecraft:damage_source_properties", "predicate": {"is_direct": True}}]),
                item("iron_nugget", conditions=[{"condition": "minecraft:damage_source_properties", "predicate": {"tags": [{"id": "minecraft:is_projectile", "expected": False}]}}]),
                item("gold_nugget", conditions=[{"condition": "minecraft:killed_by_player"}])]}]}, 6, **fields)
    # LevelBasedValue variants through the looting bonus chance
    attackers = [entity(type="minecraft:zombie", mainhand=stack("diamond_sword", enchantments={"looting": n}))
                 for n in (1, 2, 3, 5)]
    for chance in (
        0.4,
        0.6,
        {"type": "minecraft:linear", "base": 0.2, "per_level_above_first": 0.15},
        {"type": "minecraft:levels_squared", "added": -3.0},
        {"type": "minecraft:clamped", "value": {"type": "minecraft:linear", "base": 0.0, "per_level_above_first": 0.3}, "min": 0.1, "max": 0.7},
        {"type": "minecraft:fraction", "numerator": {"type": "minecraft:linear", "base": 1.0, "per_level_above_first": 1.0}, "denominator": 8.0},
        {"type": "minecraft:lookup", "values": [0.1, 0.9], "fallback": {"type": "minecraft:linear", "base": 0.5, "per_level_above_first": 0.0}},
    ):
        for attacker in attackers:
            add({"type": "minecraft:entity", "pools": [{"rolls": 6, "entries": [
                item("bone", conditions=[{"condition": "minecraft:random_chance_with_enchanted_bonus", "unenchanted_chance": 0.25,
                                          "enchanted_chance": chance, "enchantment": "minecraft:looting"}])]}]}, 2,
                te=zombie, ae=attacker, ds=player_hit)
    # item predicates: damage, count, id lists, stored enchantments
    for tool in (0, 1, 13, 14, 15, 5):
        add({"type": "minecraft:block", "pools": [{"rolls": 1, "entries": [
            item("stone", conditions=[{"condition": "minecraft:match_tool", "predicate": {"predicates": {"minecraft:damage": {"damage": {"min": 10}}}}}]),
            item("dirt", conditions=[{"condition": "minecraft:match_tool", "predicate": {"count": {"min": 2}}}]),
            item("sand", conditions=[{"condition": "minecraft:match_tool", "predicate": {"items": ["minecraft:diamond_pickaxe", "minecraft:iron_pickaxe"]}}]),
            item("gravel", conditions=[{"condition": "minecraft:match_tool", "predicate": {"predicates": {"minecraft:stored_enchantments": [{"enchantments": "minecraft:fortune", "levels": {"min": 1}}]}}}]),
            item("clay", conditions=[{"condition": "minecraft:match_tool", "predicate": {"predicates": {"minecraft:damage": {"durability": {"max": 1000}}}}}]),
            item("flint", conditions=[{"condition": "minecraft:match_tool"}]),
            item("coal", conditions=[{"condition": "minecraft:table_bonus", "enchantment": "minecraft:fortune", "chances": [0.1, 0.5, 0.9]}])]}]}, 3,
            k=tool, b=block_state("minecraft:stone"))
    # location checks that match: block predicate with states at an offset
    tall = {"blocks": [{"pos": [0, 64, 0], "state": {"name": "minecraft:tall_grass", "properties": {"half": "lower"}}},
                       {"pos": [0, 65, 0], "state": {"name": "minecraft:tall_grass", "properties": {"half": "upper"}}}]}
    for lv in (level(**tall), level(blocks=[{"pos": [0, 64, 0], "state": {"name": "minecraft:stone"}}]), level()):
        add({"type": "minecraft:chest", "pools": [{"rolls": 1, "entries": [
            item("short_grass", conditions=[{"condition": "minecraft:location_check", "predicate": {"block": {"blocks": "minecraft:tall_grass", "state": {"half": "lower"}}}, "offsetY": -1 + 1}]),
            item("tall_grass", conditions=[{"condition": "minecraft:location_check", "predicate": {"block": {"blocks": "minecraft:tall_grass", "state": {"half": "upper"}}}, "offsetY": 1}]),
            item("stone", conditions=[{"condition": "minecraft:location_check", "predicate": {"block": {"blocks": "#minecraft:base_stone_overworld"}}}]),
            item("apple", conditions=[{"condition": "minecraft:inverted", "term": {"condition": "minecraft:location_check", "predicate": {"block": {"state": {"half": "lower"}}}}}])]}]}, 2, lv=lv)
    # copy_state keeps only the properties the function's own block defines
    for state in (block_state("minecraft:wheat", {"age": "5"}),
                  block_state("minecraft:oak_stairs", {"facing": "north", "half": "top"}),
                  block_state("minecraft:farmland", {"moisture": "3"})):
        add({"type": "minecraft:block", "pools": [{"rolls": 1, "entries": [
            item("wheat", functions=[{"function": "minecraft:copy_state", "block": "minecraft:wheat",
                                      "properties": ["age", "facing", "no_such_property"]}]),
            item("oak_stairs", functions=[{"function": "minecraft:copy_state", "block": "minecraft:oak_stairs",
                                           "properties": ["facing", "half", "age"]}])]}]}, 3, k=1, b=state)
    # fill: the container path used by chests
    return cases


def fill_cases():
    cases = []
    for key, table in TABLES.items():
        if table.get('type') != 'minecraft:chest':
            continue
        bare = key[len('minecraft:'):]
        for n in range(1, 4):
            cases.append({'t': bare, 's': seed_for(bare, 100 + n), 'f': 27})
        cases.append({'t': bare, 's': seed_for(bare, 200), 'f': 27, 'fe': {'3': stack('stick', 5), '4': stack('cobblestone', 64), '26': stack('apple', 2)}})
        cases.append({'t': bare, 's': seed_for(bare, 300), 'f': 9})
    return cases


def sequence_cases(cases):
    """Re-roll a sample of the vanilla cases through named random sequences."""
    out = []
    picked = {}
    for case in cases:
        key = case.get("t")
        if key is None or "f" in case or key in picked:
            continue
        if "random_sequence" not in TABLES["minecraft:" + key]:
            continue
        picked[key] = case
    for index, (key, case) in enumerate(sorted(picked.items())):
        if index % 7:
            continue
        for world_seed, defaults in ((0, None), (123456789, None), (-987654321987, (5, True, True)), (42, (3, False, True))):
            sequence = {"world_seed": world_seed, "rolls": 3, "id": TABLES["minecraft:" + key]["random_sequence"]}
            if defaults:
                sequence.update(salt=defaults[0], include_world_seed=defaults[1], include_sequence_id=defaults[2])
            out.append({**case, "s": 1, "sq": sequence})
    return out


def expand(case):
    scenario = {
        "name": f'{case.get("t", "custom" + str(case.get("ct")))}#{case["s"]}',
        "table": "minecraft:" + case.get("t", "custom"),
        "seed": case["s"],
        "luck": case.get("l", 0.0),
        "level": LEVELS[case.get("lv", 0)],
        "params": {} if case.get("t") == "gameplay/piglin_bartering" else {"origin": [0.5, 64.5, 0.5]},
    }
    p = scenario["params"]
    if "ct" in case:
        scenario["table_json"] = CUSTOM_TABLES[case["ct"]]
    if "f" in case:
        scenario["fill"] = case["f"]
    if "fe" in case:
        scenario["fill_existing"] = case["fe"]
    if "sq" in case:
        scenario["sequence"] = case["sq"]
    if "k" in case:
        p["tool"] = TOOLS[case["k"]]
    if "b" in case:
        p["block_state"] = BLOCK_STATES[case["b"]]
    if "x" in case:
        p["explosion_radius"] = case["x"]
    if "be" in case:
        p["block_entity"] = case["be"]
    if "te" in case:
        p["this_entity"] = ENTITIES[case["te"]]
    if "ae" in case:
        p["attacking_entity"] = ENTITIES[case["ae"]]
    if "lp" in case:
        p["last_damage_player"] = True
    if "ds" in case:
        p["damage_source"] = DAMAGE_SOURCES[case["ds"]]
    return scenario


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--oracle-dir", type=Path, required=True)
    parser.add_argument("--out", type=Path, default=ROOT / "fixtures/loot-oracle-vectors.json")
    parser.add_argument("--work", type=Path, default=Path("/tmp"))
    args = parser.parse_args()

    cases = block_cases() + entity_cases() + other_cases() + synthetic_cases() + fill_cases()
    cases += sequence_cases(cases)
    scenarios = [expand(case) for case in cases]
    work = args.work
    (work / "loot-scenarios.json").write_text(json.dumps(scenarios))
    classpath = (args.oracle_dir / "cp.txt").read_text().strip()
    subprocess.run(
        ["java", "--add-exports", "java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED",
         "-cp", f"{args.oracle_dir}:{classpath}", "loot_oracle", "run",
         str(work / "loot-scenarios.json"), str(work / "loot-results.json")],
        check=True,
    )
    results = json.loads((work / "loot-results.json").read_text())
    kept, skipped = [], []
    for case, result in zip(cases, results):
        if "error" in result:
            skipped.append((case.get("t", "custom" + str(case.get("ct"))), result["error"]))
            continue
        case["r"] = ",".join(result["stacks"]) + "|" + str(result["rng_next"])
        if "f" in case:
            case["r"] = ",".join(result["stacks"])
        if "sq" in case:
            case["r"] = ";".join(result["stacks"]) + "|" + str(result["rng_next"])
        kept.append(case)
    out = {
        "format": "cinderstone-loot-oracle-v1",
        "minecraft": "1.21",
        "source": {
            "server_jar_sha1": "450698d1863ab5180c25d7c804ef0fe6369dd1ba",
            "inner_jar_sha256": "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01",
            "mappings_sha1": "31c77994d96f05ba25a870ada70f47f315330437",
            "oracle": "tools/loot_oracle.java run (unchanged original loot classes)",
            "result": "Slot wire bytes of each produced stack (empty stacks are 00), then the next nextInt() draw",
        },
        "tools": TOOLS,
        "entities": ENTITIES,
        "damage_sources": DAMAGE_SOURCES,
        "levels": LEVELS,
        "block_states": BLOCK_STATES,
        "custom_tables": CUSTOM_TABLES,
        "cases": kept,
    }
    args.out.write_text(json.dumps(out, separators=(",", ":")))
    errors = {}
    for table, error in skipped:
        errors.setdefault(error.split(":")[0] + ":" + error.split(":", 1)[1][:80], []).append(table)
    print(f"cases={len(cases)} kept={len(kept)} skipped={len(skipped)}")
    for error, tables in sorted(errors.items(), key=lambda kv: -len(kv[1]))[:12]:
        print(len(tables), error, tables[:3])


if __name__ == "__main__":
    sys.exit(main())
