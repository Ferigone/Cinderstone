#!/usr/bin/env python3
"""Original awardStat, onItemPickup and empty InventoryChanged execution."""
from pathlib import Path
import hashlib, itertools, json, os, random, subprocess, zipfile

ROOT = Path(__file__).resolve().parents[1]
ORACLE = ROOT.parent / 'mc121-oracle'
JDK = Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
FLAGS = ['--add-exports', 'java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED']
SHA = '8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'


def run(command, **kwargs):
    result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True, **kwargs)
    if result.returncode:
        raise RuntimeError((result.stdout + result.stderr)[-7000:])
    return result


def context():
    jar = ORACLE / 'server-inner-1.21.jar'
    assert hashlib.sha256(jar.read_bytes()).hexdigest() == SHA
    runtime = ROOT / 'runtime/main-item-gameplay'
    runtime.mkdir(parents=True, exist_ok=True)
    cp = os.pathsep.join(map(str, [runtime, jar, *sorted((ORACLE / 'libraries').rglob('*.jar'))]))
    run([JDK / 'javac', *FLAGS, '-proc:none', '-cp', cp, '-d', runtime,
         ROOT / 'tools/player_stats_oracle.java', ROOT / 'tools/main_item_gameplay_oracle.java'])
    return runtime, cp


def replay(queries):
    runtime, cp = context()
    result = run([JDK / 'java', *FLAGS, '-Xmx1G', '-cp', cp, 'oracle.main_item_gameplay_oracle'],
                 input=''.join(json.dumps(query) + '\n' for query in queries))
    (runtime / 'oracle.log').write_text(result.stdout + result.stderr)
    rows = [json.loads(line[7:]) for line in result.stdout.splitlines() if line.startswith('ORACLE:')]
    assert len(rows) == len(queries), (len(rows), len(queries))
    return rows


def facts():
    catalogue_file = ROOT / 'tools/game_event_context_factories.json'
    catalogue = json.loads(catalogue_file.read_text())
    states = sorted({state for block in catalogue['blocks'] for variant in block['factory_variants']
                     if variant.get('provider') and variant.get('factory_listener') is not None
                     for state in variant['state_ids']})
    assert len(states) == 490
    archive = zipfile.ZipFile(ORACLE / 'server-inner-1.21.jar')
    files = [name for name in archive.namelist() if name.startswith('data/minecraft/advancement/') and name.endswith('.json')]
    counts, examples = {}, []
    def stone(value):
        if isinstance(value, str):
            return value == 'minecraft:stone'
        if isinstance(value, list):
            return any(map(stone, value))
        return isinstance(value, dict) and any(map(stone, value.values()))
    for name in files:
        for key, criterion in json.loads(archive.read(name)).get('criteria', {}).items():
            trigger = criterion.get('trigger')
            counts[trigger] = counts.get(trigger, 0) + 1
            if trigger == 'minecraft:inventory_changed' and stone(criterion):
                examples.append(dict(file=name, criterion=key, definition=criterion))
    return dict(minecraft='1.21', oracle_inner_sha256=SHA, block_states=catalogue['states'],
                listener_state_ids=states, listener_factory_catalog_sha256=hashlib.sha256(catalogue_file.read_bytes()).hexdigest(),
                dynamic_item_and_player_listener_registrations=0, default_vanilla_advancement_files=len(files),
                default_vanilla_trigger_counts={key: counts[key] for key in ['minecraft:thrown_item_picked_up_by_entity',
                                               'minecraft:thrown_item_picked_up_by_player', 'minecraft:inventory_changed']},
                default_vanilla_stone_inventory_examples=examples, default_vanilla_recipe_unlocks_implemented=True,
                native_fixture_policy='No unported advancement criteria; the vanilla recipes/** advancements are loaded and evaluated (#27)')


def queries():
    result = [dict(item_id=item, count=count, old_value=old, owner=owner, registry=registry)
              for item, count, old, owner, registry in itertools.product(
                  [0, 1, 993], [-2147483648, -1, 0, 1, 5, 64, 2147483647],
                  [-2147483648, 0, 2147483646, 2147483647], ['none', 'player', 'entity'],
                  ['absent', 'empty_sets', 'other_player'])]
    items = json.loads((ROOT / 'fixtures/items-1.21.json').read_text())['items']
    rng = random.Random(0x7673953ca11)
    def slot(item):
        return dict(item_id=item, count=rng.choice([0, 1, items[str(item)]['stack_size']]))
    inventories = [[dict(item_id=0, count=0) for _ in range(41)],
                   [dict(item_id=1, count=64) for _ in range(41)],
                   [dict(item_id=993, count=16) for _ in range(41)]]
    inventories += [[slot(rng.randrange(1333)) for _ in range(41)] for _ in range(128)]
    for start in range(0, 1333, 41):
        inventories.append([slot(item) for item in range(start, min(1333, start + 41))] +
                           [dict(item_id=0, count=0)] * (41 - min(41, 1333 - start)))
    result += [dict(item_id=1, count=1, old_value=0, owner='none', registry='absent', inventory=inventory)
               for inventory in inventories]
    return result


def atomic_json(path, value):
    temporary = path.with_suffix('.json.tmp')
    temporary.write_text(json.dumps(value, separators=(',', ':')) + '\n')
    temporary.replace(path)


def main():
    actual_facts = facts()
    inputs = queries()
    rows = replay(inputs)
    atomic_json(ROOT / 'fixtures/main-item-gameplay-facts.json', actual_facts)
    atomic_json(ROOT / 'fixtures/main-item-gameplay-vectors.json', dict(
        minecraft='1.21', oracle_inner_sha256=SHA,
        scope='Unchanged ServerPlayer.awardStat/onItemPickup, LivingEntity.onItemPickup, Scoreboard.forAllObjectives, PickedUpItemTrigger, InventoryChangeTrigger and SimpleCriterionTrigger. Controlled world/server/advancement getters; InventoryChangeTrigger wrapper observes original captured count values then forwards unchanged empty listener handling. Not a loaded vanilla advancement catalogue.',
        cases=[dict(query=query, expected=expected) for query, expected in zip(inputs, rows)]))
    print(json.dumps(dict(ok=True, original_cases=len(rows), pickup_cases=sum('inventory' not in query for query in inputs),
                          inventory_listener_cases=sum('inventory' in query for query in inputs),
                          world_rng_unchanged=all(row['world_rng_before'] == row['world_rng_after'] for row in rows))))


if __name__ == '__main__':
    main()
