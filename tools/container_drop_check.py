#!/usr/bin/env python3
"""Verify menu drop arguments as well as dropped stacks against original Java."""
from pathlib import Path
import hashlib,json,os,subprocess

root=Path(__file__).resolve().parents[1]
oracle=root.parent/'mc121-oracle'
jdk=Path('/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin')
build=root/'target/container-drop-wrapper';build.mkdir(parents=True,exist_ok=True)
classpath=os.pathsep.join(map(str,[oracle/'server-inner-1.21.jar',*sorted((oracle/'libraries').rglob('*.jar'))]))
exports=['--add-exports','java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED']
subprocess.run([str(jdk/'javac'),*exports,'-proc:none','-cp',classpath,'-d',str(build),str(root/'tools/container_oracle.java'),str(root/'tools/container_drop_wrapper_oracle.java')],check=True)
p=subprocess.run([str(jdk/'java'),*exports,'-cp',str(build)+os.pathsep+classpath,'oracle.container_drop_wrapper_oracle'],capture_output=True,text=True)
assert p.returncode==0,p.stdout+p.stderr
wrapper=json.loads(next(line.split('ORACLE_WRAPPER:',1)[1] for line in p.stdout.splitlines() if 'ORACLE_WRAPPER:' in line))
assert wrapper==[[False,False,False],[True,False,True]],wrapper
env=os.environ.copy();env.setdefault('CARGO_HOME','/root/.local/share/mc-rust-toolchain/cargo');env.setdefault('RUSTUP_HOME','/root/.local/share/mc-rust-toolchain/rustup');env['PATH']=env['CARGO_HOME']+'/bin:'+env['PATH'];env['CARGO_INCREMENTAL']='0'
p=subprocess.run(['cargo','test','--lib','container::tests'],cwd=root,env=env,capture_output=True,text=True)
assert p.returncode==0,p.stdout+p.stderr
for name in ['actual_java_inventory_menu_matches_all_click_modes','actual_java_result_slot_consumption_and_partial_shift_drops','actual_java_close_returns_cursor_and_crafting_inputs_before_dropping_overflow']:
 assert f'container::tests::{name} ... ok' in p.stdout,name
assert '8 passed; 0 failed' in p.stdout
files=['tools/container_vectors.json','tools/container_result_vectors.json','tools/container_close_vectors.json']
states=0;true_calls=false_calls=0
for name in files:
 data=json.loads((root/name).read_text())
 for case in data['cases']:
  states+=len(case['states'])
  for around,trace in case['states'][-1]['drop_flags']:
   assert not around
   if trace:true_calls+=1
   else:false_calls+=1
report={'ok':True,'minecraft':'1.21','original_menu_states':states,'original_wrapper_calls':len(wrapper),
        'trace_true_calls':true_calls,'trace_false_calls':false_calls,'rust_tests_passed':8,
        'scope':'Original menu/ResultSlot/close Player.drop arguments and unchanged Player two-argument wrapper. Entity creation, awards and network timing are separate.',
        'native_source_sha256':hashlib.sha256((root/'src/container.rs').read_bytes()).hexdigest(),
        'fixtures_sha256':{name:hashlib.sha256((root/name).read_bytes()).hexdigest() for name in files}}
(root/'evidence/container-drop-conformance.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
