#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-only
"""Compare imported world-type login flags with real unchanged Java 1.21."""
import argparse
from datetime import datetime, timezone
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

from login_rules_flow import ROOT, Server, digest, free_port


def capture(port, directory, username):
    output = directory / 'login.json'
    subprocess.run(['node', str(Path(__file__).with_name('login_rules_flow.mjs')),
                    str(port), str(output), 'world-type', username],
                   cwd=ROOT, check=True, timeout=45)
    result = json.loads(output.read_text())
    assert result['ok'] and result['position_received'], result
    return result


def start_native(binary, run, world=None):
    run.mkdir()
    keys = run / 'keys.json'; keys.write_text('{"profilePropertyKeys":[],"playerCertificateKeys":[]}')
    port = free_port()
    backend = ['--anvil-world', str(world)] if world else ['--world', str(run / 'world.json')]
    server = Server([str(binary), '--allow-offline', '--bind', '127.0.0.1', '--port', str(port),
                     '--server-directory', str(run), '--loopback-service-keys', str(keys), *backend],
                    ROOT, run / 'server.log', '"event":"listening"')
    return server, port


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
    scratch = Path(tempfile.mkdtemp(prefix='cinderstone-world-type-'))
    java_options = ['-XX:ActiveProcessorCount=2', '-Xms128M', '-Xmx768M']
    report = {'ok': False, 'java_jar_sha1': lock['sha1'], 'minecraft': '1.21', 'protocol': 767,
              'source_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
              'main_source_sha256': digest(ROOT / 'src/main.rs'), 'binary_sha256': digest(binary),
              'runner_sha256': digest(Path(__file__)),
              'client_sha256': digest(Path(__file__).with_name('login_rules_flow.mjs')),
              'java_options': java_options,
              'cases': [], 'scope': 'Initial PLAY isDebug/isFlat flags only; no generator or gameplay parity'}
    java_server = native = None
    try:
        for preset, expected in [
            ('flat', {'is_debug': False, 'is_flat': True}),
            ('normal', {'is_debug': False, 'is_flat': False}),
            ('debug_all_block_states', {'is_debug': True, 'is_flat': False}),
        ]:
            scene = scratch / preset; scene.mkdir()
            java_dir = scene / 'java'; java_dir.mkdir()
            shutil.copyfile(args.eula_file, java_dir / 'eula.txt')
            java_port = free_port()
            properties = {'server-ip': '127.0.0.1', 'server-port': str(java_port), 'online-mode': 'false',
                          'enforce-secure-profile': 'false', 'gamemode': 'creative', 'level-seed': '0',
                          'level-type': 'minecraft:' + preset, 'generate-structures': 'false',
                          'view-distance': '2', 'simulation-distance': '2'}
            if preset == 'flat':
                properties['generator-settings'] = json.dumps({'biome': 'minecraft:plains', 'features': False,
                    'lakes': False, 'layers': [{'block': 'minecraft:stone', 'height': 128}]})
            (java_dir / 'server.properties').write_text(''.join(f'{k}={v}\n' for k, v in properties.items()))
            java_server = Server(['java', *java_options, '-jar', str(jar), 'nogui'],
                                 java_dir, java_dir / 'server.log', 'Done (')
            original = capture(java_port, java_dir, 'TypeJava')
            java_server.stop(java=True); java_server = None
            shutil.copytree(java_dir / 'world', scene / 'imported', ignore=shutil.ignore_patterns('session.lock'))
            native_dir = scene / 'native'
            native, native_port = start_native(binary, native_dir, scene / 'imported')
            imported = capture(native_port, native_dir, 'TypeNative')
            native.stop(); native = None
            assert original['flags'] == imported['flags'] == expected, (preset, original, imported, expected)
            report['cases'].append({'java_preset': preset, 'java': original, 'native_imported': imported})
        run = scratch / 'fresh'
        native, port = start_native(binary, run)
        report['native_fresh'] = capture(port, run, 'TypeNative')
        native.stop(); native = None
        assert report['native_fresh']['flags'] == {'is_debug': False, 'is_flat': True}
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
