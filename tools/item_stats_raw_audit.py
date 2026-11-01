#!/usr/bin/env python3
"""Read-only independent audit of existing Java/Rust packet captures and files."""
from pathlib import Path
import hashlib,json,subprocess,uuid
ROOT=Path(__file__).resolve().parents[1]
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
 primary_path=ROOT/'evidence/item-stats-java-rust-live-conformance.json';primary=json.loads(primary_path.read_text());assert primary['ok']
 expected={'ItemDropper':{(5,1):8,(8,22):2},'ItemObserver':{(4,1):8}}
 raw=[];projections=[];saved=[];absences={}
 for platform in ['java','rust']:
  directory=Path(primary[platform]['directory'])
  for phase in ['before_stock','after_stock','reconnect_after_drain']:
   path=directory/(phase+'.json');capture=json.loads(path.read_text());assert capture['ok'] and capture['phase']==phase
   raw.append(dict(platform=platform,phase=phase,source_sha256=sha(path),report=capture))
   for player in capture['players']:
    name=player['username'];target=expected[name];identity=uuid.UUID(bytes=hashlib.md5(('OfflinePlayer:'+name).encode()).digest(),version=3);assert str(identity)==player['uuid']
    assert len(player['requests'])==2 and player['unsolicited_before_requests']==0 and player['collect_packets']==[]
    for index,request in enumerate(player['requests']):
     packet=player['statistics'][request['response_index']];keys=[(v['categoryId'],v['statisticId']) for v in packet];assert len(keys)==len(set(keys))
     selected={(v['categoryId'],v['statisticId']):v['value'] for v in packet if (v['categoryId'],v['statisticId']) in target}
     wanted=target if index==0 and phase!='before_stock' else {};assert selected==wanted,(platform,phase,name,index,selected)
     projections.append(dict(platform=platform,phase=phase,player=name,request=index,explicit_target_entries=[dict(type_id=k[0],value_id=k[1],value=v)for k,v in sorted(selected.items())],missing_means='NOT_PRESENT/NOT_DIRTY, not a numeric zero'))
 for name,target in expected.items():
  identity=primary['initial_java_identity_files'][name]['uuid'];world=ROOT.parent/'mc121-oracle/world';paths=[f'stats/{identity}.json',f'stats/{name}.json',f'playerdata/{identity}.dat',f'advancements/{identity}.json']
  absences[name]={path:None if not(world/path).exists() else sha(world/path)for path in paths};assert all(v is None for v in absences[name].values())
  p=Path(primary['java']['directory'])/'world/stats'/(identity+'.json');data=json.loads(p.read_text());wanted={'minecraft:dropped':{'minecraft:stone':8},'minecraft:custom':{'minecraft:drop':2}} if name=='ItemDropper' else {'minecraft:picked_up':{'minecraft:stone':8}}
  for kind,values in wanted.items():
   for key,value in values.items():assert data['stats'][kind][key]==value
  saved.append(dict(player=name,uuid=identity,file_sha256=sha(p),DataVersion=data['DataVersion'],relevant_values=wanted,full_file=data))
 counter=ROOT/'tools/player_stats_login_counter_bytecode.txt';output=subprocess.run(['/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin/javap','-classpath',str(ROOT.parent/'mc121-oracle/server-inner-1.21.jar'),'-p','-c','avv','awa'],text=True,capture_output=True,check=True).stdout;counter.write_text(output)
 login=dict(ok=True,evidence_kind='static shipped bytecode and read-only source-file observation; not a fresh runtime invocation',oracle_inner_sha256=primary['oracle_inner_sha256'],methods=[dict(owner='aur/PlayerList',method='placeNewPlayer a(vt,aqv,arj)',offsets=[490,491,494],fact='aqv.I() then avv.c() marks every currently stored key dirty on each login'),dict(owner='avv/ServerStatsCounter',method='markAllDirty c()',offsets=[1,5,8,13],fact='dirty.addAll(values.keySet()); absent keys are not inserted'),dict(owner='awa/StatsCounter',method='constructor',offsets=[19,22],fact='map.defaultReturnValue(0)'),dict(owner='avv/ServerStatsCounter',method='constructor',offsets=[5,8,21,25],fact='new empty dirty set; read file only if isFile'),dict(owner='aur/PlayerList',method='getPlayerStats a(cmx)',offsets=[60,68,139,147,160],fact='UUID file first; safe legacy player-name file migration before constructor'),dict(owner='aur/PlayerList',method='remove c(aqv)',offsets=[144,148],fact='remove UUID from stats cache after save')],canonical_initial_identity_files=absences,known_private_world_warning='Completed Java stock worlds contain8/2 and8 already. Never reuse them as zero-valued baseline.',wire_rule='Missing delta entries do not establish counter values. Fresh semantic0 comes separately from absent UUID+legacy files, empty/default-zero constructors and no baseline item actions.',reconnect_rule='Every login re-marks stored keys, so the first post-stock reconnect explicitly returns8/2 or8. Only its second request must omit those keys; unrelated vanilla tick statistics may remain.',source_sha256={'tools/player_stats_login_aur_bytecode.txt':sha(ROOT/'tools/player_stats_login_aur_bytecode.txt'),'tools/player_stats_login_counter_bytecode.txt':sha(counter)})
 (ROOT/'evidence/player-stats-login-audit.json').write_text(json.dumps(login,indent=2)+'\n')
 report=dict(ok=True,evidence_kind='independent read-only audit of existing raw packets; no server launched or core modified',primary_report_sha256=sha(primary_path),oracle_inner_sha256=primary['oracle_inner_sha256'],native_binary_sha256=primary['native_binary_sha256'],raw_query_reports=raw,target_projections=projections,final_java_stat_files=saved,canonical_initial_identity_files=absences,checks=dict(six_raw_reports_embedded=True,no_duplicate_packet_map_keys=True,baseline_target_keys_absent_without_invented_wire_zeros=True,first_post_stock_and_second_reconnect_explicit_targets8_2_8=True,same_connection_repeat_target_keys_absent=True,final_java_saved_target_values_match=True),not_verified=primary['not_verified'],source_sha256={'tools/item_stats_raw_audit.py':sha(Path(__file__)),'tools/player_stats_login_aur_bytecode.txt':sha(ROOT/'tools/player_stats_login_aur_bytecode.txt'),'tools/player_stats_login_counter_bytecode.txt':sha(counter)})
 (ROOT/'evidence/item-stats-java-rust-raw-audit.json').write_text(json.dumps(report,indent=2)+'\n');print('PASS:6 raw reports,24 target projections,2 final Java stat files; no live rerun')
if __name__=='__main__':main()
