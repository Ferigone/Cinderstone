// SPDX-License-Identifier: GPL-2.0-only
// Live client settings, ordered audience barriers and real world-state updates.
import mc from 'minecraft-protocol';
import nbt from 'prismarine-nbt';
import assert from 'node:assert/strict';
import {spawn,execFileSync} from 'node:child_process';
import {mkdtemp,readFile,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import path from 'node:path';
import {createHash} from 'node:crypto';

const [binaryArg,output]=process.argv.slice(2);assert.ok(output,'BINARY REPORT');
const root=process.cwd(),binary=path.resolve(binaryArg),folder=await mkdtemp(path.join(tmpdir(),'admin-visibility-'));
const original=JSON.parse(await readFile('fixtures/admin-visibility-vectors.json','utf8'));
function uuid(name){const b=createHash('md5').update('OfflinePlayer:'+name).digest();b[6]=(b[6]&15)|48;b[8]=(b[8]&63)|128;const s=b.toString('hex');return `${s.slice(0,8)}-${s.slice(8,12)}-${s.slice(12,16)}-${s.slice(16,20)}-${s.slice(20)}`;}
await writeFile(path.join(folder,'keys.json'),'{"profilePropertyKeys":[],"playerCertificateKeys":[]}');
await writeFile(path.join(folder,'ops.json'),JSON.stringify(['VisibilitySource','VisibilityOther'].map(name=>({uuid:uuid(name),name,level:4,bypassesPlayerLimit:false}))));
const report={ok:false,native_commit:execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim(),
  native_src_tree:execFileSync('git',['rev-parse','HEAD:src'],{encoding:'utf8'}).trim(),
  native_binary_sha256:createHash('sha256').update(await readFile(binary)).digest('hex'),java_jar_sha1:original.java_jar_sha1,phases:[],
  limits:['Original ServerPlayer visibility and CommandSourceStack broadcasting execute with recording connections; this is not a full Java server protocol flow.',
    'Native flat creative profile. Shared administration feedback only; overlay chat, listener chat-disabled rejections and other message paths are outside scope.']};
let server,log='',transaction=0;const clients=[];const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
async function until(test,label,timeout=30000){const end=Date.now()+timeout;while(!test()){if(server?.exitCode!==null&&server?.exitCode!==undefined)throw Error('server exited: '+label);if(Date.now()>end)throw Error('timeout: '+label);await pause(20);}}
const has=(c,key)=>c&&typeof c==='object'&&(c.translate===key||Object.values(c).some(v=>has(v,key)));
async function connect(name){
  const client=mc.createClient({host:'127.0.0.1',port:25703,username:name,auth:'offline',version:'1.21',keepAlive:false,hideErrors:true});clients.push(client);
  let ready=false,tree=false,error;const messages=[],borders=[],difficulties=[],completions=[];
  client.on('error',e=>{error=e;});client.on('end',()=>{error=Error('client ended');});
  client.on('keep_alive',p=>client.write('keep_alive',p));client.on('chunk_batch_finished',()=>client.write('chunk_batch_received',{chunksPerTick:16}));
  client.on('position',p=>{client.write('teleport_confirm',{teleportId:p.teleportId});ready=true;});
  client.on('declare_commands',p=>{tree=p.nodes.some(n=>n.extraNodeData?.name==='weather');});
  client.on('world_border_center',p=>borders.push(p));client.on('difficulty',p=>difficulties.push(p));client.on('tab_complete',p=>completions.push(p.transactionId));
  client.on('packet',(p,m,b)=>{if(m.name==='system_chat')messages.push({component:nbt.simplify(p.content),hex:b.subarray(1).toString('hex')});});
  async function wait(test,label){await until(()=>{if(error)throw error;return test();},label);}
  await wait(()=>ready&&tree,name+' admitted operator');
  return {client,messages,borders,difficulties,wait,
    async setting(flags){
      client.write('settings',{locale:'en_US',viewDistance:2,chatFlags:flags,chatColors:true,skinParts:127,mainHand:1,enableTextFiltering:false,enableServerListing:true});
      // This connection processes settings before its subsequent completion request.
      const id=++transaction;client.write('tab_complete',{transactionId:id,text:'weather '});await wait(()=>completions.includes(id),'settings input barrier');
    },
    async command(text,key){const start=messages.length;client.write('chat_command',{command:text});await wait(()=>messages.slice(start).some(m=>m.component.translate===key||has(m.component,key)),text);return messages.slice(start).find(m=>m.component.translate===key||has(m.component,key));}
  };
}
function consoleLine(line){server.stdin.write(line+'\n');}
// Console commands are serialized; this world-state event follows all earlier
// feedback events on the actual entity audience channel, even for Hidden clients.
async function marker(source,observer,x){
  const a=source.borders.length,b=observer.borders.length;consoleLine(`worldborder center ${x} 0`);
  await observer.wait(()=>observer.borders.slice(b).some(p=>p.x===x+0.5),'ordered observer world marker');
  await source.wait(()=>source.borders.slice(a).some(p=>p.x===x+0.5),'ordered source world marker');
}
try{
  server=spawn(binary,['--allow-offline','--bind','127.0.0.1','--port','25703','--fixtures',path.join(root,'fixtures/configuration.json'),
    '--world',path.join(folder,'world.json'),'--server-directory',folder,'--view-distance','2','--loopback-service-keys',path.join(folder,'keys.json')],{cwd:folder,stdio:['pipe','pipe','pipe']});
  server.stdout.on('data',b=>{log+=b;});server.stderr.on('data',b=>{log+=b;});await until(()=>log.includes('"event":"listening"'),'startup',120000);
  const source=await connect('VisibilitySource'),observer=await connect('VisibilityOther');
  let x=10;
  for(const [visibility,flags] of [['FULL',0],['HIDDEN',2],['SYSTEM',1]]){
    await observer.setting(flags);const witness=original.rows.find(r=>r.visibility===visibility);
    const a=source.messages.length,b=observer.messages.length;
    const own=await source.command('weather clear 120','commands.weather.set.clear');
    assert.equal(own.hex,original.rows.find(r=>r.visibility==='FULL').private[0]);
    await marker(source,observer,x++);
    const admin=observer.messages.slice(b).filter(m=>m.component.translate==='chat.type.admin'&&has(m.component,'commands.weather.set.clear')).map(m=>m.hex);
    assert.deepEqual(admin,witness.admin,visibility+' operator feedback');
    assert.ok(!source.messages.slice(a).some(m=>m.component.translate==='chat.type.admin'&&has(m.component,'commands.weather.set.clear')),'source received duplicate operator feedback');
    const c=observer.messages.length;consoleLine('weather clear 120');await marker(source,observer,x++);
    const consoleAdmin=observer.messages.slice(c).filter(m=>m.component.translate==='chat.type.admin'&&has(m.component,'commands.weather.set.clear')).map(m=>m.hex);
    assert.deepEqual(consoleAdmin,witness.console_admin,visibility+' console broadcast');
    report.phases.push({visibility,private_hex:own.hex,admin,console_admin:consoleAdmin,ordered_world_barriers:true});
    if(visibility==='HIDDEN'){
      const d=observer.messages.length;await source.command('difficulty hard','commands.difficulty.success');
      await marker(source,observer,x++);assert.equal(observer.difficulties.at(-1).difficulty,'hard');
      assert.ok(!observer.messages.slice(d).some(m=>has(m.component,'commands.difficulty.success')),'hidden observer received difficulty feedback');
      report.hidden_difficulty_state_received=true;
    }
  }
  const failure=await source.command('difficulty hard','commands.difficulty.failure');
  assert.equal(failure.hex,original.rows.find(r=>r.visibility==='FULL').failure[0]);report.private_error_hex=failure.hex;
  // The existing listener refuses hidden-source commands before administration.
  await source.setting(2);const offset=source.messages.length;
  await source.command('difficulty peaceful','chat.disabled.options');await marker(source,observer,x++);
  assert.ok(!source.messages.slice(offset).some(m=>has(m.component,'commands.difficulty.success')));
  assert.equal(observer.difficulties.at(-1).difficulty,'hard');report.hidden_source_listener_rejection_preserved=true;
  await source.setting(0);await source.command('weather clear 120','commands.weather.set.clear');report.source_restored=true;
  report.ok=true;
}catch(error){report.error=error.stack;process.exitCode=1;}
finally{
  for(const client of clients)client.end();
  if(server&&server.exitCode===null){server.kill('SIGTERM');try{await until(()=>server.exitCode!==null,'shutdown');assert.equal(server.exitCode,0);}catch(error){report.shutdown_error=error.message;report.ok=false;process.exitCode=1;server.kill('SIGKILL');}}
  await writeFile(path.join(folder,'server.log'),log);report.completed_at=new Date().toISOString();await writeFile(output,JSON.stringify(report,null,2)+'\n');
  console.log(JSON.stringify({ok:report.ok,report:output,local_diagnostics:folder,error:report.error}));
}
