#!/usr/bin/env python3
"""Run vanilla in a disposable worldcopy and capture networkchunkdata."""
import json,os,shutil,subprocess,sys,time
from pathlib import Path
ROOT=Path(__file__).resolve().parent.parent
ORACLE=ROOT.parent/'mc121-oracle'
OUTPUT=Path(sys.argv[1] if len(sys.argv)>1 else '/tmp/mc121-storage-proof').resolve()
RUN=OUTPUT/'java-run'
RUN.mkdir(parents=True,exist_ok=True)
if not (RUN/'world').exists():shutil.copytree(ORACLE/'world',RUN/'world')
for name in ['server-1.21.jar','libraries','versions']:
    source=ORACLE/name
    target=RUN/name
    if source.exists() and not target.exists():target.symlink_to(source,target_is_directory=source.is_dir())
settings=(ORACLE/'server.properties').read_text().splitlines()
settings=[s for s in settings if not s.startswith(('server-port=','level-name=','spawn-protection='))]
(RUN/'server.properties').write_text('\n'.join(settings)+'\nserver-port=25587\nlevel-name=world\nspawn-protection=0\n')
(RUN/'eula.txt').write_text('eula=true\n')
JAVA=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin/java')
with (OUTPUT/'java-server.log').open('w') as log:
    p=subprocess.Popen([str(JAVA),'-Xms128M','-Xmx768M','-jar','server-1.21.jar','nogui'],cwd=RUN,stdin=subprocess.PIPE,stdout=log,stderr=subprocess.STDOUT,text=True)
    try:
        deadline=time.monotonic()+120
        while 'Done (' not in (OUTPUT/'java-server.log').read_text(errors='replace'):
            if p.poll() is not None:raise RuntimeError('Java exited')
            if time.monotonic()>deadline:raise TimeoutError('Java ready')
            time.sleep(.2)
        if '--edit' in sys.argv[2:]:
            p.stdin.write('setblock 1 63 1 minecraft:air\nsetblock -1 64 -1 minecraft:stone\n');p.stdin.flush()
            deadline=time.monotonic()+10
            while (OUTPUT/'java-server.log').read_text(errors='replace').count('Changed the block at')<2:
                if time.monotonic()>deadline:raise TimeoutError('setblock command feedback')
                time.sleep(.1)
            # Let the server's real incremental light engine settle over ticks.
            time.sleep(1)
        subprocess.run(['node',str(ROOT/'tools/storage_capture.mjs'),'25587',str(OUTPUT)],cwd=ROOT,check=True,timeout=40)
    finally:
        if p.poll() is None:
            p.stdin.write('stop\n');p.stdin.flush()
            try:p.wait(timeout=25)
            except subprocess.TimeoutExpired:p.kill();p.wait()
print(json.dumps({'ok':True,'vanilla_capture':str(OUTPUT),'original_world_unchanged':True}))
