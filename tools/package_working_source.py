#!/usr/bin/env python3
"""Save current development sources without stale binaries or runtime data."""
import argparse
import hashlib
import json
import subprocess
import zipfile
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EXCLUDED_DIRS = {'.git', 'target', 'runtime', 'node_modules', '__pycache__', 'logs', 'bin'}
EXCLUDED_SUFFIXES = {'.jar', '.class', '.pyc', '.zip', '.png', '.jpg', '.jpeg', '.mp4', '.log'}


def package(output: Path) -> dict:
    listing = subprocess.check_output(
        ['git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z'], cwd=ROOT
    ).decode().split('\0')
    entries = {}
    for name in sorted(set(filter(None, listing))):
        path = Path(name)
        if set(path.parts) & EXCLUDED_DIRS or path.suffix.lower() in EXCLUDED_SUFFIXES:
            continue
        if name in {'checkpoint.json', 'working-source.json', 'WORKING_SOURCE_NOTICE.txt'}:
            continue
        source = ROOT / path
        if source.is_symlink() or not source.is_file():
            raise ValueError(f'Not a regular source file: {name}')
        entries[name] = source.read_bytes()
    manifest = {
        'format': 'mc121-working-source-v2',
        'created_at': datetime.now(timezone.utc).isoformat(),
        'minecraft': '1.21',
        'protocol': 767,
        'full_vanilla_parity': False,
        'release_verified': False,
        'source_only': True,
        'git_base_commit': subprocess.check_output(
            ['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True
        ).strip(),
        'note': 'Development snapshot. Historical evidence is not validation of this snapshot. '
                'Full validation is deferred; no matching executable is included.',
        'files': {name: hashlib.sha256(data).hexdigest() for name, data in entries.items()},
    }
    smoke = ROOT / 'evidence/phase6-integration-live-smoke.json'
    if smoke.is_file():
        manifest['limited_runtime_check'] = json.loads(smoke.read_text())
        manifest['limited_runtime_check_matches_current_source'] = False
    manifest['known_limits'] = [
        'Full vanilla behavior is not implemented; see docs/CURRENT_WORK.md.',
        'Final comprehensive validation is deferred.',
    ]
    entries['working-source.json'] = (json.dumps(manifest, indent=2) + '\n').encode()
    entries['WORKING_SOURCE_NOTICE.txt'] = (
        'NATIVE MINECRAFT 1.21 RUST PORT — DEVELOPMENT SOURCES\n'
        'Full vanilla parity: NOT COMPLETE. Release validation: NOT COMPLETE.\n'
        'This archive has no server executable. Build the sources with Cargo.\n'
        'Historical evidence describes its original source/binary, not this snapshot.\n'
        'See mc121-rust-progress.json for the delivery checklist, not gameplay coverage.\n'
    ).encode()
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_suffix('.zip.partial')
    with zipfile.ZipFile(temporary, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=6) as archive:
        for name, data in entries.items():
            archive.writestr('mc121-rust/' + name, data)
    temporary.replace(output)
    return {'file': str(output.resolve()), 'bytes': output.stat().st_size,
            'files': len(entries), 'sha256': hashlib.sha256(output.read_bytes()).hexdigest()}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=ROOT.parent / 'mc121-rust-working-source.zip')
    print(json.dumps(package(parser.parse_args().output)))
