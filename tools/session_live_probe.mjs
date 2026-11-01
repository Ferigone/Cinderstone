#!/usr/bin/env node
// Real protocol clients verify runtime keepalive cadence, timeout and latency publication.
import mc from 'minecraft-protocol';import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';import {mkdir,writeFile} from 'node:fs/promises';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..'),args=process.argv.slice(2);
const option=(name,fallback)=>args.includes(name)?args[args.indexOf(name)+1]:fallback;
const port=Number(option('--port','25576')),binary=option('--server-binary','target/debug/mc121-rust');
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));const clients=[];let server,serverLog='';
const report={ok:false,scope:'Minecraft1.21 runtime keepalive and latency',checks:{}};
async function waitUntil(predicate,label,timeout=40000){const deadline=performance.now()+timeout;while(!predicate()){
 const error=clients.flatMap(state=>state.errors)[0];if(error)throw error;
 if(server&&server.exitCode!==null)throw new Error('server exited: '+serverLog.slice(-2500));
 if(performance.now()>deadline)throw new Error('deadline waiting for '+label);await pause(20);
}}
async function connect(username,mode){
 const state={username,mode,joined:null,challenges:[],latency:[],kicked:null,ended:null,errors:[],closing:false};
 const client=mc.createClient({host:'127.0.0.1',port,username,auth:'offline',version:'1.21',keepAlive:false,hideErrors:true});state.client=client;clients.push(state);
 client.on('error',error=>{if(!state.closing&&mode==='healthy')state.errors.push(error);});
 client.on('end',reason=>{state.ended={at:performance.now(),reason:String(reason)};if(!state.closing&&mode==='healthy')state.errors.push(new Error('healthy client ended: '+String(reason)));});
 client.on('kick_disconnect',data=>{state.kicked={at:performance.now(),reason:JSON.stringify(data)};});
 client.on('login',()=>{state.joined=performance.now();});
 client.on('position',data=>{client.write('teleport_confirm',{teleportId:data.teleportId});client.write('position_look',{x:data.x,y:data.y,z:data.z,yaw:data.yaw,pitch:data.pitch,onGround:true});});
 client.on('chunk_batch_finished',()=>client.write('chunk_batch_received',{chunksPerTick:16}));
 client.on('keep_alive',data=>{
  state.challenges.push({at:performance.now(),id:String(data.keepAliveId)});
  if(mode==='healthy')setTimeout(()=>{if(!state.closing)client.write('keep_alive',data);},40);
  if(mode==='wrong')client.write('keep_alive',{keepAliveId:0n});
 });
 client.on('player_info',data=>{
  if(!data.action.update_latency||data.action.add_player)return;
  const self=data.data.find(entry=>entry.uuid===client.uuid);
  if(self)state.latency.push({at:performance.now(),value:self.latency});
 });
 await waitUntil(()=>state.joined!==null,username+' PLAY',15000);return state;
}
try{
 server=spawn(resolve(root,binary),['--allow-offline','--port',String(port),'--world','runtime/session-conformance-world.json'],{cwd:root,stdio:['ignore','pipe','pipe']});
 server.stdout.on('data',data=>serverLog+=data);server.stderr.on('data',data=>serverLog+=data);
 await waitUntil(()=>serverLog.includes('"event":"listening"'),'startup',15000);
 const healthy=await connect('SessionHealthy','healthy'),wrong=await connect('SessionWrong','wrong'),silent=await connect('SessionSilent','silent');
 await waitUntil(()=>wrong.kicked&&wrong.ended&&silent.kicked&&silent.ended&&healthy.latency.length&&healthy.challenges.length>=2,'keepalive responses, timeout and latency publication');
 for(const state of [healthy,wrong,silent]){
  const first=state.challenges[0].at-state.joined;assert.ok(first>=14500&&first<=17000,'first keepalive after15s: '+state.username+' '+first);
 }
 report.checks.first_keepalive_after_15_seconds=true;
 const interval=healthy.challenges[1].at-healthy.challenges[0].at;assert.ok(interval>=14500&&interval<=16500,'healthy challenge cadence '+interval);
 assert.ok(!healthy.ended);report.checks.valid_responses_keep_connection_alive=true;
 assert.match(wrong.kicked.reason,/disconnect\.timeout/);assert.ok(wrong.ended.at-wrong.challenges[0].at<2000);report.checks.wrong_id_gets_timeout_disconnect=true;
 assert.match(silent.kicked.reason,/disconnect\.timeout/);const deadline=silent.ended.at-silent.challenges[0].at;assert.ok(deadline>=14500&&deadline<=17000,'pending timeout interval '+deadline);report.checks.missing_response_times_out_after_15_seconds=true;
 const update=healthy.latency[0];assert.ok(update.at-healthy.joined>=29000&&update.at-healthy.joined<=33000);assert.ok(update.value>=8&&update.value<=100,'smoothed delayed RTT '+update.value);report.checks.player_list_publishes_smoothed_latency=true;
 report.observed={first_challenge_ms:clients.map(state=>({username:state.username,milliseconds:Math.round(state.challenges[0].at-state.joined)})),healthy_interval_ms:Math.round(interval),pending_timeout_ms:Math.round(deadline),published_latency_ms:update.value};report.ok=true;
}catch(error){report.error=String(error.stack??error);process.exitCode=1;}
finally{
 for(const state of clients){state.closing=true;state.client.end();}
 if(server&&server.exitCode===null){server.kill('SIGINT');await Promise.race([new Promise(resolve=>server.once('exit',resolve)),pause(5000)]);if(server.exitCode===null)server.kill('SIGKILL');}
 await mkdir(resolve(root,'evidence'),{recursive:true});await writeFile(resolve(root,'evidence/session-live-conformance.json'),JSON.stringify(report,null,2)+'\n');await writeFile(resolve(root,'evidence/session-live-server.log'),serverLog);console.log(JSON.stringify(report));
}
