#!/usr/bin/env python3
"""Copy dependency-provided notices for the exact Cargo resolution."""
import json
from pathlib import Path
import shutil
import subprocess

ROOT=Path(__file__).resolve().parents[1]

def main():
    metadata=json.loads(subprocess.check_output(['cargo','metadata','--locked','--format-version','1'],cwd=ROOT,text=True))
    selected={node['id'] for node in metadata['resolve']['nodes']}
    target=ROOT/'third_party/licenses'
    target.mkdir(parents=True,exist_ok=True)
    records=[]
    for package in sorted(metadata['packages'],key=lambda p:(p['name'],p['version'])):
        if package['id'] not in selected or package['source'] is None:continue
        source=Path(package['manifest_path']).parent
        files=set()
        for pattern in ['LICENSE*','LICENCE*','COPYING*','NOTICE*','UNLICENSE*']:
            files.update(p for p in source.glob(pattern) if p.is_file())
        if package.get('license_file'):
            p=source/package['license_file']
            if p.is_file():files.add(p)
        if package['name']=='openssl-src':
            files.update(p for p in (source/'openssl').glob('LICENSE*') if p.is_file())
        destination=target/(package['name']+'-'+package['version'])
        destination.mkdir(exist_ok=True)
        names=[]
        for path in sorted(files):
            relative=path.relative_to(source) if path.is_relative_to(source) else Path(path.name)
            dest=destination/relative;dest.parent.mkdir(parents=True,exist_ok=True)
            shutil.copyfile(path,dest);names.append(str(relative))
        records.append({'package':package['name'],'version':package['version'],'license':package['license'],'license_files':names})
    (ROOT/'third_party/rust-dependencies.json').write_text(json.dumps(records,indent=2)+'\n')
    # This directory is generated. Removed dependencies must not retain stale
    # notices that make the packaged dependency set appear larger than it is.
    selected_directories={r['package']+'-'+r['version'] for r in records}
    for directory in target.iterdir():
        if directory.is_dir() and directory.name not in selected_directories:
            shutil.rmtree(directory)
    print(json.dumps({'packages':len(records),'notice_files':sum(len(r['license_files']) for r in records),'without_notice_files':[r['package'] for r in records if not r['license_files']]}))

if __name__=='__main__':main()
