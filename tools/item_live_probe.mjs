#!/usr/bin/env node
// Stock protocol-767 clients exercise the live item pipeline. A separate
// process adapter controls merge geometry; missing support always fails.
import mc from 'minecraft-protocol';
import minecraftData from 'minecraft-data';
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {createInterface} from 'node:readline';
import {resolve, dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {mkdir, readFile, writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
const option = (name, fallback) => {
  const index = args.indexOf(name);
  return index < 0 ? fallback : args[index + 1];
};
if (args.includes('--help')) {
  console.log(`node tools/item_live_probe.mjs [--scenario stock|merge|all] [--port 25573]
  [--server-binary target/debug/mc121-rust]
  [--java-jar /path/server-1.21.jar --java-server-dir /prepared/test/directory]
  [--label rust|java] [--wait-common-position]
  [--control-program /absolute/executable --control-args '["argument"]']

An external Java server may instead be supplied through --host/--port.
Prepared Java directories must already accept the EULA and configure offline
creative mode and the common flat test world. The probe does not accept it.
Merge/all require a separate JSONL process controller; no controller is FAIL.
Controller requests are documented in tools/item_live_probe_control.md.`);
  process.exit(0);
}
const scenario = option('--scenario', 'all');
assert.ok(['stock', 'merge', 'all'].includes(scenario), 'unknown scenario');
const label = option('--label', 'rust');
assert.match(label, /^[a-zA-Z0-9_-]+$/, 'unsafe report label');
const host = option('--host', '127.0.0.1');
const port = Number(option('--port', '25573'));
const binary = option('--server-binary', null);
const javaJar = option('--java-jar', null);
const controlProgram = option('--control-program', null);
assert.ok(!(binary && javaJar), 'choose one server launcher');
const md = minecraftData('1.21');
const stoneId = md.itemsByName.stone.id;
const itemType = md.entitiesByName.item.id;
// Original ahm$a.<clinit>: DROP_ALL_ITEMS ordinal3, DROP_ITEM ordinal4.
const DROP_ALL_ITEMS = 3;
const DROP_ITEM = 4;
const settings = {locale: 'en_us', viewDistance: 2, chatFlags: 0,
  chatColors: true, skinParts: 127, mainHand: 1,
  enableTextFiltering: false, enableServerListing: true};
const slot = count => ({itemCount: count, itemId: stoneId,
  addedComponentCount: 0, removedComponentCount: 0, components: [], removeComponents: []});
const pause = milliseconds => new Promise(done => setTimeout(done, milliseconds));
const clients = [];
let server, serverLog = '', controller, controllerLog = '', sequence = 0;
let controlId = 0;
const replies = new Map();
const report = {ok: false, scenario, label, protocol: 767, phase: 'starting',
  scope: 'live stock item drop, movement, pairing and pickup; controlled merge when selected',
  checks: {}, controls: [], not_covered: ['complete vanilla server parity', 'random trajectory byte equality']};
function phase(message) {
  report.phase = message;
  console.log(`WAIT ${message}`);
}
async function until(predicate, description, timeout = 15000) {
  phase(description);
  const deadline = Date.now() + timeout;
  while (!predicate()) {
    const error = clients.flatMap(client => client.errors)[0];
    if (error) throw error;
    if (server && server.exitCode !== null) throw new Error(`server exited: ${serverLog.slice(-3000)}`);
    if (Date.now() >= deadline) throw new Error(`FAIL: deadline waiting for ${description}`);
    await pause(20);
  }
}
function record(state, name, data) {
  const event = {sequence: ++sequence, name, data};
  state.events.push(event);
  if (name === 'spawn_entity' && data.type === itemType) {
    state.items.set(data.entityId, {id: data.entityId, uuid: data.objectUUID,
      position: {x: data.x, y: data.y, z: data.z}, spawn: data, removed: false,
      count: null, slot: null, onGround: false});
    state.spawns.push(event);
  }
  if (name === 'entity_metadata') {
    const item = state.items.get(data.entityId);
    const value = data.metadata.find(entry => entry.key === 8)?.value;
    if (item && value) { item.count = value.itemCount; item.slot = value; }
  }
  if (['rel_entity_move', 'entity_move_look', 'entity_teleport'].includes(name)) {
    const item = state.items.get(data.entityId);
    if (item) {
      if (name === 'entity_teleport') item.position = {x: data.x, y: data.y, z: data.z};
      else for (const [axis, field] of [['x', 'dX'], ['y', 'dY'], ['z', 'dZ']]) item.position[axis] += data[field] / 4096;
      item.onGround = data.onGround;
    }
  }
  if (name === 'entity_velocity') {
    const item = state.items.get(data.entityId);
    if (item) item.velocity = data.velocity;
  }
  if (name === 'entity_destroy') for (const id of data.entityIds) {
    const item = state.items.get(id);
    if (item) item.removed = true;
  }
  if (name === 'window_items' && data.windowId === 0) {
    state.inventory = structuredClone(data.items);
    state.authoritativeInventory = structuredClone(data.items);
    state.inventoryStateId = data.stateId;
    state.fullInventorySequence = event.sequence;
  }
  if (name === 'set_slot' && data.windowId === 0) {
    state.inventory[data.slot] = data.item;
    state.authoritativeInventory[data.slot] = structuredClone(data.item);
    state.inventoryStateId = data.stateId;
  }
}
async function connect(username) {
  const state = {username, id: null, join: null, position: null, inventory: [],
    authoritativeInventory: [], inventoryStateId: 0, fullInventorySequence: 0,
    chunks: new Set(), chunkBatches: 0, items: new Map(), spawns: [], events: [], errors: [], closing: false};
  const client = mc.createClient({host, port, username, auth: 'offline', version: '1.21',
    keepAlive: false, hideErrors: true, clientSettings: settings});
  state.client = client;
  clients.push(state);
  client.on('error', error => { if (!state.closing) state.errors.push(error); });
  client.on('end', reason => { if (!state.closing) state.errors.push(new Error(`${username} disconnected: ${reason}`)); });
  client.on('kick_disconnect', data => state.errors.push(new Error(`${username} kicked: ${JSON.stringify(data)}`)));
  client.on('keep_alive', data => client.write('keep_alive', data));
  client.on('chunk_batch_finished', () => {
    state.chunkBatches++;
    client.write('chunk_batch_received', {chunksPerTick: 16});
  });
  client.on('position', data => {
    state.position = {x: data.x, y: data.y, z: data.z, yaw: data.yaw, pitch: data.pitch};
    client.write('teleport_confirm', {teleportId: data.teleportId});
    client.write('position_look', {...state.position, onGround: true});
  });
  const captured = new Set(['spawn_entity', 'entity_metadata', 'entity_velocity',
    'rel_entity_move', 'entity_move_look', 'entity_teleport', 'entity_destroy',
    'collect', 'set_slot', 'window_items']);
  client.on('packet', (data, metadata) => {
    if (metadata.state !== 'play') return;
    if (metadata.name === 'login') { state.id = data.entityId; state.join = data; }
    if (metadata.name === 'map_chunk') state.chunks.add(`${data.x},${data.z}`);
    if (metadata.name === 'unload_chunk') state.chunks.delete(`${data.chunkX},${data.chunkZ}`);
    if (captured.has(metadata.name)) record(state, metadata.name, data);
  });
  await until(() => state.id !== null && state.position && state.inventory.length === 46 &&
    state.chunkBatches > 0 && state.chunks.size === 45,
    `PLAY, inventory and original forty-five-chunk view for ${username}`);
  assert.equal(state.join.worldState.gamemode, 'creative', 'probe requires creative mode');
  return state;
}
async function move(state, destination) {
  // Four-block steps avoid relying on disabled server movement checks.
  for (let count = 0; count < 50; count++) {
    if (state.closing) throw new Error('probe is closing');
    const current = state.position;
    const delta = {x: destination.x - current.x, y: destination.y - current.y, z: destination.z - current.z};
    const distance = Math.hypot(delta.x, delta.y, delta.z);
    if (distance < 0.001) {
      state.position = {...current, yaw: 0, pitch: 0};
      state.client.write('position_look', {...state.position, onGround: true});
      await pause(120);
      return;
    }
    const scale = Math.min(1, 4 / distance);
    state.position = {x: current.x + delta.x * scale, y: current.y + delta.y * scale,
      z: current.z + delta.z * scale, yaw: 0, pitch: 0};
    state.client.write('position_look', {...state.position, onGround: true});
    await pause(120);
  }
  throw new Error('FAIL: movement repeatedly corrected or destination unreachable');
}
const countStone = state => state.inventory.reduce((sum, item) =>
  sum + (item?.itemId === stoneId ? item.itemCount : 0), 0);
const matching = (state, predicate) => state.events.find(predicate);
async function readbackInventory(state, description) {
  // A raw protocol client has no vanilla menu prediction. A deliberately stale
  // harmless outside-menu click asks the original server for a full snapshot.
  // Never use our predicted display as evidence that the server accepted it.
  const before = state.fullInventorySequence;
  state.client.write('window_click', {windowId: 0,
    stateId: (state.inventoryStateId + 32767) & 32767,
    slot: -1, mouseButton: 0, mode: 0, changedSlots: [], cursorItem: {itemCount: 0}});
  await until(() => state.fullInventorySequence > before, description);
  return state.authoritativeInventory;
}
async function putHeld(state, count) {
  state.client.write('held_item_slot', {slotId: 0});
  state.inventory[36] = slot(count); // Vanilla creative local prediction.
  state.client.write('set_creative_slot', {slot: 36, item: slot(count)});
  const inventory = await readbackInventory(state, `authoritative creative stone ${count} readback`);
  assert.equal(inventory[36]?.itemCount, count, 'server creative count');
  assert.equal(inventory[36]?.itemId, stoneId, 'server creative item');
  report.checks.authoritative_creative_inventory_readback = true;
}
async function drop(state, observer, action, expected, remaining, departure) {
  const before = new Set(observer.items.keys());
  state.inventory[36] = remaining ? slot(remaining) : {itemCount: 0}; // Vanilla Q prediction.
  state.client.write('block_dig', {status: action, location: {x: 0, y: 0, z: 0}, face: 0, sequence: 0});
  // Queue departure immediately after the drop packet, before waiting for any
  // server reply; delay expiration cannot be hidden by a long spawn wait.
  const departed = move(state, departure).catch(error => error);
  await until(() => [...observer.items.values()].some(item => !before.has(item.id) && item.count === expected),
    `${action === DROP_ITEM ? 'DROP_ITEM' : 'DROP_ALL_ITEMS'} item spawn and Slot8 count${expected}`);
  const item = [...observer.items.values()].find(item => !before.has(item.id) && item.count === expected);
  await until(() => state.items.get(item.id)?.count === expected &&
    clients.every(client => matching(client, event => event.name === 'entity_velocity' && event.data.entityId === item.id)),
    `shared item${item.id} pairing and velocity for both clients`);
  const inventory = await readbackInventory(state, `authoritative selected count ${remaining} after drop`);
  assert.equal(inventory[36]?.itemCount || 0, remaining, 'server count after drop');
  report.checks.authoritative_drop_inventory_readback = true;
  for (const client of clients) {
    const spawn = client.spawns.find(event => event.data.entityId === item.id);
    const metadata = client.events.find(event => event.name === 'entity_metadata' && event.data.entityId === item.id);
    const velocity = client.events.find(event => event.name === 'entity_velocity' && event.data.entityId === item.id);
    assert.ok(spawn && metadata && velocity && spawn.sequence < metadata.sequence && metadata.sequence < velocity.sequence,
      'pairing order must be AddEntity, cached metadata, velocity');
  }
  const departureError = await departed;
  if (departureError) throw departureError;
  return item;
}
async function createSeparatedDrops(observer, dropper, base) {
  await move(observer, {x: base.x - 5, y: base.y, z: base.z});
  await move(dropper, base);
  await putHeld(dropper, 8);
  const first = await drop(dropper, observer, DROP_ITEM, 1, 7,
    {x: base.x + 4, y: base.y, z: base.z});
  const second = await drop(dropper, observer, DROP_ALL_ITEMS, 7, 0,
    {x: base.x + 8, y: base.y, z: base.z});
  return [first, second];
}
async function pickup(observer, item, count) {
  const inventoryBefore = countStone(observer);
  const starts = new Map(clients.map(client => [client, client.events.length]));
  const position = observer.items.get(item.id).position;
  await move(observer, position);
  for (const client of clients) {
    await until(() => client.events.slice(starts.get(client)).some(event => event.name === 'collect' &&
      event.data.collectedEntityId === item.id && event.data.collectorEntityId === observer.id && event.data.pickupItemCount === count),
      `collect count${count} for item${item.id} at ${client.username}`);
    await until(() => client.events.slice(starts.get(client)).some(event => event.name === 'entity_destroy' && event.data.entityIds.includes(item.id)),
      `remove collected item${item.id} at ${client.username}`);
    const collect = client.events.slice(starts.get(client)).find(event => event.name === 'collect' && event.data.collectedEntityId === item.id);
    const removed = client.events.slice(starts.get(client)).find(event => event.name === 'entity_destroy' && event.data.entityIds.includes(item.id));
    assert.ok(collect.sequence < removed.sequence, 'collect must precede item removal');
  }
  await until(() => countStone(observer) === inventoryBefore + count, 'private pickup inventory synchronization');
  await readbackInventory(observer, 'authoritative pickup inventory readback');
  assert.equal(countStone(observer), inventoryBefore + count, 'server pickup count');
  report.checks.authoritative_pickup_inventory_readback = true;
}
async function stock(observer, dropper, base) {
  const drops = await createSeparatedDrops(observer, dropper, base);
  report.checks.drop_item_and_drop_all_inventory_and_pairing = true;
  for (const item of drops) await until(() => observer.items.get(item.id).onGround &&
    observer.events.some(event => ['rel_entity_move', 'entity_move_look', 'entity_teleport'].includes(event.name) &&
      event.data.entityId === item.id && (event.data.dY < 0 || event.data.y < item.spawn.y)),
    `gravity and ground collision for item${item.id}`, 8000);
  report.checks.item_gravity_and_ground = true;
  const watched = drops[0];
  const spawnsBefore = observer.spawns.filter(event => event.data.entityId === watched.id).length;
  const removeBefore = observer.events.length;
  await move(observer, {x: base.x - 40, y: base.y, z: base.z});
  await until(() => observer.events.slice(removeBefore).some(event => event.name === 'entity_destroy' && event.data.entityIds.includes(watched.id)),
    'item leaves observer tracking range');
  await move(observer, {x: base.x - 5, y: base.y, z: base.z});
  await until(() => observer.spawns.filter(event => event.data.entityId === watched.id).length > spawnsBefore &&
    observer.items.get(watched.id)?.count === 1, 'stationary item re-pairs with same ID and cached Slot8');
  report.checks.stationary_item_view_leave_and_pair = true;
  // The bounded traversal keeps both players away from the drops. Pickup must
  // then succeed through the actual connection-phase touch and inventory path.
  await pickup(observer, drops[0], 1);
  await pickup(observer, drops[1], 7);
  report.checks.collect_before_remove_and_private_inventory = true;
}
async function control(operation, fields = {}) {
  if (!controlProgram) {
    phase('deterministic merge controller unavailable');
    throw new Error('FAIL: --control-program is required for deterministic merge; no merge check was skipped');
  }
  if (!controller) {
    controller = spawn(controlProgram, JSON.parse(option('--control-args', '[]')),
      {cwd: root, shell: false, stdio: ['pipe', 'pipe', 'pipe']});
    controller.on('error', error => replies.set('failure', error));
    controller.stdin.on('error', error => replies.set('failure', error));
    controller.stderr.on('data', data => { controllerLog += data; });
    createInterface({input: controller.stdout}).on('line', line => {
      try { const value = JSON.parse(line); if (value.id !== undefined) replies.set(value.id, value); }
      catch (error) { replies.set('failure', new Error(`invalid controller JSONL: ${line}`)); }
    });
  }
  const id = ++controlId;
  const request = {id, protocol: 1, op: operation, platform: label, server: {host, port}, ...fields};
  report.controls.push({request});
  controller.stdin.write(JSON.stringify(request) + '\n');
  await until(() => replies.has(id) || replies.has('failure') || controller.exitCode !== null,
    `controller acknowledgement ${operation}`, 10000);
  if (replies.has('failure')) throw replies.get('failure');
  const reply = replies.get(id);
  assert.ok(reply?.ok === true, `controller rejected ${operation}: ${JSON.stringify(reply)}`);
  report.controls.at(-1).reply = reply;
  return reply.result;
}
async function merge(observer, dropper, base) {
  const hello = await control('hello', {required: ['prepare_merge', 'allow_pickup']});
  assert.ok(['prepare_merge', 'allow_pickup'].every(name => hello?.capabilities?.includes(name)), 'controller lacks required operations');
  const drops = await createSeparatedDrops(observer, dropper, base);
  const entities = drops.map(item => ({entityId: item.id, uuid: item.uuid, count: item.count}));
  const prepared = await control('prepare_merge', {entities, position: {x: base.x + 4, y: base.y + 1, z: base.z + 3},
    velocity: {x: 0, y: 0, z: 0}, pickupDelay: 200, age: 0});
  assert.equal(prepared?.applied, true, 'controller did not confirm real merge preparation');
  const source = drops[0], survivor = drops[1];
  for (const client of clients) {
    await until(() => client.items.get(source.id)?.removed && client.items.get(survivor.id)?.count === 8,
      `deterministic merge source1 into destination7 at ${client.username}`, 8000);
    const removed = client.events.find(event => event.name === 'entity_destroy' && event.data.entityIds.includes(source.id));
    const metadata = client.events.find(event => event.name === 'entity_metadata' && event.data.entityId === survivor.id &&
      event.data.metadata.some(entry => entry.key === 8 && entry.value.itemCount === 8));
    assert.ok(removed.sequence < metadata.sequence, 'merge source removal must precede destination metadata');
  }
  report.checks.controlled_merge_packet_order = true;
  const allowed = await control('allow_pickup', {entities: [{entityId: survivor.id, uuid: survivor.uuid}], pickupDelay: 0});
  assert.equal(allowed?.applied, true, 'controller did not confirm pickup delay mutation');
  await pickup(observer, survivor, 8);
  report.checks.merged_item_collect_and_inventory = true;
}

try {
  if ((scenario === 'merge' || scenario === 'all') && !controlProgram) {
    report.unavailable_support = ['deterministic merge process controller'];
    if (scenario === 'merge') {
      phase('deterministic merge controller unavailable');
      throw new Error('FAIL: --control-program is required for the merge scenario; no server was started and no check was skipped');
    }
  }
  if (binary) {
    server = spawn(resolve(root, binary), ['--allow-offline', '--port', String(port), '--view-distance', '2',
      '--world', `runtime/item-probe-${label}-${process.pid}.json`], {cwd: root, stdio: ['pipe', 'pipe', 'pipe']});
  } else if (javaJar) {
    assert.ok(option('--java-server-dir', null), '--java-jar requires a prepared --java-server-dir');
    report.original_java_outer_sha1 = createHash('sha1').update(await readFile(resolve(javaJar))).digest('hex');
    assert.equal(report.original_java_outer_sha1, '450698d1863ab5180c25d7c804ef0fe6369dd1ba',
      'Java launcher requires the pinned unchanged official 1.21 server JAR');
    server = spawn(option('--java', '/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin/java'),
      ['-Xms128M', '-Xmx1024M', '-jar', resolve(javaJar), '--port', String(port), 'nogui'],
      {cwd: resolve(option('--java-server-dir', '')), stdio: ['pipe', 'pipe', 'pipe']});
  }
  if (server) {
    server.on('error', error => { serverLog += error.stack; });
    server.stdin.on('error', error => { serverLog += error.stack; });
    server.stdout.on('data', data => { serverLog += data; });
    server.stderr.on('data', data => { serverLog += data; });
    await until(() => serverLog.includes('"event":"listening"') || serverLog.includes('Done ('), 'server startup', javaJar ? 180000 : 15000);
  }
  const observer = await connect('ItemObserver');
  const dropper = await connect('ItemDropper');
  if (args.includes('--wait-common-position')) await until(() => clients.every(client =>
    client.position.x === 8.5 && client.position.y === 65 && client.position.z === 8.5), 'harness common position 8.5,65,8.5');
  const base = {x: dropper.position.x, y: dropper.position.y, z: dropper.position.z};
  if (scenario === 'stock' || scenario === 'all') await stock(observer, dropper, base);
  if (scenario === 'merge' || scenario === 'all') await merge(observer, dropper, base);
  report.ok = true;
  report.phase = 'passed';
} catch (error) {
  report.error = String(error.stack ?? error);
  report.failed_phase = report.phase;
  report.phase = 'failed';
  process.exitCode = 1;
} finally {
  for (const state of clients) { state.closing = true; state.client.end(); }
  if (controller && controller.exitCode === null) { controller.stdin.end(); controller.kill('SIGTERM'); }
  if (server && server.exitCode === null) {
    if (javaJar) server.stdin.write('stop\n'); else server.kill('SIGINT');
    await Promise.race([new Promise(done => server.once('exit', done)), pause(5000)]);
    if (server.exitCode === null) server.kill('SIGKILL');
  }
  report.events = clients.map(state => ({username: state.username, entityId: state.id, events: state.events}));
  await mkdir(resolve(root, 'evidence'), {recursive: true});
  const stringify = value => JSON.stringify(value, (_, entry) => typeof entry === 'bigint' ? entry.toString() : entry, 2) + '\n';
  await writeFile(resolve(root, `evidence/item-live-${label}.json`), stringify(report));
  if (server) await writeFile(resolve(root, `evidence/item-live-${label}-server.log`), serverLog);
  if (controller) await writeFile(resolve(root, `evidence/item-live-${label}-controller.log`), controllerLog);
  console.log(JSON.stringify({ok: report.ok, scenario, label, phase: report.phase, checks: report.checks, error: report.error}));
}
