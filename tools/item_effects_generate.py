#!/usr/bin/env python3
"""Unchanged original packet constructors/codecs and protocol registration; all float fields raw bits."""
from pathlib import Path
import hashlib
import json
import math
import os
import random
import struct
import subprocess

ROOT = Path(__file__).resolve().parents[1]
ORACLE = ROOT.parent / "mc121-oracle"
JDK = Path("/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin")
SHA = "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"
SOUNDS = ["minecraft:entity.generic.burn", "minecraft:entity.generic.extinguish_fire", "minecraft:entity.generic.splash"]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def bits(value):
    return struct.pack(">d", value).hex()


def sound_cases():
    coords = [0., -0., math.ulp(0.), -math.ulp(0.), math.nextafter(.125, 0), .125,
              math.nextafter(.125, 1), -.125, math.nextafter(-.125, 0), math.nextafter(-.125, -1),
              1., -1., 1.875, -1.875, 29999984., -29999984., 268435455.875,
              math.nextafter(268435455.875, -math.inf), math.nextafter(268435455.875, math.inf),
              -268435456., math.nextafter(-268435456., -math.inf), math.nextafter(-268435456., 0),
              1e300, -1e300, math.inf, -math.inf]
    raw_coords = [bits(c) for c in coords] + ["7ff0000000000001", "fff8123456789abc"]
    scalars = [0, 0x80000000, 1, 0x80000001, 0x3F7FFFFF, 0x3F800000, 0x3F800001,
               0x3FB33333, 0x3F333333, 0xBF800000, 0x7F7FFFFF, 0x7F800000,
               0xFF800000, 0x7FA00001, 0xFFC12345]
    seeds = ["0", "-1", "1", "-9223372036854775808", "9223372036854775807"]
    cases = []
    for sound in SOUNDS:
        for i, raw in enumerate(raw_coords):
            p = [bits(.5), bits(64.), bits(-.5)]
            p[i % 3] = raw
            cases.append({"sound_name": sound, "source_ordinal": 8, "position_bits": p,
                          "volume_bits": "3f333333", "pitch_bits": "3fcccccd", "seed": seeds[i % len(seeds)]})
        for i, scalar in enumerate(scalars):
            cases.append({"sound_name": sound, "source_ordinal": 8,
                          "position_bits": [bits(1.125), bits(-.125), bits(30000000.)],
                          "volume_bits": f"{scalar:08x}", "pitch_bits": f"{scalars[(i + 3) % len(scalars)]:08x}",
                          "seed": seeds[i % len(seeds)]})
    rng = random.Random(0x121EFFEC7)
    for i in range(48):
        cases.append({"sound_name": SOUNDS[i % 3], "source_ordinal": 8,
                      "position_bits": [f"{rng.getrandbits(64):016x}" for _ in range(3)],
                      "volume_bits": f"{rng.getrandbits(32):08x}", "pitch_bits": f"{rng.getrandbits(32):08x}",
                      "seed": str(rng.randrange(-2**63, 2**63))})
    return cases


def level_cases():
    cases = [{"event": 1009, "position": [0, 64, 0], "data": 1, "global": False}]
    values = [-2147483648, -33554433, -33554432, -2049, -2048, -1, 0, 1,
              2047, 2048, 33554431, 33554432, 2147483647]
    for i, value in enumerate(values):
        for axis in range(3):
            position = [1, 64, -1]
            position[axis] = value
            cases.append({"event": 1009, "position": position, "data": 1, "global": bool(i % 2)})
    rng = random.Random(0x1211E7E17)
    for i in range(48):
        cases.append({"event": [1009, 2001, 0, -1, 2147483647, -2147483648][i % 6],
                      "position": [rng.randrange(-2**31, 2**31) for _ in range(3)],
                      "data": rng.randrange(-2**31, 2**31), "global": bool(i % 2)})
    return cases


def main():
    jar = ORACLE / "server-inner-1.21.jar"
    assert sha(jar) == SHA
    runtime = ROOT / "runtime/item-effects"
    classes = runtime / "classes"
    classes.mkdir(parents=True, exist_ok=True)
    cp = os.pathsep.join(map(str, [classes, jar, *sorted((ORACLE / "libraries").rglob("*.jar"))]))
    source = ROOT / "tools/item_effects_oracle.java"
    compile_result = subprocess.run([JDK / "javac", "-proc:none", "-cp", cp, "-d", classes, source], capture_output=True, text=True)
    if compile_result.returncode:
        raise RuntimeError(compile_result.stdout + compile_result.stderr)
    sounds, levels = sound_cases(), level_cases()
    queries = sounds + levels
    run = subprocess.run([JDK / "java", "-cp", cp, "oracle.item_effects_oracle"],
                         input="".join(json.dumps(q) + "\n" for q in queries), text=True, capture_output=True, timeout=120)
    (runtime / "oracle.log").write_text(run.stdout + run.stderr)
    if run.returncode:
        raise RuntimeError((run.stdout + run.stderr)[-5000:])
    metadata = json.loads(next(line[7:] for line in run.stdout.splitlines() if line.startswith("HEADER:")))
    assert metadata["source"].endswith("server-inner-1.21.jar")
    assert metadata["source_ordinal"] == 8
    results = [json.loads(line[7:]) for line in run.stdout.splitlines() if line.startswith("ORACLE:")]
    assert len(results) == len(queries), (len(results), len(queries))
    assert len(metadata["packet_ids"]) == 2
    for query, expected in zip(queries, results):
        query["expected"] = expected
    fixture = {"minecraft": "1.21", "protocol": 767, "oracle_inner_sha256": SHA,
               "packet_ids": metadata["packet_ids"], "source_name": metadata["source_name"],
               "sounds": metadata["sounds"], "sound_cases": sounds, "level_event_cases": levels}
    destination = ROOT / "fixtures/item-effects-vectors.json"
    destination.write_text(json.dumps(fixture, separators=(",", ":")) + "\n")
    bytecode = subprocess.run([JDK / "javap", "-classpath", jar, "-p", "-c", "afp", "adh", "avo", "agh", "zl$1"],
                              capture_output=True, text=True, check=True).stdout
    (runtime / "original-bytecode.txt").write_text(bytecode)
    report = {"status": "PASS", "minecraft": "1.21", "oracle_inner_sha256": SHA,
              "fixture_sha256": sha(destination), "oracle_source_sha256": sha(source),
              "generator_source_sha256": sha(Path(__file__)),
              "sound_cases": len(sounds), "level_event_cases": len(levels),
              "packet_ids": metadata["packet_ids"], "sounds": metadata["sounds"],
              "source_name": metadata["source_name"], "source_ordinal": metadata["source_ordinal"],
              "packet_id_proof": "Original agh.b CLIENTBOUND_TEMPLATE.listPackets wh$a$a visitor IDs cross-recorded with original bound ProtocolInfo IdDispatchCodec.encode full packet body.",
              "constructors": "Unmodified afp(Holder,SoundSource,double,double,double,float,float,long) and adh(int,BlockPos,int,boolean).",
              "payload_proof": "Each original STREAM_CODEC encodes constructor result then decodes to verify complete consumption. Full packet includes original protocol packet ID, without transport length prefix.",
              "coordinate_proof": "Original constructor e/f/g int fields and public f/g/h getters, recorded independently; getters use i2f /8f then f2d.",
              "range_proof": "Actual registered SoundEvent.getRange(volume) called on original volume raw f32 bits; fixedRange() also recorded.",
              "original_classes_modified": False, "native_implementation_verified": False,
              "limits": ["Only the three registered ItemEntity sounds and SoundSource.AMBIENT are in scope.",
                         "Broadcast recipient selection, RNG seed consumption and effect triggering are separate host behavior.",
                         "Raw signalling-NaN results are observed on the pinned JDK/platform.",
                         "Full packet bytes exclude network compression, encryption and VarInt packet length."]}
    (ROOT / "evidence/item-effects-oracle.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
