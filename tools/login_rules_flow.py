#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-only
"""Compare PLAY UI flags on real Java 1.21 and native imported/fresh worlds."""
import argparse
from datetime import datetime, timezone
import json
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).parent / 'parity'))
from run import ROOT, Server, digest


def free_port():
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        return sock.getsockname()[1]


def capture(port, directory):
    output = directory / 'login.json'
    subprocess.run(['node', str(Path(__file__).with_suffix('.mjs')), str(port), str(output)],
                   cwd=ROOT, check=True, timeout=45)
    result = json.loads(output.read_text())
    assert result['ok'] and result['position_received'], result
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--eula-file', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    jar, binary = args.jar.resolve(), args.binary.resolve()
    lock = json.loads((ROOT / 'fixtures/source-lock.json').read_text())['oracle']
    assert jar.stat().st_size == lock['size'] and digest(jar, 'sha1') == lock['sha1']
    assert 'eula=true' in args.eula_file.read_text().splitlines(), 'Supply existing EULA acceptance'
    scratch = Path(tempfile.mkdtemp(prefix='cinderstone-login-rules-'))
    report = {'ok': False, 'java_jar_sha1': lock['sha1'], 'minecraft': '1.21', 'protocol': 767,
              'source_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
              'main_source_sha256': digest(ROOT / 'src/main.rs'), 'binary_sha256': digest(binary),
              'runner_sha256': digest(Path(__file__)), 'client_sha256': digest(Path(__file__).with_suffix('.mjs')),
              'cases': [], 'scope': 'PLAY login UI flags and teleport acknowledgment; no death/respawn gameplay'}
    java_server = native = None
    try:
        java_dir = scratch / 'java'; java_dir.mkdir()
        shutil.copyfile(args.eula_file, java_dir / 'eula.txt')
        port = free_port()
        properties = {'server-ip': '127.0.0.1', 'server-port': str(port), 'online-mode': 'false',
                      'enforce-secure-profile': 'false', 'gamemode': 'creative', 'level-seed': '0',
                      'level-type': 'minecraft:flat', 'generate-structures': 'false',
                      'view-distance': '2', 'simulation-distance': '2',
                      'generator-settings': json.dumps({'biome': 'minecraft:plains', 'features': False,
                          'lakes': False, 'layers': [{'block': 'minecraft:stone', 'height': 128}]})}
        (java_dir / 'server.properties').write_text(''.join(f'{k}={v}\n' for k, v in properties.items()))
        for debug, immediate in [(False, False), (True, False), (False, True), (True, True)]:
            java_server = Server(['java', '-Xms128M', '-Xmx1024M', '-jar', str(jar), 'nogui'],
                                 java_dir, java_dir / 'server.log', 'Done (')
            label = f'debug-{int(debug)}-immediate-{int(immediate)}'
            for rule, value in [('reducedDebugInfo', debug), ('doImmediateRespawn', immediate)]:
                java_server.command(f'gamerule {rule} {str(value).lower()}')
                java_server.wait(f'Gamerule {rule} is now set to: {str(value).lower()}')
            java_server.command(f'say ready-{label}')
            java_server.wait(f'ready-{label}')
            original = capture(port, java_dir)
            # Copy only a cleanly closed world, never concurrently written region files.
            java_server.stop(java=True); java_server = None
            run = scratch / label; run.mkdir()
            shutil.copytree(java_dir / 'world', run / 'imported', ignore=shutil.ignore_patterns('session.lock'))
            native_port = free_port()
            keys = run / 'keys.json'; keys.write_text('{"profilePropertyKeys":[],"playerCertificateKeys":[]}')
            native = Server([str(binary), '--allow-offline', '--bind', '127.0.0.1', '--port', str(native_port),
                             '--anvil-world', str(run / 'imported'), '--server-directory', str(run),
                             '--loopback-service-keys', str(keys)], ROOT, run / 'server.log', '"event":"listening"')
            imported = capture(native_port, run)
            native.stop(); native = None
            expected = {'reduced_debug_info': debug, 'show_death_screen': not immediate}
            assert original['flags'] == imported['flags'] == expected, (original, imported, expected)
            report['cases'].append({'rules': {'reducedDebugInfo': debug, 'doImmediateRespawn': immediate},
                                    'java': original, 'native_imported': imported})
        run = scratch / 'fresh'; run.mkdir(); native_port = free_port()
        keys = run / 'keys.json'; keys.write_text('{"profilePropertyKeys":[],"playerCertificateKeys":[]}')
        native = Server([str(binary), '--allow-offline', '--bind', '127.0.0.1', '--port', str(native_port),
                         '--world', str(run / 'world.json'), '--server-directory', str(run),
                         '--loopback-service-keys', str(keys)], ROOT, run / 'server.log', '"event":"listening"')
        report['native_fresh'] = capture(native_port, run)
        native.stop(); native = None
        assert report['native_fresh']['flags'] == {'reduced_debug_info': False, 'show_death_screen': True}
        report['ok'] = True
    except Exception as error:
        report['error'] = str(error)
    finally:
        for server, is_java in [(native, False), (java_server, True)]:
            if server:
                try:
                    server.stop(java=is_java)
                except Exception as error:
                    report['ok'] = False; report['shutdown_error'] = str(error)
        report['observed_at'] = datetime.now(timezone.utc).isoformat()
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps({'ok': report['ok'], 'report': str(args.report), 'local_diagnostics': str(scratch),
                          'error': report.get('error')}))
    return 0 if report['ok'] else 1


if __name__ == '__main__':
    sys.exit(main())
