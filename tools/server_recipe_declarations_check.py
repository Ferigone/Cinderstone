#!/usr/bin/env python3
"""Actual original agc/czb STREAM decode/reencode of natural and controlled data."""
import hashlib
import json
import subprocess

from server_recipe_book_generate import Original, ROOT, ORACLE, JDK, SHA, FLAGS, atomic_json


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    original = Original()
    decoder = ROOT / 'tools/server_recipe_declarations_decode.java'
    subprocess.run([JDK / 'javac', *FLAGS, '-proc:none', '-cp', original.cp, '-d', original.runtime, decoder],
                   cwd=ROOT, check=True, capture_output=True, text=True)
    paths = [ROOT / 'fixtures/server-recipe-book-natural-declarations.json',
             ROOT / 'fixtures/server-recipe-book-controlled-declarations.json']
    result = subprocess.run([JDK / 'java', *FLAGS, '-Xmx1G', '-cp', original.cp,
                             'oracle.server_recipe_declarations_decode', original.bootstrap, *paths],
                            cwd=ROOT, capture_output=True, text=True)
    (original.runtime / 'declarations-decode.log').write_text(result.stdout + result.stderr)
    if result.returncode:
        raise RuntimeError((result.stdout + result.stderr)[-7000:])
    rows = [json.loads(line[7:]) for line in result.stdout.splitlines() if line.startswith('ORACLE:')]
    assert len(rows) == 1
    output = rows[0]
    assert output['original_inner_sha256'] == SHA
    for value in output['class_sources'].values():
        assert value.removeprefix('file:').lstrip('/') == str(ORACLE / 'server-inner-1.21.jar').lstrip('/')
    natural, controlled = output['results']
    facts = json.loads((ROOT / 'fixtures/server-recipe-book-facts.json').read_text())
    facts_by_id = {row['id']:row for row in facts['recipes']}
    for index, row in enumerate(output['results']):
        assert row['remaining_bytes'] == 0 and row['roundtrip_equal']
        source = json.loads(paths[index].read_text())
        assert row['payload_sha256'] == source['payload_sha256']
        assert row['decoded_count'] == len(row['recipes']) == 1290
        assert len({r['id'] for r in row['recipes']}) == 1290
        assert {r['id'] for r in row['recipes']} == facts_by_id.keys()
        for recipe in row['recipes']:
            assert recipe['is_special'] == facts_by_id[recipe['id']]['is_special']
            assert recipe['value_class'] == facts_by_id[recipe['id']]['value_class']
    nat_by_id = {r['id']:r for r in natural['recipes']}
    con_by_id = {r['id']:r for r in controlled['recipes']}
    different_holders = [key for key in sorted(nat_by_id) if nat_by_id[key]['holder_payload_sha256'] != con_by_id[key]['holder_payload_sha256']]
    natural_order = [r['id'] for r in natural['recipes']]
    controlled_order = [r['id'] for r in controlled['recipes']]
    decoded_fixture = dict(minecraft='1.21', original_inner_sha256=SHA,
                           scope='Actual unchanged agc.STREAM_CODEC.decode+encode and czb.STREAM_CODEC.encode; natural data comes from unmodified normal-login network capture. Controlled sorted-tag data is separate. No native serializer or advancement integration is claimed.',
                           class_sources=output['class_sources'], natural=natural, controlled=controlled)
    decoded_path = ROOT / 'fixtures/server-recipe-book-declarations-decoded.json'
    atomic_json(decoded_path, decoded_fixture)
    sources = ['tools/server_recipe_declarations_decode.java', 'tools/server_recipe_declarations_check.py',
               'tools/server_recipe_book_oracle.java', 'tools/server_recipe_book_generate.py',
               'fixtures/server-recipe-book-facts.json', *[str(p.relative_to(ROOT)) for p in paths], str(decoded_path.relative_to(ROOT))]
    report = dict(ok=True, minecraft='1.21', original_inner_sha256=SHA,
                  evidence_kind='Original network capture plus actual unchanged original STREAM replay',
                  native_comparison=False, native_recipe_serializer=False, vanilla_advancement_integration=False,
                  packet_id=119,
                  natural=dict(payload_sha256=natural['payload_sha256'], payload_bytes=natural['payload_bytes'],
                               decoded_count=natural['decoded_count'], remaining_bytes=natural['remaining_bytes'],
                               encode_roundtrip_exact=natural['roundtrip_equal'], ids_equal_default_catalogue=True,
                               special_flags_equal_actual_recipe_codec_facts=True),
                  controlled=dict(payload_sha256=controlled['payload_sha256'], payload_bytes=controlled['payload_bytes'],
                                  decoded_count=controlled['decoded_count'], remaining_bytes=controlled['remaining_bytes'],
                                  encode_roundtrip_exact=controlled['roundtrip_equal']),
                  comparison=dict(raw_payload_equal=natural['payload_sha256'] == controlled['payload_sha256'],
                                  recipe_order_equal=natural_order == controlled_order,
                                  equal_holder_payloads=1290 - len(different_holders),
                                  different_holder_payloads=len(different_holders), different_holder_ids=different_holders,
                                  interpretation='Per-holder payload difference demonstrates why the sorted-tag controlled catalogue must not be substituted for natural default declaration bytes; exact natural bytes remain a pinned dataset, not a native dynamic recipe serializer.'),
                  source_sha256={name:sha(ROOT / name) for name in sources})
    atomic_json(ROOT / 'evidence/server-recipe-book-declarations-stream.json', report)
    print(json.dumps({key:report[key] for key in ['ok', 'natural', 'controlled', 'comparison']}))


if __name__ == '__main__':
    main()
