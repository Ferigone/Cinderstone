// SPDX-License-Identifier: GPL-2.0-only
// Real, unchanged Java 1.21 and native Main: list command, actual roster/team formatting and private feedback.
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
const scratch = await mkdtemp(path.join(tmpdir(), 'list-command-'));
const sha = (bytes, kind = 'sha256') => createHash(kind).update(bytes).digest('hex');
const lock = JSON.parse(await readFile('fixtures/source-lock.json', 'utf8')).oracle;
const jarBytes = await readFile(jar);
assert.equal(sha(jarBytes, 'sha1'), lock.sha1); assert.equal(jarBytes.length, lock.size);
assert.ok((await readFile(eulaArg, 'utf8')).split(/\r?\n/).includes('eula=true'));
const result = {ok: false, native_commit: execFileSync('git', ['rev-parse', 'HEAD'], {encoding: 'utf8'}).trim(),
  native_src_tree: execFileSync('git', ['rev-parse', 'HEAD:src'], {encoding: 'utf8'}).trim(),
  native_binary_sha256: sha(await readFile(binary)), java_jar_sha1: lock.sha1, traces: {},
  limits: ['Selected command output bytes/tree/completions/roster/privacy are compared; syntax-error rendering is not compared.', 'Existing native player-capacity policy is unchanged.']};
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
    ['--max-players', '37', '--allow-offline', '--bind', '127.0.0.1', '--port', String(port), '--fixtures', path.join(root, 'fixtures/configuration.json'),
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
function listTree(packet) {
  const nodes = packet.nodes, root = nodes[packet.rootIndex];
  const first = root.children.find(id => nodes[id].extraNodeData?.name === 'list');
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
async function connect(port, username = "ZuluList") {
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
  client.on('declare_commands', packet => {tree = listTree(packet);});
  client.on('update_time', packet => {age = long(packet.age); day = long(packet.time);});
  client.on('tab_complete', packet => completions.push(packet));
  client.on('scoreboard_score', packet => scores.push(packet));
  client.on('packet', (packet, meta, buffer) => {
    if (meta.name === 'system_chat') messages.push({component: nbt.simplify(packet.content), hex: buffer.subarray(1).toString('hex')});
  });
  async function wait(test, label) {await until(() => {if (error) throw error; return test();}, label);}
  await wait(() => ready && tree !== undefined, 'PLAY admission');
  return {client, messages, scores, get tree() {return tree;}, get age() {return age;}, get day() {return day;},
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

function hasList(component) {
  return component && typeof component === 'object' &&
    (component.translate === 'commands.list.players' || Object.values(component).some(hasList));
}
async function keys(folder) {
  await writeFile(path.join(folder, 'keys.json'), '{"profilePropertyKeys":[],"playerCertificateKeys":[]}');
}
async function consoleList(count, uuids = false) {
  const offset = server.log().length;
  server.command('/list' + (uuids ? ' uuids' : ''));
  await until(() => {
    const log = server.log().slice(offset);
    if (server.kind === 'java') return log.includes(`There are ${count} of a max of 37 players online:`);
    return log.split('\n').some(line => {try {
      const entry = JSON.parse(line);
      return entry.event === 'administration_feedback' && entry.result === count &&
        entry.component.translate === 'commands.list.players' && !entry.error;
    } catch {return false;}});
  }, 'console list ' + count);
}
async function teamSetup() {
  const commands = ['team add test', 'team modify test color red',
    'team modify test prefix {"text":"[T] "}', 'team modify test suffix {"text":"!"}', 'team join test ZuluList'];
  if (server.kind === 'java') await javaCommands(commands);
  else {
    for (const command of commands) server.command(command);
    await consoleList(2); // Serialized console commands form a barrier.
  }
}
try {
  const prepared = path.join(scratch, 'prepared'); await mkdir(prepared);
  await copyFile(eulaArg, path.join(prepared, 'eula.txt'));
  await writeFile(path.join(prepared, 'server.properties'),
    'server-ip=127.0.0.1\nserver-port=25641\nonline-mode=false\nenforce-secure-profile=false\nmax-players=37\n'
    + 'gamemode=creative\nlevel-seed=0\nlevel-type=minecraft:flat\ngenerate-structures=false\nview-distance=2\nsimulation-distance=2\n');
  await start('java', prepared, 25641);
  await javaCommands(['gamerule doDaylightCycle false', 'gamerule doWeatherCycle false',
    'gamerule doMobSpawning false', 'gamerule randomTickSpeed 0', 'setworldspawn 8 65 8',
    'forceload add -80 -80 80 80', 'save-all flush']);
  await stop();
  for (const kind of ['java', 'native']) {
    const folder = path.join(scratch, kind); await mkdir(folder);
    await cp(path.join(prepared, 'world'), path.join(folder, 'world'), {recursive:true});
    await copyFile(path.join(prepared, 'server.properties'), path.join(folder, 'server.properties'));
    await copyFile(eulaArg, path.join(folder, 'eula.txt')); await keys(folder);
    const port = kind === 'java' ? 25641 : 25642;
    await start(kind, folder, port);
    await consoleList(0); await consoleList(0, true);
    const probe = await connect(port); assert.ok(probe.tree, 'non-op must see list');
    const trace = {tree:probe.tree, feedback:[], completions:[], empty_console:true};
    for (const command of ['list', 'list uuids']) trace.feedback.push(await probe.command(command, 'commands.list.players'));
    const other = await connect(port, 'AlphaList'); server.command('op AlphaList');
    await consoleList(2); await pause(150);
    const offset = other.messages.length;
    for (const command of ['list', 'list uuids']) trace.feedback.push(await probe.command(command, 'commands.list.players'));
    await teamSetup();
    for (const command of ['list', 'list uuids']) trace.feedback.push(await probe.command(command, 'commands.list.players'));
    await probe.denied('list bogus'); await probe.denied('list uuids extra');
    let transaction = 1;
    for (const text of ['/li', '/list', '/list ', '/list u', '/list uuids ', '/list bogus'])
      trace.completions.push({text, ...await probe.complete(text, transaction++)});
    await consoleList(2); await consoleList(2,true); await pause(200);
    assert.ok(!other.messages.slice(offset).some(m=>hasList(m.component)), 'list leaked to other operator');
    trace.private_to_source = true; trace.invalid_denied = true; trace.console = true;
    await probe.close(); await consoleList(1);
    const rejoined = await connect(port);
    for (const command of ['list', 'list uuids']) trace.feedback.push(await rejoined.command(command, 'commands.list.players'));
    await other.close(); await consoleList(1);
    trace.feedback.push(await rejoined.command('list', 'commands.list.players'));
    trace.roster_changes = true;
    if(kind === 'java') {
      await javaCommands(['gamerule sendCommandFeedback false']);
      const offset = rejoined.messages.length;
      rejoined.client.write('chat_command',{command:'list'}); await pause(200);
      assert.ok(!rejoined.messages.slice(offset).some(m=>hasList(m.component)));
      await consoleList(1); trace.feedback_rule = true;
    }
    await rejoined.close(); await stop();
    if(kind === 'native') {
      const muted = path.join(scratch, 'muted'); await mkdir(muted); await keys(muted);
      await cp(path.join(scratch,'java/world'),path.join(muted,'world'),{recursive:true});
      await start('native',muted,port);
      const user = await connect(port);
      const offset = user.messages.length; user.client.write('chat_command',{command:'list'}); await pause(200);
      assert.ok(!user.messages.slice(offset).some(m=>hasList(m.component)));
      await consoleList(1); trace.feedback_rule = true; await user.close(); await stop();
    }
    result.traces[kind] = trace;
  }
  assert.deepEqual(result.traces.native,result.traces.java,'selected Java/native list behavior differs');
  result.ok = true;
} catch (error) {result.error=error.stack;process.exitCode=1;}
finally {
  try {await stop();} catch(error){result.shutdown_error=error.message;result.ok=false;process.exitCode=1;}
  result.completed_at=new Date().toISOString();
  await writeFile(output,JSON.stringify(result,null,2)+'\n');
  console.log(JSON.stringify({ok:result.ok,report:output,local_diagnostics:scratch,error:result.error}));
}
