#!/usr/bin/env python3
"""Compare native listener decisions and bookkeeping with original Minecraft1.21.

The Java generators invoke the unchanged signed handler and listener methods.
Oracle-only subclasses supply Entity.move/doTick and world-side callbacks.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
JAR_SHA = '8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
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
    parser.add_argument('--skip-generation', action='store_true', help='use existing original-class vectors')
    parser.add_argument('--release', action='store_true', help='also compare the optimized thinLTO build')
    args = parser.parse_args()
    runtime = ROOT / 'runtime/player-movement'
    runtime.mkdir(parents=True, exist_ok=True)
    if not args.skip_generation:
        for name in ['player_movement_generate', 'player_movement_bookkeeping_generate']:
            run([sys.executable, str(ROOT / 'tools' / (name + '.py'))], runtime / (name + '.log'))

    vector_names = ['player_movement_vectors.json', 'player_movement_bookkeeping_vectors.json']
    fixtures = [json.loads((ROOT / 'tools' / name).read_text()) for name in vector_names]
    assert all(v['oracle_inner_sha256'] == JAR_SHA for v in fixtures)
    java_reports = [json.loads((ROOT / 'evidence' / name).read_text()) for name in [
        'player-movement-oracle.json', 'player-movement-bookkeeping-oracle.json']]
    for name, report in zip(vector_names, java_reports):
        assert report['oracle_inner_sha256'] == JAR_SHA
        assert report['vectors_sha256'] == digest(ROOT / 'tools' / name)

    env = os.environ.copy()
    env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',
               RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup', CARGO_INCREMENTAL='0')
    profiles = {}
    for profile in ['debug'] + (['release'] if args.release else []):
        command = [CARGO, 'test'] + (['--release'] if profile == 'release' else [])
        command += ['--lib', 'player_movement::tests', '--', '--nocapture']
        output = run(command, runtime / ('rust-' + profile + '.log'), env)
        match = re.search(r'test result: ok\. (\d+) passed; 0 failed', output)
        assert match, output[-3000:]
        profiles[profile] = {'tests_passed': int(match[1]), 'mismatches': 0}

    sources = ['src/player_movement.rs', 'src/player_physics.rs', 'src/geometry.rs',
               'src/protocol.rs', 'tools/player_movement_oracle.java',
               'tools/player_movement_generate.py', 'tools/player_movement_bookkeeping_generate.py']
    report = {
        'ok': True, 'minecraft': '1.21', 'oracle_inner_sha256': JAR_SHA,
        'movement_handler_queries': len(fixtures[0]['cases']),
        'bookkeeping_queries': len(fixtures[1]['cases']),
        'bookkeeping_observed_states': sum(len(row['expected']) for row in fixtures[1]['cases']),
        'original_serialized_packets': sum(len(state.get('packets', []))
                                          for row in fixtures[1]['cases'] for state in row['expected']),
        'profiles': profiles,
        'source_sha256': {name: digest(ROOT / name) for name in sources},
        'vector_sha256': {name: digest(ROOT / 'tools' / name) for name in vector_names},
        'actual_java_methods': ['aru.a(ahg) handleMovePlayer', 'aru.a(DDDFFSet) teleport',
                                'aru.a(agk) acceptTeleport', 'aru.l() resetPosition',
                                'aru.d() game-listener tick', 'aqv.P/Q changing-dimension methods',
                                'aec/abz packet constructors and FriendlyByteBuf serialization'],
        'scope': 'Native prepare/postcheck/finish helpers, exact lazy query and callback order, '
                 'movement packet counters, velocity allowance and f32 speed thresholds, floating gates, '
                 'teleport/ack/reset, all32 relative masks, signed integer wrap, gravity/floating tick '
                 'limits and all657 constructed packet bytes match original-class fixture outputs.',
        'controlled_callbacks': ['Entity.move returns an explicit resolved displacement',
                                 'ServerPlayer.moveTo and doTick callbacks control position/rotation',
                                 'Collision/air queries and fall/ground/known/statistics side effects are supplied',
                                 'Network send, disconnect and common listener tick are captured'],
        'not_verified': ['Complete Entity.move/baseTick/fluid/landing and damage pipelines',
                         'Full world-backed runtime integration and concurrent tick scheduling',
                         'Vehicle-floating branch and common-listener keepalive/idling'],
        'reproduce': 'python3 tools/player_movement_check.py --release',
    }
    path = ROOT / 'evidence/player-movement-conformance.json'
    path.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
