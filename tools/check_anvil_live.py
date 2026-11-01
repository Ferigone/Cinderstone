#!/usr/bin/env python3
"""Exercise real Java-generated Anvil chunks through the native Rust listener."""
import argparse
import json
from pathlib import Path
import shutil
import signal
import subprocess
import time
import tempfile

ROOT=Path(__file__).resolve().parents[1]

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--oracle-world',type=Path,default=ROOT.parent/'mc121-oracle/world')
    p.add_argument('--binary',type=Path,default=ROOT/'target/debug/mc121-rust')
    args=p.parse_args()
    (ROOT/'runtime').mkdir(exist_ok=True)
    world=Path(tempfile.mkdtemp(prefix='anvil-live-',dir=ROOT/'runtime'))/'world'
    world.mkdir(parents=True)
    shutil.copyfile(args.oracle_world/'level.dat',world/'level.dat')
    shutil.copytree(args.oracle_world/'region',world/'region',copy_function=shutil.copyfile)
    evidence=ROOT/'evidence';evidence.mkdir(exist_ok=True)
    report={'ok':False,'scope':'native Anvil chunks, creative edit, stable relight and restart','checks':[]}
    server=None;log=None
    def start(name):
        nonlocal server,log
        path=evidence/('anvil-live-'+name+'.log');log=path.open('w')
        server=subprocess.Popen([str(args.binary.resolve()),'--allow-offline','--port','25572','--anvil-world',str(world)],cwd=ROOT,stdout=log,stderr=subprocess.STDOUT)
        deadline=time.monotonic()+20
        while '"event":"listening"' not in path.read_text():
            if server.poll() is not None:raise RuntimeError('Server exited: '+path.read_text()[-4000:])
            if time.monotonic()>deadline:raise TimeoutError('Server startup')
            time.sleep(.05)
    def stop():
        nonlocal server,log
        if server and server.poll() is None:
            server.send_signal(signal.SIGINT)
            try:server.wait(timeout=30)
            except subprocess.TimeoutExpired:server.kill();server.wait()
            if server.returncode:raise RuntimeError('Anvil server save/exit failed')
        if log:log.close()
        server=None;log=None
    try:
        for stage,extra in [('edit',[]),('restart',['--expect-air'])]:
            start(stage)
            result=subprocess.run(['node','tools/gameplay_probe.mjs','--port','25572',*extra],cwd=ROOT,text=True,capture_output=True,timeout=45)
            if result.returncode:raise RuntimeError(stage+': '+result.stdout+result.stderr)
            report['checks'].append({'stage':stage,'result':json.loads(result.stdout.strip().splitlines()[-1])})
            stop()
        metadata=subprocess.run(['node','tools/level_compare.mjs',str(args.oracle_world/'level.dat'),str(world/'level.dat')],cwd=ROOT,text=True,capture_output=True,timeout=15)
        if metadata.returncode:raise RuntimeError('level.dat: '+metadata.stdout+metadata.stderr)
        report['checks'].append({'stage':'level.dat','result':json.loads(metadata.stdout.strip())})
        report['ok']=True
    finally:
        stop()
        (evidence/'anvil-live-conformance.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report),flush=True)

if __name__=='__main__':main()
