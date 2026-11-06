#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-only
"""Run the same bounded literal team-chat flow against unchanged Java and native."""
import argparse
from datetime import datetime, timezone
import json
from pathlib import Path
import selectors
import shutil
import subprocess
import sys
import tempfile
import time

from login_rules_flow import ROOT, Server, digest, free_port


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
    scratch = Path(tempfile.mkdtemp(prefix='cinderstone-team-chat-'))
    result = {'ok': False, 'minecraft': '1.21', 'protocol': 767, 'java_jar_sha1': lock['sha1'],
              'source_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
              'binary_sha256': digest(binary), 'runner_sha256': digest(Path(__file__)),
              'client_sha256': digest(Path(__file__).with_name('team_chat_client.mjs')), 'traces': {},
              'scope': 'Literal <=256 UTF-16 offline team chat; no selectors, external filtering or real-account trust',
              'comparison': 'Exact command subgraph and ordered per-recipient chat content/type/name/target; exclude timestamps, salts and global unrelated tree nodes'}
    server = driver = None
    try:
        for kind in ['java', 'native']:
            folder = scratch / kind; folder.mkdir(); port = free_port()
            if kind == 'java':
                shutil.copyfile(args.eula_file, folder / 'eula.txt')
                props = {'server-ip': '127.0.0.1', 'server-port': str(port), 'online-mode': 'false',
                         'enforce-secure-profile': 'false', 'gamemode': 'creative', 'level-seed': '0',
                         'level-type': 'minecraft:flat', 'generate-structures': 'false', 'view-distance': '2',
                         'simulation-distance': '2', 'generator-settings': json.dumps({'biome': 'minecraft:plains',
                             'features': False, 'lakes': False, 'layers': [{'block': 'minecraft:stone', 'height': 128}]})}
                (folder / 'server.properties').write_text(''.join(f'{k}={v}\n' for k, v in props.items()))
                command = ['java', '-XX:ActiveProcessorCount=2', '-Xms128M', '-Xmx512M', '-jar', str(jar), 'nogui']
            else:
                keys = folder / 'keys.json'; keys.write_text('{"profilePropertyKeys":[],"playerCertificateKeys":[]}')
                command = [str(binary), '--allow-offline', '--bind', '127.0.0.1', '--port', str(port),
                           '--world', str(folder / 'world.json'), '--server-directory', str(folder),
                           '--loopback-service-keys', str(keys)]
            server = Server(command, folder if kind == 'java' else ROOT, folder / 'server.log',
                            'Done (' if kind == 'java' else '"event":"listening"')
            output = folder / 'trace.json'
            driver = subprocess.Popen(['node', str(Path(__file__).with_name('team_chat_client.mjs')), str(port), str(output)],
                                      cwd=ROOT, stdout=subprocess.PIPE, bufsize=0)
            deadline = time.monotonic() + 180
            with selectors.DefaultSelector() as poller:
                poller.register(driver.stdout, selectors.EVENT_READ)
                while driver.poll() is None:
                    if time.monotonic() > deadline:
                        driver.kill(); driver.wait(); raise TimeoutError('client scenario deadline')
                    for key, _ in poller.select(.2):
                        line = key.fileobj.readline()
                        if line.startswith(b'CONSOLE:'):
                            server.command(json.loads(line[len(b'CONSOLE:'):]))
            trace = json.loads(output.read_text()); result['traces'][kind] = trace
            assert driver.returncode == 0 and trace['ok'], (kind, trace.get('error'))
            driver = None
            server.stop(java=kind == 'java'); server = None
            log = (folder / 'server.log').read_text(errors='replace')
            trace['console_failure_observed'] = 'permissions.requires.entity' in log if kind == 'native' else any(
                'entit' in line.lower() and 'command' in line.lower() for line in log.splitlines())
            assert trace['console_failure_observed'], kind + ' console failure not observed'
        assert result['traces']['java'] == result['traces']['native'], 'Java/native team chat traces differ'
        result['ok'] = True
    except Exception as error:
        result['error'] = str(error)
    finally:
        if driver and driver.poll() is None:
            driver.kill(); driver.wait()
        if server:
            try:
                server.stop(java=kind == 'java')
            except Exception as error:
                result['shutdown_error'] = str(error)
        result['observed_at'] = datetime.now(timezone.utc).isoformat()
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(result, indent=2) + '\n')
        print(json.dumps({'ok': result['ok'], 'report': str(args.report), 'local_diagnostics': str(scratch),
                          'error': result.get('error')}))
    return 0 if result['ok'] else 1


if __name__ == '__main__':
    sys.exit(main())
