#!/usr/bin/env python3
"""Check the proved ItemStack STREAM normalization scope and report codec gaps."""
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
    parser.add_argument('--skip-generate', action='store_true', help='use pinned original Java STREAM corpus')
    parser.add_argument('--release', action='store_true', help='also verify optimized native tests')
    args = parser.parse_args()
    if not args.skip_generate:
        subprocess.run([sys.executable, ROOT/'tools/inventory_normalization_generate.py'], cwd=ROOT, check=True)
        # A preexisting unit regression has duplicate additions followed by two
        # absent removals. Keep its expected wire from the unchanged codec too.
        oracle = ROOT.parent/'mc121-oracle'
        classes = ROOT/'runtime/inventory-normalization/classes'
        cp = os.pathsep.join(map(str, [classes, oracle/'server-inner-1.21.jar',
                                      *sorted((oracle/'libraries').rglob('*.jar'))]))
        query = dict(name='legacy/duplicates-and-absent-removals', bytes='AQEDAgMAAwEBAgMF')
        java = subprocess.run([
            '/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin/java', '-cp', cp,
            'oracle.inventory_normalization_oracle', ROOT/'tools/crafting_metadata.json',
        ], cwd=ROOT, input=json.dumps(query)+'\n', text=True, capture_output=True, check=True)
        results = [json.loads(line.split('ORACLE:', 1)[1])
                   for line in java.stdout.splitlines() if line.startswith('ORACLE:')]
        assert len(results) == 1
        regression = dict(minecraft='1.21',
                          oracle_inner_sha256=sha(oracle/'server-inner-1.21.jar'),
                          codec='original cuq.h OPTIONAL_STREAM_CODEC decode/encode; no ItemStack.CODEC',
                          cases=[dict(query=query, expected=results[0])])
        (ROOT/'fixtures/inventory_normalization_regressions.json').write_text(
            json.dumps(regression, indent=2)+'\n')
    fixture = json.loads((ROOT/'fixtures/inventory_normalization_vectors.json').read_text())
    policy = json.loads((ROOT/'fixtures/inventory_normalization_policy.json').read_text())
    assert policy['vectors_sha256'] == sha(ROOT/'fixtures/inventory_normalization_vectors.json')
    assert policy['source_fixture_sha256'] == sha(ROOT/'fixtures/item-components-1.21.json')
    assert policy['oracle_source_sha256'] == sha(ROOT/'tools/inventory_normalization_oracle.java')
    env = os.environ.copy()
    env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',
               RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup', CARGO_INCREMENTAL='0')
    env['PATH'] = env['CARGO_HOME']+'/bin:'+env.get('PATH', '')
    runtime = ROOT/'runtime/inventory-normalization'
    runtime.mkdir(parents=True, exist_ok=True)
    tests = {}
    comparisons = None
    for profile in (['debug', 'release'] if args.release else ['debug']):
        command = [str(Path(env['CARGO_HOME'])/'bin/cargo'), 'test']
        if profile == 'release':
            command.append('--release')
        command += ['--lib', 'inventory::tests', '--', '--nocapture']
        result = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True)
        output = result.stdout+result.stderr
        (runtime/('rust-'+profile+'.log')).write_text(output)
        if result.returncode:
            raise RuntimeError(output[-10000:])
        required = 'test inventory::tests::actual_java_stream_prototype_and_removal_normalization ... ok'
        if required not in result.stdout:
            raise RuntimeError('Required normalization test did not execute\n'+output[-10000:])
        match = re.search(r'test result: ok\. (\d+) passed', result.stdout)
        report = re.search(r'^INVENTORY_NORMALIZATION_REPORT (.+)$', result.stdout, re.M)
        if not match or not report:
            raise RuntimeError('Missing native comparison report\n'+output[-10000:])
        comparison = json.loads(report.group(1))
        if comparisons is not None and comparison != comparisons:
            raise RuntimeError('Debug/release comparison results differ')
        comparisons = comparison
        tests[profile] = dict(passed=int(match.group(1)), failed=0)
    assert comparisons['strict_cases'] == 8840
    assert len(policy['removable_prototypes']) == 8310
    assert len(policy['retained_prototypes']) == 4
    report = dict(
        ok=True, full_stream_compatibility=False, minecraft='1.21',
        oracle_inner_sha256=fixture['oracle_inner_sha256'],
        codec=fixture['codec'], total_original_cases=len(fixture['cases']),
        strict_cases=comparisons['strict_cases'], strict_mismatches=0,
        all_cases_matched=sum(group['matches'] for group in comparisons['groups'].values()),
        all_cases_mismatched=len(comparisons['gaps']), legacy_regressions=1,
        removable_prototype_pairs=len(policy['removable_prototypes']),
        identity_sensitive_retained_prototype_pairs=policy['retained_prototypes'],
        groups=comparisons['groups'], tests=tests,
        scope='Exact original STREAM decode consumption and canonical encode for every 8,314 '
              'nonair prototype override, 513 component removals, 12 patch-order cases and '
              'the ender-pearl regression. Elision uses only original Java equality evidence '
              'for identical prototype payloads. Four FoodProperties overrides with nested '
              'ItemStack reference identity are retained. Added/removed duplicates keep '
              'first-entry ordering and last-value semantics; absent prototype removals elide.',
        known_original_stream_mismatches=comparisons['gaps'],
        not_verified=[
            'Prototype equality for semantically equal but nonidentical wire payloads',
            'General typed component constructor validation and canonicalization; concrete '
            'differences from all 117 independent vectors, nested Slots, NBT and text are '
            'reported explicitly below rather than counted as passing',
            'Java identity-sensitive equality of independently allocated arbitrary component values',
            'Arbitrary CompoundTag HashMap output ordering',
        ],
        source_sha256={name: sha(ROOT/name) for name in [
            'src/inventory.rs', 'src/protocol.rs', 'src/nbt.rs',
            'tools/inventory_normalization_oracle.java',
            'tools/inventory_normalization_generate.py',
            'tools/inventory_normalization_check.py',
            'fixtures/inventory_normalization_vectors.json',
            'fixtures/inventory_normalization_policy.json',
            'fixtures/inventory_normalization_regressions.json',
            'tools/inventory_vectors.json',
        ]},
        reproduce='python3 tools/inventory_normalization_check.py --release',
    )
    (ROOT/'evidence/inventory-normalization-prototype-replay.json').write_text(json.dumps(report, indent=2)+'\n')
    print(json.dumps({key: value for key, value in report.items()
                      if key not in ('known_original_stream_mismatches', 'source_sha256')}, indent=2))
    print('Known original STREAM mismatches:', len(comparisons['gaps']))


if __name__ == '__main__':
    main()
