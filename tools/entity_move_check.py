#!/usr/bin/env python3
"""Compare native Entity.move orchestration with unchanged signed1.21 methods."""
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
    process = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True)
    output = process.stdout + process.stderr
    log.write_text(output)
    if process.returncode:
        raise RuntimeError(f'{command} failed ({process.returncode}):\n{output[-9000:]}')
    return output


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--skip-generation', action='store_true')
    parser.add_argument('--release', action='store_true', help='also check optimized thinLTO arithmetic')
    args = parser.parse_args()
    runtime = ROOT / 'runtime/entity_move'
    runtime.mkdir(parents=True, exist_ok=True)
    if not args.skip_generation:
        run([sys.executable, str(ROOT / 'tools/entity_move_generate.py')], runtime / 'generation-check.log')
    fixture_path = ROOT / 'fixtures/entity_move_vectors.json'
    facts_path = ROOT / 'tools/entity_move_facts.json'
    fixture = json.loads(fixture_path.read_text())
    facts = json.loads(facts_path.read_text())
    original = json.loads((ROOT / 'evidence/entity_move_oracle.json').read_text())
    assert fixture['oracle_inner_sha256'] == facts['oracle_inner_sha256'] == original['oracle_inner_sha256'] == SHA
    assert original['vectors_sha256'] == digest(fixture_path)
    assert original['facts_sha256'] == digest(facts_path)
    assert len(facts['rows']) == 26684

    env = os.environ.copy()
    env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',
               RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup', CARGO_INCREMENTAL='0')
    profiles = {}
    for profile in ['debug'] + (['release'] if args.release else []):
        command = [CARGO, 'test'] + (['--release'] if profile == 'release' else [])
        command += ['--lib', 'entity_move::tests', '--', '--nocapture']
        output = run(command, runtime / ('rust-' + profile + '.log'), env)
        match = re.search(r'test result: ok\. (\d+) passed; 0 failed', output)
        assert match, output[-4000:]
        profiles[profile] = {'tests_passed': int(match[1]), 'mismatches': 0}

    sources = ['src/entity_move.rs', 'src/entity_state.rs', 'src/player_physics.rs',
               'src/geometry.rs', 'tools/entity_move_oracle.java', 'tools/entity_move_generate.py',
               'tools/placement_tags.java', 'fixtures/placement-block-tags-1.21.json']
    report = {
        'ok': True, 'minecraft': '1.21', 'oracle_inner_sha256': SHA,
        'queries': len(fixture['cases']),
        'move_states': sum(len(row['expected']) for row in fixture['cases']),
        'profiles': profiles,
        'block_state_facts': len(facts['rows']),
        'callback_class_sets': len(facts['callback_classes']),
        'vectors_sha256': digest(fixture_path), 'facts_sha256': digest(facts_path),
        'source_sha256': {name: digest(ROOT / name) for name in sources},
        'actual_java_methods': original['original_methods'],
        'scope': 'Entity.move ordering, noPhysics, stuck multiplier/velocity reset, per-axis piston '
                 'restrictions and game-time reset, geometric collision and step search, base setPos '
                 'and AABB rebuild, collision flags, onGround/support fallback, onPos and speed '
                 'factor/default tags, removed-after-fall exit, fire predicate and wet gates. '
                 'Native base Block/slime afterFall and slime stepOn scalar helpers are compared '
                 'before/after the actual original block callbacks.',
        'controlled_callbacks': original['controlled_callbacks'],
        'not_verified': ['Runtime chunk index and supporting-block selection provider',
                         'Full fall damage / inside-block / Player edge-backoff effects',
                         'Movement sound/game-event emission for modes other than NONE',
                         'Extinguish sound RNG/audio and complete fluid/baseTick/travel integration',
                         'Other custom block/entity virtual overrides and living slime context'],
        'reproduce': 'python3 tools/entity_move_check.py --release',
    }
    (ROOT / 'evidence/entity-move-conformance.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
