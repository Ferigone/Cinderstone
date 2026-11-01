#!/usr/bin/env python3
"""Pin the phase5 EMPTY item-runtime plumbing milestone and live regressions."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess

ROOT=Path(__file__).resolve().parents[1]
RUNTIME=ROOT/'runtime/phase5-empty'
PROBES=[
    ('entity',['tools/entity_live_probe.mjs','--equipment','--tracking-window','--port','25601']),
    ('container',['tools/container_live_probe.mjs','--port','25602']),
    ('duplicate',['tools/duplicate_login_live_probe.mjs','--port','25603','--replacements','3']),
    ('creative',['tools/creative_live_probe.mjs','--port','25604','--label','phase5']),
    ('placement',['tools/placement_live_probe.mjs','--port','25605','--label','phase5']),
    ('drop-safety',['tools/phase5_empty_drop_live_probe.mjs','--port','25606']),
]


def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()


def parse_report(path):
    for line in reversed(path.read_text().splitlines()):
        try:result=json.loads(line)
        except json.JSONDecodeError:continue
        if isinstance(result,dict)and 'ok'in result:return result
    raise RuntimeError('No probe report in '+str(path))


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run',action='store_true',help='build, run glue tests and all six live probes')
    args=parser.parse_args()
    RUNTIME.mkdir(parents=True,exist_ok=True);evidence=ROOT/'evidence/phase5-empty';evidence.mkdir(parents=True,exist_ok=True)
    binary=RUNTIME/'mc121-rust'
    owned=['src/main.rs','tools/main_item_plumbing_tests.rs','tools/phase5_empty_drop_live_probe.mjs']
    before={name:sha(ROOT/name)for name in owned}
    env=os.environ.copy();env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup',CARGO_INCREMENTAL='0')
    env['PATH']=env['CARGO_HOME']+'/bin:'+env.get('PATH','')
    if args.run:
        cargo=str(Path(env['CARGO_HOME'])/'bin/cargo')
        for label,command in [('tests',[cargo,'test','--bin','mc121-rust','phase5','--','--nocapture']),
                              ('build',[cargo,'build','--bin','mc121-rust'])]:
            result=subprocess.run(command,cwd=ROOT,env=env,text=True,capture_output=True)
            (ROOT/f'runtime/main-phase5-{label}.log').write_text(result.stdout+result.stderr)
            if result.returncode:raise RuntimeError((result.stdout+result.stderr)[-10000:])
        temporary=RUNTIME/'mc121-rust.new';shutil.copyfile(ROOT/'target/debug/mc121-rust',temporary);temporary.chmod(0o755);temporary.replace(binary)
        for name,arguments in PROBES:
            result=subprocess.run(['node',arguments[0],'--server-binary',str(binary),*arguments[1:]],cwd=ROOT,text=True,capture_output=True,timeout=90)
            (RUNTIME/f'{name}-probe.log').write_text(result.stdout+result.stderr)
            if result.returncode:raise RuntimeError((result.stdout+result.stderr)[-10000:])
    testlog=(ROOT/'runtime/main-phase5-tests.log').read_text()
    match=re.search(r'test result: ok\. (\d+) passed',testlog)
    assert match and int(match.group(1))==5
    for required in ['observer_pairing_item_transients_and_effect_recipients',
                     'lag_snapshot_suppresses_old_state_but_preserves_visible_transient',
                     'same_chunk_observer_move_discovers_previously_uncached_item',
                     'accept_order_and_confirmations_are_separate_from_ticking_range',
                     'unavailable_nonempty_drop_rolls_back_menu_and_publisher']:
        assert f'test item_plumbing_tests::phase5_{required} ... ok'in testlog
    reports={}
    for name,_ in PROBES:
        report=parse_report(RUNTIME/f'{name}-probe.log');assert report['ok'],report
        assert all(report['checks'].values()),report
        if 'server_binary_sha256'in report:assert report['server_binary_sha256']==sha(binary)
        reports[name]=report
        (evidence/f'{name}.json').write_text(json.dumps(report,indent=2)+'\n')
    assert before=={name:sha(ROOT/name)for name in owned},'owned sources changed during verification'
    for name in ['src/main.rs','tools/main_item_plumbing_tests.rs']:
        shutil.copyfile(ROOT/name,RUNTIME/Path(name).name)
    report=dict(ok=True,minecraft='1.21',protocol=767,milestone='empty authoritative item-runtime plumbing',
        full_runtime_parity=False,active_item_runtime=False,
        world_item_simulation_active=False,connection_phase_pickup_active=False,
        drop_actions='Nonempty Q/CtrlQ/menu/creative drops roll back and correct the client until world capabilities exist; EMPTY attempts consume no ID/RNG/entity',
        glue_tests=dict(passed=5,failed=0),
        live_checks={name:len(result['checks'])for name,result in reports.items()},
        server_binary_sha256=sha(binary),owned_sources_sha256=before,
        dependency_sources_sha256_at_recording={name:sha(ROOT/name)for name in [
            'src/entity.rs','src/entity_ticking_range.rs','src/entity_tracking_order.rs',
            'src/replication.rs','src/runtime_entities.rs','src/player_inventory.rs',
            'src/item_entity.rs','src/random_seed.rs','src/container.rs','src/inventory.rs',
        ]},
        established=['RuntimeEntities initialized with the actual Level random factory and shared PlayerEvent publisher',
            'Actual retained player-constructor RNG allocated with player entity ID under registry lock',
            'Shared player/item sender iteration via publish_tracking_with_players',
            'Mandatory observers, paired pickup transients, recipient-captured effects, update/removal barriers',
            'Atomic player/item/private-menu lag recovery; same-chunk item view-entry reevaluation',
            'Literal ChunkHolder confirmed-status callback; no ticket/network visibility heuristic',
            'Accepted-connection sequence preserved independently from PLAY completion/entity ID',
            'Existing player, window0, placement, UUID replacement and world-time probes retained'],
        blockers=['Active loading/lifecycle/readiness manager integration is still in progress',
            'Actual world weather/context facts and required gameplay callbacks remain explicit prerequisites',
            'World item simulation, connection-phase Player.aiStep pickup and atomic drop/menu commit are not active',
            'Full ServerPlayer.tick, advancements, mobs and the rest of vanilla gameplay are not established'],
        reproduce='python3 tools/main_phase5_empty_check.py --run')
    (ROOT/'evidence/main-phase5-empty-conformance.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report,indent=2))


if __name__=='__main__':main()
