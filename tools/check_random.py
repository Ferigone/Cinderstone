#!/usr/bin/env python3
"""Compare the Rust RNG core with the actual shipped Minecraft 1.21 classes.

No Java algorithm is reimplemented here: both processes receive the same mixed
operation transcript, and their exact integer / floating-point bit outputs are
compared. The official JAR and bundled dependency JARs remain external inputs.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import random
import shutil
import subprocess
import sys
import tempfile

EXPECTED_INNER_JAR_SHA256 = "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"


def main() -> int:
    project = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oracle-dir", type=Path, default=project.parent / "mc121-oracle")
    parser.add_argument("--java-home", type=Path, default=Path("/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1"))
    parser.add_argument("--cargo", default=shutil.which("cargo") or "/root/.local/share/mc-rust-toolchain/rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/cargo")
    parser.add_argument("--probe", type=Path)
    parser.add_argument("--seeds", type=int, default=1000)
    args = parser.parse_args()
    if args.seeds < 10:
        parser.error("at least 10 seeds are required to retain the boundary cases")
    jar = args.oracle_dir.resolve() / "server-inner-1.21.jar"
    if not jar.is_file():
        parser.error(f"missing official Minecraft 1.21 inner JAR: {jar}")
    with jar.open("rb") as source:
        jar_sha256 = hashlib.file_digest(source, "sha256").hexdigest()
    # This SHA-256 is also recorded by the official outer JAR's
    # META-INF/versions.list. Reject other artifacts before executing Java.
    if jar_sha256 != EXPECTED_INNER_JAR_SHA256:
        parser.error(f"Minecraft oracle JAR digest mismatch: {jar_sha256}")
    libraries = sorted((args.oracle_dir / "libraries").rglob("*.jar"))
    seeds = [0, 1, -1, -(1 << 63), (1 << 63) - 1, -(1 << 48), (1 << 48) - 1,
             25_214_903_917, -25_214_903_917, 0x123456789ABCDEF]
    generator = random.Random(0x121_767)
    while len(seeds) < args.seeds:
        value = generator.getrandbits(64)
        seeds.append(value if value < (1 << 63) else value - (1 << 64))
    bounds = [-(1 << 31), -1, 0, 1, 2, 3, 7, 8, 15, 16, 17, 255, 256,
              257, 65535, 65536, 65537, (1 << 30) - 1, 1 << 30,
              (1 << 30) + 1, (1 << 31) - 2, (1 << 31) - 1]
    bit_counts = [0, 1, 2, 5, 16, 24, 26, 27, 31, 32, 33, 48, 49, -1, -(1 << 31), (1 << 31) - 1]
    commands = []
    for index, seed in enumerate(seeds):
        commands.append(f"new\t{seed}")
        commands.extend(f"bits\t{bits}" for bits in bit_counts)
        commands.extend(f"bounded\t{bound}" for bound in bounds)
        for _ in range(8):
            commands.extend(["i32", "i64", "bool", "f32", "f64", f"bounded\t{(1 << 30) + 1}"])
        commands.append(f"seed\t{seeds[(index + 1) % len(seeds)]}")
        commands.extend(["i32", "i64", "bool", "f32", "f64"])
    transcript = "\n".join(commands) + "\n"
    cargo_env = os.environ.copy()
    cargo_env["PATH"] = str(Path(args.cargo).resolve().parent) + os.pathsep + cargo_env.get("PATH", "")
    if args.probe:
        probe = args.probe.resolve()
    else:
        subprocess.run([args.cargo, "build", "--locked", "--bin", "random_probe"], cwd=project, env=cargo_env, check=True)
        target_dir = Path(cargo_env.get("CARGO_TARGET_DIR", project / "target"))
        if not target_dir.is_absolute(): target_dir = project / target_dir
        probe = target_dir / "debug" / ("random_probe.exe" if os.name == "nt" else "random_probe")
    with tempfile.TemporaryDirectory(prefix="mc121-random-oracle-") as classes:
        subprocess.run([str(args.java_home / "bin" / "javac"), "-d", classes, str(project / "tools" / "LegacyRandomOracle.java")], check=True)
        classpath = os.pathsep.join([classes, str(jar), *map(str, libraries)])
        java = subprocess.run([str(args.java_home / "bin" / "java"), "-cp", classpath, "oracle.LegacyRandomOracle"], input=transcript, text=True, capture_output=True, check=True)
        if "ORACLE_CLASS=dyz;SOURCE=" not in java.stderr or jar.name not in java.stderr:
            raise RuntimeError("Java output did not verify the actual Minecraft oracle class origin")
        rust = subprocess.run([str(probe)], input=transcript, text=True, capture_output=True, check=True)
    java_lines, rust_lines = java.stdout.splitlines(), rust.stdout.splitlines()
    if len(java_lines) != len(commands) or len(rust_lines) != len(commands):
        raise RuntimeError(f"incomplete traces: input={len(commands)} java={len(java_lines)} rust={len(rust_lines)}")
    mismatches = []
    for index, (expected, observed) in enumerate(zip(java_lines, rust_lines)):
        if expected != observed:
            mismatches.append({"line": index + 1, "input": commands[index], "java": expected, "rust": observed})
            if len(mismatches) == 10: break
    result = {
        "success": not mismatches,
        "oracle": "actual Minecraft 1.21 LegacyRandomSource dyz / BitRandomSource dyn",
        "jar_sha256": jar_sha256,
        "seeds": len(seeds),
        "compared_outputs": len(commands),
        "trace_sha256": hashlib.sha256(java.stdout.encode()).hexdigest(),
        "methods": ["constructor", "setSeed", "next(bits)", "nextInt", "nextInt(bound)", "nextLong", "nextBoolean", "nextFloat(raw bits)", "nextDouble(raw bits)"],
        "excluded": ["Gaussian sampling", "positional random factories", "concurrent misuse exceptions"],
        "mismatches": mismatches,
    }
    print(json.dumps(result, indent=2))
    return 0 if result["success"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
