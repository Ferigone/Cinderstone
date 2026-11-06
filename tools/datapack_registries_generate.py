#!/usr/bin/env python3
"""Generate fixtures/datapack-registry-vectors.json.

One-step mutants of every shipped element of the synchronized registries (enchantments excepted)
are decoded by the original's loading codec and re-encoded by its network codec
(tools/loot_oracle.java `registry-codec`). Each vector names the vanilla element it starts from
(fixtures/datapack-vanilla-registries-1.21.json) and one patch:

* ``{"path": [...], "set": value}`` replaces the value at ``path`` (keys and list indices);
* ``{"path": [...], "delete": true}`` removes an object member;
* ``{"text": "..."}`` replaces the whole file text (lenient-reader cases).

The expected result is ``{"ok": true, "nbt": hex}`` (``NbtIo.writeAnyTag`` of the network form) or
``{"ok": false}``. Only the fields a client receives are mutated for biomes: the generation and
spawn settings that the loading codec also reads are not validated by the port.

    python3 tools/datapack_registries_generate.py --oracle-dir /path/outside/git [--per-registry 600]
"""
import argparse
import copy
import json
import random
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
VANILLA = ROOT / "fixtures" / "datapack-vanilla-registries-1.21.json"
OUT = ROOT / "fixtures" / "datapack-registry-vectors.json"

REGISTRIES = [
    "worldgen/biome", "chat_type", "trim_pattern", "trim_material", "wolf_variant",
    "painting_variant", "dimension_type", "damage_type", "banner_pattern", "jukebox_song",
]
BIOME_NETWORK_KEYS = {"has_precipitation", "temperature", "temperature_modifier", "downfall", "effects"}

REPLACEMENTS = [
    None, "", "x", "Bad Id!", "minecraft:no_such_thing", "#minecraft:is_overworld", "minecraft:stone",
    0, -1, 1, 15, 16, 17, 2031, 2032, 4064, 4080, -2032, -2048, 2147483648, 1.5, -0.5, 1e-6, 3.0e7, 3.1e7, 1e40,
    True, False, [], {}, "never", "always", "fall_variants", "frozen", "swamp", "content", "sender",
    {"type": "minecraft:uniform", "min_inclusive": 0, "max_inclusive": 7},
    {"type": "minecraft:uniform", "min_inclusive": 5, "max_inclusive": 3},
    {"type": "minecraft:biased_to_bottom", "min_inclusive": 0, "max_inclusive": 16},
    {"type": "minecraft:constant", "value": 3},
    {"type": "minecraft:weighted_list", "distribution": []},
    {"sound_id": "minecraft:custom.sound"}, {"sound_id": "minecraft:custom.sound", "range": 12.5},
    {"text": "hi", "color": "red"}, {"translate": "a.b"}, ["minecraft:plains", "minecraft:desert"],
    ["minecraft:plains"], ["minecraft:no_biome"], {"color": "#00ff00", "bold": True}, {"italic": True},
    {"minecraft:iron": "iron_darker"}, {"minecraft:nope": "x"},
    {"type": "minecraft:ash"}, {"type": "minecraft:dust"},
]

TEXT_MUTANTS = [
    lambda t: t.rstrip().rstrip("}") + ",}",
    lambda t: t.rstrip()[:-1],
    lambda t: t + " trailing",
    lambda t: "// comment\n" + t,
    lambda t: t.replace('"', "'"),
    lambda t: "",
    lambda t: "[]",
    lambda t: "null",
]


def paths(value, prefix, registry):
    """Every path below the root (object members and list items)."""
    if isinstance(value, dict):
        for key, child in value.items():
            if not prefix and registry == "worldgen/biome" and key not in BIOME_NETWORK_KEYS:
                continue
            yield prefix + [key], child
            yield from paths(child, prefix + [key], registry)
    elif isinstance(value, list):
        for index, child in enumerate(value):
            yield prefix + [index], child
            yield from paths(child, prefix + [index], registry)


def patched(base, patch):
    if "text" in patch:
        return patch["text"]
    value = copy.deepcopy(base)
    parent = value
    for step in patch["path"][:-1]:
        parent = parent[step]
    last = patch["path"][-1]
    if patch.get("delete"):
        del parent[last]
    else:
        parent[last] = patch["set"]
    return json.dumps(value)


def mutants(registry, element_id, text, rng):
    base = json.loads(text)
    out = [{"id": element_id, "patch": {"text": text}}]
    for path, current in paths(base, [], registry):
        if isinstance(path[-1], str):
            out.append({"id": element_id, "patch": {"path": path, "delete": True}})
        for replacement in REPLACEMENTS:
            if replacement != current:
                out.append({"id": element_id, "patch": {"path": path, "set": replacement}})
    for objpath, current in [([], base)] + [(p, c) for p, c in paths(base, [], registry) if isinstance(c, dict)]:
        out.append({"id": element_id, "patch": {"path": objpath + ["extra_field"], "set": 1}})
    for mutate in TEXT_MUTANTS:
        out.append({"id": element_id, "patch": {"text": mutate(text)}})
    rng.shuffle(out)
    return out


def run_oracle(oracle_dir, work, payload):
    work.mkdir(parents=True, exist_ok=True)
    (work / "registry-codec-in.json").write_text(json.dumps(payload))
    classpath = (oracle_dir / "cp.txt").read_text().strip()
    subprocess.run(
        ["java", "--add-exports", "java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED", "-cp", f"{oracle_dir}:{classpath}", "loot_oracle",
         "registry-codec", str(work / "registry-codec-in.json"), str(work / "registry-codec-out.json")],
        check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    return json.loads((work / "registry-codec-out.json").read_text())


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--oracle-dir", type=Path, required=True)
    parser.add_argument("--work", type=Path, default=Path("/tmp/datapack-registries-work"))
    parser.add_argument("--per-registry", type=int, default=600, help="vectors kept per registry (0 keeps all)")
    parser.add_argument("--out", type=Path, default=OUT)
    parser.add_argument("--seed", type=int, default=22)
    args = parser.parse_args()
    rng = random.Random(args.seed)
    vanilla = json.loads(VANILLA.read_text())["registries"]
    vectors = []
    for registry in REGISTRIES:
        candidates = []
        for element_id, text in sorted(vanilla[registry].items()):
            candidates.extend(mutants(registry, element_id, text, rng))
        # Unchanged elements first, then an even sample of the rest.
        unchanged = [c for c in candidates if c["patch"].get("text") == vanilla[registry][c["id"]]]
        rest = [c for c in candidates if c not in unchanged]
        rng.shuffle(rest)
        if args.per_registry:
            rest = rest[: max(0, args.per_registry - len(unchanged))]
        for candidate in unchanged + rest:
            candidate["registry"] = registry
            vectors.append(candidate)
    payload = [{"registry": f"minecraft:{v['registry']}", "text": patched(json.loads(vanilla[v["registry"]][v["id"]]), v["patch"])} for v in vectors]
    results = run_oracle(args.oracle_dir, args.work, payload)
    rows = []
    for vector, result in zip(vectors, results):
        expect = {"ok": True, "nbt": result["nbt"]} if result["ok"] else {"ok": False}
        rows.append({"registry": vector["registry"], "id": vector["id"], "patch": vector["patch"], "expect": expect})
    doc = {
        "format": "datapack-registry-vectors-v1",
        "meta": {
            "oracle": "tools/loot_oracle.java registry-codec (unchanged RegistryDataLoader loading codecs over RegistryOps<JsonOps>, RegistrySynchronization network codecs over NbtOps)",
            "generator": "tools/datapack_registries_generate.py",
            "base": "fixtures/datapack-vanilla-registries-1.21.json",
            "per_registry": args.per_registry,
            "seed": args.seed,
        },
        "vectors": rows,
    }
    args.out.write_text(json.dumps(doc, separators=(",", ":")) + "\n")
    accepted = sum(1 for r in rows if r["expect"]["ok"])
    print(f"{len(rows)} vectors ({accepted} accepted) -> {args.out}")


if __name__ == "__main__":
    main()
