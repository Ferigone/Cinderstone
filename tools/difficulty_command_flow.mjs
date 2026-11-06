// SPDX-License-Identifier: GPL-2.0-only
// Real, unchanged Java 1.21 and native Main: difficulty commands, permissions and restart.
import mc from 'minecraft-protocol';
import nbt from 'prismarine-nbt';
import assert from 'node:assert/strict';
import {spawn, execFileSync} from 'node:child_process';
import {mkdtemp, mkdir, readFile, writeFile, copyFile, cp} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import path from 'node:path';
import {createHash} from 'node:crypto';
import {gzipSync} from 'node:zlib';

const [binaryArg, jarArg, eulaArg, output] = process.argv.slice(2);
assert.ok(output, 'BINARY PINNED_JAR EXISTING_EULA REPORT');
const root = process.cwd(), binary = path.resolve(binaryArg), jar = path.resolve(jarArg);
const scratch = await mkdtemp(path.join(tmpdir(), 'difficulty-command-'));
const sha = (bytes, kind = 'sha256') => createHash(kind).update(bytes).digest('hex');
const lock = JSON.parse(await readFile('fixtures/source-lock.json', 'utf8')).oracle;
const jarBytes = await readFile(jar);
assert.equal(sha(jarBytes, 'sha1'), lock.sha1); assert.equal(jarBytes.length, lock.size);
assert.ok((await readFile(eulaArg, 'utf8')).split(/\r?\n/).includes('eula=true'));
const result = {ok: false, native_commit: execFileSync('git', ['rev-parse', 'HEAD'], {encoding: 'utf8'}).trim(),
  native_src_tree: execFileSync('git', ['rev-parse', 'HEAD:src'], {encoding: 'utf8'}).trim(),
  native_binary_sha256: sha(await readFile(binary)), java_jar_sha1: lock.sha1, traces: {},
  limits: ['Supported single overworld; imported scene checks command state, not survival or imported-world gameplay.',
    'Packet timing and grammar-error formatting are excluded. Success component bytes, selected command graph and suggestions are exact.',
    'Root-prefix d comparisons retain only the implemented deop/difficulty matches; other Java command roots are outside this native catalog.',
    'Dedicated startup reapplies configured Easy even after saved Hard, as the original does. Mob spawning effects await the mob owners.']};
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
  const token = `DIFFICULTY_FLOW_${++marker}`, offset = server.log().length;
  for (const text of commands) server.command(text);
  server.command('say ' + token);
  await until(() => server.log().slice(offset).includes(token), 'Java console barrier');
}
function long(value) {
  if (Array.isArray(value)) return Number(BigInt.asIntN(64, (BigInt(value[0] >>> 0) << 32n) | BigInt(value[1] >>> 0)));
  return Number(value);
}
function difficultyTree(packet) {
  const nodes = packet.nodes, root = nodes[packet.rootIndex];
  const first = root.children.find(id => nodes[id].extraNodeData?.name === 'difficulty');
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
function hasTranslation(component,key) {
  if (Array.isArray(component)) return component.some(value=>hasTranslation(value,key));
  return component && typeof component==='object' &&
    (component.translate===key || Object.values(component).some(value=>hasTranslation(value,key)));
}
function red(component) {
  if (Array.isArray(component)) return component.some(red);
  return component && typeof component === 'object' &&
    (component.color === 'red' || Object.values(component).some(red));
}
async function connect(port, username = "DifficultyProbe") {
  const client = mc.createClient({host: '127.0.0.1', port, username, auth: 'offline',
    version: '1.21', keepAlive: false, hideErrors: true});
  let error, ready = false, closing = false, ended = false, tree, age, day;
  const messages = [], completions = [], difficulties = [];
  client.on("difficulty", packet => difficulties.push({difficulty:["peaceful","easy","normal","hard"].indexOf(packet.difficulty),difficultyLocked:packet.difficultyLocked}));
  client.on('error', value => {error = value;});
  client.on('end', () => {ended = true; if (!closing) error = Error('Unexpected disconnect');});
  client.on('kick_disconnect', () => {error = Error('Probe kicked');});
  client.on('keep_alive', packet => client.write('keep_alive', packet));
  client.on('chunk_batch_finished', () => client.write('chunk_batch_received', {chunksPerTick: 16}));
  client.on('position', packet => {client.write('teleport_confirm', {teleportId: packet.teleportId}); ready = true;});
  client.on('declare_commands', packet => {tree = difficultyTree(packet);});
  client.on('update_time', packet => {age = long(packet.age); day = long(packet.time);});
  client.on('tab_complete', packet => completions.push(packet));
  client.on('packet', (packet, meta, buffer) => {
    if (meta.name === 'system_chat') messages.push({component: nbt.simplify(packet.content), hex: buffer.subarray(1).toString('hex')});
  });
  async function wait(test, label) {await until(() => {if (error) throw error; return test();}, label);}
  await wait(() => ready && tree !== undefined && day !== undefined && difficulties.length, 'PLAY admission');
  return {client, messages, difficulties, get tree() {return tree;}, get age() {return age;}, get day() {return day;},
    async command(text, key) {
      const offset = messages.length; client.write('chat_command', {command: text});
      await wait(() => messages.slice(offset).some(value => hasTranslation(value.component,key)), text);
      return messages.slice(offset).find(value => hasTranslation(value.component,key));
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

async function localKeys(folder) {
  await writeFile(path.join(folder, 'keys.json'), '{"profilePropertyKeys":[],"playerCertificateKeys":[]}');
}
try {
  const seed = path.join(scratch, 'seed'); await mkdir(seed);
  await copyFile(eulaArg, path.join(seed, 'eula.txt'));
  const properties = 'server-ip=127.0.0.1\nserver-port=25661\nonline-mode=false\nenforce-secure-profile=false\n'
    + 'gamemode=creative\ndifficulty=easy\nlevel-type=minecraft:flat\ngenerate-structures=false\nview-distance=2\nsimulation-distance=2\n';
  await writeFile(path.join(seed, 'server.properties'), properties);
  await start('java', seed, 25661);
  await javaCommands(['gamerule doDaylightCycle false', 'gamerule doWeatherCycle false',
    'gamerule doMobSpawning false', 'gamerule randomTickSpeed 0', 'weather clear 100000t',
    'setworldspawn 8 65 8', 'forceload add -80 -80 80 80', 'save-all flush']);
  await stop();
  // Controlled original-generated scene shared unchanged by both servers.
  const levelPath = path.join(seed, 'world/level.dat');
  const original = (await nbt.parse(await readFile(levelPath))).parsed;
  original.value.Data.value.DifficultyLocked = {type:'byte',value:1};
  await writeFile(levelPath,gzipSync(nbt.writeUncompressed(original)));
  for (const kind of ['java', 'native']) {
    const folder = path.join(scratch, kind); await mkdir(folder);
    await cp(path.join(seed, 'world'), path.join(folder, 'world'), {recursive: true});
    await localKeys(folder);
    await copyFile(eulaArg, path.join(folder, 'eula.txt'));
    await writeFile(path.join(folder, 'server.properties'), properties);
    const port = kind === 'java' ? 25661 : 25662;
    await start(kind, folder, port);
    const probe = await connect(port);
    assert.equal(probe.tree, null, 'difficulty hidden for non-op');
    assert.deepEqual(probe.difficulties[0],{difficulty:1,difficultyLocked:true});
    await probe.denied('difficulty hard');
    assert.equal(probe.difficulties.length,1);
    server.command('op DifficultyProbe'); await probe.wait(() => probe.tree !== null, 'operator tree refresh');
    const observer = await connect(port,'DifficultyOther');
    server.command('op DifficultyOther'); await observer.wait(() => observer.tree !== null,'observer operator');
    const trace = {tree:probe.tree,initial:probe.difficulties[0],success:[],completion:[],updates:[],denied_nonop:true};
    let count = observer.messages.length;
    trace.success.push({command:'difficulty',...await probe.command('difficulty','commands.difficulty.query')});
    await pause(150); assert.equal(observer.messages.length,count,'query must be private');
    trace.query_private = true;
    for (const [command,id] of [['difficulty hard',3],['difficulty peaceful',0],['difficulty normal',2]]) {
      const offset = probe.difficulties.length, otherOffset = observer.difficulties.length;
      trace.success.push({command,...await probe.command(command,'commands.difficulty.success')});
      await probe.wait(() => probe.difficulties.length>offset,'source difficulty update');
      await observer.wait(() => observer.difficulties.length>otherOffset,'observer difficulty update');
      assert.deepEqual(probe.difficulties.at(-1),{difficulty:id,difficultyLocked:true});
      assert.deepEqual(observer.difficulties.at(-1),probe.difficulties.at(-1));
      trace.updates.push(probe.difficulties.at(-1));
    }
    const before=probe.difficulties.length;
    trace.failure = await probe.command('difficulty normal','commands.difficulty.failure');
    await writeFile(path.join(folder,'failure.json'),JSON.stringify(trace.failure,null,2));
    for (const command of ['difficulty 0','difficulty HARD','difficulty hard trailing']) await probe.denied(command);
    await pause(150); assert.equal(probe.difficulties.length,before); trace.invalid_no_mutation=true;
    let transaction=1;
    for (const text of ['difficulty ','difficulty h','/difficulty p','difficulty hard ','d','/d']) {
      const completion=await probe.complete(text,transaction++);
      if (text==='d' || text==='/d') {
        completion.matches=completion.matches.filter(value=>['deop','difficulty'].includes(value));
        assert.deepEqual(completion.matches,['deop','difficulty'],'supported roots must both be suggested');
      }
      trace.completion.push({text,...completion});
    }
    server.command('difficulty hard');
    await probe.wait(() => probe.difficulties.at(-1).difficulty===3,'console update'); trace.console=true;
    await observer.close(); await probe.close();
    const rejoined=await connect(port);
    assert.deepEqual(rejoined.difficulties[0],{difficulty:3,difficultyLocked:true}); trace.rejoin=rejoined.difficulties[0];
    await rejoined.close(); await stop();
    const saved=nbt.simplify((await nbt.parse(await readFile(path.join(folder,'world/level.dat')))).parsed).Data;
    assert.equal(saved.Difficulty,3); assert.equal(saved.DifficultyLocked,1);
    assert.equal(saved.GameRules.doDaylightCycle,'false');
    assert.match(await readFile(path.join(folder,'server.properties'),'utf8'),/^difficulty=easy$/m);
    trace.saved=[saved.Difficulty,saved.DifficultyLocked]; trace.dedicated_property_unchanged=true;
    await start(kind,folder,port);
    const restarted=await connect(port);
    assert.deepEqual(restarted.difficulties[0],{difficulty:1,difficultyLocked:true}); trace.restart=restarted.difficulties[0];
    await restarted.close(); await stop(); result.traces[kind]=trace;
  }
  assert.deepEqual(result.traces.native,result.traces.java,'selected Java/native behavior differs');
  const flat=path.join(scratch,'native-flat'); await mkdir(flat); await localKeys(flat);
  await start('native',flat,25662,false);
  const probe=await connect(25662);
  server.command('op DifficultyProbe'); await probe.wait(() => probe.tree!==null,'native operator tree');
  await probe.command('difficulty hard','commands.difficulty.success');
  assert.deepEqual(probe.difficulties.at(-1),{difficulty:3,difficultyLocked:false});
  await probe.close(); await stop();
  const checkpoint=JSON.parse(await readFile(path.join(flat,'world.json'),'utf8'));
  assert.equal(checkpoint.fire_ticks.difficulty_id,3);
  await start('native',flat,25662,false);
  const restored=await connect(25662);
  assert.deepEqual(restored.difficulties[0],{difficulty:1,difficultyLocked:false});
  result.native_flat={saved_fire_difficulty:3,restart:restored.difficulties[0]};
  await restored.close(); await stop(); result.ok=true;
} catch (error) {result.error=error.stack; process.exitCode=1;}
finally {
  try {await stop();} catch (error) {result.shutdown_error=error.message; result.ok=false; process.exitCode=1;}
  result.completed_at=new Date().toISOString();
  await writeFile(output,JSON.stringify(result,null,2)+'\n');
  console.log(JSON.stringify({ok:result.ok,report:output,local_diagnostics:scratch,error:result.error}));
}
