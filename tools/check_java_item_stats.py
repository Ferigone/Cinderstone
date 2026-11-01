#!/usr/bin/env python3
"""Reuse frozen stock actions, then compare requested item stats on Java/Rust."""
from pathlib import Path
import argparse, hashlib, json, os, shutil, subprocess, tempfile, time, uuid

ROOT = Path(__file__).resolve().parents[1]
ORACLE = ROOT.parent / 'mc121-oracle'
JAVA = Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin/java')
NAMES = ['ItemDropper', 'ItemObserver']
TARGETS = {'ItemDropper': {(5, 1): 8, (8, 22): 2}, 'ItemObserver': {(4, 1): 8}}
OUTER_SHA1 = '450698d1863ab5180c25d7c804ef0fe6369dd1ba'
INNER_SHA256 = '8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def offline_uuid(name):
    raw = bytearray(hashlib.md5(('OfflinePlayer:' + name).encode()).digest())
    raw[6] = (raw[6] & 15) | 48
    raw[8] = (raw[8] & 63) | 128
    return str(uuid.UUID(bytes=bytes(raw)))


def world_digests(world):
    return {str(path.relative_to(world)): digest(path) for path in sorted(world.rglob('*')) if path.is_file()}


def initial_facts(world):
    result = {}
    for name in NAMES:
        identity = offline_uuid(name)
        paths = [f'stats/{identity}.json', f'stats/{name}.json',
                 f'playerdata/{identity}.dat', f'advancements/{identity}.json']
        result[name] = dict(uuid=identity, observed_files={path: (digest(world / path) if (world / path).is_file() else None) for path in paths})
        assert all(value is None for value in result[name]['observed_files'].values()), 'canonical test identities must be fresh; do not reuse a prior successful private world'
    return result


def wait_started(process, log, marker, timeout):
    deadline = time.monotonic() + timeout
    while marker not in log.read_text(errors='replace'):
        if process.poll() is not None:
            raise RuntimeError(f'server exited {process.returncode}: ' + log.read_text(errors='replace')[-5000:])
        if time.monotonic() >= deadline:
            raise TimeoutError('server startup: ' + log.read_text(errors='replace')[-5000:])
        time.sleep(.15)


def run(command, output, timeout=120):
    result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True, timeout=timeout)
    output.write_text(result.stdout + result.stderr)
    if result.returncode:
        raise RuntimeError(f'command exited {result.returncode}: ' + (result.stdout + result.stderr)[-7000:])
    return result


def query(directory, port, phase):
    output = directory / (phase + '.json')
    run(['node', str(ROOT / 'tools/item_stats_query_probe.mjs'), '--port', str(port),
         '--phase', phase, '--output', str(output)], output.with_suffix('.log'))
    value = json.loads(output.read_text())
    assert value['ok']
    for player in value['players']:
        assert player['uuid'] == offline_uuid(player['username'])
        assert player['unsolicited_before_requests'] == 0
        assert player['collect_packets'] == [], 'query-only baseline/session must not perform a pickup'
    return value


def selected(player, request):
    packet = player['statistics'][player['requests'][request]['response_index']]
    keys = TARGETS[player['username']]
    return {(row['categoryId'], row['statisticId']): row['value'] for row in packet
            if (row['categoryId'], row['statisticId']) in keys}


def encoded_keys(values):
    return [{'type_id': kind, 'value_id': value, 'counter': count} for (kind, value), count in sorted(values.items())]


def check_sessions(before, after, reconnect):
    result = {}
    for baseline, current, again in zip(before['players'], after['players'], reconnect['players']):
        name = baseline['username']
        assert current['username'] == again['username'] == name
        # Missing packet entries are NOT current-value zeros. The semantic
        # initial zero is separately justified by observed fresh identities,
        # empty counter constructors and absence of any baseline item action.
        assert selected(baseline, 0) == {}, 'fresh baseline unexpectedly has a target item statistic'
        assert selected(baseline, 1) == {}
        assert selected(current, 0) == TARGETS[name], ('post-stock explicit counters', name, selected(current, 0))
        assert selected(current, 1) == {}, 'same connection must drain target dirty keys'
        assert selected(again, 0) == TARGETS[name], 'new successful login must mark existing values dirty again'
        assert selected(again, 1) == {}
        result[name] = dict(uuid=baseline['uuid'], initial_semantic_values=encoded_keys({key: 0 for key in TARGETS[name]}),
                            baseline_wire_state='target keys NOT_PRESENT/NOT_DIRTY; never inferred as value0 from this packet',
                            post_stock_explicit_values=encoded_keys(selected(current, 0)),
                            immediate_repeat_target_entries=encoded_keys(selected(current, 1)),
                            reconnect_after_drain_explicit_values=encoded_keys(selected(again, 0)),
                            reconnect_immediate_repeat_target_entries=encoded_keys(selected(again, 1)))
    return result


def server_run(kind, directory, port, binary, stock, label):
    directory.mkdir(parents=True)
    log = directory / 'server.log'
    if kind == 'java':
        shutil.copytree(ORACLE / 'world', directory / 'world', copy_function=shutil.copyfile)
        shutil.copyfile(ORACLE / 'eula.txt', directory / 'eula.txt')
        updates = {'online-mode': 'false', 'gamemode': 'creative', 'force-gamemode': 'true',
                   'view-distance': '2', 'simulation-distance': '2', 'spawn-protection': '0',
                   'max-players': '20', 'level-name': 'world', 'enforce-secure-profile': 'false'}
        lines = []
        for line in (ORACLE / 'server.properties').read_text().splitlines():
            key = line.split('=', 1)[0]
            lines.append(key + '=' + updates.pop(key) if key in updates else line)
        lines += [key + '=' + value for key, value in updates.items()]
        (directory / 'server.properties').write_text('\n'.join(lines) + '\n')
        command = [JAVA, '-Xms128M', '-Xmx1024M', '-jar', ORACLE / 'server-1.21.jar', '--port', str(port), 'nogui']
        cwd, marker, startup = directory, 'Done (', 180
    else:
        command = [binary, '--allow-offline', '--port', str(port), '--view-distance', '2', '--world', directory / 'native-world.json']
        cwd, marker, startup = ROOT, '"event":"listening"', 30
    print(json.dumps({'phase': kind + ' startup', 'directory': str(directory)}), flush=True)
    stream = log.open('w')
    process = subprocess.Popen(list(map(str, command)), cwd=cwd, stdin=subprocess.PIPE, stdout=stream, stderr=subprocess.STDOUT)
    try:
        wait_started(process, log, marker, startup)
        before = query(directory, port, 'before_stock')
        stock_label = kind + '_stats_stock_' + label
        print(json.dumps({'phase': kind + ' frozen stock probe'}), flush=True)
        run(['node', stock, '--scenario', 'stock', '--host', '127.0.0.1', '--port', str(port), '--label', stock_label],
            directory / 'stock.log', timeout=150)
        actual_stock = json.loads((ROOT / 'evidence' / ('item-live-' + stock_label + '.json')).read_text())
        assert actual_stock['ok'] and len(actual_stock['checks']) == 7 and all(actual_stock['checks'].values())
        time.sleep(.25)
        after = query(directory, port, 'after_stock')
        reconnect = query(directory, port, 'reconnect_after_drain')
        comparison = check_sessions(before, after, reconnect)
        return dict(ok=True, directory=str(directory), stock_report=str(ROOT / 'evidence' / ('item-live-' + stock_label + '.json')),
                    stock_checks=actual_stock['checks'], statistics=comparison,
                    raw_query_reports=[str(directory / (phase + '.json')) for phase in ['before_stock', 'after_stock', 'reconnect_after_drain']])
    finally:
        if process.poll() is None:
            if kind == 'java':
                process.stdin.write(b'stop\n')
                process.stdin.flush()
            else:
                process.send_signal(2)
            try:
                process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
        stream.close()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--server-binary', default='target/debug/mc121-rust')
    parser.add_argument('--port', type=int, default=25813)
    args = parser.parse_args()
    binary = (ROOT / args.server_binary).resolve()
    assert binary.is_file()
    assert hashlib.sha1((ORACLE / 'server-1.21.jar').read_bytes()).hexdigest() == OUTER_SHA1
    assert digest(ORACLE / 'server-inner-1.21.jar') == INNER_SHA256
    reference_hashes = world_digests(ORACLE / 'world')
    initial = initial_facts(ORACLE / 'world')
    directory = Path(tempfile.mkdtemp(prefix='item-stats-next-stage-', dir=ROOT / 'runtime'))
    pinned_binary = directory / 'mc121-rust'
    shutil.copy2(binary, pinned_binary)
    stock = ROOT / 'runtime' / ('item-live-probe-frozen-' + uuid.uuid4().hex + '.mjs')
    shutil.copyfile(ROOT / 'tools/item_live_probe.mjs', stock)
    label = directory.name.rsplit('-', 1)[-1]
    report = dict(ok=False, minecraft='1.21', protocol=767, stage='post-v5-login-dirty-reconnect',
                  scope='Frozen stock8-item/two-drop actions plus explicit requested target statistics on same-process UUID reconnects. Compare target keyed maps, not unrelated vanilla tick stats or packet order.',
                  native_binary_sha256=digest(pinned_binary), oracle_inner_sha256=INNER_SHA256,
                  oracle_outer_sha1=OUTER_SHA1, runtime_directory=str(directory), initial_java_identity_files=initial,
                  initial_semantic_zero_authority='Observed UUID and legacy stat files/player data absent in the canonical source copied into a fresh Java process; original StatsCounter constructor/default map supplies0. Native starts a fresh process with owned empty counters and a nonexistent fixture path. Query-only baseline performs no item actions. Packet absence itself does not establish value0.',
                  not_verified=['Complete player tick/movement statistics and full packet-map equality',
                                'Native stat-file/restart persistence, advancement or recipe-book parity',
                                'Random trajectory equality or controlled merge'])
    try:
        report['rust'] = server_run('rust', directory / 'rust', args.port, pinned_binary, str(stock), label)
        report['java'] = server_run('java', directory / 'java', args.port + 1, pinned_binary, str(stock), label)
        assert report['rust']['statistics'] == report['java']['statistics']
        report['target_statistic_differences'] = 0
        report['reference_world_unchanged'] = reference_hashes == world_digests(ORACLE / 'world')
        assert report['reference_world_unchanged']
        report['ok'] = True
    except Exception as error:
        report['error'] = repr(error)
    finally:
        report['source_sha256'] = {str(path.relative_to(ROOT)): digest(path) for path in [ROOT / 'tools/check_java_item_stats.py', ROOT / 'tools/item_stats_query_probe.mjs', ROOT / 'tools/item_live_probe.mjs']}
        report['frozen_stock_probe_sha256'] = digest(stock)
        assert report['frozen_stock_probe_sha256'] == digest(ROOT / 'tools/item_live_probe.mjs')
        (ROOT / 'evidence/item-stats-java-rust-live-conformance.json').write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps(report), flush=True)
    raise SystemExit(0 if report['ok'] else 1)


if __name__ == '__main__':
    main()
