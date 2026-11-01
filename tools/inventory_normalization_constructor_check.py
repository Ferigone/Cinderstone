#!/usr/bin/env python3
"""Verify typed Slot constructor normalization against original Minecraft1.21."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def sha(name):
    return hashlib.sha256((ROOT/name).read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--regenerate', action='store_true', help='regenerate separate constructor/map/scalar oracles; preserves the original prototype checkpoint')
    parser.add_argument('--release', action='store_true', help='also verify optimized native tests')
    args = parser.parse_args()
    if args.regenerate:
        for generator in ['constructor', 'map', 'scalar']:
            subprocess.run([sys.executable, ROOT/f'tools/inventory_normalization_{generator}_generate.py'], cwd=ROOT, check=True)
    fixtures = {name: json.loads((ROOT/f'fixtures/inventory_normalization_{name}_vectors.json').read_text())
                for name in ['constructor', 'map', 'scalar']}
    original = json.loads((ROOT/'fixtures/inventory_normalization_vectors.json').read_text())
    map_proof = fixtures['map']
    assert map_proof['constructor_fixture_sha256'] == sha('fixtures/inventory_normalization_constructor_vectors.json')
    assert len(original['cases']) == 9145
    assert len(fixtures['constructor']['cases']) == 452
    assert len(fixtures['scalar']['cases']) == 46
    map_cases = [case for run in map_proof['runs'] for case in run['cases']]
    assert len(map_cases) == 21
    for case in map_cases:
        for equality in ['trim_equals', 'material_equals', 'map_equals']:
            assert case['expected'][equality] is True
        for roundtrip in ['input_roundtrip', 'reference_roundtrip']:
            assert case['expected'][roundtrip]['accepted'] is True
            assert case['expected'][roundtrip]['remaining'] == 0
    env = os.environ.copy()
    env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',
               RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup', CARGO_INCREMENTAL='0')
    env['PATH'] = env['CARGO_HOME']+'/bin:'+env.get('PATH', '')
    runtime = ROOT/'runtime/inventory-normalization-constructor'
    runtime.mkdir(parents=True, exist_ok=True)
    tests = {}
    comparison = None
    for profile in (['debug', 'release'] if args.release else ['debug']):
        command = [str(Path(env['CARGO_HOME'])/'bin/cargo'), 'test']
        if profile == 'release':
            command.append('--release')
        command += ['--lib', 'inventory::tests', '--', '--nocapture']
        result = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True)
        output = result.stdout+result.stderr
        (runtime/('rust-'+profile+'.log')).write_text(output)
        if result.returncode:
            raise RuntimeError(output[-12000:])
        for required in [
            'actual_java_stream_prototype_and_removal_normalization',
            'actual_java_stream_component_constructors_and_nested_canonical_wire',
            'actual_java_stream_scalar_semantic_prototype_equality',
            'actual_java_trim_map_identity_hash_changes_wire_but_preserves_value',
            'independent_node_vectors_match_original_java_acceptance_and_canonical_wire',
        ]:
            if f'test inventory::tests::{required} ... ok' not in result.stdout:
                raise RuntimeError('Required test did not execute: '+required)
        reports = {}
        for name, marker in [('original', 'INVENTORY_NORMALIZATION_REPORT'),
                             ('constructors', 'INVENTORY_CONSTRUCTOR_REPORT')]:
            match = re.search('^'+marker+r' (.+)$', result.stdout, re.M)
            if not match:
                raise RuntimeError('Missing native report '+marker)
            reports[name] = json.loads(match.group(1))
        if comparison is not None and comparison != reports:
            raise RuntimeError('Debug/release comparisons differ')
        comparison = reports
        count = re.search(r'test result: ok\. (\d+) passed', result.stdout)
        if not count:
            raise RuntimeError('Missing native test count')
        tests[profile] = dict(passed=int(count.group(1)), failed=0)
    assert not comparison['original']['gaps']
    constructors = comparison['constructors']
    assert constructors['semantic_map_matches'] == 7
    assert len(constructors['gaps']) == 7
    strict_constructor_matches = sum(group['matches'] for group in constructors['groups'].values())
    assert strict_constructor_matches == 445
    report = dict(
        ok=True, full_stream_compatibility=False, minecraft='1.21',
        oracle_inner_sha256=original['oracle_inner_sha256'],
        original_corpus=dict(cases=9145, mismatches=0, groups=comparison['original']['groups']),
        constructor_corpus=dict(cases=452, deterministic_wire_matches=445,
                                semantic_trim_map_matches=7, behavior_mismatches=0,
                                groups=constructors['groups']),
        scalar_semantic_prototype_cases=46, scalar_mismatches=0,
        original_trim_map_semantic_pairs=21, original_trim_map_mismatches=0,
        tests=tests,
        scope='Original STREAM acceptance, accepted-prefix consumption and canonical bytes '
              'for all 9,145 prior vectors. Adds 445 exact constructor vectors and seven '
              'inline trim maps with proved equal original cwy/material/map values. '
              'The original fastutil map keys use JVM Object identity hashes: original '
              'unmodified Java emits multiple valid orders for the same key/value map. '
              'Those seven literal byte differences remain listed; native wire is not '
              'sorted to imitate one JVM. Includes 46 boxed primitive equality probes '
              'across overlong VarInts/nonzero Boolean bytes, nested Slot normalization, '
              'effect amplifier clamping and encoder-copy hidden-effect removal, Unit19, '
              'instrument/trim 1.21 schemas, four pot positions/defaulted item lookup, '
              'NBT raw NaN and zero normalization, and lossless Java UTF16 literals.',
        literal_jvm_identity_map_wire_differences=constructors['gaps'],
        not_verified=[
            'All possible nondefault values of all 57 component types: rich text styles, '
            'filtered book pages and remaining constructor-specific validation are not '
            'established by this bounded corpus',
            'Semantic prototype equality for nonidentical complex component representations '
            'beyond the proved boxed primitive and identical-wire prototype policies',
            'Arbitrary identity-sensitive component object graphs in same-item comparisons',
            'General CompoundTag HashMap iteration order; seven trim map orders are '
            'explicitly JVM-dependent rather than deterministic canonical wire',
            'Exception classes, error text and exact failing-prefix consumption for rejected input',
            'Custom datapack registries/tags beyond the pinned captured defaults',
            'Unchecked native public struct payloads retain legacy encode fallback; '
            'encode_canonical is the fallible typed encoder',
        ],
        source_sha256={name: sha(name) for name in [
            'src/inventory.rs', 'src/protocol.rs', 'src/metadata.rs', 'src/nbt.rs',
            'fixtures/items-1.21.json', 'fixtures/item-components-1.21.json',
            'fixtures/placement-block-tags-1.21.json', 'tools/crafting_metadata.json',
            'fixtures/inventory_normalization_vectors.json',
            'fixtures/inventory_normalization_policy.json',
            *[f'fixtures/inventory_normalization_{kind}_vectors.json'
              for kind in ['constructor', 'map', 'scalar']],
            'tools/inventory_normalization_oracle.java',
            'tools/inventory_normalization_map_oracle.java',
            *[f'tools/inventory_normalization_{kind}_generate.py'
              for kind in ['constructor', 'map', 'scalar']],
            'tools/inventory_normalization_constructor_check.py',
        ]},
        reproduce='python3 tools/inventory_normalization_constructor_check.py --release',
        regenerate_original='python3 tools/inventory_normalization_constructor_check.py --regenerate --release',
        preserved_checkpoint='evidence/inventory-normalization-conformance.json',
    )
    (ROOT/'evidence/inventory-normalization-constructor-conformance.json').write_text(json.dumps(report, indent=2)+'\n')
    print(json.dumps({key: value for key, value in report.items()
                     if key not in ['literal_jvm_identity_map_wire_differences', 'source_sha256']}, indent=2))


if __name__ == '__main__':
    main()
