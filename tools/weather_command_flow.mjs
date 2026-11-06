// SPDX-License-Identifier: GPL-2.0-only
// Real, unchanged Java 1.21 and native Main: weather commands, permissions and restart.
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
const scratch = await mkdtemp(path.join(tmpdir(), 'weather-command-'));
const sha = (bytes, kind = 'sha256') => createHash(kind).update(bytes).digest('hex');
const lock = JSON.parse(await readFile('fixtures/source-lock.json', 'utf8')).oracle;
const jarBytes = await readFile(jar);
assert.equal(sha(jarBytes, 'sha1'), lock.sha1); assert.equal(jarBytes.length, lock.size);
assert.ok((await readFile(eulaArg, 'utf8')).split(/\r?\n/).includes('eula=true'));
const result = {ok: false, native_commit: execFileSync('git', ['rev-parse', 'HEAD'], {encoding: 'utf8'}).trim(),
  native_src_tree: execFileSync('git', ['rev-parse', 'HEAD:src'], {encoding: 'utf8'}).trim(),
  native_binary_sha256: sha(await readFile(binary)), java_jar_sha1: lock.sha1, traces: {},
  limits: ['Single overworld; imported scene probes administration/weather only, not general imported gameplay.',
    'Duration defaults use each server own RNG. Exact controlled RNG consumption is separately checked by the original-class vectors.',
    'Transport timing and command-error formatting are not compared; ordered weather transition packets are retained without deduplication.']};
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
  const token = `WEATHER_FLOW_${++marker}`, offset = server.log().length;
  for (const text of commands) server.command(text);
  server.command('say ' + token);
  await until(() => server.log().slice(offset).includes(token), 'Java console barrier');
}
function long(value) {
  if (Array.isArray(value)) return Number(BigInt.asIntN(64, (BigInt(value[0] >>> 0) << 32n) | BigInt(value[1] >>> 0)));
  return Number(value);
}
function weatherTree(packet) {
  const nodes = packet.nodes, root = nodes[packet.rootIndex];
  const first = root.children.find(id => nodes[id].extraNodeData?.name === 'weather');
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
async function connect(port) {
  const client = mc.createClient({host: '127.0.0.1', port, username: 'WeatherProbe', auth: 'offline',
    version: '1.21', keepAlive: false, hideErrors: true});
  let error, ready = false, closing = false, ended = false, tree, age, day;
  const messages = [], completions = [], scores = [], weather = [];
  let rain = 0, thunder = 0;
  client.on('error', value => {error = value;});
  client.on('end', () => {ended = true; if (!closing) error = Error('Unexpected disconnect');});
  client.on('kick_disconnect', () => {error = Error('Probe kicked');});
  client.on('keep_alive', packet => client.write('keep_alive', packet));
  client.on('chunk_batch_finished', () => client.write('chunk_batch_received', {chunksPerTick: 16}));
  client.on('position', packet => {client.write('teleport_confirm', {teleportId: packet.teleportId}); ready = true;});
  client.on('declare_commands', packet => {tree = weatherTree(packet);});
  client.on('update_time', packet => {age = long(packet.age); day = long(packet.time);});
  client.on('tab_complete', packet => completions.push(packet));
  client.on('game_state_change', packet => {
    if (['start_raining', 'stop_raining', 'rain_level_change', 'thunder_level_change'].includes(packet.reason))
      weather.push({reason: packet.reason, value: packet.gameMode});
    if (packet.reason === 'rain_level_change') rain = packet.gameMode;
    if (packet.reason === 'thunder_level_change') thunder = packet.gameMode;
  });
  client.on('packet', (packet, meta, buffer) => {
    if (meta.name === 'system_chat') messages.push({component: nbt.simplify(packet.content), hex: buffer.subarray(1).toString('hex')});
  });
  async function wait(test, label) {await until(() => {if (error) throw error; return test();}, label);}
  await wait(() => ready && tree !== undefined && day !== undefined, 'PLAY admission');
  return {client, messages, scores, weather, get rain() {return rain;}, get thunder() {return thunder;}, get tree() {return tree;}, get age() {return age;}, get day() {return day;},
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

function weatherState(data) {
  return [data.clearWeatherTime, data.rainTime, data.thunderTime, !!data.raining, !!data.thundering];
}
async function localKeys(folder) {
  await writeFile(path.join(folder, 'keys.json'), '{"profilePropertyKeys":[],"playerCertificateKeys":[]}');
}
try {
  const seed = path.join(scratch, 'seed'); await mkdir(seed);
  await copyFile(eulaArg, path.join(seed, 'eula.txt'));
  const properties = 'server-ip=127.0.0.1\nserver-port=25641\nonline-mode=false\nenforce-secure-profile=false\n'
    + 'gamemode=creative\nlevel-type=minecraft:flat\ngenerate-structures=false\nview-distance=2\nsimulation-distance=2\n';
  await writeFile(path.join(seed, 'server.properties'), properties);
  await start('java', seed, 25641);
  await javaCommands(['gamerule doDaylightCycle false', 'gamerule doWeatherCycle false',
    'gamerule doMobSpawning false', 'gamerule randomTickSpeed 0', 'weather clear 100000t',
    'setworldspawn 8 65 8', 'forceload add -80 -80 80 80', 'save-all flush']);
  await stop();
  for (const kind of ['java', 'native']) {
    const folder = path.join(scratch, kind); await mkdir(folder);
    await cp(path.join(seed, 'world'), path.join(folder, 'world'), {recursive: true});
    await localKeys(folder);
    await copyFile(eulaArg, path.join(folder, 'eula.txt'));
    await writeFile(path.join(folder, 'server.properties'), properties);
    const port = kind === 'java' ? 25641 : 25642;
    await start(kind, folder, port);
    const probe = await connect(port);
    assert.equal(probe.tree, null, 'weather must be hidden for a non-op');
    await probe.denied('weather thunder 1d');
    assert.equal(probe.rain, 0); assert.equal(probe.thunder, 0);
    server.command('op WeatherProbe'); await probe.wait(() => probe.tree !== null, 'operator tree refresh');
    const trace = {tree: probe.tree, success: [], completion: [], transitions: [], denied_nonop: true};
    for (const [command, type, rain, thunder] of [
      ['weather rain 12.5s', 'rain', 1, 0], ['weather thunder 1d', 'thunder', 1, 1],
      ['weather clear 20s', 'clear', 0, 0],
    ]) {
      const offset = probe.weather.length;
      trace.success.push({command, ...await probe.command(command, 'commands.weather.set.' + type)});
      await probe.wait(() => probe.rain === rain && probe.thunder === thunder, 'visible ' + type);
      await pause(100);
      trace.transitions.push({command, packets: probe.weather.slice(offset)});
    }
    for (const command of ['weather rain 0', 'weather rain -1', 'weather clear 20 trailing']) await probe.denied(command);
    await pause(200); assert.equal(probe.rain, 0); assert.equal(probe.thunder, 0);
    trace.invalid_no_mutation = true;
    let transaction = 1;
    for (const text of ['weather ', 'weather r', 'weather rain ', 'weather rain 1', '/weather thunder 12'])
      trace.completion.push({text, ...await probe.complete(text, transaction++)});
    server.command('/weather thunder 600t');
    await probe.wait(() => probe.rain === 1 && probe.thunder === 1, 'console thunder');
    trace.console = true;
    await probe.close(); await stop();
    const saved = nbt.simplify((await nbt.parse(await readFile(path.join(folder, 'world/level.dat')))).parsed).Data;
    assert.deepEqual(weatherState(saved), [0, 600, 600, true, true]);
    assert.equal(saved.GameRules.doWeatherCycle, 'false'); trace.saved_state = weatherState(saved);
    await start(kind, folder, port);
    const restarted = await connect(port);
    await restarted.wait(() => restarted.rain === 1 && restarted.thunder === 1, 'restored thunder');
    trace.restart = true; await restarted.close(); await stop(); result.traces[kind] = trace;
  }
  assert.deepEqual(result.traces.native, result.traces.java, 'selected Java/native behavior differs');
  const flat = path.join(scratch, 'native-flat'); await mkdir(flat); await localKeys(flat);
  await start('native', flat, 25642, false);
  const probe = await connect(25642);
  server.command('op WeatherProbe'); await probe.wait(() => probe.tree !== null, 'native operator tree');
  await probe.command('weather rain', 'commands.weather.set.rain');
  await probe.wait(() => probe.rain === 1, 'native default-duration rain');
  await probe.close(); await stop();
  const checkpoint = JSON.parse(await readFile(path.join(flat, 'world.json'), 'utf8'));
  assert.equal(checkpoint.weather.raining, true); assert.equal(checkpoint.weather.thundering, false);
  assert.ok(checkpoint.weather.rain_time >= 10800 && checkpoint.weather.rain_time <= 24000);
  assert.ok(checkpoint.weather.thunder_time >= 10800 && checkpoint.weather.thunder_time <= 24000);
  await start('native', flat, 25642, false);
  const restored = await connect(25642);
  await restored.wait(() => restored.rain === 1 && restored.thunder === 0, 'restored native rain');
  result.native_flat = {sampled_default_range: true, saved_raining: true, restart: true,
    rain_time: checkpoint.weather.rain_time, thunder_time: checkpoint.weather.thunder_time};
  await restored.close(); await stop(); result.ok = true;
} catch (error) {result.error = error.stack; process.exitCode = 1;}
finally {
  try {await stop();} catch (error) {result.shutdown_error = error.message; result.ok = false; process.exitCode = 1;}
  result.completed_at = new Date().toISOString();
  await writeFile(output, JSON.stringify(result, null, 2) + '\n');
  console.log(JSON.stringify({ok: result.ok, report: output, local_diagnostics: scratch, error: result.error}));
}
