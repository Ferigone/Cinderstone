// SPDX-License-Identifier: GPL-2.0-only
// One client for the unchanged original and native Main, no injected callbacks.
import mc from 'minecraft-protocol';
import {writeFile} from 'node:fs/promises';

const [port, output] = process.argv.slice(2);
const result = {ok: false, packets: [], error: null};
let failure, closing = false, position = false;
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
const client = mc.createClient({host: '127.0.0.1', port: Number(port),
  username: 'SpawnProbe', auth: 'offline', version: '1.21',
  keepAlive: false, hideErrors: true});
client.on('error', error => { if (!closing) failure = error; });
client.on('end', () => { if (!closing) failure = Error('Unexpected disconnect'); });
client.on('kick_disconnect', () => { failure = Error('Server rejected probe'); });
client.on('keep_alive', packet => client.write('keep_alive', packet));
client.on('chunk_batch_finished', () => client.write('chunk_batch_received', {chunksPerTick: 16}));
client.on('position', packet => {
  position = true;
  client.write('teleport_confirm', {teleportId: packet.teleportId});
});
client.on('packet', (packet, meta, buffer, fullBuffer) => {
  if (meta.name === 'spawn_position') result.packets.push({
    position: packet.location, angle: packet.angle,
    frame_body_hex: fullBuffer.toString('hex'),
  });
});
try {
  const deadline = Date.now() + 60000;
  while (!position || !result.packets.length) {
    if (failure) throw failure;
    if (Date.now() > deadline) throw Error('Spawn/position deadline exceeded');
    await pause(20);
  }
  await pause(300);
  if (failure) throw failure;
  if (result.packets.length !== 1) throw Error('Expected exactly one initial spawn packet');
  result.ok = true;
} catch (error) {
  result.error = error.message;
  process.exitCode = 1;
} finally {
  closing = true;
  client.end();
  await writeFile(output, JSON.stringify(result, null, 2) + '\n');
}
