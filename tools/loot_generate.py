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


def expand(case):
    scenario = {
        "name": f'{case["t"]}#{case["s"]}',
        "table": "minecraft:" + case["t"],
        "seed": case["s"],
        "luck": case.get("l", 0.0),
        "level": LEVELS[case.get("lv", 0)],
        "params": {} if case["t"] == "gameplay/piglin_bartering" else {"origin": [0.5, 64.5, 0.5]},
    }
    p = scenario["params"]
    if "k" in case:
        p["tool"] = TOOLS[case["k"]]
    if "b" in case:
        p["block_state"] = BLOCK_STATES[case["b"]]
    if "x" in case:
        p["explosion_radius"] = case["x"]
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

    cases = block_cases() + entity_cases() + other_cases()
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
            skipped.append((case["t"], result["error"]))
            continue
        case["r"] = ",".join(result["stacks"]) + "|" + str(result["rng_next"])
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
