#!/usr/bin/env python3
"""Generate fixtures/level-data-vectors.json.

The original `EndDragonFight.Data.CODEC` (the `DragonFight` member of `level.dat`) over `NbtOps`, and
the gateway order a new fight draws from the world seed (tools/loot_oracle.java `dragon-fight`), and
`TimerQueue<MinecraftServer>` with the real `FunctionCallback` / `FunctionTagCallback` and a stub
server that records which function or tag a fired event asked for (`timer-queue`).

* dragon fight data: hand-written payloads and every one-step mutant of them (members removed, each
  node replaced by wrong-typed tags), parsed and re-encoded by the original;
* gateways: `Util.shuffle` of 0..19 with `RandomSource.create(seed)` for many seeds;
* timer queue: scripted and random schedule / remove / tick / store / reload sequences and `ScheduledEvents`
  lists loaded the way `PrimaryLevelData` does, with the original's ids, stored NBT and fired callbacks.

    python3 tools/level_data_generate.py --oracle-dir /path/outside/git [--sample 4]
"""
import argparse
import json
import random
import subprocess
from pathlib import Path

from saved_data_owners_generate import compound, payload, snbt, tag_id  # typed NBT values as SNBT and bytes

ROOT = Path(__file__).resolve().parents[1]

BASES = [
    "{NeedsStateScanning:0b,DragonKilled:1b,PreviouslyKilled:1b,IsRespawning:1b,Dragon:[I;1,2,3,4],ExitPortalLocation:[I;0,64,0],Gateways:[I;3,1,2]}",
    "{NeedsStateScanning:1b,DragonKilled:0b,PreviouslyKilled:0b}",
    "{}",
    "{Gateways:[I;],Dragon:[I;-1,-2,-3,-4],ExitPortalLocation:[I;-5,70,12]}",
    "{Gateways:[1,2,3,4,5],Dragon:[1,2,3,4],ExitPortalLocation:[1,2,3]}",
    "{Gateways:[1b,2b],Dragon:[B;1b,2b,3b,4b],ExitPortalLocation:[L;1L,2L,3L]}",
    "{Gateways:[1s,2s],Dragon:[1L,2L,3L,4L],ExitPortalLocation:[1.5f,2.5f,-3.9f]}",
    "{Gateways:[4.5f,-6.9f],Dragon:[1.5d,2.5d,3.5d,4.5d],ExitPortalLocation:[1e10d,-1e10d,0.5d]}",
    "{Gateways:[3L,-4L],Dragon:[L;1L,2L,3L,4L],ExitPortalLocation:[3b,4b,5b]}",
    "{Gateways:[{WRAP:1},{WRAP:2}],Dragon:[{WRAP:1},{WRAP:2},{WRAP:3},{WRAP:4}]}",
    "{NeedsStateScanning:2,DragonKilled:256s,PreviouslyKilled:0.4f,IsRespawning:-1b}",
    "{NeedsStateScanning:\"x\",DragonKilled:[1b],PreviouslyKilled:{},IsRespawning:\"true\"}",
    "{Gateways:[],Dragon:[],ExitPortalLocation:[]}",
    "{Gateways:[I;2147483647,-2147483648,0],ExitPortalLocation:[I;2147483647,-2147483648,0]}",
    "{Dragon:[2147483648L,4294967297L,3L,4L],Gateways:[3000000000L,-3000000000L]}",
    "{Gateways:[1.9e10d,-1.9e10d,0.0d]}",
]


def hex_of(value):
    return bytes([tag_id(value)]).hex() + payload(value).hex()


def event(name, time, kind="function", callback="ns:f", time_tag="l"):
    return compound(
        Name=("str", name),
        TriggerTime=(time_tag, time),
        Callback=compound(Type=("str", kind), Name=("str", callback)),
    )


def load_step(value):
    return {"op": "load", "snbt": snbt(value), "hex": hex_of(value)}


def event_list(events):
    return ("list", 10, events)


NAMES = ["a", "b", "c", "tick", "x/y", "long.name-1", "A", "", "\u00e9", "z"]
TIMES = [0, 1, 2, 5, 5, 10, 100, 100, 1000, -5, 9223372036854775807, -9223372036854775808]
CALLBACKS = [("function", "ns:f"), ("function", "g"), ("function_tag", "ns:t"), ("function_tag", "minecraft:tick"), ("function", "a:b/c/d")]


def timer_scenarios(rng):
    out = []
    add = lambda *steps: out.append({"steps": list(steps)})
    sched = lambda name, time, kind="function", cb="ns:f": {"op": "schedule", "name": name, "time": time, "kind": kind, "id": cb}
    add({"op": "ids"}, {"op": "store"}, {"op": "tick", "time": 100}, {"op": "remove", "name": "a"})
    add(sched("a", 100), sched("b", 50, "function_tag", "ns:t"), sched("a", 100, "function", "ns:g"), sched("a", 200, "function", "ns:h"), {"op": "ids"}, {"op": "store"},
        {"op": "tick", "time": 60}, {"op": "ids"}, {"op": "tick", "time": 150}, {"op": "ids"}, {"op": "store"}, {"op": "remove", "name": "a"}, {"op": "ids"}, {"op": "store"})
    # A cell outlives an event that fired late; an exact-time tick removes it.
    add(sched("a", 10), {"op": "tick", "time": 10}, {"op": "ids"}, sched("a", 10, "function", "ns:again"), {"op": "store"}, {"op": "tick", "time": 20}, {"op": "ids"})
    add(sched("a", 10), {"op": "tick", "time": 11}, {"op": "ids"}, sched("a", 10, "function", "ns:again"), {"op": "store"}, {"op": "remove", "name": "a"}, sched("a", 10, "function", "ns:later"),
        {"op": "store"})
    add(sched("a", 5), sched("a", 5, "function_tag", "ns:other"), sched("b", 5), sched("a", 4), sched("c", 5, "function_tag", "ns:t"), {"op": "store"}, {"op": "tick", "time": 5}, {"op": "ids"}, {"op": "store"})
    # Same time, scheduling order decides.
    add(*[sched(f"n{i}", 7, "function", f"ns:f{i}") for i in range(6)], {"op": "tick", "time": 7}, {"op": "ids"})
    add(sched("a", 3), sched("b", 3), sched("a", 4), {"op": "remove", "name": "a"}, sched("c", 3), {"op": "store"}, {"op": "tick", "time": 3}, {"op": "ids"})
    # Loading: good events, unknown and malformed callbacks, wrong shapes, numeric forms of the time.
    good = [event("x", 5), event("w", 3, "function_tag", "ok:tag")]
    add(load_step(event_list(good)), {"op": "ids"}, {"op": "store"}, {"op": "tick", "time": 10})
    add(load_step(event_list(good + [event("y", 3, "nope", "a:b"), event("z", 3, "minecraft:function_tag", "Bad Name")])), {"op": "ids"}, {"op": "store"})
    for kind in ["function", "minecraft:function", "Function", ":function", "x:function", "", "function_tag", "minecraft:function_tag", "function_tag ", "FUNCTION_TAG"]:
        add(load_step(event_list([event("e", 1, kind, "ns:f")])), {"op": "store"}, {"op": "tick", "time": 1})
    for callback in ["ns:f", "f", ":f", "ns:", "", "NS:f", "ns:F", "ns:a/b", "ns:a b", "a:b:c", "ns:a\u00e9", "minecraft:minecraft"]:
        add(load_step(event_list([event("e", 1, "function", callback)])), {"op": "store"})
    for time_tag, time in [("b", 5), ("s", -3), ("i", 70000), ("l", 5), ("f", 2.5), ("f", -2.5), ("d", 2.5), ("d", -1.5), ("d", 1e30), ("d", -1e30), ("f", 3e38), ("i", -2147483648)]:
        add(load_step(event_list([event("e", time, "function", "ns:f", time_tag)])), {"op": "store"})
    add(load_step(event_list([compound(Name=("str", "n"), Callback=compound(Type=("str", "function"), Name=("str", "ns:f")))])), {"op": "store"})
    add(load_step(event_list([compound(TriggerTime=("l", 4), Callback=compound(Type=("str", "function"), Name=("str", "ns:f")))])), {"op": "ids"}, {"op": "store"})
    add(load_step(event_list([compound(Name=("str", "n"), TriggerTime=("l", 4), Callback=("str", "function"))])), {"op": "store"})
    add(load_step(event_list([compound(Name=("i", 5), TriggerTime=("str", "7"), Callback=compound(Type=("i", 1), Name=("i", 2)))])), {"op": "store"})
    add(load_step(("list", 3, [("i", 7), ("i", 8)])), {"op": "ids"}, {"op": "store"})
    add(load_step(("list", 9, [("list", 10, good)])), {"op": "ids"})
    add(load_step(("list", 10, [compound(**{"": good[0]})])), {"op": "ids"}, {"op": "store"})
    add(load_step(("list", 10, [compound(**{"": good[0]}), good[1]])), {"op": "ids"}, {"op": "store"})
    add(load_step(("ia", [1, 2, 3])), {"op": "ids"})
    add(load_step(compound(a=("i", 1))), {"op": "ids"})
    add(load_step(("str", "x")), {"op": "ids"})
    add(load_step(("list", 0, [])), {"op": "ids"}, {"op": "store"})
    # The same name and time twice in a list: the second is ignored.
    add(load_step(event_list([event("d", 5, "function", "ns:first"), event("d", 5, "function", "ns:second"), event("d", 6, "function", "ns:third")])), {"op": "store"}, {"op": "tick", "time": 6})
    # Store, reload, store: the sequence ids restart at the stored order.
    add(sched("a", 5), sched("b", 5), sched("c", 4), {"op": "store"}, {"op": "reload"}, {"op": "ids"}, {"op": "store"}, sched("d", 5), {"op": "tick", "time": 5}, {"op": "ids"})
    for _ in range(80):
        names = rng.sample(NAMES, rng.randint(1, 4))
        times = rng.sample(TIMES, rng.randint(2, 6))
        steps = []
        clock = 0
        for _ in range(rng.randint(8, 50)):
            pick = rng.random()
            if pick < 0.45:
                kind, callback = rng.choice(CALLBACKS)
                steps.append(sched(rng.choice(names), rng.choice(times), kind, callback))
            elif pick < 0.55:
                steps.append({"op": "remove", "name": rng.choice(names)})
            elif pick < 0.7:
                clock += rng.randint(0, 3) * rng.choice([1, 5, 50])
                steps.append({"op": "tick", "time": rng.choice([clock, rng.choice(times)])})
            elif pick < 0.8:
                steps.append({"op": "ids"})
            elif pick < 0.9:
                steps.append({"op": "store"})
            else:
                steps.extend([{"op": "store"}, {"op": "reload"}])
        steps += [{"op": "ids"}, {"op": "store"}]
        out.append({"steps": steps, "random": True})
    return out


def run_timer_oracle(oracle_dir, work, scenarios):
    work.mkdir(parents=True, exist_ok=True)
    (work / "timer-in.json").write_text(json.dumps(scenarios))
    classpath = (oracle_dir / "cp.txt").read_text().strip()
    done = subprocess.run(
        ["java", "--add-exports", "java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED", "-cp", f"{oracle_dir}:{classpath}", "loot_oracle",
         "timer-queue", str(work / "timer-in.json"), str(work / "timer-out.json")],
        stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True,
    )
    if done.returncode != 0:
        raise SystemExit("oracle failed:\n" + done.stderr[-1500:])
    return json.loads((work / "timer-out.json").read_text())


def run_oracle(oracle_dir, work, rows):
    work.mkdir(parents=True, exist_ok=True)
    (work / "dragon-in.json").write_text(json.dumps(rows))
    classpath = (oracle_dir / "cp.txt").read_text().strip()
    done = subprocess.run(
        ["java", "--add-exports", "java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED", "-cp", f"{oracle_dir}:{classpath}", "loot_oracle",
         "dragon-fight", str(work / "dragon-in.json"), str(work / "dragon-out.json")],
        stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True,
    )
    if done.returncode != 0:
        raise SystemExit("oracle failed:\n" + done.stderr[-1500:])
    return json.loads((work / "dragon-out.json").read_text())


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--oracle-dir", type=Path, required=True)
    parser.add_argument("--work", type=Path, default=Path("/tmp/level-data-work"))
    parser.add_argument("--sample", type=int, default=4, help="keep every Nth mutant (1 keeps all)")
    parser.add_argument("--full-out", type=Path)
    args = parser.parse_args()
    rng = random.Random(20261108)
    seeds = [0, 1, -1, 12345, 2**31 - 1, -(2**31), 2**63 - 1, -(2**63), 6710439052367273113] + [rng.randrange(-(2**63), 2**63) for _ in range(300)]
    rows = [{"op": "mutants", "snbt": base} for base in BASES] + [{"op": "gateways", "seed": seed} for seed in seeds]
    results = run_oracle(args.oracle_dir, args.work, rows)
    data = [r for r in results if "payload" in r]
    gateways = [r for r in results if "gateways" in r]
    timer = timer_scenarios(rng)
    timer_results = run_timer_oracle(args.oracle_dir, args.work, timer)
    timer_rows = [{"steps": scenario["steps"], "expect": result, "random": bool(scenario.get("random"))} for scenario, result in zip(timer, timer_results)]
    meta = {
        "oracle": "tools/loot_oracle.java dragon-fight and timer-queue (unchanged EndDragonFight.Data.CODEC over NbtOps, Util.shuffle, TimerQueue with the real callbacks)",
        "generator": "tools/level_data_generate.py",
        "dragon_fight_rows": len(data),
        "accepted": sum(1 for r in data if r["ok"]),
        "gateway_seeds": len(gateways),
    }
    if args.full_out:
        args.full_out.write_text(json.dumps({"meta": meta, "dragon_fight": data, "gateways": gateways, "timer_queue": timer_rows}))
    stride = max(1, args.sample)
    kept = data[::stride]
    random_timer = [row for row in timer_rows if row["random"]]
    kept_timer = [row for row in timer_rows if not row["random"]] + random_timer[::stride]
    meta["sampled_from"] = {"dragon_fight": len(data), "timer_queue": len(timer_rows)}
    meta["timer_queue_scenarios"] = len(kept_timer)
    meta["dragon_fight_rows"] = len(kept)
    meta["accepted"] = sum(1 for r in kept if r["ok"])
    (ROOT / "fixtures" / "level-data-vectors.json").write_text(
        json.dumps({"meta": meta, "dragon_fight": kept, "gateways": gateways, "timer_queue": kept_timer}, separators=(",", ":")) + "\n")
    print(f"{len(data)} dragon fight payloads ({meta['accepted']} kept-accepted of {len(kept)} kept), {len(gateways)} gateway seeds, "
          f"{len(timer_rows)} timer scenarios (kept {len(kept_timer)})")


if __name__ == "__main__":
    main()
