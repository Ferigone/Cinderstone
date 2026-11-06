// SPDX-License-Identifier: GPL-2.0-only
// Real, unchanged Java 1.21 and native Main: seed command, private rich feedback and restart.
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
const scratch = await mkdtemp(path.join(tmpdir(), 'seed-command-'));
const sha = (bytes, kind = 'sha256') => createHash(kind).update(bytes).digest('hex');
const lock = JSON.parse(await readFile('fixtures/source-lock.json', 'utf8')).oracle;
const jarBytes = await readFile(jar);
assert.equal(sha(jarBytes, 'sha1'), lock.sha1); assert.equal(jarBytes.length, lock.size);
assert.ok((await readFile(eulaArg, 'utf8')).split(/\r?\n/).includes('eula=true'));
const result = {ok: false, native_commit: execFileSync('git', ['rev-parse', 'HEAD'], {encoding: 'utf8'}).trim(),
  native_src_tree: execFileSync('git', ['rev-parse', 'HEAD:src'], {encoding: 'utf8'}).trim(),
  native_binary_sha256: sha(await readFile(binary)), java_jar_sha1: lock.sha1, traces: {},
  limits: ['Dedicated, single-overworld command; source seed is immutable metadata, not the obfuscated biome seed.',
    'Imported scenes exercise metadata/administration only, not general imported gameplay.',
    'Feedback bytes/tree/completions/privacy are compared. Error formatting and natural tick elapsed time are not compared.']};
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
  const token = `SEED_FLOW_${++marker}`, offset = server.log().length;
  for (const text of commands) server.command(text);
  server.command('say ' + token);
  await until(() => server.log().slice(offset).includes(token), 'Java console barrier');
}
function long(value) {
  if (Array.isArray(value)) return Number(BigInt.asIntN(64, (BigInt(value[0] >>> 0) << 32n) | BigInt(value[1] >>> 0)));
  return Number(value);
}
function seedTree(packet) {
  const nodes = packet.nodes, root = nodes[packet.rootIndex];
  const first = root.children.find(id => nodes[id].extraNodeData?.name === 'seed');
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
async function connect(port, username = "SeedProbe") {
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
  client.on('declare_commands', packet => {tree = seedTree(packet);});
  client.on('update_time', packet => {age = long(packet.age); day = long(packet.time);});
  client.on('tab_complete', packet => completions.push(packet));
  client.on('scoreboard_score', packet => scores.push(packet));
  client.on('packet', (packet, meta, buffer) => {
    if (meta.name === 'system_chat') messages.push({component: nbt.simplify(packet.content), hex: buffer.subarray(1).toString('hex')});
  });
  async function wait(test, label) {await until(() => {if (error) throw error; return test();}, label);}
  await wait(() => ready && tree !== undefined && day !== undefined, 'PLAY admission');
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

const SEED = '-1234567890123456789';
function seedText(component) {
  return component.with[0].with[0].text;
}
function hasSeed(component) {
  if (!component || typeof component !== 'object') return false;
  return component.translate === 'commands.seed.success' || Object.values(component).some(hasSeed);
}
function seedString(value) {
  if (Array.isArray(value)) return BigInt.asIntN(64, (BigInt(value[0] >>> 0) << 32n) | BigInt(value[1] >>> 0)).toString();
  return BigInt(value).toString();
}
async function keys(folder) {
  await writeFile(path.join(folder, 'keys.json'), '{"profilePropertyKeys":[],"playerCertificateKeys":[]}');
}
async function consoleSeed(expected) {
  const offset = server.log().length;
  server.command('/seed');
  await until(() => server.log().slice(offset).includes(expected) &&
    server.log().slice(offset).includes(server.kind === 'java' ? 'Seed:' : 'commands.seed.success'), 'console seed feedback');
}
try {
  const seed = path.join(scratch, 'seed'); await mkdir(seed);
  await copyFile(eulaArg, path.join(seed, 'eula.txt'));
  const properties = 'server-ip=127.0.0.1\nserver-port=25641\nonline-mode=false\nenforce-secure-profile=false\n'
    + 'gamemode=creative\nlevel-seed=' + SEED + '\nlevel-type=minecraft:flat\ngenerate-structures=false\nview-distance=2\nsimulation-distance=2\n';
  await writeFile(path.join(seed, 'server.properties'), properties);
  await start('java', seed, 25641);
  await javaCommands(['gamerule doDaylightCycle false', 'gamerule doWeatherCycle false',
    'gamerule doMobSpawning false', 'gamerule randomTickSpeed 0',
    'setworldspawn 8 65 8', 'forceload add -80 -80 80 80', 'save-all flush']);
  await stop();
  const witnesses = JSON.parse(await readFile('fixtures/seed-command-vectors.json', 'utf8')).execution;
  for (const kind of ['java', 'native']) {
    const folder = path.join(scratch, kind); await mkdir(folder);
    await cp(path.join(seed, 'world'), path.join(folder, 'world'), {recursive: true});
    await keys(folder); await copyFile(eulaArg, path.join(folder, 'eula.txt'));
    await writeFile(path.join(folder, 'server.properties'), properties);
    const port = kind === 'java' ? 25641 : 25642;
    await start(kind, folder, port);
    const probe = await connect(port); assert.equal(probe.tree, null);
    await probe.denied('seed');
    server.command('op SeedProbe'); await probe.wait(() => probe.tree !== null, 'operator tree');
    const other = await connect(port, 'SeedOther');
    server.command('op SeedOther'); await other.wait(() => other.tree !== null, 'second operator tree');
    const offset = other.messages.length;
    const message = await probe.command('seed', 'commands.seed.success');
    assert.equal(seedText(message.component), SEED);
    const expectedHex = witnesses.find(w => w.input === 'seed' && w.permission === 2 && w.feedback_hex.includes(Buffer.from(SEED).toString('hex'))).feedback_hex;
    assert.equal(message.hex, expectedHex);
    const trace = {tree: probe.tree, feedback: message, completions: [], nonop_denied: true};
    await probe.denied('seed extra');
    let transaction = 1;
    for (const text of ['/see', 'seed', 'seed ']) trace.completions.push({text, ...await probe.complete(text, transaction++)});
    await consoleSeed(SEED); await pause(250);
    assert.ok(!other.messages.slice(offset).some(m => hasSeed(m.component)), 'seed leaked to another operator');
    trace.private_to_source = true; trace.console = true;
    await other.close(); await probe.close(); await stop();
    const metadata = nbt.simplify((await nbt.parse(await readFile(path.join(folder, 'world/level.dat')))).parsed).Data;
    assert.equal(seedString(metadata.WorldGenSettings.seed), SEED); trace.saved_seed_unchanged = true;
    await start(kind, folder, port);
    const restarted = await connect(port);
    assert.equal((await restarted.command('seed', 'commands.seed.success')).hex, message.hex);
    trace.restart = true;
    if (kind === 'java') {
      await javaCommands(['gamerule sendCommandFeedback false']);
      const muted = restarted.messages.length;
      restarted.client.write('chat_command', {command:'seed'}); await pause(250);
      assert.ok(!restarted.messages.slice(muted).some(m => hasSeed(m.component)));
      await consoleSeed(SEED); trace.feedback_rule = true;
    }
    await restarted.close(); await stop();
    if (kind === 'native') {
      const muted = path.join(scratch, 'native-muted'); await mkdir(muted);
      await cp(path.join(scratch, 'java/world'), path.join(muted, 'world'), {recursive:true}); await keys(muted);
      await start('native', muted, port);
      const mutedProbe = await connect(port); server.command('op SeedProbe');
      await mutedProbe.wait(() => mutedProbe.tree !== null, 'muted operator tree');
      const offset = mutedProbe.messages.length;
      mutedProbe.client.write('chat_command', {command:'seed'}); await pause(250);
      assert.ok(!mutedProbe.messages.slice(offset).some(m => hasSeed(m.component)));
      await consoleSeed(SEED); trace.feedback_rule = true;
      await mutedProbe.close(); await stop();
    }
    result.traces[kind] = trace;
  }
  assert.deepEqual(result.traces.native, result.traces.java, 'selected Java/native seed behavior differs');
  const flat = path.join(scratch, 'native-flat'); await mkdir(flat); await keys(flat);
  await start('native', flat, 25642, false);
  const probe = await connect(25642); server.command('op SeedProbe');
  await probe.wait(() => probe.tree !== null, 'native seed tree');
  const zero = await probe.command('seed', 'commands.seed.success');
  assert.equal(seedText(zero.component), '0');
  assert.equal(zero.hex, witnesses.find(w => w.seed === 0 && w.input === 'seed').feedback_hex);
  await consoleSeed('0'); await probe.close(); await stop();
  await start('native', flat, 25642, false);
  const restored = await connect(25642);
  assert.equal((await restored.command('seed', 'commands.seed.success')).hex, zero.hex);
  result.native_flat = {seed:'0', feedback_hex:zero.hex, console:true, restart:true};
  await restored.close(); await stop(); result.ok = true;
} catch (error) {result.error = error.stack; process.exitCode = 1;}
finally {
  try {await stop();} catch (error) {result.shutdown_error = error.message; result.ok = false; process.exitCode = 1;}
  result.completed_at = new Date().toISOString();
  await writeFile(output, JSON.stringify(result, null, 2) + '\n');
  console.log(JSON.stringify({ok: result.ok, report: output, local_diagnostics: scratch, error: result.error}));
}
