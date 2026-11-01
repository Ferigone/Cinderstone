#!/usr/bin/env node
// Observe actual weather from an isolated, original-NBT-prepared Anvil world.
import mc from 'minecraft-protocol';import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {createHash} from 'node:crypto';import {resolve,dirname} from 'node:path';import {fileURLToPath} from 'node:url';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..'),args=process.argv.slice(2);
const opt=(key,value)=>args.includes(key)?args[args.indexOf(key)+1]:value;
const binary=resolve(root,opt('--server-binary','runtime/phase5-world-context/mc121-rust'));
const world=opt('--anvil-world',null);assert.ok(world);const port=Number(opt('--port','25607'));
let server,client,log='',closing=false,position=false;const errors=[],timeline=[];
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
async function until(test,label){const end=performance.now()+30000;while(!test()){
 if(errors.length)throw errors[0];if(server?.exitCode!==null&&server?.exitCode!==undefined)throw Error('server exited '+log.slice(-3000));
 if(performance.now()>end)throw Error('deadline '+label);await pause(20);
}}
const report={ok:false,minecraft:'1.21',protocol:767,scope:'actual Anvil dimension/weather/rule integration and initial level-info ordering',checks:{}};
try {
 server=spawn(binary,['--allow-offline','--port',String(port),'--anvil-world',world],{cwd:root,stdio:['ignore','pipe','pipe']});
 server.stdout.on('data',data=>log+=data);server.stderr.on('data',data=>log+=data);
 await until(()=>log.includes('"event":"listening"'),'listening');
 client=mc.createClient({host:'127.0.0.1',port,username:'WorldCtxProbe',auth:'offline',version:'1.21',keepAlive:false,hideErrors:true});
 client.on('error',error=>{if(!closing)errors.push(error);});client.on('end',reason=>{if(!closing)errors.push(Error('closed '+reason));});
 client.on('kick_disconnect',data=>errors.push(Error(JSON.stringify(data))));client.on('keep_alive',data=>client.write('keep_alive',data));
 client.on('chunk_batch_finished',()=>client.write('chunk_batch_received',{chunksPerTick:16}));
 client.on('position',data=>{client.write('teleport_confirm',{teleportId:data.teleportId});position=true;});
 client.on('packet',(data,meta)=>{if(['update_time','spawn_position','game_state_change'].includes(meta.name)){
 if(meta.name==='game_state_change')data={...data,reason:({start_raining:1,stop_raining:2,rain_level_change:7,thunder_level_change:8,level_chunks_load_start:13})[data.reason]??data.reason};
 timeline.push({name:meta.name,data});}});
 await until(()=>timeline.some(p=>p.name==='game_state_change'&&p.data.reason===8),'initial weather');
 // This captured Anvil world includes unfinished surrounding generation. Its
 // initial level-info precedes transport; full chunk/login parity is separate.
 closing=true;client.end();report.completed_initial_teleport=position;
 const reasons=timeline.filter(p=>p.name==='game_state_change').map(p=>p.data.reason);
 assert.deepEqual(reasons.slice(0,4),[1,7,8,13]);
 const rain=timeline.find(p=>p.name==='game_state_change'&&p.data.reason===7).data.gameMode;
 const thunder=timeline.find(p=>p.name==='game_state_change'&&p.data.reason===8).data.gameMode;
 assert.equal(rain,1);assert.equal(thunder,1);
 report.checks.actual_persisted_rain_and_thunder_initial_levels=true;
 const time=timeline.findIndex(p=>p.name==='update_time'),spawnIndex=timeline.findIndex(p=>p.name==='spawn_position'),start=timeline.findIndex(p=>p.name==='game_state_change'&&p.data.reason===1);
 assert.ok(time>=0&&spawnIndex>time&&start>spawnIndex);report.checks.original_time_spawn_weather_chunk_start_order=true;
 await pause(250);assert.ok(server.exitCode===null);report.checks.do_weather_cycle_false_keeps_supported_tick_running=true;
 report.ok=true;report.timeline=timeline;report.server_binary_sha256=createHash('sha256').update(await readFile(binary)).digest('hex');
}catch(error){report.error=String(error.stack??error);process.exitCode=1;}
finally {
 report.timeline=timeline;closing=true;client?.end();if(server&&server.exitCode===null){server.kill('SIGINT');await Promise.race([new Promise(resolve=>server.once('exit',resolve)),pause(5000)]);if(server.exitCode===null)server.kill('SIGKILL');}
 await mkdir(resolve(root,'runtime/phase5-world-context'),{recursive:true});await writeFile(resolve(root,'runtime/phase5-world-context/live.json'),JSON.stringify(report,null,2)+'\n');await writeFile(resolve(root,'runtime/phase5-world-context/server.log'),log);console.log(JSON.stringify(report));
}

process.exit(process.exitCode??0);
