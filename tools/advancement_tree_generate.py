#!/usr/bin/env python3
"""Generate fixtures/advancement-tree-1.21.json from the original's loaded vanilla tree.

tools/loot_oracle.java `advancement-tree` loads the vanilla advancements through the unchanged
ServerAdvancementManager and records, for every node, its parent, its children in the original's
iteration order (identity-hash order of a ReferenceOpenHashSet) and its display position after
TreeNodePosition.run, plus the bytes of a ClientboundUpdateAdvancementsPacket (every holder sorted
by id, one removed id, one progress entry); and AdvancementVisibilityEvaluator's visible set for 60
sets of done advancements (`advancement-visibility`).

    python3 tools/advancement_tree_generate.py --oracle-dir /path/outside/git
"""
import argparse
import json
import random
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "fixtures" / "advancement-tree-1.21.json"
PROGRESS = [
    {
        "advancement": "minecraft:adventure/kill_all_mobs",
        "removed": "minecraft:story/mine_stone",
        "all_holders": True,
        # Insertion order of the decoded criteria (a JSON object keeps it in the original).
        "granted_order": ["minecraft:zombie", "minecraft:creeper", "minecraft:wither"],
        "progress": {
            "criteria": {
                "minecraft:zombie": "2024-01-02 03:04:05 +0000",
                "minecraft:creeper": "2023-12-31 23:59:59 +0100",
                "minecraft:wither": "2024-06-01 12:00:00 +0000",
            },
            "done": False,
        },
    },
    # Fresh progress (nothing decoded): the copied map starts empty.
    {"advancement": "minecraft:adventure/kill_all_mobs", "removed": "minecraft:story/root", "granted_order": [], "progress": {"done": False}},
    {"advancement": "minecraft:husbandry/bred_all_animals", "removed": "minecraft:story/root", "granted_order": [], "progress": {"done": False}},
    {"advancement": "minecraft:adventure/adventuring_time", "removed": "minecraft:story/root", "granted_order": ["minecraft:plains"],
     "progress": {"criteria": {"minecraft:plains": "2024-01-01 00:00:00 +0000"}, "done": False}},
]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--oracle-dir", type=Path, required=True)
    parser.add_argument("--work", type=Path, default=Path("/tmp/advancement-tree-work"))
    args = parser.parse_args()
    args.work.mkdir(parents=True, exist_ok=True)
    (args.work / "progress.json").write_text(json.dumps(PROGRESS))
    classpath = (args.oracle_dir / "cp.txt").read_text().strip()
    subprocess.run(
        ["java", "--add-exports", "java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED", "-cp", f"{args.oracle_dir}:{classpath}", "loot_oracle",
         "advancement-tree", str(args.work / "tree.json"), str(args.work / "progress.json")],
        check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    tree = json.loads((args.work / "tree.json").read_text())
    # Visibility: random sets of done advancements (displayed ones weighted), from empty to dense.
    rng = random.Random(26)
    ids = sorted(tree["nodes"])
    shown = [i for i in ids if "x" in tree["nodes"][i]]
    done_sets = [[], shown, ids]
    for k in range(57):
        density = (k % 19) / 18
        done_sets.append(sorted(i for i in shown if rng.random() < density) + sorted(i for i in ids if i not in shown and rng.random() < density / 4))
    (args.work / "done.json").write_text(json.dumps(done_sets))
    subprocess.run(
        ["java", "--add-exports", "java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED", "-cp", f"{args.oracle_dir}:{classpath}", "loot_oracle",
         "advancement-visibility", str(args.work / "done.json"), str(args.work / "visible.json")],
        check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    visible = json.loads((args.work / "visible.json").read_text())
    tree["visibility"] = [{"done": done, "visible": shown_now} for done, shown_now in zip(done_sets, visible)]
    doc = {
        "format": "advancement-tree-v1",
        "oracle": "tools/loot_oracle.java advancement-tree (unchanged ServerAdvancementManager, TreeNodePosition and ClientboundUpdateAdvancementsPacket)",
        "generator": "tools/advancement_tree_generate.py",
        "progress_specs": PROGRESS,
        **tree,
    }
    OUT.write_text(json.dumps(doc, sort_keys=True, separators=(",", ":")) + "\n")
    print(f"{len(tree['nodes'])} nodes, {len(tree['packets'])} packets, {len(tree['visibility'])} visibility cases -> {OUT}")


if __name__ == "__main__":
    main()
