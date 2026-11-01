#!/usr/bin/env node
// Independent protocol clients verify replacement of a live offline UUID.
import mc from 'minecraft-protocol';
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {createHash,randomUUID} from 'node:crypto';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';

const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const args=process.argv.slice(2);
const option=(name,fallback)=>args.includes(name)?args[args.indexOf(name)+1]:fallback;
const port=Number(option('--port','25579'));
const binary=resolve(root,option('--server-binary','target/debug/mc121-rust'));
const rounds=Number(option('--replacements','3'));
assert.ok(Number.isInteger(rounds)&&rounds>0&&rounds<=20,'replacements must be1..20');
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const clients=[];
let server,log='';
const report={ok:false,minecraft:'1.21',protocol:767,scope:'live duplicate offline UUID replacement, disconnect and observer cleanup',checks:{},rounds:[],not_verified:['forced entity-broadcast lag or slow-writer cancellation','Microsoft authentication']};

async function until(predicate,label,timeout=15000){
 const deadline=performance.now()+timeout;
 while(!predicate()){
  const error=clients.flatMap(state=>state.errors)[0];if(error)throw error;
  if(server&&server.exitCode!==null)throw new Error('server exited: '+log.slice(-2500));
  if(performance.now()>deadline)throw new Error('deadline: '+label);
  await pause(20);
 }
}

async function connect(username,label){
 const state={username,label,id:null,ready:false,kick:null,ended:false,expectedDisconnect:false,closing:false,errors:[],spawns:[],destroyed:[],removed:[],infos:[],animations:[],timeline:[],tab:new Map()};
 const client=mc.createClient({host:'127.0.0.1',port,username,auth:'offline',version:'1.21',keepAlive:false,hideErrors:true});
 state.client=client;clients.push(state);
 client.on('error',error=>{if(!state.closing&&!state.expectedDisconnect)state.errors.push(error);});
 client.on('end',reason=>{
  state.ended=true;
  if(!state.closing&&!state.expectedDisconnect)state.errors.push(new Error(label+' ended: '+String(reason)));
 });
 client.on('kick_disconnect',data=>{
  state.kick=data.reason;
  if(!state.closing&&!state.expectedDisconnect)state.errors.push(new Error(label+' kicked: '+JSON.stringify(data)));
 });
 client.on('keep_alive',data=>client.write('keep_alive',data));
 client.on('chunk_batch_finished',()=>client.write('chunk_batch_received',{chunksPerTick:16}));
 client.on('position',data=>{
  client.write('teleport_confirm',{teleportId:data.teleportId});
  client.write('position_look',{x:data.x,y:data.y,z:data.z,yaw:data.yaw,pitch:data.pitch,onGround:true});
  state.ready=true;
 });
 client.on('packet',(data,metadata)=>{
  if(metadata.state!=='play')return;
  switch(metadata.name){
   case 'login':state.id=data.entityId;break;
   case 'spawn_entity':
    state.spawns.push(data);state.timeline.push({kind:'spawn',id:data.entityId,uuid:data.objectUUID});break;
   case 'entity_destroy':
    state.destroyed.push(...data.entityIds);
    for(const id of data.entityIds)state.timeline.push({kind:'destroy',id});
    break;
   case 'player_info':
    state.infos.push(data);
    if(data.action.add_player)for(const entry of data.data){
     state.tab.set(entry.uuid,entry.player.name);
     state.timeline.push({kind:'tab_add',uuid:entry.uuid,name:entry.player.name});
    }
    break;
   case 'player_remove':
    state.removed.push(...data.players);
    for(const uuid of data.players){state.tab.delete(uuid);state.timeline.push({kind:'tab_remove',uuid});}
    break;
   case 'animation':state.animations.push(data);break;
  }
 });
 await until(()=>state.id!==null&&state.ready,label+' initial PLAY');
 return state;
}

try{
 report.server_binary_sha256=createHash('sha256').update(await readFile(binary)).digest('hex');
 server=spawn(binary,['--allow-offline','--port',String(port),'--world','runtime/duplicate-login-'+randomUUID()+'.json'],{cwd:root,stdio:['ignore','pipe','pipe']});
 server.stdout.on('data',data=>log+=data);server.stderr.on('data',data=>log+=data);
 await until(()=>log.includes('"event":"listening"'),'server startup');
 const observer=await connect('DupObserver','observer');
 let subject=await connect('DupSubject','subject0');
 await until(()=>observer.spawns.some(packet=>packet.entityId===subject.id),'initial subject visible');
 const uuid=observer.spawns.find(packet=>packet.entityId===subject.id).objectUUID;
 assert.equal(uuid,subject.client.uuid);
 assert.equal(observer.tab.get(uuid),'DupSubject');
 report.checks.initial_subject_has_visible_entity_and_tab_identity=true;

 for(let round=1;round<=rounds;round++){
  const old=subject,from=observer.timeline.length;
  old.expectedDisconnect=true;
  subject=await connect('DupSubject','subject'+round);
  assert.notEqual(subject.id,old.id,'replacement has a new entity ID');
  assert.equal(subject.client.uuid,uuid,'offline UUID remains the same');
  await until(()=>old.kick&&old.ended,'old session receives disconnect and closes');
  // Protocol767 sends a typed anonymous NBT Component, not JSON text.
  assert.equal(old.kick.type,'compound');
  assert.deepEqual(old.kick.value.translate,{type:'string',value:'multiplayer.disconnect.duplicate_login'});
  await until(()=>observer.destroyed.includes(old.id)&&observer.spawns.some(packet=>packet.entityId===subject.id),'observer replaces subject entity');
  const events=observer.timeline.slice(from);
  const destroy=events.findIndex(event=>event.kind==='destroy'&&event.id===old.id);
  const spawn=events.findIndex(event=>event.kind==='spawn'&&event.id===subject.id);
  const remove=events.findIndex(event=>event.kind==='tab_remove'&&event.uuid===uuid);
  const add=events.findIndex(event=>event.kind==='tab_add'&&event.uuid===uuid);
  assert.ok(destroy>=0&&spawn>destroy,'old entity removed before replacement spawn');
  assert.ok(remove>=0&&add>remove,'tab identity removed then added for replacement');
  assert.equal(observer.tab.get(uuid),'DupSubject');

  // The old connection's TCP close follows its guard cleanup. A successful
  // animation afterwards proves the replacement still exists in the registry.
  const animations=observer.animations.length;
  subject.client.write('arm_animation',{hand:0});
  await until(()=>observer.animations.slice(animations).some(packet=>packet.entityId===subject.id&&packet.animation===0),'replacement survives old presence cleanup');
  await pause(150);
  assert.equal(subject.ended,false);assert.equal(subject.kick,null);
  assert.ok(!observer.destroyed.includes(subject.id));
  report.rounds.push({old_entity_id:old.id,new_entity_id:subject.id,same_uuid:true,nbt_disconnect:'multiplayer.disconnect.duplicate_login',observer_removed_old_before_new_spawn:true,tab_remove_before_add:true,replacement_acts_after_old_close:true});
 }
 report.checks.same_uuid_receives_new_entity_id=true;
 report.checks.old_session_gets_typed_duplicate_login_disconnect=true;
 report.checks.observer_removes_old_entity_and_replaces_tab_identity=true;
 report.checks.old_presence_cleanup_preserves_replacement=true;
 report.checks.replacement_remains_connected_and_can_act=true;
 report.checks.observer_remains_connected=true;
 report.replacements=rounds;report.ok=true;
}catch(error){report.error=String(error.stack??error);process.exitCode=1;}
finally{
 for(const state of clients){state.closing=true;state.client.end();}
 if(server?.exitCode===null){
  server.kill('SIGINT');
  await Promise.race([new Promise(resolve=>server.once('exit',resolve)),pause(5000)]);
  if(server.exitCode===null)server.kill('SIGKILL');
 }
 await mkdir(resolve(root,'evidence'),{recursive:true});
 await writeFile(resolve(root,'evidence/duplicate-login-live-conformance.json'),JSON.stringify(report,null,2)+'\n');
 console.log(JSON.stringify(report));
}
