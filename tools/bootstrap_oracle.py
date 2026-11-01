#!/usr/bin/env python3
"""Fetch the exact official Java 1.21 oracle and verify every downloaded byte."""
import argparse
import hashlib
import json
import urllib.request
import zipfile
from pathlib import Path

PROJECT = Path(__file__).resolve().parents[1]


def download_verified(asset, destination):
    destination = Path(destination)
    destination.parent.mkdir(parents=True, exist_ok=True)
    if destination.exists():
        data = destination.read_bytes()
        if len(data) == asset['size'] and hashlib.sha1(data).hexdigest() == asset['sha1']:
            return destination
    temporary = destination.with_suffix(destination.suffix + '.partial')
    with urllib.request.urlopen(asset['url'], timeout=90) as source, temporary.open('wb') as target:
        while block := source.read(1024 * 1024):
            target.write(block)
    data = temporary.read_bytes()
    if len(data) != asset['size'] or hashlib.sha1(data).hexdigest() != asset['sha1']:
        raise ValueError('Official artifact checksum or length mismatch: ' + destination.name)
    temporary.replace(destination)
    return destination


def bootstrap(directory):
    directory = Path(directory).resolve()
    lock = json.loads((PROJECT / 'fixtures/source-lock.json').read_text())
    jar = download_verified(lock['oracle'], directory / 'server-1.21.jar')
    download_verified(lock['mappings'], directory / 'server-1.21-mappings.txt')
    with zipfile.ZipFile(jar) as bundle:
        checksum, version, member = bundle.read('META-INF/versions.list').decode().strip().split('\t')
        if version != '1.21':
            raise ValueError('Unexpected bundled server version')
        data = bundle.read('META-INF/versions/' + member)
        if hashlib.sha256(data).hexdigest() != checksum:
            raise ValueError('Inner server digest mismatch')
        (directory / 'server-inner-1.21.jar').write_bytes(data)
        for entry in bundle.infolist():
            if entry.filename.startswith('META-INF/libraries/') and entry.filename.endswith('.jar'):
                relative = Path(entry.filename).relative_to('META-INF/libraries')
                if relative.is_absolute() or '..' in relative.parts:
                    raise ValueError('Invalid bundled library path')
                out = directory / 'libraries' / relative
                out.parent.mkdir(parents=True, exist_ok=True)
                out.write_bytes(bundle.read(entry))
    return directory


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--oracle-dir', type=Path, default=PROJECT / 'runtime/oracle')
    args = parser.parse_args()
    print(json.dumps({'oracle_dir': str(bootstrap(args.oracle_dir)), 'version': '1.21', 'protocol': 767}))
