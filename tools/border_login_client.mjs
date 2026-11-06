// SPDX-License-Identifier: GPL-2.0-only
// Same real protocol client for both unchanged Java and native Main.
import mc from 'minecraft-protocol';
import {writeFile} from 'node:fs/promises';
const [port, output] = process.argv.slice(2);
const result = {ok: false, packets: []};
let failure, position, closing = false;
const client = mc.createClient({host: '127.0.0.1', port: Number(port),
  username: 'BorderProbe', auth: 'offline', version: '1.21',
  keepAlive: false, hideErrors: true});
client.on('error', e => { if (!closing) failure = e; });
client.on('end', () => { if (!closing) failure = Error('Unexpected disconnect'); });
client.on('kick_disconnect', () => { failure = Error('Server kicked probe'); });
client.on('keep_alive', p => client.write('keep_alive', p));
client.on('chunk_batch_finished', () => client.write('chunk_batch_received', {chunksPerTick: 16}));
client.on('position', p => {
  position = p;
  client.write('teleport_confirm', {teleportId: p.teleportId});
});
client.on('packet', (p, meta, buffer) => {
  if (meta.name === 'initialize_world_border') {
    if (buffer[0] !== 0x25) failure = Error('Unexpected initialize-border packet id');
    result.packets.push({fields: p, payload_hex: buffer.subarray(1).toString('hex')});
  }
});
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
try {
  const deadline = Date.now() + 60000;
  while (!position || !result.packets.length) {
    if (failure) throw failure;
    if (Date.now() >= deadline) throw Error('Missing initial border or position');
    await pause(20);
  }
  await pause(300);
  if (failure) throw failure;
  if (result.packets.length !== 1) throw Error('Expected exactly one initial border');
  result.ok = true;
} catch (e) { result.error = e.message; process.exitCode = 1; }
finally {
  closing = true; client.end();
  await writeFile(output, JSON.stringify(result, (_, value) =>
    typeof value === 'bigint' ? String(value) : value, 2) + '\n');
}
