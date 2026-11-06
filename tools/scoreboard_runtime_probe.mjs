// SPDX-License-Identifier: GPL-2.0-only
// One flow used unchanged against native Main and the unchanged Java server.
import mc from 'minecraft-protocol';
import nbt from 'prismarine-nbt';
import {execFileSync} from 'node:child_process';
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {mkdtemp,readFile,writeFile,mkdir,copyFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import path from 'node:path';
import {createHash} from 'node:crypto';
import {createWriteStream} from 'node:fs';
const [binary,jar,eula,report]=process.argv.slice(2);
assert.ok(report,'Usage: node tools/scoreboard_runtime_probe.mjs BINARY JAVA_JAR EXISTING_EULA REPORT');
const root=process.cwd(),scratch=await mkdtemp(path.join(tmpdir(),'scoreboard-runtime-'));
const result={schema_version:1,ok:false,native_commit:execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim(),native_src_tree:execFileSync('git',['rev-parse','HEAD:src'],{encoding:'utf8'}).trim(),completed_at:null,traces:{},differences:[]};
const pause=ms=>new Promise(r=>setTimeout(r,ms));
async function until(test,label,timeout=90000){const end=Date.now()+timeout;while(!test()){if(Date.now()>end)throw Error('Timeout: '+label);await pause(25);}}
const names=new Set(['teams','scoreboard_objective','scoreboard_display_objective','scoreboard_score','reset_score']);
let server;
async function start(kind,folder,port){
  let log='';const stream=createWriteStream(path.join(folder,'current-probe-server.log'));
  const args=kind==='java'?['-XX:ActiveProcessorCount=2','-Xms128M','-Xmx512M','-jar',path.resolve(jar),'nogui']:
    ['--allow-offline','--world',path.join(folder,'world.json'),'--port',String(port),'--fixtures',path.join(root,'fixtures/configuration.json'),'--loopback-service-keys',path.join(folder,'keys.json')];
  const child=spawn(kind==='java'?'java':path.resolve(binary),args,{cwd:folder,stdio:['pipe','pipe','pipe']});
  child.stdout.on('data',b=>{log+=b;stream.write(b)});child.stderr.on('data',b=>{log+=b;stream.write(b)});child.on('exit',()=>stream.end());
  child.on('error',e=>{log+=e.message});
  server={child,kind,folder,log:()=>log,command:line=>child.stdin.write(line+'\n')};
  await until(()=>log.includes(kind==='java'?'Done (':'"event":"listening"')||child.exitCode!==null,'server startup');
  assert.equal(child.exitCode,null,log.slice(-2000));return server;
}
async function stop(){if(!server)return;const {child,kind,folder}=server;if(kind==='java')server.command('stop');else child.kill('SIGTERM');
  await until(()=>child.exitCode!==null||child.signalCode!==null,'graceful shutdown');
  await writeFile(path.join(folder,'probe-server.log'),server.log());assert.equal(child.exitCode,0,server.log().slice(-3000));server=null;}
async function connect(port,username,record){
  const client=mc.createClient({host:'127.0.0.1',port,username,version:'1.21',auth:'offline',keepAlive:false,hideErrors:true});
  let ready=false,error;
  client.on('error',e=>error=e);client.on('kick_disconnect',p=>error=Error(JSON.stringify(p)));
  client.on('keep_alive',p=>client.write('keep_alive',p));
  client.on('chunk_batch_finished',()=>client.write('chunk_batch_received',{chunksPerTick:16}));
  client.on('position',p=>{client.write('teleport_confirm',{teleportId:p.teleportId});ready=true;});
  client.on('raw',(b,m)=>{if(names.has(m.name))record.push({name:m.name,hex:b.toString('hex')});});
  const packets=[],chatPackets=[];client.on('packet',(p,m)=>{if(names.has(m.name))packets.push({name:m.name,value:p});if(['profileless_chat','player_chat'].includes(m.name))chatPackets.push(m.name);});
  await until(()=>ready||error,'client join');if(error)throw error;
  return {client,packets,chatPackets,async close(){client.end();await until(()=>client.ended,'client disconnect');await pause(350);}};
}
try{
  result.native_binary_sha256=createHash('sha256').update(await readFile(binary)).digest('hex');
  result.java_jar_sha1=createHash('sha1').update(await readFile(jar)).digest('hex');
  assert.equal(result.java_jar_sha1,'450698d1863ab5180c25d7c804ef0fe6369dd1ba');
  assert.match(await readFile(eula,'utf8'),/^eula=true$/m);
  for(const [kind,port] of [['native',25621],['java',25622]]){
    const folder=path.join(scratch,kind);await mkdir(folder);
    if(kind==='java'){
      await copyFile(eula,path.join(folder,'eula.txt'));
      await writeFile(path.join(folder,'server.properties'),[
        'server-ip=127.0.0.1','server-port='+port,'online-mode=false','enforce-secure-profile=false','gamemode=creative',
        'level-seed=0','level-type=minecraft:flat','generate-structures=false','spawn-protection=0','view-distance=2','simulation-distance=2',
        'generator-settings={"biome":"minecraft:plains","layers":[{"block":"minecraft:bedrock","height":1},{"block":"minecraft:dirt","height":2},{"block":"minecraft:grass_block","height":1}]}',
      ].join('\n')+'\n');
    }else await writeFile(path.join(folder,'keys.json'),'{"playerCertificateKeys":[],"profilePropertyKeys":[]}');
    await start(kind,folder,port);
    for(const cmd of ['scoreboard objectives add leaves minecraft.custom:minecraft.leave_game','scoreboard objectives setdisplay sidebar leaves','scoreboard objectives modify leaves displayautoupdate true',
      'team add test','team modify test prefix {"text":"[T] ","font":"minecraft:uniform","hoverEvent":{"action":"show_text","contents":"Team"}}','team join test ScoreProbe'])server.command(cmd);
    await pause(700);
    const live=[],observer=await connect(port,'ScoreProbe',live);
    await until(()=>observer.packets.some(p=>p.name==='teams'&&p.value.mode===0)&&observer.packets.some(p=>p.name==='scoreboard_objective'),'team and sidebar join sync');
    observer.client.chat('rich team chat probe');
    await until(()=>observer.chatPackets.length>0,'rich team chat without disconnect');
    server.command('scoreboard players display name ScoreProbe leaves {"text":"Label"}');
    server.command('scoreboard players display numberformat ScoreProbe leaves blank');
    await until(()=>observer.packets.some(p=>p.name==='scoreboard_score'&&p.value.itemName==='ScoreProbe'&&p.value.number_format===0),'display name/numberformat grammar');
    const other=await connect(port,'OtherProbe',[]);await other.close();
    await until(()=>observer.packets.some(p=>p.name==='scoreboard_score'&&p.value.itemName==='OtherProbe'&&p.value.value===1),'real leave_game criterion update');
    // Player command route, trigger privilege, and denied non-op mutation.
    observer.client.write('chat_command',{command:'team add forbidden'});
    await pause(250);
    assert.ok(!observer.packets.some(p=>p.name==='teams'&&p.value.team==='forbidden'),'non-op mutation must be denied');
    for(const cmd of ['scoreboard objectives add button trigger','scoreboard objectives modify button displayautoupdate true',
      'scoreboard players display name ScoreProbe button {"text":"Old"}','scoreboard players enable ScoreProbe button'])server.command(cmd);
    await pause(400);
    observer.client.write('chat_command',{command:'trigger button add 3'});await pause(350);
    server.command('scoreboard objectives setdisplay list button');
    await until(()=>observer.packets.some(p=>p.name==='scoreboard_score'&&p.value.itemName==='ScoreProbe'&&p.value.scoreName==='button'&&p.value.value===3),'non-op trigger command');
    const liveEnd=live.length;
    await observer.close();await stop();
    await start(kind,folder,port);
    const restarted=[],again=await connect(port,'ScoreProbe',restarted);
    await until(()=>again.packets.some(p=>p.name==='scoreboard_score'&&p.value.itemName==='OtherProbe'&&p.value.value===1)&&again.packets.some(p=>p.name==='scoreboard_score'&&p.value.scoreName==='button'&&p.value.value===3),'restart score restoration');
    assert.ok(again.packets.some(p=>p.name==='teams'&&p.value.team==='test'&&p.value.players.includes('ScoreProbe')),'team membership restored');
    result.traces[kind]={live:live.slice(0,liveEnd),restart:restarted.slice(),checks:{real_criterion:true,team_prefix:true,permission_denial:true,nonop_trigger:true,display_grammar:true,display_auto_update:true,rich_team_chat:true,restart:true}};
    await again.close();await stop();
    const parsedNbt=(await nbt.parse(await readFile(path.join(folder,kind==='java'?'world/data/scoreboard.dat':'data/scoreboard.dat')))).parsed;
    const parsed=nbt.simplify(parsedNbt);
    const canonical=v=>Array.isArray(v)?v.map(canonical):v&&typeof v==='object'?Object.fromEntries(Object.keys(v).sort().map(k=>[k,canonical(v[k])])):v;
    result.traces[kind].saved_data=canonical(parsed);
    result.traces[kind].saved_data_nbt=canonical(parsedNbt);

  }
  for(const phase of ['live','restart']){
    if(JSON.stringify(result.traces.native[phase])!==JSON.stringify(result.traces.java[phase]))result.differences.push(phase);
  }
  // ScoreboardSaveData's PlayerScores list serializes an objective-identity map.
  // Its row order is not semantic. Keep the raw rows in the report, compare by key.
  const scoreMap=v=>{const copy=structuredClone(v);copy.value.data.value.PlayerScores.value.value.sort((a,b)=>a.Name.value.localeCompare(b.Name.value,'en')||a.Objective.value.localeCompare(b.Objective.value,'en'));return copy;};
  result.saved_data_comparison='NBT values equal; PlayerScores rows compared by (Name,Objective), original row order retained above';
  assert.deepEqual(scoreMap(result.traces.native.saved_data_nbt),scoreMap(result.traces.java.saved_data_nbt),'saved scoreboard NBT differs');
  assert.deepEqual(result.differences,[],'scoreboard packet bytes/order differ');result.ok=true;
}catch(e){result.error=e.stack;process.exitCode=1;}
finally{try{await stop()}catch(e){result.shutdown_error=e.message;process.exitCode=1;}result.completed_at=new Date().toISOString();await writeFile(report,JSON.stringify(result,null,2)+'\n');}
