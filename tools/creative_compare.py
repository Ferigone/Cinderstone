#!/usr/bin/env python3
"""Run one client scenario against Rust and an unchanged exact 1.21 Java JAR."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import time

ROOT=Path(__file__).resolve().parents[1]

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle',type=Path,default=ROOT.parent/'mc121-oracle')
    parser.add_argument('--java',default='/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin/java')
    parser.add_argument('--binary',type=Path,default=ROOT/'target/debug/mc121-rust')
    parser.add_argument('--scenario',choices=['creative','placement'],default='creative')
    args=parser.parse_args()
    oracle=args.oracle.resolve()
    jar=oracle/'server-1.21.jar'
    if hashlib.sha1(jar.read_bytes()).hexdigest()!='450698d1863ab5180c25d7c804ef0fe6369dd1ba':
        raise RuntimeError('Exact unchanged official 1.21 server is required')
    if 'eula=true' not in (oracle/'eula.txt').read_text():
        raise RuntimeError('Oracle has no existing EULA acceptance')
    # A unique copy also prevents stale inode/mtime based caches from reusing an
    # earlier scenario's edited world. Never run the Java server on the oracle.
    runtime=ROOT/'runtime';runtime.mkdir(exist_ok=True)
    run=Path(tempfile.mkdtemp(prefix=args.scenario+'-java-oracle-',dir=runtime))
    (run/'world').mkdir(parents=True)
    for name in ['level.dat']:
        shutil.copyfile(oracle/'world'/name,run/'world'/name)
    shutil.copytree(oracle/'world/region',run/'world/region',copy_function=shutil.copyfile)
    for name in ['libraries','versions']:
        (run/name).symlink_to(oracle/name,target_is_directory=True)
    shutil.copyfile(oracle/'eula.txt',run/'eula.txt')
    settings=(oracle/'server.properties').read_text().splitlines()
    replacements={'server-port':'25571','level-name':'world','max-players':'20'}
    settings=[key+'='+replacements[key] if (key:=line.partition('=')[0]) in replacements else line for line in settings]
    (run/'server.properties').write_text('\n'.join(settings)+'\n')
    evidence=ROOT/'evidence';evidence.mkdir(exist_ok=True)
    probe='tools/'+args.scenario+'_live_probe.mjs'
    report={'ok':False,'scenario':args.scenario,'checks':{}}
    server=None
    log_path=evidence/(args.scenario+'-java-oracle.log')
    with log_path.open('w') as log:
        try:
            rust=subprocess.run(['node',probe,'--server-binary',str(args.binary.resolve()),'--label','rust'],cwd=ROOT,text=True,capture_output=True,timeout=45)
            if rust.returncode:raise RuntimeError('Rust scenario failed: '+rust.stdout+rust.stderr)
            server=subprocess.Popen([args.java,'-Xms128M','-Xmx1024M','-jar',str(jar),'nogui'],cwd=run,stdin=subprocess.PIPE,stdout=log,stderr=subprocess.STDOUT,text=True)
            deadline=time.monotonic()+180
            while 'Done (' not in log_path.read_text():
                if server.poll() is not None:raise RuntimeError('Java oracle exited')
                if time.monotonic()>deadline:raise TimeoutError('Java startup')
                time.sleep(.2)
            client=subprocess.Popen(['node',probe,'--port','25571','--label','java','--wait-common-position'],cwd=ROOT,text=True,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
            deadline=time.monotonic()+40
            teleported=False
            while client.poll() is None:
                if not teleported and 'RustActionProbe joined the game' in log_path.read_text():
                    server.stdin.write('tp RustActionProbe 8.5 65 8.5\n');server.stdin.flush();teleported=True
                if time.monotonic()>deadline:client.kill();raise TimeoutError('Java scenario')
                time.sleep(.1)
            out,err=client.communicate()
            if client.returncode:raise RuntimeError('Java scenario failed: '+out+err)
            rust_report=json.loads((evidence/(args.scenario+'-live-rust.json')).read_text())
            java_report=json.loads((evidence/(args.scenario+'-live-java.json')).read_text())
            if rust_report['checks']!=java_report['checks']:raise AssertionError('Different semantic scenario results')
            report.update(ok=True,checks=rust_report['checks'],oracle_sha1='450698d1863ab5180c25d7c804ef0fe6369dd1ba',not_covered=['other block placement behavior','survival','entity collisions beyond standing player','lighting after edits'])
        finally:
            if server and server.poll() is None:
                server.stdin.write('stop\n');server.stdin.flush()
                try:server.wait(timeout=30)
                except subprocess.TimeoutExpired:server.kill();server.wait()
            (evidence/(args.scenario+'-differential.json')).write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report),flush=True)

if __name__=='__main__':main()
