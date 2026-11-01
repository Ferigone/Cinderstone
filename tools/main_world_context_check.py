#!/usr/bin/env python3
"""Original NbtIo-prepared private-world integration proof for Main WorldContext."""
from pathlib import Path
import hashlib
import json
import os
import shutil
import subprocess
import tempfile

ROOT=Path(__file__).resolve().parents[1]
ORACLE=ROOT.parent/'mc121-oracle'
JDK=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
RUNTIME=ROOT/'runtime/phase5-world-context'
SHA='8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'


def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()


def world_digest(path):
    files={str(file.relative_to(path)):sha(file)for file in sorted(path.rglob('*'))if file.is_file()}
    return hashlib.sha256(json.dumps(files,sort_keys=True,separators=(',',':')).encode()).hexdigest()


def run(command,log,**kwargs):
    result=subprocess.run(command,cwd=ROOT,text=True,capture_output=True,**kwargs)
    log.write_text(result.stdout+result.stderr)
    if result.returncode:raise RuntimeError((result.stdout+result.stderr)[-10000:])
    return result


def main():
    RUNTIME.mkdir(parents=True,exist_ok=True)
    env=os.environ.copy();env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup',CARGO_INCREMENTAL='0')
    env['PATH']=env['CARGO_HOME']+'/bin:'+env.get('PATH','')
    cargo=str(Path(env['CARGO_HOME'])/'bin/cargo')
    tests=run([cargo,'test','--bin','mc121-rust','phase5','--','--nocapture'],RUNTIME/'tests.log',env=env)
    assert 'test result: ok. 6 passed'in tests.stdout
    run([cargo,'build','--bin','mc121-rust'],RUNTIME/'build.log',env=env)
    binary=RUNTIME/'mc121-rust';temporary=RUNTIME/'mc121-rust.new';shutil.copyfile(ROOT/'target/debug/mc121-rust',temporary);temporary.chmod(0o755);temporary.replace(binary)
    jar=ORACLE/'server-inner-1.21.jar';assert sha(jar)==SHA
    original=ORACLE/'world';before=world_digest(original)
    private=Path(tempfile.mkdtemp(prefix='anvil-',dir=RUNTIME))
    # A real copy with fresh mtimes, never a hardlink/symlink to oracle data.
    shutil.copytree(original,private/'world',copy_function=shutil.copyfile)
    world=private/'world'
    classes=RUNTIME/'classes';classes.mkdir(exist_ok=True)
    cp=os.pathsep.join(map(str,[classes,jar,*sorted((ORACLE/'libraries').rglob('*.jar'))]))
    run([JDK/'javac','-proc:none','-cp',cp,'-d',classes,ROOT/'tools/main_world_nbt_oracle.java'],RUNTIME/'javac.log')
    prepared=run([JDK/'java','-cp',cp,'oracle.main_world_nbt_oracle','prepare',world],RUNTIME/'prepare.log')
    parse=lambda text:json.loads(next(line.split('WORLD_NBT:',1)[1]for line in text.splitlines()if line.startswith('WORLD_NBT:')))
    expected=parse(prepared.stdout)
    probe=run(['node','tools/main_world_context_live_probe.mjs','--server-binary',binary,'--anvil-world',world],RUNTIME/'probe.log',timeout=90)
    live=json.loads(probe.stdout.splitlines()[-1]);assert live['ok'],live
    inspected=run([JDK/'java','-cp',cp,'oracle.main_world_nbt_oracle','inspect',world],RUNTIME/'inspect.log')
    actual=parse(inspected.stdout)
    assert actual==expected,(expected,actual)
    assert actual['unknown_utf16']==[65,0,55296,90]
    assert before==world_digest(original),'Original oracle world changed'
    report=dict(ok=True,minecraft='1.21',protocol=767,full_runtime_parity=False,
        oracle_inner_sha256=SHA,glue_tests=dict(passed=6,failed=0),
        live_checks=live['checks'],saved_weather_and_marker_fields_match_original_reader=True,
        saved_state=actual,original_world_unchanged=True,original_world_digest=before,
        server_binary_sha256=sha(binary),
        source_sha256={name:sha(ROOT/name)for name in [
            'src/main.rs','src/world_context.rs','src/world_rules.rs','src/random_seed.rs',
            'src/tick.rs','src/level.rs','src/nbt.rs','tools/main_item_plumbing_tests.rs',
            'tools/main_world_nbt_oracle.java','tools/main_world_context_live_probe.mjs',
            'tools/main_world_context_check.py',
        ]},
        established=['Captured registry dimension facts are required instead of world-name defaults',
            'Actual Anvil weather and GameRules drive initial level-info and persisted state',
            'World RNG weather transition occurs after border and before clock/sender phases',
            'Time/weather share a revisioned ordered world outbox and lag-state barrier',
            'Original compressed NbtIo confirms weather and unknown UTF16/long fields after native save'],
        source_phase_refs=dict(minecraft_server='tickServer tickCount++ then tickChildren timeSync before aqu.tick; send chunks after connection/player phases',
            server_level='aqu.tick offsets35 border,52 advanceWeatherCycle,174 tickTime,296 chunkSource',
            initial_level_info='aur.sendLevelInfo time then default spawn, conditional rain1/levels7/8, chunk-load-start13'),
        not_verified=['Active item simulation/pickup and gameplay hooks remain gated',
            'PlayerChunkSender quota/ACK transport and chunk readiness manager are a separate integration',
            'Chunk weather effects such as lightning/ice/snow are not ported by this state-loop proof',
            'Full login/chunk streaming in this private Anvil probe: captured surrounding chunks include unfinished generation',
            'All vanilla initial level-info packets and complete world/server loop parity'],
        reproduce='python3 tools/main_world_context_check.py')
    (ROOT/'evidence/main-world-context-conformance.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report,indent=2))


if __name__=='__main__':main()
