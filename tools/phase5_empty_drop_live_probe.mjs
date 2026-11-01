#!/usr/bin/env node
// Explicit EMPTY milestone safety proof, not successful live item-runtime parity.
import mc from 'minecraft-protocol';
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {createHash,randomUUID} from 'node:crypto';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..'),args=process.argv.slice(2);
const opt=(name,fallback)=>args.includes(name)?args[args.indexOf(name)+1]:fallback;
const binary=resolve(root,opt('--server-binary','runtime/phase5-empty/mc121-rust')),port=Number(opt('--port','25606'));
const unsupportedHistory=args.includes('--unsupported-history');
const report={ok:false,scope:unsupportedHistory?'uncertified saved-world drop refusal without inventory loss or fabricated entities':'phase5 empty-runtime drop refusal without inventory loss or fabricated entities',active_item_runtime:false,checks:{}};
const reportPath=opt('--report','evidence/phase5-empty/drop-safety.json');
let server,client,log='',closing=false,ready=false,full=0,stateId=0,items=[],carried=null;
const errors=[],spawns=[];const pause=ms=>new Promise(r=>setTimeout(r,ms));
const itemCount=item=>item?.itemCount??0;
const stone=n=>({itemCount:n,itemId:1,addedComponentCount:0,removedComponentCount:0,components:[],removeComponents:[]});
async function until(predicate,label){const end=performance.now()+15000;while(!predicate()){
 if(errors.length)throw errors[0];if(server&&server.exitCode!==null)throw Error('server exited '+log.slice(-2000));
 if(performance.now()>end)throw Error('deadline '+label);await pause(10);
}}
async function refused(action,label){const before=full;action();await until(()=>full>before,label);assert.equal(itemCount(items[36]),5);assert.equal(spawns.filter(p=>p.type===58).length,0);}
try {
 const worldPath=`runtime/phase5-empty/drop-${randomUUID()}.json`;
 await mkdir(resolve(root,'runtime/phase5-empty'),{recursive:true});
 if(unsupportedHistory)await writeFile(resolve(root,worldPath),JSON.stringify({format:'mc121-flat-diffs-v1',minecraft:'1.21',changes:[]}));
 server=spawn(binary,['--allow-offline','--port',String(port),'--world',worldPath],{cwd:root,stdio:['ignore','pipe','pipe']});
 server.stdout.on('data',data=>log+=data);server.stderr.on('data',data=>log+=data);
 await until(()=>log.includes('"event":"listening"'),'listening');
 client=mc.createClient({host:'127.0.0.1',port,username:'EmptyDropProbe',auth:'offline',version:'1.21',keepAlive:false,hideErrors:true});
 client.on('error',error=>{if(!closing)errors.push(error);});client.on('end',reason=>{if(!closing)errors.push(Error('closed '+reason));});
 client.on('kick_disconnect',data=>errors.push(Error(JSON.stringify(data))));client.on('keep_alive',data=>client.write('keep_alive',data));
 client.on('chunk_batch_finished',()=>client.write('chunk_batch_received',{chunksPerTick:16}));
 client.on('position',data=>{client.write('teleport_confirm',{teleportId:data.teleportId});client.write('position_look',{x:data.x,y:data.y,z:data.z,yaw:data.yaw,pitch:data.pitch,onGround:true});ready=true;});
 client.on('spawn_entity',data=>spawns.push(data));
 client.on('window_items',data=>{if(data.windowId===0){items=data.items;carried=data.carriedItem;stateId=data.stateId;full++;}});
 client.on('set_slot',data=>{stateId=data.stateId;if(data.windowId===0&&data.slot>=0)items[data.slot]=data.item;else if(data.slot===-1)carried=data.item;});
 await until(()=>ready&&items.length===46,'initial inventory');
 client.write('held_item_slot',{slotId:0});client.write('set_creative_slot',{slot:36,item:stone(5)});
 await until(()=>itemCount(items[36])===5,'initial selected stack');
 for(const [status,name]of [[4,'Q'],[3,'CtrlQ']]){
  await refused(()=>client.write('block_dig',{status,location:{x:0,y:0,z:0},face:0,sequence:0}),name+' correction');
  report.checks[name+'_refuses_before_consumption']=true;
 }
 await refused(()=>client.write('window_click',{windowId:0,stateId,slot:36,mouseButton:0,mode:4,changedSlots:[],cursorItem:{itemCount:0}}),'throw correction');
 report.checks.menu_throw_rolls_back_and_corrects_prediction=true;
 await refused(()=>client.write('set_creative_slot',{slot:-1,item:stone(32)}),'creative negative correction');
 report.checks.negative_creative_has_no_fabricated_entity=true;
 for(let slot=9;slot<=45;slot++)client.write('set_creative_slot',{slot,item:stone(64)});
 await until(()=>Array.from({length:37},(_,i)=>9+i).every(slot=>itemCount(items[slot])===64),'full inventory');
 client.write('window_click',{windowId:0,stateId,slot:36,mouseButton:2,mode:3,changedSlots:[],cursorItem:{itemCount:0}});
 await until(()=>itemCount(carried)===64,'creative clone cursor');
 const beforeClose=full;client.write('close_window',{windowId:0});await until(()=>full>beforeClose,'close correction');
 assert.equal(itemCount(carried),64);assert.ok(Array.from({length:37},(_,i)=>9+i).every(slot=>itemCount(items[slot])===64));
 assert.equal(spawns.filter(p=>p.type===58).length,0);
 report.checks.close_overflow_preserves_cursor_until_world_ready=true;
 report.ok=true;report.server_binary_sha256=createHash('sha256').update(await readFile(binary)).digest('hex');
}catch(error){report.error=String(error.stack??error);process.exitCode=1;}
finally {
 closing=true;client?.end();if(server&&server.exitCode===null){server.kill('SIGINT');await Promise.race([new Promise(resolve=>server.once('exit',resolve)),pause(5000)]);if(server.exitCode===null)server.kill('SIGKILL');}
 await mkdir(resolve(root,'evidence/phase5-empty'),{recursive:true});await mkdir(dirname(resolve(root,reportPath)),{recursive:true});await writeFile(resolve(root,reportPath),JSON.stringify(report,null,2)+'\n');await writeFile(resolve(root,'runtime/phase5-empty/drop-safety-server.log'),log);console.log(JSON.stringify(report));
}
