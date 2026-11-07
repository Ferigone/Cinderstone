// SPDX-License-Identifier: GPL-2.0-only
// Literal say broadcasts from player/console with live identity and chat visibility.
import mc from 'minecraft-protocol';
import nbt from 'prismarine-nbt';
import assert from 'node:assert/strict';
import {spawn,execFileSync} from 'node:child_process';
import {mkdtemp,readFile,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import path from 'node:path';
import {createHash} from 'node:crypto';

const [binaryArg,output]=process.argv.slice(2);assert.ok(output,'BINARY REPORT');
const root=process.cwd(),binary=path.resolve(binaryArg),folder=await mkdtemp(path.join(tmpdir(),'say-command-'));
const original=JSON.parse(await readFile('fixtures/say-vectors.json','utf8'));
function uuid(name){const b=createHash('md5').update('OfflinePlayer:'+name).digest();b[6]=(b[6]&15)|48;b[8]=(b[8]&63)|128;const s=b.toString('hex');return `${s.slice(0,8)}-${s.slice(8,12)}-${s.slice(12,16)}-${s.slice(16,20)}-${s.slice(20)}`;}
await writeFile(path.join(folder,'keys.json'),'{"profilePropertyKeys":[],"playerCertificateKeys":[]}');
await writeFile(path.join(folder,'ops.json'),JSON.stringify([{uuid:uuid('SaySource'),name:'SaySource',level:2,bypassesPlayerLimit:false}]));
const report={ok:false,native_commit:execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim(),native_src_tree:execFileSync('git',['rev-parse','HEAD:src'],{encoding:'utf8'}).trim(),
  native_binary_sha256:createHash('sha256').update(await readFile(binary)).digest('hex'),java_jar_sha1:original.java_jar_sha1,execution:[],
  limits:['Original SayCommand/MessageArgument executes to a recording PlayerList broadcast sink; this is not a full Java server protocol flow.',
    'Original body/name bytes are compared in complete packets using the actual configured say registry ID. Selectors, external moderation and real-account trust are outside scope.']};
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));let server,log='',transaction=0,marker=0;const clients=[],entries=[];
async function until(test,label,timeout=30000){const end=Date.now()+timeout;while(!test()){if(server?.exitCode!==null&&server?.exitCode!==undefined)throw Error('server exited: '+label+' '+log.slice(-1400));if(Date.now()>end)throw Error('timeout: '+label);await pause(20);}}
async function start(){log='';server=spawn(binary,['--allow-offline','--bind','127.0.0.1','--port','25706','--fixtures',path.join(root,'fixtures/configuration.json'),
  '--world',path.join(folder,'world.json'),'--server-directory',folder,'--view-distance','2','--loopback-service-keys',path.join(folder,'keys.json')],{cwd:folder,stdio:['pipe','pipe','pipe']});server.stdout.on('data',b=>{log+=b;});server.stderr.on('data',b=>{log+=b;});await until(()=>log.includes('"event":"listening"'),'startup',120000);}
async function stop(){if(!server)return;for(const c of clients.splice(0))c.end();const current=server;current.kill('SIGTERM');await until(()=>current.exitCode!==null,'shutdown');assert.equal(current.exitCode,0,log.slice(-1800));await writeFile(path.join(folder,`server-${Date.now()}.log`),log);server=null;}
const has=(c,key)=>c&&typeof c==='object'&&(c.translate===key||Object.values(c).some(v=>has(v,key)));
const red=c=>c&&typeof c==='object'&&(c.color==='red'||Object.values(c).some(red));
function tree(packet){const nodes=packet.nodes,id=nodes[packet.rootIndex].children.find(i=>nodes[i].extraNodeData?.name==='say');if(id===undefined)return null;const out=[];function walk(i,p){const n=nodes[i],e=n.extraNodeData,v=[...p,e.name];out.push({path:v.join(' '),executable:!!n.flags.has_command,parser:e.parser??null,suggestion:e.suggestionType??null});n.children.forEach(c=>walk(c,v));}walk(id,[]);return out;}
async function connect(name){
  const client=mc.createClient({host:'127.0.0.1',port:25706,username:name,auth:'offline',version:'1.21',keepAlive:false,hideErrors:true});clients.push(client);
  let ready=false,commandTree,error;const messages=[],chat=[],teams=[],completion=[];let sayId;
  client.on('registry_data',p=>{if(p.id==='minecraft:chat_type')sayId=p.entries.findIndex(e=>e.key==='minecraft:say_command');});
  client.on('error',e=>{error=e;});client.on('end',()=>{error=Error(name+' ended');});
  client.on('keep_alive',p=>client.write('keep_alive',p));client.on('chunk_batch_finished',()=>client.write('chunk_batch_received',{chunksPerTick:16}));
  client.on('position',p=>{client.write('teleport_confirm',{teleportId:p.teleportId});ready=true;});client.on('declare_commands',p=>{commandTree=tree(p);});client.on('teams',p=>teams.push(p));client.on('tab_complete',p=>completion.push(p));
  client.on('packet',(p,m,b)=>{if(m.name==='system_chat')messages.push({component:nbt.simplify(p.content),hex:b.subarray(1).toString('hex')});if(m.name==='profileless_chat')chat.push({packet:p,hex:b.subarray(1).toString('hex')});});
  async function wait(test,label){await until(()=>{if(error)throw error;return test();},label);}
  await wait(()=>ready&&commandTree!==undefined,name+' admitted');
  const entry={client,messages,chat,teams,wait,get sayId(){return sayId;},get tree(){return commandTree;},
    async complete(text){const id=++transaction;client.write('tab_complete',{transactionId:id,text});await wait(()=>completion.some(p=>p.transactionId===id),'completion '+text);const p=completion.find(p=>p.transactionId===id);return {start:p.start,end:p.start+p.length,texts:p.matches.map(v=>v.match)};},
    async command(text,key){
      // Respect the real non-op command-spam policy instead of granting operator status.
      await pause(1050);const n=messages.length;client.write('chat_command',{command:text});
      await wait(()=>{const rows=messages.slice(n);if(key&&rows.some(m=>red(m.component))&&!rows.some(m=>has(m.component,key)))throw Error('unexpected feedback for '+text+': '+JSON.stringify(rows));return rows.some(m=>key?has(m.component,key):red(m.component));},text);
      return messages.slice(n).find(m=>key?has(m.component,key):red(m.component));
    },
    async setting(flags){client.write('settings',{locale:'en_US',viewDistance:2,chatFlags:flags,chatColors:true,skinParts:127,mainHand:1,enableTextFiltering:false,enableServerListing:true});await this.complete('say ');}
  };entries.push(entry);return entry;
}
function consoleLine(line){server.stdin.write(line+'\n');}
async function consoleBoard(line){const n=log.length;consoleLine(line);let event;await until(()=>{event=log.slice(n).split('\n').flatMap(line=>{try{return [JSON.parse(line)];}catch{return [];}}).find(e=>e.event==='scoreboard_command'&&e.command===line);return !!event;},line);assert.equal(event.ok,true,JSON.stringify(event));}
async function barrier(observer){const name='say_sync_'+(++marker);consoleLine('team add '+name);await Promise.all(entries.map(p=>p.wait(()=>p.teams.some(t=>t.team===name),'ordered audience barrier')));}
async function broadcast(source,observer,row){
  const peers=[source,observer],before=peers.map(p=>({chat:p.chat.length,system:p.messages.length}));const n=(log.match(/"event":"say_command"/g)||[]).length;
  if(row.console)consoleLine(row.input);else source.client.write('chat_command',{command:row.input});
  await until(()=>(log.match(/"event":"say_command"/g)||[]).length>n,'actual say completion');await barrier(observer);
  const delivered=[];for(let i=0;i<peers.length;i++){
    const p=peers[i],actual=p.chat.slice(before[i].chat);const visible=i===0||observerVisible;
    assert.equal(actual.length,visible?1:0,'say visibility');assert.equal(p.messages.length,before[i].system,'no private/admin success echo');
    if(visible){assert.ok(p.sayId>=0&&p.sayId<127);const expected=row.body_hex+(p.sayId+1).toString(16).padStart(2,'0')+row.name_hex+'00';assert.equal(actual[0].hex,expected);assert.equal(actual[0].packet.type.chatType,p.sayId);delivered.push({recipient:i===0?'SaySource':'SayOther',hex:actual[0].hex,configured_type:p.sayId});}
  }report.execution.push({input:row.input,console:row.console,observer_visible:observerVisible,delivered});
}
let observerVisible=true;
try{
  await start();const source=await connect('SaySource'),observer=await connect('SayOther');assert.equal(observer.tree,null);assert.deepEqual(source.tree.map(n=>({path:n.path,executable:n.executable,parser:n.parser})),[{path:'say',executable:false,parser:null},{path:'say message',executable:true,parser:'minecraft:message'}]);report.tree=source.tree;
  assert.ok((await source.complete('/sa')).texts.includes('say'));assert.ok(!(await observer.complete('/sa')).texts.includes('say'));report.permission_tree_and_completion=true;
  const counts=[source.chat.length,observer.chat.length];await observer.command('say denied');await barrier(observer);assert.deepEqual([source.chat.length,observer.chat.length],counts);report.denied_nonop_does_not_broadcast=true;
  await broadcast(source,observer,original.execution[0]);
  for(const line of ['team add sayteam','team modify sayteam color blue','team modify sayteam prefix {"text":"[S] ","font":"minecraft:uniform"}','team modify sayteam suffix {"keybind":"key.jump"}','team join sayteam SaySource'])await consoleBoard(line);
  await barrier(observer);await broadcast(source,observer,original.execution[1]);
  for(const visibility of [1,2]){await observer.setting(visibility);observerVisible=false;await broadcast(source,observer,original.execution[2]);}report.commands_only_and_hidden_suppressed=true;
  await observer.setting(0);observerVisible=true;await broadcast(source,observer,original.execution[2]);report.live_full_restoration=true;
  const before=[source.chat.length,observer.chat.length];for(const text of ['say','say @a','say '+ '😀'.repeat(129)])await source.command(text);await barrier(observer);assert.deepEqual([source.chat.length,observer.chat.length],before);report.empty_selector_and_oversize_do_not_broadcast=true;
  await consoleBoard('team leave SaySource');await barrier(observer);await broadcast(source,observer,original.execution[0]);report.live_source_identity=true;
  await stop();report.ok=true;
}catch(error){report.error=error.stack;process.exitCode=1;}
finally{try{await stop();}catch(error){report.shutdown_error=error.message;report.ok=false;process.exitCode=1;server?.kill('SIGKILL');}report.completed_at=new Date().toISOString();await writeFile(output,JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify({ok:report.ok,report:output,local_diagnostics:folder,error:report.error}));}
