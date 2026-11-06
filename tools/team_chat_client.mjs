// SPDX-License-Identifier: GPL-2.0-only
// Same three-player scenario for Java and native. Console requests go to the parent runner.
import mc from 'minecraft-protocol';
import nbt from 'prismarine-nbt';
import assert from 'node:assert/strict';
import {writeFile} from 'node:fs/promises';
const [port, output] = process.argv.slice(2);
const result = {ok: false, phases: []};
const clients = [];
let phase, failure;
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
const canonical = value => Array.isArray(value) ? value.map(canonical)
  : value && typeof value === 'object' ? Object.fromEntries(Object.keys(value).sort().map(k => [k, canonical(value[k])])) : value;
const component = value => value == null ? null : canonical(nbt.simplify(value));
async function until(test, label) {
  const deadline = Date.now() + 30000;
  while (!test()) {
    if (failure) throw failure;
    if (Date.now() > deadline) throw Error('Timeout: ' + label);
    await pause(20);
  }
}
function consoleCommand(command) { process.stdout.write('CONSOLE:' + JSON.stringify(command) + '\n'); }
function graphSlice(packet) {
  const nodes = packet.nodes;
  function node(index) {
    const n = nodes[index];
    return {name: n.extraNodeData?.name, kind: n.flags.command_node_type,
      executable: !!n.flags.has_command, parser: n.extraNodeData?.parser ?? null,
      redirect: n.flags.has_redirect_node ? nodes[n.redirectNode].extraNodeData.name : null,
      children: n.children.map(node)};
  }
  return nodes[packet.rootIndex].children.filter(i => ['teammsg', 'tm'].includes(nodes[i].extraNodeData?.name))
    .map(node).sort((a, b) => a.name.localeCompare(b.name));
}
async function connect(name) {
  const client = mc.createClient({host: '127.0.0.1', port: Number(port), username: name,
    version: '1.21', auth: 'offline', keepAlive: false, hideErrors: true});
  const entry = {name, client, ready: false, systems: [], teams: [], tree: null}; clients.push(entry);
  client.on('error', e => { failure = e; });
  client.on('kick_disconnect', () => { failure = Error('Kicked: ' + name); });
  client.on('keep_alive', p => client.write('keep_alive', p));
  client.on('chunk_batch_finished', () => client.write('chunk_batch_received', {chunksPerTick: 16}));
  client.on('position', p => { client.write('teleport_confirm', {teleportId: p.teleportId}); entry.ready = true; });
  client.on('declare_commands', p => { entry.tree = graphSlice(p); });
  client.on('teams', p => entry.teams.push(p));
  client.on('system_chat', p => entry.systems.push(component(p.content)));
  client.on('packet', (p, meta) => {
    if (!phase || !['player_chat', 'profileless_chat'].includes(meta.name)) return;
    const player = meta.name === 'player_chat';
    phase.messages[name].push({kind: meta.name, content: player ? p.plainMessage : component(p.message),
      type: canonical(p.type), name: component(player ? p.networkName : p.name),
      target: component(player ? p.networkTargetName : p.target),
      ...(player ? {sender: p.senderUuid, has_signature: !!p.signature, filter: p.filterType} : {})});
  });
  await until(() => entry.ready && entry.tree, 'join ' + name);
  assert.equal(entry.tree.length, 2, 'teammsg and tm must be in non-op tree');
  return entry;
}
async function barrier(entry) {
  const count = entry.systems.length;
  await paced(entry, 'chat_command', {command: 'list'});
  await until(() => entry.systems.slice(count).some(c => JSON.stringify(c).includes('commands.list.players')), 'list barrier ' + entry.name);
}
async function paced(entry, name, payload) {
  // Exercise the unchanged spam owners without exceeding Java's normal budget.
  await pause(Math.max(0, 800 - (Date.now() - (entry.lastCommand || 0))));
  entry.lastCommand = Date.now();
  entry.client.write(name, payload);
}
function begin(name) {
  phase = {name, messages: Object.fromEntries(clients.map(c => [c.name, []]))};
  result.phases.push(phase); return phase;
}
async function delivered(expected) {
  await until(() => Object.entries(expected).every(([name, count]) => phase.messages[name].length >= count), 'message delivery');
  for (const entry of clients) await barrier(entry);
  for (const entry of clients) assert.equal(phase.messages[entry.name].length, expected[entry.name] || 0,
    'audience ' + phase.name + ': ' + entry.name);
  phase = null;
}
try {
  const alice = await connect('TeamAlice'), bob = await connect('TeamBob'), cara = await connect('OtherCara');
  result.tree = alice.tree;
  begin('no-team');
  await paced(alice, 'chat_command', {command: 'teammsg before'});
  await until(() => alice.systems.some(c => JSON.stringify(c).includes('commands.teammsg.failed.noteam')), 'no-team failure');
  result.no_team_error = alice.systems.find(c => JSON.stringify(c).includes('commands.teammsg.failed.noteam'));
  await delivered({});
  for (const command of ['team add red', 'team modify red displayName {"text":"Red","bold":true}',
    'team modify red color red', 'team modify red prefix {"text":"[R] "}',
    'team join red TeamAlice', 'team join red TeamBob', 'team add blue', 'team join blue OtherCara']) consoleCommand(command);
  await until(() => cara.teams.some(p => p.team === 'blue' && p.players?.includes('OtherCara')), 'team setup');
  begin('unsigned-literal');
  await paced(alice, 'chat_command', {command: 'teammsg   "hello  team"  '});
  await delivered({TeamAlice: 1, TeamBob: 1});
  function settings(chatFlags) {
    bob.client.write('settings', {locale: 'en_US', viewDistance: 2, chatFlags, chatColors: true,
      skinParts: 127, mainHand: 1, enableTextFiltering: true, enableServerListing: true});
  }
  settings(1); await barrier(bob);
  begin('system-visibility-recipient');
  await paced(alice, 'chat_command', {command: 'teammsg only-full-chat'});
  await delivered({TeamAlice: 1});
  settings(0); await barrier(bob);
  begin('signed-command-envelope');
  await paced(alice, 'chat_command_signed', {command: 'tm signed  hello', timestamp: BigInt(Date.now()), salt: 0n,
    argumentSignatures: [], messageCount: 0, acknowledged: Buffer.alloc(3)});
  await delivered({TeamAlice: 1, TeamBob: 1});
  begin('console-denied-and-membership-change');
  consoleCommand('teammsg console-no-entity');
  consoleCommand('team join blue TeamBob');
  await until(() => bob.teams.some(p => p.team === 'blue' && p.players?.includes('TeamBob')), 'membership change after console failure');
  await delivered({});
  begin('alias-after-membership-change');
  await paced(alice, 'chat_command', {command: 'tm only-alice'});
  await delivered({TeamAlice: 1});
  begin('ordinary-chat-still-public');
  await paced(alice, 'chat_message', {message: 'public after team', timestamp: BigInt(Date.now()), salt: 0n,
    signature: undefined, offset: 0, acknowledged: Buffer.alloc(3)});
  await delivered({TeamAlice: 1, TeamBob: 1, OtherCara: 1});
  result.ok = true;
} catch (e) { result.error = e.message; process.exitCode = 1; }
finally {
  phase = null;
  for (const {client} of clients) client.end();
  await writeFile(output, JSON.stringify(result, null, 2) + '\n');
}
