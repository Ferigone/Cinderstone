#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-only
"""Compare initial shared-spawn packets from real Java 1.21 and native Main.

Use a disposable Java-generated 1.21 flat fixture as --source-world and an
existing --eula-file. Only copies are modified. No saves or Java artifacts are
included in the sanitized report. The native default is checked separately.
"""
import argparse
import json
import shutil
import struct
import subprocess
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent / 'parity'))
from run import Server, digest

ROOT = Path(__file__).resolve().parents[1]
SPAWN = {'x': 13, 'y': 70, 'z': -19}
ANGLE = 37.5


def packet_hex(position, angle):
    packed = ((position['x'] & 0x3ffffff) << 38 |
              (position['z'] & 0x3ffffff) << 12 | position['y'] & 0xfff)
    return (b'\x56' + struct.pack('>Qf', packed, angle)).hex()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--native-source-commit', required=True,
                        help='Exact source commit used to build --binary')
    parser.add_argument('--source-world', type=Path, required=True)
    parser.add_argument('--eula-file', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    jar, binary = args.jar.resolve(), args.binary.resolve()
    lock = json.loads((ROOT / 'fixtures/source-lock.json').read_text())['oracle']
    if jar.stat().st_size != lock['size'] or digest(jar, 'sha1') != lock['sha1']:
        parser.error('Use the unchanged pinned Java 1.21 JAR')
    if 'eula=true' not in args.eula_file.read_text().splitlines():
        parser.error('Supply an existing eula=true acceptance')
    scratch = Path(tempfile.mkdtemp(prefix='cinderstone-spawn-'))
    report = {'schema_version': 1, 'ok': False, 'completed_at': None,
              'native_commit': subprocess.check_output(['git', 'rev-parse', args.native_source_commit + '^{commit}'], cwd=ROOT, text=True).strip(),
              'native_binary_sha256': digest(binary), 'java_jar_sha1': lock['sha1'],
              'scope': 'Initial default-spawn packet only; no player spawn search or imported gameplay.',
              'traces': {}, 'error': None}
    server = None
    java_active = False
    try:
        java_dir = scratch / 'java'; java_dir.mkdir()
        shutil.copytree(args.source_world, java_dir / 'world')
        shutil.copyfile(args.eula_file, java_dir / 'eula.txt')
        (java_dir / 'server.properties').write_text(
            'server-ip=127.0.0.1\nserver-port=25621\nonline-mode=false\n'
            'enforce-secure-profile=false\nview-distance=2\nsimulation-distance=2\n')
        server = Server(['java', '-Xms128M', '-Xmx1536M', '-jar', str(jar), 'nogui'],
                        java_dir, scratch / 'java.log', 'Done (')
        java_active = True
        server.command('setworldspawn 13 70 -19 37.5')
        server.command('say SPAWN_PROBE_READY')
        server.wait('SPAWN_PROBE_READY')

        def probe(name, port):
            output = scratch / (name + '.json')
            subprocess.run(['node', str(ROOT / 'tools/spawn_login_probe.mjs'), str(port), str(output)],
                           cwd=ROOT, check=True, timeout=90)
            trace = json.loads(output.read_text())
            report['traces'][name] = trace
            assert trace['ok'], trace['error']
            return trace['packets'][0]

        original = probe('java', 25621)
        assert original == {'position': SPAWN, 'angle': ANGLE, 'frame_body_hex': packet_hex(SPAWN, ANGLE)}
        server.stop(java=True); server = None; java_active = False
        imported = scratch / 'imported'; imported.mkdir()
        shutil.copytree(java_dir / 'world', imported / 'world')
        (scratch / 'keys.json').write_text('{"profilePropertyKeys":[],"playerCertificateKeys":[]}')
        for name, world_args, expected in [
            ('native_imported', ['--anvil-world', str(imported / 'world')], original),
            ('native_flat', ['--world', str(scratch / 'native/world.json')],
             {'position': {'x': 8, 'y': 64, 'z': 8}, 'angle': 0,
              'frame_body_hex': packet_hex({'x': 8, 'y': 64, 'z': 8}, 0)}),
        ]:
            server = Server([str(binary), '--allow-offline', '--bind', '127.0.0.1', '--port', '25622',
                             '--fixtures', str(ROOT / 'fixtures/configuration.json'),
                             '--enforce-secure-profile', 'false', '--loopback-service-keys', str(scratch / 'keys.json'),
                             *world_args], ROOT, scratch / (name + '.log'), '"event":"listening"')
            assert probe(name, 25622) == expected, name + ' spawn differs'
            server.stop(); server = None
        report['ok'] = True
    except Exception as error:
        report['error'] = str(error)
    finally:
        if server is not None:
            try:
                server.stop(java=java_active)
            except Exception as error:
                report['ok'] = False
                report['shutdown_error'] = str(error)
        report['completed_at'] = datetime.now(timezone.utc).isoformat()
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'ok': report['ok'], 'error': report['error'], 'scratch': str(scratch)}))
    return 0 if report['ok'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
