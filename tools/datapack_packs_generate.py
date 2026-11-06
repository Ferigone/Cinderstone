#!/usr/bin/env python3
"""Generate fixtures/datapack-packs-vectors.json.

The original `PackRepository` (tools/loot_oracle.java `packs`) discovers packs in a prepared
`datapacks` folder. Each case materialises a folder (directories, zip archives, stray files) and
records what the original found and how selection changes behave. The Rust repository must agree.

    python3 tools/datapack_packs_generate.py --oracle-dir /path/outside/git [--sample 4]
"""
import argparse
import base64
import copy
import io
import json
import random
import shutil
import subprocess
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

BASES = [
    {"pack": {"description": "plain", "pack_format": 48}},
    {"pack": {"description": {"text": "styled", "color": "gold", "extra": ["x"]}, "pack_format": 48}},
    {"pack": {"description": "range single", "pack_format": 10, "supported_formats": 48}},
    {"pack": {"description": "range pair", "pack_format": 45, "supported_formats": [40, 50]}},
    {"pack": {"description": "range object", "pack_format": 45, "supported_formats": {"min_inclusive": 1, "max_inclusive": 60}}},
    {"pack": {"description": "format outside range", "pack_format": 30, "supported_formats": [40, 50]}},
    {"pack": {"description": "too new", "pack_format": 99}},
    {"pack": {"description": "too old", "pack_format": 3}},
    {"pack": {"description": "features", "pack_format": 48}, "features": {"enabled": ["minecraft:bundle", "trade_rebalance"]}},
    {"pack": {"description": "vanilla feature", "pack_format": 48}, "features": {"enabled": ["minecraft:vanilla"]}},
    {"pack": {"description": "overlays", "pack_format": 48}, "overlays": {"entries": [
        {"formats": [40, 50], "directory": "ov-a"}, {"formats": 48, "directory": "ov_b.1"}, {"formats": {"min_inclusive": 1, "max_inclusive": 9}, "directory": "old"}]}},
]
WRONG = ["x", "", 1, 47, 48, 2.5, -1, True, None, [], {}, [1], [1, 2], [2, 1], ["a"], {"a": 1}, "minecraft:bundle", "bad dir!"]
TEXT_CORRUPTIONS = [
    lambda t: t[:len(t) // 2], lambda t: t + ",", lambda t: "﻿" + t, lambda t: t.replace('"', "'"), lambda t: "// c\n" + t,
    lambda t: t.replace("{", "[", 1), lambda t: t + " {}", lambda t: "", lambda t: " " + t + "\n", lambda t: t.replace('"pack"', "pack", 1),
    lambda t: t.replace(",", ";", 1), lambda t: "[" + t + "]", lambda t: "null", lambda t: '{"pack": null}', lambda t: '{"pack": []}',
    # Gson accepts raw control characters inside strings and keeps numbers of any size.
    lambda t: t.replace('": "', '": "\t', 1), lambda t: t.replace('": "', '": "\n', 1), lambda t: t.replace('": "', '": "\u0000', 1),
    lambda t: t.replace('": "', '": "\u001f', 1), lambda t: t.replace('": "', '": "\u2028', 1),
    lambda t: t.replace("48", "1e400", 1), lambda t: t.replace("48", "-1e400", 1), lambda t: t.replace("48", "1E40", 1),
    lambda t: t.replace("48", "4294967344", 1), lambda t: t.replace("48", "48.9", 1), lambda t: t.replace("48", "4.8e1", 1),
    lambda t: t.replace("48", "0.0048e4", 1), lambda t: t.replace("48", "03", 1), lambda t: t.replace("48", "+48", 1),
    lambda t: t.replace("48", "9223372036854775808", 1), lambda t: t.replace("48", "1E999", 1),
]


def walk(node, path=()):
    yield path, node
    if isinstance(node, dict):
        for key, value in node.items():
            yield from walk(value, path + (key,))
    elif isinstance(node, list):
        for index, value in enumerate(node):
            yield from walk(value, path + (index,))


def edit(root, path, value, remove=False):
    root = copy.deepcopy(root)
    parent = root
    for step in path[:-1]:
        parent = parent[step]
    if remove:
        del parent[path[-1]]
    else:
        parent[path[-1]] = value
    return root


def mutants(base):
    out = []
    for path, node in walk(base):
        if not path:
            continue
        out.append(edit(base, path, None, remove=True))
        for wrong in WRONG:
            if wrong != node:
                out.append(edit(base, path, wrong))
    return out


def zip_bytes(entries, method=zipfile.ZIP_DEFLATED):
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w", method) as archive:
        for name, data in entries.items():
            archive.writestr(name, data)
    return buffer.getvalue()


def metadata_case():
    seen, texts = set(), []
    def add(text):
        if text not in seen:
            seen.add(text)
            texts.append(text)
    for base in BASES:
        add(json.dumps(base))
        for mutant in mutants(base):
            add(json.dumps(mutant))
        for corrupt in TEXT_CORRUPTIONS:
            add(corrupt(json.dumps(base)))
    files = {}
    for index, text in enumerate(texts):
        files[f"p{index:04d}/pack.mcmeta"] = text
    return {"name": "metadata", "files": files, "steps": [], "texts": len(texts)}


def structure_case():
    good = json.dumps(BASES[0])
    files = {
        "dir_ok/pack.mcmeta": good,
        "dir_nometa/readme.txt": "x",
        "dir_empty_meta/pack.mcmeta": "",
        "Ünïcode ✓/pack.mcmeta": good,
        "with space/pack.mcmeta": good,
        "a.zip": ("bytes", zip_bytes({"pack.mcmeta": good})),
        "stored.zip": ("bytes", zip_bytes({"pack.mcmeta": good}, zipfile.ZIP_STORED)),
        "nested.zip": ("bytes", zip_bytes({"inner/pack.mcmeta": good, "inner/data/x": "y"})),
        "nometa.zip": ("bytes", zip_bytes({"data/x": "y"})),
        "garbage.zip": ("bytes", b"not a zip"),
        "empty.zip": ("bytes", b""),
        "notazip.txt": "x",
        "UPPER.ZIP": ("bytes", zip_bytes({"pack.mcmeta": good})),
        "z.zip.bak": ("bytes", zip_bytes({"pack.mcmeta": good})),
        "feat/pack.mcmeta": json.dumps(BASES[8]),
        "feat2/pack.mcmeta": json.dumps(BASES[9]),
        "zz_last/pack.mcmeta": good,
        "Aa_first/pack.mcmeta": good,
    }
    return {"name": "structure", "files": files, "steps": [
        {"op": "select", "ids": ["file/dir_ok", "vanilla", "file/feat", "missing", "file/dir_ok"]},
        {"op": "add", "id": "bundle"}, {"op": "add", "id": "bundle"}, {"op": "add", "id": "nope"},
        {"op": "remove", "id": "vanilla"}, {"op": "remove", "id": "vanilla"},
        {"op": "select", "ids": ["trade_rebalance", "file/feat2", "vanilla"]}, {"op": "reload"},
        {"op": "select", "ids": []}, {"op": "add", "id": "file/a.zip"}, {"op": "add", "id": "file/Ünïcode ✓"},
        {"op": "select", "ids": ["file/zz_last", "file/Aa_first", "file/nested.zip"]}, {"op": "reload"},
    ]}


def selection_case(rng):
    good = json.dumps(BASES[0])
    files = {f"s{i}/pack.mcmeta": good for i in range(6)}
    files["f1/pack.mcmeta"] = json.dumps(BASES[8])
    ids = [f"file/s{i}" for i in range(6)] + ["file/f1", "vanilla", "bundle", "trade_rebalance", "ghost"]
    steps = []
    for _ in range(60):
        kind = rng.choice(["select", "add", "remove", "reload"])
        if kind == "select":
            steps.append({"op": "select", "ids": [rng.choice(ids) for _ in range(rng.randint(0, 6))]})
        elif kind == "reload":
            steps.append({"op": "reload"})
        else:
            steps.append({"op": kind, "id": rng.choice(ids)})
    return {"name": "selection", "files": files, "steps": steps}


def materialise(base, case):
    folder = base / case["name"]
    if folder.exists():
        shutil.rmtree(folder)
    folder.mkdir(parents=True)
    for relative, content in case["files"].items():
        target = folder / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        if isinstance(content, tuple):
            target.write_bytes(content[1])
        else:
            target.write_text(content, encoding="utf-8")
    return folder


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--oracle-dir", type=Path, required=True)
    parser.add_argument("--work", type=Path, default=Path("/tmp"))
    parser.add_argument("--sample", type=int, default=4, help="keep every Nth metadata pack in the committed fixture (1 keeps all)")
    args = parser.parse_args()
    rng = random.Random(1021)
    cases = [metadata_case(), structure_case(), selection_case(rng), selection_case(rng)]
    cases[3]["name"] = "selection2"
    work = args.work / "packs"
    work.mkdir(parents=True, exist_ok=True)
    spec = []
    for case in cases:
        folder = materialise(work, case)
        spec.append({"dir": str(folder), "steps": case["steps"]})
    (args.work / "packs-in.json").write_text(json.dumps(spec))
    classpath = (args.oracle_dir / "cp.txt").read_text().strip()
    subprocess.run(
        ["java", "--add-exports", "java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED",
         "-cp", f"{args.oracle_dir}:{classpath}", "loot_oracle", "packs", str(args.work / "packs-in.json"), str(args.work / "packs-out.json")],
        check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    results = json.loads((args.work / "packs-out.json").read_text())
    out_cases = []
    for case, result in zip(cases, results):
        files = case["files"]
        if case["name"] == "metadata":
            keep = {k: v for k, v in files.items() if int(k[1:5]) % args.sample == 0}
            kept_ids = {f"file/{k.split('/')[0]}" for k in keep}
            result = {**result, "available": [p for p in result["available"] if not p["id"].startswith("file/") or p["id"] in kept_ids]}
            files = keep
        encoded = {k: ({"base64": base64.b64encode(v[1]).decode()} if isinstance(v, tuple) else v) for k, v in files.items()}
        out_cases.append({"name": case["name"], "files": encoded, "steps": case["steps"], "expect": result})
    origin = {
        "server_jar_sha1": "450698d1863ab5180c25d7c804ef0fe6369dd1ba",
        "inner_jar_sha256": "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01",
        "mappings_sha1": "31c77994d96f05ba25a870ada70f47f315330437",
        "oracle": "tools/loot_oracle.java packs (unchanged PackRepository with ServerPacksSource and FolderRepositorySource over a prepared folder)",
    }
    (ROOT / "fixtures/datapack-packs-vectors.json").write_text(json.dumps({
        "format": "cinderstone-datapack-packs-v1", "minecraft": "1.21", "source": origin,
        "result": "files (text or {base64}) are written under a temporary datapacks folder; expect: available packs and selection after each step",
        "cases": out_cases,
    }, separators=(",", ":"), ensure_ascii=False))
    # WorldDataConfiguration: bases and their one-step NBT mutants, decoded as level.dat is read.
    bases = [
        '{DataPacks:{Enabled:["vanilla","file/a"],Disabled:["bundle"]},enabled_features:["minecraft:vanilla","minecraft:bundle"]}',
        '{DataPacks:{Enabled:["vanilla"],Disabled:[]},enabled_features:["minecraft:vanilla"]}',
        '{DataPacks:{Enabled:[],Disabled:["a","b"]},enabled_features:["vanilla","bundle","trade_rebalance"]}',
        '{enabled_features:["minecraft:trade_rebalance"]}',
        '{DataPacks:{Enabled:["x"]}}',
        "{}",
    ]
    (args.work / "wdc-in.json").write_text(json.dumps(bases))
    subprocess.run(
        ["java", "--add-exports", "java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED",
         "-cp", f"{args.oracle_dir}:{classpath}", "loot_oracle", "world-data-config", str(args.work / "wdc-in.json"), str(args.work / "wdc-out.json")],
        check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    rows = json.loads((args.work / "wdc-out.json").read_text())
    (ROOT / "fixtures/datapack-config-vectors.json").write_text(json.dumps({
        "format": "cinderstone-datapack-config-v1", "minecraft": "1.21", "source": origin,
        "result": "[payload NBT hex, encoded NBT hex of the WorldDataConfiguration read from it (DEFAULT on failure)]",
        "cases": [[r["payload"], r["encoded"]] for r in rows],
    }, separators=(",", ":")))
    metadata = results[0]
    print(f"world_data_config_rows={len(rows)}")
    print(f"metadata_packs={cases[0]['texts']} discovered={sum(1 for p in metadata['available'] if p['id'].startswith('file/'))} "
          f"kept={len(out_cases[0]['files'])}")


if __name__ == "__main__":
    main()
