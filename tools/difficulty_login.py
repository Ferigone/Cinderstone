#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-only
"""Focused native PLAY check with a synthetic level.dat, without Java gameplay."""
import argparse
import gzip
import hashlib
import json
import socket
import struct
import subprocess
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def named(kind, name, value):
    name = name.encode()
    return bytes([kind]) + struct.pack('>H', len(name)) + name + value


def level_document(locked):
    data = named(3, 'DataVersion', struct.pack('>i', 3953))
    data += named(1, 'Difficulty', bytes([2]))
    data += named(1, 'DifficultyLocked', bytes([locked]))
    data += named(4, 'Time', struct.pack('>q', 0))
    data += named(4, 'DayTime', struct.pack('>q', 0))
    data += b'\0'
    return named(10, '', named(10, 'Data', data) + b'\0')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--oracle', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    binary = args.binary.resolve()
    scratch = Path(tempfile.mkdtemp(prefix='difficulty-9d300416-'))
    oracle = json.loads(args.oracle.read_text())
    report = {'ok': False, 'source_commit': subprocess.check_output(
        ['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
        'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
        'java_jar_sha1': json.loads((ROOT / 'fixtures/source-lock.json').read_text())['oracle']['sha1'],
        'scene': 'Synthetic exact-version level.dat; no region chunks or imported gameplay',
        'oracle': oracle, 'cases': []}
    for name, difficulty, locked in [('imported-locked', 1, True),
                                     ('imported-unlocked', 3, False),
                                     ('native-configured', 3, False)]:
        run = scratch / name
        run.mkdir()
        (run / 'keys.json').write_text('{"playerCertificateKeys":[],"profilePropertyKeys":[]}')
        with socket.socket() as listener:
            listener.bind(('127.0.0.1', 0))
            port = listener.getsockname()[1]
        command = [str(binary), '--allow-offline', '--port', str(port), '--difficulty', str(difficulty),
                   '--fixtures', str(ROOT / 'fixtures/configuration.json'),
                   '--loopback-service-keys', str(run / 'keys.json'), '--server-directory', str(run)]
        if name.startswith('imported'):
            world = run / 'world'
            (world / 'region').mkdir(parents=True)
            (world / 'level.dat').write_bytes(gzip.compress(level_document(locked)))
            command += ['--anvil-world', str(world)]
        else:
            command += ['--world', str(run / 'world.json')]
        with (run / 'server.log').open('w') as log:
            process = subprocess.Popen(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT)
            try:
                deadline = time.monotonic() + 120
                while '"event":"listening"' not in (run / 'server.log').read_text():
                    if process.poll() is not None or time.monotonic() > deadline:
                        raise RuntimeError('Startup failed: ' + str(run / 'server.log'))
                    time.sleep(.1)
                probe = subprocess.run(['node', str(ROOT / 'tools/difficulty_probe.mjs'),
                    str(port), str(difficulty), str(locked).lower()], cwd=ROOT,
                    text=True, capture_output=True, timeout=40)
                if probe.returncode:
                    raise RuntimeError('Protocol probe failed: ' + probe.stderr)
                observation = json.loads(probe.stdout)
                expected = next(p for p in oracle['packets']
                    if p['difficulty'] == difficulty and p['locked'] == locked)
                assert observation['payloads'] == [expected['payload_hex']]
                report['cases'].append({'name': name, 'observation': observation,
                    'original_packet_equal': True})
            finally:
                if process.poll() is None:
                    process.terminate()
                process.wait(timeout=60)
                if process.returncode != 0:
                    raise RuntimeError('Unclean server shutdown: ' + str(run / 'server.log'))
    report['ok'] = True
    report['completed_at'] = datetime.now(timezone.utc).isoformat()
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'ok': True, 'cases': len(report['cases']), 'report': str(args.report)}))


if __name__ == '__main__':
    main()
