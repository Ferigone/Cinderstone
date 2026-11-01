#!/usr/bin/env node
// Query only: no inventory mutation, movement experiment or drop implementation.
import mc from 'minecraft-protocol';
import assert from 'node:assert/strict';
import {writeFile} from 'node:fs/promises';

const args = process.argv.slice(2);
const option = (name, fallback) => args.includes(name) ? args[args.indexOf(name) + 1] : fallback;
const host = option('--host', '127.0.0.1');
const port = Number(option('--port', '25783'));
const output = option('--output', null);
assert.ok(output, '--output is required');
const names = option('--names', 'ItemDropper,ItemObserver').split(',');
const settings = {locale: 'en_us', viewDistance: 2, chatFlags: 0, chatColors: true,
  skinParts: 127, mainHand: 1, enableTextFiltering: false, enableServerListing: true};
const pause = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));
const report = {ok: false, minecraft: '1.21', protocol: 767, phase: option('--phase', 'query'),
  scope: 'same-process offline-UUID login plus two explicit REQUEST_STATS packets per connection', players: []};
let active, closing = false;
try {
  for (const username of names) {
    const errors = [];
    const state = {username, uuid: null, entity_id: null, position_received: false,
      inventory_received: false, chunk_batches: 0, loaded_chunks: 0, collect_packets: [], statistics: [], requests: []};
    const chunks = new Set();
    report.players.push(state);
    closing = false;
    const client = mc.createClient({host, port, username, auth: 'offline', version: '1.21',
      keepAlive: false, hideErrors: true, clientSettings: settings});
    active = client;
    client.on('error', error => { if (!closing) errors.push(error); });
    client.on('end', reason => { if (!closing) errors.push(Error(`unexpected disconnect ${reason}`)); });
    client.on('kick_disconnect', data => { if (!closing) errors.push(Error(JSON.stringify(data))); });
    client.on('keep_alive', data => client.write('keep_alive', data));
    client.on('chunk_batch_finished', () => { state.chunk_batches++; client.write('chunk_batch_received', {chunksPerTick: 16}); });
    client.on('map_chunk', data => { chunks.add(`${data.x},${data.z}`); state.loaded_chunks = chunks.size; });
    client.on('unload_chunk', data => { chunks.delete(`${data.chunkX},${data.chunkZ}`); state.loaded_chunks = chunks.size; });
    client.on('position', data => { state.position_received = true; client.write('teleport_confirm', {teleportId: data.teleportId}); });
    client.on('login', data => { state.entity_id = data.entityId; state.uuid = client.uuid; });
    client.on('window_items', data => { if (data.windowId === 0 && data.items.length === 46) state.inventory_received = true; });
    client.on('collect', data => state.collect_packets.push(data));
    client.on('statistics', data => state.statistics.push(structuredClone(data.entries)));
    const until = async (predicate, description) => {
      const deadline = Date.now() + 20000;
      while (!predicate()) {
        if (errors.length) throw errors[0];
        if (Date.now() >= deadline) throw Error(`deadline ${username}: ${description}`);
        await pause(15);
      }
    };
    await until(() => state.entity_id !== null && state.position_received && state.inventory_received && state.loaded_chunks === 45,
      'PLAY identity, position, authoritative inventory and forty-five-chunk view');
    state.unsolicited_before_requests = state.statistics.length;
    for (let request = 0; request < 2; request++) {
      const before = state.statistics.length;
      state.requests.push({action_id: 1, before_packet_count: before});
      client.write('client_command', {actionId: 1});
      await until(() => state.statistics.length > before, `REQUEST_STATS response ${request + 1}`);
      state.requests.at(-1).response_index = before;
    }
    assert.equal(state.requests.length, 2);
    closing = true;
    const ended = new Promise(resolve => client.once('end', resolve));
    client.end();
    await Promise.race([ended, pause(1000)]);
    active = null;
    await pause(150);
  }
  report.ok = true;
} catch (error) {
  report.error = String(error.stack ?? error);
  process.exitCode = 1;
} finally {
  closing = true;
  active?.end();
  await writeFile(output, JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify({ok: report.ok, phase: report.phase, players: report.players.map(player => ({
    username: player.username, uuid: player.uuid, response_packets: player.statistics.length})), error: report.error}));
}
