#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-only
"""Reproduce the six packet witnesses using a bootstrapped, unchanged 1.21 JAR."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import zipfile

from bootstrap_oracle import PROJECT, download_verified

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--oracle-dir', type=Path, required=True)
parser.add_argument('--out', type=Path, required=True)
args = parser.parse_args()
lock = json.loads((PROJECT / 'fixtures/source-lock.json').read_text())
download_verified(lock['oracle'], args.oracle_dir / 'server-1.21.jar')
with zipfile.ZipFile(args.oracle_dir / 'server-1.21.jar') as bundle:
    checksum, version, _ = bundle.read('META-INF/versions.list').decode().strip().split('\t')
    inner = (args.oracle_dir / 'server-inner-1.21.jar').read_bytes()
    if version != '1.21' or hashlib.sha256(inner).hexdigest() != checksum:
        parser.error('Inner JAR differs from the pinned unchanged server')
classpath = ':'.join([str((args.oracle_dir / 'server-inner-1.21.jar').resolve()),
                      *map(str, sorted((args.oracle_dir / 'libraries').resolve().rglob('*.jar')))])
with tempfile.TemporaryDirectory(prefix='border-packet-oracle-') as build:
    subprocess.run(['java', '-m', 'jdk.compiler/com.sun.tools.javac.Main', '-proc:none',
                    '-cp', classpath, '-d', build,
                    str(PROJECT / 'tools/border_login_oracle.java')], check=True)
    result = subprocess.run(['java', '-cp', build + ':' + classpath,
                             'oracle.border_login_oracle'], text=True, capture_output=True, check=True)
    marker = 'BORDER_PACKETS:'
    vectors = next(json.loads(line.split(marker, 1)[1])
                   for line in result.stdout.splitlines() if marker in line)
    args.out.write_text(json.dumps(vectors, indent=2) + '\n')
    print(f'{len(vectors)} original packet witnesses written to {args.out}')
