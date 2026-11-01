#!/usr/bin/env node
// Real protocol767 transport: initial quota, held ACK, ordered resumed batches.
import mc from 'minecraft-protocol';import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';import {mkdir,readFile,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';import {resolve,dirname} from 'node:path';import {fileURLToPath} from 'node:url';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..'),args=process.argv.slice(2);
const opt=(key,value)=>args.includes(key)?args[args.indexOf(key)+1]:value;
const binary=resolve(root,opt('--server-binary','runtime/phase5-chunk-transport/mc121-rust'));
const port=Number(opt('--port','25608'));let server,client,log='',closing=false,position=false,current=null;
const errors=[],batches=[],loaded=new Set(),centers=[],unloads=[];let autoAck=false;
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
async function until(test,label){const end=performance.now()+20000;while(!test()){
 if(errors.length)throw errors[0];if(server?.exitCode!==null&&server?.exitCode!==undefined)throw Error('server exited '+log.slice(-3000));
 if(performance.now()>end)throw Error('deadline '+label);await pause(15);
}}
const report={ok:false,minecraft:'1.21',protocol:767,full_runtime_parity:false,scope:'native TCP PlayerChunkSender transport glue',checks:{}};
try {
 server=spawn(binary,['--allow-offline','--port',String(port),'--world','runtime/chunk-transport-live-world.json'],{cwd:root,stdio:['ignore','pipe','pipe']});
 server.stdout.on('data',data=>log+=data);server.stderr.on('data',data=>log+=data);
 await until(()=>log.includes('"event":"listening"'),'listening');
 client=mc.createClient({host:'127.0.0.1',port,username:'ChunkQuotaProbe',auth:'offline',version:'1.21',keepAlive:false,hideErrors:true,clientSettings:{locale:'en_us',viewDistance:2,chatFlags:0,chatColors:true,skinParts:127,mainHand:1,enableTextFiltering:false,enableServerListing:true}});
 client.on('error',error=>{if(!closing)errors.push(error);});client.on('end',reason=>{if(!closing)errors.push(Error('closed '+reason));});
 client.on('kick_disconnect',data=>errors.push(Error(JSON.stringify(data))));client.on('keep_alive',data=>client.write('keep_alive',data));
 client.on('position',data=>{client.write('teleport_confirm',{teleportId:data.teleportId});position=true;});
 client.on('chunk_batch_start',()=>{assert.equal(current,null);current=[];});
 client.on('map_chunk',data=>{assert.ok(current);current.push([data.x,data.z]);loaded.add(`${data.x},${data.z}`);});
 client.on('chunk_batch_finished',data=>{assert.ok(current);assert.equal(current.length,data.batchSize);batches.push(current);current=null;if(autoAck)client.write('chunk_batch_received',{chunksPerTick:16});});
 client.on('update_view_position',data=>centers.push([data.chunkX,data.chunkZ]));
 client.on('unload_chunk',data=>{unloads.push([data.chunkX,data.chunkZ]);loaded.delete(`${data.chunkX},${data.chunkZ}`);});
 await until(()=>position&&batches.length===1,'initial9 and teleport');
 assert.equal(batches[0].length,9);assert.equal(loaded.size,9);report.checks.first_original_quota_nine=true;
 await pause(300);assert.equal(batches.length,1);report.checks.no_more_batches_before_client_ack=true;
 client.write('chunk_batch_received',{chunksPerTick:16});
 await until(()=>loaded.size===45,'remaining45 chunks after ACK');
 assert.deepEqual(batches.map(batch=>batch.length),[9,16,16,4]);report.checks.ack_0x08_resumes_sixteen_sixteen_four=true;
 const goldenPath=opt('--golden',null);if(goldenPath){const golden=JSON.parse(await readFile(resolve(root,goldenPath),'utf8'));assert.deepEqual(batches,golden.batches);report.checks.actual_original_arq_entire_batch_order=true;}
 report.initial_batches=batches.map(batch=>batch.map(p=>[...p]));
 autoAck=true;client.write('position',{x:-151.5,y:65,z:168.5,onGround:true});
 await until(()=>centers.some(p=>p[0]===-10&&p[1]===10)&&loaded.size===45&&unloads.length===45,'new view45 and unload old');
 assert.ok([...loaded].every(key=>{const[x,z]=key.split(',').map(Number);return Math.abs(x+10)<=3&&Math.abs(z-10)<=3;}));report.checks.view_move_unloads_old_and_streams_new=true;
 report.ok=true;report.server_binary_sha256=createHash('sha256').update(await readFile(binary)).digest('hex');
}catch(error){report.error=String(error.stack??error);process.exitCode=1;}
finally {
 report.final_transport={centers,unloads:unloads.length,loaded:loaded.size,batch_sizes:batches.map(b=>b.length)};closing=true;client?.end();if(server&&server.exitCode===null){server.kill('SIGINT');await Promise.race([new Promise(resolve=>server.once('exit',resolve)),pause(5000)]);if(server.exitCode===null)server.kill('SIGKILL');}
 await mkdir(resolve(root,'runtime/phase5-chunk-transport'),{recursive:true});await writeFile(resolve(root,'runtime/phase5-chunk-transport/live.json'),JSON.stringify(report,null,2)+'\n');await writeFile(resolve(root,'runtime/phase5-chunk-transport/server.log'),log);console.log(JSON.stringify(report));
}
process.exit(process.exitCode??0);
