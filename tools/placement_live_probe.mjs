#!/usr/bin/env node
import mc from 'minecraft-protocol';
import minecraftData from 'minecraft-data';
import blockLoader from 'prismarine-block';
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {randomUUID} from 'node:crypto';
import {mkdir,writeFile} from 'node:fs/promises';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const args=process.argv.slice(2),opt=(n,d)=>{let i=args.indexOf(n);return i<0?d:args[i+1];};
const port=Number(opt('--port','25573')),binary=opt('--server-binary',null),label=opt('--label','rust');
const data=minecraftData('1.21'),Block=blockLoader('1.21');
const pause=ms=>new Promise(r=>setTimeout(r,ms));
const updates=new Map(),acks=new Set(),errors=[];
let server,client,log='',position,closing=false,sequence=200;
const report={ok:false,scope:'native placement state and neighbor updates compared with vanilla',checks:{}};
const key=p=>`${p.x},${p.y},${p.z}`;
async function until(f,label,timeout=15000){const end=Date.now()+timeout;while(!f()){if(errors.length)throw errors[0];if(server&&server.exitCode!==null)throw new Error(log);if(Date.now()>end)throw new Error('Deadline '+label);await pause(20);}}
function properties(p){let id=updates.get(key(p));return id===undefined?null:Block.fromStateId(id,0).getProperties();}
async function place(item,location,direction,expected,check){
 const seq=++sequence;
 client.write('set_creative_slot',{slot:36,item:{itemCount:64,itemId:data.itemsByName[item].id,addedComponentCount:0,removedComponentCount:0,components:[],removeComponents:[]}});
 client.write('block_place',{hand:0,location,direction,cursorX:.5,cursorY:direction===1?1:.5,cursorZ:.5,insideBlock:false,sequence:seq});
 await until(()=>acks.has(seq)&&check(properties(expected)),'placement '+item);
}
try{
 if(binary){
  server=spawn(resolve(root,binary),['--allow-offline','--port',String(port),'--world',resolve(root,`runtime/placement-live-${randomUUID()}.json`)],{cwd:root,stdio:['ignore','pipe','pipe']});
  server.stdout.on('data',d=>log+=d);server.stderr.on('data',d=>log+=d);
  await until(()=>log.includes('"event":"listening"'),'startup');
 }
 client=mc.createClient({host:'127.0.0.1',port,username:'RustActionProbe',auth:'offline',version:'1.21',keepAlive:false,hideErrors:true});
 client.on('error',e=>{if(!closing)errors.push(e);});client.on('end',r=>{if(!closing)errors.push(new Error('Disconnected '+r));});
 client.on('kick_disconnect',d=>errors.push(new Error(JSON.stringify(d))));
 client.on('keep_alive',d=>client.write('keep_alive',d));client.on('chunk_batch_finished',()=>client.write('chunk_batch_received',{chunksPerTick:16}));
 client.on('position',p=>{position=p;client.write('teleport_confirm',{teleportId:p.teleportId});client.write('position_look',{x:p.x,y:p.y,z:p.z,yaw:0,pitch:0,onGround:true});});
 client.on('block_change',p=>updates.set(key(p.location),p.type));
 // Vanilla batches multiple changes in one section, including the existing
 // fence's connection change. Decode those just like individual block updates.
 client.on('multi_block_change',p=>{for(const record of p.records){
  const location={x:p.chunkCoordinates.x*16+((record>>>8)&15),y:p.chunkCoordinates.y*16+(record&15),z:p.chunkCoordinates.z*16+((record>>>4)&15)};
  updates.set(key(location),record>>>12);
 }});
 client.on('acknowledge_player_digging',p=>acks.add(p.sequenceId));
 await until(()=>position,'position');
 if(args.includes('--wait-common-position'))await until(()=>position.x===8.5&&position.y===65&&position.z===8.5,'common position');
 client.write('held_item_slot',{slotId:0});
 const base={x:5,y:64,z:8},logpos={x:6,y:64,z:8};
 await place('stone',{x:5,y:63,z:8},1,base,p=>p!==null);
 await place('oak_log',base,5,logpos,p=>p?.axis==='x');
 report.checks.pillar_axis_from_clicked_face=true;
 const slab={x:10,y:64,z:10};
 await place('oak_slab',{x:10,y:63,z:10},1,slab,p=>p?.type==='bottom');
 await place('oak_slab',slab,1,slab,p=>p?.type==='double'&&p.waterlogged===false);
 report.checks.slab_merge_and_waterlogged_reset=true;
 const a={x:6,y:64,z:10},b={x:7,y:64,z:10};
 await place('oak_fence',{x:6,y:63,z:10},1,a,p=>p?.east===false);
 await place('oak_fence',{x:7,y:63,z:10},1,b,p=>p?.west===true);
 await until(()=>properties(a)?.east===true,'existing fence neighbor connection');
 const dig=++sequence;client.write('block_dig',{status:0,location:b,face:1,sequence:dig});
 await until(()=>acks.has(dig)&&updates.get(key(b))===0&&properties(a)?.east===false,'fence neighbor disconnection');
 report.checks.fence_neighbors_connect_and_disconnect=true;
 client.write('position_look',{x:8.5,y:65,z:8.5,yaw:90,pitch:0,onGround:true});
 const stair={x:10,y:64,z:6};
 await place('oak_stairs',{x:10,y:63,z:6},1,stair,p=>p?.facing==='west'&&p.half==='bottom'&&p.shape==='straight');
 report.checks.stair_orientation_from_player_yaw=true;
 report.ok=true;
}catch(e){report.error=String(e.stack??e);process.exitCode=1;}
finally{
 closing=true;client?.end();
 if(server&&server.exitCode===null){server.kill('SIGINT');await Promise.race([new Promise(r=>server.once('exit',r)),pause(5000)]);if(server.exitCode===null)server.kill('SIGKILL');}
 await mkdir(resolve(root,'evidence'),{recursive:true});await writeFile(resolve(root,`evidence/placement-live-${label}.json`),JSON.stringify(report,null,2)+'\n');
 console.log(JSON.stringify(report));
}
