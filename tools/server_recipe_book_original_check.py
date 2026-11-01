#!/usr/bin/env python3
"""Replay shipped Java classes against captured recipe-book fixtures.

This checks the oracle and provenance only. It does not execute native Rust or
claim advancement/datapack integration. The native comparison has its own tool.
"""
import base64
import copy
from collections import Counter
import hashlib
import json
import subprocess

from server_recipe_book_generate import Original, ROOT, ORACLE, JDK, SHA, atomic_json


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def comparable(query, row):
    value = copy.deepcopy(row)
    # Literal ordered-packet inputs/decodes stay byte exact, including INIT.
    explicitly_ordered = query['name'].startswith(('ordered-packet/', 'decode/ordered-packet/'))
    for state in value['states']:
        if len(state['known']) > 1 or len(state['highlight']) > 1:
            state.pop('nbt_wire', None)
        for packet in state['packets']:
            if packet['state'] == 0 and not explicitly_ordered:
                if len(packet['recipes']) > 1 or len(packet['highlights']) > 1:
                    packet.pop('wire', None)
                packet['recipes'].sort()
                packet['highlights'].sort()
    return value


def first_difference(left, right, path='$'):
    if type(left) is not type(right):
        return dict(path=path, expected_type=type(left).__name__, actual_type=type(right).__name__)
    if isinstance(left, dict):
        if left.keys() != right.keys():
            return dict(path=path, expected_keys=list(left), actual_keys=list(right))
        for key in left:
            result = first_difference(left[key], right[key], path + '.' + key)
            if result:
                return result
    elif isinstance(left, list):
        if len(left) != len(right):
            return dict(path=path, expected_length=len(left), actual_length=len(right))
        for index, (old, new) in enumerate(zip(left, right)):
            result = first_difference(old, new, path + '[' + str(index) + ']')
            if result:
                return result
    elif left != right:
        return dict(path=path, expected=str(left)[:180], actual=str(right)[:180])
    return None


def replay(oracle, fixture):
    inputs = [row['query'] for row in fixture['cases']]
    actual = oracle.run(inputs)
    failures = []
    states = packets = exact_packet_wires = exact_nbt_wires = 0
    world_rng_unchanged = True
    exceptions = Counter()
    callbacks = Counter()
    for case, row in zip(fixture['cases'], actual):
        expected = comparable(case['query'], case['expected'])
        observed = comparable(case['query'], row)
        difference = first_difference(expected, observed)
        if difference:
            failures.append(dict(name=case['query']['name'], difference=difference))
        for state in observed['states']:
            states += 1
            packets += len(state['packets'])
            exact_nbt_wires += 'nbt_wire' in state
            exact_packet_wires += sum('wire' in packet for packet in state['packets'])
            if state['error']:
                exceptions[state['error']['class']] += 1
            callbacks.update(event['callback'] for event in state['events'])
            if state['world_rng_before'] != state['world_rng_after']:
                world_rng_unchanged = False
                failures.append(dict(name=case['query']['name'], difference=dict(path='world_rng', expected='unchanged', actual='changed')))
    log = (oracle.runtime / 'transitions.log').read_text()
    sources = [line.removeprefix('SOURCE:') for line in log.splitlines() if line.startswith('SOURCE:')]
    expected_source = (ORACLE / 'server-inner-1.21.jar').as_uri()
    # Java URL spells local paths with file:/, pathlib with file:///.
    if len(sources) != 2 or any(source.removeprefix('file:').lstrip('/') != expected_source.removeprefix('file:').lstrip('/') for source in sources):
        failures.append(dict(name='original-class-provenance', expected=expected_source, actual=sources))
    return dict(ok=not failures, histories=len(inputs), operation_snapshots=states, packet_snapshots=packets,
                packet_wire_exact_snapshots=exact_packet_wires, nbt_wire_exact_snapshots=exact_nbt_wires,
                expected_exceptions=dict(exceptions), callback_observations=dict(callbacks),
                world_rng_unchanged=world_rng_unchanged, failures=failures[:10])


def main():
    oracle = Original()
    fixture_paths = ['fixtures/server-recipe-book-facts.json', 'fixtures/server-recipe-book-vectors.json',
                     'fixtures/server-recipe-book-codec-edges.json', 'fixtures/server-recipe-book-controlled-declarations.json']
    facts = json.loads((ROOT / fixture_paths[0]).read_text())
    current_facts = oracle.run(mode='facts')
    current_facts['tag_binding_source_sha256'] = digest(ROOT / 'fixtures/crafting-1.21.json')
    fact_difference = first_difference(facts, current_facts)
    controlled = json.loads((ROOT / fixture_paths[3]).read_text())
    current_controlled = oracle.run(mode='declarations')
    current_controlled['payload_sha256'] = hashlib.sha256(base64.b64decode(current_controlled['payload'])).hexdigest()
    current_controlled['tag_binding_source_sha256'] = current_facts['tag_binding_source_sha256']
    declaration_difference = first_difference(controlled, current_controlled)
    transitions = replay(oracle, json.loads((ROOT / fixture_paths[1]).read_text()))
    edges = replay(oracle, json.loads((ROOT / fixture_paths[2]).read_text()))
    sources = ['tools/server_recipe_book_oracle.java', 'tools/server_recipe_book_generate.py',
               'tools/server_recipe_book_codec_generate.py', 'tools/server_recipe_book_original_check.py',
               'fixtures/crafting-1.21.json', *fixture_paths]
    bytecode_paths = sorted(ROOT.glob('tools/server_recipe_book_*_bytecode.txt'))
    report = dict(ok=fact_difference is None and declaration_difference is None and transitions['ok'] and edges['ok'],
                  minecraft='1.21', original_inner_sha256=SHA,
                  evidence_kind='Executable unchanged-original-class replay; no native comparison in this report',
                  native_comparison=False, default_vanilla_advancement_integration=False,
                  jdk_version=subprocess.run([JDK / 'java', '-version'], text=True, capture_output=True).stderr.strip(),
                  facts=dict(ok=fact_difference is None, recipes=len(facts['recipes']), special_recipes=sum(r['is_special'] for r in facts['recipes']),
                             clientbound_recipe_packet_id=facts['packet_id'], difference=fact_difference),
                  transitions=transitions, codec_edges=edges,
                  controlled_declarations=dict(ok=declaration_difference is None, packet_id=controlled['packet_id'],
                                               recipe_count=controlled['decoded_count'], payload_bytes=len(base64.b64decode(controlled['payload'])),
                                               payload_sha256=controlled['payload_sha256'], difference=declaration_difference,
                                               natural_vanilla_order=False),
                  ordering_policy=dict(known_and_highlight='Logical sets',
                                       add_remove_and_explicit_ordered_packet='Literal ordered IDs and exact original STREAM bytes',
                                       server_initial='Logical sets; exact wire retained only when each set has at most one ID',
                                       saved_nbt='Logical recipe/highlight sets; exact named-root NBT retained only when each set has at most one ID; fixed top-level key order checked'),
                  environment_scope=dict(original_methods=['avu', 'avs', 'avt', 'aee', 'dt', 'dv', 'Recipe.CODEC', 'Recipe.isSpecial', 'agc.STREAM_CODEC'],
                                         observed_boundaries=['Network send', 'ServerPlayer environment getters', 'PlayerAdvancements.award'],
                                         forwarding_observer='Controlled dt subclass observes the original predicate capture and invokes unchanged dt/dv protected body; an.g points to it in this process only',
                                         excluded=['Default advancement registration/rewards/persistence', 'Dynamic datapack reload', 'Natural TagLoader declaration ordering', 'Production Java dependency'],
                                         self_copy='One explicit Java-only adversarial alias case; unsafe partial-map aliasing is not a native API requirement'),
                  source_sha256={name:digest(ROOT / name) for name in sources},
                  bytecode_dump_sha256={str(path.relative_to(ROOT)):digest(path) for path in bytecode_paths})
    atomic_json(ROOT / 'evidence/server-recipe-book-original-oracle.json', report)
    print(json.dumps({key:report[key] for key in ['ok', 'facts', 'transitions', 'codec_edges', 'controlled_declarations']}))
    if not report['ok']:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
