#!/usr/bin/env python3
"""Replay original counter/drop fixtures, then run bounded native comparison."""
from pathlib import Path
import copy, hashlib, json, os, re, subprocess
from player_stats_drop_generate import context, replay, JDK, FLAGS, SHA

ROOT = Path(__file__).resolve().parents[1]


def run(command, **kwargs):
    result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True, **kwargs)
    if result.returncode:
        raise RuntimeError((result.stdout + result.stderr)[-9000:])
    return result


def comparable_counter(row, query):
    result = copy.deepcopy(row)
    for state, operation in zip(result['states'], query['operations']):
        for packet in state['packets']:
            # Original identity hash map iteration is observable but not a
            # semantic protocol order. Retain byte proof for ordered input
            # and all zero/single-entry packets, compare larger maps by keys.
            if operation['op'] != 'ordered_packet' and len(packet['values']) > 1:
                packet.pop('wire')
    return result


def main():
    runtime, cp = context()
    registry = json.loads((ROOT / 'fixtures/player-stats-registry.json').read_text())
    facts = run([JDK / 'java', *FLAGS, '-Xmx1G', '-cp', cp, 'oracle.player_stats_oracle', 'facts'])
    observed = [json.loads(line[7:]) for line in facts.stdout.splitlines() if line.startswith('ORACLE:')]
    assert observed == [registry], 'original statistic registry changed'
    fixture = json.loads((ROOT / 'fixtures/player-stats-vectors.json').read_text())
    original = run([JDK / 'java', *FLAGS, '-Xmx1G', '-cp', cp, 'oracle.player_stats_oracle'],
                   input=''.join(json.dumps(row['query']) + '\n' for row in fixture['cases']))
    (runtime / 'check-original.log').write_text(original.stdout + original.stderr)
    observed = [json.loads(line[7:]) for line in original.stdout.splitlines() if line.startswith('ORACLE:')]
    assert len(observed) == len(fixture['cases'])
    for index, (row, actual) in enumerate(zip(fixture['cases'], observed)):
        assert comparable_counter(actual, row['query']) == comparable_counter(row['expected'], row['query']), f'original counter case {index} drifted'
    drops = json.loads((ROOT / 'fixtures/player-stats-drop-vectors.json').read_text())
    observed_drop = replay([row['query'] for row in drops['cases']])
    assert observed_drop == [row['expected'] for row in drops['cases']], 'original ServerPlayer.drop fixture drifted'
    env = os.environ.copy()
    env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',
               RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup', CARGO_INCREMENTAL='0')
    native = run(['/root/.local/share/mc-rust-toolchain/cargo/bin/cargo', 'test', '--lib',
                  'player_stats::tests', '--', '--nocapture'], env=env)
    (runtime / 'check-native.log').write_text(native.stdout + native.stderr)
    assert int(re.search(r'test result: ok\. (\d+) passed', native.stdout).group(1)) == 3
    reports = {}
    for line in native.stdout.splitlines():
        for label in ['PLAYER_STATS_CONFORMANCE', 'PLAYER_STATS_DROP_CONFORMANCE']:
            if line.startswith(label + ':'):
                reports[label] = json.loads(line.removeprefix(label + ':'))
    assert len(reports) == 2 and all(row['ok'] and row['mismatches'] == 0 for row in reports.values())
    report = dict(ok=True, minecraft='1.21', kind='bounded native differential conformance',
                  oracle_inner_sha256=SHA, native_tests_passed=3, full_gameplay_compatibility=False,
                  original_registry_keys=8060, counter=reports['PLAYER_STATS_CONFORMANCE'],
                  drop=reports['PLAYER_STATS_DROP_CONFORMANCE'],
                  current_stat_file_scope='Valid canonical DataVersion3953 registry names and int32 values; merge/load, no new dirty marks, and semantic JSON object equality. No legacy DFU or malformed-file compatibility claim.',
                  packet_order_scope='Unordered original multi-entry HashSet/Object2IntOpenHashMap packets compare decoded maps. Explicit Object2IntLinkedOpenHashMap and empty/single-entry packets compare literal payload bytes.',
                  not_verified=['Active scoreboard objectives and scoreboard callbacks',
                                'Advancement criteria and advancement packets',
                                'Every gameplay statistic trigger beyond the proved pickup/drop event inputs',
                                'Historical DataFixer upgrades, unknown names and malformed stat files',
                                'Live client statistic request dispatch, player login/loading and disconnect disk persistence'])
    sources = ['src/player_stats.rs', 'src/protocol.rs', 'tools/player_stats_oracle.java',
               'tools/player_stats_drop_oracle.java', 'tools/player_stats_generate.py',
               'tools/player_stats_drop_generate.py', 'tools/player_stats_check.py',
               'tools/player_stats_tests.rs', 'fixtures/player-stats-registry.json',
               'fixtures/player-stats-vectors.json', 'fixtures/player-stats-drop-vectors.json',
               'fixtures/item-pickup-vectors.json']
    report['source_sha256'] = {name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest() for name in sources}
    (ROOT / 'evidence/player-stats-conformance.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({key: value for key, value in report.items() if key not in ['source_sha256', 'not_verified']}))


if __name__ == '__main__':
    main()
