#!/usr/bin/env python3
"""Generate fixtures/commands-brigadier-tree-packet-vectors.json by EXECUTING the original server.

    python3 tools/loot_oracle_setup.py --oracle-dir /path/outside/git     # builds server-named.jar + cp.txt
    python3 tools/brigadier_tree_packet_generate.py --oracle-dir /path/outside/git

WP-E3 stage 5. Seeded random command trees (the generator of tools/brigadier_dispatch_generate.py:
literals, typed Brigadier arguments, executable nodes, redirects, merged duplicate registrations) are
built with the unchanged com.mojang.brigadier 1.2.9 builders and wrapped in the dedicated server's own
ClientboundCommandsPacket; the bytes its StreamCodec writes are recorded (stage 8 adds, per permission level 0-4, the packet of the tree
Commands.fillUsableCommands builds for a source of that level; stage 6 appends one fixed tree per
Minecraft position argument type, stages 7, 9 and 10 one per simple value, range or operation argument). Extra decoration for this
stage: custom suggestion providers (registered names and an unregistered provider), and copies of whole
subtrees planted under other nodes so that structurally equal nodes exist (the original numbers nodes
with CommandNode.equals, so those must share one index).

The jar and libraries are proprietary downloads and stay outside git.
"""
import argparse
import copy
import json
import random
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import brigadier_dispatch_generate as dispatch  # noqa: E402  (tree generator shared with stage 2)

ROOT = Path(__file__).resolve().parents[1]
SEED = 20261105
PROVIDERS = [None, None, "minecraft:all_recipes", "minecraft:available_sounds", "minecraft:summonable_entities", "minecraft:ask_server"]


def nodes_of(spec, found):
    found.append(spec)
    for child in spec.get("children", []):
        nodes_of(child, found)
    return found


def decorate(rng, spec):
    if spec["t"] == "arg" and rng.random() < 0.35:
        name = rng.choice(PROVIDERS)
        spec["sug"] = {} if name is None else {"name": name}
    for child in spec.get("children", []):
        decorate(rng, child)


def plant_duplicates(rng, tree):
    """Copy a subtree (without redirects) under another node: equal nodes the packet must merge."""
    everything = []
    for top in tree:
        nodes_of(top, everything)
    for _ in range(rng.randint(0, 3)):
        sources = [n for n in everything if "redirect" not in n and n.get("children") or n.get("cmd") is not None]
        targets = [n for n in everything if "redirect" not in n]
        if not sources or not targets:
            return
        source = rng.choice(sources)
        target = rng.choice(targets)
        if "redirect" in json.dumps(source):
            continue
        target.setdefault("children", []).append(copy.deepcopy(source))


def build_tests(rng):
    tests = []
    for _ in range(160):
        tree = dispatch.gen_tree(rng)
        for node in tree:
            decorate(rng, node)
        plant_duplicates(rng, tree)
        tests.append({"tree": tree})
    # Fixed trees appended after the random ones (the random stream is unchanged): one position
    # argument each, so the registry ids and empty templates of stage 6 are compared too.
    for kind in ["blockpos", "columnpos", "vec3", "vec2"]:
        tests.append({"tree": [{"t": "lit", "name": "p", "children": [{"t": "arg", "name": "a", "type": {"k": kind}, "cmd": 1}]}]})
    # stage 7: simple value arguments (time with the default and with a minimum, whose template is an int)
    for spec in [{"k": "time"}, {"k": "time", "min": 5}, {"k": "time", "min": -100}, {"k": "uuid"}, {"k": "anchor"}, {"k": "angle"}, {"k": "swizzle"}]:
        tests.append({"tree": [{"t": "lit", "name": "p", "children": [{"t": "arg", "name": "a", "type": spec, "cmd": 1}]}]})
    # stage 8: redirect targets that only the original's structural map (CommandNode.equals, requirements ignored)
    # can resolve. Tree 1: the target "t" needs level 3, an equal node a/t does not, so below level 3 the redirect
    # still finds a/t's copy. Tree 2: t and a/t are equal in the source, but t's child "c" needs level 2, so below
    # level 2 their copies differ, and the redirect must resolve to the later one (put overwrites the equal key).
    tests.append({"tree": [
        {"t": "lit", "name": "t", "req": 3, "children": [{"t": "lit", "name": "c", "cmd": 1}]},
        {"t": "lit", "name": "a", "children": [
            {"t": "lit", "name": "t", "children": [{"t": "lit", "name": "c", "cmd": 1}]},
            {"t": "lit", "name": "r", "redirect": {"to": "t", "kind": "plain"}}]}]})
    tests.append({"tree": [
        {"t": "lit", "name": "t", "children": [{"t": "lit", "name": "c", "req": 2, "cmd": 1}]},
        {"t": "lit", "name": "a", "children": [
            {"t": "lit", "name": "t", "children": [{"t": "lit", "name": "c", "cmd": 1}]},
            {"t": "lit", "name": "r", "redirect": {"to": "t", "kind": "plain"}}]}]})
    # stage 9: int and float range arguments (registry ids 38 and 39, no template)
    for spec in [{"k": "int_range"}, {"k": "float_range"}]:
        tests.append({"tree": [{"t": "lit", "name": "p", "children": [{"t": "arg", "name": "a", "type": spec, "cmd": 1}]}]})
    # stage 10: the scoreboard operation argument (registry id 25, no template)
    tests.append({"tree": [{"t": "lit", "name": "p", "children": [{"t": "arg", "name": "a", "type": {"k": "operation"}, "cmd": 1}]}]})
    return tests


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--oracle-dir", type=Path, required=True)
    parser.add_argument("--out", type=Path, default=ROOT / "fixtures/commands-brigadier-tree-packet-vectors.json")
    args = parser.parse_args()
    oracle = args.oracle_dir.resolve()
    classpath_file = oracle / "cp.txt"
    if not classpath_file.exists():
        sys.exit("run tools/loot_oracle_setup.py --oracle-dir <dir> first (it builds server-named.jar and cp.txt)")
    classpath = classpath_file.read_text().strip()
    rng = random.Random(SEED)
    tests = build_tests(rng)
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        subprocess.run(["javac", "-proc:none", "-Xlint:none", "-cp", classpath, "-d", str(tmp), str(ROOT / "tools/brigadier_tree_packet_oracle.java")], check=True)
        (tmp / "cases.json").write_text(json.dumps(tests))
        subprocess.run(["java", "-cp", f"{classpath}:{tmp}", "cinderstone.oracle.brigadier_tree_packet_oracle", str(tmp / "cases.json"), str(tmp / "out.json")], check=True, timeout=300)
        outputs = json.loads((tmp / "out.json").read_text())
    assert len(outputs) == len(tests)
    cases = [{"tree": test["tree"], "packet": output["packet"], "usable": output["usable"]} for test, output in zip(tests, outputs)]
    sizes = [len(case["packet"]) // 2 for case in cases]
    document = {
        "format": "brigadier-1.2.9-tree-packet-vectors-v1",
        "source": "unchanged com.mojang.brigadier 1.2.9 and the pinned 1.21 server's ClientboundCommandsPacket (official names)",
        "seed": SEED,
        "tests": cases,
    }
    args.out.write_text(json.dumps(document, separators=(",", ":"), ensure_ascii=True) + "\n")
    print(f"wrote {args.out} ({len(cases)} trees, packets of {min(sizes)}-{max(sizes)} bytes, {args.out.stat().st_size} bytes)")


if __name__ == "__main__":
    sys.exit(main())
