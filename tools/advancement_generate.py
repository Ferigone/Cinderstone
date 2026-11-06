#!/usr/bin/env python3
"""Generate fixtures/advancement-progress-vectors.json.

The original `PlayerAdvancements` progress file codec (tools/loot_oracle.java
`advancement-progress`: `DataFixTypes.ADVANCEMENTS.wrapCodec(PlayerAdvancements.Data.CODEC, ..,
1343)` over a strictly parsed document, then `AdvancementProgress.update(requirements)` and the
save step) is run over generated progress files and the answers recorded: accepted or rejected, the
re-encoded document and every entry's `done`. The Rust port must agree on every row.

    python3 tools/advancement_generate.py --oracle-dir /path/outside/git [--sample 6] [--full-out FILE]

`--sample N` keeps one row in N of the generated set (the committed fixture); `--full-out`
writes the complete set, which the Rust test can run with `ADVANCEMENT_PARITY_FILE=<file>`.
"""
import argparse
import copy
import itertools
import json
import random
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
VERSION = 3953
STAMP = "2024-03-04 05:06:07 +0100"

BASE_ENTRIES = {
    "minecraft:story/root": {"criteria": {"crafting_table": STAMP}, "done": True},
    "minecraft:adventure/adventuring_time": {
        "criteria": {"minecraft:plains": "2023-12-31 23:59:59 +0000", "minecraft:desert": "2024-02-29 00:00:00 -0800"},
        "done": False,
    },
    "custom:pack/thing": {"criteria": {"a": "2001-09-09 01:46:40 +0000"}, "done": False},
    "minecraft:nether/all": {"done": False},
    "minecraft:husbandry/empty": {"criteria": {}, "done": False},
}
BASE_REQUIREMENTS = {
    "minecraft:story/root": [["crafting_table"]],
    "minecraft:adventure/adventuring_time": [["minecraft:plains"], ["minecraft:desert"], ["minecraft:forest"]],
    "custom:pack/thing": [["a", "b"]],
    "minecraft:nether/all": [["x"], ["y"]],
    "minecraft:husbandry/empty": [["z"]],
}
WRONG = ["x", "", 1, 0, -1, 2.5, True, False, None, [], {}, [1], ["a"], {"a": 1}, {"criteria": {}}, STAMP]
DATA_VERSIONS = [VERSION, VERSION + 1, 4325, 4000, 9999999, 2147483647, 2147483648, 4294967296 + VERSION, -1, 0, 1, 1343, 3952,
                 3953.0, 3953.5, 3952.9, 3954.2, 1e3, 3.953e3, 3953e0, "3953", "x", "", True, False, None, [], {}, [3953], {"a": 3953}]


def document(entries=None, version=VERSION):
    result = copy.deepcopy(BASE_ENTRIES if entries is None else entries)
    if version is not None:
        result["DataVersion"] = version
    return result


def case(text, requirements=None, note=""):
    return {"text": text if isinstance(text, str) else json.dumps(text, separators=(",", ":")),
            "requirements": copy.deepcopy(BASE_REQUIREMENTS if requirements is None else requirements), "note": note}


def walk(node, path=()):
    yield path, node
    if isinstance(node, dict):
        for key, value in node.items():
            yield from walk(value, path + (key,))
    elif isinstance(node, list):
        for index, value in enumerate(node):
            yield from walk(value, path + (index,))


def edit(root, path, value, remove=False):
    root = copy.deepcopy(root)
    parent = root
    for step in path[:-1]:
        parent = parent[step]
    if remove:
        del parent[path[-1]]
    else:
        parent[path[-1]] = value
    return root


def structure_cases():
    base = document()
    out = []
    for path, node in walk(base):
        if not path:
            continue
        out.append(case(edit(base, path, None, remove=True), note="member removed"))
        for wrong in WRONG:
            if wrong != node:
                out.append(case(edit(base, path, wrong), note="member replaced"))
    return out


LEGACY = {
    "minecraft:adventure/adventuring_time": {"criteria": {"minecraft:deep_warm_ocean": STAMP, "minecraft:nether": STAMP, "minecraft:plains": STAMP}},
    "minecraft:adventure/kill_a_mob": {"criteria": {"minecraft:zombie_pigman": STAMP, "minecraft:zombie": STAMP}},
    "minecraft:husbandry/complete_catalogue": {"criteria": {"textures/entity/cat/red.png": STAMP, "minecraft:british": STAMP, "minecraft:tabby": STAMP}},
    "minecraft:recipes/misc/sugar": {"criteria": {"has_sugar_cane": STAMP}},
    "minecraft:recipes/misc/composter": {"criteria": {"has_composter": STAMP}},
    "minecraft:recipes/building_blocks/stonebrick": {"criteria": {"has_stone": STAMP}},
    "minecraft:husbandry/play_jukebox_in_meadows": {"criteria": {"play_jukebox_in_meadows": STAMP}},
    "minecraft:adventure/caves_and_cliff": {"criteria": {"caves_and_cliff": STAMP}},
    "minecraft:adventure/ride_strider_in_overworld_lava": {"criteria": {"ride_strider_in_overworld_lava": STAMP}},
}
LEGACY_REQUIREMENTS = {name: [[criterion] for criterion in value["criteria"]] for name, value in LEGACY.items()}
LEGACY_REQUIREMENTS.update({
    "minecraft:adventure/fall_from_world_height": [["caves_and_cliff"]], "minecraft:nether/ride_strider_in_overworld_lava": [["ride_strider_in_overworld_lava"]],
    "minecraft:adventure/play_jukebox_in_meadows": [["play_jukebox_in_meadows"]], "minecraft:recipes/misc/sugar_from_sugar_cane": [["has_sugar_cane"]],
    "minecraft:recipes/decorations/composter": [["has_composter"]], "minecraft:recipes/building_blocks/stone_bricks": [["has_stone"]],
})


def legacy_cases():
    """Old files: the original fixes them; the port reports that it cannot (older than 3097)."""
    out = []
    for version in [None, 1, 1343, 1500, 1501, 2099, 2100, 2502, 2503, 2845, 2846, 3085, 3086, 3096, 3097, 3098, 3200, 3465, 3700, 3952, 3953]:
        out.append(case(document(copy.deepcopy(LEGACY), version), LEGACY_REQUIREMENTS, "legacy content"))
    out.append(case(document({"x:y": {"criteria": {"a": STAMP}}}, 3096), {"x:y": [["a"]]}, "legacy content"))
    out.append(case(document({"x:y": {"criteria": {"a": "bad"}}}, 3096), {"x:y": [["a"]]}, "legacy content"))
    return out


def version_cases():
    out = []
    for version in DATA_VERSIONS:
        out.append(case(document(version=version), note="DataVersion"))
    out.append(case(document(version=None), note="no DataVersion"))
    # Raw text spellings of the version number (Gson keeps the digits, `intValue` parses them).
    for spelling in ["3953", "3953.0", "3953.9", "3.953E3", "3953e0", "39530e-1", "4294970249", "-4294963343", "1e3", "0.5", "-0", "9223372036854775807", "9223372036854775808", "1e18"]:
        out.append(case('{"minecraft:story/root":{"criteria":{"crafting_table":"%s"}},"DataVersion":%s}' % (STAMP, spelling), note="version spelling"))
    out.append(case(json.dumps(document()).replace('"DataVersion": 3953', '"DataVersion": 3953, "DataVersion": 1'), note="duplicate DataVersion"))
    out.append(case(json.dumps(document()).replace('"DataVersion": 3953', '"DataVersion": 1, "DataVersion": 3953'), note="duplicate DataVersion"))
    return out


KEYS = ["minecraft:story/root", "story/root", "story/root:", ":story/root", "Minecraft:story/root", "minecraft:Story/root", "minecraft:story root",
        "a:b:c", ":", "", "minecraft:", "mine craft:x", "x", "minecraft:x", "_:_", "a-b.c:d/e-f.g_h", "a/b:c", "a:b\\u00e9", "é:x", "a:b\\", "ns:" + "x/" * 40,
        "DataVersion", "dataversion", "minecraft:dataversion", "a:b//c", "a:/b", "a:b/"]


def key_cases():
    out = []
    for key in KEYS:
        text = '{"%s":{"criteria":{"c":"%s"}},"DataVersion":%d}' % (key, STAMP, VERSION)
        out.append(case(text, {"minecraft:story/root": [["c"]], "story/root:": [["c"]], "minecraft:x": [["c"]], "a:b": [["c"]], "minecraft:": [["c"]], "minecraft:story/root:": [["c"]]}, "id spelling"))
    # Two spellings of one id collapse onto the later member (and the first one's content is dropped).
    for first, second in itertools.permutations(["x", "minecraft:x", ":x"], 2):
        text = '{"%s":{"criteria":{"c":"%s"}},"%s":{"criteria":{"d":"%s"}},"DataVersion":%d}' % (first, STAMP, second, STAMP, VERSION)
        out.append(case(text, {"minecraft:x": [["c"], ["d"]]}, "duplicate ids"))
    # A bad entry anywhere rejects the whole file, including one for an advancement nobody asked for.
    out.append(case('{"minecraft:story/root":{"criteria":{"crafting_table":"%s"}},"unknown:id":{"criteria":{"c":"nope"}},"DataVersion":%d}' % (STAMP, VERSION), note="bad unknown entry"))
    out.append(case('{"unknown:id":{"criteria":{"c":"%s"}},"minecraft:story/root":{"criteria":{"crafting_table":"%s"}},"DataVersion":%d}' % (STAMP, STAMP, VERSION), note="unrequested entry"))
    return out


def stamp_cases():
    """Date strings: a hand list, then every field of a canonical stamp set to edge values."""
    stamps = [
        STAMP, "2024-03-04 05:06:07 +0000", "2024-03-04 05:06:07 -0000", "2024-03-04 05:06:07 +0100", "2024-03-04 05:06:07 -0100",
        "2024-03-04 05:06:07 +1800", "2024-03-04 05:06:07 -1800", "2024-03-04 05:06:07 +1801", "2024-03-04 05:06:07 -1801", "2024-03-04 05:06:07 +2359",
        "2024-03-04 05:06:07 +2400", "2024-03-04 05:06:07 +0060", "2024-03-04 05:06:07 +0059", "2024-03-04 05:06:07 +0530", "2024-03-04 05:06:07 +0545",
        "2024-03-04 05:06:07 +00", "2024-03-04 05:06:07 +000", "2024-03-04 05:06:07 +00000", "2024-03-04 05:06:07 +01:00", "2024-03-04 05:06:07 Z",
        "2024-03-04 05:06:07 UTC", "2024-03-04 05:06:07 GMT", "2024-03-04 05:06:07 0000", "2024-03-04 05:06:07", "2024-03-04 05:06:07 ", "2024-03-04 05:06:07  +0000",
        " 2024-03-04 05:06:07 +0000", "2024-03-04 05:06:07 +0000 ", "2024-03-04T05:06:07 +0000", "2024-03-04 05:06:07+0000", "2024-3-4 5:6:7 +0000",
        "2024-03-04 05:06 +0000", "2024-03-04 05:06:07.5 +0000", "2024-03-04 05:06:07.000 +0000", "24-03-04 05:06:07 +0000", "2024/03/04 05:06:07 +0000",
        "2024-03-04 5:06:07 +0000", "2024-03-04 05:6:07 +0000", "2024-03-04 05:06:7 +0000", "2024-03-04  05:06:07 +0000", "2024-03-04 05:06:07 +0000Z",
        "2024-03-04 05:06:07 +0000 UTC", "2024-03-04 05:06:07 +0000\n", "\t2024-03-04 05:06:07 +0000", "2024-03-04\u00a005:06:07 +0000",
        "\u0662\u0660\u0662\u0664-03-04 05:06:07 +0000", "2024-0\u0663-04 05:06:07 +0000", "2024-03-04 05:06:07 +0000".upper(), "1970-01-01 00:00:00 +0000",
        "1969-12-31 23:59:59 +0000", "0001-01-01 00:00:00 +0000", "0001-01-01 00:00:00 +0100", "0001-01-01 00:00:00 -0100", "0001-01-01 00:00:00 +1800",
        "0000-12-31 23:59:59 +0000", "0000-01-01 00:00:00 +0000", "9999-12-31 23:59:59 +0000", "9999-12-31 23:59:59 -0100", "9999-12-31 24:00:00 +0000",
        "+10000-01-01 00:00:00 +0000", "+9999-01-01 00:00:00 +0000", "+09999-01-01 00:00:00 +0000", "+0000-01-01 00:00:00 +0000", "+00001-01-01 00:00:00 +0000",
        "+12345-06-07 08:09:10 +0000", "+99999-12-31 23:59:59 +0000", "+999999999-12-31 23:59:59 +0000", "+999999999-12-31 24:00:00 +0000",
        "+999999999-12-31 23:59:59 -1800", "+1000000000-01-01 00:00:00 +0000", "+1000000000-01-01 00:00:00 +0100", "+9999999999999999999-01-01 00:00:00 +0000",
        "+99999999999999999999-01-01 00:00:00 +0000", "+000000000000000001-01-01 00:00:00 +0000", "+0000000000000000001-01-01 00:00:00 +0000",
        "10000-01-01 00:00:00 +0000", "12345-06-07 08:09:10 +0000", "-0001-01-01 00:00:00 +0000", "-0000-01-01 00:00:00 +0000", "-12345-01-01 00:00:00 +0000",
        "-2024-03-04 05:06:07 +0000", "202-03-04 05:06:07 +0000", "20240-03-04 05:06:07 +0000", "2024-03-04 05:06:07 +0000".replace("2024", "02024"),
        "2023-02-29 12:00:00 +0000", "2024-02-29 12:00:00 +0000", "2024-02-30 12:00:00 +0000", "2024-02-31 12:00:00 +0000", "2024-04-31 12:00:00 +0000",
        "2024-06-31 12:00:00 +0000", "2024-09-31 12:00:00 +0000", "2024-11-31 12:00:00 +0000", "2024-12-31 12:00:00 +0000", "2024-01-32 12:00:00 +0000",
        "2024-00-10 12:00:00 +0000", "2024-13-10 12:00:00 +0000", "2024-01-00 12:00:00 +0000", "1900-02-29 12:00:00 +0000", "2000-02-29 12:00:00 +0000",
        "2100-02-29 12:00:00 +0000", "2023-02-31 24:00:00 +0000", "2024-02-30 24:00:00 -0800", "2024-12-31 24:00:00 +0000", "2024-12-31 24:00:00 -1800",
        "2024-01-01 24:00:00 +1800", "2024-01-01 24:00:01 +0000", "2024-01-01 24:01:00 +0000", "2024-01-01 24:00:60 +0000", "2024-01-01 25:00:00 +0000",
        "2024-01-01 99:00:00 +0000", "2024-01-01 23:60:00 +0000", "2024-01-01 23:59:60 +0000", "2024-01-01 23:59:61 +0000", "2024-01-01 00:00:00 +0000",
        "2024-01-01 12:00:00 +0000", "2024-01-01 12:00:00 -0000", "2024-01-01 12:00:00 +0001", "2024-01-01 12:00:00 -0001", "2024-01-01 12:00:00 +1759",
        "2024-01-01 12:00:00 +1800", "2024-01-01 12:00:00 +1801", "2024-01-01 12:00:00 +1860", "2024-01-01 12:00:00 +2300", "2024-01-01 12:00:00 +99999",
        "2024-01-01 12:00:00 +ABCD", "2024-01-01 12:00:00 +００００", "2024-01-01 12:00:00 --0100", "2024-01-01 12:00:00 +-100", "２０２４-01-01 12:00:00 +0000",
        "2024-01-01 12:00:00 \u2212", "2024-01-01 12:00:00 \u22120100", "2024-01-01 12:00:00 +0100", "2024-01-01 12:00:00 +0130", "2024-01-01 12:00:00 -0130",
        "2024-01-01 12:00:00 -0000", "2024-01-01 12:00:00 -0000", "0001-01-01 00:00:00 -0000", "2024-1-01 12:00:00 +0000", "2024-01-1 12:00:00 +0000",
        "", " ", "x", "null", "0", "2024", "true",
    ]
    fields = {
        "year": (0, 4, ["0000", "0001", "0999", "1000", "1969", "1970", "2038", "9999", "+10000", "+99999", "12345", "-001", "-0001", "+0001", "202", "20", "2", "+2024", "02024", ""]),
        "month": (5, 7, ["00", "01", "02", "12", "13", "99", "1", "001", "ab", "-1", "+1", " 1", "1 ", "٣", ""]),
        "day": (8, 10, ["00", "01", "28", "29", "30", "31", "32", "99", "1", "001", "-1", "+1", " 1", ""]),
        "hour": (11, 13, ["00", "12", "23", "24", "25", "99", "1", "-1", "+1", " 1", ""]),
        "minute": (14, 16, ["00", "30", "59", "60", "99", "1", "-1", "+1", ""]),
        "second": (17, 19, ["00", "30", "59", "60", "61", "99", "1", "-1", "+1", ""]),
        "offset": (20, 25, ["+0000", "-0000", "+0100", "-0100", "+1800", "-1800", "+1801", "-1801", "+0059", "+0060", "+2359", "+0", "", "0100", "+01", "+010", "+01000", "+0100 ", "Z", "+00:00", " 0100", "++100", "+１００"]),
    }
    for name, (start, end, values) in fields.items():
        for value in values:
            stamps.append(STAMP[:start] + value + STAMP[end:])
    # Calendar edges crossed with end-of-day and offsets near the limits.
    for year, month, day in itertools.product(["1900", "2000", "2023", "2024", "2100"], ["02", "04", "12"], ["28", "29", "30", "31"]):
        for hour, offset in [("24:00:00", "+0000"), ("23:59:59", "-1800"), ("00:00:00", "+1800"), ("24:00:00", "-1800")]:
            stamps.append(f"{year}-{month}-{day} {hour} {offset}")
    seen = []
    for stamp in stamps:
        if stamp not in seen:
            seen.append(stamp)
    out = []
    for stamp in seen:
        out.append(case({"minecraft:story/root": {"criteria": {"crafting_table": stamp}}, "DataVersion": VERSION}, {"minecraft:story/root": [["crafting_table"]]}, "stamp"))
    return out


def text_cases():
    base = json.dumps(document(), separators=(",", ":"))
    pretty = json.dumps(document(), indent=2)
    root = '{"minecraft:story/root":{"criteria":{"crafting_table":"2024-03-04 05:06:07 +0100"}},"DataVersion":3953}'
    corruptions = {
        "bom": "\ufeff" + base, "double bom": "\ufeff\ufeff" + base, "trailing comma": base + ",", "trailing garbage": base + "garbage", "trailing value": base + " {}",
        "trailing newline": base + "\n", "leading space": " \n\t " + base, "pretty": pretty, "single quotes": base.replace('"', "'"), "comment": "// c\n" + base,
        "block comment": "/* c */" + base, "hash comment": "# c\n" + base, "unquoted key": root.replace('"DataVersion"', "DataVersion"), "semicolon": base.replace(",", ";", 1),
        "truncated half": base[: len(base) // 2], "truncated one": base[:-1], "array root": "[" + base + "]", "null root": "null", "number root": "1", "string root": '"x"',
        "empty": "", "whitespace": "  ", "empty object": "{}", "version only": '{"DataVersion":3953}', "nan": root.replace("3953", "NaN"),
        "infinity": root.replace("3953", "Infinity"), "hex": root.replace("3953", "0x1"), "plus number": root.replace("3953", "+3953"), "leading zero": root.replace("3953", "03953"),
        "dot number": root.replace("3953", ".5"), "trailing dot": root.replace("3953", "3953."), "exponent": root.replace("3953", "3953E+0"), "bad exponent": root.replace("3953", "3953E"),
        "raw tab in key": root.replace("crafting_table", "crafting\ttable"), "raw newline in name": root.replace("crafting_table", "crafting\ntable"),
        "raw nul": root.replace("crafting_table", "crafting\u0000table"), "raw 0x1f": root.replace("crafting_table", "crafting\u001ftable"), "raw del": root.replace("crafting_table", "crafting\u007ftable"),
        "raw tab in value": root.replace("05:06:07", "05:06:07\t"), "raw line separator": root.replace("crafting_table", "crafting\u2028table"),
        "escaped tab": root.replace("crafting_table", "crafting\\ttable"), "escaped unicode": root.replace("crafting_table", "crafting\\u005ftable"),
        "escaped slash": root.replace("crafting_table", "crafting\\/table"), "bad escape": root.replace("crafting_table", "crafting\\xtable"), "bad unicode escape": root.replace("crafting_table", "crafting\\u00zztable"),
        "short unicode escape": root.replace("crafting_table", "crafting\\u00"), "lone high surrogate": root.replace("crafting_table", "crafting\\ud800table"),
        "lone low surrogate": root.replace("crafting_table", "crafting\\udc00table"), "surrogate pair": root.replace("crafting_table", "crafting\\ud83d\\ude00table"),
        "emoji": root.replace("crafting_table", "crafting\U0001f600table"), "escaped key": root.replace('"minecraft:story/root"', '"minecraft\\u003astory\\/root"'),
        "escaped id invalid": root.replace('"minecraft:story/root"', '"minecraft\\u003aStory"'), "duplicate member": root.replace('"DataVersion":3953', '"minecraft:story/root":{"criteria":{}},"DataVersion":3953'),
        "duplicate criterion": root.replace('"crafting_table":"2024-03-04 05:06:07 +0100"', '"crafting_table":"2024-03-04 05:06:07 +0100","crafting_table":"2020-01-01 00:00:00 +0000"'),
        "duplicate criterion bad first": root.replace('"crafting_table":"2024-03-04 05:06:07 +0100"', '"crafting_table":"bad","crafting_table":"2020-01-01 00:00:00 +0000"'),
        "duplicate criterion bad last": root.replace('"crafting_table":"2024-03-04 05:06:07 +0100"', '"crafting_table":"2020-01-01 00:00:00 +0000","crafting_table":"bad"'),
        "extra members": root.replace('"criteria"', '"extra":[1,{"a":null}],"criteria"'), "done only": '{"minecraft:story/root":{"done":true},"DataVersion":3953}',
        "criteria null": '{"minecraft:story/root":{"criteria":null,"done":false},"DataVersion":3953}', "entry null": '{"minecraft:story/root":null,"DataVersion":3953}',
        "criteria value null": '{"minecraft:story/root":{"criteria":{"crafting_table":null}},"DataVersion":3953}', "empty key": '{"":{"criteria":{}},"DataVersion":3953}',
        "empty criterion name": '{"minecraft:story/root":{"criteria":{"":"2024-03-04 05:06:07 +0100"}},"DataVersion":3953}',
        "deep extra": '{"minecraft:story/root":{"criteria":{"crafting_table":"2024-03-04 05:06:07 +0100"},"x":' + "[" * 100 + "]" * 100 + '},"DataVersion":3953}',
        "deeper extra": '{"minecraft:story/root":{"criteria":{"crafting_table":"2024-03-04 05:06:07 +0100"},"x":' + "[" * 126 + "]" * 126 + '},"DataVersion":3953}',
        "large number": root.replace('"done"', '"done"'), "big done": '{"minecraft:story/root":{"done":1e400},"DataVersion":3953}',
        "criteria string": '{"minecraft:story/root":{"criteria":"x"},"DataVersion":3953}', "criteria array": '{"minecraft:story/root":{"criteria":[]},"DataVersion":3953}',
        "entry array": '{"minecraft:story/root":[],"DataVersion":3953}', "entry string": '{"minecraft:story/root":"x","DataVersion":3953}',
    }
    out = [case(text, {"minecraft:story/root": [["crafting_table"]], "x:y": [["a"]]}, "text: " + name) for name, text in corruptions.items()]
    return out


def requirement_cases(rng):
    out = []
    base_doc = document()
    names = {entry: sorted(value.get("criteria", {})) for entry, value in BASE_ENTRIES.items()}
    shapes = []
    for entry, criteria in names.items():
        extra = ["unknown", "other"]
        everything = criteria + extra
        shapes.append([[name] for name in criteria])
        shapes.append([criteria] if criteria else [])
        shapes.append([[name] for name in everything])
        shapes.append([everything])
        shapes.append([[]])
        shapes.append([])
        shapes.append([[], criteria] if criteria else [[], ["unknown"]])
        shapes.append([criteria + criteria])
        shapes.append([[name] for name in criteria] + [["unknown"]])
        shapes.append([["unknown"]])
        shapes.append([[""]])
        shapes.append([[name, name] for name in criteria])
        shapes.append([criteria[:1], criteria[1:]] if len(criteria) > 1 else [criteria])
        shapes.append([[name for name in criteria[::2]], [name for name in criteria[1::2]]])
    for _ in range(400):
        requirements = {}
        for entry in BASE_ENTRIES:
            if rng.random() < 0.15:
                continue
            requirements[entry] = rng.choice(shapes)
        out.append(case(base_doc, requirements, "requirements"))
    for requirements in shapes:
        out.append(case(base_doc, {entry: requirements for entry in BASE_ENTRIES}, "requirements"))
    out.append(case(base_doc, {}, "no requirements"))
    # Requirements with many criteria: every combination of done and not done criteria of one entry.
    criteria = [f"c{i}" for i in range(6)]
    for mask in range(1 << len(criteria)):
        entries = {"x:y": {"criteria": {name: STAMP for index, name in enumerate(criteria) if mask >> index & 1}}}
        for requirements in ([[name] for name in criteria], [criteria], [criteria[:3], criteria[3:]], [criteria[:2], criteria[2:4], criteria[4:]], [criteria[::2], criteria[1::2]], [criteria[:1], criteria[1:]]):
            out.append(case(document(entries), {"x:y": requirements}, "done masks"))
    return out


SMALL_REQUIREMENTS = {"minecraft:story/root": [["crafting_table"]], "x:y": [["a"]]}


def stamp_fuzz_cases(rng):
    """One-character edits of dates, plus dates assembled from random field widths."""
    bases = [STAMP, "+12345-06-07 08:09:10 -1800", "2024-02-29 24:00:00 +0000", "0001-01-01 00:00:00 +1800", "9999-12-31 23:59:59 +0000", "2023-04-31 12:00:00 -0000"]
    alphabet = "0159+-: Zz.T\u00a0\u0663\uff10e"
    texts = set()
    for base in bases:
        for index in range(len(base)):
            texts.add(base[:index] + base[index + 1:])
            for char in alphabet:
                texts.add(base[:index] + char + base[index + 1:])
                texts.add(base[:index] + char + base[index:])
        for char in alphabet:
            texts.add(base + char)
            texts.add(char + base)
    for _ in range(2500):
        year = rng.choice(["", "+", "-"]) + "".join(rng.choice("0123456789") for _ in range(rng.choice([1, 2, 3, 4, 4, 4, 5, 6, 9, 10])))
        parts = [year] + ["".join(rng.choice("0123456789") for _ in range(rng.choice([1, 2, 2, 2, 3]))) for _ in range(5)]
        offset = rng.choice(["+", "-", ""]) + "".join(rng.choice("0123456789") for _ in range(rng.choice([2, 4, 4, 4, 5])))
        texts.add("%s-%s-%s %s:%s:%s %s" % (*parts, offset))
    return [case({"minecraft:story/root": {"criteria": {"crafting_table": text}}, "DataVersion": VERSION}, SMALL_REQUIREMENTS, "stamp fuzz") for text in sorted(texts)]


def text_fuzz_cases(rng):
    """One-character edits of the JSON text, which exercise the strict reader."""
    base = '{"minecraft:story/root":{"criteria":{"crafting_table":"2024-03-04 05:06:07 +0100"},"done":true},"DataVersion":3953}'
    alphabet = ['{', '}', '[', ']', '"', "'", ',', ':', ';', '=', '\\', ' ', '\t', '\n', '\r', '\f', '\u000b', '/', '*', '#', '0', '1', '-', '+', '.', 'e', 'E', 'n', 't', 'x', 'u', '\u00e9', '\u2028', '\ufeff', '\u0000', '\u007f']
    texts = set()
    for index in range(len(base)):
        texts.add(base[:index] + base[index + 1:])
        for char in alphabet:
            texts.add(base[:index] + char + base[index + 1:])
            texts.add(base[:index] + char + base[index:])
    for char in alphabet:
        texts.add(base + char)
        texts.add(char + base)
    fragments = ['\\u00e9', '\\ud83d\\ude00', '\\ud800', '\\udc00x', '\\n', '\\"', '\\/', '\\\\', '\\u0041', '\\u004', '\\x', '\\', 'é', '\U0001f600', '\n', '\t', '\u0000']
    for _ in range(1500):
        text = base
        for _ in range(rng.choice([1, 2, 3])):
            index = rng.randrange(len(text) + 1)
            op = rng.random()
            if op < 0.4:
                text = text[:index] + rng.choice(fragments) + text[index:]
            elif op < 0.7:
                text = text[:index] + rng.choice(alphabet) + text[index + 1:]
            else:
                text = text[:index] + text[index + rng.choice([1, 2, 3]):]
        texts.add(text)
    return [case(text, SMALL_REQUIREMENTS, "text fuzz") for text in sorted(texts)]


def all_cases():
    rng = random.Random(7)
    cases = structure_cases() + legacy_cases() + version_cases() + key_cases() + stamp_cases() + text_cases() + requirement_cases(rng)
    cases += stamp_fuzz_cases(rng) + text_fuzz_cases(rng)
    # Randomly combined edits: two structural edits of the base document at once.
    base = document()
    paths = [(path, node) for path, node in walk(base) if path]
    for _ in range(400):
        mutated = copy.deepcopy(base)
        for _ in range(2):
            path, _node = rng.choice(paths)
            try:
                mutated = edit(mutated, path, rng.choice(WRONG), remove=rng.random() < 0.3)
            except (KeyError, TypeError, IndexError):
                pass
        cases.append(case(mutated, note="two edits"))
    return cases


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--oracle-dir", type=Path, required=True)
    parser.add_argument("--work", type=Path, default=Path("/tmp/advancement-work"))
    parser.add_argument("--sample", type=int, default=4)
    parser.add_argument("--full-out", type=Path)
    args = parser.parse_args()
    args.work.mkdir(parents=True, exist_ok=True)
    cases = all_cases()
    (args.work / "advancement-in.json").write_text(json.dumps(cases))
    classpath = (args.oracle_dir / "cp.txt").read_text().strip()
    subprocess.run(
        ["java", "--add-exports", "java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED", "-cp", f"{args.oracle_dir}:{classpath}", "loot_oracle",
         "advancement-progress", str(args.work / "advancement-in.json"), str(args.work / "advancement-out.json")],
        check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    results = json.loads((args.work / "advancement-out.json").read_text())
    rows = []
    for spec, result in zip(cases, results):
        row = {"text": spec["text"], "requirements": spec["requirements"], "note": spec["note"], "ok": result["ok"]}
        if result["ok"]:
            row["json"] = result["json"]
            row["done"] = result["done"]
        if "version" in result:
            row["version"] = result["version"]
        rows.append(row)
    accepted = sum(1 for row in rows if row["ok"])
    meta = {
        "oracle": "tools/loot_oracle.java advancement-progress (DataFixTypes.ADVANCEMENTS.wrapCodec over PlayerAdvancements.Data.CODEC, then AdvancementProgress.update)",
        "generator": "tools/advancement_generate.py",
        "total": len(rows),
        "accepted": accepted,
    }
    if args.full_out:
        args.full_out.write_text(json.dumps({"meta": meta, "rows": rows}))
    kept = rows[:: max(1, args.sample)]
    # Rows that exercise a distinct note stay whatever the stride.
    firsts = {}
    for row in rows:
        firsts.setdefault((row["note"], row["ok"]), row)
    for row in firsts.values():
        if row not in kept:
            kept.append(row)
    meta["sampled_from"] = len(rows)
    meta["total"] = len(kept)
    meta["accepted"] = sum(1 for row in kept if row["ok"])
    (ROOT / "fixtures" / "advancement-progress-vectors.json").write_text(json.dumps({"meta": meta, "rows": kept}, indent=None, separators=(",", ":")) + "\n")
    print(f"{len(rows)} generated, {accepted} accepted; kept {len(kept)}")


if __name__ == "__main__":
    main()
