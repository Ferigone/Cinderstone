#!/usr/bin/env python3
"""Build the local, uncommitted loot oracle: remap the pinned server and compile the harness.

    python3 tools/loot_oracle_setup.py --oracle-dir /path/outside/git

The pinned 1.21 server jar and its official mappings are downloaded with SHA-1 and size
verification (tools/bootstrap_oracle.py), the server classes are renamed to their official
names with tools/loot_remap.java, and tools/loot_oracle.java is compiled against the result.
Everything is written below --oracle-dir; none of it may be committed (proprietary Java
binaries). Requires a JDK 21 on PATH. The directory then holds `cp.txt` and the compiled
`loot_oracle` class that tools/loot_generate.py consumes.
"""
import argparse
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools"))
import bootstrap_oracle  # noqa: E402

EXPORTS = [
    "--add-exports", "java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED",
    "--add-exports", "java.base/jdk.internal.org.objectweb.asm.commons=ALL-UNNAMED",
]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--oracle-dir", type=Path, required=True)
    directory = parser.parse_args().oracle_dir.resolve()
    bootstrap_oracle.bootstrap(directory)
    remap_out = directory / "remap"
    remap_out.mkdir(exist_ok=True)
    subprocess.run(["javac", "-proc:none", *EXPORTS, "-d", str(remap_out), str(ROOT / "tools/loot_remap.java")], check=True)
    named = directory / "server-named.jar"
    subprocess.run(
        ["java", *EXPORTS, "-cp", str(remap_out), "loot_remap",
         str(directory / "server-1.21-mappings.txt"), str(directory / "server-inner-1.21.jar"), str(named)],
        check=True,
    )
    libraries = sorted(str(p) for p in (directory / "libraries").rglob("*.jar"))
    classpath = ":".join([str(named), *libraries])
    (directory / "cp.txt").write_text(classpath + "\n")
    subprocess.run(
        ["javac", "-proc:none", "--add-exports", "java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED",
         "-cp", classpath, "-d", str(directory), str(ROOT / "tools/loot_oracle.java")],
        check=True,
    )
    print(f"oracle ready in {directory}")


if __name__ == "__main__":
    main()
