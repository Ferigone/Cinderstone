// SPDX-License-Identifier: GPL-2.0-only
// The same real protocol client captures both Java and native PLAY login flags.
import mc from 'minecraft-protocol';
import {writeFile} from 'node:fs/promises';
const [port, output, kind = 'ui', username = 'LoginRulesProbe'] = process.argv.slice(2);
let client, login, failure, position;
const result = {ok: false};
const timeout = setTimeout(() => { failure = Error('PLAY login/position deadline'); }, 30000);
try {
  if (!['ui', 'world-type'].includes(kind)) throw Error('Unknown login capture kind');
  client = mc.createClient({host: '127.0.0.1', port: Number(port), username,
    auth: 'offline', version: '1.21', keepAlive: false, hideErrors: true});
  client.on('error', e => { failure = e; });
  client.on('kick_disconnect', () => { failure = Error('Kicked before admission'); });
  client.on('end', () => { failure ||= Error('Disconnected before admission'); });
  client.on('keep_alive', p => client.write('keep_alive', p));
  client.on('chunk_batch_finished', () => client.write('chunk_batch_received', {chunksPerTick: 16}));
  client.on('packet', (p, meta) => {
    if (meta.state === 'play' && meta.name === 'login') {
      login = kind === 'world-type'
        ? {is_debug: p.worldState.isDebug, is_flat: p.worldState.isFlat}
        : {reduced_debug_info: p.reducedDebugInfo, show_death_screen: p.enableRespawnScreen};
    }
  });
  client.on('position', p => {
    client.write('teleport_confirm', {teleportId: p.teleportId});
    position = true;
  });
  while (!login || !position) {
    if (failure) throw failure;
    await new Promise(resolve => setTimeout(resolve, 20));
  }
  if (!Object.values(login).every(value => typeof value === 'boolean'))
    throw Error('Missing login flags');
  Object.assign(result, {ok: true, flags: login, position_received: true});
} catch (e) { result.error = e.message; process.exitCode = 1; }
finally {
  clearTimeout(timeout);
  client?.end();
  await writeFile(output, JSON.stringify(result, null, 2) + '\n');
}
