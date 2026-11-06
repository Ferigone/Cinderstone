#!/usr/bin/env python3
"""Fresh original replay followed by the bounded binary gameplay adapter tests."""
from pathlib import Path
import argparse, hashlib, json, os, re
from main_item_gameplay_generate import replay, facts, run, SHA

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--release', action='store_true')
    args = parser.parse_args()
    fixture = json.loads((ROOT / 'fixtures/main-item-gameplay-vectors.json').read_text())
    stored_facts = json.loads((ROOT / 'fixtures/main-item-gameplay-facts.json').read_text())
    assert facts() == stored_facts, 'original source/listener/advancement facts drifted'
    observed = replay([row['query'] for row in fixture['cases']])
    assert observed == [row['expected'] for row in fixture['cases']], 'original gameplay fixture drifted'
    env = os.environ.copy()
    env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',
               RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup', CARGO_INCREMENTAL='0')
    native = run(['/root/.local/share/mc-rust-toolchain/cargo/bin/cargo', 'test', *(['--release'] if args.release else []), '--bin', 'mc121-rust',
                  'main_item_gameplay::tests', '--', '--nocapture'], env=env)
    runtime = ROOT / 'runtime/main-item-gameplay'
    (runtime / 'native.log').write_text(native.stdout + native.stderr)
    assert int(re.search(r'test result: ok\. (\d+) passed', native.stdout).group(1)) == 5
    results = {}
    for line in native.stdout.splitlines():
        for name in ['MAIN_ITEM_GAMEPLAY_PICKUP_CONFORMANCE', 'MAIN_ITEM_GAMEPLAY_INVENTORY_CONFORMANCE', 'MAIN_ITEM_GAMEPLAY_RECONNECT_CONFORMANCE']:
            if line.startswith(name + ':'):
                results[name] = json.loads(line.removeprefix(name + ':'))
    assert len(results) == 3 and all(row['ok'] and row['mismatches'] == 0 for row in results.values())
    report = dict(ok=True, minecraft='1.21', kind='bounded native bin adapter differential conformance', native_profile='release' if args.release else 'debug',
                  oracle_inner_sha256=SHA, original_scenes=len(observed), native_tests_passed=5, stage='post-v5-login-dirty-reconnect',
                  pickup=results['MAIN_ITEM_GAMEPLAY_PICKUP_CONFORMANCE'],
                  inventory=results['MAIN_ITEM_GAMEPLAY_INVENTORY_CONFORMANCE'],
                  reconnect=results['MAIN_ITEM_GAMEPLAY_RECONNECT_CONFORMANCE'],
                  source_scope='NativeFixtureEmptyCriteria: fresh native source, only known Item/ServerPlayer dynamic sources, all block changes observed, owned empty statistic objectives and criterion sets. Imported/unowned/nonempty sources return an explicit error.',
                  game_event_chunk_scope='Only source-certified positive chunks after manager quiescence; every query additionally checks healthy/quiescent current FULL. Unknown/unconfirmed/poisoned states are errors, never assumed absent. Original getChunkNow cache, visible-map and general generation-FULL absence semantics are not implemented here.',
                  phase_scope='NativeItemCallbacksOnly', full_server_player_tick=False, full_connection_do_tick=False,
                  default_vanilla_recipe_unlocks_implemented=True, full_gameplay_compatibility=False,
                  prepared_menu_scope='Exclusive gameplay token validates/scans before item construction, leaves authoritative stats/observations unchanged if dropped, and infallibly assigns staged stats then empty-listener observations at commit. Callback inputs derive from the separately original-proved container local baseline.',
                  source_facts=dict(listener_block_states=len(stored_facts['listener_state_ids']),
                                    default_vanilla_advancement_files=stored_facts['default_vanilla_advancement_files'],
                                    default_vanilla_trigger_counts=stored_facts['default_vanilla_trigger_counts'],
                                    exact_stone_inventory_predicates=len(stored_facts['default_vanilla_stone_inventory_examples'])),
                  not_verified=['Loading/registering the vanilla advancement trees other than recipes/** (#26)',
                                'Executing nonempty scoreboard objectives or advancement predicates/awards',
                                'Generic ItemEntity.getOwner cached-reference resolution for arbitrary nonplayer entities or imported UUIDs',
                                'Complete ServerPlayer.tick and connection ServerPlayer.doTick',
                                'Full main transaction and normal player-tick menu scheduling of local listener observations; container/host primitives have separate reports',
                                'One combined original scene of live chunk-manager futures plus gameplay event traversal; the adapter composes their separately proved interfaces',
                                'General original getChunkNow four-entry cache, visible-holder generation status and absent-chunk behavior',
                                'Imported entity/BE/dynamic listener sources and nonempty vibration dispatch',
                                'Live client end-to-end gameplay, stat files and recipe-book persistence'])
    sources = ['src/main_item_gameplay.rs', 'src/player_stats.rs', 'src/game_event_context.rs',
               'src/item_host.rs', 'src/inventory.rs', 'src/runtime_chunk_manager.rs',
               'tools/main_item_gameplay_oracle.java', 'tools/player_stats_oracle.java',
               'tools/main_item_gameplay_generate.py', 'tools/main_item_gameplay_check.py',
               'tools/main_item_gameplay_tests.rs', 'fixtures/main-item-gameplay-facts.json',
               'fixtures/main-item-gameplay-vectors.json', 'tools/game_event_context_factories.json', 'tools/player_stats_login_aur_bytecode.txt']
    report['source_sha256'] = {name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest() for name in sources}
    report_path = ROOT / ('evidence/main-item-gameplay-login-release-conformance.json' if args.release else 'evidence/main-item-gameplay-login-conformance.json')
    report_path.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({key: value for key, value in report.items() if key not in ['source_sha256', 'not_verified']}))


if __name__ == '__main__':
    main()
