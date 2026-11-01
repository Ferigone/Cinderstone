#!/usr/bin/env node
// Independent clients verify visible player entities, movement and leave
// cleanup. Optional child startup keeps server/client in one network namespace.
import mc from 'minecraft-protocol';
import minecraftData from 'minecraft-data';
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {mkdir,readFile,writeFile} from 'node:fs/promises';

const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const args=process.argv.slice(2);
function option(name,fallback){const i=args.indexOf(name);return i<0?fallback:args[i+1];}
const port=Number(option('--port','25569'));
const binary=option('--server-binary',null);
const checkEquipment=args.includes('--equipment'),md=minecraftData('1.21');
const checkTracking=args.includes('--tracking-window');
const settings=viewDistance=>({locale:'en_us',viewDistance,chatFlags:0,chatColors:true,skinParts:127,mainHand:1,enableTextFiltering:false,enableServerListing:true});
const pause=ms=>new Promise(r=>setTimeout(r,ms));
const clients=[];
let server,serverLog='';
const report={ok:false,scope:'protocol767 player entity replication',checks:{}};
async function waitUntil(predicate,label,timeout=15000){
 const deadline=Date.now()+timeout;
 while(!predicate()){
  const failure=clients.flatMap(c=>c.errors)[0];
  if(failure)throw failure;
  if(server&&server.exitCode!==null)throw new Error('Rust server exited: '+serverLog.slice(-3000));
  if(Date.now()>deadline)throw new Error('Deadline waiting for '+label);
  await pause(20);
 }
}
async function connect(username){
 const state={username,id:null,position:null,join:null,chunks:new Set(),radiusPackets:[],expectedClose:false,errors:[],spawns:[],infos:[],moves:[],heads:[],animations:[],metadata:[],equipment:[],statuses:[],destroyed:[],removed:[]};
 const client=mc.createClient({host:'127.0.0.1',port,username,auth:'offline',version:'1.21',keepAlive:false,hideErrors:true,clientSettings:settings(2)});
 state.client=client;clients.push(state);
 client.on('error',error=>{if(!state.expectedClose)state.errors.push(error);});
 client.on('end',reason=>{if(!state.expectedClose)state.errors.push(new Error(username+' disconnected: '+String(reason)));});
 client.on('kick_disconnect',data=>state.errors.push(new Error(username+' kicked: '+JSON.stringify(data))));
 client.on('keep_alive',data=>client.write('keep_alive',data));
 client.on('chunk_batch_finished',()=>client.write('chunk_batch_received',{chunksPerTick:16}));
 client.on('position',data=>{
  state.position=data;
  client.write('teleport_confirm',{teleportId:data.teleportId});
  client.write('position_look',{x:data.x,y:data.y,z:data.z,yaw:data.yaw,pitch:data.pitch,onGround:true});
 });
 client.on('packet',(data,metadata)=>{
  if(metadata.state!=='play')return;
  switch(metadata.name){
   case 'login':state.id=data.entityId;state.join=data;break;
   case 'map_chunk':state.chunks.add(data.x+','+data.z);break;
   case 'unload_chunk':state.chunks.delete(data.chunkX+','+data.chunkZ);break;
   case 'update_view_distance':state.radiusPackets.push(data);break;
   case 'spawn_entity':state.spawns.push(data);break;
   case 'player_info':state.infos.push(data);break;
   case 'rel_entity_move':case 'entity_move_look':case 'entity_look':case 'entity_teleport':state.moves.push({name:metadata.name,...data});break;
   case 'entity_head_rotation':state.heads.push(data);break;
   case 'animation':state.animations.push(data);break;
   case 'entity_metadata':state.metadata.push(data);break;
   case 'entity_equipment':state.equipment.push(data);break;
   case 'entity_status':state.statuses.push(data);break;
   case 'entity_destroy':state.destroyed.push(...data.entityIds);break;
   case 'player_remove':state.removed.push(...data.players);break;
  }
 });
 await waitUntil(()=>state.id!==null&&state.position,'PLAY and initial position for '+username);
 return state;
}
try{
 if(binary){
  server=spawn(resolve(root,binary),['--allow-offline','--port',String(port),'--view-distance',checkTracking?'6':'2','--world','runtime/entity-conformance-world.json'],{cwd:root,stdio:['ignore','pipe','pipe']});
  server.stdout.on('data',data=>{serverLog+=data;});server.stderr.on('data',data=>{serverLog+=data;});
  await waitUntil(()=>serverLog.includes('"event":"listening"'),'server startup');
 }
 const a=await connect('EntityObserver');
 if(checkTracking){
  const golden=JSON.parse(await readFile(resolve(root,'fixtures/entity_tracking_views.json'),'utf8'));
  const expected=radius=>new Set(golden.iterations.find(row=>row.radius===radius&&row.center[0]===0&&row.center[1]===0).expected.map(p=>p[0]+','+p[1]));
  const small=expected(2),large=expected(6);assert.equal(small.size,45);assert.equal(large.size,201);
  await waitUntil(()=>a.chunks.size===45,'original 45 chunk view');assert.deepEqual(a.chunks,small);assert.equal(a.join.viewDistance,6);
  report.checks.original_rounded_requested_view_window=true;
  a.client.write('settings',settings(6));await waitUntil(()=>a.chunks.size===201,'expanded stationary view');assert.deepEqual(a.chunks,large);
  a.client.write('settings',settings(2));await waitUntil(()=>a.chunks.size===45,'shrunk stationary view');assert.deepEqual(a.chunks,small);assert.equal(a.radiusPackets.length,0);
  report.checks.settings_changes_view_without_movement_or_server_radius_packet=true;
 }
 const b=await connect('EntitySubject');
 await waitUntil(()=>a.spawns.some(p=>p.entityId===b.id&&p.type===128)&&b.spawns.some(p=>p.entityId===a.id&&p.type===128),'mutual player spawns');
 assert.ok(a.infos.some(p=>p.data.some(e=>e.player?.name==='EntitySubject')));
 assert.ok(b.infos.some(p=>p.data.some(e=>e.player?.name==='EntityObserver')));
 assert.ok(!a.spawns.some(p=>p.entityId===a.id));assert.ok(!b.spawns.some(p=>p.entityId===b.id));
 report.checks.mutual_player_info_and_spawns=true;report.checks.no_self_spawn=true;
 await pause(100);
 const start=b.position;
 const moveStart=a.moves.length;
 b.client.write('position_look',{x:start.x+.25,y:start.y,z:start.z,yaw:90,pitch:0,onGround:true});
 await waitUntil(()=>a.moves.slice(moveStart).some(p=>p.entityId===b.id&&p.name==='entity_move_look'&&p.dX===1024&&p.dY===0&&p.dZ===0&&p.yaw===64),'relative position and rotation');
 await waitUntil(()=>a.heads.some(p=>p.entityId===b.id&&p.headYaw===64),'head rotation');
 report.checks.relative_move_and_head_rotation=true;
 b.client.write('arm_animation',{hand:0});
 await waitUntil(()=>a.animations.some(p=>p.entityId===b.id&&p.animation===0),'main-hand animation');
 report.checks.arm_animation=true;
 const hasMetadata=(from,key,predicate)=>a.metadata.slice(from).some(packet=>packet.entityId===b.id&&packet.metadata.some(entry=>entry.key===key&&predicate(entry.value)));
 let metadataStart=a.metadata.length;
 b.client.write('settings',{locale:'en_us',viewDistance:2,chatFlags:0,chatColors:true,skinParts:63,mainHand:0,enableTextFiltering:false,enableServerListing:true});
 await waitUntil(()=>hasMetadata(metadataStart,17,value=>value===63)&&hasMetadata(metadataStart,18,value=>value===0),'skin and dominant-hand changes');
 metadataStart=a.metadata.length;
 b.client.write('entity_action',{entityId:b.id,actionId:'start_sneaking',jumpBoost:0});
 await waitUntil(()=>hasMetadata(metadataStart,0,value=>(value&2)!==0),'sneak flag set');
 metadataStart=a.metadata.length;
 b.client.write('entity_action',{entityId:b.id,actionId:'stop_sneaking',jumpBoost:0});
 await waitUntil(()=>hasMetadata(metadataStart,0,value=>(value&2)===0),'sneak flag reset to default');
 report.checks.skin_main_hand_and_sneak_resets=true;
 if(checkEquipment){
  const item=(name,count=1)=>({itemCount:count,itemId:md.itemsByName[name].id,addedComponentCount:0,removedComponentCount:0,components:[],removeComponents:[]});
  const equipped=(from,slot,name,count)=>a.equipment.slice(from).some(packet=>packet.entityId===b.id&&packet.equipments.some(entry=>entry.slot===slot&&entry.item.itemCount===count&&(count===0||entry.item.itemId===md.itemsByName[name].id)));
  let from=a.equipment.length;
  b.client.write('set_creative_slot',{slot:36,item:item('stone',3)});b.client.write('set_creative_slot',{slot:45,item:item('dirt',2)});b.client.write('set_creative_slot',{slot:5,item:item('iron_helmet')});
  await waitUntil(()=>equipped(from,0,'stone',3)&&equipped(from,1,'dirt',2)&&equipped(from,5,'iron_helmet',1),'held/off-hand/head equipment');
  from=a.equipment.length;b.client.write('held_item_slot',{slotId:8});await waitUntil(()=>equipped(from,0,null,0),'empty selected hotbar equipment');
  from=a.equipment.length;b.client.write('held_item_slot',{slotId:0});await waitUntil(()=>equipped(from,0,'stone',3),'restored selected hotbar equipment');
  const statuses=a.statuses.length;from=a.equipment.length;
  b.client.write('block_dig',{status:6,location:{x:0,y:0,z:0},face:0,sequence:77});
  await waitUntil(()=>a.statuses.slice(statuses).some(packet=>packet.entityId===b.id&&packet.entityStatus===55),'hand-swap status55');
  await pause(100);assert.ok(a.equipment.slice(from).every(packet=>packet.entityId!==b.id||packet.equipments.every(entry=>entry.slot!==0&&entry.slot!==1)),'hand swap suppresses redundant hand equipment entries');
  assert.ok(!b.equipment.some(packet=>packet.entityId===b.id),'self equipment is driven by local inventory');
  report.checks.equipment_hands_armor_hotbar_and_swap=true;
 }
 b.client.write('position_look',{x:start.x+9,y:start.y,z:start.z,yaw:-1,pitch:-1,onGround:true});
 await waitUntil(()=>a.moves.some(p=>p.entityId===b.id&&p.name==='entity_teleport'&&p.x===start.x+9&&p.yaw===0&&p.pitch===0),'absolute teleport after relative overflow');
 report.checks.large_move_teleport=true;
 const subjectUuid=a.spawns.find(p=>p.entityId===b.id).objectUUID;
 const countAddedInfo=()=>a.infos.filter(p=>p.action?.add_player).flatMap(p=>p.data).filter(p=>p.uuid===subjectUuid).length;
 const originalInfos=countAddedInfo();assert.equal(originalInfos,1,'one subject tab-list identity');
 const originalSpawns=a.spawns.filter(p=>p.entityId===b.id).length;
 const viewLeaves=a.destroyed.filter(id=>id===b.id).length;
 b.client.write('position_look',{x:start.x+56,y:start.y,z:start.z,yaw:0,pitch:0,onGround:true});
 await waitUntil(()=>a.destroyed.filter(id=>id===b.id).length>viewLeaves,'entity leaves observer chunk window');
 assert.ok(!a.removed.includes(subjectUuid),'leaving view must retain the tab-list identity');
 b.client.write('position_look',{x:start.x+.5,y:start.y,z:start.z,yaw:0,pitch:0,onGround:true});
 await waitUntil(()=>a.spawns.filter(p=>p.entityId===b.id).length>originalSpawns,'entity re-enters observer chunk window');
 await waitUntil(()=>a.moves.some(p=>p.entityId===b.id&&p.name==='entity_teleport'&&p.x===start.x+.5),'re-entry catches up to global movement baseline');
 assert.equal(countAddedInfo(),originalInfos,'view entry must not duplicate tab-list ADD_PLAYER');
 report.checks.view_leave_return_keeps_identity=true;
 if(checkTracking){
  let removed=a.destroyed.filter(id=>id===b.id).length;
  b.client.write('position_look',{x:start.x+30,y:start.y,z:start.z+30,yaw:0,pitch:0,onGround:true});
  await waitUntil(()=>a.destroyed.filter(id=>id===b.id).length>removed,'loaded diagonal chunk outside entity radius');assert.ok(a.chunks.has('2,2'));
  const spawns=a.spawns.filter(p=>p.entityId===b.id).length;
  a.client.write('position_look',{x:start.x+8,y:start.y,z:start.z+8,yaw:0,pitch:0,onGround:true});
  await waitUntil(()=>a.spawns.filter(p=>p.entityId===b.id).length>spawns,'moving observer pairs stationary subject');
  removed=a.destroyed.filter(id=>id===b.id).length;
  a.client.write('position_look',{x:start.x,y:start.y,z:start.z,yaw:0,pitch:0,onGround:true});
  await waitUntil(()=>a.destroyed.filter(id=>id===b.id).length>removed,'moving observer unpairs stationary subject');
  const returned=a.spawns.filter(p=>p.entityId===b.id).length;
  b.client.write('position_look',{x:start.x+.5,y:start.y,z:start.z,yaw:0,pitch:0,onGround:true});
  await waitUntil(()=>a.spawns.filter(p=>p.entityId===b.id).length>returned,'subject returns inside radius');
  report.checks.horizontal_range_and_observer_movement=true;
 }
 const destroyedBeforeDisconnect=a.destroyed.filter(id=>id===b.id).length;
 b.expectedClose=true;b.client.end();
 await waitUntil(()=>a.destroyed.filter(id=>id===b.id).length>destroyedBeforeDisconnect&&a.removed.includes(subjectUuid),'entity and tab-list removal');
 report.checks.entity_and_player_list_removed=true;
 report.ok=true;
}catch(error){report.error=String(error.stack??error);process.exitCode=1;}
finally{
 for(const state of clients){state.expectedClose=true;state.client.end();}
 if(server&&server.exitCode===null){
  server.kill('SIGINT');
  await Promise.race([new Promise(r=>server.once('exit',r)),pause(5000)]);
  if(server.exitCode===null)server.kill('SIGKILL');
 }
 await mkdir(resolve(root,'evidence'),{recursive:true});
 await writeFile(resolve(root,'evidence/entity-live-conformance.json'),JSON.stringify(report,null,2)+'\n');
 if(server)await writeFile(resolve(root,'evidence/entity-live-server.log'),serverLog);
 console.log(JSON.stringify(report));
}
