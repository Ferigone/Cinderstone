#!/usr/bin/env node
// End-to-end semantic probe; the same client operations run against Rust and
// an unchanged Java oracle. This deliberately covers ordinary stone only.
import mc from 'minecraft-protocol';
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {mkdir,writeFile,rm} from 'node:fs/promises';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const args=process.argv.slice(2);
const option=(name,fallback)=>{let i=args.indexOf(name);return i<0?fallback:args[i+1];};
const port=Number(option('--port','25570'));
const binary=option('--server-binary',null);
const label=option('--label','rust');
let server,log='',client,closing=false;
const errors=[],packets=[];
const pause=ms=>new Promise(r=>setTimeout(r,ms));
const report={ok:false,scope:'creative stone placement, inventory and time over protocol767',checks:{}};
async function until(predicate,label,timeout=15000){
 const deadline=Date.now()+timeout;
 while(!predicate()){
  if(errors.length)throw errors[0];
  if(server&&server.exitCode!==null)throw new Error('Server exited: '+log.slice(-3000));
  if(Date.now()>deadline)throw new Error('Deadline waiting for '+label);
  await pause(20);
 }
}
try{
 if(binary){
  const world=resolve(root,'runtime/creative-probe-world.json');
  await rm(world,{force:true});
  server=spawn(resolve(root,binary),['--allow-offline','--port',String(port),'--world',world],{cwd:root,stdio:['ignore','pipe','pipe']});
  server.stdout.on('data',d=>log+=d);server.stderr.on('data',d=>log+=d);
  await until(()=>log.includes('"event":"listening"'),'startup');
 }
 client=mc.createClient({host:'127.0.0.1',port,username:'RustActionProbe',auth:'offline',version:'1.21',keepAlive:false,hideErrors:true});
 client.on('error',e=>{if(!closing)errors.push(e);});
 client.on('end',r=>{if(!closing)errors.push(new Error('Disconnected: '+r));});
 client.on('kick_disconnect',d=>errors.push(new Error('Kick: '+JSON.stringify(d))));
 client.on('keep_alive',p=>client.write('keep_alive',p));
 client.on('chunk_batch_finished',()=>client.write('chunk_batch_received',{chunksPerTick:16}));
 let position;
 client.on('position',p=>{
  position=p;
  client.write('teleport_confirm',{teleportId:p.teleportId});
  client.write('position_look',{x:p.x,y:p.y,z:p.z,yaw:p.yaw,pitch:p.pitch,onGround:true});
 });
 client.on('packet',(p,m)=>{if(m.state==='play')packets.push({name:m.name,data:p});});
 await until(()=>position&&packets.some(p=>p.name==='window_items'),'position and initial inventory');
 // For an externally controlled oracle, the harness sends /tp after join.
 if(args.includes('--wait-common-position')) await until(()=>position.x===8.5&&position.y===65&&position.z===8.5,'common test position');
 const initial=packets.find(p=>p.name==='window_items').data;
 assert.equal(initial.items.length,46);
 assert.ok(initial.items.every(s=>!s.itemCount));
 report.checks.empty_player_inventory_46_slots=true;
 await until(()=>packets.some(p=>p.name==='update_time'),'world time');
 report.checks.world_time=true;
 client.write('held_item_slot',{slotId:0});
 client.write('set_creative_slot',{slot:36,item:{itemCount:64,itemId:1,addedComponentCount:0,removedComponentCount:0,components:[],removeComponents:[]}});
 await pause(150);
 const target={x:10,y:63,z:8};
 const place={x:10,y:64,z:8};
 const isChange=(p,state)=>p.name==='block_change'&&p.data.location.x===place.x&&p.data.location.y===place.y&&p.data.location.z===place.z&&p.data.type===state;
 client.write('block_place',{hand:0,location:target,direction:1,cursorX:.5,cursorY:1,cursorZ:.5,insideBlock:false,sequence:101});
 await until(()=>packets.some(p=>isChange(p,1))&&packets.some(p=>p.name==='acknowledge_player_digging'&&p.data.sequenceId>=101),'stone placement and ack');
 report.checks.creative_slot_selected_and_stone_placed=true;
 client.write('block_dig',{status:0,location:place,face:1,sequence:102});
 await until(()=>packets.some(p=>isChange(p,0))&&packets.some(p=>p.name==='acknowledge_player_digging'&&p.data.sequenceId>=102),'stone break and ack');
 report.checks.creative_stone_break=true;
 report.ok=true;
}catch(error){report.error=String(error.stack??error);process.exitCode=1;}
finally{
 closing=true;client?.end();
 if(server&&server.exitCode===null){server.kill('SIGINT');await Promise.race([new Promise(r=>server.once('exit',r)),pause(5000)]);if(server.exitCode===null)server.kill('SIGKILL');}
 await mkdir(resolve(root,'evidence'),{recursive:true});
 await writeFile(resolve(root,'evidence/creative-live-'+label+'.json'),JSON.stringify(report,null,2)+'\n');
 console.log(JSON.stringify(report));
}
