#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-only
"""Run original StopCommand against the actual halt(false) running flag.

Prepare the named original using tools/loot_remap.java and the pinned bootstrap;
keep all proprietary inputs and compiled harness classes outside the checkout.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--named-jar', type=Path, required=True)
parser.add_argument('--original-jar', type=Path, required=True)
parser.add_argument('--libraries', type=Path, required=True)
parser.add_argument('--out', type=Path, required=True)
args = parser.parse_args()
lock = json.loads((ROOT / 'fixtures/source-lock.json').read_text())['oracle']
assert args.original_jar.stat().st_size == lock['size']
assert hashlib.sha1(args.original_jar.read_bytes()).hexdigest() == lock['sha1']
for artifact in [args.named_jar, args.original_jar, args.libraries]:
    if artifact.resolve().is_relative_to(ROOT):
        parser.error('Keep Java artifacts outside the checkout')
directory = Path(tempfile.mkdtemp(prefix='stop-command-oracle-'))
classpath = ':'.join([str(args.named_jar.resolve()),
                      *map(str, sorted(args.libraries.resolve().rglob('*.jar')))])
subprocess.run(['java', '-m', 'jdk.compiler/com.sun.tools.javac.Main', '-proc:none',
                '-cp', classpath, '-d', str(directory),
                str(ROOT / 'tools/stop_command_oracle.java')], check=True)
subprocess.run(['java', '-cp', str(directory) + ':' + classpath,
                'cinderstone.oracle.stop_command_oracle', str(args.out.resolve())], check=True)
