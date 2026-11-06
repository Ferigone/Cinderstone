#!/usr/bin/env python3
"""Generate fixtures/bossbar-vectors.json.

The original `CustomBossEvents` / `CustomBossEvent` / `ServerBossEvent` and `ClientboundBossEventPacket`
run with stub players whose connections record every frame (tools/loot_oracle.java `bossbar` and
`bossbar-mutants`).

* scenarios: scripted sequences (create, value/max/progress, name/colour/overlay/flags, visibility,
  players added, removed and replaced, offline players, connect/disconnect, save and reload, the
  six packet forms) with the original's state, saved NBT and the frames each player was sent;
* mutants: one-step mutants of a saved boss bar loaded by the original.

    python3 tools/bossbar_generate.py --oracle-dir /path/outside/git [--sample 3] [--full-out FILE]
"""
import argparse
import json
import random
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

PLAYERS = {
    "p1": "00000000-0000-0001-0000-000000000002",
    "p2": "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
    "p3": "123e4567-e89b-12d3-a456-426614174000",
    "p4": "ffffffff-ffff-ffff-ffff-ffffffffffff",
    "p5": "00000000-0000-0000-0000-000000000000",
    "p6": "80000000-0000-0000-8000-000000000000",
    "p7": "00000001-0002-0003-0004-000000000005",
    "p8": "7fffffff-ffff-ffff-7fff-ffffffffffff",
    "p9": "11111111-2222-3333-4444-555555555555",
    "p10": "5eb9aa33-e9a7-4de1-8b0a-3a6d3c0b2c11",
}
NAMES = [
    "Hello",
    {"text": "Boss", "color": "red"},
    {"translate": "entity.minecraft.wither"},
    {"text": "A", "extra": [{"text": "B", "bold": True}]},
    {"text": "multi\nline \"quoted\" é中"},
    {"text": "", "extra": ["x"]},
    {"translate": "k", "with": [1, "two", {"text": "3"}]},
    [{"text": "list"}, "form"],
    {"text": "styled", "bold": True, "italic": False, "color": "#ff8800", "font": "minecraft:uniform"},
    {"keybind": "key.jump"},
]
IDS = ["test:boss", "minecraft:a", "ns:b/c", "x", "ns:b/d", "zz:q", "a:b", "foo:bar_baz.qux-1"]


def op(operation, /, **extra):
    return {"op": operation, **extra}


def set_players(id_, names):
    return op("set_players", id=id_, players=list(names), uuids={name: PLAYERS[name] for name in names})


def player_step(operation, id_, name, **extra):
    return op(operation, id=id_, player=name, uuid=PLAYERS[name], **extra)


def hand_scenarios():
    out = []
    add = lambda *steps: out.append({"steps": list(steps)})
    t = "test:boss"
    add(op("create", id=t, name="Hello"), op("state", id=t), op("save"), op("ids"))
    add(op("create", id=t, name=NAMES[1]), player_step("add_player", t, "p1"), op("set_value", id=t, value=50), op("set_max", id=t, value=200),
        op("set_color", id=t, value="red"), op("set_color", id=t, value="red"), op("set_overlay", id=t, value="notched_20"), op("set_overlay", id=t, value="bogus"),
        op("set_darken", id=t, value=True), op("set_darken", id=t, value=True), op("set_music", id=t, value=True), op("set_fog", id=t, value=True),
        op("set_name", id=t, name="Hello"), op("set_name", id=t, name="Hello"), op("set_name", id=t, name={"text": "Hello"}), op("state", id=t), op("save"))
    # Visibility and the add/remove packets.
    add(op("create", id=t, name="Hello"), player_step("add_player", t, "p1"), player_step("add_player", t, "p1"), player_step("add_player", t, "p2"),
        op("set_visible", id=t, value=False), op("set_value", id=t, value=10), player_step("add_player", t, "p3"), player_step("remove_player", t, "p1"),
        op("set_visible", id=t, value=True), op("set_visible", id=t, value=True), op("set_name", id=t, name="New"), op("remove_all", id=t), op("save"))
    # Progress arithmetic: zero and negative maxima, NaN, infinities, overflow.
    steps = [op("create", id=t, name="Hello"), player_step("add_player", t, "p1")]
    for value, max_ in [(1, 3), (-5, 100), (500, 100), (0, 0), (10, 0), (-10, 0), (2147483647, 1), (-2147483648, 1), (1, -1), (-1, -1), (7, 1000), (0, 5), (3, 7)]:
        steps += [op("set_max", id=t, value=max_), op("set_value", id=t, value=value)]
    steps += [op("set_progress", id=t, bits=0x7fc00000), op("set_progress", id=t, bits=0x7fc00000), op("set_progress", id=t, bits=0x80000000), op("set_progress", id=t, bits=0),
              op("set_progress", id=t, bits=0x3f800000), op("state", id=t), op("save")]
    add(*steps)
    # Players: remembered UUIDs, replacing the set, connecting and disconnecting.
    add(op("create", id=t, name="Hello"), player_step("add_player", t, "p1"), player_step("add_player", t, "p2"), op("add_offline", id=t, uuid=PLAYERS["p3"]),
        op("save"), set_players(t, ["p1"]), set_players(t, ["p1"]), set_players(t, []),
        player_step("connect", t, "p3") if False else op("connect", player="p3", uuid=PLAYERS["p3"]), op("save"))
    add(op("create", id=t, name="Hello"), op("add_offline", id=t, uuid=PLAYERS["p1"]), op("add_offline", id=t, uuid=PLAYERS["p2"]),
        op("connect", player="p1", uuid=PLAYERS["p1"]), op("connect", player="p3", uuid=PLAYERS["p3"]), op("disconnect", player="p1", uuid=PLAYERS["p1"]),
        op("connect", player="p1", uuid=PLAYERS["p1"]), set_players(t, ["p1", "p3"]), op("save"))
    add(op("create", id=t, name="Hello"), set_players(t, ["p1", "p2", "p3"]), set_players(t, ["p3", "p4"]),
        op("set_visible", id=t, value=False), set_players(t, ["p4", "p5"]), op("set_visible", id=t, value=True), op("remove_all", id=t), op("save"))
    # Many remembered players: the HashSet order of the saved list across resizes and removals.
    many = list(PLAYERS)
    add(op("create", id=t, name="Hello"), *[op("add_offline", id=t, uuid=PLAYERS[name]) for name in many], op("save"),
        set_players(t, many[:4]), op("save"), *[player_step("remove_player", t, name) for name in many[:4]], op("save"))
    # Several events, replacement and removal, ids in HashMap order, save and reload.
    steps = [op("create", id=i, name=NAMES[n % len(NAMES)]) for n, i in enumerate(IDS)]
    steps += [op("ids"), op("save"), op("remove", id="minecraft:a"), op("ids"), op("create", id=t, name="Again"), op("ids"), op("save"), op("load"), op("ids"), op("state", id=t)]
    add(*steps)
    add(op("create", id=t, name="Hello"), op("set_value", id=t, value=33), op("set_color", id=t, value="purple"), op("set_overlay", id=t, value="notched_10"),
        op("set_music", id=t, value=True), player_step("add_player", t, "p1"), op("save"), op("remove", id=t), op("load"), op("state", id=t), op("save"))
    # The six packets.
    for kind in ("add", "remove", "progress", "name", "style", "properties"):
        add(op("create", id=t, name=NAMES[3]), op("set_value", id=t, value=40), op("set_color", id=t, value="green"), op("set_overlay", id=t, value="notched_12"),
            op("set_darken", id=t, value=True), op("set_fog", id=t, value=True), op("packet", id=t, kind=kind))
    for name in range(len(NAMES)):
        add(op("create", id=t, name=NAMES[name]), op("packet", id=t, kind="add"), op("packet", id=t, kind="name"))
    return out


def random_scenarios(rng, count):
    out = []
    names = list(PLAYERS)
    for _ in range(count):
        ids = rng.sample(IDS, rng.randint(1, 3))
        steps = [op("create", id=i, name=rng.choice(NAMES)) for i in ids]
        for _ in range(rng.randint(10, 60)):
            i = rng.choice(ids)
            kind = rng.random()
            name = rng.choice(names)
            if kind < 0.12:
                steps.append(op("set_value", id=i, value=rng.choice([0, 1, 5, 50, 100, 150, -3, 2147483647])))
            elif kind < 0.2:
                steps.append(op("set_max", id=i, value=rng.choice([0, 1, 10, 100, 1000, -1, 7])))
            elif kind < 0.27:
                steps.append(op("set_color", id=i, value=rng.choice(["pink", "blue", "red", "green", "yellow", "purple", "white", "nope"])))
            elif kind < 0.33:
                steps.append(op("set_overlay", id=i, value=rng.choice(["progress", "notched_6", "notched_10", "notched_12", "notched_20", "nope"])))
            elif kind < 0.4:
                steps.append(op(rng.choice(["set_darken", "set_music", "set_fog", "set_visible"]), id=i, value=rng.random() < 0.5))
            elif kind < 0.45:
                steps.append(op("set_name", id=i, name=rng.choice(NAMES)))
            elif kind < 0.6:
                steps.append(player_step("add_player", i, name))
            elif kind < 0.68:
                steps.append(player_step("remove_player", i, name))
            elif kind < 0.72:
                steps.append(op("add_offline", id=i, uuid=PLAYERS[name]))
            elif kind < 0.78:
                steps.append(set_players(i, rng.sample(names, rng.randint(0, 4))))
            elif kind < 0.84:
                steps.append(op("connect", player=name, uuid=PLAYERS[name]))
            elif kind < 0.9:
                steps.append(op("disconnect", player=name, uuid=PLAYERS[name]))
            elif kind < 0.93:
                steps.append(op("remove_all", id=i))
            else:
                steps.append(op("save"))
        steps += [op("save"), op("ids")] + [op("state", id=i) for i in ids]
        out.append({"steps": steps, "random": True})
    return out


MUTANT_BASES = [
    '{Name:"\\"Hello\\"",Visible:1b,Value:30,Max:60,Color:"red",Overlay:"notched_6",DarkenScreen:1b,PlayBossMusic:0b,CreateWorldFog:1b,Players:[[I;1,2,3,4],[I;-1,-2,-3,-4]]}',
    '{Name:"{\\"text\\":\\"Boss\\",\\"color\\":\\"gold\\"}",Visible:0b,Value:0,Max:100,Color:"white",Overlay:"progress",DarkenScreen:0b,PlayBossMusic:0b,CreateWorldFog:0b,Players:[]}',
    '{Name:"[\\"a\\",{\\"text\\":\\"b\\"}]",Visible:1b,Value:5,Max:0,Color:"purple",Overlay:"notched_20",DarkenScreen:0b,PlayBossMusic:1b,CreateWorldFog:0b}',
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
    parser.add_argument("--work", type=Path, default=Path("/tmp/bossbar-work"))
    parser.add_argument("--sample", type=int, default=3, help="keep every Nth random scenario and mutant (1 keeps all)")
    parser.add_argument("--full-out", type=Path)
    args = parser.parse_args()
    rng = random.Random(20261107)
    scenarios = hand_scenarios() + random_scenarios(rng, 120)
    results = run_oracle(args.oracle_dir, args.work, "bossbar", scenarios, "scenarios")
    rows = [{"steps": s["steps"], "expect": r, "random": bool(s.get("random"))} for s, r in zip(scenarios, results)]
    mutants = run_oracle(args.oracle_dir, args.work, "bossbar-mutants", [{"id": "test:boss", "bases": MUTANT_BASES}], "mutants")
    meta = {
        "oracle": "tools/loot_oracle.java bossbar / bossbar-mutants (unchanged CustomBossEvents, ServerBossEvent and ClientboundBossEventPacket with stub players)",
        "generator": "tools/bossbar_generate.py",
        "players": PLAYERS,
    }
    if args.full_out:
        args.full_out.write_text(json.dumps({"meta": meta, "scenarios": rows, "mutants": mutants}))
    stride = max(1, args.sample)
    randoms = [row for row in rows if row["random"]]
    keep = [row for row in rows if not row["random"]] + randoms[::stride]
    kept_mutants = mutants[::stride]
    meta["sampled_from"] = {"scenarios": len(rows), "mutants": len(mutants)}
    meta["scenarios"] = len(keep)
    meta["mutants"] = len(kept_mutants)
    (ROOT / "fixtures" / "bossbar-vectors.json").write_text(
        json.dumps({"meta": meta, "scenarios": keep, "mutants": kept_mutants}, separators=(",", ":")) + "\n")
    errors = sum(1 for row in rows for result in row["expect"] if "error" in result)
    print(f"{len(rows)} scenarios ({errors} steps raised), {len(mutants)} mutants; kept {len(keep)} and {len(kept_mutants)}")


if __name__ == "__main__":
    main()
