#!/usr/bin/env python3
"""Generate fixtures/saved-data-owners-vectors.json.

The original `CommandStorage` (`data/command_storage_<namespace>.dat`) and `ForcedChunksSavedData`
(`data/chunks.dat`) run over a real `DimensionDataStorage` and a temporary folder
(tools/loot_oracle.java `saved-data` and `saved-data-mutants`).

* scenarios: scripted `cs_set` / `cs_get` / `cs_keys` and `fc_apply` sequences, files put on disk
  before the first access, `get` / `compute` of `chunks`, `save` steps (the file as written, member
  order included) and `read_tag` at old data versions;
* mutants: one-step mutants of `chunks.dat` and `command_storage_<ns>.dat` payloads loaded by the
  original.

    python3 tools/saved_data_owners_generate.py --oracle-dir /path/outside/git [--sample 4]
"""
import argparse
import json
import random
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def op(kind, **extra):
    return {"op": kind, **extra}

import struct


# A small typed NBT value model so one value can be given to the original as SNBT and to the Rust
# test as bytes. ("b", 1) byte, ("s", 1) short, ("i", 1) int, ("l", 1) long, ("f", 1.5) float,
# ("d", 1.5) double, ("str", "x"), ("list", tag_id, [values]), ("c", {name: value}),
# ("ia", [ints]), ("ba", [bytes]).
def snbt(value):
    kind = value[0]
    if kind == "b":
        return f"{value[1]}b"
    if kind == "s":
        return f"{value[1]}s"
    if kind == "i":
        return f"{value[1]}"
    if kind == "l":
        return f"{value[1]}L"
    if kind == "f":
        return f"{value[1]}f"
    if kind == "d":
        return f"{value[1]}d"
    if kind == "str":
        return json.dumps(value[1])
    if kind == "list":
        return "[" + ",".join(snbt(v) for v in value[2]) + "]"
    if kind == "c":
        return "{" + ",".join(f'{json.dumps(k) if k != "" else "WRAP"}:{snbt(v)}' for k, v in value[1].items()) + "}"
    if kind == "ia":
        return "[I;" + ",".join(str(v) for v in value[1]) + "]"
    if kind == "ba":
        return "[B;" + ",".join(f"{v}b" for v in value[1]) + "]"
    raise ValueError(kind)


def payload(value):
    kind = value[0]
    if kind == "b":
        return struct.pack(">b", value[1])
    if kind == "s":
        return struct.pack(">h", value[1])
    if kind == "i":
        return struct.pack(">i", value[1])
    if kind == "l":
        return struct.pack(">q", value[1])
    if kind == "f":
        return struct.pack(">f", value[1])
    if kind == "d":
        return struct.pack(">d", value[1])
    if kind == "str":
        data = value[1].encode()
        return struct.pack(">H", len(data)) + data
    if kind == "list":
        return bytes([value[1]]) + struct.pack(">i", len(value[2])) + b"".join(payload(v) for v in value[2])
    if kind == "c":
        body = b""
        for key, member in value[1].items():
            data = key.encode()
            body += bytes([tag_id(member)]) + struct.pack(">H", len(data)) + data + payload(member)
        return body + b"\x00"
    if kind == "ia":
        return struct.pack(">i", len(value[1])) + b"".join(struct.pack(">i", v) for v in value[1])
    if kind == "ba":
        return struct.pack(">i", len(value[1])) + b"".join(struct.pack(">b", v) for v in value[1])
    raise ValueError(kind)


def tag_id(value):
    return {"b": 1, "s": 2, "i": 3, "l": 4, "f": 5, "d": 6, "ba": 7, "str": 8, "list": 9, "c": 10, "ia": 11}[value[0]]


def cs_set(ident, value):
    return op("cs_set", id=ident, snbt=snbt(value), hex=bytes([tag_id(value)]).hex() + payload(value).hex())


def compound(**members):
    return ("c", members)


EMPTY = ("c", {})


def put(name, snbt, **extra):
    return op("put_snbt", name=name, snbt=snbt, **extra)


def pack(x, z):
    """ChunkPos.asLong, as a signed 64-bit integer."""
    value = (x & 0xFFFFFFFF) | ((z & 0xFFFFFFFF) << 32)
    return value - (1 << 64) if value >= 1 << 63 else value


KEYS = ["a", "b", "c", "x", "y", "z", "foo", "bar", "baz", "key", "stored", "p/q", "p/r", "deep/er/path", "with.dot", "with-dash", "with_under",
        "n1", "n2", "n3", "n10", "n11", "n12", "n20", "alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta", "iota", "kappa", "lambda",
        "mu", "nu", "xi", "omicron", "pi", "rho", "sigma", "tau", "upsilon", "phi", "chi", "psi", "omega", "aa", "ab", "ac", "ba", "bb", "bc"]
NAMESPACES = ["minecraft", "ns", "other", "my_pack", "a", "zz", "x.y", "q-r"]
VALUES = [compound(v=("i", 1)), compound(s=("str", "text")), compound(a=("b", 1), b=("s", 2), c=("i", 3), d=("l", 4)),
          compound(l=("list", 3, [("i", 1), ("i", 2), ("i", 3)])), compound(nested=compound(x=("i", 1), y=compound(z=("i", 2)))),
          compound(f=("f", 1.5), d=("d", -2.0)), compound(arr=("ia", [1, 2]), bytes=("ba", [1])), EMPTY, {"": 0} and ("c", {"": ("i", 1)})]
SMALL = compound(v=("i", 1))


def command_storage_scenarios(rng):
    out = []
    add = lambda *steps: out.append({"world_seed": 1, "steps": list(steps)})

    add(op("cs_keys"), op("cs_get", id="ns:a"), op("cs_keys"), op("save"))
    add(cs_set("ns:a", compound(x=("i", 1))), op("cs_get", id="ns:a"), op("cs_get", id="ns:b"), op("cs_keys"), op("save"))
    add(cs_set("ns:a", compound(x=("i", 1))), cs_set("ns:a", EMPTY), op("cs_keys"), op("cs_get", id="ns:a"), op("save"))
    add(cs_set("ns:a", EMPTY), op("cs_keys"), op("save"))
    add(cs_set("minecraft:a", compound(x=("i", 1))), cs_set("a:minecraft", compound(y=("i", 1))), cs_set("other:z", compound(q=("b", 1))),
        cs_set("ns:b/c", compound(y=("str", "s"))), op("cs_keys"), op("save"))
    add(op("cs_get", id="ns:a"), op("cs_get", id="other:b"), op("cs_keys"), cs_set("ns:a", EMPTY), op("cs_keys"), op("save"))
    # Files on disk before the first access.
    add(put("command_storage_ns", "{contents:{a:{x:1},b:{y:2},c:{z:3},d:{w:4},e:{u:5}}}"), op("cs_keys"), op("cs_get", id="ns:c"), op("cs_keys"),
        cs_set("ns:f", compound(k=("i", 1))), op("cs_keys"), op("save"))
    add(put("command_storage_ns", "{contents:{a:1,b:\"s\",c:{x:1},d:[1,2]}}"), op("cs_get", id="ns:a"), op("cs_get", id="ns:c"), op("cs_keys"), op("save"))
    add(put("command_storage_ns", "{contents:{a:1}}"), op("cs_get", id="ns:a"), cs_set("ns:a", EMPTY), op("save"))
    add(put("command_storage_ns", "{}"), op("cs_get", id="ns:a"), op("cs_keys"), op("save"))
    add(put("command_storage_ns", "{contents:[]}"), op("cs_get", id="ns:a"), op("cs_keys"), op("save"))
    add(put("command_storage_ns", "{contents:{Bad:{x:1}}}"), op("cs_get", id="ns:a"), op("cs_keys"))
    add(put("command_storage_ns", "{contents:{WRAP:{x:1}}}"), op("cs_get", id="ns:a"), op("cs_keys"))
    add(put("command_storage_ns", "{contents:{good:{x:1}}}"), op("cs_get", id="ns:good"), cs_set("ns:Bad", compound(x=("i", 1))), op("cs_keys"))
    add(put("command_storage_ns", "{contents:{a:{x:1}}}", data_version=1), op("cs_get", id="ns:a"), op("save"))
    add(put("command_storage_ns", "{contents:{a:{x:1}}}", data_version=9999), op("cs_get", id="ns:a"), op("save"))
    add(put("command_storage_ns", "{contents:{a:{x:1}}}", gzip=False), op("cs_get", id="ns:a"), cs_set("ns:b", compound(y=("i", 1))), op("save"))
    add(put("command_storage_ns", "{contents:{a:{x:1}}}"), op("cs_keys"), op("cs_get", id="ns:a"), op("cs_keys"))
    add(put("command_storage_ns", "{contents:{a:{x:1}}}"), put("command_storage_other", "{contents:{b:{x:1}}}"), op("cs_get", id="other:b"), op("cs_get", id="ns:a"), op("cs_keys"))
    for version in (1, 1343, 3952):
        add(put("command_storage_ns", "{contents:{a:{x:1}}}", data_version=version), op("read_tag", name="command_storage_ns", version=3953))
    # Many keys: the table doubles, entries are removed (the table stays big), then more are added.
    pool = list(KEYS)
    add(*[cs_set(f"ns:{key}", compound(v=("i", 1))) for key in pool], op("cs_keys"), op("save"),
        *[cs_set(f"ns:{key}", EMPTY) for key in pool[::2]], op("cs_keys"), op("save"),
        *[cs_set(f"ns:{key}", compound(w=("i", 2))) for key in pool[::3]], op("cs_keys"), op("save"))
    for _ in range(40):
        steps = []
        namespaces = rng.sample(NAMESPACES, rng.randint(1, 4))
        for _ in range(rng.randint(5, 80)):
            kind = rng.random()
            ident = f"{rng.choice(namespaces)}:{rng.choice(KEYS)}"
            if kind < 0.5:
                steps.append(cs_set(ident, rng.choice(VALUES) if rng.random() < 0.8 else EMPTY))
            elif kind < 0.7:
                steps.append(op("cs_get", id=ident))
            elif kind < 0.85:
                steps.append(op("cs_keys"))
            else:
                steps.append(op("save"))
        steps += [op("cs_keys"), op("save")]
        out.append({"world_seed": 1, "steps": steps, "random": True})
    # Reload what a scenario wrote: the files exist, then a second round of accesses.
    for _ in range(10):
        namespace = rng.choice(NAMESPACES)
        keys = rng.sample(KEYS, rng.randint(1, 40))
        contents = ",".join(f'"{key}":{{v:{index}}}' for index, key in enumerate(keys))
        steps = [put(f"command_storage_{namespace}", "{contents:{" + contents + "}}"), op("cs_keys")]
        for _ in range(rng.randint(1, 30)):
            ident = f"{namespace}:{rng.choice(KEYS)}"
            steps.append(cs_set(ident, rng.choice(VALUES)) if rng.random() < 0.6 else op("cs_get", id=ident))
        steps += [op("cs_keys"), op("save")]
        out.append({"world_seed": 1, "steps": steps, "random": True})
    return out


def forced_chunks_scenarios(rng):
    out = []
    add = lambda *steps: out.append({"world_seed": 1, "steps": list(steps)})

    def apply(x, z, forced=True, quiet=False):
        # `quiet` leaves the whole set out of the answer; the next `save` still records it.
        extra = {"quiet": True} if quiet else {}
        return op("fc_apply", pos=pack(x, z), forced=forced, **extra)

    add(op("get", name="chunks"), op("compute", name="chunks"), op("save"))
    add(apply(0, 0), op("save"))
    add(apply(0, 0), apply(0, 0), apply(0, 0, False), apply(0, 0, False), op("save"))
    add(apply(1, 0), apply(0, 1), apply(-1, -1), apply(100000, -100000), apply(0, 0), op("save"), apply(1, 0, False), op("save"))
    add(op("compute", name="chunks"), op("set_dirty", name="chunks"), op("save"))
    add(put("chunks", "{Forced:[L;5L,1L,3L,0L,-1L,4294967296L]}"), op("get", name="chunks"), apply(7, 7), op("save"))
    add(put("chunks", "{Forced:[L;]}"), op("get", name="chunks"), apply(1, 1), op("save"))
    add(put("chunks", "{Forced:[L;1L,1L,1L]}"), op("get", name="chunks"), apply(1, 1, False), op("get", name="chunks"), op("save"))
    add(put("chunks", "{Forced:[I;1,2]}"), op("get", name="chunks"), apply(1, 1))
    add(put("chunks", "{Forced:5L}"), op("get", name="chunks"))
    add(put("chunks", "{}"), op("get", name="chunks"), apply(2, 2), op("save"))
    add(put("chunks", "{Forced:[L;1L]}", data_version=1), op("get", name="chunks"), op("save"))
    add(put("chunks", "{Forced:[L;1L]}", gzip=False), op("get", name="chunks"), apply(2, 2), op("save"))
    for version in (1, 1343, 3952, 3953, 4000):
        add(put("chunks", "{Forced:[L;1L,2L]}", data_version=version), op("read_tag", name="chunks", version=3953))
    # Growth and shrinking: a run of positions, then most of them removed.
    run = [(x, z) for x in range(-12, 13) for z in range(-6, 7)]
    add(*[apply(x, z, quiet=index % 40 != 0) for index, (x, z) in enumerate(run)], op("save"),
        *[apply(x, z, False, quiet=index % 40 != 0) for index, (x, z) in enumerate(run[:-5])], op("save"),
        *[apply(x, z, quiet=index % 10 != 0) for index, (x, z) in enumerate(run[::7])], op("save"))
    for _ in range(50):
        span = rng.choice([3, 8, 30, 200])
        steps = []
        held = []
        for _ in range(rng.randint(10, 260)):
            quiet = rng.random() < 0.7
            if held and rng.random() < 0.35:
                x, z = rng.choice(held)
                steps.append(apply(x, z, False, quiet))
            else:
                x, z = rng.randint(-span, span), rng.randint(-span, span)
                held.append((x, z))
                steps.append(apply(x, z, rng.random() < 0.9, quiet))
            if rng.random() < 0.04:
                steps.append(op("save"))
        steps.append(op("save"))
        out.append({"world_seed": 1, "steps": steps, "random": True})
    # Positions that collide in the table: multiples of large powers of two in one coordinate.
    for _ in range(20):
        steps = [apply(rng.choice([0, 65536, -65536, 1 << 20, 1 << 24]) * rng.randint(-3, 3), rng.choice([0, 1 << 16, 1 << 22]) * rng.randint(-3, 3),
                       rng.random() < 0.8) for _ in range(rng.randint(5, 60))]
        out.append({"world_seed": 1, "steps": steps + [op("save")], "random": True})
    return out


def mutant_specs():
    return [
        {"name": "chunks", "world_seed": 1, "bases": ["{Forced:[L;5L,1L,3L,0L,-1L,4294967296L,9223372036854775807L,-9223372036854775808L]}", "{Forced:[L;]}", "{}"]},
        {"name": "command_storage_ns", "world_seed": 1, "bases": [
            "{contents:{a:{x:1},b:{y:2},\"p/q\":{z:3}}}", "{contents:{}}", "{}", "{contents:{n1:{},n2:{v:[1,2]},n3:{s:\"t\"}}}"]},
    ]


def run_oracle(oracle_dir, work, mode, payload, name):
    work.mkdir(parents=True, exist_ok=True)
    (work / f"{name}-in.json").write_text(json.dumps(payload))
    classpath = (oracle_dir / "cp.txt").read_text().strip()
    subprocess.run(
        ["java", "--add-exports", "java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED", "-cp", f"{oracle_dir}:{classpath}", "loot_oracle", mode,
         str(work / f"{name}-in.json"), str(work / f"{name}-out.json")],
        check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    return json.loads((work / f"{name}-out.json").read_text())


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--oracle-dir", type=Path, required=True)
    parser.add_argument("--work", type=Path, default=Path("/tmp/saved-data-owners-work"))
    parser.add_argument("--sample", type=int, default=5, help="keep every Nth random scenario and mutant (1 keeps all)")
    parser.add_argument("--full-out", type=Path)
    args = parser.parse_args()
    rng = random.Random(20261106)
    hand_cs = command_storage_scenarios(rng)
    hand_fc = forced_chunks_scenarios(rng)
    scenarios = hand_cs + hand_fc
    results = run_oracle(args.oracle_dir, args.work, "saved-data", scenarios, "scenarios")
    rows = [{"world_seed": s["world_seed"], "steps": s["steps"], "expect": r, "random": bool(s.get("random"))} for s, r in zip(scenarios, results)]
    mutants = run_oracle(args.oracle_dir, args.work, "saved-data-mutants", mutant_specs(), "mutants")
    meta = {
        "oracle": "tools/loot_oracle.java saved-data / saved-data-mutants (unchanged CommandStorage and ForcedChunksSavedData over a DimensionDataStorage)",
        "generator": "tools/saved_data_owners_generate.py",
        "scenarios": len(rows),
        "mutants": len(mutants),
    }
    if args.full_out:
        args.full_out.write_text(json.dumps({"meta": meta, "scenarios": rows, "mutants": mutants}))
    stride = max(1, args.sample)
    def lean(row):
        # The Rust test compares the written stream (`raw`); the re-read tag is not needed.
        row = json.loads(json.dumps(row))
        for result in row["expect"]:
            for file in result.get("files", []):
                file.pop("nbt", None)
        return row

    randoms = [row for row in rows if row["random"]]
    keep = [lean(row) for row in rows if not row["random"]] + [lean(row) for row in randoms[::stride]]
    kept_mutants = mutants[::stride]
    meta["sampled_from"] = {"scenarios": len(rows), "mutants": len(mutants)}
    meta["scenarios"] = len(keep)
    meta["mutants"] = len(kept_mutants)
    (ROOT / "fixtures" / "saved-data-owners-vectors.json").write_text(
        json.dumps({"meta": meta, "scenarios": keep, "mutants": kept_mutants}, separators=(",", ":")) + "\n")
    print(f"{len(rows)} scenarios, {len(mutants)} mutants; kept {len(keep)} and {len(kept_mutants)}")


if __name__ == "__main__":
    main()
