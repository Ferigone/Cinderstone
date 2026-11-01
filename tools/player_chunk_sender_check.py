#!/usr/bin/env python3
"""Check native PlayerChunkSender against original arq and shipped dependency order."""
import argparse,hashlib,json,os,re,subprocess,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
CARGO='/root/.local/share/mc-rust-toolchain/cargo/bin/cargo'
def digest(path):return hashlib.sha256((ROOT/path).read_bytes()).hexdigest()
def main():
 p=argparse.ArgumentParser();p.add_argument('--skip-generation',action='store_true');p.add_argument('--release',action='store_true');p.add_argument('--optimized-standalone',action='store_true');a=p.parse_args()
 if not a.skip_generation:subprocess.run([sys.executable,'tools/player_chunk_sender_generate.py'],cwd=ROOT,check=True)
 oracle=json.loads((ROOT/'evidence/player-chunk-sender-oracle.json').read_text())
 assert digest('fixtures/player_chunk_sender.json')==oracle['fixture_sha256']
 for path,sha in oracle['source_sha256'].items():assert digest(path)==sha,path
 env=os.environ.copy();env.update(CARGO_HOME='/root/.local/share/mc-rust-toolchain/cargo',RUSTUP_HOME='/root/.local/share/mc-rust-toolchain/rustup',CARGO_INCREMENTAL='0')
 profiles={};runtime=ROOT/'runtime/player_chunk_sender'
 for profile in ['debug']+(['release'] if a.release else []):
  command=[CARGO,'test','--offline','--lib']+(['--release'] if profile=='release' else [])+['player_chunk_sender','--','--nocapture']
  result=subprocess.run(command,cwd=ROOT,env=env,text=True,capture_output=True);output=result.stdout+result.stderr;(runtime/(profile+'.log')).write_text(output)
  if result.returncode:raise RuntimeError(output[-7000:])
  assert re.search(r'test result: ok\. 4 passed; 0 failed',output),output[-5000:]
  assert f"{oracle['states']} states, {oracle['ready_queries']} readiness queries, {oracle['transport_failure_prefixes']} transport-failure prefixes" in output
  profiles[profile]=dict(tests_passed=4,differences=0,cargo_target_directory=env.get('CARGO_TARGET_DIR','default'))
 if a.optimized_standalone:
  deps=ROOT/'target/debug/deps';serde=max(deps.glob('libserde_json-*.rlib'),key=lambda path:path.stat().st_mtime);anyhow=max(deps.glob('libanyhow-*.rlib'),key=lambda path:path.stat().st_mtime)
  binary=runtime/'sender-optimized-standalone-tests'
  env['CARGO_MANIFEST_DIR']=str(ROOT)
  rustc='/root/.local/share/mc-rust-toolchain/rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc'
  command=[rustc,'--edition=2021','--test','tools/player_chunk_sender_standalone_tests.rs','-C','opt-level=3','--extern','serde_json='+str(serde),'--extern','anyhow='+str(anyhow),'-L','dependency='+str(deps),'-o',str(binary)]
  subprocess.run(command,cwd=ROOT,env=env,check=True)
  result=subprocess.run([binary,'--nocapture'],cwd=ROOT,env=env,text=True,capture_output=True);output=result.stdout+result.stderr;(runtime/'optimized-standalone.log').write_text(output)
  if result.returncode:raise RuntimeError(output[-7000:])
  assert re.search(r'test result: ok\. 4 passed; 0 failed',output),output[-5000:]
  assert f"{oracle['states']} states, {oracle['ready_queries']} readiness queries, {oracle['transport_failure_prefixes']} transport-failure prefixes" in output
  profiles['optimized_standalone']=dict(tests_passed=4,differences=0,compiler_flags=['opt-level=3'],whole_crate_cargo_release=False)
 node_script="import mc from 'minecraft-protocol';const s=mc.createSerializer({state:'play',isServer:false,version:'1.21'});process.stdout.write(s.createPacketBuffer({name:'chunk_batch_received',params:{chunksPerTick:16}}).toString('hex'));"
 node=subprocess.run(['node','--input-type=module','-e',node_script],cwd=ROOT,text=True,capture_output=True,check=True);assert node.stdout=='0841800000',node.stdout
 paths=['src/player_chunk_sender.rs','src/player_chunk_sender_order.rs','tools/player_chunk_sender_tests.rs','tools/player_chunk_sender_oracle.java','tools/player_chunk_sender_generate.py','tools/player_chunk_sender_check.py','tools/player_chunk_sender_bytecode.txt','tools/player_chunk_sender_audit.md','tools/player_chunk_sender_standalone_tests.rs']
 report=dict(ok=True,minecraft='1.21',oracle_inner_sha256=oracle['oracle_inner_sha256'],profiles=profiles,histories=oracle['histories'],states=oracle['states'],ready_queries=oracle['ready_queries'],transport_failure_prefixes=oracle['transport_failure_prefixes'],successful_batches=oracle['successful_batches'],original_methods=oracle['original_methods'],original_classes_rewritten=False,ack_packet=dict(original_registered_id=oracle['ack_packet_id'],node_protocol767_chunks_per_tick16_hex=node.stdout),fixture_sha256=oracle['fixture_sha256'],source_sha256={path:digest(path) for path in paths},scope=['Actual original scheduling, ACK, drop, readiness lookup order, selection/removal, successful Begin/Chunk/End and callback-failure prefixes','Original actual packet constructors in a controlled empty-section chunk / no-light-layer context','Shipped fastutil stream table order and Guava bounded TopK independently compared in sibling corpus','Required live host facts and transport callbacks; no synthetic readiness or self ACK'],not_verified=oracle['not_verified']+['Native normal-world chunk/light payload and main-loop integration are separate provider concerns','Whole Minecraft server parity'],reproduce='python3 tools/player_chunk_sender_check.py --release')
 if 'release' not in profiles:
  report['whole_crate_cargo_release_verified']=False
  failed=runtime/'release.log'
  if failed.exists() and 'undefined hidden symbol' in failed.read_text():
   report['cargo_release_link_failure']=dict(log='runtime/player_chunk_sender/release.log',log_sha256=hashlib.sha256(failed.read_bytes()).hexdigest(),reason='rust-lld undefined hidden symbols in unrelated rustls/ring thin-LTO dependencies; not a native test assertion failure')
  report['reproduce']='python3 tools/player_chunk_sender_check.py --optimized-standalone; full Cargo release remains a separate required check'
 else:
  report['whole_crate_cargo_release_verified']=True
  report['release_scope']='Cargo --release --lib player_chunk_sender: full library test binary compilation and four selected tests; not an all-target project suite'
  if env.get('CARGO_TARGET_DIR'):report['reproduce']='CARGO_TARGET_DIR='+env['CARGO_TARGET_DIR']+' python3 tools/player_chunk_sender_check.py --release'
 (ROOT/'evidence/player-chunk-sender-conformance.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
