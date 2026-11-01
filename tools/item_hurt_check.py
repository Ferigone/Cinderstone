#!/usr/bin/env python3
"""Replay frozen ItemEntity.hurt probes against the unchanged original class.

This checks original executable evidence only; no native Rust damage handler is
invoked or claimed to pass. The frozen vectors are never rewritten.
"""
from pathlib import Path
import hashlib
import json
import os
import subprocess
from urllib.parse import unquote, urlparse

ROOT = Path(__file__).resolve().parents[1]
ORACLE = ROOT.parent / 'mc121-oracle'
JDK = Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
SHA = '8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'))


def main():
    jar = ORACLE / 'server-inner-1.21.jar'
    assert digest(jar) == SHA, 'wrong original server JAR'
    fixture = ROOT / 'fixtures/item-hurt-vectors.json'
    vectors = json.loads(fixture.read_text())
    audit = json.loads((ROOT / 'evidence/item-hurt-audit.json').read_text())
    assert vectors['oracle_inner_sha256'] == audit['oracle_inner_sha256'] == SHA
    assert vectors['native_comparison'] is False and audit['native_comparison'] is False
    for name, expected in audit['source_sha256'].items():
        assert digest(ROOT / name) == expected, 'changed audit input: ' + name
    cases = vectors['cases']
    assert len(cases) == vectors['queries'] == audit['original_executable_queries'] == 728
    runtime = ROOT / 'runtime/item-hurt-replay'
    classes = runtime / 'classes'
    classes.mkdir(parents=True, exist_ok=True)
    cp = os.pathsep.join(map(str, [classes, jar, *sorted((ORACLE / 'libraries').rglob('*.jar'))]))
    flags = ['--add-exports', 'java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED']
    compile = subprocess.run(
        [JDK / 'javac', *flags, '-proc:none', '-cp', cp, '-d', classes,
         ROOT / 'tools/survival_oracle.java', ROOT / 'tools/item_hurt_oracle.java'],
        capture_output=True, text=True)
    if compile.returncode:
        raise RuntimeError(compile.stdout + compile.stderr)
    run = subprocess.run(
        [JDK / 'java', *flags, '-Xmx512M', '-cp', cp, 'oracle.item_hurt_oracle', '--replay'],
        input=''.join(canonical(case['query']) + '\n' for case in cases),
        capture_output=True, text=True)
    (runtime / 'oracle.log').write_text(run.stdout + run.stderr)
    if run.returncode:
        raise RuntimeError((run.stdout + run.stderr)[-7000:])
    origins = [line.removeprefix('SOURCE:') for line in run.stdout.splitlines() if line.startswith('SOURCE:')]
    assert len(origins) == 1 and Path(unquote(urlparse(origins[0]).path)).resolve() == jar.resolve(), 'unexpected class origin'
    actual = [json.loads(line.removeprefix('ORACLE:')) for line in run.stdout.splitlines() if line.startswith('ORACLE:')]
    assert len(actual) == len(cases), 'incomplete original-class replay'
    for index, (case, observed) in enumerate(zip(cases, actual)):
        if canonical(case['result']) != canonical(observed):
            raise AssertionError(f'original result changed at case {index}: expected={canonical(case["result"])} actual={canonical(observed)}')
    assert digest(fixture) == audit['source_sha256']['fixtures/item-hurt-vectors.json'], 'frozen vectors changed'
    print(json.dumps(dict(ok=True, minecraft='1.21', original_queries=len(cases), mismatches=0,
                          native_comparison=False, scope='Original-class executable evidence replay with controlled DamageSource/world callbacks')))


if __name__ == '__main__':
    main()
