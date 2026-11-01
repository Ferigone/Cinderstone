#!/usr/bin/env python3
"""Reproducible unchanged-1.21 ItemEntity ServerEntity sender/cache vectors."""
import base64
import hashlib
import json
import pathlib
import random
import subprocess

ROOT = pathlib.Path(__file__).resolve().parent.parent
ORACLE = ROOT.parent / "mc121-oracle"
JDK = pathlib.Path("/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin")
JAR = ORACLE / "server-inner-1.21.jar"
SHA = "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"
CLASSPATH = ":".join(map(str, [JAR, *sorted((ORACLE / "libraries").rglob("*.jar"))]))
EXPORTS = ["--add-exports", "java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED"]


def initial(count=40, item="stone", velocity=None, no_gravity=False, position=None):
    return {"stack": {"id": "minecraft:" + item, "count": count},
            "position": position or {"x": 0.0, "y": 64.0, "z": 0.0, "yaw": 0.0, "pitch": 0.0, "on_ground": False},
            "velocity": velocity or [0.0, 0.0, 0.0], "no_gravity": no_gravity}


def cases():
    out = []
    def add(name, steps, init=None):
        out.append({"name": name, "initial": init or initial(), "steps": steps})
    add("constructor_dirty_and_pairing", [{"op": "pair"}, {"pair": True}, {}, {"op": "pair"}])
    add("partial_pickup_shaped_shrink_keeps_cache", [{}, {"shrink": 7, "pair": True}, *[{} for _ in range(21)], {"op": "pair"}])
    add("actual_partial_player_touch_keeps_cached_count", [{}, {"pickup": 7, "pair": True}, {"silent": True, "pair": True}, {"pickup": 8, "pair": True}, {"op": "pair"}])
    add("actual_player_touch_no_capacity", [{}, {"pickup": 0, "pair": True}, {"pickup": 0, "hurt": True, "pair": True}])
    add("actual_full_player_touch_restores_count_after_discard", [{}, {"pickup": 100, "pair": True}])
    add("same_reference_after_shrink_stays_clean", [{}, {"shrink": 6, "set_same_reference": True, "pair": True}, {"no_gravity": True, "pair": True}])
    add("in_place_count_then_unrelated_silent_refresh", [{}, {"count": 31, "pair": True}, {"silent": True, "pair": True}, {"count": 28, "pair": True}, {"silent": False, "pair": True}])
    add("same_contents_new_stack_is_dirty", [{}, {"stack": {"id": "minecraft:stone", "count": 40}, "pair": True}, {"stack": {"id": "minecraft:stone", "count": 40}, "pair": True}])
    add("new_stack_refresh_before_send_and_late_pair", [{}, {"op": "pair", "stack": {"id": "minecraft:dirt", "count": 12}}, {"pair": True}, {"op": "pair"}])
    add("generic_dirty_captures_slot_with_no_slot_packet", [{}, {"count": 29}, {"air_supply": 123, "pair": True}, {"count": 23}, {"no_gravity": True, "pair": True}, {"ticks_frozen": 32, "pair": True}, {"shared_flags": 0x42, "pair": True}])
    add("hurt_ignores_interval_and_does_not_update_sent_velocity", [{}, {"velocity": [.2, -.3, .4], "hurt": True, "pair": True}, {"op": "pair"}, {"hurt": True, "pair": True}, *[{} for _ in range(18)], {"pair": True}])
    add("impulse_updates_then_hurt_extra_packet", [{}, {"velocity": [.125, .25, -.5], "impulse": True, "hurt": True, "silent": True, "pair": True}, {}, {"impulse": True, "hurt": True, "pair": True}])
    add("tick_zero_hurt_and_impulse", [{"velocity": [1., 2., 3.], "impulse": True, "hurt": True, "pair": True}, {"pair": True}])
    add("physical_changes_not_published_before_interval", [{}, {"op": "pair", "velocity": [3., 2., 1.], "position": {"x": 2., "y": 65., "z": 4., "yaw": 50., "pitch": 30., "on_ground": True}}, {"pair": True}, *[{} for _ in range(19)], {"pair": True}])
    add("merge_destination_new_stack_dirty", [{}, {"merge_destination": {"id": "minecraft:stone", "count": 10}, "pair": True}], initial(60))
    add("merge_source_partial_shrink_not_dirty", [{}, {"merge_source": {"id": "minecraft:stone", "count": 60}, "pair": True}, {"silent": True, "pair": True}], initial(10))
    add("default_metadata_is_absent_after_reset", [{}, {"no_gravity": True, "silent": True, "air_supply": 20, "pair": True}, {"no_gravity": False, "silent": False, "air_supply": 300, "pair": True}])
    add("empty_nondefault_reference_keeps_slot8_empty_wire", [{}, {"count": 0, "pair": True}, {"silent": True, "pair": True}, {"silent": False, "pair": True}])
    add("canonical_empty_reset_cache_null_repeat_no_dirty_restore", [{}, {"empty_reference": True, "pair": True}, {"empty_reference": True, "pair": True}, {"silent": True, "pair": True}, {"empty_reference": True, "pair": True}, {"silent": False, "pair": True}, {"stack": {"id": "minecraft:stone", "count": 12}, "pair": True}])
    add("canonical_empty_pending_pair_keeps_old_cache", [{}, {"op": "pair", "empty_reference": True}, {"op": "pair"}, {"pair": True}, {"op": "pair"}, {"stack": {"id": "minecraft:dirt", "count": 8}, "pair": True}])
    add("canonical_empty_before_constructor_dirty_drains", [{"op": "pair", "empty_reference": True}, {"pair": True}, {"empty_reference": True, "pair": True}, {"air_supply": 75, "pair": True}, {"air_supply": 300, "pair": True}])
    custom = initial(3, "iron_pickaxe")
    custom["stack"]["components"] = {"minecraft:damage": 5, "minecraft:custom_name": '"Original sender"'}
    add("component_stack_count_and_cache_copy", [{}, {"count": 2, "pair": True}, {"silent": True, "pair": True}, {"stack": custom["stack"], "pair": True}], custom)
    # Deterministic mixed histories exercise both the 20 tick and 60 tick gates,
    # physical-to-sent baselines, exact byte ordering, and metadata identity.
    rng = random.Random(7672101)
    velocities = [[0., 0., 0.], [.0001, 0., 0.], [.0004, -.0003, .0002], [.1, -.2, .3], [3.9, -3.9, 4.0], [-4., 3.89999, -1e-9]]
    for number in range(72):
        pos = {"x": rng.uniform(-20, 20), "y": rng.uniform(-40, 160), "z": rng.uniform(-20, 20), "yaw": rng.uniform(-720, 720), "pitch": rng.uniform(-180, 180), "on_ground": bool(number % 2)}
        init = initial(1 + number % 64, "iron_pickaxe" if number % 7 == 0 else "stone", velocities[number % len(velocities)], bool(number % 3 == 0), pos)
        steps = [{"op": "pair"}, {}]
        flags = 0
        for tick in range(67):
            step = {}
            if tick % 4 == 1:
                step["count"] = 1 + rng.randrange(64)
            if tick % 9 == 2:
                step["stack"] = {"id": "minecraft:dirt" if tick % 2 else "minecraft:stone", "count": 1 + rng.randrange(64)}
            if tick % 13 == 3:
                step["silent"] = bool((tick // 13) % 2)
            if tick % 17 == 4:
                step["air_supply"] = 20 + tick
            if tick % 7 == 2:
                step["velocity"] = velocities[(number + tick) % len(velocities)]
            if tick % 5 == 1:
                pos = {**pos, "x": pos["x"] + rng.choice([1 / 4096, .002, 7.9999, -8.01]), "yaw": pos["yaw"] + rng.choice([0., 1.4, -90., 720.])}
                step["position"] = pos
            if tick % 11 == 2:
                step["impulse"] = True
            if tick % 19 == 5:
                step["hurt"] = True
            if tick % 23 == 7:
                flags ^= 0x42
                step["shared_flags"] = flags
            if tick % 8 == 3:
                step["pair"] = True
            steps.append(step)
        out.append({"name": f"mixed_history_{number}", "initial": init, "steps": steps})
    return out


def main():
    assert hashlib.sha256(JAR.read_bytes()).hexdigest() == SHA
    build = ROOT / "target/item-replication-oracle"
    logs = ROOT / "runtime/item_replication"
    build.mkdir(parents=True, exist_ok=True); logs.mkdir(parents=True, exist_ok=True)
    sources = [ROOT / "tools/item_entity_oracle.java", ROOT / "tools/item_replication_oracle.java"]
    compile_result = subprocess.run([str(JDK / "javac"), *EXPORTS, "-proc:none", "-cp", CLASSPATH, "-d", str(build), *map(str, sources)], capture_output=True, text=True)
    (logs / "compile.log").write_text(compile_result.stdout + compile_result.stderr)
    assert compile_result.returncode == 0, compile_result.stderr
    queries = cases()
    run = subprocess.run([str(JDK / "java"), *EXPORTS, "-cp", str(build) + ":" + CLASSPATH, "oracle.item_replication_oracle"], input="\n".join(json.dumps(case, separators=(",", ":")) for case in queries) + "\n", capture_output=True, text=True)
    (logs / "oracle.stdout.log").write_text(run.stdout); (logs / "oracle.stderr.log").write_text(run.stderr)
    assert run.returncode == 0, run.stderr[-6000:]
    results = [json.loads(line.split("ORACLE:", 1)[1]) for line in run.stdout.splitlines() if "ORACLE:" in line]
    assert len(results) == len(queries), (len(results), len(queries), run.stderr[-6000:])
    errors = [(case["name"], case["error"]) for case in results if "error" in case]
    assert not errors, errors[:10]
    by_name = {case["name"]: case for case in results}
    ordering = by_name["impulse_updates_then_hurt_extra_packet"]["steps"][1]["expected"]
    assert [packet["id"] for packet in ordering["packets"]] == [0x5a, 0x58, 0x5a]
    assert [state["tick_count"] for state in ordering["emission_states"]] == [1, 1, 2]
    assert [state["has_impulse"] for state in ordering["emission_states"]] == [True, True, False]
    assert [state["hurt_marked"] for state in ordering["emission_states"]] == [True, True, False]
    assert [state["dirty"] for state in ordering["emission_states"]] == [True, False, False]
    partial = by_name["actual_partial_player_touch_keeps_cached_count"]["steps"][1]["expected"]
    assert partial["physical_stack"]["count"] == 33 and partial["dirty_mask"] == 0 and partial["packets"] == []
    assert partial["pickup_callbacks"][1] == {"callback": "collect", "count": 40}
    assert partial["cached_metadata"] == by_name["actual_partial_player_touch_keeps_cached_count"]["initial_expected"]["cached_metadata"]
    empty = by_name["canonical_empty_reset_cache_null_repeat_no_dirty_restore"]["steps"]
    assert empty[1]["expected"]["physical_stack_reference_is_default"] is True
    assert empty[1]["expected"]["cached_metadata"] is None
    assert [packet["id"] for packet in empty[1]["expected"]["pairing_packets"]] == [0x01, 0x5a]
    assert empty[2]["expected"]["packets"] == [] and empty[2]["expected"]["dirty_mask"] == 0
    assert [packet["id"] for packet in empty[3]["expected"]["pairing_packets"]] == [0x01, 0x58, 0x5a]
    fixture = ROOT / "fixtures/item_replication_sender.json"
    fixture.write_text(json.dumps({"minecraft": "1.21", "protocol": 767, "oracle_inner_sha256": SHA, "schema": 1, "oracle": "unchanged original aqt.a()/sendChanges and aqt.a(aqv,Consumer)/sendPairingData on actual cjh/ItemEntity; environmental Level adapter only", "cases": results}, separators=(",", ":")) + "\n")
    snapshots = [case["initial_expected"] for case in results] + [step["expected"] for case in results for step in case["steps"]]
    packet_count = sum(len(state["packets"]) + len(state.get("pairing_packets", [])) for state in snapshots)
    packet_bytes = sum(len(base64.b64decode(packet["payload"])) for state in snapshots for packet in state["packets"] + state.get("pairing_packets", []))
    report = {"minecraft": "1.21", "protocol": 767, "oracle_inner_sha256": SHA, "actual_original_classes": ["aqt", "cjh", "aka", "cuq"], "original_method": "ServerEntity.sendChanges + sendPairingData", "cases": len(results), "states": len(snapshots), "packets": packet_count, "packet_payload_bytes": packet_bytes, "oracle_errors": 0, "original_class_sources": [line.split("SOURCE:", 1)[1] for line in run.stdout.splitlines() if "SOURCE:" in line], "scope": ["unmounted actual ItemEntity; empty passengers", "new-stack identity versus in-place count mutation", "actual original inner merge method and original ItemStack shrink", "actual original ItemEntity.playerTouch with controlled inventory capacity and captured collect/stat/onPickup callbacks", "dirty metadata refresh of full pairing cache", "interval/impulse/hurtMarked ordering and sent baselines", "consumer captures tick/impulse/hurt/dirty/sent-velocity state at every packet emission", "late pairing uses cached metadata and tracked position/velocity"], "aqt_bytecode_offsets": {"constructor_cache": "127-135", "main_gate": "211-243", "regular_velocity_before_movement": "858-1057", "movement_send": "1062-1073", "dirty_metadata": "1078-1079", "impulse_reset": "1173-1178", "tick_increment": "1181-1188", "hurt_reset_then_extra_velocity": "1191-1221", "dirty_pack_refresh_cache": "private g():8-22"}, "limitations": ["controlled environmental Level and external pickup inventory/player callbacks; no complete running-world tick proof", "no mounted entity or passenger branches", "native Rust comparison is performed separately by parent"], "files": {str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest() for path in [*sources, pathlib.Path(__file__).resolve(), fixture]}}
    evidence = ROOT / "evidence/item-replication-oracle.json"
    evidence.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({key: report[key] for key in ["cases", "states", "packets", "packet_payload_bytes", "oracle_errors"]}))


if __name__ == "__main__":
    main()
