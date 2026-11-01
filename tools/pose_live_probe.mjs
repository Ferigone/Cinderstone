#!/usr/bin/env node
// Runtime wiring test for the independently verified player-pose algorithm.
import mc from 'minecraft-protocol';
import minecraftData from 'minecraft-data';
import blockLoader from 'prismarine-block';
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {randomUUID} from 'node:crypto';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';

const root=resolve(dirname(fileURLToPath(import.meta.url)),'..'),args=process.argv.slice(2);
const option=(key,fallback)=>args.includes(key)?args[args.indexOf(key)+1]:fallback;
const port=Number(option('--port','25579')),binary=resolve(root,option('--server-binary','target/debug/mc121-rust'));
const md=minecraftData('1.21'),Block=blockLoader('1.21');
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const events=[],errors=[];let server,client,log='',id,ready=false,closing=false,pose=0;
const report={ok:false,scope:'native runtime pose selection, flight input and collision-dependent dimensions; full movement validation excluded',checks:{}};
async function until(predicate,label,timeout=12000){const end=performance.now()+timeout;while(!predicate()){
 if(errors.length)throw errors[0];if(server?.exitCode!==null)throw new Error('server exited: '+log.slice(-3000));
 if(performance.now()>end)throw new Error('deadline '+label);await pause(20);
}}
async function transition(expected,action){const from=events.length;action();await until(()=>events.slice(from).some(e=>e.pose===expected),'pose '+expected);assert.equal(pose,expected);}
function sneak(value){client.write('entity_action',{entityId:id,actionId:value?'start_sneaking':'stop_sneaking',jumpBoost:0});}
function move(x){client.write('position_look',{x,y:64,z:8.5,yaw:0,pitch:0,onGround:true});}
try{
 await mkdir(resolve(root,'runtime'),{recursive:true});
 const slab=md.blocksByName.stone_slab;
 let top;
 for(let state=slab.minStateId;state<=slab.maxStateId;state++){
  const p=Block.fromStateId(state,0).getProperties();if(p.type==='top'&&p.waterlogged===false){top=state;break;}
 }
 assert.notEqual(top,undefined);
 const world=resolve(root,'runtime/pose-'+randomUUID()+'.json');
 await writeFile(world,JSON.stringify({format:'mc121-flat-diffs-v1',minecraft:'1.21',changes:[
  {x:12,y:65,z:8,state:top},{x:15,y:65,z:8,state:md.blocksByName.stone.defaultState},
 ]}));
 report.server_binary_sha256=createHash('sha256').update(await readFile(binary)).digest('hex');
 server=spawn(binary,['--allow-offline','--port',String(port),'--world',world],{cwd:root,stdio:['ignore','pipe','pipe']});
 server.stdout.on('data',data=>log+=data);server.stderr.on('data',data=>log+=data);
 await until(()=>log.includes('"event":"listening"'),'startup');
 client=mc.createClient({host:'127.0.0.1',port,username:'PoseRuntime',auth:'offline',version:'1.21',hideErrors:true});
 client.on('error',error=>{if(!closing)errors.push(error);});
 client.on('end',reason=>{if(!closing)errors.push(new Error('client ended: '+reason));});
 client.on('login',data=>{id=data.entityId;});
 client.on('position',data=>{client.write('teleport_confirm',{teleportId:data.teleportId});move(8.5);ready=true;});
 client.on('chunk_batch_finished',()=>client.write('chunk_batch_received',{chunksPerTick:16}));
 client.on('entity_metadata',data=>{if(data.entityId===id)for(const entry of data.metadata){
  if(entry.key===6){pose=entry.value;events.push({pose});}
 }});
 await until(()=>ready,'join');
 await transition(5,()=>sneak(true));report.checks.sneak_selects_crouching=true;
 await transition(0,()=>sneak(false));report.checks.release_selects_standing=true;
 await transition(5,()=>sneak(true));
 await transition(0,()=>client.write('abilities',{flags:2}));report.checks.flying_ignores_crouch_preference=true;
 await transition(5,()=>client.write('abilities',{flags:0}));report.checks.stopping_flight_restores_crouching=true;
 await transition(0,()=>sneak(false));
 await transition(5,()=>move(12.5));report.checks.half_slab_ceiling_forces_crouching=true;
 await transition(3,()=>move(15.5));report.checks.low_ceiling_forces_swimming_pose=true;
 await transition(0,()=>move(8.5));report.checks.exiting_ceiling_restores_standing=true;
 report.ok=true;
}catch(error){report.error=String(error.stack??error);process.exitCode=1;}
finally{
 closing=true;client?.end();
 if(server?.exitCode===null){server.kill('SIGINT');await Promise.race([new Promise(resolve=>server.once('exit',resolve)),pause(5000)]);if(server.exitCode===null)server.kill('SIGKILL');}
 await mkdir(resolve(root,'evidence'),{recursive:true});
 await writeFile(resolve(root,'evidence/pose-live-conformance.json'),JSON.stringify(report,null,2)+'\n');
 await writeFile(resolve(root,'evidence/pose-live-server.log'),log);
 console.log(JSON.stringify(report));
}
