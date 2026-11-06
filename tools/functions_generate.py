#!/usr/bin/env python3
"""Generate fixtures/functions-vectors.json: function files and macro instantiation.

tools/loot_oracle.java `functions` runs the unchanged CommandFunction.fromLines over the real 1.21
command dispatcher (a recording copy notes every command string parsed and whether parseCommand
accepts it), then MacroFunction.instantiate for argument compounds. Inputs are hand-written files
plus seeded variants (lines shuffled, continuations, macro lines, comments, slashes).

    python3 tools/functions_generate.py --oracle-dir /path/outside/git
"""
import argparse
import json
import random
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "fixtures" / "functions-vectors.json"
COMMANDS = ["say hi", "time set day", "seed", "weather clear", "say a b c", "difficulty peaceful", "gamerule doDaylightCycle false",
            "notacommand", "time set", "say", "kill @e[type=cow]", "tellraw @a \"x\"", "give @s stone 5", "scoreboard objectives list"]
MACROS = ["$say $(x)", "$say $(x) and $(y)", "$time set $(t)", "$give @s $(item) $(n)", "$say $$(x)", "$say $(x)$(x)", "$tellraw @a $(json)",
          "$say $(", "$say $(x", "$say nothing", "$say $(bad-name)", "$say $()", "$say $(é)", "$$(x)", "$say cost $5 $(x)"]
ARGUMENTS = [None, "{}", "{x:1}", "{x:'hi',y:2}", "{x:1.5f,y:2.5d,t:'day',item:stone,n:3b}", "{x:0.1f,y:1e20d}", "{x:-0.0d,y:3s,n:9999999999L}",
             "{x:[1,2],y:{a:'b'},json:'\"t\"'}", "{x:'@s',y:1.0000000000000002d}", "{t:night,item:'minecraft:diamond',n:64}",
             "{x:0.000000000000000123d}", "{x:123456789.987654321d}", "{x:2.5d,y:0.5d}", "{x:1.0E-15d}", "{x:5e-16d}"]
FIXED = [
    ["# comment", "say hi", "", "  time set day  ", "seed"],
    ["say a \\", "  b"], ["say a \\", "b \\", "c"], ["say a \\"], ["/say hi"], ["//x"], ["/"], ["  /say"], ["# only"], [],
    ["$say $(x) and $(y)", "say plain", "$time set $(t)"], ["say first", "$say $(x)", "say last"],
    ["$say a \\", "$(x)"], ["notacommand"], ["say ok", "time set"], ["\t# tab", " say nbsp"],
]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--oracle-dir", type=Path, required=True)
    parser.add_argument("--work", type=Path, default=Path("/tmp/functions-work"))
    parser.add_argument("--random", type=int, default=400)
    args = parser.parse_args()
    rng = random.Random(28)
    files = [list(f) for f in FIXED]
    for _ in range(args.random):
        lines = []
        for _ in range(rng.randint(1, 6)):
            kind = rng.random()
            if kind < 0.35:
                line = rng.choice(COMMANDS)
            elif kind < 0.7:
                line = rng.choice(MACROS)
            elif kind < 0.8:
                line = "# " + rng.choice(COMMANDS)
            elif kind < 0.85:
                line = ""
            elif kind < 0.92:
                line = rng.choice(COMMANDS) + " \\"
            else:
                line = rng.choice(["/", "//", " "]) + rng.choice(COMMANDS)
            if rng.random() < 0.2:
                line = rng.choice(["  ", "\t", " \t "]) + line + rng.choice(["", " ", "\t"])
            lines.append(line)
        files.append(lines)
    specs = [{"lines": lines, "arguments": ARGUMENTS} for lines in files]
    args.work.mkdir(parents=True, exist_ok=True)
    (args.work / "in.json").write_text(json.dumps(specs))
    classpath = (args.oracle_dir / "cp.txt").read_text().strip()
    subprocess.run(["java", "--add-exports", "java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED", "-cp", f"{args.oracle_dir}:{classpath}", "loot_oracle",
                    "functions", str(args.work / "in.json"), str(args.work / "out.json")], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    out = json.loads((args.work / "out.json").read_text())
    cases = [{"lines": spec["lines"], "expect": result} for spec, result in zip(specs, out["results"])]
    OUT.write_text(json.dumps({"format": "functions-vectors-v1", "generator": "tools/functions_generate.py",
                               "oracle": "tools/loot_oracle.java functions (unchanged CommandFunction.fromLines and MacroFunction over the 1.21 dispatcher)",
                               "arguments": ARGUMENTS, "parsed": out["parsed"], "cases": cases}, ensure_ascii=False, separators=(",", ":")) + "\n")
    ok = sum(1 for c in cases if c["expect"]["ok"])
    print(f"{len(cases)} files ({ok} accepted), {len(out['parsed'])} command strings -> {OUT}")


if __name__ == "__main__":
    main()
