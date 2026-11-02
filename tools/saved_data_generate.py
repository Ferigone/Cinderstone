#!/usr/bin/env python3
"""Generate fixtures/saved-data-vectors.json and fixtures/saved-data-mutants.json.

The original `DimensionDataStorage`, `SavedData`, `RandomSequences` and `MapIndex` run over a real
temporary directory (tools/loot_oracle.java `saved-data` and `saved-data-mutants`).

* vectors: hand-written scenarios (lazy load, caching of a missing file, dirty tracking, unknown
  files left alone, plain and damaged files, data-version handling, the ids and draws the owners
  hand out). Each records the oracle's result for every step; `put_snbt` records the file bytes it
  produced so the Rust test can write the identical file.
* mutants: one-step mutants of the owners' payloads (members removed, every node replaced by 19
  wrong-typed tags), loaded through the original factories; the saved form after loading is the
  expected result.

    python3 tools/saved_data_generate.py --oracle-dir /path/outside/git [--sample 10]
"""
import argparse
import gzip
import json
import struct
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def put(name, snbt, **extra):
    return {"op": "put_snbt", "name": name, "snbt": snbt, **extra}


def put_hex(name, data):
    return {"op": "put_hex", "name": name, "hex": data.hex()}


def op(kind, **extra):
    return {"op": kind, **extra}


def nbt_int(key, value):
    return b"\x03" + struct.pack(">H", len(key)) + key.encode() + struct.pack(">i", value)


def nbt_compound(key, body):
    return b"\x0a" + struct.pack(">H", len(key)) + key.encode() + body + b"\x00"


def document(version=3953, map_id=5, root_name="", root_id=10):
    body = nbt_compound("data", nbt_int("map", map_id)) + (nbt_int("DataVersion", version) if version is not None else b"")
    return bytes([root_id]) + struct.pack(">H", len(root_name)) + root_name.encode() + body + b"\x00"


SEQ = ('{salt:3,include_world_seed:0b,include_sequence_id:1b,sequences:{"minecraft:a":{source:[L;1L,2L]},'
       '"minecraft:chests/b":{source:[L;0L,0L]},"ns:c":{source:[L;-5L,9223372036854775807L]}}}')


def scenarios():
    out = []
    add = lambda seed, *steps: out.append({"world_seed": seed, "steps": list(steps)})

    add(1, op("get", name="idcounts"), op("compute", name="idcounts"), op("free_map_id"), op("free_map_id"), op("free_map_id"),
        op("save"), op("read_tag", name="idcounts"))
    add(2, put("idcounts", "{map:5,foo:3L,bar:\"x\",baz:2s,f:2.7f,d:-1.5d,b:1b,neg:-3,big:5000000000L,nested:{a:1}}"),
        op("get", name="idcounts"), op("free_map_id"), op("save"))
    add(3, op("get", name="idcounts"), op("compute", name="idcounts"), op("save"), op("set_dirty", name="idcounts"), op("save"),
        op("get", name="idcounts"))
    add(4, put("idcounts", "{map:-1}"), op("free_map_id"), op("free_map_id"), op("save"))
    add(5, put("idcounts", "{map:2147483647}"), op("free_map_id"), op("save"))
    add(6, put("idcounts", "{map:7}"), op("get", name="idcounts"), op("save"), op("get", name="idcounts"))

    add(7, put("random_sequences", SEQ), op("get", name="random_sequences"), op("draw", id="minecraft:a", n=3),
        op("draw", id="minecraft:fresh", n=2), op("draw", id="chests/b", n=2), op("save"), op("get", name="random_sequences"))
    add(8, op("compute", name="random_sequences"), op("save"), op("draw", id="minecraft:x", n=1), op("save"))
    add(9, op("compute", name="random_sequences"), op("defaults", salt=77, include_world_seed=False, include_sequence_id=True),
        op("draw", id="minecraft:x", n=2), op("reset", id="minecraft:x"), op("draw", id="minecraft:x", n=2),
        op("reset", id="minecraft:y", salt=5, include_world_seed=False, include_sequence_id=False), op("draw", id="minecraft:y", n=2),
        op("save"))
    add(-4242, put("random_sequences", SEQ), op("compute", name="random_sequences"), op("clear"), op("save"), op("get", name="random_sequences"))
    add(10, put("random_sequences", "{sequences:{}}"), op("get", name="random_sequences"), op("save"))
    add(11, put("random_sequences", "{}"), op("get", name="random_sequences"))
    add(12, put("random_sequences", "{salt:5,sequences:{\"minecraft:a\":{source:[L;1L]}}}"), op("get", name="random_sequences"))
    add(13, put("random_sequences", "{salt:5L,include_world_seed:1,sequences:{\"a\":{source:[I;1,2]},\"B\":{source:[1L,2L]},\"c\":{source:[1b,2b]}}}"),
        op("get", name="random_sequences"))
    add(14, put("random_sequences", "{salt:2.9f,include_world_seed:5b,include_sequence_id:0b,sequences:{\"x:y\":{source:[L;3L,4L],extra:1}}}"),
        op("get", name="random_sequences"), op("draw", id="x:y", n=2))

    # Data versions and the one fixer (RandomSequenceSettingsFix at 3565).
    for version in (1343, 3564, 3565, 3953, 4000, 99999, None):
        add(15, put("random_sequences", "{salt:3,sequences:{}}", data_version=version), op("read_tag", name="random_sequences"),
            op("get", name="random_sequences"))
        add(15, put("idcounts", "{map:5}", data_version=version), op("read_tag", name="idcounts"), op("get", name="idcounts"))
    add(15, put("random_sequences", "{salt:3,sequences:{}}", data_version=3000), op("read_tag", name="random_sequences", version=3000),
        op("read_tag", name="random_sequences", version=3564), op("read_tag", name="random_sequences", version=3565))

    # Plain and damaged files.
    good = document()
    gz = gzip.compress(good)
    add(16, put_hex("idcounts", good), op("get", name="idcounts"), op("read_tag", name="idcounts"))
    add(16, put_hex("idcounts", gz), op("get", name="idcounts"))
    add(16, put_hex("idcounts", gz + b"trailing"), op("get", name="idcounts"))
    add(16, put_hex("idcounts", gzip.compress(good + b"\x00\x00\x00 trailing inside")), op("get", name="idcounts"))
    add(16, put_hex("idcounts", gz[:-4]), op("get", name="idcounts"))            # damaged trailer
    add(16, put_hex("idcounts", gz[:-8]), op("get", name="idcounts"))            # trailer missing
    add(16, put_hex("idcounts", gz[:len(gz) // 2]), op("get", name="idcounts"))  # truncated stream
    add(16, put_hex("idcounts", gz[:-8] + b"\x00" * 8), op("get", name="idcounts"))  # wrong CRC and size
    add(16, put_hex("idcounts", gz + gz), op("get", name="idcounts"))            # two members
    add(16, put_hex("idcounts", b""), op("get", name="idcounts"), op("compute", name="idcounts"), op("save"))
    add(16, put_hex("idcounts", b"\x1f"), op("get", name="idcounts"))
    add(16, put_hex("idcounts", b"\x1f\x8b"), op("get", name="idcounts"))
    add(16, put_hex("idcounts", b"\x0a"), op("get", name="idcounts"))
    add(16, put_hex("idcounts", document(root_id=9)), op("get", name="idcounts"))
    add(16, put_hex("idcounts", document(root_id=1)), op("get", name="idcounts"))
    add(16, put_hex("idcounts", document(root_id=0)), op("get", name="idcounts"))
    add(16, put_hex("idcounts", document(root_name="named")), op("get", name="idcounts"))
    add(16, put_hex("idcounts", gzip.compress(document(root_name="named"))), op("get", name="idcounts"))
    add(16, put_hex("idcounts", b"\x0a\x00"), op("get", name="idcounts"))
    add(16, put_hex("idcounts", b"\x0a\x00\x05ab"), op("get", name="idcounts"))
    add(16, put_hex("idcounts", b"\x0a\x00\x00" + nbt_int("DataVersion", 3953)), op("get", name="idcounts"))  # no end byte
    add(16, put_hex("idcounts", b"\x0a\x00\x00" + nbt_int("DataVersion", 3953) + b"\x00"), op("get", name="idcounts"))  # no data
    add(16, put_hex("idcounts", b"\x0a\x00\x00" + nbt_int("data", 3) + nbt_int("DataVersion", 3953) + b"\x00"), op("get", name="idcounts"))
    add(16, put_hex("random_sequences", b"\x0a\x00\x00" + nbt_int("data", 3) + nbt_int("DataVersion", 1343) + b"\x00"),
        op("read_tag", name="random_sequences"), op("get", name="random_sequences"))
    add(16, put_hex("random_sequences", b"\x0a\x00\x00" + nbt_int("DataVersion", 1343) + b"\x00"),
        op("read_tag", name="random_sequences"), op("get", name="random_sequences"))
    add(16, put_hex("random_sequences", b"\x0a\x00\x00" + nbt_int("DataVersion", 3564) + b"\x00"),
        op("read_tag", name="random_sequences"), op("get", name="random_sequences"), op("compute", name="random_sequences"))

    # Cache: a missing or unreadable file is cached as absent.
    add(17, op("get", name="idcounts"), put("idcounts", "{map:9}"), op("get", name="idcounts"), op("compute", name="idcounts"),
        op("get", name="idcounts"), op("free_map_id"), op("save"))
    add(17, put_hex("idcounts", b"\x1f"), op("get", name="idcounts"), op("compute", name="idcounts"), op("save"),
        op("set_dirty", name="idcounts"), op("save"), op("get", name="idcounts"))
    add(17, put("scoreboard", "{Objectives:[]}"), put("idcounts", "{map:1}"), op("compute", name="idcounts"), op("free_map_id"),
        op("save"), op("read_tag", name="idcounts"))
    add(17, put("idcounts", "{map:1}"), op("free_map_id"), op("save"), op("save"), op("free_map_id"), op("save"))
    return out


def run(oracle_dir, work, mode, spec_name, spec):
    work.mkdir(parents=True, exist_ok=True)
    source = work / f"{spec_name}-in.json"
    target = work / f"{spec_name}-out.json"
    source.write_text(json.dumps(spec))
    classpath = (oracle_dir / "cp.txt").read_text().strip()
    subprocess.run(
        ["java", "--add-exports", "java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED",
         "-cp", f"{oracle_dir}:{classpath}", "loot_oracle", mode, str(source), str(target)],
        check=True,
    )
    return json.loads(target.read_text())


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--oracle-dir", type=Path, required=True)
    parser.add_argument("--work", type=Path, default=Path("/tmp"))
    parser.add_argument("--sample", type=int, default=10, help="keep every Nth mutant in the committed fixture (1 keeps all)")
    args = parser.parse_args()
    source = {
        "server_jar_sha1": "450698d1863ab5180c25d7c804ef0fe6369dd1ba",
        "inner_jar_sha256": "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01",
        "mappings_sha1": "31c77994d96f05ba25a870ada70f47f315330437",
        "oracle": "tools/loot_oracle.java saved-data / saved-data-mutants (unchanged DimensionDataStorage, SavedData, RandomSequences, MapIndex over a real temporary directory)",
    }

    spec = scenarios()
    results = run(args.oracle_dir, args.work, "saved-data", "saved-data", spec)
    cases = [{"world_seed": s["world_seed"], "steps": s["steps"], "expect": r} for s, r in zip(spec, results)]
    (ROOT / "fixtures/saved-data-vectors.json").write_text(json.dumps({
        "format": "cinderstone-saved-data-oracle-v1", "minecraft": "1.21", "source": source,
        "result": "per step: the oracle result (present/dirty/data hex/draws/ids/files). put_snbt records the file bytes it wrote",
        "cases": cases,
    }, separators=(",", ":")))

    bases = [
        {"name": "idcounts", "world_seed": 5, "bases": ["{map:5}", "{map:5,foo:3,bar:2}", "{map:1,x:2L,y:3s,z:4.5f}", "{}"]},
        {"name": "random_sequences", "world_seed": 6, "bases": [
            "{salt:3,include_world_seed:0b,include_sequence_id:1b,sequences:{\"minecraft:a\":{source:[L;1L,2L]}}}",
            "{salt:0,include_world_seed:1b,include_sequence_id:1b,sequences:{\"minecraft:a\":{source:[L;1L,2L]},\"minecraft:b/c\":{source:[L;-3L,4L]}}}",
            "{sequences:{\"a\":{source:[L;0L,0L]}}}",
        ]},
    ]
    rows = run(args.oracle_dir, args.work, "saved-data-mutants", "saved-data-mutants", bases)
    kept = [[r["name"], r["world_seed"], r["payload"], r["present"], r.get("data")] if r["present"] else [r["name"], r["world_seed"], r["payload"], False]
            for index, r in enumerate(rows) if index % args.sample == 0]
    (ROOT / "fixtures/saved-data-mutants.json").write_text(json.dumps({
        "format": "cinderstone-saved-data-mutants-v1", "minecraft": "1.21", "source": source,
        "sampling": f"every {args.sample}th of {len(rows)} generated mutants",
        "result": "[name, world_seed, payload NBT hex (the `data` compound), loaded, save() NBT hex when loaded]",
        "cases": kept,
    }, separators=(",", ":")))
    print(f"scenarios={len(cases)} mutants={len(rows)} loaded={sum(1 for r in rows if r['present'])} kept={len(kept)}")


if __name__ == "__main__":
    main()
