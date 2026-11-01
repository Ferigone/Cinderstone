#!/usr/bin/env python3
"""Execute original recipe-book state/settings/NBT/packet transitions."""
from pathlib import Path
import argparse, base64, hashlib, json, os, random, struct, subprocess, zipfile

ROOT = Path(__file__).resolve().parents[1]
ORACLE = ROOT.parent / 'mc121-oracle'
JDK = Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
SHA = '8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
FLAGS = ['--add-exports', 'java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED']
SETTING_NAMES = ['isGuiOpen', 'isFilteringCraftable', 'isFurnaceGuiOpen', 'isFurnaceFilteringCraftable',
                 'isBlastingFurnaceGuiOpen', 'isBlastingFurnaceFilteringCraftable', 'isSmokerGuiOpen', 'isSmokerFilteringCraftable']


def atomic_json(path, value):
    temporary = path.with_suffix('.json.tmp')
    temporary.write_text(json.dumps(value, separators=(',', ':'), ensure_ascii=True) + '\n')
    temporary.replace(path)


class Original:
    def __init__(self):
        self.runtime = ROOT / 'runtime/server-recipe-book'
        self.runtime.mkdir(parents=True, exist_ok=True)
        jar = ORACLE / 'server-inner-1.21.jar'
        assert hashlib.sha256(jar.read_bytes()).hexdigest() == SHA
        archive = zipfile.ZipFile(jar)
        recipes = {'minecraft:' + name[len('data/minecraft/recipe/'):-5]: json.loads(archive.read(name))
                   for name in sorted(archive.namelist()) if name.startswith('data/minecraft/recipe/') and name.endswith('.json')}
        assert len(recipes) == 1290
        source = json.loads((ROOT / 'fixtures/crafting-1.21.json').read_text())
        assert source['source']['jar_sha256'] == SHA
        self.bootstrap = self.runtime / 'bootstrap.json'
        atomic_json(self.bootstrap, dict(recipes=recipes, tags=source['tags']))
        self.cp = os.pathsep.join(map(str, [self.runtime, jar, *sorted((ORACLE / 'libraries').rglob('*.jar'))]))
        result = subprocess.run([JDK / 'javac', *FLAGS, '-proc:none', '-cp', self.cp, '-d', self.runtime,
                                 ROOT / 'tools/server_recipe_book_oracle.java'], cwd=ROOT, text=True, capture_output=True)
        if result.returncode:
            raise RuntimeError((result.stdout + result.stderr)[-6000:])

    def run(self, queries=None, mode=None):
        command = [JDK / 'java', *FLAGS, '-Xmx1G', '-cp', self.cp,
                   'oracle.server_recipe_book_oracle', self.bootstrap]
        if mode:
            command.append(mode)
        result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True,
                                input=None if queries is None else ''.join(json.dumps(q, ensure_ascii=True) + '\n' for q in queries))
        (self.runtime / ((mode or 'transitions') + '.log')).write_text(result.stdout + result.stderr)
        if result.returncode:
            raise RuntimeError((result.stdout + result.stderr)[-7000:])
        rows = [json.loads(line[7:]) for line in result.stdout.splitlines() if line.startswith('ORACLE:')]
        assert len(rows) == (1 if mode else len(queries)), (len(rows), mode)
        return rows[0] if mode else rows


def nbt(kind, value=None, **extra):
    result = dict(type=kind, **extra)
    if value is not None:
        result['value'] = value
    return result


def compound(values):
    return nbt(10, values)


def strings(values):
    return nbt(9, [nbt(8, value) if not isinstance(value, dict) else nbt(8, **value) for value in values])


def settings(mask):
    return [bool(mask & (1 << index)) for index in range(8)]


def make_queries(facts):
    recipes = facts['recipes']
    normal = [row['id'] for row in recipes if not row['is_special']]
    special = [row['id'] for row in recipes if row['is_special']]
    queries = []
    def case(name, operations, **extra):
        queries.append(dict(name=name, operations=operations, **extra))
    for row in recipes:
        rid = row['id']
        case('default/' + rid, [dict(op='add_recipes', recipes=[rid, rid]), dict(op='contains', recipe=rid),
                               dict(op='will_highlight', recipe=rid), dict(op='init'),
                               dict(op='remove_recipes', recipes=[rid, rid]), dict(op='contains_id', id=rid), dict(op='init')])
    for mask in range(256):
        case('settings/' + str(mask), [dict(op='set_settings', settings=settings(mask)), dict(op='get_settings_copy'),
                                      dict(op='external_set', settings=settings(mask ^ 255)), dict(op='init'),
                                      dict(op='set_external'), dict(op='external_set', settings=settings(mask)), dict(op='init')])
    for value in range(256):
        raw = bytes([value] * 8) + (b'\xab' if value % 17 == 0 else b'')
        case('boolean-wire/' + str(value), [dict(op='settings_wire_decode', wire=base64.b64encode(raw).decode()), dict(op='init')])
    # Actual NBT numeric coercion, including low-byte wrap, negative fractions,
    # signed zeros, subnormals, infinities and NaNs for every named setting.
    numerics = {1: [-128, -1, 0, 1, 127], 2: [-32768, -257, -256, -255, -1, 0, 1, 255, 256, 257, 32767],
                3: [-2147483648, -65537, -256, -1, 0, 1, 255, 256, 257, 2147483647],
                4: ['-9223372036854775808', '-4294967297', '-256', '-1', '0', '1', '255', '256', '257', '9223372036854775807']}
    floating = {5: ['00000000', '80000000', '00000001', '80000001', '3f000000', 'bf000000', '437fffff', '43800000',
                    '7f800000', 'ff800000', '7fc00000', '7fa00001', 'cf000000', '4f000000'],
                6: ['0000000000000000', '8000000000000000', '0000000000000001', '8000000000000001',
                    '3fe0000000000000', 'bfe0000000000000', '406fffffffffffe0', '4070000000000000',
                    '7ff0000000000000', 'fff0000000000000', '7ff8000000000000', '7ff0000000000001',
                    'c1e0000000000000', '41e0000000000000']}
    for field in SETTING_NAMES:
        for kind, values in numerics.items():
            for value in values:
                case(f'nbt-bool/{field}/{kind}/{value}', [dict(op='load_nbt', nbt=compound({field: nbt(kind, value)})), dict(op='init')])
        for kind, values in floating.items():
            for bits in values:
                case(f'nbt-bool/{field}/{kind}/{bits}', [dict(op='load_nbt', nbt=compound({field: nbt(kind, bits=bits)})), dict(op='init')])
        for kind, value in [(8, 'true'), (9, []), (10, {}), (7, [1]), (11, [1]), (12, ['1'])]:
            case(f'nbt-bool-wrong-type/{field}/{kind}', [dict(op='load_nbt', nbt=compound({field: nbt(kind, value)}))])
    edge_ids = ['', ':', ':foo', 'foo:', 'minecraft:', 'minecraft:stone', 'stone', 'abc.def-1_:some/path_1.2',
                'foo::bar', 'A:b', 'minecraft:Upper', 'a b', 'foo\\bar', 'foo/../bar', 'a\0b', 'é',
                dict(utf16=[0xd800]), dict(utf16=[0xdc00]), dict(utf16=[0xd800, 0xdc00]), dict(utf16=[0x61, 0xd800, 0x62])]
    for index, rid in enumerate(edge_ids):
        case(f'resource-id/{index}', [dict(op='parse_id', id=rid),
                                     dict(op='load_nbt', nbt=compound({'recipes': strings([rid]), 'toBeDisplayed': strings([rid])}))])
    for rid in special:
        case('special-highlight/' + rid, [dict(op='add_highlight', recipe=rid), dict(op='contains', recipe=rid),
                                         dict(op='will_highlight', recipe=rid), dict(op='init'),
                                         dict(op='load_nbt', nbt=compound({'recipes': strings([rid]), 'toBeDisplayed': strings([rid])})),
                                         dict(op='direct_remove', recipe=rid), dict(op='init')])
    seed = normal[:9] + special[:3]
    case('nbt-merge-does-not-clear', [dict(op='add_recipes', recipes=normal[:3]), dict(op='set_settings', settings=settings(255)),
                                    dict(op='load_nbt', nbt=compound({'recipes': strings([normal[1], normal[3], 'test:missing']),
                                                                   'toBeDisplayed': strings([normal[4], *special[:1]])})),
                                    dict(op='init'), dict(op='load_nbt', nbt=compound({})), dict(op='init')])
    for wrong in [nbt(1, 1), nbt(8, normal[0]), nbt(9, [nbt(3, 1)]), nbt(10, {})]:
        case('wrong-recipe-list/' + str(wrong['type']), [dict(op='load_nbt', nbt=compound({'recipes': wrong, 'toBeDisplayed': wrong})), dict(op='init')])
    for mask in [0, 1, 85, 170, 255]:
        case('copy-distinct/' + str(mask), [dict(op='add_recipes', recipes=normal[8:12]),
                                          dict(op='copy_from', known=normal[:3] + special[:1], highlight=normal[3:5] + special[:2], settings=settings(mask)),
                                          dict(op='init'), dict(op='get_settings_copy'), dict(op='external_set', settings=settings(mask ^ 255)), dict(op='init')])
    case('self-copy-java-adversarial', [dict(op='add_recipes', recipes=normal[:3]), dict(op='self_copy')], java_only=True)
    a, b, c = normal[0:3]
    for phase in ['criterion', 'award']:
        for mutation in ['remove', 'highlight', 'settings', 'reentrant_add', 'throw']:
            callback = dict(phase=phase, mutation=mutation, recipes=[a] if mutation == 'remove' else [b], settings=settings(255))
            initial = [dict(op='register_criterion', recipe=a, name='match-first')] if phase == 'award' else []
            case(f'callback/{phase}/{mutation}', [*initial, dict(op='add_recipes', recipes=[a, c], callback=callback), dict(op='init')])
        initial = [dict(op='register_criterion', recipe=a, name='match-first')] if phase == 'award' else []
        case(f'callback/{phase}/duplicate-after-remove', [*initial,
             dict(op='add_recipes', recipes=[a, a], callback=dict(phase=phase, mutation='remove', recipes=[a])), dict(op='init')])
        case(f'callback/{phase}/reentrant-current-id', [*initial,
             dict(op='add_recipes', recipes=[a, c], callback=dict(phase=phase, mutation='reentrant_add', recipes=[a])), dict(op='init')])
    # Actual holder values determine isSpecial, even under an aliased ID.
    for rid, template in [('test:ordinary_alias', a), (special[0], a), (a, special[0]), ('test:special_alias', special[0])]:
        h = dict(id=rid, template=template)
        case('holder-alias/' + rid + '/' + template, [dict(op='add_recipes', recipes=[h, h]), dict(op='contains', recipe=h), dict(op='init'), dict(op='remove_recipes', recipes=[h])])
    for state in range(3):
        for ids in [[], [a], [a, b, a], [':foo', 'foo:', '', ':']]:
            case(f'ordered-packet/{state}/{len(ids)}', [dict(op='set_settings', settings=settings(170)),
                                                      dict(op='packet_ordered', state=state, recipes=ids, highlights=[b, a]),
                                                      dict(op='set_settings', settings=settings(85)), dict(op='saved_packet', index=0)])
    # All declarations of recipe-book IDs in one explicit ordered INIT list,
    # independent of HashSet iteration in ServerRecipeBook.sendInitial.
    case('ordered-packet/all-default-ids', [dict(op='packet_ordered', state=0,
                                               recipes=[r['id'] for r in recipes], highlights=[r['id'] for r in recipes if r['is_special']])])
    rng = random.Random(0x7673953b00c)
    for index in range(32):
        chosen = rng.sample(normal, 12) + rng.sample(special, 2)
        operations = []
        for step in range(40):
            op = rng.choice(['add_recipes', 'remove_recipes', 'direct_add', 'direct_remove', 'add_highlight',
                             'remove_highlight', 'contains', 'will_highlight', 'init', 'set_open', 'set_filter'])
            value = dict(op=op)
            if op in ['add_recipes', 'remove_recipes']:
                value['recipes'] = [rng.choice(chosen) for _ in range(rng.randrange(5))]
            elif op in ['set_open', 'set_filter']:
                value.update(type=rng.randrange(4), value=bool(rng.randrange(2)))
            elif op != 'init':
                value['recipe'] = rng.choice(chosen)
            operations.append(value)
        operations.append(dict(op='init'))
        case('random-history/' + str(index), operations)
    case('contains-null', [dict(op='contains', recipe=None)])
    return queries


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--facts-only', action='store_true')
    parser.add_argument('--skip-declarations', action='store_true')
    args = parser.parse_args()
    original = Original()
    facts = original.run(mode='facts')
    facts['tag_binding_source_sha256'] = hashlib.sha256((ROOT / 'fixtures/crafting-1.21.json').read_bytes()).hexdigest()
    atomic_json(ROOT / 'fixtures/server-recipe-book-facts.json', facts)
    print(json.dumps(dict(phase='facts', count=len(facts['recipes']), special=sum(r['is_special'] for r in facts['recipes']), packet_id=facts['packet_id'])), flush=True)
    if not args.skip_declarations:
        try:
            declared = original.run(mode='declarations')
            declared['payload_sha256'] = hashlib.sha256(base64.b64decode(declared['payload'])).hexdigest()
            declared['tag_binding_source_sha256'] = facts['tag_binding_source_sha256']
            assert declared['decoded_ids'] == [r['id'] for r in facts['recipes']]
            atomic_json(ROOT / 'fixtures/server-recipe-book-controlled-declarations.json', declared)
            print(json.dumps(dict(phase='controlled_declarations', count=declared['decoded_count'], packet_id=declared['packet_id'], bytes=len(base64.b64decode(declared['payload'])))), flush=True)
        except Exception as error:
            atomic_json(ROOT / 'evidence/server-recipe-book-controlled-declarations-error.json', dict(ok=False, error=repr(error), scope='Optional controlled-tag-binding STREAM extraction; no empty declaration substituted'))
            print(json.dumps(dict(phase='controlled_declarations', ok=False, error=repr(error))), flush=True)
    if args.facts_only:
        return
    queries = make_queries(facts)
    rows = original.run(queries)
    # Add literal packet decode roundtrips derived from actual original encoder
    # outputs, not an independent handwritten wire-format formula.
    decode = []
    for query, row in zip(queries, rows):
        if query['name'].startswith('ordered-packet/'):
            packet = next(state['packets'][0] for state in row['states'] if state['packets'])
            decode.append(dict(name='decode/' + query['name'], operations=[dict(op='packet_decode', wire=packet['wire'])]))
    decoded = original.run(decode)
    fixture = dict(minecraft='1.21', oracle_inner_sha256=SHA,
                   scope='Actual original avu/avs/avt/aee methods and Recipe.CODEC/isSpecial. Forwarding observer at original predicate boundary; real dt/dv run with controlled empty/registered criterion map and outer award observation. Explicit aliases do not claim datapack registration. INIT multi-ID sets and NBT multi-ID list order compare semantically; ADD/REMOVE and explicit ordered packet inputs preserve literal wire order.',
                   cases=[dict(query=query, expected=row) for query, row in zip(queries + decode, rows + decoded)])
    atomic_json(ROOT / 'fixtures/server-recipe-book-vectors.json', fixture)
    print(json.dumps(dict(ok=True, original_cases=len(fixture['cases']), operation_snapshots=sum(len(r['states']) for r in rows + decoded),
                          packet_snapshots=sum(len(s['packets']) for r in rows + decoded for s in r['states']),
                          original_errors=sum(s['error'] is not None for r in rows + decoded for s in r['states']),
                          world_rng_unchanged=all(s['world_rng_before'] == s['world_rng_after'] for r in rows + decoded for s in r['states']))), flush=True)


if __name__ == '__main__':
    main()
