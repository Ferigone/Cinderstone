#!/usr/bin/env python3
"""Fetch an unmodified official 1.21 Java client for a local rendering check.

The copyright-owned client and assets remain external test inputs. Audio is
omitted by default; it is unnecessary for the network/render compatibility check.
"""
from __future__ import annotations
import argparse
import concurrent.futures
import hashlib
import json
from pathlib import Path
import urllib.request
import zipfile

PROJECT=Path(__file__).resolve().parents[1]


def fetch(url,path,size,sha1):
    path=Path(path)
    if path.is_file() and path.stat().st_size==size and hashlib.sha1(path.read_bytes()).hexdigest()==sha1:
        return path
    path.parent.mkdir(parents=True,exist_ok=True)
    temporary=path.with_suffix(path.suffix+'.partial')
    with urllib.request.urlopen(url,timeout=60) as source,temporary.open('wb') as target:
        while block:=source.read(1024*1024):target.write(block)
    if temporary.stat().st_size!=size or hashlib.sha1(temporary.read_bytes()).hexdigest()!=sha1:
        raise RuntimeError('Client input checksum or size mismatch: '+str(path))
    temporary.replace(path)
    return path


def library_allowed(library):
    rules=library.get('rules',[])
    allowed=not rules
    for rule in rules:
        operating=rule.get('os',{})
        if operating.get('name','linux')!='linux':continue
        if operating.get('arch','x86_64') not in ('x86_64','amd64'):continue
        if rule.get('features'):continue
        allowed=rule['action']=='allow'
    return allowed


def bootstrap(metadata,directory,audio=False):
    metadata=Path(metadata).resolve()
    source_lock=json.loads((PROJECT/'fixtures/source-lock.json').read_text())
    expected_metadata=source_lock['metadata_sha1']
    if not metadata.is_file():
        url=f'https://piston-meta.mojang.com/v1/packages/{expected_metadata}/1.21.json'
        raw=urllib.request.urlopen(url,timeout=45).read()
        if hashlib.sha1(raw).hexdigest()!=expected_metadata:
            raise ValueError('Official client metadata digest mismatch')
        metadata.parent.mkdir(parents=True,exist_ok=True)
        metadata.write_bytes(raw)
    raw=metadata.read_bytes()
    if hashlib.sha1(raw).hexdigest()!=expected_metadata:
        raise ValueError('The exact source-locked Mojang metadata is required')
    version=json.loads(raw)
    if version['id']!='1.21':raise ValueError('Only the exact 1.21 client is accepted')
    directory=Path(directory).resolve()
    directory.mkdir(parents=True,exist_ok=True)
    client=version['downloads']['client']
    pending=[(client['url'],directory/'client-1.21.jar',client['size'],client['sha1'])]
    classpath=[]
    for library in version['libraries']:
        if not library_allowed(library):continue
        artifact=library['downloads']['artifact']
        path=directory/'libraries'/artifact['path']
        pending.append((artifact['url'],path,artifact['size'],artifact['sha1']))
        classpath.append(str(path))
    index=version['assetIndex']
    index_path=fetch(index['url'],directory/'assets/indexes'/f"{index['id']}.json",index['size'],index['sha1'])
    assets=json.loads(index_path.read_text())
    skipped=0
    for key,asset in assets['objects'].items():
        if not audio and ('/sounds/' in key or key.endswith('.ogg')):
            skipped+=1
            continue
        digest=asset['hash']
        pending.append((f'https://resources.download.minecraft.net/{digest[:2]}/{digest}',
                        directory/'assets/objects'/digest[:2]/digest,asset['size'],digest))
    print(json.dumps({'action':'download-client-inputs','files':len(pending),'bytes':sum(item[2] for item in pending),'skipped_audio_assets':skipped}),flush=True)
    with concurrent.futures.ThreadPoolExecutor(max_workers=12) as pool:
        list(pool.map(lambda entry:fetch(*entry),pending))
    natives=directory/'natives'
    natives.mkdir(exist_ok=True)
    for path in classpath:
        if 'natives-linux' not in path:continue
        with zipfile.ZipFile(path) as jar:
            for entry in jar.infolist():
                if entry.filename.endswith('.so'):
                    (natives/Path(entry.filename).name).write_bytes(jar.read(entry))
    classpath.append(str(directory/'client-1.21.jar'))
    launch={'minecraft':'1.21','main_class':version['mainClass'],'metadata_sha1':hashlib.sha1(raw).hexdigest(),
            'client_sha1':client['sha1'],'classpath':classpath,'natives':str(natives),'asset_index':index['id'],
            'skipped_audio_assets':skipped,'client_modified':False}
    (directory/'launch.json').write_text(json.dumps(launch,indent=2)+'\n')
    return launch


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--metadata',type=Path,default=PROJECT.parent/'mc121-oracle/version.json')
    parser.add_argument('--client-dir',type=Path,default=PROJECT.parent/'mc121-client')
    parser.add_argument('--audio',action='store_true')
    args=parser.parse_args()
    result=bootstrap(args.metadata,args.client_dir,args.audio)
    print(json.dumps({'ok':True,'minecraft':result['minecraft'],'client_sha1':result['client_sha1'],'client_modified':False}))
