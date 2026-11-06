#!/usr/bin/env python3
"""Generate fixtures/tag-parser-vectors.json: SNBT accept/reject and parsed tags.

The original `TagParser.parseTag` (tools/loot_oracle.java `snbt`) parses hand-written SNBT
strings, typed number tokens and one-character mutants of both (each character deleted,
duplicated, or replaced by a character from a fixed set). Each vector is the input text and
either the parsed compound (`NbtIo.writeAnyTag`, hex) or a rejection.

    python3 tools/tag_parser_generate.py --oracle-dir /path/outside/git [--sample 4]
"""
import argparse
import json
import random
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "fixtures" / "tag-parser-vectors.json"

SEEDS = [
    "{}", "{ }", "{a:1}", "{a:1,}", "{a:1,b:2}", "{a : 1 , b : 2 }", "{\"a b\":1}", "{'k':'v'}",
    "{a:\"x\\\"y\"}", "{a:'x\\'y'}", "{a:'x\\\\y'}", "{a:\"x\\ny\"}", "{a:\"\"}", "{\"\":1}",
    "{a:{b:{c:[]}}}", "{a:[1,2,3]}", "{a:[1b,2b]}", "{a:[1,2b]}", "{a:[{},{x:1}]}", "{a:[[],[1]]}",
    "{a:[I;1,2,3]}", "{a:[B;1b,2b]}", "{a:[L;1l,2L]}", "{a:[I;]}", "{a:[B;1,2]}", "{a:[I;1b]}",
    "{a:[X;1]}", "{a:[\"I\";1]}", "{a:[ I;1]}", "{a:[I ;1]}", "{a:[I; 1 , 2 ]}",
    "{a:true,b:FALSE,c:True}", "{a:1b,b:-1B,c:127b,d:128b,e:-128b,f:-129b}", "{a:1s,b:32768s,c:-32768S}",
    "{a:2147483647,b:2147483648,c:-2147483648,d:-2147483649}", "{a:9223372036854775807l,b:9223372036854775808L}",
    "{a:1.5,b:1.,c:.5,d:1e5,e:1.5e-3,f:1E3,g:+1.5,h:-.5}", "{a:1.5f,b:1F,c:1e40f,d:.5f,e:1.f,f:-0f}",
    "{a:1.5d,b:1D,c:1e400d,d:1d,e:007,f:00.5,g:01b,h:-0}", "{a:1e5,b:1e,c:e5,d:1.5e+3,e:1.5.5}",
    "{a:abc,b:a-b_c.d+e,c:minecraft:stone}", "{a:x y}", "{a:1 2}", "{a:}", "{:1}", "{a}", "{a:1",
    "{a:1}}", "{a:1} ", " {a:1}", "{a:1} x", "[]", "1", "", "{a:[}", "{a:[1,]}", "{a:[,]}",
    "{a:1,a:2}", "{a:{x:1},a:{y:2}}", "{a:'unterminated}", "{a:\"bad\\escape\"}", "{a:[1,2,]}",
    "{id:\"minecraft:stone\",Count:1b,tag:{display:{Name:'{\"text\":\"x\"}'}}}",
    "{Pos:[0.0d,64.0d,0.0d],Motion:[0d,0d,0d],Rotation:[0f,0f],Tags:[\"a\",\"b\"]}",
    "{a:\u00e9,b:\"\u00e9\u4e2d\U0001F600\"}", "{a\u00a0:1}", "{\ta\n:\r1\u000b}", "{a:\u2003 1}",
    "{a:[L;-9223372036854775808L]}", "{a:[B;true,false]}", "{a:[I;2147483648]}",
]

TOKENS = [
    "0", "-0", "+0", "00", "1", "+1", "-1", "0b", "+1b", "1.", ".1", ".", "+.", "-.e1", "1e", "1e+", "1e-2",
    "1.5e2f", "1.5e2F", "NaN", "Infinity", "-Infinity", "1_000", "0x10", "1l", "1L", "1s", "1S", "1d", "1D",
    "1f", "1b", "1B", "tRuE", "fAlSe", "true1", "1true", "3.4028236e38f", "1.4e-46f", "4.9e-325d", "-",
    "+", "e", "E5", "5e5e5", "1..2", "..", "1.5ff", "1bb", "123456789012345678901234567890",
    "2147483647", "2147483648", "32767s", "32768s", "-32769s", "127b", "128b", "-128b", "-129b",
]

REPLACEMENTS = ["{", "}", "[", "]", ",", ":", ";", "\"", "'", "\\", " ", "a", "1", ".", "-", "b", "I", "e"]


def mutants(text, rng, per_seed):
    out = set()
    for i in range(len(text)):
        out.add(text[:i] + text[i + 1:])
        out.add(text[:i] + text[i] + text[i:])
        for r in REPLACEMENTS:
            out.add(text[:i] + r + text[i + 1:])
    out = sorted(out)
    rng.shuffle(out)
    return out[:per_seed]


def run_oracle(oracle_dir, work, texts):
    work.mkdir(parents=True, exist_ok=True)
    (work / "snbt-in.json").write_text(json.dumps(texts))
    classpath = (oracle_dir / "cp.txt").read_text().strip()
    subprocess.run(
        ["java", "--add-exports", "java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED", "-cp", f"{oracle_dir}:{classpath}", "loot_oracle",
         "snbt", str(work / "snbt-in.json"), str(work / "snbt-out.json")],
        check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    return json.loads((work / "snbt-out.json").read_text())


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--oracle-dir", type=Path, required=True)
    parser.add_argument("--work", type=Path, default=Path("/tmp/tag-parser-work"))
    parser.add_argument("--per-seed", type=int, default=60, help="mutants kept per seed (0 keeps all)")
    parser.add_argument("--out", type=Path, default=OUT)
    parser.add_argument("--seed", type=int, default=5)
    args = parser.parse_args()
    rng = random.Random(args.seed)
    texts = list(SEEDS) + [f"{{v:{token}}}" for token in TOKENS] + [f"{{v:[{token}]}}" for token in TOKENS]
    seeds = list(texts)
    seen = set(texts)
    for seed in seeds:
        for mutant in mutants(seed, rng, args.per_seed or 10**9):
            if mutant not in seen:
                seen.add(mutant)
                texts.append(mutant)
    results = run_oracle(args.oracle_dir, args.work, texts)
    rows = [[text, result["nbt"] if result["ok"] else None] for text, result in zip(texts, results)]
    doc = {
        "format": "tag-parser-vectors-v1",
        "meta": {
            "oracle": "tools/loot_oracle.java snbt (unchanged net.minecraft.nbt.TagParser.parseTag)",
            "generator": "tools/tag_parser_generate.py",
            "per_seed": args.per_seed,
            "seed": args.seed,
            "row": "[text, NbtIo.writeAnyTag hex of the parsed compound, or null when rejected]",
        },
        "vectors": rows,
    }
    args.out.write_text(json.dumps(doc, ensure_ascii=False, separators=(",", ":")) + "\n")
    accepted = sum(1 for row in rows if row[1] is not None)
    print(f"{len(rows)} vectors ({accepted} accepted) -> {args.out}")


if __name__ == "__main__":
    main()
