#!/usr/bin/env python3
"""Generate fixtures/datapack-tags-vectors.json and fixtures/datapack-vanilla-tags-1.21.json.

The original `TagLoader` (tools/loot_oracle.java `tags`) loads folder packs from disk; `export-tags`
copies the shipped vanilla `data/minecraft/tags/**` JSON. The Rust loader must resolve the same tags
with the same members in the same order, including cycles, missing and optional references,
`replace` across packs and invalid files.

    python3 tools/datapack_generate.py --oracle-dir /path/outside/git [--scenarios 600]
"""
import argparse
import json
import random
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
NAMESPACES = ["minecraft", "mod", "pack_a"]
KNOWN = [f"{ns}:{name}" for ns in NAMESPACES for name in ["stone", "dirt", "oak/log", "a_b", "x.y", "zz", "q-1", "k"]]
TAGS = [f"{ns}:{name}" for ns in NAMESPACES for name in ["blocks", "logs", "logs/oak", "ores", "all", "a", "b", "c", "d", "e"]]
INVALID = ["not json", "", "{", "[]", "{}", '{"values": 5}', '{"values": [1]}', '{"values": ["Bad Id!"]}', '{"values": ["#"]}',
           '{"values": [], "replace": "yes"}', '{"values": [{"id": 5}]}', '{"values": [{"required": false}]}',
           '{"values": [{"id": "minecraft:stone", "required": 1}]}', "{'values': ['minecraft:stone']}", '{values: []}',
           '{"values": ["minecraft:stone"]} trailing', "null", '{"values": null}']


def entry(rng, tags, own):
    kind = rng.random()
    name = rng.choice(KNOWN) if rng.random() < 0.8 else rng.choice(["minecraft:nothing", "mod:ghost"])
    # A tag that names itself makes the original recurse forever once anything else depends on it;
    # keep that rare so most scenarios exercise the resolver rather than the crash.
    others = [tag for tag in tags if tag != own] or tags
    target = (rng.choice(tags) if rng.random() < 0.03 else rng.choice(others)) if rng.random() < 0.9 else rng.choice(["minecraft:ghost", "mod:phantom"])
    if kind < 0.45:
        return name
    if kind < 0.75:
        return "#" + target
    if kind < 0.85:
        return {"id": name, "required": rng.random() < 0.5}
    if kind < 0.95:
        return {"id": "#" + target, "required": rng.random() < 0.5}
    return rng.choice([name, "#" + target])


def tag_file(rng, tags, own):
    if rng.random() < 0.06:
        return rng.choice(INVALID)
    document = {"values": [entry(rng, tags, own) for _ in range(rng.randint(0, 6))]}
    if rng.random() < 0.25:
        document["replace"] = rng.random() < 0.7
    if rng.random() < 0.05:
        document["extra"] = 1
    return json.dumps(document)


def scenario(rng):
    tags = rng.sample(TAGS, rng.randint(2, 14))
    packs = []
    for index in range(rng.randint(1, 3)):
        chosen = rng.sample(tags, rng.randint(1, len(tags)))
        packs.append({"id": f"pack{index}", "files": {tag: tag_file(rng, tags, tag) for tag in chosen}})
    return {"known": rng.sample(KNOWN, rng.randint(4, len(KNOWN))), "packs": packs}


def run(oracle_dir, work, mode, files):
    work.mkdir(parents=True, exist_ok=True)
    classpath = (oracle_dir / "cp.txt").read_text().strip()
    subprocess.run(
        ["java", "--add-exports", "java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED",
         "-cp", f"{oracle_dir}:{classpath}", "loot_oracle", mode, *files],
        check=True,
    )


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--oracle-dir", type=Path, required=True)
    parser.add_argument("--work", type=Path, default=Path("/tmp"))
    parser.add_argument("--scenarios", type=int, default=600)
    args = parser.parse_args()
    rng = random.Random(1021)
    scenarios = [scenario(rng) for _ in range(args.scenarios)]
    source, target = args.work / "tags-in.json", args.work / "tags-out.json"
    args.work.mkdir(parents=True, exist_ok=True)
    source.write_text(json.dumps(scenarios))
    run(args.oracle_dir, args.work, "tags", [str(source), str(target)])
    results = json.loads(target.read_text())
    origin = {
        "server_jar_sha1": "450698d1863ab5180c25d7c804ef0fe6369dd1ba",
        "inner_jar_sha256": "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01",
        "mappings_sha1": "31c77994d96f05ba25a870ada70f47f315330437",
        "oracle": "tools/loot_oracle.java tags (unchanged TagLoader over folder packs on disk) / export-tags",
    }
    (ROOT / "fixtures/datapack-tags-vectors.json").write_text(json.dumps({
        "format": "cinderstone-datapack-tags-v1", "minecraft": "1.21", "source": origin,
        "result": "expect: tag id -> members in collection order for every tag the original resolved; error: StackOverflowError where the original recurses forever",
        "cases": [({**s, "expect": r["tags"]} if "tags" in r else {**s, "error": r["error"]}) for s, r in zip(scenarios, results)],
    }, separators=(",", ":")))
    vanilla = args.work / "vanilla-tags.json"
    run(args.oracle_dir, args.work, "export-tags", [str(vanilla)])
    (ROOT / "fixtures/datapack-vanilla-tags-1.21.json").write_text(json.dumps({
        "format": "cinderstone-datapack-vanilla-tags-v1", "minecraft": "1.21", "source": origin,
        "result": "shipped data/<namespace>/tags/<directory>/** JSON text by directory (the first path segment) and tag id",
        "directories": json.loads(vanilla.read_text()),
    }, separators=(",", ":")))
    resolved = sum(len(r["tags"]) for r in results if "tags" in r)
    print(f"scenarios={len(scenarios)} resolved_tags={resolved} overflows={sum(1 for r in results if 'error' in r)}")


if __name__ == "__main__":
    main()
