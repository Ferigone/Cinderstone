// SPDX-License-Identifier: GPL-2.0-only
// Exercise the actual client with a protocol peer that drops after the final ack.
import {test} from 'node:test';
import assert from 'node:assert/strict';
import {EventEmitter} from 'node:events';
import {readFile} from 'node:fs/promises';
import {createContext, SourceTextModule, SyntheticModule} from 'node:vm';
import {createHash} from 'node:crypto';
const source = await readFile(new URL('./client.mjs', import.meta.url), 'utf8');
const specification = await readFile(new URL('./scenarios.json', import.meta.url), 'utf8');
for (const lateDisconnect of [false, true]) test(`final observation disconnect=${lateDisconnect}`, async () => {
  let result;
  const process = {argv: ['node', 'client.mjs', '25592', 'scenario.json', 'trace.json']};
  const context = createContext({Buffer, process, setTimeout, clearTimeout});
  const peer = new EventEmitter();
  peer.end = () => {};
  peer.write = (name, packet) => {
    if (name !== 'block_place' && name !== 'block_dig') return;
    const state = name === 'block_place' ? 1 : 0;
    setTimeout(() => {
      peer.emit('packet', {location: {x: 10, y: 64, z: 8}, type: state}, {name: 'block_change'});
      peer.emit('packet', {sequenceId: packet.sequence}, {name: 'acknowledge_player_digging'});
      if (state === 0 && lateDisconnect) setTimeout(() => peer.emit('end'), 50);
    }, 0);
  };
  const dependencies = {
    'minecraft-protocol': {default: {createClient() {
      setTimeout(() => {
        peer.emit('login', {worldState: {hashedSeed: [0, 0]}});
        peer.emit('map_chunk', {x: 0, z: 0, chunkData: Buffer.alloc(0)});
        peer.emit('position', {x: 8.5, y: 65, z: 8.5, yaw: 0, pitch: 0, teleportId: 1});
        peer.emit('window_items', {windowId: 0, items: Array.from({length: 46}, () => ({itemCount: 0}))});
      }, 0);
      return peer;
    }}},
    'node:assert/strict': {default: assert},
    'node:crypto': {createHash},
    'prismarine-chunk': {default: () => class {load() {} getBlockStateId() {return 0;}}},
    'node:fs/promises': {
      readFile: async () => JSON.stringify(JSON.parse(specification)['creative-stone']),
      writeFile: async (_path, text) => { result = JSON.parse(text); }
    }
  };
  const module = new SourceTextModule(source, {context});
  await module.link(name => {
    const entries = dependencies[name];
    assert.ok(entries, `Unexpected import ${name}`);
    return new SyntheticModule(Object.keys(entries), function () {
      for (const [key, value] of Object.entries(entries)) this.setExport(key, value);
    }, {context});
  });
  await module.evaluate();
  assert.equal(result.ok, !lateDisconnect);
  assert.equal(result.error, lateDisconnect ? 'Unexpected disconnect' : null);
  assert.equal(process.exitCode, lateDisconnect ? 1 : undefined);
});
