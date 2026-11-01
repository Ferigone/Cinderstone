#!/usr/bin/env python3
"""Package a pinned, validated source tree and matching native binary."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import zipfile

ROOT=Path(__file__).resolve().parents[1]

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--output',type=Path,default=ROOT.parent/'mc121-rust-checkpoint.zip')
    p.add_argument('--tests',type=int,required=True)
    args=p.parse_args()
    binary=ROOT/'target/release/mc121-rust'
    digest=hashlib.sha256(binary.read_bytes()).hexdigest()
    for name in ['native-conformance.json','vanilla-client-conformance.json']:
        report=json.loads((ROOT/'evidence'/name).read_text())
        assert report['ok'] and report['server_binary_sha256']==digest,'stale/failed report: '+name
    log=(ROOT/'evidence/release-tests.log').read_text()
    summaries=re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',log)
    assert summaries and sum(int(row[0]) for row in summaries)==args.tests,'release test total differs'
    assert all(int(failed)==0 and int(ignored)==0 for _,failed,ignored in summaries) and 'FAILED' not in log
    shutil.copyfile(binary,ROOT/'bin/mc121-rust-linux-x86_64')
    (ROOT/'bin/mc121-rust-linux-x86_64').chmod(0o755)
    tracked=subprocess.check_output(['git','ls-files','-z'],cwd=ROOT).decode().split('\0')
    paths={Path(name) for name in tracked if name}
    paths.update(path.relative_to(ROOT) for path in (ROOT/'third_party').rglob('*') if path.is_file())
    paths.update([Path('tools/package_checkpoint.py'),Path('tools/collect_licenses.py'),Path('evidence/release-tests.log')])
    invalid=[str(path) for path in paths if (ROOT/path).is_symlink() or not (ROOT/path).is_file()]
    if invalid:
        raise RuntimeError('Checkpoint entries must be regular local files: '+', '.join(sorted(invalid)))
    manifest={'minecraft':'1.21','protocol':767,'full_vanilla_parity':False,
              'source_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
              'build':'CARGO_INCREMENTAL=0 cargo build --release --locked --bin mc121-rust',
              'rustc':subprocess.check_output(['rustc','--version'],text=True).strip(),
              'release_tests_passed':args.tests,'release_test_binaries':len(summaries),'binary_sha256':digest,
              'files':{str(path):hashlib.sha256((ROOT/path).read_bytes()).hexdigest() for path in sorted(paths)}}
    (ROOT/'checkpoint.json').write_text(json.dumps(manifest,indent=2)+'\n')
    paths.add(Path('checkpoint.json'))
    temporary=args.output.with_suffix('.zip.partial')
    with zipfile.ZipFile(temporary,'w',compression=zipfile.ZIP_DEFLATED,compresslevel=9) as archive:
        for path in sorted(paths):archive.write(ROOT/path,'mc121-rust/'+str(path))
    with zipfile.ZipFile(temporary) as archive:assert archive.testzip() is None
    temporary.replace(args.output)
    print(json.dumps({'file':str(args.output.resolve()),'bytes':args.output.stat().st_size,'files':len(paths),'sha256':hashlib.sha256(args.output.read_bytes()).hexdigest(),'binary_sha256':digest}))

if __name__=='__main__':main()
