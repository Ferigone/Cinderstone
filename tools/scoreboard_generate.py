#!/usr/bin/env python3
"""Generate fixtures/scoreboard-vectors.json and fixtures/scoreboard-load-mutants.json.

The original `ServerScoreboard`, `Scoreboard`, the five scoreboard packets and `ScoreboardSaveData`
run scripted operations (tools/loot_oracle.java `scoreboard`) and one-step mutants of saved
payloads (`scoreboard-load-mutants`). Results: errors, return values, broadcast packet frames (id +
body), full state dumps (objective, score, team and display-slot order included) and saved NBT.

    python3 tools/scoreboard_generate.py --oracle-dir /path/outside/git [--sample 4]
"""
import argparse
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
GOLD = {"text": "Kills", "color": "gold"}


def obj(name, criteria="dummy", display=None, render="integer", auto=False, fmt=None):
    return {"op": "objective_add", "name": name, "criteria": criteria, "display": display if display is not None else name,
            "render": render, "auto": auto, "format": fmt}


def score(op, holder, objective, value=None, **extra):
    step = {"op": op, "holder": holder, "objective": objective, **extra}
    if value is not None:
        step["value"] = value
    return step


def team_set(name, field, value):
    return {"op": "team_set", "name": name, "field": field, "value": value}


DUMP = {"op": "dump"}
SAVE = {"op": "save"}
BLANK = {"type": "minecraft:blank"}
FIXED = {"type": "minecraft:fixed", "value": {"text": "!", "color": "red"}}
STYLED = {"type": "minecraft:styled", "color": "yellow", "bold": True}


def scenarios():
    out = []
    add = lambda *steps, **kw: out.append({"steps": list(steps), **kw})

    # objectives and criteria
    add(obj("kills", display=GOLD), obj("kills"), obj("a", "health", render="hearts"), obj("b", "nonexistent"), obj("c", "minecraft.mined:minecraft.stone"),
        obj("d", "minecraft.custom:minecraft.jump"), obj("e", "minecraft.mined:minecraft.nothing"), obj("f", "minecraft.bogus:minecraft.stone"),
        obj("g", "minecraft.killed:minecraft.zombie"), obj("h", "minecraft.mined:minecraft.sto!ne"), obj("i", "teamkill.red"), obj("j", "killedByTeam.dark_blue"),
        obj("k", "xp"), obj("l", "trigger", fmt=BLANK), obj("m", "dummy", fmt=FIXED, auto=True), obj("n", "deathCount", fmt=STYLED), DUMP, SAVE)
    add(obj("o"), {"op": "track", "objective": "o"}, {"op": "objective_set", "name": "o", "display": GOLD},
        {"op": "objective_set", "name": "o", "render": "hearts"}, {"op": "objective_set", "name": "o", "auto": True},
        {"op": "objective_set", "name": "o", "format": FIXED}, {"op": "objective_set", "name": "o", "format": None},
        {"op": "objective_set", "name": "missing", "render": "integer"}, {"op": "untrack", "objective": "o"},
        {"op": "objective_set", "name": "o", "display": "after"}, DUMP)
    # scores
    add(obj("s"), obj("ro", "health"), score("score_set", "A", "s", 5), score("score_add", "A", "s", 3), score("score_set", "A", "s", 8),
        score("score_get", "A", "s"), score("score_set", "A", "ro", 3), score("score_set", "A", "ro", 3, force=True),
        score("score_get", "B", "ro"), score("score_reset", "A", "s"), {"op": "track", "objective": "s"},
        score("score_set", "A", "s", 4), score("score_lock", "A", "s"), score("score_unlock", "A", "s"), score("score_unlock", "A", "s"),
        score("score_display", "A", "s", display={"text": "Alex", "bold": True}), score("score_display", "A", "s", display=None),
        score("score_format", "A", "s", format=STYLED), score("score_format", "A", "s", format=None),
        score("score_set", "Z", "s", 1, holder_display={"text": "Zed"}), DUMP, SAVE)
    add(obj("t", auto=True), {"op": "track", "objective": "t"}, score("score_set", "P", "t", 0, holder_display={"text": "Pat", "color": "red"}),
        score("score_set", "P", "t", 0, holder_display={"text": "Pat", "color": "red"}), score("score_set", "P", "t", 1, holder_display={"text": "Pat2"}),
        score("score_display", "P", "t", display={"text": "Manual"}), score("score_set", "P", "t", 1, holder_display={"text": "Pat3"}), DUMP)
    add(obj("u"), obj("v"), {"op": "track", "objective": "u"}, {"op": "track", "objective": "v"}, score("score_set", "A", "u", 1), score("score_set", "A", "v", 2),
        {"op": "score_remove", "holder": "A", "objective": "u"}, DUMP, {"op": "score_remove", "holder": "A", "objective": "v"}, DUMP,
        score("score_set", "B", "u", 1), {"op": "score_remove_all", "holder": "B"}, {"op": "score_remove_all", "holder": "B"},
        {"op": "score_remove", "holder": "Q", "objective": "u"})
    # many holders: fastutil growth and shifting
    holders = [f"player{index}" for index in range(40)] + ["Steve", "Alex", "Notch", "jeb_", "Dinnerbone", "*"]
    add(obj("w"), obj("x"), {"op": "track", "objective": "w"},
        *[score("score_set", name, "w", index) for index, name in enumerate(holders)], DUMP,
        *[score("score_set", name, "x", index) for index, name in enumerate(holders[:20])],
        *[{"op": "score_remove_all", "holder": name} for name in holders[::3]], DUMP, {"op": "start_packets", "objective": "w"}, SAVE)
    add(*[obj(f"obj{index}") for index in range(30)], DUMP, *[{"op": "objective_remove", "name": f"obj{index}"} for index in range(0, 30, 2)], DUMP,
        *[{"op": "objective_remove", "name": f"obj{index}"} for index in range(1, 28, 2)], DUMP, SAVE)
    # display slots and tracking
    slots = ["list", "sidebar", "below_name", "sidebar.team.red", "sidebar.team.black", "sidebar.team.white", "bogus"]
    add(obj("d1"), obj("d2"), score_set := score("score_set", "A", "d1", 3),
        {"op": "display", "slot": "sidebar", "objective": "d1"}, score("score_set", "A", "d1", 4), {"op": "display", "slot": "list", "objective": "d1"},
        {"op": "display", "slot": "sidebar", "objective": "d2"}, {"op": "display", "slot": "list", "objective": None},
        {"op": "display", "slot": "sidebar", "objective": None}, {"op": "display", "slot": "below_name", "objective": "missing"},
        {"op": "display", "slot": "sidebar.team.red", "objective": "d2"}, {"op": "display", "slot": "sidebar.team.red", "objective": "d2"},
        {"op": "display", "slot": "sidebar.team.white", "objective": "d2"}, {"op": "objective_remove", "name": "d2"}, DUMP, SAVE)
    add(obj("e1"), *[{"op": "display", "slot": slot, "objective": "e1"} for slot in slots[:6]], {"op": "start_packets", "objective": "e1"},
        {"op": "stop_packets", "objective": "e1"}, {"op": "objective_remove", "name": "e1"}, {"op": "objective_remove", "name": "e1"}, DUMP)
    add(obj("f1", fmt=FIXED), obj("f2", fmt=STYLED, display=GOLD), obj("f3", fmt=BLANK), score("score_set", "A", "f1", 1), score("score_format", "A", "f1", format=FIXED),
        score("score_display", "A", "f1", display={"text": "x"}), {"op": "display", "slot": "sidebar", "objective": "f1"},
        {"op": "start_packets", "objective": "f1"}, {"op": "start_packets", "objective": "f2"}, {"op": "start_packets", "objective": "f3"},
        {"op": "stop_packets", "objective": "f1"}, SAVE)
    # teams
    add({"op": "team_add", "name": "red"}, {"op": "team_add", "name": "red"}, team_set("red", "color", "red"), team_set("red", "color", "Dark Red"),
        team_set("red", "color", "nope"), team_set("red", "color", "bold"), team_set("red", "color", "reset"),
        team_set("red", "display", {"text": "Reds", "bold": True}), team_set("red", "prefix", {"text": "[R]"}), team_set("red", "suffix", {"text": "!"}),
        team_set("red", "prefix", None), team_set("red", "friendly", False), team_set("red", "invisibles", False),
        team_set("red", "nametag", "hideForOtherTeams"), team_set("red", "death", "never"), team_set("red", "collision", "pushOwnTeam"),
        team_set("red", "options", 3), team_set("red", "options", 0), team_set("red", "options", 1), DUMP, SAVE)
    names = ["Steve", "Alex", "Notch", "jeb_", "Dinnerbone", "Herobrine", "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "player1", "player2", "player3"]
    add({"op": "team_add", "name": "blue"}, {"op": "team_add", "name": "green"},
        *[{"op": "team_join", "team": "blue", "player": name} for name in names], DUMP,
        {"op": "team_join", "team": "blue", "player": "Steve"}, {"op": "team_join", "team": "green", "player": "Steve"},
        {"op": "team_leave", "player": "Alex"}, {"op": "team_leave", "player": "nobody"}, {"op": "team_leave_from", "player": "Notch", "team": "green"},
        {"op": "team_leave_from", "player": "Notch", "team": "blue"}, {"op": "team_join", "team": "missing", "player": "x"}, DUMP,
        {"op": "team_remove", "name": "blue"}, {"op": "team_remove", "name": "blue"}, DUMP, SAVE)
    add(*[{"op": "team_add", "name": f"team{index}"} for index in range(25)], DUMP, *[{"op": "team_remove", "name": f"team{index}"} for index in range(0, 25, 3)], DUMP, SAVE)
    return out


def run(oracle_dir, work, mode, name, spec):
    work.mkdir(parents=True, exist_ok=True)
    source, target = work / f"{name}-in.json", work / f"{name}-out.json"
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
    parser.add_argument("--sample", type=int, default=20, help="keep every Nth load mutant in the committed fixture (1 keeps all)")
    args = parser.parse_args()
    source = {
        "server_jar_sha1": "450698d1863ab5180c25d7c804ef0fe6369dd1ba",
        "inner_jar_sha256": "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01",
        "mappings_sha1": "31c77994d96f05ba25a870ada70f47f315330437",
        "oracle": "tools/loot_oracle.java scoreboard / scoreboard-load-mutants (unchanged ServerScoreboard, ScoreboardSaveData and the five packets; the player list is empty)",
    }
    spec = scenarios()
    results = run(args.oracle_dir, args.work, "scoreboard", "scoreboard", spec)
    # Second pass: reload what the original saved, then run a few operations on the result.
    saved = sorted({step["data"] for scenario in results for step in scenario if "data" in step}, key=lambda data: (len(data), data))
    loads = [{"load": data, "steps": [DUMP, {"op": "save"}]} for data in saved]
    load_results = run(args.oracle_dir, args.work, "scoreboard", "scoreboard-reload", loads)
    cases = [{"steps": s["steps"], "expect": r} for s, r in zip(spec, results)]
    cases += [{"load": l["load"], "steps": l["steps"], "expect": r} for l, r in zip(loads, load_results)]
    (ROOT / "fixtures/scoreboard-vectors.json").write_text(json.dumps({
        "format": "cinderstone-scoreboard-oracle-v1", "minecraft": "1.21", "source": source,
        "result": "per step: error, ret, packets (start/stop tracking frames), broadcast (frames of PlayerList.broadcastAll), dump, data (scoreboard.dat payload hex)",
        "cases": cases,
    }, separators=(",", ":")))
    # Mutate the payloads up to 3000 hex digits; the larger ones are still reloaded above.
    mutants = run(args.oracle_dir, args.work, "scoreboard-load-mutants", "scoreboard-mutants", [data for data in saved if len(data) <= 3000])
    kept = [row for index, row in enumerate(mutants) if index % args.sample == 0]
    (ROOT / "fixtures/scoreboard-load-mutants.json").write_text(json.dumps({
        "format": "cinderstone-scoreboard-load-mutants-v1", "minecraft": "1.21", "source": source,
        "sampling": f"every {args.sample}th of {len(mutants)} generated mutants of the saved payloads up to 3000 hex digits",
        "result": "{payload hex, error | dump + data}",
        "cases": kept,
    }, separators=(",", ":")))
    print(f"scenarios={len(cases)} saved_payloads={len(saved)} mutants={len(mutants)} loaded={sum(1 for r in mutants if 'error' not in r)} kept={len(kept)}")


if __name__ == "__main__":
    main()
