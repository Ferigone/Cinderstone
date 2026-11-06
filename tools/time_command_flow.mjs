// SPDX-License-Identifier: GPL-2.0-only
// Real, unchanged Java 1.21 and native Main: time commands, permissions and restart.
import mc from 'minecraft-protocol';
import nbt from 'prismarine-nbt';
import assert from 'node:assert/strict';
import {spawn, execFileSync} from 'node:child_process';
import {mkdtemp, mkdir, readFile, writeFile, copyFile, cp} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import path from 'node:path';
import {createHash} from 'node:crypto';

const [binaryArg, jarArg, eulaArg, output, vectorsOut] = process.argv.slice(2);
assert.ok(vectorsOut, 'BINARY PINNED_JAR EXISTING_EULA REPORT EXECUTION_VECTORS');
const root = process.cwd(), binary = path.resolve(binaryArg), jar = path.resolve(jarArg);
const scratch = await mkdtemp(path.join(tmpdir(), 'time-command-'));
const sha = (bytes, kind = 'sha256') => createHash(kind).update(bytes).digest('hex');
const lock = JSON.parse(await readFile('fixtures/source-lock.json', 'utf8')).oracle;
const jarBytes = await readFile(jar);
assert.equal(sha(jarBytes, 'sha1'), lock.sha1); assert.equal(jarBytes.length, lock.size);
assert.ok((await readFile(eulaArg, 'utf8')).split(/\r?\n/).includes('eula=true'));
const result = {ok: false, native_commit: execFileSync('git', ['rev-parse', 'HEAD'], {encoding: 'utf8'}).trim(),
  native_src_tree: execFileSync('git', ['rev-parse', 'HEAD:src'], {encoding: 'utf8'}).trim(),
  native_binary_sha256: sha(await readFile(binary)), java_jar_sha1: lock.sha1, traces: {},
  limits: ['Single overworld, actual imported clock with doDaylightCycle=false; no general imported gameplay claim.',
    'Only time grammar, completion and selected success packets/day-time are compared. Game-age values and transport timing are not compared.',
    'Java-only result vectors use execute-store into a tracked scoreboard; no native execute command is claimed.']};
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
  const token = `TIME_FLOW_${++marker}`, offset = server.log().length;
  for (const text of commands) server.command(text);
  server.command('say ' + token);
  await until(() => server.log().slice(offset).includes(token), 'Java console barrier');
}
function long(value) {
  if (Array.isArray(value)) return Number(BigInt.asIntN(64, (BigInt(value[0] >>> 0) << 32n) | BigInt(value[1] >>> 0)));
  return Number(value);
}
function timeTree(packet) {
  const nodes = packet.nodes, root = nodes[packet.rootIndex];
  const first = root.children.find(id => nodes[id].extraNodeData?.name === 'time');
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
  const client = mc.createClient({host: '127.0.0.1', port, username: 'TimeProbe', auth: 'offline',
    version: '1.21', keepAlive: false, hideErrors: true});
  let error, ready = false, closing = false, ended = false, tree, age, day;
  const messages = [], completions = [], scores = [];
  client.on('error', value => {error = value;});
  client.on('end', () => {ended = true; if (!closing) error = Error('Unexpected disconnect');});
  client.on('kick_disconnect', () => {error = Error('Probe kicked');});
  client.on('keep_alive', packet => client.write('keep_alive', packet));
  client.on('chunk_batch_finished', () => client.write('chunk_batch_received', {chunksPerTick: 16}));
  client.on('position', packet => {client.write('teleport_confirm', {teleportId: packet.teleportId}); ready = true;});
  client.on('declare_commands', packet => {tree = timeTree(packet);});
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
function value(message) {return Number(message.component.with[0]);}
const commands = [
  ['time query daytime', 'query', 0], ['time set day', 'set', 1000], ['time query daytime', 'query', 1000],
  ['time set noon', 'set', 6000], ['time set night', 'set', 13000], ['time set midnight', 'set', 18000],
  ['time set 2d', 'set', 48000], ['time query day', 'query', 2], ['time query daytime', 'query', 0],
  ['time add 1.5s', 'set', 30], ['time add 0.5d', 'set', 12030], ['time query day', 'query', 2],
  ['time query daytime', 'query', 12030], ['time set 2147483647t', 'set', 2147483647],
  ['time query daytime', 'query', 11647], ['time set 7s', 'set', 140], ['time add 0.125s', 'set', 143],
  ['time set night', 'set', 13000]];
try {
  const seed = path.join(scratch, 'seed'); await mkdir(seed);
  await copyFile(eulaArg, path.join(seed, 'eula.txt'));
  const properties = 'server-ip=127.0.0.1\nserver-port=25641\nonline-mode=false\nenforce-secure-profile=false\n'
    + 'gamemode=creative\nlevel-type=minecraft:flat\ngenerate-structures=false\nview-distance=2\nsimulation-distance=2\n';
  await writeFile(path.join(seed, 'server.properties'), properties);
  await start('java', seed, 25641);
  await javaCommands(['gamerule doDaylightCycle false', 'gamerule doWeatherCycle false',
    'gamerule doMobSpawning false', 'gamerule randomTickSpeed 0', 'weather clear', 'time set 0',
    'forceload add -64 -64 64 64']);
  // Native import cannot generate missing FULL chunks. Observe the original's
  // actual readiness before saving the scene; do not patch chunk statuses.
  const fullPrefix = `TIME_FULL_${++marker}`, positions = [];
  for (let x = -4; x <= 4; x++) for (let z = -4; z <= 4; z++) positions.push([x, z]);
  const fullDeadline = Date.now() + 90000;
  while (!positions.every(([x, z]) => server.log().includes(`${fullPrefix}_${x}_${z}`))) {
    if (Date.now() > fullDeadline) throw Error('Original scene FULL readiness deadline');
    for (const [x, z] of positions)
      if (!server.log().includes(`${fullPrefix}_${x}_${z}`))
        server.command(`execute if loaded ${x * 16} 64 ${z * 16} run say ${fullPrefix}_${x}_${z}`);
    await pause(100);
  }
  await javaCommands(['save-all flush']);
  await stop();
  for (const kind of ['java', 'native']) {
    const folder = path.join(scratch, kind); await mkdir(folder);
    await cp(path.join(seed, 'world'), path.join(folder, 'world'), {recursive: true});
    await writeFile(path.join(folder, 'keys.json'), '{"profilePropertyKeys":[],"playerCertificateKeys":[]}');
    await copyFile(eulaArg, path.join(folder, 'eula.txt'));
    await writeFile(path.join(folder, 'server.properties'), properties);
    const port = kind === 'java' ? 25641 : 25642;
    await start(kind, folder, port);
    if (kind === 'java') await javaCommands(['tick freeze']);
    const probe = await connect(port); assert.equal(probe.tree, null, 'time must be hidden for a non-op');
    await probe.denied('time set night');
    server.command('op TimeProbe'); await probe.wait(() => probe.tree !== null, 'operator tree refresh');
    const trace = {tree: probe.tree, success: [], completion: [], denied_nonop: true, invalid_no_mutation: true};
    // The first query also proves that the denied command did not set night.
    for (const [command, suffix, expected] of commands) {
      const message = await probe.command(command, 'commands.time.' + suffix);
      assert.equal(value(message), expected, command); trace.success.push({command, ...message});
    }
    for (const command of ['time add -1', 'time set 1q', 'time set day trailing']) await probe.denied(command);
    assert.equal(value(await probe.command('time query daytime', 'commands.time.query')), 13000);
    let transaction = 1;
    for (const text of ['time ', 'time set ', 'time set 1', 'time query ', '/time add 12'])
      trace.completion.push({text, ...await probe.complete(text, transaction++)});
    const age = value(await probe.command('time query gametime', 'commands.time.query'));
    assert.ok(age >= 0); trace.gametime_query = true;
    await probe.wait(() => probe.day === -13000, 'real night time publication'); trace.client_night = true;
    // Console and player share the dispatcher; restore night after a console mutation.
    server.command('/time set noon'); await probe.wait(() => probe.day === -6000, 'console time mutation');
    trace.console = true; await probe.command('time set night', 'commands.time.set');
    if (kind === 'java') {
      await javaCommands(['scoreboard objectives add time_result dummy', 'scoreboard objectives setdisplay sidebar time_result']);
      async function stored(holder, command) {
        const offset = probe.scores.length;
        server.command(`scoreboard players reset ${holder} time_result`);
        server.command(`execute store result score ${holder} time_result run ${command}`);
        await probe.wait(() => probe.scores.slice(offset).some(p => p.scoreName === 'time_result' && p.itemName === holder), 'original command result');
        return probe.scores.slice(offset).find(p => p.scoreName === 'time_result' && p.itemName === holder).value;
      }
      const gameTime = await stored('TimeAge', 'time query gametime');
      let dayTime = 13000; const vectors = [];
      for (const input of ['time set day', 'time set 2d', 'time add 1.5s', 'time query day',
        'time query daytime', 'time query gametime', 'time set 2147483647t', 'time add 1', 'time set night']) {
        const initial = dayTime, commandResult = await stored('TimeResult', input);
        const wholeDays = await stored('TimeDay', 'time query day');
        const partialDay = await stored('TimeClock', 'time query daytime');
        dayTime = wholeDays * 24000 + partialDay;
        vectors.push({input, game_time: gameTime, day_time: initial, after_day_time: dayTime, result: commandResult});
      }
      assert.equal(await stored('TimeAge', 'time query gametime'), gameTime);
      await writeFile(vectorsOut, JSON.stringify(vectors, null, 2) + '\n');
    }
    await probe.close(); await stop();
    const saved = nbt.simplify((await nbt.parse(await readFile(path.join(folder, 'world/level.dat')))).parsed).Data;
    assert.equal(long(saved.DayTime), 13000); assert.equal(saved.GameRules.doDaylightCycle, 'false');
    trace.saved_day_time = long(saved.DayTime);
    await start(kind, folder, port);
    const restarted = await connect(port);
    assert.equal(value(await restarted.command('time query daytime', 'commands.time.query')), 13000);
    await restarted.wait(() => restarted.day === -13000, 'restored night publication');
    trace.restart = true; await restarted.close(); await stop(); result.traces[kind] = trace;
  }
  assert.deepEqual(result.traces.native, result.traces.java, 'selected Java/native behavior differs');
  // Also exercise the fully supported fresh-native profile and its actual JSON checkpoint.
  const flat = path.join(scratch, 'native-flat'); await mkdir(flat);
  await writeFile(path.join(flat, 'keys.json'), '{"profilePropertyKeys":[],"playerCertificateKeys":[]}');
  await start('native', flat, 25642, false);
  const flatProbe = await connect(25642);
  assert.equal(flatProbe.tree, null); await flatProbe.denied('time set night');
  server.command('op TimeProbe'); await flatProbe.wait(() => flatProbe.tree !== null, 'native operator tree');
  assert.ok(value(await flatProbe.command('time query daytime', 'commands.time.query')) < 1000);
  assert.equal(value(await flatProbe.command('time set midnight', 'commands.time.set')), 18000);
  await flatProbe.wait(() => flatProbe.day >= 18000 && flatProbe.day < 19000, 'native midnight publication');
  server.command('/time set night');
  await flatProbe.wait(() => flatProbe.day >= 13000 && flatProbe.day < 14000, 'native console night');
  await flatProbe.close(); await stop();
  const checkpoint = JSON.parse(await readFile(path.join(flat, 'world.json'), 'utf8'));
  assert.equal(checkpoint.clock.daylight_cycle, true);
  assert.ok(checkpoint.clock.day_time >= 13000 && checkpoint.clock.day_time < 15000);
  await start('native', flat, 25642, false);
  const restored = await connect(25642);
  const restoredDay = value(await restored.command('time query daytime', 'commands.time.query'));
  assert.ok(restoredDay >= checkpoint.clock.day_time && restoredDay < checkpoint.clock.day_time + 1200);
  result.native_flat = {permission_denial: true, player_and_console: true, client_night: true,
    saved_day_time: checkpoint.clock.day_time, restored_day_time: restoredDay, cycle_unchanged: true, restart: true};
  await restored.close(); await stop();
  result.ok = true;
} catch (error) {result.error = error.stack; process.exitCode = 1;}
finally {
  try {await stop();} catch (error) {result.shutdown_error = error.message; result.ok = false; process.exitCode = 1;}
  result.completed_at = new Date().toISOString();
  await writeFile(output, JSON.stringify(result, null, 2) + '\n');
  console.log(JSON.stringify({ok: result.ok, report: output, local_diagnostics: scratch, error: result.error}));
}
