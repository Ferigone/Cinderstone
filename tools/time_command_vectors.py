#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-only
"""Execute the original TimeCommand registration/parser outside the checkout."""
import argparse
from pathlib import Path
import subprocess
from bootstrap_oracle import bootstrap, PROJECT

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--oracle-dir', type=Path, required=True)
parser.add_argument('--out', type=Path, required=True)
args = parser.parse_args()
directory = args.oracle_dir.resolve()
if directory.is_relative_to(PROJECT):
    parser.error('Keep the proprietary oracle and compiled harness outside the checkout')
bootstrap(directory)
exports = ['--add-exports', 'java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED',
           '--add-exports', 'java.base/jdk.internal.org.objectweb.asm.commons=ALL-UNNAMED']
subprocess.run(['java', '-m', 'jdk.compiler/com.sun.tools.javac.Main', '-proc:none', *exports,
                '-d', str(directory), str(PROJECT / 'tools/loot_remap.java')], check=True)
named = directory / 'server-named.jar'
subprocess.run(['java', *exports, '-cp', str(directory), 'loot_remap',
                str(directory / 'server-1.21-mappings.txt'), str(directory / 'server-inner-1.21.jar'),
                str(named)], check=True)
classpath = ':'.join([str(named), *map(str, sorted((directory / 'libraries').rglob('*.jar')))])
subprocess.run(['java', '-m', 'jdk.compiler/com.sun.tools.javac.Main', '-proc:none', '-cp', classpath,
                '-d', str(directory), str(PROJECT / 'tools/time_command_oracle.java')], check=True)
subprocess.run(['java', '-cp', str(directory) + ':' + classpath,
                'cinderstone.oracle.time_command_oracle', str(args.out.resolve())], check=True)
