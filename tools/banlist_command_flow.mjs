// SPDX-License-Identifier: GPL-2.0-only
// Real, unchanged Java 1.21 and native Main: banlist queries, permissions, live owners and restart.
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
const scratch = await mkdtemp(path.join(path.dirname(output), 'banlist-command-'));
const sha = (bytes, kind = 'sha256') => createHash(kind).update(bytes).digest('hex');
const lock = JSON.parse(await readFile('fixtures/source-lock.json', 'utf8')).oracle;
const jarBytes = await readFile(jar);
assert.equal(sha(jarBytes, 'sha1'), lock.sha1); assert.equal(jarBytes.length, lock.size);
assert.ok((await readFile(eulaArg, 'utf8')).split(/\r?\n/).includes('eula=true'));
const result = {ok: false, native_commit: execFileSync('git', ['rev-parse', 'HEAD'], {encoding: 'utf8'}).trim(),
  native_src_tree: execFileSync('git', ['rev-parse', 'HEAD:src'], {encoding: 'utf8'}).trim(),
  native_binary_sha256: sha(await readFile(binary)), java_jar_sha1: lock.sha1, traces: {},
  limits: ['Existing access-list owner and its expiry behavior; no profile-cache/date/locale or HashMap bucket parity claim.',
    'Entry encounter order within player/IP maps is excluded across languages; membership and each component byte string are exact. Player entries must precede IP entries.',
    'Syntax-error text and packet timing are excluded. Query payloads, filtered contents, command subtree and completion are exact.']};
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
  const token = `BANLIST_FLOW_${++marker}`, offset = server.log().length;
  for (const text of commands) server.command(text);
  server.command('say ' + token);
  await until(() => server.log().slice(offset).includes(token), 'Java console barrier');
}
function long(value) {
  if (Array.isArray(value)) return Number(BigInt.asIntN(64, (BigInt(value[0] >>> 0) << 32n) | BigInt(value[1] >>> 0)));
  return Number(value);
}
function banlistTree(packet) {
  const nodes = packet.nodes, root = nodes[packet.rootIndex];
  const first = root.children.find(id => nodes[id].extraNodeData?.name === 'banlist');
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
async function connect(port, username = "BanlistProbe") {
  const client = mc.createClient({host: '127.0.0.1', port, username, auth: 'offline',
    version: '1.21', keepAlive: false, hideErrors: true});
  let error, ready = false, closing = false, ended = false, tree, age, day;
  const messages = [], completions = [];
  client.on('error', value => {error = value;});
  client.on('end', () => {ended = true; if (!closing) error = Error('Unexpected disconnect');});
  client.on('kick_disconnect', () => {error = Error('Probe kicked');});
  client.on('keep_alive', packet => client.write('keep_alive', packet));
  client.on('chunk_batch_finished', () => client.write('chunk_batch_received', {chunksPerTick: 16}));
  client.on('position', packet => {client.write('teleport_confirm', {teleportId: packet.teleportId}); ready = true;});
  client.on('declare_commands', packet => {tree = banlistTree(packet);});
  client.on('update_time', packet => {age = long(packet.age); day = long(packet.time);});
  client.on('tab_complete', packet => completions.push(packet));
  client.on('packet', (packet, meta, buffer) => {
    if (meta.name === 'system_chat') messages.push({component: nbt.simplify(packet.content), hex: buffer.subarray(1).toString('hex')});
  });
  async function wait(test, label) {await until(() => {if (error) throw error; return test();}, label);}
  await wait(() => ready && tree !== undefined && day !== undefined , 'PLAY admission');
  return {client, messages, get tree() {return tree;}, get age() {return age;}, get day() {return day;},
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
async function query(probe,text,count){
  const start=probe.messages.length;probe.client.write('chat_command',{command:text});
  const selected=()=>probe.messages.slice(start).filter(m=>['commands.banlist.none','commands.banlist.list','commands.banlist.entry'].some(k=>hasTranslation(m.component,k)));
  await probe.wait(()=>selected().length>=count+1,text);
  const rows=selected();assert.equal(rows.length,count+1);return rows;
}
function normalized(rows){return [rows[0],...rows.slice(1).sort((a,b)=>JSON.stringify(a.component).localeCompare(JSON.stringify(b.component)))];}
const players=[{uuid:'00000000-0000-0000-0000-000000000001',name:'BannedOne',created:'2020-01-01 00:00:00 +0000',source:'Operator',expires:'forever',reason:'Reason "one" ☃'},
 {uuid:'00000000-0000-0000-0000-000000000002',name:'ExpiredOne',created:'2020-01-01 00:00:00 +0000',source:'Earlier',expires:'2020-01-01 00:00:01 +0000',reason:'Expired reason'}];
try{
  for(const kind of ['java','native']){
    const folder=path.join(scratch,kind);await mkdir(folder);await localKeys(folder);await copyFile(eulaArg,path.join(folder,'eula.txt'));
    await writeFile(path.join(folder,'server.properties'),'server-ip=127.0.0.1\nserver-port=25691\nonline-mode=false\nenforce-secure-profile=false\ngamemode=creative\nlevel-type=minecraft:flat\ngenerate-structures=false\nview-distance=2\nsimulation-distance=2\n');
    await writeFile(path.join(folder,'banned-players.json'),JSON.stringify(players));
    await writeFile(path.join(folder,'banned-ips.json'),JSON.stringify([{ip:'192.0.2.7',created:players[0].created,source:'Console',expires:'forever',reason:'IP reason'}]));
    const port=kind==='java'?25691:25692;await start(kind,folder,port,false);
    const probe=await connect(port);assert.equal(probe.tree,null);await probe.denied('banlist');
    server.command('op BanlistProbe');await probe.wait(()=>probe.tree!==null,'op graph');
    const observer=await connect(port,'BanlistOther');server.command('op BanlistOther');await observer.wait(()=>observer.tree!==null,'observer op');
    const trace={tree:probe.tree,denied_nonop:true,queries:[],completion:[]};
    const observerBanCount=()=>observer.messages.filter(m=>['commands.banlist.none','commands.banlist.list','commands.banlist.entry'].some(k=>hasTranslation(m.component,k))).length;
    const before=observerBanCount();
    for(const [command,count]of [['banlist',3],['banlist players',2],['banlist ips',1]]){
      const rows=await query(probe,command,count);
      if(command==='banlist'){assert.equal(rows.at(-1).component.with[0],'192.0.2.7');assert.ok(rows.slice(1,3).some(m=>m.component.with[0]==='ExpiredOne'));}
      trace.queries.push({command,messages:normalized(rows)});
    }
    await pause(150);assert.equal(observerBanCount(),before);trace.private=true;trace.query_retains_expired=true;
    for(const text of ['banlist invalid','banlist players trailing','banlist PLAYERS','banlist  players'])await probe.denied(text);
    trace.invalid_no_mutation=normalized(await query(probe,'banlist',3));
    for(const [i,text]of ['banlist ','banlist i','/banlist p','banlist ips ','b'].entries()){
      const c=await probe.complete(text,i+1);if(text==='b')c.matches=c.matches.filter(v=>['ban','ban-ip','banlist'].includes(v));trace.completion.push({text,...c});
    }
    const offset=server.log().length;server.command('banlist ips');
    await until(()=>{const log=server.log().slice(offset);return kind==='java'?log.includes('192.0.2.7 was banned by Console: IP reason'):log.includes('commands.banlist.entry')&&log.includes('192.0.2.7')&&log.includes('IP reason');},'console query');trace.console=true;
    server.command('pardon-ip 192.0.2.7');
    if(kind==='java')await javaCommands([]);else await until(()=>server.log().includes('commands.pardonip.success'),'pardon IP');
    trace.removed_ip=await query(probe,'banlist ips',0);
    await observer.close();await probe.close();await stop();
    await start(kind,folder,port,false);const restarted=await connect(port);await restarted.wait(()=>restarted.tree!==null,'retained operator');
    trace.restart_players=normalized(await query(restarted,'banlist players',2));trace.restart_ips=await query(restarted,'banlist ips',0);
    await restarted.close();await stop();result.traces[kind]=trace;
  }
  assert.deepEqual(result.traces.native,result.traces.java,'selected Java/native banlist behavior differs');result.ok=true;
}catch(error){result.error=error.stack;process.exitCode=1;}
finally{try{await stop();}catch(error){result.shutdown_error=error.message;result.ok=false;process.exitCode=1;}result.completed_at=new Date().toISOString();await writeFile(output,JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify({ok:result.ok,report:output,local_diagnostics:scratch,error:result.error}));}
