#!/usr/bin/env python3
"""Original avu NBT load log argument and synchronous state observations."""
import argparse
import hashlib
import json
import subprocess

from server_recipe_book_generate import Original, ROOT, JDK, FLAGS, SHA, atomic_json, compound, strings, settings
from server_recipe_book_original_check import comparable, first_difference


def queries():
    facts = json.loads((ROOT / 'fixtures/server-recipe-book-facts.json').read_text())
    normal = [r['id'] for r in facts['recipes'] if not r['is_special']]
    special = [r['id'] for r in facts['recipes'] if r['is_special']]
    return [
        dict(name='log/load-sequential-state', operations=[
            dict(op='direct_add', recipe=normal[0]), dict(op='add_highlight', recipe=normal[1]),
            dict(op='set_settings', settings=settings(255)),
            dict(op='load_nbt', nbt=compound({
                'recipes':strings([normal[2], 'Bad:id', 'test:missing', normal[3]]),
                'toBeDisplayed':strings([special[0], 'test:absent', 'a b', normal[4]])})), dict(op='init')]),
        dict(name='log/special-known-filter', operations=[dict(op='load_nbt', nbt=compound({
            'recipes':strings([special[0], 'test:absent', normal[0]]),
            'toBeDisplayed':strings([special[0], 'test:absent'])})), dict(op='init')]),
        dict(name='log/repeated-invalid-and-unknown', operations=[dict(op='load_nbt', nbt=compound({
            'recipes':strings(['INVALID', 'INVALID', 'test:absent', 'test:absent']),
            'toBeDisplayed':strings(['INVALID', 'test:absent'])}))]),
        dict(name='log/normalized-resource-identifiers', operations=[dict(op='load_nbt', nbt=compound({
            'recipes':strings([':foo', '', ':', 'foo:', {'utf16':[0xd800]}, {'utf16':[0xdc00]}]),
            'toBeDisplayed':strings(['minecraft:Upper'])}))]),
        dict(name='log/empty-compound-no-log', operations=[dict(op='direct_add', recipe=normal[0]),
            dict(op='load_nbt', nbt=compound({}))]),
    ]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--check', action='store_true', help='Replay and compare the frozen fixture without replacing it')
    args = parser.parse_args()
    original = Original()
    source = ROOT / 'tools/server_recipe_book_load_logs.java'
    subprocess.run([JDK / 'javac', *FLAGS, '-proc:none', '-cp', original.cp, '-d', original.runtime, source],
                   cwd=ROOT, check=True, capture_output=True, text=True)
    inputs = queries()
    result = subprocess.run([JDK / 'java', *FLAGS, '-Xmx1G', '-cp', original.cp,
                             'oracle.server_recipe_book_load_logs', original.bootstrap],
                            cwd=ROOT, capture_output=True, text=True,
                            input=''.join(json.dumps(q, ensure_ascii=True) + '\n' for q in inputs))
    (original.runtime / 'load-logs.log').write_text(result.stdout + result.stderr)
    if result.returncode:
        raise RuntimeError((result.stdout + result.stderr)[-6000:])
    actual = [json.loads(line[7:]) for line in result.stdout.splitlines() if line.startswith('ORACLE:')]
    assert len(actual) == len(inputs)
    assert all(s['error'] is None for row in actual for s in row['states'])
    fixture = dict(minecraft='1.21', oracle_inner_sha256=SHA,
                   scope='Unchanged avu.fromNbt/loadRecipes executes with forwarding org.slf4j.Logger observer at its outer error-call boundary. Every message template, argument UTF-16 code unit sequence and book state is captured synchronously, then forwarded to the original logger. argument_utf16 is authoritative for isolated surrogates; the human-readable argument field passes through the UTF-8 stdout encoder. No original class is transformed. This is a separate log-observation corpus; the frozen transition corpus is unchanged.',
                   cases=[dict(query=q, expected=row) for q, row in zip(inputs, actual)])
    path = ROOT / 'fixtures/server-recipe-book-load-logs.json'
    if args.check:
        frozen = json.loads(path.read_text())
        assert len(frozen['cases']) == len(fixture['cases'])
        for expected, observed in zip(frozen['cases'], fixture['cases']):
            assert expected['query'] == observed['query']
            difference = first_difference(comparable(expected['query'], expected['expected']),
                                          comparable(observed['query'], observed['expected']))
            assert difference is None, (expected['query']['name'], difference)
    else:
        atomic_json(path, fixture)
    source_paths = ['tools/server_recipe_book_load_logs.java', 'tools/server_recipe_book_load_logs_generate.py',
                    'tools/server_recipe_book_oracle.java', 'tools/server_recipe_book_generate.py', str(path.relative_to(ROOT))]
    report = dict(ok=True, minecraft='1.21', original_inner_sha256=SHA, native_comparison=False,
                  evidence_kind='Actual unchanged-original-class capture and frozen-fixture replay' if args.check else 'Actual unchanged-original-class capture',
                  histories=len(inputs), snapshots=sum(len(row['states']) for row in actual),
                  literal_log_observations=sum(e['callback'] == 'load_log' for row in actual for s in row['states'] for e in s['events']),
                  source_sha256={name:hashlib.sha256((ROOT / name).read_bytes()).hexdigest() for name in source_paths})
    atomic_json(ROOT / 'evidence/server-recipe-book-load-logs.json', report)
    print(json.dumps(report))


if __name__ == '__main__':
    main()
