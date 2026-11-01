#!/usr/bin/env node
// Independent protocol clients exercise movement normalization and disconnects.
import mc from 'minecraft-protocol';
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {randomUUID} from 'node:crypto';
import {mkdir,writeFile} from 'node:fs/promises';

const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const args=process.argv.slice(2);
const option=(key,fallback)=>args.includes(key)?args[args.indexOf(key)+1]:fallback;
const port=Number(option('--port','25578'));
const binary=resolve(root,option('--server-binary','target/debug/mc121-rust'));
const report={ok:false,scope:'movement packet validation and rotation normalization; complete movement state machine excluded',checks:{}};
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const clients=[];let server,log='';
async function until(predicate,label,timeout=12000){
 const deadline=performance.now()+timeout;
 while(!predicate()){
  const errors=clients.flatMap(c=>c.errors);if(errors.length)throw errors[0];
  if(server.exitCode!==null)throw new Error('server exited: '+log.slice(-3000));
  if(performance.now()>deadline)throw new Error('deadline: '+label);
  await pause(20);
 }
}
async function connect(name){
 const state={joined:false,ready:false,kick:null,entityId:null,heads:[],errors:[],closing:false};
 const client=mc.createClient({host:'127.0.0.1',port,username:name,auth:'offline',version:'1.21',hideErrors:true});
 state.client=client;clients.push(state);
 client.on('error',error=>{if(!state.closing&&!state.kick)state.errors.push(error);});
 client.on('login',data=>{state.joined=true;state.entityId=data.entityId;});
 client.on('position',data=>{
  client.write('teleport_confirm',{teleportId:data.teleportId});
  client.write('position_look',{x:data.x,y:data.y,z:data.z,yaw:data.yaw,pitch:data.pitch,onGround:true});
  state.ready=true;
 });
 client.on('chunk_batch_finished',()=>client.write('chunk_batch_received',{chunksPerTick:16}));
 client.on('entity_head_rotation',data=>state.heads.push(data));
 client.on('kick_disconnect',data=>{state.kick=JSON.stringify(data);});
 await until(()=>state.joined&&state.ready,name+' ready');return state;
}
try{
 server=spawn(binary,['--allow-offline','--port',String(port),'--world','runtime/movement-'+randomUUID()+'.json'],{cwd:root,stdio:['ignore','pipe','pipe']});
 server.stdout.on('data',data=>log+=data);server.stderr.on('data',data=>log+=data);
 await until(()=>log.includes('"event":"listening"'),'startup');
 const observer=await connect('MoveObserver');
 const mover=await connect('MoveRotation');
 // Mth.wrapDegrees(1_000_000_000f)=-80f; ServerEntity floor(-80*256/360)=-57.
 mover.client.write('look',{yaw:1_000_000_000,pitch:0,onGround:true});
 await until(()=>observer.heads.some(p=>p.entityId===mover.entityId&&p.headYaw===-57),'wrapped yaw');
 report.checks.large_finite_yaw_wraps_before_tracking=true;
 mover.client.write('look',{yaw:Infinity,pitch:0,onGround:true});
 await until(()=>mover.kick,'infinite rotation disconnect');
 assert.match(mover.kick,/multiplayer\.disconnect\.invalid_player_movement/);
 report.checks.infinite_rotation_gets_vanilla_disconnect=true;
 const invalidPosition=await connect('MoveNaN');
 invalidPosition.client.write('position',{x:NaN,y:65,z:8.5,onGround:true});
 await until(()=>invalidPosition.kick,'NaN position disconnect');
 assert.match(invalidPosition.kick,/multiplayer\.disconnect\.invalid_player_movement/);
 report.checks.nan_position_gets_vanilla_disconnect=true;
 assert.equal(observer.kick,null);report.checks.other_player_remains_connected=true;
 report.ok=true;
}catch(error){report.error=String(error.stack??error);process.exitCode=1;}
finally{
 for(const state of clients){state.closing=true;state.client.end();}
 if(server?.exitCode===null){server.kill('SIGINT');await Promise.race([new Promise(resolve=>server.once('exit',resolve)),pause(5000)]);if(server.exitCode===null)server.kill('SIGKILL');}
 await mkdir(resolve(root,'evidence'),{recursive:true});
 await writeFile(resolve(root,'evidence/movement-live-conformance.json'),JSON.stringify(report,null,2)+'\n');
 await writeFile(resolve(root,'evidence/movement-live-server.log'),log);
 console.log(JSON.stringify(report));
}
