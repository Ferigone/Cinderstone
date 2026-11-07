// SPDX-License-Identifier: GPL-2.0-only
// Actual native Main packets compared with executed-original feedback witnesses.
import mc from 'minecraft-protocol';
import nbt from 'prismarine-nbt';
import assert from 'node:assert/strict';
import {spawn, execFileSync} from 'node:child_process';
import {mkdtemp, readFile, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import path from 'node:path';
import {createHash} from 'node:crypto';

const [binaryArg, output] = process.argv.slice(2);
assert.ok(output, 'BINARY REPORT');
const root=process.cwd(), binary=path.resolve(binaryArg);
const folder=await mkdtemp(path.join(tmpdir(),'admin-feedback-'));
const witnesses=JSON.parse(await readFile('fixtures/admin-feedback-vectors.json','utf8'));
function offlineUUID(name) {
  const bytes=createHash('md5').update('OfflinePlayer:'+name).digest();
  bytes[6]=(bytes[6]&15)|48; bytes[8]=(bytes[8]&63)|128;
  const s=bytes.toString('hex');return `${s.slice(0,8)}-${s.slice(8,12)}-${s.slice(12,16)}-${s.slice(16,20)}-${s.slice(20)}`;
}
await writeFile(path.join(folder,'keys.json'),'{"profilePropertyKeys":[],"playerCertificateKeys":[]}');
await writeFile(path.join(folder,'ops.json'),JSON.stringify(['FeedbackSource','FeedbackOther'].map(name=>({uuid:offlineUUID(name),name,level:4,bypassesPlayerLimit:false}))));
const report={ok:false,native_commit:execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim(),
  native_src_tree:execFileSync('git',['rev-parse','HEAD:src'],{encoding:'utf8'}).trim(),
  native_binary_sha256:createHash('sha256').update(await readFile(binary)).digest('hex'),
  java_jar_sha1:witnesses.java_jar_sha1,rows:[],
  limits:['Original Player display formatting and CommandSourceStack feedback are executed with recording connections and an actual team; this is not a full Java server flow.',
    'Native flat creative profile, two operators, selected success packets only; signing, selectors, errors and global command parity are outside scope.']};
let log='', server;
const clients=[];
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
async function until(test,label,timeout=30000) {
  const end=Date.now()+timeout;
  while(!test()) {
    if(server?.exitCode!==null && server?.exitCode!==undefined)throw Error('server exited: '+label);
    if(Date.now()>end)throw Error('timeout: '+label);
    await pause(20);
  }
}
function hasKey(component,key) {
  return component && typeof component==='object' &&
    (component.translate===key || Object.values(component).some(v=>hasKey(v,key)));
}
async function connect(name) {
  const client=mc.createClient({host:'127.0.0.1',port:25702,username:name,auth:'offline',version:'1.21',keepAlive:false,hideErrors:true});
  clients.push(client);let ready=false,error,tree=false;
  const messages=[], borders=[], difficulties=[];
  client.on('error',e=>{error=e;});client.on('kick_disconnect',p=>{error=Error(JSON.stringify(p));});
  client.on('keep_alive',p=>client.write('keep_alive',p));
  client.on('chunk_batch_finished',()=>client.write('chunk_batch_received',{chunksPerTick:16}));
  client.on('position',p=>{client.write('teleport_confirm',{teleportId:p.teleportId});ready=true;});
  client.on('declare_commands',p=>{tree=p.nodes.some(n=>n.extraNodeData?.name==='time');});
  client.on('world_border_center',p=>borders.push(p));
  client.on('difficulty',p=>difficulties.push(p));
  client.on('packet',(p,meta,buffer)=>{
    if(meta.name==='system_chat')messages.push({component:nbt.simplify(p.content),hex:buffer.subarray(1).toString('hex')});
  });
  async function wait(test,label) {await until(()=>{if(error)throw error;return test();},label);}
  await wait(()=>ready&&tree,name+' admitted operator');
  return {client,messages,borders,difficulties,wait};
}
function consoleLine(line) {server.stdin.write(line+'\n');}
async function teamCommand(line) {
  const offset=log.length;consoleLine(line);
  let event;
  await until(()=>{
    event=log.slice(offset).split('\n').flatMap(line=>{try{return [JSON.parse(line)];}catch{return [];}})
      .find(e=>e.event==='scoreboard_command'&&e.command===line);
    return !!event;
  },line);
  assert.equal(event.ok,true,JSON.stringify(event));
}
async function run(command,key,source,observer,phase) {
  const a=source.messages.length,b=observer.messages.length,l=log.length;
  if(phase==='console')consoleLine(command);else source.client.write('chat_command',{command});
  await observer.wait(()=>observer.messages.slice(b).some(m=>m.component.translate==='chat.type.admin'&&hasKey(m.component,key)),command+' operator broadcast');
  const admin=observer.messages.slice(b).find(m=>m.component.translate==='chat.type.admin'&&hasKey(m.component,key));
  let own;
  if(phase==='console') {
    await until(()=>log.slice(l).includes(`"translate":"${key}"`),'console feedback');
    const events=log.slice(l).split('\n').flatMap(line=>{try{return [JSON.parse(line)];}catch{return [];}});
    own=events.find(e=>e.event==='administration_feedback'&&e.component?.translate===key)?.component;
    assert.ok(own,'console private feedback');
    assert.deepEqual(own,admin.component.with[1],'console reports the original success component');
  } else {
    await source.wait(()=>source.messages.slice(a).some(m=>m.component.translate===key),command+' private feedback');
    own=source.messages.slice(a).find(m=>m.component.translate===key);
  }
  if(phase) {
    const expected=witnesses.rows.find(v=>v.phase===phase&&v.command===command);assert.ok(expected);
    assert.equal(admin.hex,expected.admin_hex,phase+' '+command+' original operator bytes');
    if(phase!=='console')assert.equal(own.hex,expected.private_hex,phase+' '+command+' private bytes');
    report.rows.push({phase,command,private_hex:phase==='console'?null:own.hex,admin_hex:admin.hex});
  }
}
try {
  server=spawn(binary,['--allow-offline','--bind','127.0.0.1','--port','25702','--fixtures',path.join(root,'fixtures/configuration.json'),
    '--world',path.join(folder,'world.json'),'--server-directory',folder,'--view-distance','2','--difficulty','normal','--loopback-service-keys',path.join(folder,'keys.json')],
    {cwd:folder,stdio:['pipe','pipe','pipe']});
  server.stdout.on('data',bytes=>{log+=bytes;});server.stderr.on('data',bytes=>{log+=bytes;});
  await until(()=>log.includes('"event":"listening"'),'listening',120000);
  const source=await connect('FeedbackSource'),observer=await connect('FeedbackOther');
  await teamCommand('team add feedback');
  for(const phase of ['red','blue','none','console']) {
    if(phase==='red'||phase==='blue') {
      await teamCommand(`team modify feedback color ${phase}`);
      await teamCommand('team modify feedback prefix '+JSON.stringify(phase==='red'?{text:'[T] '}:{text:'[U] ',font:'minecraft:uniform'}));
      await teamCommand('team modify feedback suffix '+JSON.stringify(phase==='red'?{text:'!'}:{keybind:'key.jump'}));
      if(phase==='red')await teamCommand('team join feedback FeedbackSource');
    } else if(phase==='none')await teamCommand('team leave FeedbackSource');
    await run('difficulty easy','commands.difficulty.success',source,observer,null);
    for(const [command,key] of [['time set 1000','commands.time.set'],['weather clear 120','commands.weather.set.clear'],
      ['difficulty hard','commands.difficulty.success'],['worldborder center 12 -8','commands.worldborder.center.success']]) {
      // Reset the border through its real command before checking a fresh mutation.
      if(command.startsWith('worldborder')&&phase!=='red')await run('worldborder center 0 0','commands.worldborder.center.success',source,observer,null);
      await run(command,key,source,observer,phase);
    }
    assert.deepEqual(observer.borders.at(-1),{x:12.5,z:-7.5});
    assert.equal(observer.difficulties.at(-1).difficulty,'hard');
  }
  report.current_team_changes=true;report.world_border_mutation=true;report.live_difficulty=3;
  report.console_source='Server';report.ok=true;
} catch(error) {report.error=error.stack;process.exitCode=1;}
finally {
  for(const c of clients)c.end();
  if(server&&server.exitCode===null) {
    server.kill('SIGTERM');
    try{await until(()=>server.exitCode!==null,'graceful shutdown');assert.equal(server.exitCode,0);}
    catch(error){report.shutdown_error=error.message;report.ok=false;process.exitCode=1;server.kill('SIGKILL');}
  }
  await writeFile(path.join(folder,'server.log'),log);
  report.completed_at=new Date().toISOString();await writeFile(output,JSON.stringify(report,null,2)+'\n');
  console.log(JSON.stringify({ok:report.ok,report:output,local_diagnostics:folder,error:report.error}));
}
