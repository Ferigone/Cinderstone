#!/usr/bin/env python3
"""Check native heightmap updates against unchanged Java oracle vectors.

--world additionally compares five actual Java/Rust chunk packets after repeated
native column edits, using unique disposable copies of the oracle world.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
ORACLE = ROOT.parent / 'mc121-oracle'


def run(command, **kwargs):
    result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, **kwargs)
    if result.returncode:
        raise RuntimeError(f'{command[0]} failed:\n{result.stdout[-6000:]}\n{result.stderr[-6000:]}')
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--world', action='store_true')
    args = parser.parse_args()
    env = os.environ.copy()
    env['CARGO_INCREMENTAL'] = '0'
    toolchain = Path('/root/.local/share/mc-rust-toolchain')
    if toolchain.exists():
        env.setdefault('CARGO_HOME', str(toolchain / 'cargo'))
        env.setdefault('RUSTUP_HOME', str(toolchain / 'rustup'))
        env['PATH'] = env['CARGO_HOME'] + '/bin:' + env['PATH']
    evidence = ROOT / 'evidence'
    evidence.mkdir(exist_ok=True)
    tests = run(['cargo', 'test', '--lib', 'storage::tests', '--', '--nocapture'], env=env)
    (evidence / 'heightmap-storage-tests.log').write_text(tests.stdout + tests.stderr)
    result = json.loads(next(line.removeprefix('HEIGHTMAP_CONFORMANCE:')
                             for line in tests.stdout.splitlines()
                             if line.startswith('HEIGHTMAP_CONFORMANCE:')))
    assert '0 failed' in tests.stdout and 'FAILED' not in tests.stdout
    result.update(oracle_inner_sha256='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01',
                  source_sha256=hashlib.sha256((ROOT / 'src/storage.rs').read_bytes()).hexdigest(),
                  fixture_sha256=hashlib.sha256((ROOT / 'fixtures/heightmap-update-vectors.json').read_bytes()).hexdigest(),
                  imported_map_validation='full prime before first edit; malformed cached maps re-prime; never use imported stale maps as an incremental baseline')
    if args.world:
        original_files = [ORACLE / 'world/level.dat', *sorted((ORACLE / 'world/region').glob('*.mca'))]
        original_hashes = {str(file): hashlib.sha256(file.read_bytes()).hexdigest() for file in original_files}
        runtime = ROOT / 'runtime'
        runtime.mkdir(exist_ok=True)
        work = Path(tempfile.mkdtemp(prefix='heightmap-world-', dir=runtime))
        native_world = work / 'native-world'
        java_world = work / 'java-run/world'
        for world in [native_world, java_world]:
            world.mkdir(parents=True)
            shutil.copyfile(ORACLE / 'world/level.dat', world / 'level.dat')
            shutil.copytree(ORACLE / 'world/region', world / 'region', copy_function=shutil.copyfile)
        build = run(['cargo', 'build', '--lib', '--bin', 'storage_probe', '--message-format=json'], env=env)
        artifacts = [json.loads(line) for line in build.stdout.splitlines() if line.startswith('{')]
        library = next(Path(name) for row in artifacts if row.get('reason') == 'compiler-artifact'
                       and row['target']['name'] == 'mc121_rust'
                       for name in row['filenames'] if name.endswith('.rlib'))
        probe = work / 'heightmap_world_probe'
        run(['rustc', '--edition=2021', str(ROOT / 'tools/heightmap_world_probe.rs'),
             '--extern', 'mc121_rust=' + str(library),
             '-L', 'dependency=' + str(ROOT / 'target/debug/deps'),
             '-L', 'dependency=' + str(library.parent), '-o', str(probe)], env=env)
        edits = json.loads(run([str(probe), str(native_world)], env=env).stdout)
        run([str(ROOT / 'target/debug/storage_probe'), 'relight', str(native_world), str(work)], env=env)
        run(['python3', 'tools/storage_capture.py', str(work), '--edit'], env=env)
        comparison = run(['node', 'tools/storage_compare.mjs', str(work)], env=env)
        world_result = json.loads(comparison.stdout.strip().splitlines()[-1])
        assert world_result['actual_java_chunk_comparisons'] == 5
        assert world_result['all_heightmaps_equal']
        result['world_packet_comparison'] = world_result
        result['world_saved_heightmaps_comparison'] = json.loads(
            run(['node', 'tools/heightmap_compare.mjs', str(work)], env=env).stdout)
        result['native_edit_sequence'] = edits
        result['original_world_unchanged'] = all(hashlib.sha256(Path(file).read_bytes()).hexdigest() == digest
                                                 for file, digest in original_hashes.items())
        assert result['original_world_unchanged']
    (evidence / 'heightmap-conformance.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result))


if __name__ == '__main__':
    main()
