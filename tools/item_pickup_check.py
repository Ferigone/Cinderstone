#!/usr/bin/env python3
"""Replay frozen original playerTouch/Inventory.add timing, then native path."""
from pathlib import Path
import hashlib,json,os,re,subprocess
from urllib.parse import unquote,urlparse
ROOT=Path(__file__).resolve().parents[1];ORACLE=ROOT.parent/'mc121-oracle';JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin');SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
def main():
    jar=ORACLE/'server-inner-1.21.jar';assert hashlib.sha256(jar.read_bytes()).hexdigest()==SHA
    fixture=ROOT/'fixtures/item-pickup-vectors.json';data=json.loads(fixture.read_text());assert data['oracle_inner_sha256']==SHA
    runtime=ROOT/'runtime/item-pickup';runtime.mkdir(parents=True,exist_ok=True);cp=os.pathsep.join(map(str,[runtime,jar,*sorted((ORACLE/'libraries').rglob('*.jar'))]));flags=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED']
    helper_sources=['tools/survival_oracle.java','tools/item_hurt_oracle.java','tools/container_oracle.java','tools/item_pickup_oracle.java']
    compile=subprocess.run([JDK/'javac',*flags,'-proc:none','-cp',cp,'-d',runtime,*[ROOT/p for p in helper_sources]],text=True,capture_output=True)
    if compile.returncode:raise RuntimeError(compile.stdout+compile.stderr)
    original=subprocess.run([JDK/'java',*flags,'-Xmx1G','-cp',cp,'oracle.item_pickup_oracle'],input=''.join(json.dumps(row['query'])+'\n'for row in data['cases']),text=True,capture_output=True)
    (runtime/'replay.log').write_text(original.stdout+original.stderr)
    if original.returncode:raise RuntimeError((original.stdout+original.stderr)[-7000:])
    origins=[s[7:]for s in original.stdout.splitlines()if s.startswith('SOURCE:')];assert len(origins)==2 and all(Path(unquote(urlparse(s).path)).resolve()==jar.resolve()for s in origins)
    actual=[json.loads(s[7:])for s in original.stdout.splitlines()if s.startswith('ORACLE:')];assert len(actual)==len(data['cases'])
    for index,(row,value)in enumerate(zip(data['cases'],actual)):assert row['expected']==value,f'original replay changed case{index}'
    env=os.environ.copy();env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup',CARGO_INCREMENTAL='0')
    native=subprocess.run(['/root/.local/share/mc-rust-toolchain/cargo/bin/cargo','test','--lib','item_entity::pickup_tests','--','--nocapture'],cwd=ROOT,env=env,text=True,capture_output=True);(runtime/'native.log').write_text(native.stdout+native.stderr)
    if native.returncode:raise RuntimeError((native.stdout+native.stderr)[-7000:])
    assert int(re.search(r'test result: ok\. (\d+) passed',native.stdout).group(1))==1
    reports=[json.loads(line.removeprefix('ITEM_PICKUP_CONFORMANCE:'))for line in native.stdout.splitlines()if line.startswith('ITEM_PICKUP_CONFORMANCE:')];assert len(reports)==1;report=reports[0];assert report['ok']and report['mismatches']==0
    report.update(minecraft='1.21',oracle_inner_sha256=SHA,native_tests_passed=1,kind='original-class runtime differential conformance',not_verified=['Actual ServerPlayer.take/broadcastChanges packets and remote-menu baseline integration','Real stat counters/advancements and live client collection animation','Player spatial query/connection iteration order in this corpus'])
    sources=[*helper_sources,'tools/item_pickup_generate.py','tools/item_pickup_check.py','tools/item_pickup_tests.rs','fixtures/item-pickup-vectors.json','src/item_entity.rs','src/container.rs','src/inventory.rs']
    report['source_sha256']={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest()for p in sources};(ROOT/'evidence/item-pickup-conformance.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps({k:v for k,v in report.items()if k not in ['source_sha256','not_verified','scope']}))
if __name__=='__main__':main()
