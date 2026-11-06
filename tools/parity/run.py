#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-only
"""Run a catalogue scenario against unchanged Java 1.21 and native Main."""
import argparse
import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path

from diff import compare

ROOT = Path(__file__).resolve().parents[2]

def digest(path, algorithm='sha256'):
    h = hashlib.new(algorithm)
    with path.open('rb') as f:
        for block in iter(lambda: f.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()

class Server:
    def __init__(self, command, cwd, logfile, marker):
        self.logfile = logfile
        self.log = logfile.open('w')
        self.process = subprocess.Popen(command, cwd=cwd, stdin=subprocess.PIPE,
                                        stdout=self.log, stderr=subprocess.STDOUT, text=True)
        try:
            self.wait(marker)
        except Exception:
            if self.process.poll() is None:
                self.process.terminate()
                try:
                    self.process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    self.process.kill()
                    self.process.wait()
            self.log.close()
            raise

    def wait(self, marker, timeout=180):
        deadline = time.monotonic() + timeout
        while marker not in self.logfile.read_text(errors='replace'):
            if self.process.poll() is not None:
                raise RuntimeError(f'Server exited ({self.process.returncode}); see local server log')
            if time.monotonic() > deadline:
                self.stop()
                raise TimeoutError('Server startup/command deadline: ' + marker)
            time.sleep(.1)

    def command(self, text):
        self.process.stdin.write(text + '\n')
        self.process.stdin.flush()

    def stop(self, java=False):
        if self.process.poll() is None:
            if java:
                self.command('stop')
            else:
                self.process.terminate()
            try:
                self.process.wait(timeout=60)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()
                raise RuntimeError('Server required forced shutdown')
        self.log.close()
        if self.process.returncode != 0:
            raise RuntimeError(f'Nonzero server exit: {self.process.returncode}')

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--java', default='java')
    parser.add_argument('--scenario', default='creative-stone')
    parser.add_argument('--eula-file', type=Path, required=True, help='Existing acceptance for local Java oracle')
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    jar, binary = args.jar.resolve(), args.binary.resolve()
    lock = json.loads((ROOT / 'fixtures/source-lock.json').read_text())['oracle']
    if jar.stat().st_size != lock['size'] or digest(jar, 'sha1') != lock['sha1']:
        parser.error('JAR must be the unchanged source-lock.json Java 1.21 server')
    if 'eula=true' not in args.eula_file.read_text().splitlines():
        parser.error('Supply an existing eula=true acceptance; runner never accepts terms')
    catalogue = json.loads((Path(__file__).with_name('scenarios.json')).read_text())
    if args.scenario not in catalogue:
        parser.error('Unknown catalogue scenario')
    spec = catalogue[args.scenario]
    scratch = Path(tempfile.mkdtemp(prefix='cinderstone-parity-'))
    report = {'schema_version': 1, 'ok': False, 'completed_at': None, 'scenario': args.scenario,
              'specification': spec, 'native_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
              'native_binary_sha256': digest(binary), 'java_jar_sha1': lock['sha1'],
              'java_version': subprocess.check_output([args.java, '-version'], stderr=subprocess.STDOUT, text=True).splitlines()[0],
              'traces': {}, 'differences': [], 'error': None}
    server = None
    java_active = False
    try:
        seed = scratch / 'seed'; seed.mkdir()
        shutil.copyfile(args.eula_file, seed / 'eula.txt')
        properties = {'server-ip': '127.0.0.1', 'server-port': '25591', 'online-mode': 'false',
                      'enforce-secure-profile': 'false', 'gamemode': 'creative', 'level-seed': '0',
                      'level-type': 'minecraft:flat', 'generate-structures': 'false', 'spawn-protection': '0',
                      'view-distance': '4', 'simulation-distance': '4', 'sync-chunk-writes': 'true',
                      'generator-settings': json.dumps({'biome': 'minecraft:plains', 'features': False, 'lakes': False,
                        'layers': [{'block': 'minecraft:stone', 'height': 125}, {'block': 'minecraft:dirt', 'height': 2},
                                   {'block': 'minecraft:grass_block', 'height': 1}]}, separators=(',', ':'))}
        (seed / 'server.properties').write_text(''.join(k + '=' + v + '\n' for k, v in properties.items()))
        java_cmd = [args.java, '-Xms128M', '-Xmx1536M', '-jar', str(jar), 'nogui']
        server = Server(java_cmd, seed, scratch / 'prepare.log', 'Done ('); java_active = True
        for command in ['gamerule doMobSpawning false', 'gamerule randomTickSpeed 0', 'gamerule doDaylightCycle false',
                        'gamerule doWeatherCycle false', 'weather clear', 'time set day',
                        'setworldspawn 8 64 8', 'forceload add -64 -64 64 64', 'save-all flush']:
            server.command(command)
        server.wait('Saved the game')
        server.stop(java=True); server = None
        pristine = seed / 'world'
        # Retain provenance for the original-generated scene; compare its blocks on wire.
        world_hash = hashlib.sha256()
        for path in sorted(pristine.rglob('*')):
            if path.is_file() and path.name != 'session.lock':
                world_hash.update(str(path.relative_to(pristine)).encode())
                world_hash.update(bytes.fromhex(digest(path)))
        report['pristine_world_sha256'] = world_hash.hexdigest()
        scenario_file = scratch / 'scenario.json'; scenario_file.write_text(json.dumps(spec))
        for label in ['java', 'native']:
            run = scratch / label; run.mkdir()
            if label == 'java':
                shutil.copytree(pristine, run / 'world')
            output = run / 'trace.json'
            if label == 'java':
                shutil.copyfile(args.eula_file, run / 'eula.txt')
                shutil.copyfile(seed / 'server.properties', run / 'server.properties')
                server = Server(java_cmd, run, run / 'server.log', 'Done ('); java_active = True
                port = 25591
            else:
                keys = run / 'keys.json'
                keys.write_text('{"profilePropertyKeys":[],"playerCertificateKeys":[]}')
                server = Server([str(binary), '--allow-offline', '--bind', '127.0.0.1', '--port', '25592',
                                 '--world', str(run / 'world.json'), '--server-directory', str(run),
                                 '--loopback-service-keys', str(keys)], ROOT, run / 'server.log', '"event":"listening"')
                java_active = False; port = 25592
            client = subprocess.Popen(['node', str(Path(__file__).with_name('client.mjs')), str(port),
                                       str(scenario_file), str(output)], cwd=ROOT)
            deadline = time.monotonic() + 100
            teleported = False
            while client.poll() is None:
                if java_active and not teleported and 'ParityProbe joined the game' in server.logfile.read_text(errors='replace'):
                    server.command('tp ParityProbe 8.5 65 8.5'); teleported = True
                if time.monotonic() > deadline:
                    client.kill(); client.wait()
                    raise TimeoutError(label + ' client deadline')
                time.sleep(.05)
            report['traces'][label] = json.loads(output.read_text())
            server.stop(java=java_active); server = None
        report['differences'] = compare(report['traces']['java'], report['traces']['native'])
        report['ok'] = not report['differences']
    except Exception as error:
        report['error'] = str(error)
    finally:
        if server:
            try:
                server.stop(java=java_active)
            except Exception as error:
                report['ok'] = False; report['error'] = str(error)
        report['completed_at'] = datetime.now(timezone.utc).isoformat()
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps({'ok': report['ok'], 'report': str(args.report), 'local_diagnostics': str(scratch),
                          'differences': len(report['differences']), 'error': report['error']}))
    return 0 if report['ok'] else 1

if __name__ == '__main__':
    sys.exit(main())
