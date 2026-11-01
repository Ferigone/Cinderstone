#!/usr/bin/env python3
"""Create an audited public source tree without local recovery history."""
import hashlib
import json
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT.parent / 'Cinderstone-public'
EXCLUDED_DIRS = {'.git', 'runtime', 'target', 'logs', 'evidence', 'node_modules', '__pycache__'}
EXCLUDED_FILES = {'working-source.json', 'WORKING_SOURCE_NOTICE.txt', 'mc121-rust-progress.json', 'checkpoint.json'}
EXCLUDED_SUFFIXES = {'.jar', '.class', '.log', '.zip', '.png', '.jpg', '.jpeg', '.mp4', '.pyc', '.dat', '.mca', '.mcc'}

def selected(name):
    p = Path(name)
    return not (set(p.parts) & EXCLUDED_DIRS or p.name in EXCLUDED_FILES
        or p.suffix.lower() in EXCLUDED_SUFFIXES or 'bytecode' in p.name
        or name.startswith('tools/hotspot_math_sources/')
        or name in {'tools/rng_startup_aqv_constructors.txt', 'tools/rng_startup_cmx_constructors.txt', 'tools/rng_startup_btn_constructors.txt'})

def main():
    names = subprocess.check_output(['git','ls-files','--cached','--others','--exclude-standard','-z'],cwd=ROOT).decode().split('\0')
    DEST.mkdir(parents=True, exist_ok=True)
    files=[]
    for name in sorted(set(filter(None,names))):
        source=ROOT/name
        if not selected(name) or not source.is_file() or source.is_symlink(): continue
        target=DEST/name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source,target)
        files.append(name)
    progress=json.loads((ROOT/'mc121-rust-progress.json').read_text())
    public={k:progress[k] for k in ['format','updated_at','scope_version','metric','not_behavior_coverage','not_engineering_effort_percentage','total_points','code_ported_points','main_integrated_points','code_ported_percent','main_integrated_percent','full_vanilla_parity']}
    public['rows']=[{k:r[k] for k in ['id','area','points','code_ported','main_integrated']} for r in progress['rows']]
    target=DEST/'progress/checklist.json';target.parent.mkdir(parents=True,exist_ok=True)
    target.write_text(json.dumps(public,indent=2)+'\n');files.append('progress/checklist.json')
    print(json.dumps({'directory':str(DEST),'files':len(files)}))

if __name__=='__main__': main()
