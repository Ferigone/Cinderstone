#!/usr/bin/env python3
"""One compile-only verified ABI header; never a runtime Minecraft class.

The original archive is read-only. Only its class-level generic Signature is
omitted; superclass/interfaces and public/protected JVM descriptors are kept.
Every concrete body is replaced by an unconditional throw, not engine behavior.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import zipfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--server-jar", type=Path, required=True)
parser.add_argument("--out-dir", type=Path, required=True,
                    help="External compile-only directory, never on a runtime classpath")
args = parser.parse_args()
JAR = args.server_jar.resolve()
OUT = args.out_dir.resolve()
for parent in [OUT, *OUT.parents]:
    if (parent / "AGENTS.md").is_file() and (parent / "Cargo.toml").is_file():
        raise RuntimeError("compile-only class output must stay outside the checkout")
EXPECTED = "e8c211b41317a9f5a780c98a89592ecb72eb39a6e475d4ac9657e5bc9ffaf55f"


def require(value, message):
    if not value:
        raise RuntimeError(message)


require(hashlib.sha256(JAR.read_bytes()).hexdigest() == EXPECTED, "wrong official server artifact")
with zipfile.ZipFile(JAR) as archive:
    original = archive.read("net/minecraft/server/MinecraftServer.class")


class Reader:
    def __init__(self, data):
        self.data, self.pos = data, 0

    def take(self, size):
        out = self.data[self.pos:self.pos + size]
        require(len(out) == size, "truncated original class")
        self.pos += size
        return out

    def u2(self):
        return struct.unpack(">H", self.take(2))[0]

    def u4(self):
        return struct.unpack(">I", self.take(4))[0]


u2 = lambda n: struct.pack(">H", n)
u4 = lambda n: struct.pack(">I", n)
r = Reader(original)
prefix = r.take(8)
require(prefix[:4] == b"\xca\xfe\xba\xbe", "invalid original class magic")
count = r.u2()
pool = u2(count)
strings = {}
index = 1
while index < count:
    start = r.pos
    tag = r.take(1)[0]
    if tag == 1:
        raw = r.take(r.u2())
        strings[index] = raw.decode("utf-8", errors="surrogatepass")
    else:
        r.take({3: 4, 4: 4, 5: 8, 6: 8, 7: 2, 8: 2, 9: 4, 10: 4,
                11: 4, 12: 4, 15: 3, 16: 2, 17: 4, 18: 4, 19: 2, 20: 2}[tag])
    pool += original[start:r.pos]
    index += 2 if tag in (5, 6) else 1


def attrs():
    out = []
    for _ in range(r.u2()):
        name = r.u2()
        out.append((name, r.take(r.u4())))
    return out


def encode_attrs(values):
    return u2(len(values)) + b"".join(u2(n) + u4(len(b)) + b for n, b in values)


head = r.take(6)  # verified access, actual class, actual raw superclass
interfaces = r.take(r.u2() * 2)
head += u2(len(interfaces) // 2) + interfaces
fields, methods, audit = [], [], []
for kind, destination in [("field", fields), ("method", methods)]:
    for _ in range(r.u2()):
        access, name, descriptor = r.u2(), r.u2(), r.u2()
        values = attrs()
        if not access & 0x0005:  # public/protected declarations only
            continue
        if strings[name] == "<clinit>":
            continue
        rewritten = []
        for attr, body in values:
            if kind == "method" and strings[attr] == "Code":
                # Keep the original locals count; aconst_null;athrow cannot
                # provide fake world behavior even if this header were loaded.
                locals_count = struct.unpack(">H", body[2:4])[0]
                body = u2(1) + u2(locals_count) + u4(2) + b"\x01\xbf" + u2(0) + u2(0)
            rewritten.append((attr, body))
        destination.append(u2(access) + u2(name) + u2(descriptor) + encode_attrs(rewritten))
        audit.append(dict(kind=kind, name=strings[name], descriptor=strings[descriptor], access=access))
class_attrs = attrs()
require(r.pos == len(original), "unparsed original class bytes")
removed = [body for name, body in class_attrs if strings[name] == "Signature"]
require(len(removed) == 1, "unexpected original generic signature count")
signature = strings[struct.unpack(">H", removed[0])[0]]
require("Laqz<Lxm;>;" in signature, "unexpected original generic inheritance")
class_attrs = [(name, body) for name, body in class_attrs if strings[name] != "Signature"]
header = (prefix + pool + head + u2(len(fields)) + b"".join(fields)
          + u2(len(methods)) + b"".join(methods) + encode_attrs(class_attrs))
path = OUT / "net/minecraft/server/MinecraftServer.class"
path.parent.mkdir(parents=True, exist_ok=True)
path.write_bytes(header)
report = dict(original_jar_sha256=EXPECTED, original_class_sha256=hashlib.sha256(original).hexdigest(),
              header_sha256=hashlib.sha256(header).hexdigest(), original_class_signature=signature,
              only_generic_omission="class-level Signature; raw superclass/interfaces unchanged",
              bodies="all retained concrete methods throw; no runtime behavior supplied",
              runtime_classpath_forbidden=str(path.parents[3]), declarations=audit)
(OUT / "header-audit.json").write_text(json.dumps(report, indent=2) + "\n")
require(hashlib.sha256(JAR.read_bytes()).hexdigest() == EXPECTED, "original archive changed")
print(json.dumps({k: report[k] for k in report if k != "declarations"}))
