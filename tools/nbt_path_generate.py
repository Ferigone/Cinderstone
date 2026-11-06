#!/usr/bin/env python3
"""Generate fixtures/nbt-path-vectors.json: NBT path parsing and operations.

The original `NbtPathArgument.NbtPath` (tools/loot_oracle.java `nbt-path`) parses each path
(hand-written ones and one-character mutants), then runs `get`, `countMatching`,
`getOrCreate(CompoundTag::new)`, `set`, `remove` and `insert` on a set of root compounds. Each
vector records whether the path parsed, the operation's result (count or matched tags) and the
root after the operation, or the stage at which it failed.

    python3 tools/nbt_path_generate.py --oracle-dir /path/outside/git [--per-path 8]
"""
import argparse
import json
import random
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "fixtures" / "nbt-path-vectors.json"

PATHS = [
    "a", "a.b", "a.b.c", "a[0]", "a[-1]", "a[5]", "a[-9]", "a[]", "a[].x", "a[][]", "a[0][1]", "list[{k:1}]",
    "list[{k:1}].v", "list[{}]", "list[{k:[1]}]", "list[{n:{m:2}}]", "{k:1}", "{k:1}.a", "{}", "{}.list[]",
    "\"a b\"", "'q'.r", "\"x\\\"y\"", "a{k:1}", "a{}", "obj{n:{m:2}}.m", "obj{n:{m:3}}", "arr[0]", "arr[-1]",
    "arr[]", "ints[1]", "longs[]", "bytes[2]", "missing", "missing.deeper", "missing[0]", "missing[]",
    "missing[{k:1}]", "missing{k:1}", "a.", ".a", "a..b", "a[", "a[x]", "a[0", "a[{k:1}", "[0]", "[]", "[{k:1}]",
    "a b", "a[0] b", "a]", "a}", "{k:1}{k:1}", "a.{k:1}", "a.[0]", "a[1.5]", "a[--1]", "a[2147483648]",
    "list[0].k", "list[-1].v", "list[].k", "list[{k:2}]", "deep.x.y.z", "deep.x[0]", "num", "num[0]", "str.x",
    "empty[]", "empty[0]", "empty[{k:1}]", "mixed[0]", "'single'", "\"\"", "''", "aé", "中",
]

ROOTS = [
    "{}",
    "{a:{b:{c:1}},list:[{k:1,v:'one'},{k:2,v:'two'},{k:1,v:'three'}],arr:[I;1,2,3],ints:[1,2,3]}",
    "{a:[1,2,3],obj:{n:{m:2,o:1}},bytes:[B;1b,2b,3b],longs:[L;5l,6l],num:5,str:'s',empty:[]}",
    "{a:[[1,2],[3]],\"a b\":1,q:{r:2},'x\"y':3,deep:{x:{y:{z:1}}},list:[]}",
    "{k:1,a:{x:1},list:[{k:[1,2]},{k:[3]},{n:{m:2}}],mixed:[{}],'q r':{a:1}}",
]

SET_VALUES = ["1", "2b", "{x:1}", "[1,2]", "'s'", "[I;7]", "1.5f", "{k:1}", "[]", "{}"]

INSERTS = [(0, ["9"]), (-1, ["9", "10"]), (1, ["{z:1}"]), (99, ["1"]), (-99, ["1"]), (0, ["'s'"]), (0, [])]

REPLACEMENTS = ["[", "]", "{", "}", ".", " ", "\"", "'", "0", "-", "a", ":"]


def mutants(text, rng, count):
    out = set()
    for i in range(len(text)):
        out.add(text[:i] + text[i + 1:])
        for r in REPLACEMENTS:
            out.add(text[:i] + r + text[i + 1:])
    out = sorted(out)
    rng.shuffle(out)
    return out[:count]


def run_oracle(oracle_dir, work, specs):
    work.mkdir(parents=True, exist_ok=True)
    (work / "nbt-path-in.json").write_text(json.dumps(specs))
    classpath = (oracle_dir / "cp.txt").read_text().strip()
    subprocess.run(
        ["java", "--add-exports", "java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED", "-cp", f"{oracle_dir}:{classpath}", "loot_oracle",
         "nbt-path", str(work / "nbt-path-in.json"), str(work / "nbt-path-out.json")],
        check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    return json.loads((work / "nbt-path-out.json").read_text())


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--oracle-dir", type=Path, required=True)
    parser.add_argument("--work", type=Path, default=Path("/tmp/nbt-path-work"))
    parser.add_argument("--per-path", type=int, default=6, help="mutants kept per hand-written path (0 keeps all)")
    parser.add_argument("--out", type=Path, default=OUT)
    parser.add_argument("--seed", type=int, default=9)
    parser.add_argument("--sample", type=int, default=8, help="keep every Nth vector in the output (1 keeps all)")
    args = parser.parse_args()
    rng = random.Random(args.seed)
    paths = list(PATHS)
    seen = set(paths)
    for path in PATHS:
        for mutant in mutants(path, rng, args.per_path or 10**9):
            if mutant not in seen:
                seen.add(mutant)
                paths.append(mutant)
    specs = []
    for path in paths:
        specs.append({"path": path})
        for root in ROOTS:
            for op in ("get", "count", "get_or_create", "remove"):
                specs.append({"path": path, "root": root, "op": op})
            for value in SET_VALUES:
                specs.append({"path": path, "root": root, "op": "set", "value": value})
            for index, values in INSERTS:
                specs.append({"path": path, "root": root, "op": "insert", "index": index, "values": values})
    results = run_oracle(args.oracle_dir, args.work, specs)
    rows = []
    for index, (spec, result) in enumerate(zip(specs, results)):
        if index % args.sample == 0:
            rows.append({**spec, "expect": result})
    doc = {
        "format": "nbt-path-vectors-v1",
        "meta": {
            "oracle": "tools/loot_oracle.java nbt-path (unchanged NbtPathArgument.NbtPath over TagParser-parsed roots)",
            "generator": "tools/nbt_path_generate.py",
            "per_path": args.per_path,
            "seed": args.seed,
            "sample": f"every {args.sample}th of {len(specs)} runs",
        },
        "vectors": rows,
    }
    args.out.write_text(json.dumps(doc, ensure_ascii=False, separators=(",", ":")) + "\n")
    ok = sum(1 for row in rows if row["expect"]["ok"])
    print(f"{len(rows)} vectors ({ok} ok) over {len(paths)} paths -> {args.out}")


if __name__ == "__main__":
    main()
