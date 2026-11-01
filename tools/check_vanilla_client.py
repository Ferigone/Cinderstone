#!/usr/bin/env python3
"""Launch the official unchanged Java client against the local Rust checkpoint.

Requires Java21+, Xvfb and OpenGL. All processes intentionally share this single
invocation's network namespace. Uses a synthetic offline test identity; it does
not verify Microsoft authentication or full vanilla gameplay parity.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import time

PROJECT=Path(__file__).resolve().parents[1]


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--client-dir',type=Path,default=PROJECT.parent/'mc121-client')
    parser.add_argument('--java-home',type=Path,default=Path(os.environ.get('JAVA_HOME','/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1')))
    parser.add_argument('--binary',type=Path,default=PROJECT/'target/debug/mc121-rust')
    parser.add_argument('--xvfb',type=Path)
    parser.add_argument('--port',type=int,default=25568)
    parser.add_argument('--timeout',type=int,default=100)
    args=parser.parse_args()
    directory=args.client_dir.resolve()
    evidence=PROJECT/'evidence'
    evidence.mkdir(exist_ok=True)
    report={'ok':False,'minecraft':'1.21','protocol':767,'client_modified':False,
            'scope':'unmodified graphical Java client with synthetic offline identity',
            'not_verified':['Microsoft online authentication','complete vanilla gameplay parity']}
    children=[]
    files=[]
    try:
        launch=json.loads((directory/'launch.json').read_text())
        client_jar=directory/'client-1.21.jar'
        actual=hashlib.sha1(client_jar.read_bytes()).hexdigest()
        if actual!=launch['client_sha1'] or actual!='0e9a07b9bb3390602f977073aa12884a4ce12431':
            raise RuntimeError('The official unmodified 1.21 client digest is required')
        report['client_sha1']=actual
        report['server_binary_sha256']=hashlib.sha256(args.binary.resolve().read_bytes()).hexdigest()
        local_display=directory/'display'
        xvfb=args.xvfb or (Path(shutil.which('Xvfb')) if shutil.which('Xvfb') else local_display/'usr/bin/Xvfb')
        if not xvfb.is_file():raise RuntimeError('Xvfb is unavailable')
        env=os.environ.copy()
        env['DISPLAY']='127.0.0.1:97'
        env['LIBGL_ALWAYS_SOFTWARE']='1'
        display_lib=local_display/'usr/lib/x86_64-linux-gnu'
        env['LD_LIBRARY_PATH']=str(display_lib)+os.pathsep+env.get('LD_LIBRARY_PATH','')
        display_log=(evidence/'vanilla-client-display.log').open('w');files.append(display_log)
        display=subprocess.Popen([str(xvfb),':97','-screen','0','1024x768x24','-ac','-nolisten','unix','-listen','tcp'],env=env,stdout=display_log,stderr=subprocess.STDOUT)
        children.append(display)
        time.sleep(1)
        if display.poll() is not None:raise RuntimeError('Xvfb exited: '+(evidence/'vanilla-client-display.log').read_text()[-4000:])
        server_path=evidence/'vanilla-client-server.log'
        server_log=server_path.open('w');files.append(server_log)
        server=subprocess.Popen([str(args.binary.resolve()),'--allow-offline','--port',str(args.port),
                                 '--world',str(PROJECT/'runtime/vanilla-client-world.json')],cwd=PROJECT,env=env,stdout=server_log,stderr=subprocess.STDOUT)
        children.append(server)
        ready=time.monotonic()+15
        while '"event":"listening"' not in server_path.read_text():
            if server.poll() is not None:raise RuntimeError('Rust fixture exited before listening')
            if time.monotonic()>ready:raise TimeoutError('Rust fixture startup timeout')
            time.sleep(.1)
        game=directory/'game'
        game.mkdir(exist_ok=True)
        (game/'options.txt').write_text('version:3953\nonboardAccessibility:false\nlang:en_us\nautoJump:false\nenableVsync:false\nmaxFps:30\nrenderDistance:2\nsimulationDistance:2\nfullscreen:false\nsoundCategory_master:0.0\npauseOnLostFocus:false\n')
        uuid=bytearray(hashlib.md5(b'OfflinePlayer:VanillaProbe').digest())
        uuid[6]=(uuid[6]&15)|48;uuid[8]=(uuid[8]&63)|128
        command=[str(args.java_home/'bin/java'),'-Xms128M','-Xmx768M',
          '-Djava.library.path='+launch['natives'],'-Dorg.lwjgl.system.SharedLibraryExtractPath='+launch['natives'],
          '-Dio.netty.native.workdir='+launch['natives'],'-Djava.awt.headless=false',
          '-cp',os.pathsep.join(launch['classpath']),launch['main_class'],
          '--username','VanillaProbe','--version','1.21','--gameDir',str(game),
          '--assetsDir',str(directory/'assets'),'--assetIndex',launch['asset_index'],
          '--uuid',uuid.hex(),'--accessToken','0','--userType','msa','--versionType','release',
          '--quickPlayPath',str(directory/'quick-play.json'),'--quickPlayMultiplayer',f'127.0.0.1:{args.port}',
          '--width','1024','--height','768']
        client_path=evidence/'vanilla-client-runtime.log'
        client_log=client_path.open('w');files.append(client_log)
        client=subprocess.Popen(command,cwd=directory,env=env,stdout=client_log,stderr=subprocess.STDOUT)
        children.append(client)
        deadline=time.monotonic()+args.timeout
        joined=None
        while True:
            server_text=server_path.read_text(errors='replace')
            client_text=client_path.read_text(errors='replace')
            if client.poll() is not None:raise RuntimeError('Official Java client exited: '+client_text[-6000:])
            if '"event":"play_join"' in server_text and '"username":"VanillaProbe"' in server_text:
                if joined is None:joined=time.monotonic()
                if time.monotonic()-joined>=10:
                    report['server_play_join']=True
                    report['connected_for_seconds']=round(time.monotonic()-joined,2)
                    report['client_alive']=True
                    report['client_connecting_log']='Connecting to 127.0.0.1' in client_text
                    failures=[line for line in client_text.splitlines() if any(token in line for token in ['Failed to handle packet','Error executing task on Client','Exception in thread'])]
                    if failures:raise RuntimeError('Client packet/runtime errors: '+'\n'.join(failures[-8:]))
                    screen=directory/'vanilla-client-proof.png'
                    shot=subprocess.run(['ffmpeg','-y','-nostdin','-loglevel','error','-f','x11grab','-video_size','1024x768',
                                         '-i',env['DISPLAY'],'-frames:v','1',str(screen)],env=env,text=True,capture_output=True,timeout=15)
                    report['screenshot']=str(screen) if shot.returncode==0 else None
                    report['screenshot_error']=shot.stderr if shot.returncode else None
                    report['ok']=True
                    break
            if time.monotonic()>deadline:raise TimeoutError('Official Java client did not reach a stable PLAY session: '+client_text[-6000:])
            time.sleep(.3)
    except Exception as error:
        report['error']=str(error)
    finally:
        for child in reversed(children):
            if child.poll() is None:
                child.send_signal(signal.SIGTERM)
                try:child.wait(timeout=5)
                except subprocess.TimeoutExpired:child.kill();child.wait()
        for file in files:file.close()
        (evidence/'vanilla-client-conformance.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report,indent=2))
    return 0 if report['ok'] else 1


if __name__=='__main__':raise SystemExit(main())
