// SPDX-License-Identifier: GPL-2.0-only
// Real, unchanged Java 1.21 and native Main: pardon-ip grammar, actual ban removal and restored admission.
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
const scratch = await mkdtemp(path.join(tmpdir(), 'pardon-ip-command-'));
const sha = (bytes, kind = 'sha256') => createHash(kind).update(bytes).digest('hex');
const lock = JSON.parse(await readFile('fixtures/source-lock.json', 'utf8')).oracle;
const jarBytes = await readFile(jar);
assert.equal(sha(jarBytes, 'sha1'), lock.sha1); assert.equal(jarBytes.length, lock.size);
assert.ok((await readFile(eulaArg, 'utf8')).split(/\r?\n/).includes('eula=true'));
const result = {ok: false, native_commit: execFileSync('git', ['rev-parse', 'HEAD'], {encoding: 'utf8'}).trim(),
  native_src_tree: execFileSync('git', ['rev-parse', 'HEAD:src'], {encoding: 'utf8'}).trim(),
  native_binary_sha256: sha(await readFile(binary)), java_jar_sha1: lock.sha1, traces: {},
  limits: ['Existing access-list host and its retained-entry behavior; no general date/locale/HashMap or profile-cache parity claim.', 'Selected command/component bytes, live-key completions and real admission are compared; syntax-error rendering and packet timing are not.']};
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
function pardonTree(packet) {
  const nodes = packet.nodes, root = nodes[packet.rootIndex];
  const first = root.children.find(id => nodes[id].extraNodeData?.name === 'pardon-ip');
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
async function connect(port, username = "PardonProbe") {
  const client = mc.createClient({host: '127.0.0.1', port, username, auth: 'offline',
    version: '1.21', keepAlive: false, hideErrors: true});
  let error, ready = false, closing = false, ended = false, tree, age, day;
  const messages = [], completions = [], scores = [];
  client.on('error', value => {error = value;});
  client.on('end', () => {ended = true; if (!closing) error = Error('Unexpected disconnect');});
  client.on('kick_disconnect', () => {error = Error('Probe kicked');});
  client.on('keep_alive', packet => client.write('keep_alive', packet));
  client.on('chunk_batch_finished', () => client.write('chunk_batch_received', {chunksPerTick: 16}));
  client.on('position', packet => {client.write('teleport_confirm', {teleportId: packet.teleportId}); ready = true;});
  client.on('declare_commands', packet => {tree = pardonTree(packet);});
  client.on('update_time', packet => {age = long(packet.age); day = long(packet.time);});
  client.on('tab_complete', packet => completions.push(packet));
  client.on('scoreboard_score', packet => scores.push(packet));
  client.on('packet', (packet, meta, buffer) => {
    if (meta.name === 'system_chat') messages.push({component: nbt.simplify(packet.content), hex: buffer.subarray(1).toString('hex')});
  });
  async function wait(test, label) {await until(() => {if (error) throw error; return test();}, label);}
  await wait(() => ready && tree !== undefined, 'PLAY admission');
  return {client, messages, scores, get ended(){return ended;}, get tree() {return tree;}, get age() {return age;}, get day() {return day;},
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

function hasKey(c,key){return c&&typeof c==='object'&&(c.translate===key||Object.values(c).some(v=>hasKey(v,key)));}
async function execute(probe,text,key){
  const offset=probe.messages.length;probe.client.write('chat_command',{command:text});
  await probe.wait(()=>probe.messages.slice(offset).some(m=>hasKey(m.component,key)),text);
  return probe.messages.slice(offset).find(m=>hasKey(m.component,key));
}
async function keys(folder){await writeFile(path.join(folder,'keys.json'),'{"profilePropertyKeys":[],"playerCertificateKeys":[]}');}
async function retained(folder){return JSON.parse(await readFile(path.join(folder,'banned-ips.json'),'utf8')).map(row=>row.ip).sort();}
async function consolePardon(ip){
  if(server.kind==='java'){await javaCommands(['pardon-ip '+ip]);return;}
  const offset=server.log().length;server.command('/pardon-ip '+ip);
  await until(()=>{const log=server.log().slice(offset);return log.includes('commands.pardonip.success')&&log.includes(ip);},'console pardon');
}
async function blockedLogin(port){
  const client=mc.createClient({host:'127.0.0.1',port,username:'PardonBlocked',auth:'offline',version:'1.21',keepAlive:false,hideErrors:true});
  let reason,ended=false,play=false;
  client.on('error',()=>{});client.on('end',()=>{ended=true;});client.on('login',()=>{play=true;});
  client.on('disconnect',packet=>{reason=typeof packet.reason==='string'?JSON.parse(packet.reason):nbt.simplify(packet.reason);});
  client.on('kick_disconnect',packet=>{reason=typeof packet.reason==='string'?JSON.parse(packet.reason):nbt.simplify(packet.reason);});
  await until(()=>ended&&reason,'IP-banned login');assert.ok(!play,'banned connection reached PLAY');
  assert.ok(reason.translate?.includes('banned_ip'),JSON.stringify(reason));return reason.translate;
}
const initial=['192.0.2.7','198.51.100.9','203.0.113.15'].map(ip=>({ip,created:'2020-01-01 00:00:00 +0000',source:'Console',reason:'fixture',expires:ip==='198.51.100.9'?'2020-01-01 00:00:01 +0000':'forever'}));
try{
  const vectors=JSON.parse(await readFile('fixtures/pardon-ip-command-vectors.json','utf8')).execution;
  for(const kind of ['java','native']){
    const folder=path.join(scratch,kind);await mkdir(folder);await keys(folder);await copyFile(eulaArg,path.join(folder,'eula.txt'));
    await writeFile(path.join(folder,'server.properties'),'server-ip=127.0.0.1\nserver-port=25641\nonline-mode=false\nenforce-secure-profile=false\ngamemode=creative\nlevel-type=minecraft:flat\ngenerate-structures=false\nview-distance=2\nsimulation-distance=2\n');
    await writeFile(path.join(folder,'banned-ips.json'),JSON.stringify(initial));
    const port=kind==='java'?25641:25642;await start(kind,folder,port,false);
    const probe=await connect(port);assert.equal(probe.tree,null);
    const before=await readFile(path.join(folder,'banned-ips.json'));
    await probe.denied('pardon-ip 192.0.2.7');assert.deepEqual(await readFile(path.join(folder,'banned-ips.json')),before);
    server.command('op PardonProbe');await probe.wait(()=>probe.tree!==null,'operator tree');
    const other=await connect(port,'PardonOther');server.command('op PardonOther');await other.wait(()=>other.tree!==null,'observer operator tree');
    const trace={tree:probe.tree,nonop_denied_without_write:true,feedback:[],operator_feedback:[],completions:[],remaining:[]};
    let transaction=1;
    for(const text of ['/pardon-i','pardon-ip ','pardon-ip 2','pardon-ip 19'])trace.completions.push({text,...await probe.complete(text,transaction++)});
    const observerOffset=other.messages.length;
    for(const [text,key]of [['pardon-ip 999.1.1.1','commands.pardonip.invalid'],['pardon-ip 203.0.113.8','commands.pardonip.failed']]){
      const reply=await execute(probe,text,key);assert.equal(vectors.find(row=>row.input===text).error_key,key);trace.feedback.push({text,hex:reply.hex});
    }
    await probe.denied('pardon-ip 192.0.2.7 extra');await probe.denied('pardon-ip ::1');
    assert.deepEqual(await readFile(path.join(folder,'banned-ips.json')),before);
    await other.complete('pardon-ip ',998);
    assert.ok(!other.messages.slice(observerOffset).some(m=>hasKey(m.component,'commands.pardonip.invalid')||hasKey(m.component,'commands.pardonip.failed')));
    trace.failures_private_without_write=true;
    for(const ip of ['198.51.100.9','192.0.2.7']){
      const offset=other.messages.length,text='pardon-ip '+ip;
      const reply=await execute(probe,text,'commands.pardonip.success');
      assert.equal(reply.hex,vectors.find(row=>row.input===text).feedback_hex[0]);trace.feedback.push({text,hex:reply.hex});
      await other.wait(()=>other.messages.slice(offset).some(m=>hasKey(m.component,'commands.pardonip.success')),'operator unban feedback');
      trace.operator_feedback.push(other.messages.slice(offset).find(m=>hasKey(m.component,'commands.pardonip.success')).hex);
      const remaining=await retained(folder);assert.ok(!remaining.includes(ip));trace.remaining.push(remaining);
      trace.completions.push({text:'pardon-ip ',...await probe.complete('pardon-ip ',transaction++)});
    }
    trace.removed_retained_expired=true;
    other.client.write('settings',{locale:'en_US',viewDistance:2,chatFlags:2,chatColors:true,skinParts:127,mainHand:1,enableTextFiltering:false,enableServerListing:true});
    await other.complete('pardon-ip ',999);const hiddenOffset=other.messages.length;
    await consolePardon('203.0.113.15');await pause(150);
    assert.ok(!other.messages.slice(hiddenOffset).some(m=>hasKey(m.component,'commands.pardonip.success')));trace.hidden_feedback_suppressed=true;trace.console=true;
    assert.deepEqual(await retained(folder),[]);
    server.command('ban-ip 127.0.0.1 fixture');await until(()=>probe.ended&&other.ended,'IP ban disconnected actual players');
    trace.blocked_login_reason=await blockedLogin(port);
    await consolePardon('127.0.0.1');assert.deepEqual(await retained(folder),[]);
    const allowed=await connect(port);assert.ok(allowed.tree);trace.admission_restored=true;
    await allowed.close();await stop();
    await start(kind,folder,port,false);const restarted=await connect(port);assert.ok(restarted.tree);
    const missing=await execute(restarted,'pardon-ip 192.0.2.7','commands.pardonip.failed');
    trace.restart_failure_hex=missing.hex;
    assert.deepEqual((await restarted.complete('pardon-ip ',1000)).matches,[]);assert.deepEqual(await retained(folder),[]);trace.restart_persisted=true;
    await restarted.close();await stop();result.traces[kind]=trace;
  }
  assert.deepEqual(result.traces.native,result.traces.java,'selected Java/native pardon-ip behavior differs');result.ok=true;
}catch(error){result.error=error.stack;process.exitCode=1;}
finally{
  try{await stop();}catch(error){result.shutdown_error=error.message;result.ok=false;process.exitCode=1;}
  result.completed_at=new Date().toISOString();await writeFile(output,JSON.stringify(result,null,2)+'\n');
  console.log(JSON.stringify({ok:result.ok,report:output,local_diagnostics:scratch,error:result.error}));
}
