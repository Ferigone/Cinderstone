#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-only
"""Compare initial default/custom border packets on real Java/native logins."""
import argparse
from datetime import datetime, timezone
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools/parity'))
from run import Server, digest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--jar', required=True, type=Path)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--eula-file', required=True, type=Path)
    parser.add_argument('--report', required=True, type=Path)
    args = parser.parse_args()
    jar, binary = args.jar.resolve(), args.binary.resolve()
    lock = json.loads((ROOT / 'fixtures/source-lock.json').read_text())['oracle']
    if digest(jar, 'sha1') != lock['sha1'] or jar.stat().st_size != lock['size']:
        parser.error('Use the unchanged source-lock.json Java 1.21 JAR')
    if 'eula=true' not in args.eula_file.read_text().splitlines():
        parser.error('An existing EULA acceptance is required')
    scratch = Path(tempfile.mkdtemp(prefix='border-login-b291-'))
    java = scratch / 'java'; java.mkdir()
    shutil.copyfile(args.eula_file, java / 'eula.txt')
    (java / 'server.properties').write_text(
        'server-ip=127.0.0.1\nserver-port=25631\nonline-mode=false\n'
        'enforce-secure-profile=false\ngamemode=creative\nlevel-type=minecraft:flat\n'
        'generate-structures=false\nview-distance=2\nsimulation-distance=2\n')
    java_command = ['java', '-Xms128M', '-Xmx1024M', '-jar', str(jar), 'nogui']
    result = {'ok': False, 'native_commit': subprocess.check_output(
        ['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
        'native_binary_sha256': digest(binary), 'java_jar_sha1': lock['sha1'],
        'native_source_tree': subprocess.check_output(
            ['git', 'rev-parse', 'HEAD:src'], cwd=ROOT, text=True).strip(),
        'java_version': subprocess.check_output(
            ['java', '-version'], stderr=subprocess.STDOUT, text=True).splitlines()[0],
        'flows': {}, 'limits': ['Initial border packets only; no imported-world gameplay, '
                               'runtime listener packets, border damage or broad parity claim.']}
    server = None
    java_active = False

    def probe(name, port):
        path = scratch / (name + '.json')
        completed = subprocess.run(['node', str(ROOT / 'tools/border_login_client.mjs'),
                                    str(port), str(path)], cwd=ROOT, timeout=80)
        trace = json.loads(path.read_text())
        result['flows'][name] = trace
        if completed.returncode or not trace['ok']:
            raise RuntimeError(name + ' failed: ' + trace.get('error', 'client failure'))
        return trace['packets'][0]['payload_hex']

    try:
        server = Server(java_command, java, java / 'prepare.log', 'Done ('); java_active = True
        default_java = probe('java_default', 25631)
        for command in ['gamerule doMobSpawning false', 'gamerule randomTickSpeed 0',
                        'worldborder center 12.25 -7.5', 'worldborder set 200',
                        'worldborder warning distance 7', 'worldborder warning time 23',
                        'save-all flush']:
            server.command(command)
        server.wait('Saved the game')
        server.stop(java=True); server = None
        # Reload the saved border in Java too: both sides use the same persisted source.
        server = Server(java_command, java, java / 'custom.log', 'Done (')
        custom_java = probe('java_custom', 25631)
        server.stop(java=True); server = None
        vectors = json.loads((ROOT / 'fixtures/border-login-vectors.json').read_text())
        if default_java != vectors[0]['payload_hex'] or custom_java != vectors[1]['payload_hex']:
            raise AssertionError('Actual Java login does not match packet-codec witnesses')
        for label, imported in [('native_default', False), ('native_custom', True)]:
            run = scratch / label; run.mkdir()
            keys = run / 'keys.json'
            keys.write_text('{"profilePropertyKeys":[],"playerCertificateKeys":[]}')
            command = [str(binary), '--allow-offline', '--bind', '127.0.0.1', '--port', '25632',
                       '--world', str(run / 'world.json'), '--server-directory', str(run),
                       '--loopback-service-keys', str(keys)]
            if imported:
                shutil.copytree(java / 'world', run / 'world')
                command += ['--anvil-world', str(run / 'world')]
            server = Server(command, ROOT, run / 'server.log', '"event":"listening"')
            java_active = False
            payload = probe(label, 25632)
            if payload != (custom_java if imported else default_java):
                raise AssertionError(label + ': packet bytes differ from Java')
            server.stop(); server = None
        result['ok'] = True
    except Exception as error:
        result['error'] = str(error)
    finally:
        if server:
            try:
                server.stop(java=java_active)
            except Exception as error:
                result['ok'] = False; result['shutdown_error'] = str(error)
        result['completed_at'] = datetime.now(timezone.utc).isoformat()
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(result, indent=2) + '\n')
        print(json.dumps({'ok': result['ok'], 'report': str(args.report),
                          'local_diagnostics': str(scratch), 'error': result.get('error')}))
    return 0 if result['ok'] else 1


if __name__ == '__main__':
    sys.exit(main())
