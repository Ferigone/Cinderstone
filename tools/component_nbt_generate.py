#!/usr/bin/env python3
"""Generate fixtures/component-nbt-vectors.json and fixtures/component-nbt-parity.json.

The original `ComponentSerialization.CODEC` over `NbtOps` (tools/loot_oracle.java `component-nbt`)
decides, for every NBT input, whether it is accepted and which NBT it re-encodes to. The Rust
`Component::from_nbt` must agree.

* vectors: the NBT encodings the original produces for every accepted input of
  tools/component_generate.py (so every component shape round-trips), plus hand-written
  NBT-specific forms: numeric booleans, array tags, `{WRAP: x}` wrappers, lists of every type.
* parity: every vector mutated one step at a time on the Java side (each member or element
  removed, each node replaced by 19 wrong-typed tags, foreign `type` members added).

    python3 tools/component_nbt_generate.py --oracle-dir /path/outside/git [--sample 200]
"""
import argparse
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools"))
import component_generate as base  # noqa: E402

# SNBT cannot spell an empty key: WRAP stands for the `""` of a `{"": element}` wrapper and the
# oracle renames it after parsing.
HAND_WRITTEN = [
    '"hello"', '""', '{text:"x"}', '{text:"x",extra:["y"]}', '{text:"x",extra:["y","z"]}',
    '["a","b"]', '[]', '[[]]', '[{WRAP:"a"},{text:"b"}]',
    '[{WRAP:"a"}]', '[{WRAP:{text:"a",bold:1b}}]', '[{WRAP:{WRAP:"a"}}]', '{WRAP:"a"}',
    '{text:"x",extra:[{WRAP:"y"}]}', '{text:"x",extra:[{WRAP:"y"},{text:"z",italic:0b}]}',
    '{text:"x",extra:[{WRAP:{text:"y",bold:1b}}]}', '{text:"x",extra:[B;1b]}', '{text:"x",extra:[I;1]}', '{text:"x",extra:[]}',
    '{text:"x",extra:[1b]}', '{text:"x",extra:[[]]}', 
    '1b', '1', '2.5f', '[1b]', '[B;1b]', '[I;1]', '[L;1L]', '{}', '{text:1b}', '{text:["a"]}',
]
for flag in ["bold", "italic", "underlined", "strikethrough", "obfuscated"]:
    for value in ['1b', '0b', '2b', '-1b', '1s', '256s', '256', '65536', '1L', '256L', '0.4f', '1.5f', '256.0f', '257.0f', '0.4d', '256.0d', '1e30d',
                  '"true"', '""', '[]', '{}', '[B;1b]', '[1b]']:
        HAND_WRITTEN.append(f'{{text:"f",{flag}:{value}}}')
for value in ['1b', '0b', '2', '256', '0.4f', '"x"', '[]', '{}']:
    HAND_WRITTEN += [f'{{nbt:"p",entity:"@s",interpret:{value}}}', f'{{nbt:"p",entity:"@s",separator:{value}}}', f'{{translate:"k",fallback:{value}}}']
for arguments in ['1b', '1s', '1', '1L', '1.5f', '1.5d', '"a"', '[]', '{}', '[1b]', '{text:"q"}', '{text:"q",bold:1b}', '{WRAP:"a"}', '{WRAP:1b}',
                  '{WRAP:{text:"q"}}', '{WRAP:{text:"q",bold:1b}}', '{translate:"i",with:[1b,2b]}', '[B;1b,2b]', '[I;1,2]', '[L;1L,2L]', '[B;]',
                  '["a"]', '[1b,2b]', '[1,2]', '[1L,2L]', '[1s,2s]', '[1.5f,2.5f]', '[1.5d,0.1d]', '[[1b],[2b]]',
                  '[{WRAP:"a"},{WRAP:1b}]', '[{WRAP:"a"},{text:"q",bold:1b}]',
                  '[{WRAP:{WRAP:"a"}}]', '[{WRAP:{WRAP:{text:"q",bold:1b}}}]', '[{text:"q",bold:1b},{text:"r",bold:1b}]']:
    HAND_WRITTEN.append(f'{{translate:"k",with:{arguments}}}')
    HAND_WRITTEN.append(f'{{translate:"k",with:[{arguments}]}}')
for color in ['"red"', '"#FF00aa"', '"#GG"', '1b', '""']:
    HAND_WRITTEN.append(f'{{text:"c",color:{color}}}')


def run(oracle_dir, work, mode_args):
    classpath = (oracle_dir / "cp.txt").read_text().strip()
    subprocess.run(
        ["java", "--add-exports", "java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED",
         "-cp", f"{oracle_dir}:{classpath}", "loot_oracle", *mode_args],
        check=True,
    )


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--oracle-dir", type=Path, required=True)
    parser.add_argument("--work", type=Path, default=Path("/tmp"))
    parser.add_argument("--sample", type=int, default=200, help="keep every Nth mutant in the committed fixture (1 keeps all)")
    args = parser.parse_args()
    work = args.work
    work.mkdir(parents=True, exist_ok=True)

    # Base inputs: the NBT forms the original itself produces for every accepted JSON vector.
    json_cases = base.hand_written() + base.combinations_block()
    json_results = base.run_oracle(args.oracle_dir, work, json_cases)
    snbt = []
    for result in json_results:
        # Tag.toString() prints the wrapper's empty key as "", which SNBT cannot read back.
        text = result["snbt"].replace('{"":', "{WRAP:") if result["ok"] else None
        if text and text not in snbt:
            snbt.append(text)
    vector_inputs = HAND_WRITTEN + snbt
    (work / "component-nbt-in.json").write_text(json.dumps(vector_inputs))
    run(args.oracle_dir, work, ["component-nbt", str(work / "component-nbt-in.json"), str(work / "component-nbt-vectors-out.json")])
    vectors = json.loads((work / "component-nbt-vectors-out.json").read_text())

    run(args.oracle_dir, work, ["component-nbt", str(work / "component-nbt-in.json"), str(work / "component-nbt-mutants-out.json"), "mutate"])
    mutants_all = json.loads((work / "component-nbt-mutants-out.json").read_text())
    vector_hex = {row["in"] for row in vectors}
    mutants = [row for row in mutants_all if row["in"] not in vector_hex]

    source = {
        "server_jar_sha1": "450698d1863ab5180c25d7c804ef0fe6369dd1ba",
        "inner_jar_sha256": "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01",
        "mappings_sha1": "31c77994d96f05ba25a870ada70f47f315330437",
        "oracle": "tools/loot_oracle.java component-nbt (ComponentSerialization.CODEC over NbtOps with the vanilla registries)",
    }
    row = lambda r: [r["in"], True, r["out"]] if r["ok"] else [r["in"], False]
    (ROOT / "fixtures/component-nbt-vectors.json").write_text(json.dumps({
        "format": "cinderstone-component-nbt-oracle-v1", "minecraft": "1.21", "source": source,
        "result": "[input NBT hex (NbtIo.writeAnyTag), accepted, re-encoded NBT hex when accepted]",
        "cases": [row(r) for r in vectors],
    }, separators=(",", ":")))
    kept = [row(r) for index, r in enumerate(mutants) if index % args.sample == 0]
    (ROOT / "fixtures/component-nbt-parity.json").write_text(json.dumps({
        "format": "cinderstone-component-nbt-parity-v1", "minecraft": "1.21", "source": source,
        "sampling": f"every {args.sample}th of {len(mutants)} generated mutants",
        "result": "[input NBT hex, accepted, re-encoded NBT hex when accepted]",
        "cases": kept,
    }, separators=(",", ":")))
    print(f"vectors={len(vectors)} (accepted {sum(1 for r in vectors if r['ok'])}) "
          f"mutants={len(mutants)} accepted={sum(1 for r in mutants if r['ok'])} kept={len(kept)}")


if __name__ == "__main__":
    main()
