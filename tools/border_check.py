#!/usr/bin/env python3
"""Verify border settings and runtime math against original Minecraft1.21."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--skip-generate', action='store_true', help='use the pinned Java corpus')
    parser.add_argument('--release', action='store_true', help='also check optimized thinLTO tests')
    args = parser.parse_args()
    if not args.skip_generate:
        subprocess.run([sys.executable, ROOT / 'tools/border_generate.py'], check=True, cwd=ROOT)
    path = ROOT / 'fixtures/border_vectors.json'
    fixture = json.loads(path.read_text())
    runtime = ROOT / 'runtime/border-oracle'
    runtime.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',
               RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup', CARGO_INCREMENTAL='0')
    env['PATH'] = env['CARGO_HOME'] + '/bin:' + env.get('PATH', '')
    cargo = str(Path(env['CARGO_HOME']) / 'bin/cargo')
    profiles = ['debug', 'release'] if args.release else ['debug']
    tests = {}
    required = [
        'border::tests::actual_java_settings_numeric_conversion_defaults_and_canonical_nbt',
        'border::tests::actual_java_moving_border_bounds_tick_and_point_queries',
        'level::tests::border_checkpoint_preserves_unknown_fields_and_interpolation',
    ]
    for profile in profiles:
        command = [cargo, 'test'] + (['--release'] if profile == 'release' else [])
        command += ['--lib', 'border', '--', '--nocapture']
        result = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True)
        output = result.stdout + result.stderr
        (runtime / ('rust-' + profile + '.log')).write_text(output)
        if result.returncode:
            raise RuntimeError(output[-7000:])
        for name in required:
            if 'test ' + name + ' ... ok' not in result.stdout:
                raise RuntimeError('Required test did not execute: ' + name + '\n' + output[-7000:])
        count = re.search(r'test result: ok\. (\d+) passed', result.stdout)
        if not count:
            raise RuntimeError('Missing native test result: ' + output[-7000:])
        tests[profile] = dict(passed=int(count.group(1)), failed=0, mismatches=0)
    report = dict(
        ok=True, minecraft='1.21', oracle_inner_sha256=fixture['oracle_inner_sha256'],
        scope='Original WorldBorder.Settings.read/write, static/moving border bounds, interpolation, '
              'tick conversion and point/distance queries. Only Util.timeSource is injected; '
              'the original Minecraft classes and methods are unchanged.',
        settings_cases=len(fixture['settings_cases']),
        world_cases=len(fixture['world_cases']),
        world_states=sum(len(case['expected']) for case in fixture['world_cases']),
        point_queries=sum(len(state['queries']) for case in fixture['world_cases'] for state in case['expected']),
        official_defaults=True, tests=tests,
        source_sha256={name: sha(ROOT / name) for name in [
            'src/border.rs', 'src/level.rs', 'tools/border_oracle.java',
            'tools/border_generate.py', 'fixtures/border_vectors.json',
        ]},
        not_verified=['Border listeners and client packet ordering', 'Border damage effects',
                      'Malformed settings that produce invalid collision shapes'],
        reproduce='python3 tools/border_check.py --release',
    )
    (ROOT / 'evidence/border-conformance.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
