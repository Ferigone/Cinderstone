#!/usr/bin/env python3
"""Focused constructor/STREAM fixtures. Existing oracle, original classes and old corpus stay unchanged."""
from pathlib import Path
import hashlib
import itertools
import json
import os
import struct
import subprocess

from inventory_normalization_generate import SHA, JDK, ROOT, ORACLE, b64, slot, vi

OLD_ORACLE_SHA = "d5f6913082d975885e4f248ec0b6cf0e23ea2fbd1f4a67ab75bfc1b0df0a07ca"


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def f32(bits):
    return bits.to_bytes(4, "big")


def mutf(units):
    """Build raw DataOutput.writeUTF input from UTF-16 units, without Python Unicode normalization."""
    raw = bytearray()
    for c in units:
        if 0 < c < 0x80:
            raw.append(c)
        elif c <= 0x7FF:
            raw.extend([0xC0 | (c >> 6), 0x80 | (c & 63)])
        else:
            raw.extend([0xE0 | (c >> 12), 0x80 | ((c >> 6) & 63), 0x80 | (c & 63)])
    return len(raw).to_bytes(2, "big") + raw


def ascii_utf(value):
    return mutf(list(value.encode("ascii")))


def wire_string(value):
    raw = value.encode("utf-8")
    return vi(len(raw)) + raw


def nbt_string(units):
    return b"\x08" + mutf(units)


def compound(key, child):
    return b"\x0a" + child[:1] + ascii_utf(key) + child[1:] + b"\0"


def details(amp=1, duration=60, flags=(0, 1, 1), hidden=None):
    return vi(amp) + vi(duration) + bytes(flags) + bytes([hidden is not None]) + (hidden or b"")


def effect(amp=1, duration=60, flags=(0, 1, 1), hidden=None, kind=1):
    return vi(kind) + details(amp, duration, flags, hidden)


def food(effects=(), conversion=None, nutrition=1, saturation=0x3F800000,
         always=0, seconds=0x3F800000):
    return (vi(nutrition) + f32(saturation) + bytes([always]) + f32(seconds)
            + bytes([conversion is not None]) + (conversion or b"") + vi(len(effects))
            + b"".join(e + f32(probability) for e, probability in effects))


def potion(effects=(), color=None, potion_id=None):
    return (bytes([potion_id is not None]) + (vi(potion_id) if potion_id is not None else b"")
            + bytes([color is not None]) + (struct.pack(">i", color) if color is not None else b"")
            + vi(len(effects)) + b"".join(effects))


def build_cases():
    cases = []

    def add(name, group, raw, **descriptor):
        cases.append({"name": name, "group": group, "query": {"bytes": b64(raw)},
                      **({"descriptor": descriptor} if descriptor else {})})

    def component(name, group, kind, payload, **descriptor):
        add(name, group, slot(1, [(kind, payload)]), component_id=kind, **descriptor)

    # Unit uses a map codec, not a zero-byte marker and not arbitrary NBT coercion.
    primitive_roots = [b"\0", b"\1\1", b"\2\0\1", b"\3\0\0\0\1",
                       b"\4" + bytes(8), b"\5" + f32(0x7FA00001),
                       b"\6" + (0x8000000000000000).to_bytes(8, "big"),
                       b"\7\0\0\0\2\0\1", nbt_string([65]),
                       b"\11\3\0\0\0\1\0\0\0\1",
                       b"\13\0\0\0\1\0\0\0\1", b"\14\0\0\0\1" + bytes(8)]
    compounds = [b"\12\0", compound("probe", nbt_string([0, 0x03BB, 0xD800])),
                 compound("nested", compound("x", b"\5" + f32(0x80000000))),
                 compound("values", b"\11\5\0\0\0\2" + f32(0x7FA00001) + f32(0xFFC12345))]
    for i, raw in enumerate(primitive_roots + compounds):
        component(f"unit/root/{i}", "unit", 19, raw, nbt_hex=raw.hex())

    # Full hidden chains are decoded before encoder getters copy only the visible head.
    amps = [-2147483648, -1, 0, 1, 127, 254, 255, 256, 2147483647]
    durations = [-2147483648, -1, 0, 1, 127, 128, 2147483647]
    for kind in [20, 31]:
        for ai, amp in enumerate(amps):
            for depth in range(4):
                hidden = None
                for d in range(depth):
                    hidden = details(amps[(ai + d + 1) % len(amps)], durations[(ai + d) % len(durations)],
                                     (d % 2, (d + 1) % 2, d % 2), hidden)
                e = effect(amp, durations[(ai + depth) % len(durations)],
                           (ai % 2, depth % 2, (ai + depth) % 2), hidden)
                payload = food([(e, 0x3F000000)]) if kind == 20 else potion([e])
                # A second retained component proves the entire hidden chain was consumed.
                add(f"effect/{kind}/amp{amp}/depth{depth}", "effect", slot(1, [(kind, payload), (1, vi(63))]),
                    component_id=kind, amplifier=amp, depth=depth, duration=durations[(ai + depth) % len(durations)])
        for depth in [1, 2, 4, 8]:
            hidden = details(256, -1)
            for _ in range(depth - 1):
                hidden = details(-1, 128, hidden=hidden)
            e = effect(256, 2147483647, hidden=hidden)
            payload = food([(e, 0x7FA00001)]) if kind == 20 else potion([e], color=-1, potion_id=1)
            component(f"effect/{kind}/deep{depth}", "effect", kind, payload, depth=depth)
            component(f"effect/{kind}/truncated{depth}", "effect_truncated", kind, payload[:-1], depth=depth)
        for count in [0, 1, 2, 3, 8]:
            effects = [effect(amps[i % len(amps)], durations[i % len(durations)],
                              hidden=details(999, -1)) for i in range(count)]
            payload = food([(e, 0x3F800000) for e in effects]) if kind == 20 else potion(effects)
            component(f"effect/{kind}/list{count}", "effect_list", kind, payload, count=count)
    for i, bits in enumerate([0, 0x80000000, 1, 0x3F800000, 0xBF800000,
                              0x7F800000, 0xFF800000, 0x7FA00001, 0xFFC12345]):
        component(f"food/scalar_bits/{bits:08x}", "food_scalars", 20,
                  food(nutrition=durations[i % len(durations)], saturation=bits, always=i % 2, seconds=bits),
                  float_bits=f"{bits:08x}")

    # Instrument direct holder is sound holder + VAR_INT duration + f32 range, with no description.
    use_durations = [-2147483648, -1, 0, 1, 127, 128, 255, 16383, 16384, 2147483647]
    ranges = [0, 0x80000000, 1, 0x3F800000, 0xBF800000, 0x7F800000, 0x7FA00001]
    for duration, range_bits in itertools.product(use_durations, ranges):
        component(f"instrument/direct/{duration}/{range_bits:08x}", "instrument", 40,
                  b"\0" + vi(1) + vi(duration) + f32(range_bits),
                  use_duration=duration, range_bits=f"{range_bits:08x}")
    for duration in [0, 1, 128, -1]:
        for fixed in [None, 0x80000000, 0x7FA00001]:
            sound = b"\0" + vi(len("minecraft:probe")) + b"minecraft:probe" + bytes([fixed is not None])
            sound += f32(fixed) if fixed is not None else b""
            component(f"instrument/inline_sound/{duration}/{fixed}", "instrument_sound", 40,
                      b"\0" + sound + vi(duration) + f32(0x3F800000), use_duration=duration,
                      fixed_range_bits=None if fixed is None else f"{fixed:08x}")
    for instrument in range(8):
        component(f"instrument/registry/{instrument}", "instrument_reference", 40, vi(instrument + 1))

    # 1.21 trim material has itemModelIndex:f32, and override map keys are registry holders.
    for bits in [0, 0x80000000, 1, 0x3F000000, 0x3F800000, 0x7F800000, 0x7FA00001]:
        for overrides in [[], [(0, "probe")], [(1, "one"), (0, "zero")]]:
            material = (b"\0" + wire_string("probe") + vi(1) + f32(bits) + vi(len(overrides))
                        + b"".join(vi(k) + wire_string(v) for k, v in overrides) + nbt_string([77]))
            component(f"trim/inline_material/{bits:08x}/{len(overrides)}", "trim", 35,
                      material + vi(1) + b"\1", item_model_index_bits=f"{bits:08x}", overrides=overrides)
    for i in range(4):
        component(f"trim/references/{i}", "trim", 35, vi(i + 1) + vi(i + 1) + bytes([i % 2]))
    for decal in [0, 1]:
        for description in [nbt_string([80]), compound("text", nbt_string([80])), nbt_string([0xD800])]:
            pattern = b"\0" + wire_string("minecraft:probe") + vi(1) + description + bytes([decal])
            component(f"trim/inline_pattern/{decal}/{description.hex()}", "trim", 35,
                      vi(1) + pattern + b"\1", decal=decal)
    # Deliberately omitted model-index bytes reproduce the cross-version schema error.
    malformed_material = b"\0" + wire_string("probe") + vi(1) + vi(0) + nbt_string([77])
    component("trim/missing_model_index", "trim_rejected", 35, malformed_material + vi(1) + b"\1")

    # Constructor maps explicit BRICK to absent slots; ordered() emits exactly four bricks/items.
    patterns = [[], [1], [921], [0], [1, 921], [921, 1], [1, 0, 921],
                [0, 1, 921, 1332], [921] * 4, [1] * 4, [1332, 1331, 1330, 1329]]
    for values in patterns:
        component(f"pot/values/{'-'.join(map(str, values))}", "pot", 51,
                  vi(len(values)) + b"".join(vi(v) for v in values), items=values)
    for length in range(5):
        for index in range(max(1, length)):
            values = [921] * length
            if length:
                values[index] = 1
            component(f"pot/brick_padding/{length}/{index}", "pot", 51,
                      vi(length) + b"".join(vi(v) for v in values), items=values)
    for length in [5, 6, 128, -1]:
        component(f"pot/reject_length/{length}", "pot_rejected", 51, vi(length) + vi(1) * max(0, length))
    component("pot/unknown_item_default", "pot_boundary", 51, vi(1) + vi(999999))
    for item_id in [-1, -2147483648]:
        component(f"pot/unknown_item_default/{item_id}", "pot_boundary", 51, vi(1) + vi(item_id), item_id=item_id)
    for count in [1, 64]:
        add(f"slot/unknown_item_default/{count}", "slot_boundary", slot(999999, count=count), item_id=999999, count=count)

    # Child normalization must recurse through list and optional slots without collapsing list length.
    children = [b"\0", slot(993, [(1, vi(16))]), slot(1, [(1, vi(63))]),
                slot(1, [(0, compound("x", b"\5" + f32(0x80000000)))]),
                slot(1, [(5, compound("text", nbt_string([65, 0xD800])))]),
                slot(1, [(31, potion([effect(256, 128, hidden=details(999, -1))]))])]
    variants = [[], *[[c] for c in children], [children[1], b"\0", b"\0"],
                [b"\0", children[1], b"\0"], [children[3], children[4]], children]
    for kind in [29, 30, 52]:
        for i, values in enumerate(variants):
            component(f"nested/{kind}/{i}", "nested", kind, vi(len(values)) + b"".join(values), count=len(values))
    for i, child in enumerate(children):
        component(f"nested/food/{i}", "nested", 20, food(conversion=child), child_index=i)
    for kind in [29, 30, 52]:
        child = children[1]
        for depth in [2, 4, 8]:
            for _ in range(depth):
                child = slot(1, [(kind, vi(1) + child)])
            add(f"nested/deep/{kind}/{depth}", "nested", child, depth=depth)

    # Network NBT factories normalize signed zero; NaN raw payloads survive the Netty writer.
    float_bits = [0, 0x80000000, 1, 0x80000001, 0x3F800000, 0x7F800000,
                  0x7F800001, 0x7FA00001, 0x7FC00000, 0x7FFFFFFF, 0xFF800001, 0xFFC12345]
    double_bits = [0, 0x8000000000000000, 1, 0x8000000000000001, 0x3FF0000000000000,
                   0x7FF0000000000000, 0x7FF0000000000001, 0x7FF123456789ABCD,
                   0x7FF8000000000000, 0x7FFFFFFFFFFFFFFF, 0xFFF0000000000001, 0xFFF8123456789ABC]
    for kind, width, values in [(5, 4, float_bits), (6, 8, double_bits)]:
        for bits in values:
            raw = compound("x", bytes([kind]) + bits.to_bytes(width, "big"))
            component(f"nbt/float/{kind}/{bits:0{width * 2}x}", "nbt_float", 0, raw,
                      tag_type=kind, bits=f"{bits:0{width * 2}x}")

    strings = [[], [0], [65], [0x7F, 0x80, 0x7FF, 0x800], [0x03BB], [0xD800], [0xDC00],
               [0xD83D, 0xDE00], [0xD800, 65, 0xDC00], [0xFFFF, 0xFFFE], [65, 0, 66], [0x5C, 0x22, 0x0A]]
    for i, units in enumerate(strings):
        descriptor = {"utf16_hex": "".join(f"{u:04x}" for u in units)}
        component(f"nbt/string/{i}", "nbt_string", 0, compound("x", nbt_string(units)), **descriptor)
        for kind in [5, 6]:
            component(f"text/{kind}/literal/{i}", "text", kind, nbt_string(units), **descriptor)
            component(f"text/{kind}/compound/{i}", "text", kind, compound("text", nbt_string(units)), **descriptor)
    invalid_text = [b"\0", b"\12\0", compound("probe", nbt_string([65])),
                    compound("text", b"\3\0\0\0\1"), compound("translate", b"\3\0\0\0\1"),
                    b"\11\0\0\0\0\0", b"\11\10\0\0\0\1" + mutf([65]),
                    b"\3\0\0\0\1", b"\1\1", compound("unknown", b"\1\1")]
    for kind in [5, 6]:
        for i, raw in enumerate(invalid_text):
            component(f"text/{kind}/nonliteral/{i}", "text_nonliteral", kind, raw, nbt_hex=raw.hex())

    # Multiple retained component kinds, repeated kinds and removals exercise true map order.
    entries = [(1, vi(63)), (3, vi(7)), (13, vi(11)), (16, vi(2))]
    for i, order in enumerate(itertools.permutations(entries)):
        additions = list(order)
        if i % 3 == 1:
            additions += [(additions[0][0], vi(9))]
        removed = [additions[1][0], additions[0][0]] if i % 3 == 2 else []
        add(f"mixed_order/{i}", "mixed_order", slot(1, additions, removed),
            added_ids=[c for c, _ in additions], removed_ids=removed)
    return cases


def builtin_registry_ids(runtime, classes, cp):
    # Reflection is restricted to an auxiliary observer; original classes remain untouched.
    helper = runtime / "constructor_registry_ids.java"
    helper.write_text('''package oracle;
import java.util.*;
public final class constructor_registry_ids {
 public static void main(String[] args)throws Exception {
  inventory_normalization_oracle.init();
  Map<String,Object> registries=new LinkedHashMap<>();
  for(String[] spec:new String[][]{{"armor_material","ap"},{"instrument","ak"}}){
   Object registry=Class.forName("lt").getField(spec[1]).get(null);
   Map<String,Integer> ids=new LinkedHashMap<>();
   for(Object value:(Iterable<?>)registry){
    Object name=inventory_normalization_oracle.call(registry,"b",new Class<?>[]{Object.class},value);
    int id=(int)inventory_normalization_oracle.call(registry,"a",new Class<?>[]{Object.class},value);
    ids.put(name.toString(),id);
   }
   registries.put(spec[0],ids);
  }
  inventory_normalization_oracle.OUT.println("REGISTRIES:"+inventory_normalization_oracle.JSON.toJson(registries));
 }
}
''')
    result = subprocess.run([JDK / "javac", "-proc:none", "-cp", cp, "-d", classes, helper], capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(result.stdout + result.stderr)
    result = subprocess.run([JDK / "java", "-cp", cp, "oracle.constructor_registry_ids"], capture_output=True, text=True, check=True)
    (runtime / "registry-observer.log").write_text(result.stdout + result.stderr)
    return json.loads(next(line[11:] for line in result.stdout.splitlines() if line.startswith("REGISTRIES:")))


def main():
    source = ROOT / "tools/inventory_normalization_oracle.java"
    jar = ORACLE / "server-inner-1.21.jar"
    assert sha(jar) == SHA
    assert sha(source) == OLD_ORACLE_SHA, "frozen oracle source changed"
    frozen = [ROOT / p for p in ["tools/inventory_normalization_generate.py", "fixtures/inventory_normalization_vectors.json",
                                  "fixtures/inventory_normalization_policy.json", "evidence/inventory-normalization-oracle.json"]]
    frozen_hashes = {str(p.relative_to(ROOT)): sha(p) for p in frozen}
    runtime = ROOT / "runtime/inventory-normalization-constructor"
    classes = runtime / "classes"
    classes.mkdir(parents=True, exist_ok=True)
    cp = os.pathsep.join(map(str, [classes, jar, *sorted((ORACLE / "libraries").rglob("*.jar"))]))
    compile_result = subprocess.run([JDK / "javac", "-proc:none", "-cp", cp, "-d", classes, source], capture_output=True, text=True)
    if compile_result.returncode:
        raise RuntimeError(compile_result.stdout + compile_result.stderr)
    builtin_ids = builtin_registry_ids(runtime, classes, cp)
    cases = build_cases()
    assert 300 <= len(cases) <= 600, len(cases)
    queries = "".join(json.dumps(c["query"], separators=(",", ":")) + "\n" for c in cases)
    run = subprocess.run([JDK / "java", "-Xmx1G", "-cp", cp, "oracle.inventory_normalization_oracle",
                          ROOT / "tools/crafting_metadata.json"], input=queries, text=True, capture_output=True, timeout=180)
    (runtime / "oracle.log").write_text(run.stdout + run.stderr)
    if run.returncode:
        raise RuntimeError((run.stdout + run.stderr)[-6000:])
    assert "SOURCE:" in run.stdout and jar.name in run.stdout and "ACCESS:originalpd" in run.stdout
    expected = [json.loads(line[7:]) for line in run.stdout.splitlines() if line.startswith("ORACLE:")]
    assert len(expected) == len(cases), (len(expected), len(cases))
    for case, result in zip(cases, expected):
        case["expected"] = result
    assert all(c["expected"]["accepted"] for c in cases if c["group"] == "mixed_order")
    header = {"minecraft": "1.21", "protocol": 767, "oracle_inner_sha256": SHA,
              "codec": "Original cuq.h ItemStack.OPTIONAL_STREAM_CODEC decode->encode; never ItemStack.CODEC",
              "registry_context": "Original pd.a values in unmodified MappedRegistry with exact captured configuration IDs; builtins merged",
              "oracle_source_sha256": sha(source), "registry_metadata_sha256": sha(ROOT / "tools/crafting_metadata.json"),
              "builtin_registry_ids": builtin_ids}
    fixture = ROOT / "fixtures/inventory_normalization_constructor_vectors.json"
    fixture.write_text(json.dumps({**header, "cases": cases}, separators=(",", ":")) + "\n")
    group_stats = {}
    for group in sorted({c["group"] for c in cases}):
        rows = [c for c in cases if c["group"] == group]
        group_stats[group] = {"cases": len(rows), "accepted": sum(c["expected"]["accepted"] for c in rows),
                              "canonical_changed": sum(c["expected"]["accepted"] and c["query"]["bytes"] != c["expected"]["canonical"] for c in rows),
                              "prefix_only": sum(c["expected"]["accepted"] and c["expected"]["remaining"] != 0 for c in rows)}
    bytecode = subprocess.run([JDK / "javap", "-classpath", jar, "-p", "-c", "cpr", "cpr$b", "cwu", "cui", "brz", "brz$b", "drm", "azs", "cwx", "cwy", "cxa"],
                              capture_output=True, text=True, check=True).stdout
    (runtime / "constructor-bytecode.txt").write_text(bytecode)
    contracts = {
        "FoodProperties": {"class": "cpr", "mapping_lines": "59428-59448", "contract": "DIRECT_STREAM_CODEC fields: VAR_INT nutrition, f32 saturation, bool alwaysEat, f32 seconds, optional nested cuq.h conversion, list PossibleEffect. Record constructor stores fields."},
        "PossibleEffect": {"class": "cpr$b", "mapping_lines": "59468-59480", "contract": "effect() getter a() returns new brz(storedEffect); STREAM encoder uses getter, so copy removes hidden chain."},
        "PotionContents": {"class": "cwu", "mapping_lines": "64207-64243", "contract": "customEffects() d() returns Lists.transform(list, MobEffectInstance::new), STREAM encoder traverses defensive copies."},
        "MobEffectInstance": {"class": "brz/brz$b", "mapping_lines": "42130-42223", "contract": "Constructor clamps amplifier to [0,255]. Details STREAM recursively decodes all hidden details. brz copy constructor/setDetailsFrom copies main fields only, not hiddenEffect."},
        "Instrument": {"class": "cui", "mapping_lines": "61741-61757", "contract": "Direct holder payload sound holder avo.d, VAR_INT yv.g useDuration, f32 yv.i range. No description field. Constructor does not clamp scalar fields."},
        "ArmorTrim": {"class": "cwx/cwy/cxa", "mapping_lines": "64296-64400", "contract": "cwx STREAM: material holder, pattern holder, tooltip bool. cwy inline: STRING assetName, ITEM registry holder yv.b(lu.K), F32 itemModelIndex, map<ARMOR_MATERIAL registry holder yv.b(lu.az),STRING suffix>, Component. cxa inline: ResourceLocation, ITEM registry holder, Component, bool decal. Registry-only holder wires are direct varint IDs; direct-or-reference holders use 0 for inline, registry ID+1 otherwise."},
        "Unit19": {"class": "azs/kq.u", "contract": "intangible_projectile uses map-shaped Codec.unit via fallback NBT STREAM, not zero-byte unit STREAM. Any compound content is discarded; other NBT types reject."},
        "PotDecorations": {"class": "drm", "mapping_lines": "77872+", "contract": "STREAM list max4. Constructor fills missing indices0..3 absent and maps cut.qL BRICK to absent. ordered() a() returns exactly four items with absent slots replaced by BRICK (item ID921). Original Item registry stream defaults unknown item ID999999 to AIR; this is observed separately as pot_boundary."},
        "Text": {"components": [5, 6], "contract": "Observed original Component codec accepts string literal or text compound and encodes simple unstyled literals as StringTag. Nonliteral cases are recorded individually, not presumed invalid."},
        "NBT": {"contract": "Original NbtIo factories normalize signed zero; Netty NBT output preserves raw NaN bits and Java modified-UTF encoding of UTF16 units."}}
    report = {"status": "PASS", **header, "cases": len(cases), "groups": group_stats,
              "fixture_sha256": sha(fixture), "generator_sha256": sha(Path(__file__)),
              "constructor_bytecode_sha256": hashlib.sha256(bytecode.encode()).hexdigest(),
              "contracts": contracts, "original_classes_modified": False,
              "native_implementation_verified": False,
              "frozen_inputs": frozen_hashes,
              "limits": ["Finite targeted corpus, not complete component semantic validation.",
                         "No original method or constructor is replaced; only valid/invalid wire input is injected.",
                         "Prefix acceptance is explicitly recorded; callers must check consumed/remaining.",
                         "Text advanced style/translation/event validation and pathological nesting depth are not exhaustive.",
                         "Multi-entry trim override maps use original Java map iteration; generic Java map ordering is not proved by fixed vectors.",
                         "General CompoundTag key ordering is outside one-key NBT tests."]}
    evidence = ROOT / "evidence/inventory-normalization-constructor-oracle.json"
    evidence.write_text(json.dumps(report, indent=2) + "\n")
    assert frozen_hashes == {str(p.relative_to(ROOT)): sha(p) for p in frozen}, "frozen files changed"
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
