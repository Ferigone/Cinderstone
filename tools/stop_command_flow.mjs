// SPDX-License-Identifier: GPL-2.0-only
// Real, unchanged Java 1.21 and native Main: operator stop, ordered shutdown and save/restart.
import mc from 'minecraft-protocol';
import nbt from 'prismarine-nbt';
import assert from 'node:assert/strict';
import {spawn, execFileSync} from 'node:child_process';
import {mkdtemp, mkdir, readFile, writeFile, copyFile, cp, rmdir} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import path from 'node:path';
import {createHash} from 'node:crypto';

const [binaryArg, jarArg, eulaArg, output] = process.argv.slice(2);
assert.ok(output, 'BINARY PINNED_JAR EXISTING_EULA REPORT');
const root = process.cwd(), binary = path.resolve(binaryArg), jar = path.resolve(jarArg);
const scratch = await mkdtemp(path.join(path.dirname(output), 'stop-command-'));
const sha = (bytes, kind = 'sha256') => createHash(kind).update(bytes).digest('hex');
const lock = JSON.parse(await readFile('fixtures/source-lock.json', 'utf8')).oracle;
const jarBytes = await readFile(jar);
assert.equal(sha(jarBytes, 'sha1'), lock.sha1); assert.equal(jarBytes.length, lock.size);
assert.ok((await readFile(eulaArg, 'utf8')).split(/\r?\n/).includes('eula=true'));
const result = {ok: false, native_commit: execFileSync('git', ['rev-parse', 'HEAD'], {encoding: 'utf8'}).trim(),
  native_src_tree: execFileSync('git', ['rev-parse', 'HEAD:src'], {encoding: 'utf8'}).trim(),
  native_binary_sha256: sha(await readFile(binary)), java_jar_sha1: lock.sha1, traces: {},
  limits: ['Supported single-level creative profile, admitted PLAY clients; login/configuration phase shutdown parity is excluded.',
    'Selected subtree/completions, stop feedback/operator components and shutdown disconnect bytes/order are exact. General error rendering and wall-clock tick timing are excluded.',
    'Time recovery allows 200 ticks for startup/admission; inventory slot identity/count must match exactly. Native PLAY drain is bounded by the 15s write deadline plus 1s, then saves retained actors.',
    'Root completions retain implemented save-all/seed/setidletimeout/stop roots, excluding the unimplemented stopsound sibling; an empty selected completion normalizes its range to 0/0. Full Java catalog is outside this work package.']};
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
let server;
async function until(test, label, timeout = 60000) {
  const deadline = Date.now() + timeout;
  while (!test()) {
    if (server && server.child.exitCode !== null) throw Error('Server exited: ' + label);
    if (Date.now() > deadline) throw Error('Timeout: ' + label);
    await pause(20);
  }
}
async function start(kind, folder, port, imported = true) {
  let log = '';
  const args = kind === 'java' ? ['-XX:ActiveProcessorCount=2', '-Xms128M', '-Xmx768M', '-jar', jar, 'nogui'] :
    ['--allow-offline', '--bind', '127.0.0.1', '--port', String(port), '--fixtures', path.join(root, 'fixtures/configuration.json'),
      ...(imported ? ['--anvil-world', path.join(folder, 'world')] : []), '--world', path.join(folder, 'world.json'),
      '--server-directory', folder, '--view-distance', '2', '--loopback-service-keys', path.join(folder, 'keys.json')];
  const child = spawn(kind === 'java' ? 'java' : binary, args, {cwd: folder, stdio: ['pipe', 'pipe', 'pipe']});
  child.stdout.on('data', bytes => {log += bytes;}); child.stderr.on('data', bytes => {log += bytes;});
  server = {kind, folder, child, log: () => log, command: text => child.stdin.write(text + '\n')};
  await until(() => log.includes(kind === 'java' ? 'Done (' : '"event":"listening"'), 'startup', 180000);
}
async function stop() {
  if (!server) return;
  const current = server;
  if (current.kind === 'java') current.command('stop'); else current.child.kill('SIGTERM');
  await until(() => current.child.exitCode !== null || current.child.signalCode !== null, 'normal shutdown');
  await writeFile(path.join(current.folder, `server-${Date.now()}.log`), current.log());
  server = null; assert.equal(current.child.exitCode, 0, current.log().slice(-2500));
}
let marker = 0;
async function javaCommands(commands) {
  const token = `LIST_FLOW_${++marker}`, offset = server.log().length;
  for (const text of commands) server.command(text);
  server.command('say ' + token);
  await until(() => server.log().slice(offset).includes(token), 'Java console barrier');
}
function long(value) {
  if (Array.isArray(value)) return Number(BigInt.asIntN(64, (BigInt(value[0] >>> 0) << 32n) | BigInt(value[1] >>> 0)));
  return Number(value);
}
function stopTree(packet) {
  const nodes = packet.nodes, root = nodes[packet.rootIndex];
  const first = root.children.find(id => nodes[id].extraNodeData?.name === 'stop');
  if (first === undefined) return null;
  const rows = [];
  function walk(id, names) {
    const node = nodes[id], extra = node.extraNodeData;
    const here = [...names, extra.name];
    rows.push({path: here.join(' '), executable: !!node.flags.has_command,
      parser: extra.parser || null, properties: extra.properties || null});
    for (const child of node.children) walk(child, here);
  }
  walk(first, []); return rows.sort((a, b) => a.path.localeCompare(b.path));
}
function red(component) {
  if (Array.isArray(component)) return component.some(red);
  return component && typeof component === 'object' &&
    (component.color === 'red' || Object.values(component).some(red));
}
async function connect(port, username = "StopProbe") {
  const client = mc.createClient({host: '127.0.0.1', port, username, auth: 'offline',
    version: '1.21', keepAlive: false, hideErrors: true});
  let error, ready = false, closing = false, ended = false, tree, age, day;
  const messages = [], completions = [], scores = [], inventories = [], terminal = [];
  client.on('error', value => {error = value;});
  client.on('end', () => {ended = true; if (!closing) error = Error('Unexpected disconnect');});
  client.on('kick_disconnect', () => {if(!closing)error = Error('Probe kicked');});
  client.on('keep_alive', packet => client.write('keep_alive', packet));
  client.on('chunk_batch_finished', () => client.write('chunk_batch_received', {chunksPerTick: 16}));
  client.on('position', packet => {client.write('teleport_confirm', {teleportId: packet.teleportId}); ready = true;});
  client.on('declare_commands', packet => {tree = stopTree(packet);});
  client.on('update_time', packet => {age = long(packet.age); day = long(packet.time);});
  client.on('tab_complete', packet => completions.push(packet));
  client.on('scoreboard_score', packet => scores.push(packet));
  client.on('window_items', packet => inventories.push(packet));
  client.on('packet', (packet, meta, buffer) => {
    if(meta.name==='system_chat'){
      const row={component:nbt.simplify(packet.content),hex:buffer.subarray(1).toString('hex')};messages.push(row);
      if(hasKey(row.component,'commands.stop.stopping'))terminal.push({kind:'feedback',...row});
    }
    if(meta.name==='kick_disconnect')terminal.push({kind:'disconnect',component:nbt.simplify(packet.reason),hex:buffer.subarray(1).toString('hex')});
  });
  async function wait(test, label) {await until(() => {if (error) throw error; return test();}, label);}
  await wait(() => ready && tree !== undefined, 'PLAY admission');
  return {client, messages, scores, inventories, terminal, expectStop(){closing=true;}, get ended(){return ended;}, get tree() {return tree;}, get age() {return age;}, get day() {return day;},
    async command(text, key) {
      const offset = messages.length; client.write('chat_command', {command: text});
      await wait(() => messages.slice(offset).some(value => value.component.translate === key), text);
      return messages.slice(offset).find(value => value.component.translate === key);
    },
    async denied(text) {
      const offset = messages.length; client.write('chat_command', {command: text});
      await wait(() => messages.slice(offset).some(value => red(value.component)), 'denied ' + text);
    },
    async complete(text, transactionId) {
      client.write('tab_complete', {transactionId, text});
      await wait(() => completions.some(value => value.transactionId === transactionId), 'completion ' + text);
      const packet = completions.find(value => value.transactionId === transactionId);
      return {start: packet.start, length: packet.length, matches: packet.matches.map(value => value.match)};
    },
    wait, async close() {closing = true; client.end(); await until(() => ended, 'client disconnect', 10000);}
  };
}

function hasKey(component,key) {
  return component&&typeof component==='object'&&(component.translate===key||Object.values(component).some(v=>hasKey(v,key)));
}
async function keys(folder){await writeFile(path.join(folder,'keys.json'),'{"profilePropertyKeys":[],"playerCertificateKeys":[]}');}
function offlineUuid(name){const b=createHash('md5').update('OfflinePlayer:'+name).digest();b[6]=(b[6]&15)|48;b[8]=(b[8]&63)|128;const h=b.toString('hex');return `${h.slice(0,8)}-${h.slice(8,12)}-${h.slice(12,16)}-${h.slice(16,20)}-${h.slice(20)}`;}
async function finishStop(clients){
  const current=server,deadline=Date.now()+30000;
  while(!clients.every(p=>p.ended)||current.child.exitCode===null){
    if(Date.now()>deadline)throw Error('shutdown did not finish: '+current.log().slice(-2500));await pause(20);
  }
  assert.equal(current.child.exitCode,0,current.log().slice(-2500));
  await writeFile(path.join(current.folder,`server-${Date.now()}.log`),current.log());server=null;
  for(const p of clients){
    assert.equal(p.terminal.at(-1)?.kind,'disconnect');assert.ok(hasKey(p.terminal.at(-1).component,'multiplayer.disconnect.server_shutdown'));
  }
}
try{
  for(const kind of ['java','native']){
    const folder=path.join(scratch,kind);await mkdir(folder);await keys(folder);await copyFile(eulaArg,path.join(folder,'eula.txt'));
    await writeFile(path.join(folder,'server.properties'),'server-ip=127.0.0.1\nserver-port=25701\nonline-mode=false\nenforce-secure-profile=false\ngamemode=creative\nlevel-seed=0\nlevel-type=minecraft:flat\ngenerate-structures=false\nview-distance=2\nsimulation-distance=2\n');
    const port=kind==='java'?25701:25702;await start(kind,folder,port,false);
    const probe=await connect(port);assert.equal(probe.tree,null);await probe.denied('stop');
    server.command('op StopProbe');await probe.wait(()=>probe.tree!==null,'operator stop graph');
    const observer=await connect(port,'StopOther');server.command('op StopOther');await observer.wait(()=>observer.tree!==null,'observer graph');
    const trace={tree:probe.tree,denied_nonop:true,invalid_no_halt:true,completions:[]};
    for(const text of ['stop extra','stop  extra','STOP'])await probe.denied(text);
    for(const [i,text]of ['stop','stop ','/st','s'].entries()){
      const c=await probe.complete(text,i+1);if(text==='s')c.matches=c.matches.filter(v=>['save-all','seed','setidletimeout','stop'].includes(v));if(text==='/st'||text==='stop'){c.matches=c.matches.filter(v=>v==='stop');if(!c.matches.length){c.start=0;c.length=0;}}trace.completions.push({text,...c});
    }
    probe.client.write('set_creative_slot',{slot:36,item:{itemCount:17,itemId:1,addedComponentCount:0,removedComponentCount:0,components:[],removeComponents:[]}});
    await probe.command('time set 12345','commands.time.set');
    probe.expectStop();observer.expectStop();probe.client.write('chat_command',{command:'stop'});
    await finishStop([probe,observer]);
    assert.equal(probe.terminal[0].kind,'feedback');assert.equal(observer.terminal[0].kind,'feedback');
    trace.player_stop={source:probe.terminal,observer:observer.terminal};
    await start(kind,folder,port,false);const restored=await connect(port);
    await restored.wait(()=>restored.inventories.some(p=>p.windowId===0),'restored inventory');
    const item=restored.inventories.find(p=>p.windowId===0).items[36];assert.equal(item.itemCount,17);assert.equal(item.itemId,1);
    const time=await restored.command('time query daytime','commands.time.query');const day=Number(time.component.with[0]);assert.ok(day>=12345&&day<12545,'world time not saved');trace.restart_world_inventory=true;
    const again=await connect(port,'StopOther');restored.expectStop();again.expectStop();server.command('/stop');
    const current=server;await finishStop([restored,again]);
    assert.ok(current.log().includes(kind==='java'?'Stopping the server':'commands.stop.stopping'));
    trace.console_stop={source:restored.terminal,observer:again.terminal};result.traces[kind]=trace;
  }
  assert.deepEqual(result.traces.native,result.traces.java,'selected Java/native stop behavior differs');
  const folder=path.join(scratch,'native');await start('native',folder,25702,false);const signal=await connect(25702);signal.expectStop();server.child.kill('SIGTERM');await finishStop([signal]);result.native_sigterm={reason:signal.terminal.at(-1),normal_exit:true};result.ok=true;
}catch(error){result.error=error.stack;process.exitCode=1;}
finally{try{await stop();}catch(error){result.shutdown_error=error.message;result.ok=false;process.exitCode=1;}result.completed_at=new Date().toISOString();await writeFile(output,JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify({ok:result.ok,report:output,local_diagnostics:scratch,error:result.error}));}
