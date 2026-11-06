// GPL-2.0-only. One protocol client, used unchanged for both real servers.
import mc from 'minecraft-protocol';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import chunkFactory from 'prismarine-chunk';
const Chunk = chunkFactory('1.21');
import {readFile, writeFile} from 'node:fs/promises';
const [port, scenarioPath, output] = process.argv.slice(2);
const scenario = JSON.parse(await readFile(scenarioPath, 'utf8'));
const result = {ok: false, phases: [], error: null};
let client, closing = false, failure, position, inventory;
let phase, sceneHash, seedHash;
const pause = ms => new Promise(r => setTimeout(r, ms));
async function until(test, label) {
  const deadline = Date.now() + 60000;
  while (!test()) {
    if (failure) throw failure;
    if (Date.now() > deadline) throw Error('Timeout: ' + label);
    await pause(20);
  }
}
try {
  client = mc.createClient({host: '127.0.0.1', port: Number(port), username: 'ParityProbe',
    auth: 'offline', version: '1.21', keepAlive: false, hideErrors: true});
  client.on('error', e => { if (!closing) failure = e; });
  client.on('end', () => { if (!closing) failure = Error('Unexpected disconnect'); });
  client.on('kick_disconnect', () => { failure = Error('Server kicked scenario player'); });
  client.on('keep_alive', p => client.write('keep_alive', p));
  client.on('chunk_batch_finished', () => client.write('chunk_batch_received', {chunksPerTick: 16}));
  let chunk = false;
  client.on('login', p => { seedHash = p.worldState.hashedSeed; });
  client.on('map_chunk', p => {
    if (p.x !== 0 || p.z !== 0 || chunk) return;
    try {
      const decoded = new Chunk({minY: -64, worldHeight: 384});
      decoded.load(p.chunkData);
      const states = Buffer.alloc(16 * 16 * 384 * 4);
      let offset = 0;
      for (let y = -64; y < 320; y++) for (let z = 0; z < 16; z++) for (let x = 0; x < 16; x++) {
        states.writeUInt32BE(decoded.getBlockStateId({x, y, z}), offset); offset += 4;
      }
      sceneHash = createHash('sha256').update(states).digest('hex');
      chunk = true;
    } catch (error) { failure = error; }
  });
  client.on('position', p => {
    position = p;
    client.write('teleport_confirm', {teleportId: p.teleportId});
    client.write('position_look', {x: p.x, y: p.y, z: p.z, yaw: p.yaw, pitch: p.pitch, onGround: true});
  });
  client.on('window_items', p => { if (p.windowId === 0 && !inventory) inventory = p.items; });
  client.on('packet', (p, meta) => {
    if (!phase) return;
    if (meta.name === 'block_change' && ['x', 'y', 'z'].every(k => p.location[k] === scenario.target[k]))
      phase.events.push({name: 'block_change', state: p.type});
    if (meta.name === 'acknowledge_player_digging')
      phase.events.push({name: 'acknowledge_player_digging', sequence: p.sequenceId});
  });
  await until(() => position && inventory && chunk, 'join, inventory and target chunk');
  await until(() => scenario.position.every((n, i) => position[['x','y','z'][i]] === n), 'common player position');
  assert.equal(inventory.length, 46);
  assert.ok(inventory.every(s => !s.itemCount), 'fresh player inventory must be empty');
  result.phases.push({name: 'join', events: [{name: 'scene', chunk: [0, 0], block_states_sha256: sceneHash, hashed_seed: seedHash}, {name: 'inventory', slots: inventory.map(s => ({count: s.itemCount || 0}))}]});
  client.write('held_item_slot', {slotId: 0});
  client.write('set_creative_slot', {slot: 36, item: {itemCount: 64, itemId: 1,
    addedComponentCount: 0, removedComponentCount: 0, components: [], removeComponents: []}});
  await pause(250);
  for (const [name, state, sequence] of [['place', 1, 101], ['break', 0, 102]]) {
    phase = {name, events: []}; result.phases.push(phase);
    if (name === 'place') client.write('block_place', {hand: 0,
      location: {...scenario.target, y: scenario.target.y - 1}, direction: 1,
      cursorX: .5, cursorY: 1, cursorZ: .5, insideBlock: false, sequence});
    else client.write('block_dig', {status: 0, location: scenario.target, face: 1, sequence});
    await until(() => phase.events.some(e => e.name === 'block_change' && e.state === state) &&
      phase.events.some(e => e.name === 'acknowledge_player_digging' && e.sequence >= sequence), name);
    await pause(250); // Include duplicate updates; do not silently deduplicate.
  }
  phase = null;
  result.ok = true;
} catch (e) { result.error = e.message; process.exitCode = 1; }
finally {
  closing = true; client?.end();
  await writeFile(output, JSON.stringify(result, null, 2) + '\n');
}
