// SPDX-License-Identifier: GPL-2.0-only
// Actual target-only title display packets, typed feedback and source/operator policy.
import mc from 'minecraft-protocol';
import nbt from 'prismarine-nbt';
import assert from 'node:assert/strict';
import {spawn,execFileSync} from 'node:child_process';
import {mkdtemp,readFile,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import path from 'node:path';
import {createHash} from 'node:crypto';
const [binaryArg,output]=process.argv.slice(2);assert.ok(output,'BINARY REPORT');const binary=path.resolve(binaryArg),root=process.cwd(),folder=await mkdtemp(path.join(tmpdir(),'title-'));
function uuid(name){const b=createHash("md5").update("OfflinePlayer:"+name).digest();b[6]=(b[6]&15)|48;b[8]=(b[8]&63)|128;const s=b.toString("hex");return `${s.slice(0,8)}-${s.slice(8,12)}-${s.slice(12,16)}-${s.slice(16,20)}-${s.slice(20)}`;}
const original=JSON.parse(await readFile('fixtures/title-vectors.json','utf8'));await writeFile(path.join(folder,'keys.json'),'{"profilePropertyKeys":[],"playerCertificateKeys":[]}');await writeFile(path.join(folder,'ops.json'),JSON.stringify([{uuid:uuid('TitleSource'),name:'TitleSource',level:2,bypassesPlayerLimit:false}]));
const report={ok:false,native_commit:execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim(),native_src_tree:execFileSync('git',['rev-parse','HEAD:src'],{encoding:'utf8'}).trim(),native_binary_sha256:createHash('sha256').update(await readFile(binary)).digest('hex'),java_jar_sha1:original.java_jar_sha1,execution:[],limits:['Original TitleCommand and ComponentArgument/ComponentUtils execute with actual typed feedback and recording connections; no full Java server/client renderer.','One online name and supported static JSON only. Original completion comparison projects out unsupported selectors. Animation state belongs to the client.']};
const pause=ms=>new Promise(r=>setTimeout(r,ms));const entries=[];let server,log='',transaction=0,marker=0;
async function until(test,label,timeout=30000){const end=Date.now()+timeout;while(!test()){if(server?.exitCode!==null&&server?.exitCode!==undefined)throw Error('server exited: '+label+' '+log.slice(-1200));if(Date.now()>end)throw Error('timeout: '+label);await pause(20);}}
async function start(){log="";server=spawn(binary,['--allow-offline','--bind','127.0.0.1','--port','25711','--fixtures',path.join(root,'fixtures/configuration.json'),'--world',path.join(folder,'world.json'),'--server-directory',folder,'--view-distance','2','--loopback-service-keys',path.join(folder,'keys.json')],{cwd:folder,stdio:['pipe','pipe','pipe']});server.stdout.on('data',b=>log+=b);server.stderr.on('data',b=>log+=b);await until(()=>log.includes('"event":"listening"'),'startup',120000);}
async function stop(){if(!server)return;for(const e of entries.splice(0))e.client.end();const s=server;s.kill('SIGTERM');await until(()=>s.exitCode!==null,'shutdown');assert.equal(s.exitCode,0,log.slice(-2000));await writeFile(path.join(folder,'server.log'),log);server=null;}
const red=c=>c&&typeof c==='object'&&(c.color==='red'||Object.values(c).some(red));
const has=(c,key)=>c&&typeof c==='object'&&(c.translate===key||Object.values(c).some(v=>has(v,key)));
function tree(p){const out=[];function walk(i,path){const n=p.nodes[i],e=n.extraNodeData,parts=[...path,e.name];out.push({path:parts.join(' '),executable:!!n.flags.has_command,parser:e.parser??null,properties:e.properties??null,suggestion:e.suggestionType??null,redirect:n.flags.has_redirect_node?p.nodes[n.redirectNode].extraNodeData.name:null});n.children.forEach(c=>walk(c,parts));}p.nodes[p.rootIndex].children.filter(i=>['title'].includes(p.nodes[i].extraNodeData?.name)).forEach(i=>walk(i,[]));return out;}
async function connect(name){
 const client=mc.createClient({host:'127.0.0.1',port:25711,username:name,auth:'offline',version:'1.21',keepAlive:false,hideErrors:true});let ready=false,error;const e={name,client,display:[],order:[],system:[],teams:[],completions:[],tree:null};entries.push(e);
 client.on('login',p=>e.id=p.entityId);client.on('error',v=>error=v);client.on('end',()=>{error=Error(name+' ended');});client.on('keep_alive',p=>client.write('keep_alive',p));client.on('chunk_batch_finished',()=>client.write('chunk_batch_received',{chunksPerTick:16}));client.on('position',p=>{client.write('teleport_confirm',{teleportId:p.teleportId});ready=true;});client.on('declare_commands',p=>e.tree=tree(p));client.on('teams',p=>e.teams.push(p));client.on('tab_complete',p=>e.completions.push(p));client.on('packet',(p,m,b)=>{if(['clear_titles','action_bar','set_title_subtitle','set_title_text','set_title_time'].includes(m.name)){e.display.push({id:b[0],hex:b.subarray(1).toString('hex')});e.order.push('display');}if(m.name==='system_chat'){e.system.push({component:nbt.simplify(p.content),hex:b.subarray(1).toString('hex')});e.order.push('feedback');}});
 e.wait=async(test,label)=>until(()=>{if(error)throw error;return test();},label);await e.wait(()=>ready&&e.tree,'admitted '+name);
 e.complete=async text=>{const id=++transaction;client.write('tab_complete',{transactionId:id,text});await e.wait(()=>e.completions.some(p=>p.transactionId===id),'completion '+text);const p=e.completions.find(p=>p.transactionId===id);return {start:p.start,end:p.start+p.length,texts:p.matches.map(v=>v.match)};};
 e.setting=async flags=>{client.write('settings',{locale:'en_US',viewDistance:2,chatFlags:flags,chatColors:true,skinParts:127,mainHand:1,enableTextFiltering:false,enableServerListing:true});await e.complete('title ');};
 return e;
}
function consoleLine(line){server.stdin.write(line+'\n');}
async function barrier(){const name='raw_sync_'+(++marker);consoleLine('team add '+name);await Promise.all(entries.map(e=>e.wait(()=>e.teams.some(p=>p.team===name),'ordered private audience barrier')));}
async function consoleBoard(line){const n=log.length;consoleLine(line);let event;await until(()=>{event=log.slice(n).split('\n').flatMap(v=>{try{return[JSON.parse(v)];}catch{return[];}}).find(v=>v.event==='scoreboard_command'&&v.command===line);return !!event;},line);assert.equal(event.ok,true,JSON.stringify(event));}
function events(offset){return log.slice(offset).split('\n').flatMap(v=>{try{return[JSON.parse(v)];}catch{return[];}});}
async function execute(source,target,outside,row){
 await target.setting(row.visibility);await barrier();const before=entries.map(e=>({display:e.display.length,system:e.system.length,order:e.order.length})),offset=log.length,actor=row.permission===0?outside:source;await pause(1050);const actorBefore=before[entries.indexOf(actor)].system;
 if(row.console)consoleLine(row.input);else actor.client.write('chat_command',{command:row.input});let actual;
 if(row.ok){await until(()=>events(offset).some(e=>e.event==='title_command'),'title effect');actual=events(offset).find(e=>e.event==='title_command');assert.equal(actual.result,row.result);}
 else await actor.wait(()=>actor.system.slice(actorBefore).some(m=>red(m.component)),'private failure '+row.input);
 await barrier();const delivered={},feedback={};for(let i=0;i<entries.length;i++){
  const e=entries[i],display=e.display.slice(before[i].display),messages=e.system.slice(before[i].system);
  assert.deepEqual(display,row.ok?row.packets[e.name]:[],e.name+' display audience');delivered[e.name]=display;
  if(row.console&&e===source)assert.deepEqual(messages.map(m=>m.hex),row.admin);
  else if(!row.console&&e===actor){assert.equal(messages.length,1);if(row.ok){assert.equal(messages[0].hex,row.feedback[0]);assert.ok(has(messages[0].component,row.feedback_key));}else{assert.ok(red(messages[0].component));if(row.error_key==='argument.entity.notfound.player')assert.equal(messages[0].hex,row.feedback[0]);}}
  else assert.deepEqual(messages,[],e.name+' feedback audience');feedback[e.name]=messages.map(m=>m.hex);
 }
 assert.equal(events(offset).filter(e=>e.event==='title_command').length,row.ok?1:0);
 if(row.ok&&row.input.includes('TitleSource '))assert.deepEqual(source.order.slice(before[0].order),['display','feedback'],'self display precedes private success');
 let consoleFeedback;if(row.console){consoleFeedback=events(offset).find(e=>e.event==='administration_feedback'&&has(e.component,row.feedback_key));assert.ok(consoleFeedback);assert.equal(consoleFeedback.result,1);}
 report.execution.push({input:row.input.length>1024?row.input.slice(0,80)+'…':row.input,input_utf16_length:row.input.length,ok:row.ok,console:row.console,visibility:row.visibility,delivered,feedback,...(actual?{actual_result:actual.result,actual_target:actual.target,kind:actual.kind}:{}),...(consoleFeedback?{console_feedback:consoleFeedback}:{})});
}
try{
 await start();const source=await connect('TitleSource'),target=await connect('TitleTarget'),outside=await connect('TitleOutside');
 const decoder=mc.createDeserializer({state:mc.states.PLAY,isServer:false,version:'1.21'});const reference=decoder.parsePacketBuffer(Buffer.concat([Buffer.from([0x11]),Buffer.from(original.trees.find(v=>v.permission===2).hex,'hex')])).data.params;
 assert.deepEqual(source.tree,tree(reference));assert.deepEqual(target.tree,[]);assert.deepEqual(outside.tree,[]);report.tree=source.tree;
 assert.deepEqual((await source.complete('/tit')).texts,['title']);assert.deepEqual((await target.complete('/tit')).texts,[]);
 report.completions=[];for(const row of original.suggestions.filter(v=>v.input.startsWith('title '))){const actual=await source.complete(row.input);assert.deepEqual(actual,{start:row.start,end:row.end,texts:row.texts.filter(v=>!v.startsWith('@'))});report.completions.push({input:row.input,...actual});}
 for(let i=0;i<original.execution.length;i++){
  if(i===1){for(const line of ['team add title','team modify title color gold','team modify title prefix {"text":"[T] ","font":"minecraft:uniform"}','team modify title suffix {"keybind":"key.jump"}','team join title TitleTarget'])await consoleBoard(line);await barrier();}
  if(i===2){await consoleBoard('team leave TitleTarget');await barrier();}
  await execute(source,target,outside,original.execution[i]);
 }
 for(const input of ['title @a clear','title 00000000-0000-0000-0000-000000000000 reset','title TitleTarget title {"selector":"@a"}','title TitleTarget subtitle {"text":"x","hoverEvent":{"action":"show_text","contents":{"score":{"name":"x","objective":"y"}}}}','title TitleTarget actionbar '+JSON.stringify('界'.repeat(22000))])await execute(source,target,outside,{input,ok:false,permission:2,console:false,visibility:0});
 await execute(source,target,outside,original.execution[4]);report.unsupported_and_unencodable_components_fail_privately_with_source_connected=true;
 assert.deepEqual(JSON.parse(await readFile(path.join(folder,'ops.json'),'utf8')).map(v=>v.name),['TitleSource']);await stop();report.ok=true;
}catch(error){report.error=error.stack;process.exitCode=1;}
finally{try{await stop();}catch(e){report.shutdown_error=e.message;report.ok=false;process.exitCode=1;server?.kill('SIGKILL');}report.completed_at=new Date().toISOString();await writeFile(output,JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify({ok:report.ok,report:output,local_diagnostics:folder,error:report.error}));}
