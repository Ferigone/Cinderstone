// SPDX-License-Identifier: GPL-2.0-only
// Real, unchanged Java 1.21 and native Main: explicit saves, ordered feedback and crash recovery.
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
const scratch = await mkdtemp(path.join(tmpdir(), 'save-all-command-'));
const sha = (bytes, kind = 'sha256') => createHash(kind).update(bytes).digest('hex');
const lock = JSON.parse(await readFile('fixtures/source-lock.json', 'utf8')).oracle;
const jarBytes = await readFile(jar);
assert.equal(sha(jarBytes, 'sha1'), lock.sha1); assert.equal(jarBytes.length, lock.size);
assert.ok((await readFile(eulaArg, 'utf8')).split(/\r?\n/).includes('eula=true'));
const result = {ok: false, native_commit: execFileSync('git', ['rev-parse', 'HEAD'], {encoding: 'utf8'}).trim(),
  native_src_tree: execFileSync('git', ['rev-parse', 'HEAD:src'], {encoding: 'utf8'}).trim(),
  native_binary_sha256: sha(await readFile(binary)), java_jar_sha1: lock.sha1, traces: {},
  limits: ['Supported single-level native creative profile; native synchronous checkpoint writes implement both modes.', 'Selected command packets/tree/completions and explicit-save recovery are compared; filesystem scheduling and syntax-error rendering are not.']};
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
function saveTree(packet) {
  const nodes = packet.nodes, root = nodes[packet.rootIndex];
  const first = root.children.find(id => nodes[id].extraNodeData?.name === 'save-all');
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
async function connect(port, username = "SaveProbe") {
  const client = mc.createClient({host: '127.0.0.1', port, username, auth: 'offline',
    version: '1.21', keepAlive: false, hideErrors: true});
  let error, ready = false, closing = false, ended = false, tree, age, day;
  const messages = [], completions = [], scores = [], inventories = [];
  client.on('error', value => {error = value;});
  client.on('end', () => {ended = true; if (!closing) error = Error('Unexpected disconnect');});
  client.on('kick_disconnect', () => {error = Error('Probe kicked');});
  client.on('keep_alive', packet => client.write('keep_alive', packet));
  client.on('chunk_batch_finished', () => client.write('chunk_batch_received', {chunksPerTick: 16}));
  client.on('position', packet => {client.write('teleport_confirm', {teleportId: packet.teleportId}); ready = true;});
  client.on('declare_commands', packet => {tree = saveTree(packet);});
  client.on('update_time', packet => {age = long(packet.age); day = long(packet.time);});
  client.on('tab_complete', packet => completions.push(packet));
  client.on('scoreboard_score', packet => scores.push(packet));
  client.on('window_items', packet => inventories.push(packet));
  client.on('packet', (packet, meta, buffer) => {
    if (meta.name === 'system_chat') messages.push({component: nbt.simplify(packet.content), hex: buffer.subarray(1).toString('hex')});
  });
  async function wait(test, label) {await until(() => {if (error) throw error; return test();}, label);}
  await wait(() => ready && tree !== undefined, 'PLAY admission');
  return {client, messages, scores, inventories, get tree() {return tree;}, get age() {return age;}, get day() {return day;},
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
async function consoleSave(flush){
  const offset=server.log().length;server.command('/save-all'+(flush?' flush':''));
  await until(()=>server.log().slice(offset).includes(server.kind==='java'?'Saved the game':'commands.save.success'),'console save completion');
}
async function crash(){
  const current=server;current.child.kill('SIGKILL');
  await until(()=>current.child.signalCode!==null,'hard stop');
  await writeFile(path.join(current.folder,'crash.log'),current.log());server=null;
}
try {
  const prepared=path.join(scratch,'prepared');await mkdir(prepared);await copyFile(eulaArg,path.join(prepared,'eula.txt'));
  await writeFile(path.join(prepared,'server.properties'),'server-ip=127.0.0.1\nserver-port=25641\nonline-mode=false\nenforce-secure-profile=false\n'
    +'gamemode=creative\nlevel-seed=0\nlevel-type=minecraft:flat\ngenerate-structures=false\nview-distance=2\nsimulation-distance=2\n');
  await start('java',prepared,25641);
  await javaCommands(['gamerule doDaylightCycle false','gamerule doWeatherCycle false','gamerule doMobSpawning false',
    'gamerule randomTickSpeed 0','setworldspawn 8 65 8','forceload add -80 -80 80 80','save-all flush']);await stop();
  const witnesses=JSON.parse(await readFile('fixtures/save-all-command-vectors.json','utf8')).execution;
  for(const kind of ['java','native']){
    const folder=path.join(scratch,kind);await mkdir(folder);await keys(folder);
    if(kind==='java'){
      await cp(path.join(prepared,'world'),path.join(folder,'world'),{recursive:true});
      await copyFile(path.join(prepared,'server.properties'),path.join(folder,'server.properties'));
      await copyFile(eulaArg,path.join(folder,'eula.txt'));
    }
    const port=kind==='java'?25641:25642;await start(kind,folder,port,kind==='java');
    const probe=await connect(port);assert.equal(probe.tree,null);await probe.denied('save-all');
    server.command('op SaveProbe');await probe.wait(()=>probe.tree!==null,'operator tree');
    const other=await connect(port,'SaveOther');server.command('op SaveOther');await other.wait(()=>other.tree!==null,'observer tree');
    const trace={tree:probe.tree,feedback:[],operator_feedback:[],completions:[],permission_denied:true};
    await probe.denied('save-all extra');await probe.denied('save-all flush extra');trace.invalid_denied=true;
    if(kind==='native'){
      const blocked=path.join(folder,'world.json');await mkdir(blocked);
      const offset=probe.messages.length;probe.client.write('chat_command',{command:'save-all'});
      await probe.wait(()=>probe.messages.slice(offset).some(m=>hasKey(m.component,'commands.save.failed')),'failed save');
      const messages=probe.messages.slice(offset);
      assert.ok(hasKey(messages[0].component,'commands.save.saving'));
      assert.ok(!messages.some(m=>hasKey(m.component,'commands.save.success')));
      result.native_io_failure={saving_then_failed:true,no_success:true};await rmdir(blocked);
    }
    for(const [index,command] of ['save-all','save-all flush'].entries()){
      const inventoryCount=23+index, day=123456+index*1000;
      probe.client.write('set_creative_slot',{slot:36,item:{itemCount:inventoryCount,itemId:1,addedComponentCount:0,removedComponentCount:0,components:[],removeComponents:[]}});
      await probe.command('time set '+day,'commands.time.set');
      const offset=probe.messages.length,otherOffset=other.messages.length;
      await probe.command(command,'commands.save.success');
      const messages=probe.messages.slice(offset).filter(m=>hasKey(m.component,'commands.save.saving')||hasKey(m.component,'commands.save.success'));
      assert.deepEqual(messages.map(m=>m.hex),witnesses.find(w=>w.input===command&&w.ok).feedback.map(m=>m.hex));
      trace.feedback.push({command,hex:messages.map(m=>m.hex)});
      await other.wait(()=>other.messages.slice(otherOffset).some(m=>hasKey(m.component,'commands.save.success')),'observer saved feedback');
      assert.ok(!other.messages.slice(otherOffset).some(m=>hasKey(m.component,'commands.save.saving')),'saving announcement leaked');
      trace.operator_feedback.push(other.messages.slice(otherOffset).find(m=>hasKey(m.component,'commands.save.success')).hex);
      if(kind==='java'){
        const data=nbt.simplify((await nbt.parse(await readFile(path.join(folder,'world/level.dat')))).parsed).Data;
        assert.equal(long(data.DayTime),day);
        const player=nbt.simplify((await nbt.parse(await readFile(path.join(folder,'world/playerdata',offlineUuid('SaveProbe')+'.dat')))).parsed);
        assert.ok(player.Inventory.some(item=>item.Slot===0&&item.id==='minecraft:stone'&&Number(item.count??item.Count)===inventoryCount));
      }else{
        const data=JSON.parse(await readFile(path.join(folder,'world.json'),'utf8'));assert.ok(data.clock.day_time>=day&&data.clock.day_time<day+100);
        const player=JSON.parse(await readFile(path.join(folder,'world.players',offlineUuid('SaveProbe')+'.native.json'),'utf8'));
        const item=Buffer.from(player.slots[36],'base64');assert.equal(item[0],inventoryCount);assert.equal(item[1],1);
      }
    }
    trace.world_player_saved_before_ack=true;
    let transaction=1;for(const text of ['/save-a','save-all','save-all ','save-all f','save-all flush '])
      trace.completions.push({text,...await probe.complete(text,transaction++)});
    other.client.write('settings',{locale:'en_US',viewDistance:2,chatFlags:2,chatColors:true,skinParts:127,mainHand:1,enableTextFiltering:false,enableServerListing:true});
    await other.complete("save-all f",999);const hiddenOffset=other.messages.length;
    await consoleSave(false);await consoleSave(true);await pause(150);
    assert.ok(!other.messages.slice(hiddenOffset).some(m=>hasKey(m.component,'commands.save.success')),'hidden observer received system feedback');
    trace.hidden_feedback_suppressed=true;trace.console=true;
    await crash();
    await start(kind,folder,port,kind==='java');const restored=await connect(port);
    await restored.wait(()=>restored.inventories.some(packet=>packet.windowId===0),'restored inventory');
    const item=restored.inventories.find(packet=>packet.windowId===0).items[36];assert.equal(item.itemCount,24);assert.equal(item.itemId,1);
    const time=await restored.command('time query daytime','commands.time.query');const value=Number(time.component.with[0]);
    assert.ok(value>=124456%24000&&value<(124456%24000)+100,'saved time was not recovered');
    trace.hard_restart_world_player=true;
    await restored.close();await stop();result.traces[kind]=trace;
  }
  assert.deepEqual(result.traces.native,result.traces.java,'selected Java/native explicit-save behavior differs');result.ok=true;
}catch(error){result.error=error.stack;process.exitCode=1;}
finally{
  try{await stop();}catch(error){result.shutdown_error=error.message;result.ok=false;process.exitCode=1;}
  result.completed_at=new Date().toISOString();await writeFile(output,JSON.stringify(result,null,2)+'\n');
  console.log(JSON.stringify({ok:result.ok,report:output,local_diagnostics:scratch,error:result.error}));
}
