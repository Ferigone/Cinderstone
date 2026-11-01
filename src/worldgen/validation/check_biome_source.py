#!/usr/bin/env python3
"""Regenerate ordered registry parameters and original biome-source witnesses."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
INNER = "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"


def digest(path, algorithm="sha256"):
    return hashlib.new(algorithm, path.read_bytes()).hexdigest()


def run(command):
    result = subprocess.run(command, capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(f"command failed: {command[0]}\n{result.stdout}\n{result.stderr}")
    return result.stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oracle-dir", type=Path, required=True)
    parser.add_argument("--java-home", type=Path)
    args = parser.parse_args()
    oracle = args.oracle_dir.resolve()
    lock = json.loads((ROOT / "fixtures/source-lock.json").read_text())
    for section, filename in [("oracle", "server-1.21.jar"), ("mappings", "server-1.21-mappings.txt")]:
        path = oracle / filename
        if path.stat().st_size != lock[section]["size"] or digest(path, "sha1") != lock[section]["sha1"]:
            raise RuntimeError(f"wrong pinned artifact: {filename}")
    jar = oracle / "server-inner-1.21.jar"
    if digest(jar) != INNER:
        raise RuntimeError("wrong pinned inner server")
    java = str(args.java_home / "bin/java") if args.java_home else shutil.which("java")
    javac = str(args.java_home / "bin/javac") if args.java_home else shutil.which("javac")
    if not java or not javac:
        raise RuntimeError("Java 21 java/javac must be available")
    source = HERE / "WorldgenBiomeSourceOracle.java"
    with tempfile.TemporaryDirectory(prefix="worldgen-biome-source-") as temporary:
        work = Path(temporary)
        classes = work / "classes"
        classes.mkdir()
        cp = os.pathsep.join(map(str, [classes, jar, *sorted((oracle / "libraries").rglob("*.jar"))]))
        run([javac, "-proc:none", "-cp", cp, "-d", str(classes), str(source)])
        run([java, "-Xmx1024m", "-cp", cp, "oracle.WorldgenBiomeSourceOracle", str(work), str(source)])
        results = []
        for filename in ["world-generation-multinoise-parameters.json", "world-generation-biome-source-vectors.json"]:
            observed = json.loads((work / filename).read_text())
            committed = json.loads((ROOT / "fixtures" / filename).read_text())
            if observed != committed:
                raise RuntimeError(f"original observation differs from fixture: {filename}")
            results.append(dict(fixture=filename, matched_original=True, sha256=digest(ROOT / "fixtures" / filename)))
        print(json.dumps(dict(ok=True, minecraft="1.21", source_inner_sha256=INNER,
                              oracle_source_sha256=digest(source), checks=results,
                              scope="Ordered original registry parameters and original source searches/thresholds only; native focused checks are separate.")))


if __name__ == "__main__":
    main()
