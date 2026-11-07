// SPDX-License-Identifier: GPL-2.0-only
// Non-op trigger use, live enable/lock state, real scoreboard packets and restart.
import mc from 'minecraft-protocol';
import nbt from 'prismarine-nbt';
import assert from 'node:assert/strict';
import {spawn,execFileSync} from 'node:child_process';
import {mkdtemp,readFile,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import path from 'node:path';
import {createHash} from 'node:crypto';

const [binaryArg,output]=process.argv.slice(2);assert.ok(output,'BINARY REPORT');
const root=process.cwd(),binary=path.resolve(binaryArg),folder=await mkdtemp(path.join(tmpdir(),'trigger-command-'));
const original=JSON.parse(await readFile('fixtures/trigger-vectors.json','utf8'));
function uuid(name){const b=createHash('md5').update('OfflinePlayer:'+name).digest();b[6]=(b[6]&15)|48;b[8]=(b[8]&63)|128;const s=b.toString('hex');return `${s.slice(0,8)}-${s.slice(8,12)}-${s.slice(12,16)}-${s.slice(16,20)}-${s.slice(20)}`;}
await writeFile(path.join(folder,'keys.json'),'{"profilePropertyKeys":[],"playerCertificateKeys":[]}');
await writeFile(path.join(folder,'ops.json'),JSON.stringify([{uuid:uuid('TriggerOp'),name:'TriggerOp',level:4,bypassesPlayerLimit:false}]));
const report={ok:false,native_commit:execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim(),native_src_tree:execFileSync('git',['rev-parse','HEAD:src'],{encoding:'utf8'}).trim(),
  native_binary_sha256:createHash('sha256').update(await readFile(binary)).digest('hex'),java_jar_sha1:original.java_jar_sha1,execution:[],
  limits:['Original TriggerCommand and real ServerScoreboard execute with recording transport; this is not a full Java server protocol flow.',
    'Selected complete success/domain-failure/admin/score packet payloads are compared. General syntax-error context decoration and other scoreboard roots are outside scope.']};
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));let server,log='',transaction=0,marker=0;const clients=[];
async function until(test,label,timeout=30000){const end=Date.now()+timeout;while(!test()){if(server?.exitCode!==null&&server?.exitCode!==undefined)throw Error('server exited: '+label+' '+log.slice(-1400));if(Date.now()>end)throw Error('timeout: '+label);await pause(20);}}
async function start(){log='';server=spawn(binary,['--allow-offline','--bind','127.0.0.1','--port','25705','--fixtures',path.join(root,'fixtures/configuration.json'),
  '--world',path.join(folder,'world.json'),'--server-directory',folder,'--view-distance','2','--loopback-service-keys',path.join(folder,'keys.json')],{cwd:folder,stdio:['pipe','pipe','pipe']});server.stdout.on('data',b=>{log+=b;});server.stderr.on('data',b=>{log+=b;});await until(()=>log.includes('"event":"listening"'),'startup',120000);}
async function stop(){if(!server)return;for(const c of clients.splice(0))c.end();const current=server;current.kill('SIGTERM');await until(()=>current.exitCode!==null,'shutdown');assert.equal(current.exitCode,0,log.slice(-1800));await writeFile(path.join(folder,`server-${Date.now()}.log`),log);server=null;}
const has=(c,key)=>c&&typeof c==='object'&&(c.translate===key||Object.values(c).some(v=>has(v,key)));
const red=c=>c&&typeof c==='object'&&(c.color==='red'||Object.values(c).some(red));
function tree(packet){const nodes=packet.nodes,id=nodes[packet.rootIndex].children.find(i=>nodes[i].extraNodeData?.name==='trigger');if(id===undefined)return null;const out=[];function walk(i,p){const n=nodes[i],e=n.extraNodeData,v=[...p,e.name];out.push({path:v.join(' '),executable:!!n.flags.has_command,parser:e.parser??null,suggestion:e.suggestionType??null});n.children.forEach(c=>walk(c,v));}walk(id,[]);return out;}
async function connect(name){
  const client=mc.createClient({host:'127.0.0.1',port:25705,username:name,auth:'offline',version:'1.21',keepAlive:false,hideErrors:true});clients.push(client);
  let ready=false,commandTree,error;const messages=[],scores=[],teams=[],completion=[];
  client.on('error',e=>{error=e;});client.on('end',()=>{error=Error(name+' ended');});
  client.on('keep_alive',p=>client.write('keep_alive',p));client.on('chunk_batch_finished',()=>client.write('chunk_batch_received',{chunksPerTick:16}));
  client.on('position',p=>{client.write('teleport_confirm',{teleportId:p.teleportId});ready=true;});client.on('declare_commands',p=>{commandTree=tree(p);});client.on('teams',p=>teams.push(p));client.on('tab_complete',p=>completion.push(p));
  client.on('packet',(p,m,b)=>{if(m.name==='system_chat')messages.push({component:nbt.simplify(p.content),hex:b.subarray(1).toString('hex')});if(m.name==='scoreboard_score')scores.push({packet:p,hex:b.subarray(1).toString('hex')});});
  async function wait(test,label){await until(()=>{if(error)throw error;return test();},label);}
  await wait(()=>ready&&commandTree!==undefined,name+' admitted');
  return {client,messages,scores,teams,wait,get tree(){return commandTree;},
    async complete(text){const id=++transaction;client.write('tab_complete',{transactionId:id,text});await wait(()=>completion.some(p=>p.transactionId===id),'completion '+text);const p=completion.find(p=>p.transactionId===id);return {start:p.start,end:p.start+p.length,texts:p.matches.map(v=>v.match)};},
    async command(text,key){
      // Respect the real non-op command-spam policy instead of granting operator status.
      await pause(1050);const n=messages.length;client.write('chat_command',{command:text});
      await wait(()=>{const rows=messages.slice(n);if(key&&rows.some(m=>red(m.component))&&!rows.some(m=>has(m.component,key)))throw Error('unexpected feedback for '+text+': '+JSON.stringify(rows));return rows.some(m=>key?has(m.component,key):red(m.component));},text);
      return messages.slice(n).find(m=>key?has(m.component,key):red(m.component));
    },
    async setting(flags){client.write('settings',{locale:'en_US',viewDistance:2,chatFlags:flags,chatColors:true,skinParts:127,mainHand:1,enableTextFiltering:false,enableServerListing:true});await this.complete('trigger ');}
  };
}
function consoleLine(line){server.stdin.write(line+'\n');}
async function consoleScore(line){const n=log.length;consoleLine(line);let event;await until(()=>{event=log.slice(n).split('\n').flatMap(line=>{try{return [JSON.parse(line)];}catch{return [];}}).find(e=>e.event==='scoreboard_command'&&e.command===line);return !!event;},line);assert.equal(event.ok,true,JSON.stringify(event));}
async function barrier(observer){const name='trigger_sync_'+(++marker);consoleLine('team add '+name);await observer.wait(()=>observer.teams.some(p=>p.team===name),'ordered scoreboard barrier');}
async function reset(observer,value){await consoleScore(`scoreboard players set TriggerUser click ${value}`);await consoleScore('scoreboard players enable TriggerUser click');await barrier(observer);}
async function run(source,observer,row,hidden=false){
  const a=observer.messages.length,b=observer.scores.length;const key=row.ok?(row.input.includes(' add ')?'commands.trigger.add.success':row.input.includes(' set ')?'commands.trigger.set.success':'commands.trigger.simple.success'):row.error_key;
  const own=await source.command(row.input,key);assert.equal(own.hex,row.feedback[0]);await barrier(observer);
  const admins=observer.messages.slice(a).filter(m=>m.component.translate==='chat.type.admin'&&['commands.trigger.simple.success','commands.trigger.add.success','commands.trigger.set.success'].some(k=>has(m.component,k))).map(m=>m.hex);
  assert.deepEqual(admins,hidden?[]:row.admin);assert.deepEqual(observer.scores.slice(b).map(p=>p.hex),row.score_packets);
  report.execution.push({input:row.input,ok:row.ok,feedback_hex:own.hex,admin:admins,score_packets:observer.scores.slice(b).map(p=>p.hex),hidden_observer:hidden});
}
try{
  await start();let source=await connect('TriggerUser'),observer=await connect('TriggerOp');assert.ok(source.tree);report.tree=source.tree;
  assert.deepEqual(source.tree.map(n=>({path:n.path,executable:n.executable,parser:n.parser})),[
    {path:'trigger',executable:false,parser:null},{path:'trigger objective',executable:true,parser:'minecraft:objective'},
    {path:'trigger objective add',executable:false,parser:null},{path:'trigger objective add value',executable:true,parser:'brigadier:integer'},
    {path:'trigger objective set',executable:false,parser:null},{path:'trigger objective set value',executable:true,parser:'brigadier:integer'}]);
  assert.equal(source.tree[1].suggestion,'minecraft:ask_server');assert.deepEqual((await source.complete('/tr')).texts,['trigger']);
  for(const name of ['click','deep_click','deep.click','locked','absent','dummy'])await consoleScore(`scoreboard objectives add ${name} ${name==='dummy'?'dummy':'trigger'}${name==='click'?' {"text":"Press","color":"gold","bold":true}':''}`);
  await consoleScore('scoreboard objectives setdisplay sidebar click');await barrier(observer);assert.deepEqual((await source.complete('trigger ')).texts,[]);
  const row=input=>original.execution.find(v=>v.input===input&&!v.console);
  for(const input of ['trigger missing','trigger dummy','trigger absent'])await run(source,observer,row(input));
  const disabled=original.execution.find(v=>v.input==='trigger click'&&!v.enabled);await run(source,observer,disabled);
  for(const name of ['click','deep_click','deep.click'])await consoleScore('scoreboard players enable TriggerUser '+name);await barrier(observer);
  report.completions=[];for(const c of original.suggestions.filter(c=>c.input.startsWith('trigger '))){const v=await source.complete(c.input);assert.deepEqual(v,{start:c.start,end:c.end,texts:c.texts});report.completions.push({input:c.input,...v});}
  const before=observer.scores.length;for(const input of ['trigger click add 5 trailing','trigger click set bad','trigger click add 2147483648']){await source.command(input);assert.ok((await source.complete('trigger ')).texts.includes('click'));}await barrier(observer);assert.equal(observer.scores.length,before);report.invalid_input_preserves_value_and_enable=true;
  const consoleOffset=log.length;consoleLine('trigger click');await until(()=>log.slice(consoleOffset).includes('permissions.requires.player'),'console player requirement');assert.ok((await source.complete('trigger ')).texts.includes('click'));report.console_does_not_consume_enable=true;
  await run(source,observer,original.execution[0]);assert.ok(!(await source.complete('trigger ')).texts.includes('click'));await run(source,observer,disabled);report.one_use_lock=true;
  await reset(observer,5);await run(source,observer,row('trigger click add 3'));
  await reset(observer,2147483647);await run(source,observer,row('trigger click add 1'));report.wrapping_result=-2147483648;
  await observer.setting(2);await reset(observer,5);await run(source,observer,row('trigger click add 3'),true);await observer.setting(0);report.hidden_observer_still_receives_score_packets=true;
  await reset(observer,5);await run(source,observer,row('trigger click set -7'));
  const ops=JSON.parse(await readFile(path.join(folder,'ops.json'),'utf8'));assert.deepEqual(ops.map(v=>v.name),['TriggerOp']);report.source_remains_nonop=true;
  await stop();let data=nbt.simplify((await nbt.parse(await readFile(path.join(folder,'data/scoreboard.dat')))).parsed).data;
  const score=data.PlayerScores.find(v=>v.Name==='TriggerUser'&&v.Objective==='click');assert.equal(score.Score,-7);assert.equal(score.Locked,1);assert.ok(!data.PlayerScores.some(v=>v.Name==='TriggerUser'&&['missing','absent','dummy'].includes(v.Objective)));report.saved_score={value:score.Score,locked:score.Locked};
  await start();source=await connect('TriggerUser');observer=await connect('TriggerOp');assert.ok(!(await source.complete('trigger ')).texts.includes('click'));await run(source,observer,disabled);await stop();report.cold_restart_preserves_value_and_lock=true;report.ok=true;
}catch(error){report.error=error.stack;process.exitCode=1;}
finally{try{await stop();}catch(error){report.shutdown_error=error.message;report.ok=false;process.exitCode=1;server?.kill('SIGKILL');}report.completed_at=new Date().toISOString();await writeFile(output,JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify({ok:report.ok,report:output,local_diagnostics:folder,error:report.error}));}
