#!/usr/bin/env node
// Fresh native item-callback scope: actual request_stats packet and dirty drain.
import mc from 'minecraft-protocol';import assert from 'node:assert/strict';import {spawn} from 'node:child_process';
import {readFile,writeFile,mkdir} from 'node:fs/promises';import {createHash} from 'node:crypto';import {resolve} from 'node:path';
const args=process.argv.slice(2),opt=(n,d)=>args.includes(n)?args[args.indexOf(n)+1]:d;
const binary=resolve(opt('--server-binary','runtime/phase5-floor-final/mc121-rust')),port=Number(opt('--port','25627'));
let server,client,log='',closing=false,ready=false,chunks=0,held=0,item=null,collected=false;const stats=[],errors=[],timeline=[],moves=[];let inventory=[],stateId=0;
const report={ok:false,scope:'fresh NativeItemCallbacksOnly request_stats, drop/pickup dirty deltas',full_player_tick_parity:false,checks:{}};
const pause=ms=>new Promise(r=>setTimeout(r,ms));async function until(test,label){const end=Date.now()+15000;while(!test()){if(errors.length)throw errors[0];if(server?.exitCode!==null&&server?.exitCode!==undefined)throw Error('server exited '+log.slice(-3000));if(Date.now()>end)throw Error('deadline '+label);await pause(15);}}
try {
 server=spawn(binary,['--allow-offline','--port',String(port),'--world',`runtime/stats-probe-${process.pid}.json`],{stdio:['ignore','pipe','pipe']});server.stdout.on('data',b=>log+=b);server.stderr.on('data',b=>log+=b);await until(()=>log.includes('"event":"listening"'),'listen');
 client=mc.createClient({host:'127.0.0.1',port,username:'ItemStatsProbe',auth:'offline',version:'1.21',keepAlive:false,hideErrors:true});client.on('error',e=>{if(!closing)errors.push(e);});client.on('end',r=>{if(!closing)errors.push(Error(String(r)));});client.on('kick_disconnect',d=>errors.push(Error(JSON.stringify(d))));client.on('keep_alive',d=>client.write('keep_alive',d));
 client.on('position',d=>{client.write('teleport_confirm',{teleportId:d.teleportId});ready=true;});client.on('map_chunk',()=>chunks++);client.on('chunk_batch_finished',()=>client.write('chunk_batch_received',{chunksPerTick:16}));
 client.on('window_items',d=>{if(d.windowId===0){inventory=d.items;stateId=d.stateId;}});
 client.on('set_slot',d=>{stateId=d.stateId;if(d.windowId===0&&d.slot>=0){inventory[d.slot]=d.item;if(d.slot===36)held=d.item.itemCount;}});
 client.on('packet',(data,meta)=>{if(['spawn_entity','entity_teleport','rel_entity_move','entity_move_look','collect','entity_destroy','set_slot','window_items','statistics'].includes(meta.name)&&timeline.length<600)timeline.push({name:meta.name,data:structuredClone(data)});});client.on('statistics',d=>stats.push(d.entries));
 client.on('spawn_entity',d=>{if(d.type===58)item={id:d.entityId,x:d.x,y:d.y,z:d.z};});
 client.on('rel_entity_move',d=>{if(item?.id===d.entityId){item.x+=d.dX/4096;item.y+=d.dY/4096;item.z+=d.dZ/4096;}});
 client.on('entity_move_look',d=>{if(item?.id===d.entityId){item.x+=d.dX/4096;item.y+=d.dY/4096;item.z+=d.dZ/4096;}});
 client.on('entity_teleport',d=>{if(item?.id===d.entityId)Object.assign(item,{x:d.x,y:d.y,z:d.z});});client.on('collect',d=>{if(item?.id===d.collectedEntityId)collected=true;});
 await until(()=>ready&&chunks===45,'initial45');client.write('held_item_slot',{slotId:0});client.write('set_creative_slot',{slot:36,item:{itemCount:3,itemId:1,addedComponentCount:0,removedComponentCount:0,components:[],removeComponents:[]}});await until(()=>held===3,'creative held3');
 client.write('block_dig',{status:4,location:{x:0,y:0,z:0},face:0,sequence:0});await until(()=>item,'drop');assert.equal(stats.length,0);report.checks.awards_do_not_emit_unsolicited_stats=true;
 client.write('client_command',{actionId:1});await until(()=>stats.length===1,'drop stats');const has=(rows,c,id,value)=>rows.some(r=>r.categoryId===c&&r.statisticId===id&&r.value===value);
 assert.ok(has(stats[0],5,1,1));assert.ok(has(stats[0],8,22,1));report.checks.request_stats_contains_drop_and_custom_drop=true;
 client.write('client_command',{actionId:1});await until(()=>stats.length===2,'empty second response');assert.deepEqual(stats[1],[]);report.checks.repeated_request_emits_empty_dirty_delta=true;
 let moved=0;await until(()=>{if(!collected&&Date.now()-moved>80){moved=Date.now();const target={x:item.x,y:Math.max(64,item.y+0.05),z:item.z,onGround:true};moves.push({...target,at:Date.now()});client.write('position',target);}return collected;},'pickup after actual delay');
 assert.equal(stats.length,2);client.write('client_command',{actionId:1});await until(()=>stats.length===3,'pickup stats');assert.ok(has(stats[2],4,1,1));report.checks.pickup_stat_is_next_requested_dirty_delta=true;
 report.ok=true;report.statistics=stats;report.server_binary_sha256=createHash('sha256').update(await readFile(binary)).digest('hex');
}catch(e){report.error=String(e.stack??e);process.exitCode=1;
 if(client&&server?.exitCode===null){client.write('client_command',{actionId:1});client.write('window_click',{windowId:0,stateId:(stateId+32767)&32767,slot:-1,mouseButton:0,mode:0,changedSlots:[],cursorItem:{itemCount:0}});await pause(350);}
}
finally{report.diagnostics={item,collected,held,inventory,stateId,statistics:stats,movements:moves,timeline};closing=true;client?.end();if(server&&server.exitCode===null){server.kill('SIGINT');await Promise.race([new Promise(r=>server.once('exit',r)),pause(5000)]);if(server.exitCode===null)server.kill('SIGKILL');}await mkdir('evidence',{recursive:true});await writeFile('evidence/item-stats-live-conformance.json',JSON.stringify(report,null,2)+'\n');await writeFile('runtime/phase5-floor-final/stats-server.log',log);console.log(JSON.stringify(report));}
process.exit(process.exitCode??0);
