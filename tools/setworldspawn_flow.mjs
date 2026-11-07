// SPDX-License-Identifier: GPL-2.0-only
// Real Main: spawn notifications, source coordinates, admission and existing save transaction.
import mc from 'minecraft-protocol';
import nbt from 'prismarine-nbt';
import assert from 'node:assert/strict';
import {spawn,execFileSync} from 'node:child_process';
import {mkdtemp,mkdir,readFile,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import path from 'node:path';
import {createHash} from 'node:crypto';
import {gzipSync} from 'node:zlib';

const [binaryArg,output]=process.argv.slice(2);assert.ok(output,'BINARY REPORT');
const root=process.cwd(),binary=path.resolve(binaryArg),scratch=await mkdtemp(path.join(tmpdir(),'setworldspawn-'));
const vectors=JSON.parse(await readFile('fixtures/setworldspawn-vectors.json','utf8'));
const report={ok:false,native_commit:execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim(),
  native_src_tree:execFileSync('git',['rev-parse','HEAD:src'],{encoding:'utf8'}).trim(),native_binary_sha256:createHash('sha256').update(await readFile(binary)).digest('hex'),
  java_jar_sha1:vectors.java_jar_sha1,commands:[],limits:['Native flat creative admission is anchored one block above shared spawn; this does not implement the full vanilla randomized/safe-placement or respawn algorithm.',
    'Original command/argument/result/component and notification codec execute with a recording level mutation sink; spawn tickets/generation are outside scope.',
    'Imported level.dat case uses a synthetic current-version metadata world with console/status and preservation checks, without claiming imported gameplay.']};
const clients=[];let server,log='',folder,transaction=0;
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
async function until(test,label,timeout=30000){const end=Date.now()+timeout;while(!test()){if(server?.exitCode!==null&&server?.exitCode!==undefined)throw Error('server exited: '+label+' '+log.slice(-1200));if(Date.now()>end)throw Error('timeout: '+label);await pause(20);}}
function uuid(name){const b=createHash('md5').update('OfflinePlayer:'+name).digest();b[6]=(b[6]&15)|48;b[8]=(b[8]&63)|128;const s=b.toString('hex');return `${s.slice(0,8)}-${s.slice(8,12)}-${s.slice(12,16)}-${s.slice(16,20)}-${s.slice(20)}`;}
async function prepare(name){const f=path.join(scratch,name);await mkdir(f);await writeFile(path.join(f,'keys.json'),'{"profilePropertyKeys":[],"playerCertificateKeys":[]}');await writeFile(path.join(f,'ops.json'),JSON.stringify([{uuid:uuid('SpawnOther'),name:'SpawnOther',level:4,bypassesPlayerLimit:false}]));return f;}
async function start(f,imported=false){
  folder=f;log='';server=spawn(binary,['--allow-offline','--bind','127.0.0.1','--port','25704','--fixtures',path.join(root,'fixtures/configuration.json'),
    '--world',path.join(f,'world.json'),'--server-directory',f,'--view-distance','2','--loopback-service-keys',path.join(f,'keys.json'),...(imported?['--anvil-world',path.join(f,'world')]:[])],{cwd:f,stdio:['pipe','pipe','pipe']});
  server.stdout.on('data',b=>{log+=b;});server.stderr.on('data',b=>{log+=b;});await until(()=>log.includes('"event":"listening"'),'startup',120000);
}
async function stop(){if(!server)return;for(const c of clients.splice(0))c.end();const current=server;current.kill('SIGTERM');await until(()=>current.exitCode!==null,'normal shutdown');assert.equal(current.exitCode,0,log.slice(-1800));await writeFile(path.join(folder,`server-${Date.now()}.log`),log);server=null;}
function consoleLine(line){server.stdin.write(line+'\n');}
const has=(c,key)=>c&&typeof c==='object'&&(c.translate===key||Object.values(c).some(v=>has(v,key)));
const red=c=>c&&typeof c==='object'&&(c.color==='red'||Object.values(c).some(red));
function subtree(packet){const nodes=packet.nodes,start=nodes[packet.rootIndex].children.find(i=>nodes[i].extraNodeData?.name==='setworldspawn');if(start===undefined)return null;const out=[];function visit(i,prefix){const n=nodes[i],e=n.extraNodeData,p=[...prefix,e.name];out.push({path:p.join(' '),executable:!!n.flags.has_command,parser:e.parser??null});n.children.forEach(c=>visit(c,p));}visit(start,[]);return out;}
async function connect(name){
  const client=mc.createClient({host:'127.0.0.1',port:25704,username:name,auth:'offline',version:'1.21',keepAlive:false,hideErrors:true});clients.push(client);
  let initial,tree,error,closing=false;const messages=[],spawns=[],completions=[],centers=[];
  client.on('error',e=>{if(!closing)error=e;});client.on('end',()=>{if(!closing)error=Error(name+' ended');});
  client.on('keep_alive',p=>client.write('keep_alive',p));client.on('chunk_batch_finished',()=>client.write('chunk_batch_received',{chunksPerTick:16}));
  client.on('position',p=>{client.write('teleport_confirm',{teleportId:p.teleportId});initial??=p;});
  client.on('declare_commands',p=>{tree=subtree(p);});client.on('world_border_center',p=>centers.push(p));client.on('tab_complete',p=>completions.push(p));
  client.on('packet',(p,m,b)=>{if(m.name==='system_chat')messages.push({component:nbt.simplify(p.content),hex:b.subarray(1).toString('hex')});if(m.name==='spawn_position')spawns.push({packet:p,hex:b.subarray(1).toString('hex')});});
  async function wait(test,label){await until(()=>{if(error)throw error;return test();},label);}
  await wait(()=>initial&&tree!==undefined&&spawns.length,name+' admission');
  return {client,messages,spawns,centers,wait,get initial(){return initial;},get tree(){return tree;},
    async denied(command){const n=messages.length;client.write('chat_command',{command});await wait(()=>messages.slice(n).some(m=>red(m.component)),'denied '+command);},
    async complete(text){const id=++transaction;client.write('tab_complete',{transactionId:id,text});await wait(()=>completions.some(p=>p.transactionId===id),'completion '+text);const p=completions.find(p=>p.transactionId===id);return {start:p.start,end:p.start+p.length,texts:p.matches.map(m=>m.match)};},
    async close(){closing=true;client.end();await pause(50);}
  };
}
async function barrier(observer){const n=observer.messages.length;consoleLine('weather clear 120');await observer.wait(()=>observer.messages.slice(n).some(m=>m.component.translate==='chat.type.admin'&&has(m.component,'commands.weather.set.clear')),'ordered feedback barrier');}
async function execute(source,observer,row,console=false){
  const a=source.messages.length,b=observer.messages.length,s=source.spawns.length,o=observer.spawns.length;
  if(console)consoleLine(row.input);else source.client.write('chat_command',{command:row.input});
  await source.wait(()=>source.spawns.length>s,row.input+' source spawn packet');await observer.wait(()=>observer.spawns.length>o,row.input+' observer spawn packet');
  assert.equal(source.spawns.at(-1).hex,row.spawn_packet_hex);assert.equal(observer.spawns.at(-1).hex,row.spawn_packet_hex);
  await observer.wait(()=>observer.messages.slice(b).some(m=>m.component.translate==='chat.type.admin'&&has(m.component,'commands.setworldspawn.success')),row.input+' operator feedback');
  const admin=observer.messages.slice(b).find(m=>m.component.translate==='chat.type.admin'&&has(m.component,'commands.setworldspawn.success'));
  assert.equal(admin.hex,console?row.console_admin_hex:row.admin_hex);
  if(!console){await source.wait(()=>source.messages.slice(a).some(m=>m.component.translate==='commands.setworldspawn.success'),row.input+' private feedback');assert.equal(source.messages.slice(a).find(m=>m.component.translate==='commands.setworldspawn.success').hex,row.feedback_hex);}
  report.commands.push({command:row.input,console,pos:row.pos,angle_bits:row.angle_bits,spawn_packet_hex:row.spawn_packet_hex,admin_hex:admin.hex});
}
async function metadataWorld(f){
  await mkdir(path.join(f,'world/region'),{recursive:true});
  const integer=v=>({type:'int',value:v}),float=v=>({type:'float',value:v}),text=v=>({type:'string',value:v});
  const data={DataVersion:integer(3953),Time:{type:'long',value:[0,0]},DayTime:{type:'long',value:[0,0]},SpawnX:integer(4),SpawnY:integer(70),SpawnZ:integer(-5),SpawnAngle:float(12.5),
    WorldGenSettings:{type:'compound',value:{seed:{type:'long',value:[0,0]}}},GameRules:{type:'compound',value:{doDaylightCycle:text('false'),doWeatherCycle:text('false')}},opaque:text('spawn-unknown-field')};
  await writeFile(path.join(f,'world/level.dat'),gzipSync(nbt.writeUncompressed({type:'compound',name:'',value:{Data:{type:'compound',value:data},extension:text('retained')}})));
}
try{
  const witness=input=>vectors.execution.find(r=>r.input===input&&r.permission===2);
  const f=await prepare('native');await start(f);let observer=await connect('SpawnOther');
  assert.deepEqual(observer.spawns[0].packet,{location:{x:8,y:64,z:8},angle:0});
  consoleLine('setworldspawn 12 64 18 540');await observer.wait(()=>observer.spawns.length>1,'initial configured source spawn');
  assert.equal(observer.spawns.at(-1).hex,witness('setworldspawn 12 64 18 540').spawn_packet_hex);
  let source=await connect('SpawnSource');assert.equal(source.tree,null);
  assert.deepEqual([source.initial.x,source.initial.y,source.initial.z,source.initial.yaw],[12.5,65,18.5,-180]);report.actual_source_position_rotation=[12.5,65,18.5,-180];
  const before=observer.spawns.length;await source.denied('setworldspawn 12 64 18');await barrier(observer);assert.equal(observer.spawns.length,before);report.nonop_denied_without_mutation=true;
  consoleLine('op SpawnSource');await source.wait(()=>source.tree!==null,'permission tree');
  assert.deepEqual(source.tree,[{path:'setworldspawn',executable:true,parser:null},{path:'setworldspawn pos',executable:true,parser:'minecraft:block_pos'},{path:'setworldspawn pos angle',executable:true,parser:'minecraft:angle'}]);report.tree=source.tree;
  const grammar=vectors.grammar.find(g=>g.permission===2);report.completions=[];
  for(const row of grammar.completions.filter(c=>c.input.startsWith('setworldspawn '))){const c=await source.complete(row.input);assert.deepEqual(c,{start:row.start,end:row.end,texts:row.texts});report.completions.push({input:row.input,...c});}
  const invalid=observer.spawns.length;for(const command of ['setworldspawn 30000000 64 0','setworldspawn 1 2','setworldspawn 12 64 18 4 extra'])await source.denied(command);await barrier(observer);assert.equal(observer.spawns.length,invalid);report.invalid_denied_without_mutation=true;
  await execute(source,observer,witness('setworldspawn 12 64 18'));
  const fresh=await connect('SpawnFresh');assert.equal(fresh.spawns[0].hex,witness('setworldspawn 12 64 18').spawn_packet_hex);assert.deepEqual([fresh.initial.x,fresh.initial.y,fresh.initial.z,fresh.initial.yaw],[12.5,65,18.5,0]);await fresh.close();report.fresh_admission_uses_spawn=true;
  for(const input of ['setworldspawn ~1 ~-1 ~2 ~30','setworldspawn ^1 ^0 ^2 ~-450','setworldspawn'])await execute(source,observer,witness(input));
  const consoleRow=vectors.execution.find(r=>r.console_case);await execute(source,observer,consoleRow,true);
  const center=observer.centers.length;consoleLine('worldborder center ~ ~');await observer.wait(()=>observer.centers.length>center,'dynamic console coordinates');assert.deepEqual(observer.centers.at(-1),{x:13,z:19});report.console_coordinates_follow_spawn=true;
  const saveOffset=source.messages.length;source.client.write('chat_command',{command:'save-all flush'});await source.wait(()=>source.messages.slice(saveOffset).some(m=>m.component.translate==='commands.save.success'),'existing explicit save feedback');
  const saves=JSON.parse(await readFile('fixtures/save-all-command-vectors.json','utf8')).execution.find(r=>r.input==='save-all flush'&&r.save_success);
  for(const expected of saves.feedback){assert.ok(source.messages.slice(saveOffset).some(m=>m.hex===expected.hex),'existing save feedback regression');}report.explicit_save_feedback_preserved=true;
  await stop();const saved=JSON.parse(await readFile(path.join(f,'world.json'),'utf8'));assert.deepEqual(saved.world_spawn,{x:13,y:65,z:19,angle:45});report.native_checkpoint_spawn=saved.world_spawn;
  await start(f);source=await connect('SpawnSource');assert.equal(source.spawns[0].hex,consoleRow.spawn_packet_hex);assert.equal(source.initial.x,12.5);assert.equal(source.initial.z,18.5);report.saved_player_position_preserved=true;
  const next=await connect('SpawnNew');assert.deepEqual([next.initial.x,next.initial.y,next.initial.z,next.initial.yaw],[13.5,66,19.5,45]);report.cold_restart=true;await stop();
  const legacy=await prepare('legacy');await writeFile(path.join(legacy,'world.json'),JSON.stringify({format:'mc121-flat-diffs-v1',minecraft:'1.21',changes:[]}));await start(legacy);const old=await connect('SpawnLegacy');assert.deepEqual(old.spawns[0].packet,{location:{x:8,y:64,z:8},angle:0});report.legacy_default=true;await stop();
  const imported=await prepare('imported');await metadataWorld(imported);await start(imported,true);
  const status=await new Promise((resolve,reject)=>mc.ping({host:'127.0.0.1',port:25704,version:'1.21',closeTimeout:5000},(error,value)=>error?reject(error):resolve(value)));assert.equal(status.version.protocol,767);
  const offset=log.length;consoleLine('setworldspawn 20 70 -20 -90');await until(()=>log.slice(offset).includes('commands.setworldspawn.success'),'imported console mutation');await stop();
  const readLevel=async()=>nbt.simplify((await nbt.parse(await readFile(path.join(imported,'world/level.dat')))).parsed);
  let level=await readLevel();assert.deepEqual([level.Data.SpawnX,level.Data.SpawnY,level.Data.SpawnZ,level.Data.SpawnAngle],[20,70,-20,-90]);assert.equal(level.Data.opaque,'spawn-unknown-field');assert.equal(level.extension,'retained');
  await start(imported,true);await stop();level=await readLevel();assert.deepEqual([level.Data.SpawnX,level.Data.SpawnY,level.Data.SpawnZ,level.Data.SpawnAngle],[20,70,-20,-90]);assert.equal(level.Data.opaque,'spawn-unknown-field');report.imported_level_dat_spawn_and_unknown_fields_preserved=true;
  report.ok=true;
}catch(error){report.error=error.stack;process.exitCode=1;}
finally{try{await stop();}catch(error){report.shutdown_error=error.message;report.ok=false;process.exitCode=1;server?.kill('SIGKILL');}report.completed_at=new Date().toISOString();await writeFile(output,JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify({ok:report.ok,report:output,local_diagnostics:scratch,error:report.error}));}
