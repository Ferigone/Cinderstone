#!/usr/bin/env node
// An independent unmodified protocol client drives real window0 packets.
import mc from 'minecraft-protocol';import data from 'minecraft-data';
import assert from 'node:assert/strict';import {spawn} from 'node:child_process';
import {resolve,dirname} from 'node:path';import {fileURLToPath} from 'node:url';import {mkdir,writeFile} from 'node:fs/promises';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..'),args=process.argv.slice(2);
const option=(n,f)=>{const i=args.indexOf(n);return i<0?f:args[i+1];};
const port=Number(option('--port','25573')),binary=option('--server-binary',null),md=data('1.21');
let server,client,serverLog='',closing=false,position=false,stateId=0,items=Array(46).fill(null),carried=null,fullSyncs=0;const errors=[],packets=[];
const report={ok:false,minecraft:'1.21',protocol:767,scope:'native window0 packet clicks, predictions and2x2crafting',checks:{}};
const pause=ms=>new Promise(r=>setTimeout(r,ms));
async function until(predicate,label,timeout=15000){const deadline=Date.now()+timeout;while(!predicate()){if(errors.length)throw errors[0];if(server&&server.exitCode!==null)throw Error('Serverexited '+serverLog.slice(-3000));if(Date.now()>deadline)throw Error('Deadline '+label);await pause(10);}}
const count=s=>s?.itemCount??0;const empty=s=>count(s)<=0;
const stack=(name,n)=>({itemCount:n,itemId:md.itemsByName[name].id,addedComponentCount:0,removedComponentCount:0,components:[],removeComponents:[]});
const is=(s,name,n)=>count(s)===n&&(n===0||s.itemId===md.itemsByName[name].id);
async function creative(slot,name,n){client.write('set_creative_slot',{slot,item:stack(name,n)});await until(()=>is(items[slot],name,n),'creative echo '+slot);}
async function click(slot,button,mode,predicate,label,extra={}){const packet={windowId:0,stateId,slot,mouseButton:button,mode,changedSlots:[],cursorItem:{itemCount:0},...extra};carried=packet.cursorItem;for(const change of packet.changedSlots)items[change.location]=change.item;client.write('window_click',packet);await until(predicate,label);}
try {
 if(binary){server=spawn(resolve(root,binary),['--allow-offline','--port',String(port),'--world','runtime/container-conformance-world.json'],{cwd:root,stdio:['ignore','pipe','pipe']});server.stdout.on('data',b=>serverLog+=b);server.stderr.on('data',b=>serverLog+=b);await until(()=>serverLog.includes('"event":"listening"'),'startup');}
 client=mc.createClient({host:'127.0.0.1',port,username:'ContainerProbe',auth:'offline',version:'1.21',keepAlive:false,hideErrors:true});
 client.on('error',e=>{if(!closing)errors.push(e);});client.on('end',reason=>{if(!closing)errors.push(Error('Disconnected '+String(reason)));});client.on('kick_disconnect',v=>errors.push(Error(JSON.stringify(v))));client.on('keep_alive',p=>client.write('keep_alive',p));client.on('chunk_batch_finished',()=>client.write('chunk_batch_received',{chunksPerTick:16}));
 client.on('position',p=>{position=true;client.write('teleport_confirm',{teleportId:p.teleportId});client.write('position_look',{x:p.x,y:p.y,z:p.z,yaw:p.yaw,pitch:p.pitch,onGround:true});});
 client.on('window_items',p=>{if(p.windowId===0){items=p.items;carried=p.carriedItem;stateId=p.stateId;fullSyncs++;packets.push({name:'window_items',stateId});}});
 client.on('set_slot',p=>{stateId=p.stateId;if(p.windowId===0&&p.slot>=0){items[p.slot]=p.item;}else if(p.slot===-1){carried=p.item;}packets.push({name:'set_slot',stateId,slot:p.slot});});
 await until(()=>position&&fullSyncs>0,'initial inventory');assert.equal(items.length,46);report.checks.initial_inventory46=true;
 await creative(9,'stone',10);
 await click(9,0,0,()=>empty(items[9])&&is(carried,'stone',10),'left pickup');
 await click(10,1,0,()=>is(items[10],'stone',1)&&is(carried,'stone',9),'right deposit one');
 await click(10,0,0,()=>is(items[10],'stone',10)&&empty(carried),'merge cursor');report.checks.pickup_split_merge=true;
 const before=fullSyncs;
 await click(-1,0,0,()=>fullSyncs>before,'stale full resynchronization',{stateId:(stateId+32767)&32767,changedSlots:[{location:10,item:stack('diamond',64)}],cursorItem:stack('diamond',64)});
 assert.ok(is(items[10],'stone',10));assert.ok(empty(carried));report.checks.stale_state_and_false_prediction_corrected=true;
 await creative(36,'oak_planks',6);
 await click(36,0,0,()=>empty(items[36])&&is(carried,'oak_planks',6),'craft pickup planks');
 await click(1,1,0,()=>is(items[1],'oak_planks',1)&&is(carried,'oak_planks',5),'first craft ingredient');
 await click(3,1,0,()=>is(items[3],'oak_planks',1)&&is(carried,'oak_planks',4)&&is(items[0],'stick',4),'second craft ingredient and result');
 await click(36,0,0,()=>is(items[36],'oak_planks',4)&&empty(carried),'return unused planks');
 await click(0,0,0,()=>is(carried,'stick',4)&&empty(items[0])&&empty(items[1])&&empty(items[3]),'take crafted sticks and consume ingredients');
 await click(37,0,0,()=>is(items[37],'stick',4)&&empty(carried),'store crafted sticks');report.checks.manual2x2craft_result_and_consumption=true;
 await click(36,0,0,()=>empty(items[36])&&is(carried,'oak_planks',4),'pickup remaining planks');
 await click(1,1,0,()=>is(items[1],'oak_planks',1)&&is(carried,'oak_planks',3),'craft grid first stack1');
 await click(1,1,0,()=>is(items[1],'oak_planks',2)&&is(carried,'oak_planks',2),'craft grid first stack2');
 await click(3,1,0,()=>is(items[3],'oak_planks',1)&&is(carried,'oak_planks',1),'craft grid second stack1');
 await click(3,1,0,()=>is(items[3],'oak_planks',2)&&empty(carried)&&is(items[0],'stick',4),'craft grid second stack2');
 await click(0,0,1,()=>is(items[37],'stick',12)&&empty(items[0])&&empty(items[1])&&empty(items[3]),'shift craft repeats and merges');report.checks.shift_craft_repeats_and_merges=true;
 assert.ok(empty(carried));
 await creative(1,'oak_planks',2);await until(()=>is(items[0],'oak_button',1),'creative grid recalculates button output');
 await creative(3,'oak_planks',2);await until(()=>is(items[0],'stick',4),'creative grid recalculates stick output');report.checks.creative_grid_recalculates_immediately=true;
 await creative(36,'stone',5);await click(36,0,0,()=>empty(items[36])&&is(carried,'stone',5),'pickup before closing inventory');
 client.write('close_window',{windowId:0});
 const planks=()=>items.filter(s=>!empty(s)&&s.itemId===md.itemsByName.oak_planks.id).reduce((n,s)=>n+count(s),0);
 await until(()=>empty(carried)&&empty(items[0])&&empty(items[1])&&empty(items[3])&&is(items[10],'stone',15)&&planks()===4,'close returns cursor and uncrafted inputs');report.checks.close_returns_cursor_and_crafting_inputs=true;
 report.final={stone10:count(items[10]),sticks37:count(items[37]),returnedUncraftedPlanks:planks(),cursorMergedStone10:count(items[10]),stateId,serverInventoryPackets:packets.length};report.ok=true;
}catch(e){report.error=String(e.stack??e);process.exitCode=1;}
finally {closing=true;client?.end();if(server&&server.exitCode===null){server.kill('SIGINT');await Promise.race([new Promise(r=>server.once('exit',r)),pause(5000)]);if(server.exitCode===null)server.kill('SIGKILL');}
 await mkdir(resolve(root,'evidence'),{recursive:true});await writeFile(resolve(root,'evidence/container-live-conformance.json'),JSON.stringify(report,null,2)+'\n');if(server)await writeFile(resolve(root,'evidence/container-live-server.log'),serverLog);console.log(JSON.stringify(report));}
