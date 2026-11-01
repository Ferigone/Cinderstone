#!/usr/bin/env python3
"""Execute the narrow native checkpoint checks in one isolated network namespace."""
import argparse
import concurrent.futures
import json
import hashlib
import signal
import socket
import subprocess
import time
from pathlib import Path

PROJECT=Path(__file__).resolve().parents[1]


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--binary',type=Path,default=PROJECT/'target/debug/mc121-rust')
    parser.add_argument('--port',type=int,default=25565)
    args=parser.parse_args()
    binary=args.binary.resolve()
    evidence=PROJECT/'evidence'
    evidence.mkdir(exist_ok=True)
    runtime=PROJECT/'runtime/native-conformance'
    runtime.mkdir(parents=True,exist_ok=True)
    world=runtime/'world.json'
    world.unlink(missing_ok=True)
    result={'scope':'protocol-767 creative compatibility checkpoint',
            'server_binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),
            'not_verified':['unmodified graphical Java client','online authentication','full vanilla mechanics','performance parity'],
            'checks':{},'ok':False}
    process=None
    log=None

    def command(name,argv,timeout=40):
        call=subprocess.run(argv,cwd=PROJECT,text=True,capture_output=True,timeout=timeout)
        record={'exit_code':call.returncode,'stdout':call.stdout,'stderr':call.stderr}
        if call.stdout.strip():
            try:record['result']=json.loads(call.stdout.strip().splitlines()[-1])
            except ValueError:pass
        result['checks'][name]=record
        print(json.dumps({'check':name,**record}),flush=True)
        if call.returncode:raise RuntimeError('Native check failed: '+name)
        return record

    def start(label):
        nonlocal process,log
        log_path=evidence/('rust-server-'+label+'.log')
        log=log_path.open('w')
        process=subprocess.Popen([str(binary),'--allow-offline','--port',str(args.port),
                                  '--world',str(world)],cwd=PROJECT,stdout=log,stderr=subprocess.STDOUT)
        deadline=time.monotonic()+20
        while True:
            if process.poll() is not None:raise RuntimeError('Native process exited: '+log_path.read_text()[-4000:])
            if '"event":"listening"' in log_path.read_text():break
            if time.monotonic()>deadline:raise TimeoutError('Native startup deadline')
            time.sleep(.05)

    def stop():
        nonlocal process,log
        if process is not None and process.poll() is None:
            process.send_signal(signal.SIGINT)
            try:process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill();process.wait()
        if log is not None:log.close()
        process=None;log=None

    try:
        guard=subprocess.run([str(binary)],cwd=PROJECT,text=True,capture_output=True,timeout=10)
        if guard.returncode==0 or '--allow-offline' not in guard.stderr:
            raise AssertionError('Server must require explicit offline opt-in')
        result['checks']['offline_opt_in']={'ok':True,'exit_code':guard.returncode}
        start('initial')
        command('status',['node','tools/probe.mjs','--port',str(args.port)])
        command('interop',['node','tools/interop.mjs','--port',str(args.port),
                           '--capture',str(evidence/'rust-capture'),'--source','Native Rust checkpoint'])
        reference=json.loads((PROJECT/'fixtures/configuration.json').read_text())
        observed=json.loads((evidence/'rust-capture/configuration.json').read_text())
        expected=[(p['name'],p['id'],p['packet']) for p in reference['packets']]
        actual=[(p['name'],p['id'],p['packet']) for p in observed['packets']]
        if actual!=expected:raise AssertionError('Native configuration transcript differs from actual Java oracle')
        result['checks']['configuration_bytes']={'ok':True,'identical_packets':len(actual),
          'registry_packets':sum(p[0]=='registry_data' for p in actual)}
        expected_join=json.loads((PROJECT/'fixtures/join.json').read_text())
        actual_join=json.loads((evidence/'rust-capture/join.json').read_text())
        if actual_join['worldState']['hashedSeed']!=expected_join['worldState']['hashedSeed']:
            raise AssertionError('Client biome seed hash differs from unchanged Java fixture')
        result['checks']['join_biome_seed']={'ok':True,'matches_original_java':True}
        with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
            work=[pool.submit(command,'concurrent_'+str(i),['node','tools/interop.mjs','--port',str(args.port),
                  '--username','RustPeer'+str(i)]) for i in range(3)]
            for task in work:task.result()
        malformed=[b'\xff'*6,b'\x00',b'\x80',b'\xff\xff\xff\xff\x07',b'\x01\xff']
        for payload in malformed:
            with socket.create_connection(('127.0.0.1',args.port),timeout=5) as client:
                client.sendall(payload)
                client.shutdown(socket.SHUT_WR)
        command('status_after_malformed',['node','tools/probe.mjs','--port',str(args.port)])
        result['checks']['malformed_connections']={'ok':True,'cases':len(malformed)}
        command('creative_break',['node','tools/gameplay_probe.mjs','--port',str(args.port)])
        stop()
        saved=json.loads(world.read_text())
        if not any(c=={'x':8,'y':63,'z':8,'state':0} for c in saved['changes']):
            raise AssertionError('Creative break missing from atomic world diff')
        start('restart')
        command('persisted_chunk_after_restart',['node','tools/gameplay_probe.mjs','--port',str(args.port),'--expect-air'])
        result['checks']['persisted_world_diff']={'ok':True,'format':saved['format']}
        result['ok']=True
    except Exception as error:
        result['error']=str(error)
        raise
    finally:
        stop()
        (evidence/'native-conformance.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps({'ok':result['ok'],'checks':list(result['checks']),
                      'report':'evidence/native-conformance.json'}),flush=True)


if __name__=='__main__':main()
