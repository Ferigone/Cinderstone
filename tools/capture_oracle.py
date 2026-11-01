#!/usr/bin/env python3
"""Start the exact local vanilla oracle and capture protocol fixtures through a real client."""
import argparse
import json
import os
import subprocess
import time
from pathlib import Path
from bootstrap_oracle import PROJECT, bootstrap


def run(args):
    if not args.accept_eula:
        raise SystemExit('Running the Mojang oracle requires --accept-eula; see https://aka.ms/MinecraftEULA')
    directory = bootstrap(args.oracle_dir)
    fixtures = args.capture.resolve()
    fixtures.mkdir(parents=True, exist_ok=True)
    (directory / 'eula.txt').write_text('eula=true\n')
    settings = {
        'server-ip': '127.0.0.1', 'server-port': str(args.port), 'online-mode': 'false',
        'enforce-secure-profile': 'false', 'max-players': '4', 'spawn-protection': '0',
        'gamemode': 'creative', 'force-gamemode': 'true', 'view-distance': '2',
        'simulation-distance': '2', 'level-type': 'minecraft:flat', 'level-seed': '0',
        'generate-structures': 'false', 'spawn-animals': 'false', 'spawn-npcs': 'false',
        'spawn-monsters': 'false', 'max-tick-time': '60000', 'enable-query': 'false',
        'enable-rcon': 'false', 'motd': 'Official Mojang 1.21 conformance oracle',
        'generator-settings': json.dumps({'biome': 'minecraft:plains', 'layers': [
            {'block':'minecraft:bedrock','height':1}, {'block':'minecraft:stone','height':124},
            {'block':'minecraft:dirt','height':2}, {'block':'minecraft:grass_block','height':1}]}),
    }
    (directory / 'server.properties').write_text(''.join(k+'='+v+'\n' for k,v in settings.items()))
    log_path = PROJECT / 'evidence/oracle-server.log'
    started = time.monotonic()
    with log_path.open('w') as log:
        process = subprocess.Popen([args.java, '-Xms128M', '-Xmx1024M', '-jar', 'server-1.21.jar', 'nogui'],
                                   cwd=directory, stdin=subprocess.PIPE, stdout=log, stderr=subprocess.STDOUT, text=True)
        try:
            deadline = time.monotonic() + 180
            while True:
                text = log_path.read_text(errors='replace')
                if 'Done (' in text:
                    break
                if process.poll() is not None:
                    raise RuntimeError('Vanilla oracle exited: ' + text[-5000:])
                if time.monotonic() > deadline:
                    raise TimeoutError('Vanilla startup deadline: ' + text[-5000:])
                time.sleep(.2)
            print(json.dumps({'oracle':'ready','version':'1.21','startup_seconds':round(time.monotonic()-started,3)}), flush=True)
            checks = [('status', ['node', str(PROJECT/'tools/probe.mjs'), '--port', str(args.port)]),
                      ('join', ['node', str(PROJECT/'tools/interop.mjs'), '--port', str(args.port),
                                '--capture', str(fixtures), '--source', 'Official Mojang vanilla 1.21'])]
            results = {}
            for label, command in checks:
                result = subprocess.run(command, cwd=PROJECT, text=True, capture_output=True, timeout=45)
                results[label] = {'exit_code':result.returncode, 'stdout':result.stdout, 'stderr':result.stderr}
                print(json.dumps({'check':label,**results[label]}), flush=True)
                if result.returncode:
                    raise RuntimeError('Oracle client check failed: '+label)
            (PROJECT/'evidence/oracle-checks.json').write_text(json.dumps(results,indent=2)+'\n')
        finally:
            if process.poll() is None:
                try:
                    process.stdin.write('stop\n')
                    process.stdin.flush()
                    process.wait(timeout=25)
                except (BrokenPipeError,subprocess.TimeoutExpired):
                    process.terminate()
                    try: process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait()


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--oracle-dir',type=Path,default=PROJECT/'runtime/oracle')
    parser.add_argument('--capture',type=Path,default=PROJECT/'fixtures')
    parser.add_argument('--port',type=int,default=25566)
    parser.add_argument('--java',default=os.environ.get('MC_JAVA','java'))
    parser.add_argument('--accept-eula',action='store_true')
    run(parser.parse_args())
