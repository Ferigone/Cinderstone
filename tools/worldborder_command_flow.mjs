// SPDX-License-Identifier: GPL-2.0-only
// Real, unchanged Java 1.21 and native Main: border commands, permissions and checkpoint restart.
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
const scratch = await mkdtemp(path.join(path.dirname(output), 'worldborder-command-')); 
const sha = (bytes, kind = 'sha256') => createHash(kind).update(bytes).digest('hex');
const lock = JSON.parse(await readFile('fixtures/source-lock.json', 'utf8')).oracle;
const jarBytes = await readFile(jar);
assert.equal(sha(jarBytes, 'sha1'), lock.sha1); assert.equal(jarBytes.length, lock.size);
assert.ok((await readFile(eulaArg, 'utf8')).split(/\r?\n/).includes('eula=true'));
const result = {ok: false, native_commit: execFileSync('git', ['rev-parse', 'HEAD'], {encoding: 'utf8'}).trim(),
  native_src_tree: execFileSync('git', ['rev-parse', 'HEAD:src'], {encoding: 'utf8'}).trim(),
  native_binary_sha256: sha(await readFile(binary)), java_jar_sha1: lock.sha1, traces: {},
  limits: ['Selected single-overworld command behavior, not border damage gameplay or broad vanilla parity.',
    'Exact controlled-clock interpolation is covered separately; live size transitions use target/remaining invariants.',
    'World-border packets and feedback are exact for commands issued from a static size. Live interpolation completion allows 500ms scheduling drift.']};
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
  const token = `BORDER_FLOW_${++marker}`, offset = server.log().length;
  for (const text of commands) server.command(text);
  server.command('say ' + token);
  await until(() => server.log().slice(offset).includes(token), 'Java console barrier');
}
function long(value) {
  if (Array.isArray(value)) return Number(BigInt.asIntN(64, (BigInt(value[0] >>> 0) << 32n) | BigInt(value[1] >>> 0)));
  return Number(value);
}
function borderTree(packet) {
  const nodes = packet.nodes, root = nodes[packet.rootIndex];
  const first = root.children.find(id => nodes[id].extraNodeData?.name === 'worldborder');
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
async function connect(port, username = "BorderProbe") {
  const client = mc.createClient({host: '127.0.0.1', port, username, auth: 'offline',
    version: '1.21', keepAlive: false, hideErrors: true});
  let error, ready = false, closing = false, ended = false, tree, age, day;
  const messages = [], completions = [], borders = [];
  client.on('error', value => {error = value;});
  client.on('end', () => {ended = true; if (!closing) error = Error('Unexpected disconnect');});
  client.on('kick_disconnect', () => {error = Error('Probe kicked');});
  client.on('keep_alive', packet => client.write('keep_alive', packet));
  client.on('chunk_batch_finished', () => client.write('chunk_batch_received', {chunksPerTick: 16}));
  client.on('position', packet => {client.write('teleport_confirm', {teleportId: packet.teleportId}); ready = true;});
  client.on('declare_commands', packet => {tree = borderTree(packet);});
  client.on('update_time', packet => {age = long(packet.age); day = long(packet.time);});
  client.on('tab_complete', packet => completions.push(packet));
  client.on('packet', (packet, meta, buffer) => {
    if (meta.name === 'initialize_world_border' || meta.name.startsWith('world_border_')) borders.push({name:meta.name,packet,hex:buffer.subarray(1).toString('hex')});
    if (meta.name === 'system_chat') messages.push({component: nbt.simplify(packet.content), hex: buffer.subarray(1).toString('hex')});
  });
  async function wait(test, label) {await until(() => {if (error) throw error; return test();}, label);}
  await wait(() => ready && tree !== undefined && day !== undefined && borders.length, 'PLAY admission');
  return {client, messages, borders, get tree() {return tree;}, get age() {return age;}, get day() {return day;},
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
  const seed=path.join(scratch,'seed');await mkdir(seed);await copyFile(eulaArg,path.join(seed,'eula.txt'));
  const properties='server-ip=127.0.0.1\nserver-port=25681\nonline-mode=false\nenforce-secure-profile=false\ngamemode=creative\nlevel-type=minecraft:flat\ngenerate-structures=false\nview-distance=2\nsimulation-distance=2\n';
  await writeFile(path.join(seed,'server.properties'),properties);await start('java',seed,25681);
  await javaCommands(['gamerule doDaylightCycle false','gamerule doWeatherCycle false','gamerule doMobSpawning false','gamerule randomTickSpeed 0','gamerule spawnRadius 0','setworldspawn 8 65 8','forceload add -48 -48 48 48','save-all flush']);await stop();
  for(const kind of ['java','native']){
    const folder=path.join(scratch,kind);await mkdir(folder);await cp(path.join(seed,'world'),path.join(folder,'world'),{recursive:true});await localKeys(folder);await copyFile(eulaArg,path.join(folder,'eula.txt'));await writeFile(path.join(folder,'server.properties'),properties);
    const port=kind==='java'?25681:25682;await start(kind,folder,port);
    const probe=await connect(port);assert.equal(probe.tree,null);await probe.denied('worldborder set 32');assert.equal(probe.borders.length,1);
    server.command('op BorderProbe');await probe.wait(()=>probe.tree!==null,'op graph');
    const observer=await connect(port,'BorderOther');server.command('op BorderOther');await observer.wait(()=>observer.tree!==null,'observer op');
    const trace={tree:probe.tree,denied_nonop:true,success:[],updates:[],completion:[]};
    const queries=()=>observer.messages.filter(m=>hasTranslation(m.component,'commands.worldborder.get')).length;const count=queries();trace.success.push(await probe.command('worldborder get','commands.worldborder.get'));await pause(150);assert.equal(queries(),count);trace.query_private=true;
    for(const [command,key,name] of [
      ['worldborder set 64','commands.worldborder.set.immediate','world_border_size'],
      ['worldborder center 12 -8','commands.worldborder.center.success','world_border_center'],
      ['worldborder warning time 7','commands.worldborder.warning.time.success','world_border_warning_delay'],
      ['worldborder warning distance 9','commands.worldborder.warning.distance.success','world_border_warning_reach'],
      ['worldborder damage amount 0.125','commands.worldborder.damage.amount.success',null],
      ['worldborder damage buffer 2.675','commands.worldborder.damage.buffer.success',null]]){
      const offset=probe.borders.length,other=observer.borders.length;
      trace.success.push(await probe.command(command,key));
      if(name){await probe.wait(()=>probe.borders.length>offset,'border update');await observer.wait(()=>observer.borders.length>other,'observer border');assert.equal(probe.borders.at(-1).name,name);assert.deepEqual(probe.borders.at(-1),observer.borders.at(-1));trace.updates.push(probe.borders.at(-1));}
      else{await pause(100);assert.equal(probe.borders.length,offset);}
    }
    for(const [command,key]of [['worldborder set 64','commands.worldborder.set.failed.nochange'],['worldborder set 0','commands.worldborder.set.failed.small'],['worldborder center 30000000 0','commands.worldborder.set.failed.far']])trace.success.push(await probe.command(command,key));
    const before=probe.borders.length;for(const command of ['worldborder set 64 trailing','worldborder set 12 -1'])await probe.denied(command);assert.equal(probe.borders.length,before);trace.invalid_no_mutation=true;
    for(const [i,text]of ['worldborder ','worldborder center ','worldborder warning ','w'].entries()){
      const c=await probe.complete(text,i+1);if(text==='w'){c.matches=c.matches.filter(v=>['weather','whitelist','worldborder'].includes(v));assert.deepEqual(c.matches,['weather','whitelist','worldborder']);}trace.completion.push({text,...c});
    }
    trace.success.push(await probe.command('worldborder set 128 2','commands.worldborder.set.grow'));await probe.wait(()=>probe.borders.at(-1).name==='world_border_lerp_size','lerp packet');trace.updates.push(probe.borders.at(-1));await pause(2500);trace.success.push(await probe.command('worldborder get','commands.worldborder.get'));
    server.command('worldborder center ~ ~');await probe.wait(()=>probe.borders.at(-1).name==='world_border_center','console center');trace.updates.push(probe.borders.at(-1));trace.console=true;
    await observer.close();await probe.close();const rejoined=await connect(port);trace.rejoin=rejoined.borders[0];assert.equal(trace.rejoin.packet.newDiameter,128);await rejoined.close();await stop();
    const saved=nbt.simplify((await nbt.parse(await readFile(path.join(folder,'world/level.dat')))).parsed).Data;
    trace.saved=Object.fromEntries(['BorderCenterX','BorderCenterZ','BorderSize','BorderSafeZone','BorderDamagePerBlock','BorderWarningBlocks','BorderWarningTime','BorderSizeLerpTime','BorderSizeLerpTarget'].map(k=>[k,saved[k]]));
    await start(kind,folder,port);const restarted=await connect(port);trace.restart=restarted.borders[0];assert.deepEqual(trace.restart,trace.rejoin);server.command('worldborder set 256 60');await restarted.wait(()=>restarted.borders.at(-1).name==='world_border_lerp_size','imported long lerp');await restarted.close();await stop();
    const savedLerp=nbt.simplify((await nbt.parse(await readFile(path.join(folder,'world/level.dat')))).parsed).Data;
    assert.equal(savedLerp.BorderSizeLerpTarget,256);assert.ok(long(savedLerp.BorderSizeLerpTime)>0&&long(savedLerp.BorderSizeLerpTime)<=60000);
    await start(kind,folder,port);const lerped=await connect(port);assert.equal(lerped.borders[0].packet.newDiameter,256);assert.ok(long(lerped.borders[0].packet.speed)>0&&long(lerped.borders[0].packet.speed)<=long(savedLerp.BorderSizeLerpTime));await lerped.close();await stop();trace.imported_lerp_restart=true;result.traces[kind]=trace;
  }
  assert.deepEqual(result.traces.native,result.traces.java,'selected Java/native behavior differs');
  const flat=path.join(scratch,'native-flat');await mkdir(flat);await localKeys(flat);await start('native',flat,25682,false);
  const probe=await connect(25682);server.command('op BorderProbe');await probe.wait(()=>probe.tree!==null,'native op');
  await probe.command('worldborder set 64','commands.worldborder.set.immediate');await probe.command('worldborder set 128 60','commands.worldborder.set.grow');await probe.close();await stop();
  const checkpoint=JSON.parse(await readFile(path.join(flat,'world.json'),'utf8'));assert.equal(checkpoint.border.size_lerp_target,128);assert.ok(checkpoint.border.size_lerp_time>0&&checkpoint.border.size_lerp_time<=60000);
  await start('native',flat,25682,false);const restored=await connect(25682);assert.equal(restored.borders[0].packet.newDiameter,128);assert.ok(long(restored.borders[0].packet.speed)>0&&long(restored.borders[0].packet.speed)<=checkpoint.border.size_lerp_time);result.native_lerp_checkpoint={saved:checkpoint.border,restored:restored.borders[0].packet};await restored.close();await stop();
  const legacy=path.join(scratch,'native-legacy');await mkdir(legacy);await localKeys(legacy);
  // A pre-border, non-authoritative checkpoint; editing a protected current
  // checkpoint would correctly trip the existing world/entity digest guard.
  await writeFile(path.join(legacy,'world.json'),JSON.stringify({format:'mc121-flat-diffs-v1',minecraft:'1.21',changes:[]}));
  await start('native',legacy,25682,false);const old=await connect(25682);assert.equal(old.borders[0].packet.newDiameter,59999968);result.legacy_checkpoint_default=true;await old.close();await stop();result.ok=true;
}catch(error){result.error=error.stack;process.exitCode=1;}
finally{try{await stop();}catch(error){result.shutdown_error=error.message;result.ok=false;process.exitCode=1;}result.completed_at=new Date().toISOString();await writeFile(output,JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify({ok:result.ok,report:output,local_diagnostics:scratch,error:result.error}));}
