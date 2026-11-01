#!/usr/bin/env python3
"""Execute unchanged Java 1.21 local listeners and real synchronizer packets."""
from pathlib import Path
import copy
import hashlib
import json
import os
import random
import subprocess

ROOT = Path(__file__).resolve().parents[1]
ORACLE = ROOT.parent / "mc121-oracle"
JDK = Path("/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin")
SHA = "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"


def stack(item="stone", count=1, components=None):
    return {"id": "minecraft:" + item, "count": count, **({"components": components} if components else {})}


def cases():
    out = []
    def add(name, ops, slots=None, carried=None, state=0, native=True):
        out.append(dict(name=name, slots=slots or [None]*46, carried=carried, state_id=state,
                        native=native, operations=copy.deepcopy(ops)))
    filled = [stack("stone" if i%2 else "dirt", 1+i%30) for i in range(46)]
    add("all_slots_local_then_remote_and_inventory_identity_filter",
        [dict(op="add"), dict(op="add"), dict(op="broadcast"), dict(op="full"), dict(op="send_all")], filled, stack("arrow", 3), 32766)
    for slot in range(46):
        slots = [None]*46; slots[slot] = stack(count=slot+1)
        add(f"individual_inventory_filter_{slot}", [dict(op="add"), dict(op="set_slot",slot=slot,stack=None), dict(op="broadcast"), dict(op="broadcast")], slots)
    add("send_all_leaves_local_stale_even_after_registration",
        [dict(op="send_all"), dict(op="add"), dict(op="set_slot",slot=9,stack=stack(count=4)),
         dict(op="send_all"), dict(op="broadcast"), dict(op="set_slot",slot=9,stack=None),
         dict(op="send_all"), dict(op="full"), dict(op="full")], filled)
    add("broadcast_without_listener_updates_shared_local_baseline_before_add",
        [dict(op="broadcast"),dict(op="add"),dict(op="set_slot",slot=36,stack=stack(count=8)),
         dict(op="broadcast"),dict(op="add"),dict(op="set_slot",slot=45,stack=stack("arrow",3)),dict(op="full")],filled)
    add("predicted_equal_remote_still_triggers_local_and_criteria",
        [dict(op="add"),dict(op="set_slot",slot=9,stack=stack(count=3)),
         dict(op="remote_slot",slot=9,stack=stack(count=3)),dict(op="broadcast"),
         dict(op="set_slot",slot=45,stack=stack("arrow",4)),dict(op="remote_slot",slot=45,stack=stack("arrow",4)),
         dict(op="carried",stack=stack("apple",2)),dict(op="remote_carried",stack=stack("apple",2)),dict(op="broadcast"),
         dict(op="remote_slot",slot=9,stack=stack(count=5)),dict(op="broadcast")])
    add("stale_full_local_diff_before_initial_regardless_of_prediction",
        [dict(op="add"),dict(op="set_slot",slot=36,stack=stack(count=4)),
         dict(op="remote_slot",slot=36,stack=stack(count=4)),dict(op="full"),dict(op="full")],state=32767)
    add("copies_survive_live_in_place_counts_and_empty_copy_is_canonical",
        [dict(op="add"),dict(op="count",slot=9,count=5),dict(op="broadcast"),dict(op="count",slot=9,count=0),
         dict(op="broadcast"),dict(op="count",slot=9,count=7),dict(op="full"),dict(op="count",slot=9,count=-4),dict(op="full")],
        [stack(count=3) if i==9 else None for i in range(46)])
    add("noncanonical_empty_setter_and_remote_copy_canonicalization",
        [dict(op="add"),dict(op="set_slot",slot=9,stack=stack(count=2),count=0),
         dict(op="remote_slot",slot=9,stack=stack(count=3),count=0),dict(op="broadcast"),
         dict(op="count",slot=9,count=4),dict(op="broadcast"),dict(op="set_slot",slot=9,stack=None),dict(op="broadcast")])
    add("set_synchronizer_uses_send_all_and_preserves_local_cache",
        [dict(op="set_sync"),dict(op="add"),dict(op="set_slot",slot=36,stack=stack("apple",5)),
         dict(op="set_sync"),dict(op="full")],filled)
    add("multiple_listener_shared_baseline_not_new_listener_initial_snapshot",
        [dict(op="add"),dict(op="add",listener="other"),dict(op="set_slot",slot=9,stack=stack(count=3)),
         dict(op="broadcast"),dict(op="remove"),dict(op="set_slot",slot=9,stack=stack(count=5)),
         dict(op="broadcast"),dict(op="add"),dict(op="remove",listener="other"),dict(op="add")],filled,native=False)
    add("suppressed_remote_still_updates_local_then_full_ignores_suppression",
        [dict(op="suppress"),dict(op="add"),dict(op="set_slot",slot=9,stack=stack(count=3)),
         dict(op="broadcast"),dict(op="carried",stack=stack("arrow",3)),dict(op="send_all"),
         dict(op="set_slot",slot=45,stack=stack("arrow",6)),dict(op="full"),dict(op="resume"),dict(op="broadcast")],filled,native=False)
    add("remote_no_copy_alias_can_hide_count_mutation_but_not_local_diff",
        [dict(op="add"),dict(op="remote_live",slot=9),dict(op="count",slot=9,count=7),dict(op="broadcast"),
         dict(op="count",slot=9,count=0),dict(op="broadcast"),dict(op="count",slot=9,count=2),dict(op="full")],
        [stack(count=3) if i==9 else None for i in range(46)],native=False)
    rng = random.Random(121767456)
    pool = [None,stack(count=1),stack(count=3),stack(count=64),stack("dirt",8),stack("arrow",4),
            stack("iron_pickaxe",1,{"minecraft:damage":5}),stack(count=2,components={"minecraft:custom_name":'"Local copy"'})]
    for number in range(48):
        slots = [copy.deepcopy(rng.choice(pool)) if rng.random()<.2 else None for _ in range(46)]
        ops = [dict(op="add")] if number%3 else [dict(op="send_all"),dict(op="add")]
        for step in range(30):
            slot = rng.randrange(46); value = copy.deepcopy(rng.choice(pool))
            choice = rng.randrange(10)
            if choice<3:
                ops.append(dict(op="set_slot",slot=slot,stack=value))
                if choice==0:ops.append(dict(op="remote_slot",slot=slot,stack=value))
            elif choice==3:ops.append(dict(op="count",slot=slot,count=rng.choice([0,1,2,7,32,-1])))
            elif choice==4:ops.append(dict(op="carried",stack=value))
            elif choice==5:ops.append(dict(op="remote_slot",slot=slot,stack=value))
            else:ops.append(dict(op=["broadcast","full","send_all","add"][choice-6]))
        ops.extend([dict(op="broadcast"),dict(op="full")])
        add(f"mixed_history_{number}",ops,slots,copy.deepcopy(rng.choice(pool)),[0,32766,32767][number%3])
    return out


def main():
    jar=ORACLE/"server-inner-1.21.jar";assert hashlib.sha256(jar.read_bytes()).hexdigest()==SHA
    runtime=ROOT/"runtime/container-local-listener";build=runtime/"classes";build.mkdir(parents=True,exist_ok=True)
    cp=os.pathsep.join(map(str,[build,jar,*sorted((ORACLE/"libraries").rglob("*.jar"))]))
    exports=["--add-exports","java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED"]
    sources=["tools/container_oracle.java","tools/inventory_normalization_oracle.java","tools/container_local_listener_oracle.java"]
    result=subprocess.run([JDK/"javac",*exports,"-proc:none","-cp",cp,"-d",build,*[ROOT/p for p in sources]],capture_output=True,text=True,cwd=ROOT)
    (runtime/"compile.log").write_text(result.stdout+result.stderr);assert result.returncode==0,result.stderr
    queries=cases();result=subprocess.run([JDK/"java",*exports,"-Xmx1G","-cp",cp,"oracle.container_local_listener_oracle"],cwd=ROOT,input="".join(json.dumps(q,separators=(",",":"))+"\n" for q in queries),capture_output=True,text=True,timeout=120)
    (runtime/"oracle.log").write_text(result.stdout+result.stderr);assert result.returncode==0,result.stderr[-6000:]
    results=[json.loads(line.split("ORACLE:",1)[1]) for line in result.stdout.splitlines() if line.startswith("ORACLE:")]
    assert len(results)==len(queries),(len(results),len(queries));assert not any("error" in row for row in results),[row for row in results if "error" in row][:3]
    rows=[]
    for q,r in zip(queries,results):
        assert q["name"]==r["name"];q["operations"]=r["expected"].pop("operations");rows.append(dict(query=q,expected=r["expected"]))
    header=dict(minecraft="1.21",protocol=767,oracle_inner_sha256=SHA,original_classes_modified=False,
                environment="Unmodified cpu/cqw/aqv$1/aqv$2/cuq codecs. Network sink and criteria-trigger callback captured; original inventory identity/ResultSlot gate executed. Existing controlled InventoryMenu recipe recalculation hook.")
    fixture=ROOT/"fixtures/container_local_listener.json";fixture.write_text(json.dumps(dict(**header,cases=rows),separators=(",",":"))+"\n")
    states=[s for row in rows for s in row["expected"]["states"]];events=[e for s in states for e in s["events"]]
    by_name={row["query"]["name"]:row for row in rows}
    all_events=by_name["all_slots_local_then_remote_and_inventory_identity_filter"]["expected"]["states"][0]["events"]
    assert [e["slot"] for e in all_events if e["kind"]=="local"]==list(range(46))
    assert [e["slot"] for e in all_events if e["kind"]=="criteria"]==list(range(5,46))
    predicted=by_name["predicted_equal_remote_still_triggers_local_and_criteria"]["expected"]["states"][3]
    assert [e["kind"] for e in predicted["events"]]==["local","criteria"]
    stale=by_name["send_all_leaves_local_stale_even_after_registration"]["expected"]["states"]
    assert all(e["kind"]=="packet" for e in stale[0]["events"])
    assert stale[0]["last_slots"]==["AA=="]*46 and stale[0]["remote_slots"]==stale[0]["slots"]
    report=dict(ok=True,**header,cases=len(rows),operations=len(states),native_cases=sum(q["native"] for q in queries),
                native_operations=sum(len(q["operations"]) for q in queries if q["native"]),
                local_callbacks=sum(e["kind"]=="local" for e in events),criteria_identity_gate_calls=sum(e["kind"]=="criteria" for e in events),packets=sum(e["kind"]=="packet" for e in events),
                original_sources=[line.split("SOURCE:",1)[1] for line in result.stdout.splitlines() if line.startswith("SOURCE:")],
                fixture_sha256=hashlib.sha256(fixture.read_bytes()).hexdigest(),
                source_sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in sources+["tools/container_local_listener_generate.py","tools/container_local_listener_tests.rs","tools/container_local_listener_audit.md"]},
                verified=["lastSlots vs remoteSlots initialization and independent copied baselines","add/remove listener dedup and insertion order; existing baseline is global","broadcastChanges slot-ascending local-before-remote timeline; cursor remote-only","broadcastFullState local loop before Initial; sendAll/setSynchronizer remote-only even suppressed","predicted remote equality suppresses network without suppressing local listener","actual aqv$2 ResultSlot exclusion and Inventory identity gate for all46slots","in-place count changes, retained callback copies, canonical empty copies; shared local/remote reference in broadcastChanges","real aqv$1 packet codecs/state-id wrapping; simple default/damage/name components"],
                not_verified=["Mutating/reentrant local listeners; native API intentionally records immutable observations","Advancement predicates/rewards: cg callback captured after original aqv$2 gate","Recipe-result recalculation (controlled existing hook)","Arbitrary complex component Object.equals semantics and all custom data patches","Oracle-only multi-listener/remove/suppression/setRemoteSlotNoCopy cases are not native API claims"])
    (ROOT/"evidence/container-local-listener-oracle.json").write_text(json.dumps(report,indent=2)+"\n")
    print(json.dumps({k:report[k] for k in ["cases","operations","native_cases","native_operations","local_callbacks","criteria_identity_gate_calls","packets","fixture_sha256"]}))


if __name__=="__main__":main()
