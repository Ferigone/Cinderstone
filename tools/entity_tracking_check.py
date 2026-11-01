#!/usr/bin/env python3
"""Verify native tracking/view policy and its per-observer replication adapter."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
SHA = '8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
CARGO = '/root/.local/share/mc-rust-toolchain/cargo/bin/cargo'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(command, log, env=None):
    result = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True)
    output = result.stdout + result.stderr
    log.write_text(output)
    if result.returncode:
        raise RuntimeError(f'{command} failed ({result.returncode}):\n{output[-8000:]}')
    return output


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--skip-generation', action='store_true')
    parser.add_argument('--release', action='store_true')
    args = parser.parse_args()
    runtime = ROOT / 'runtime/entity_tracking'
    runtime.mkdir(parents=True, exist_ok=True)
    if not args.skip_generation:
        run([sys.executable, str(ROOT / 'tools/entity_tracking_generate.py')], runtime / 'generation-check.log')
    original = json.loads((ROOT / 'evidence/entity-tracking-oracle.json').read_text())
    assert original['oracle_inner_sha256'] == SHA
    for key, name in [('vectors_sha256', 'fixtures/entity_tracking_vectors.json'),
                      ('facts_sha256', 'tools/entity_tracking_facts.json'),
                      ('view_vectors_sha256', 'fixtures/entity_tracking_views.json')]:
        assert original[key] == digest(ROOT / name)
    env = os.environ.copy()
    env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',
               RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup', CARGO_INCREMENTAL='0')
    profiles = {}
    for profile in ['debug'] + (['release'] if args.release else []):
        tests = {}
        for target in ['entity_tracking::tests', 'replication::tests']:
            command = [CARGO, 'test'] + (['--release'] if profile == 'release' else [])
            command += ['--lib', target, '--', '--nocapture']
            output = run(command, runtime / ('rust-' + profile + '-' + target.split('::')[0] + '.log'), env)
            match = re.search(r'test result: ok\. (\d+) passed; 0 failed', output)
            assert match, output[-3000:]
            tests[target] = int(match[1])
        profiles[profile] = {'tests_passed': tests, 'mismatches': 0}
    sources = ['src/entity_tracking.rs', 'src/entity.rs', 'src/replication.rs', 'src/item_entity.rs',
               'tools/entity_tracking_oracle.java', 'tools/entity_tracking_generate.py']
    report = {
        'ok': True, 'minecraft': '1.21', 'oracle_inner_sha256': SHA,
        'queries': original['queries'], 'states': original['states'],
        'type_facts': original['type_facts'], 'default_requested_view_distance': 2,
        'view_iterations': original['view_iterations'], 'view_differences': original['view_differences'],
        'profiles': profiles, 'source_sha256': {name: digest(ROOT / name) for name in sources},
        'vectors_sha256': original['vectors_sha256'], 'facts_sha256': original['facts_sha256'],
        'view_vectors_sha256': original['view_vectors_sha256'],
        'actual_java_methods': original['original_methods'] + ['aqg.Positioned.forEach', 'aqg.difference'],
        'scope': 'Inclusive XZ radius, player view clamp, dedicated i32 range scaling, all indirect '
                 'passenger types, self exclusion, lazy broadcast/chunk gates, pending send exclusion, '
                 'ServerPlayer spectator/camera rules and pair/unpair state transitions. Original '
                 'rounded chunk-view lists and difference callback order also match. Replication '
                 'adapters require observer context and preserve existing revision/metadata/equipment '
                 'semantics while applying the validated policy.',
        'not_verified': ['Exact ChunkMap invocation scheduling: section changes, observer movement and chunk sends',
                         'Live chunk pending/send/drop concurrency and full ServerEntity pairing callback side effects',
                         'Vehicles in the current host, although all indirect passenger ranges are explicit policy inputs'],
        'reproduce': 'python3 tools/entity_tracking_check.py --release',
    }
    (ROOT / 'evidence/entity-tracking-conformance.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
