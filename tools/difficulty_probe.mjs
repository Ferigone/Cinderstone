// SPDX-License-Identifier: GPL-2.0-only
// Observe actual PLAY initialization; never inject a difficulty packet.
import mc from 'minecraft-protocol';
import assert from 'node:assert/strict';
const [port, expectedDifficulty, expectedLock] = process.argv.slice(2);
const client = mc.createClient({host: '127.0.0.1', port: Number(port),
  username: 'DifficultyProbe', auth: 'offline', version: '1.21',
  keepAlive: false, hideErrors: true});
const packets = [];
const payloads = [];
let login = false;
const done = new Promise((resolve, reject) => {
  const timeout = setTimeout(() => reject(Error('Difficulty login timeout')), 30000);
  client.on('error', reject);
  client.on('kick_disconnect', () => reject(Error('Login rejected')));
  client.on('keep_alive', p => client.write('keep_alive', p));
  client.on('login', () => { login = true; });
  client.on('raw', (buffer, meta) => {
    if (meta.name === 'difficulty') {
      assert.equal(buffer[0], 0x0b, 'protocol 767 difficulty packet id');
      payloads.push(buffer.subarray(1).toString('hex'));
    }
  });
  client.on('difficulty', packet => {
    packets.push(packet);
    // This check ends at initial level-info, before chunk transport. Imported
    // gameplay admission and terrain completeness belong to other work packages.
    clearTimeout(timeout);
    resolve();
  });
});
try {
  await done;
  assert.ok(login, 'difficulty must follow PLAY login');
  assert.equal(packets.length, 1);
  assert.deepEqual(packets[0], {difficulty: ['peaceful', 'easy', 'normal', 'hard'][Number(expectedDifficulty)],
    difficultyLocked: expectedLock === 'true'});
  assert.deepEqual(payloads, [Buffer.from([Number(expectedDifficulty), Number(expectedLock === 'true')]).toString('hex')]);
  console.log(JSON.stringify({ok: true, login, packets, payloads}));
} finally { client.end(); }
