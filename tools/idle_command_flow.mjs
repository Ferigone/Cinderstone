// SPDX-License-Identifier: GPL-2.0-only
// Real, unchanged Java 1.21 and native Main: idle command, action clock and real idle disconnect.
import mc from 'minecraft-protocol';
import nbt from 'prismarine-nbt';
import assert from 'node:assert/strict';
import {spawn, execFileSync} from 'node:child_process';
import {mkdtemp, mkdir, readFile, writeFile, copyFile, cp} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import path from 'node:path';
import {createHash} from 'node:crypto';

const [binaryArg, jarArg, eulaArg, output] = process.argv.slice(2);
assert.ok(output, 'BINARY PINNED_JAR EXISTING_EULA REPORT');
const root = process.cwd(), binary = path.resolve(binaryArg), jar = path.resolve(jarArg);
const scratch = await mkdtemp(path.join(tmpdir(), 'idle-command-'));
const sha = (bytes, kind = 'sha256') => createHash(kind).update(bytes).digest('hex');
const lock = JSON.parse(await readFile('fixtures/source-lock.json', 'utf8')).oracle;
const jarBytes = await readFile(jar);
assert.equal(sha(jarBytes, 'sha1'), lock.sha1); assert.equal(jarBytes.length, lock.size);
assert.ok((await readFile(eulaArg, 'utf8')).split(/\r?\n/).includes('eula=true'));
const result = {ok: false, native_commit: execFileSync('git', ['rev-parse', 'HEAD'], {encoding: 'utf8'}).trim(),
  native_src_tree: execFileSync('git', ['rev-parse', 'HEAD:src'], {encoding: 'utf8'}).trim(),
  native_binary_sha256: sha(await readFile(binary)), java_jar_sha1: lock.sha1, traces: {},
  limits: ['Native creative supported actions; unsupported entity/sign/recipe actions remain outside this profile.', 'Real elapsed monotonic time, original component bytes and disconnect reasons; syntax-error rendering and tick-exact latency are not compared.']};
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
function idleTree(packet) {
  const nodes = packet.nodes, root = nodes[packet.rootIndex];
  const first = root.children.find(id => nodes[id].extraNodeData?.name === 'setidletimeout');
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
async function connect(port, username = "IdleProbe") {
  const client = mc.createClient({host: '127.0.0.1', port, username, auth: 'offline',
    version: '1.21', keepAlive: false, hideErrors: true});
  let error, ready = false, closing = false, ended = false, tree, age, day, kick;
  const messages = [], completions = [], scores = [];
  client.on('error', value => {error = value;});
  client.on('end', () => {ended = true; if (!closing) error = Error('Unexpected disconnect');});
  client.on('kick_disconnect', packet => {kick=nbt.simplify(packet.reason);closing=true;});
  client.on('keep_alive', packet => client.write('keep_alive', packet));
  client.on('chunk_batch_finished', () => client.write('chunk_batch_received', {chunksPerTick: 16}));
  client.on('position', packet => {client.write('teleport_confirm', {teleportId: packet.teleportId}); ready = true;});
  client.on('declare_commands', packet => {tree = idleTree(packet);});
  client.on('update_time', packet => {age = long(packet.age); day = long(packet.time);});
  client.on('tab_complete', packet => completions.push(packet));
  client.on('scoreboard_score', packet => scores.push(packet));
  client.on('packet', (packet, meta, buffer) => {
    if (meta.name === 'system_chat') messages.push({component: nbt.simplify(packet.content), hex: buffer.subarray(1).toString('hex')});
  });
  async function wait(test, label) {await until(() => {if (error) throw error; return test();}, label);}
  await wait(() => ready && tree !== undefined, 'PLAY admission');
  return {client, messages, scores, get kick(){return kick;}, get ended(){return ended;}, get tree() {return tree;}, get age() {return age;}, get day() {return day;},
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

async function keys(folder) {await writeFile(path.join(folder,'keys.json'),'{"profilePropertyKeys":[],"playerCertificateKeys":[]}');}
function property(bytes) {
  const values = [...bytes.matchAll(/^player-idle-timeout\s*[=:]\s*(-?\d+)\s*$/gm)];
  return values.length ? Number(values.at(-1)[1]) : 0;
}
async function consoleIdle(minutes) {
  const offset=server.log().length;server.command('/setidletimeout '+minutes);
  await until(()=>server.log().slice(offset).includes(server.kind==='java'?'Set the idle kick timer':'commands.setidletimeout.success'),'console idle');
}
try {
  const prepared=path.join(scratch,'prepared');await mkdir(prepared);await copyFile(eulaArg,path.join(prepared,'eula.txt'));
  await writeFile(path.join(prepared,'server.properties'),'server-ip=127.0.0.1\nserver-port=25641\nonline-mode=false\nenforce-secure-profile=false\n'
    +'gamemode=creative\nlevel-seed=0\nlevel-type=minecraft:flat\ngenerate-structures=false\nview-distance=2\nsimulation-distance=2\nplayer-idle-timeout=0\nunknown.setting=keep-me\n');
  await start('java',prepared,25641);
  await javaCommands(['gamerule doDaylightCycle false','gamerule doWeatherCycle false','gamerule doMobSpawning false',
    'gamerule randomTickSpeed 0','setworldspawn 8 65 8','forceload add -80 -80 80 80','save-all flush']);
  await stop();
  const witnesses=JSON.parse(await readFile('fixtures/idle-command-vectors.json','utf8')).execution;
  for(const kind of ['java','native']) {
    const folder=path.join(scratch,kind);await mkdir(folder);
    await cp(path.join(prepared,'world'),path.join(folder,'world'),{recursive:true});
    await copyFile(path.join(prepared,'server.properties'),path.join(folder,'server.properties'));
    await copyFile(eulaArg,path.join(folder,'eula.txt'));await keys(folder);
    const port=kind==='java'?25641:25642;await start(kind,folder,port);
    const probe=await connect(port);assert.equal(probe.tree,null);await probe.denied('setidletimeout 1');
    assert.equal(property(await readFile(path.join(folder,'server.properties'),'utf8')),0);
    server.command('op IdleProbe');await probe.wait(()=>probe.tree!==null,'operator tree');
    const idle=await connect(port,'IdleOther');server.command('op IdleOther');
    await idle.wait(()=>idle.tree!==null,'other operator tree');
    const active=await connect(port,'IdleActive');
    const propertyPath=path.join(folder,'server.properties');
    const trace={tree:probe.tree,feedback:[],completions:[],permission_denied:true};
    const adminOffset=idle.messages.length;
    for(const minutes of [1,0]) {
      const message=await probe.command('setidletimeout '+minutes,'commands.setidletimeout.success');
      assert.equal(message.hex,witnesses.find(w=>w.input==='setidletimeout '+minutes&&w.ok).feedback_hex);
      trace.feedback.push({minutes,hex:message.hex});
    }
    await idle.wait(()=>idle.messages.slice(adminOffset).filter(m=>m.component.translate==='chat.type.admin').length>=2,'operator feedback');
    trace.operator_feedback=idle.messages.slice(adminOffset).filter(m=>m.component.translate==='chat.type.admin').slice(0,2).map(m=>m.hex);
    const before=await readFile(propertyPath,'utf8');
    await probe.denied('setidletimeout -1');await probe.denied('setidletimeout 1 extra');
    assert.equal(await readFile(propertyPath,'utf8'),before);trace.invalid_unchanged=true;
    let transaction=1;for(const text of ['/seti','setidletimeout','setidletimeout ','setidletimeout 1 '])
      trace.completions.push({text,...await probe.complete(text,transaction++)});
    // No forged runtime clock. Keep-alive and movement are deliberately not activity.
    let slot=0;const swing=setInterval(()=>{
      active.client.write('arm_animation',{hand:0});
      active.client.write('held_item_slot',{slotId:(slot++%2)});
      idle.client.write('flying',{onGround:true});
    },3000);
    try {
      const began=Date.now();await until(()=>Date.now()-began>=61500,'real zero-disabled interval',65000);
      assert.ok(!probe.ended&&!idle.ended&&!active.ended,'timeout0 kicked a client');
      trace.zero_disabled_after_minute=true;
      await consoleIdle(1);
      await until(()=>probe.ended&&idle.ended,'actual idle disconnect',6000);
      assert.deepEqual(probe.kick,{translate:'multiplayer.disconnect.idling'});
      assert.deepEqual(idle.kick,probe.kick);assert.ok(!active.ended,'active swing/hotbar player kicked');
      trace.idle_operator_reason=probe.kick;trace.movement_not_activity=true;trace.active_player_remains=true;
    } finally {clearInterval(swing);}
    await active.close();await stop();
    const persisted=await readFile(propertyPath,'utf8');assert.equal(property(persisted),1);
    assert.ok(persisted.includes('unknown.setting=keep-me'));trace.persisted=true;
    await start(kind,folder,port);const rejoined=await connect(port);
    assert.ok(rejoined.tree); // Existing op identity and setting both survive startup.
    assert.equal(property(await readFile(propertyPath,'utf8')),1);
    await consoleIdle(0);await rejoined.close();await stop();
    assert.equal(property(await readFile(propertyPath,'utf8')),0);trace.restart=true;
    result.traces[kind]=trace;
  }
  assert.deepEqual(result.traces.native,result.traces.java,'selected Java/native idle behavior differs');result.ok=true;
} catch(error){result.error=error.stack;process.exitCode=1;}
finally {
  try{await stop();}catch(error){result.shutdown_error=error.message;result.ok=false;process.exitCode=1;}
  result.completed_at=new Date().toISOString();await writeFile(output,JSON.stringify(result,null,2)+'\n');
  console.log(JSON.stringify({ok:result.ok,report:output,local_diagnostics:scratch,error:result.error}));
}
