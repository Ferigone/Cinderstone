#!/usr/bin/env python3
"""Execute unchanged ServerPlayer.drop to prove literal statistic gates."""
from pathlib import Path
import hashlib, itertools, json, os, subprocess

ROOT = Path(__file__).resolve().parents[1]
ORACLE = ROOT.parent / 'mc121-oracle'
JDK = Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
SHA = '8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
FLAGS = ['--add-exports', 'java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED']


def context():
    jar = ORACLE / 'server-inner-1.21.jar'
    assert hashlib.sha256(jar.read_bytes()).hexdigest() == SHA
    runtime = ROOT / 'runtime/player-stats'
    runtime.mkdir(parents=True, exist_ok=True)
    cp = os.pathsep.join(map(str, [runtime, jar, *sorted((ORACLE / 'libraries').rglob('*.jar'))]))
    compile = subprocess.run([JDK / 'javac', *FLAGS, '-proc:none', '-cp', cp, '-d', runtime,
                              ROOT / 'tools/player_stats_oracle.java', ROOT / 'tools/player_stats_drop_oracle.java'],
                             text=True, capture_output=True)
    if compile.returncode:
        raise RuntimeError(compile.stdout + compile.stderr)
    return runtime, cp


def replay(queries):
    runtime, cp = context()
    original = subprocess.run([JDK / 'java', *FLAGS, '-Xmx1G', '-cp', cp, 'oracle.player_stats_drop_oracle'],
                              input=''.join(json.dumps(query) + '\n' for query in queries), text=True, capture_output=True)
    (runtime / 'drop-oracle.log').write_text(original.stdout + original.stderr)
    if original.returncode:
        raise RuntimeError((original.stdout + original.stderr)[-7000:])
    rows = [json.loads(line[7:]) for line in original.stdout.splitlines() if line.startswith('ORACLE:')]
    assert len(rows) == len(queries), (len(rows), len(queries))
    return rows


def main():
    registry = json.loads((ROOT / 'fixtures/player-stats-registry.json').read_text())
    items = next(kind['values'] for kind in registry['types'] if kind['type_id'] == 4)
    item_ids = {item['value_name']: item['value_id'] for item in items}
    queries = [dict(item_id=item, count=count, trace_item=trace, around=around,
                    world_accepts=accepts, after_add=after, replacement_item_id=item_ids['minecraft:dirt'])
               for item, count, trace, around, accepts, after in itertools.product(
                   [item_ids['minecraft:air'], item_ids['minecraft:stone'], item_ids['minecraft:ender_pearl']],
                   [-1, 0, 1, 5, 64, 99, 2147483647], [False, True], [False, True], [False, True],
                   ['none', 'empty', 'replace', 'replace_empty'])]
    rows = replay(queries)
    fixture = dict(minecraft='1.21', oracle_inner_sha256=SHA,
                   scope='Unchanged ServerPlayer.drop and Player.drop; only world add callback and outer awardStat observed. Actual original StatsCounter.increment consumes every award; scoreboard/advancement effects are outside this fixture.',
                   cases=[dict(query=query, expected=row) for query, row in zip(queries, rows)])
    output = ROOT / 'fixtures/player-stats-drop-vectors.json'
    temporary = output.with_suffix('.json.tmp')
    temporary.write_text(json.dumps(fixture, separators=(',', ':')) + '\n')
    temporary.replace(output)
    print(json.dumps(dict(ok=True, cases=len(rows), award_stat_callbacks=sum(
        sum(event['callback'] == 'award_stat' for event in row['callbacks']) for row in rows),
        sha256=hashlib.sha256(output.read_bytes()).hexdigest())))


if __name__ == '__main__':
    main()
