#!/usr/bin/env node
import mc from 'minecraft-protocol';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { performance } from 'node:perf_hooks';

function parseOptions(argv) {
  const result = { host: '127.0.0.1', port: 25565, timeoutMs: 25000,
    username: 'RustProbe', source: 'unspecified', requireKeepAlive: true };
  const keys = { '--host': 'host', '--port': 'port', '--timeout-ms': 'timeoutMs',
    '--username': 'username', '--capture': 'capture', '--source': 'source' };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--no-keepalive') { result.requireKeepAlive = false; continue; }
    const key = keys[argv[i]];
    if (!key || i + 1 === argv.length) throw new Error(`Unknown or incomplete option: ${argv[i]}`);
    const value = argv[++i];
    result[key] = ['port', 'timeoutMs'].includes(key) ? Number(value) : value;
  }
  if (!Number.isInteger(result.port) || result.port < 1 || result.port > 65535) throw new Error('Invalid port');
  if (!Number.isInteger(result.timeoutMs) || result.timeoutMs < 1) throw new Error('Invalid timeout');
  if (!/^[A-Za-z0-9_]{1,16}$/.test(result.username)) throw new Error('Invalid offline username');
  return result;
}

function json(value) {
  return JSON.stringify(value, (_key, item) => typeof item === 'bigint' ? item.toString() : item, 2) + '\n';
}

function packetId(buffer) {
  let value = 0;
  for (let i = 0; i < Math.min(buffer.length, 5); i++) {
    value |= (buffer[i] & 0x7f) << (i * 7);
    if (!(buffer[i] & 0x80)) return { value, bytes: i + 1 };
  }
  throw new Error('Invalid serialized packet ID');
}

async function run(options) {
  const started = performance.now();
  const serializers = new Map();
  const configuration = [];
  const checks = { loginSuccess: false, configurationComplete: false, play: false,
    join: false, chunk: false, position: false, teleportAckSent: false,
    keepAliveReceived: false, keepAliveEchoSent: false };
  const counts = { registryData: 0, chunks: 0, keepAlive: 0, settingsSent: 0,
    knownPacksResponses: 0, chunkBatchResponses: 0 };
  const source = { label: options.source, host: options.host, port: options.port,
    minecraft: '1.21', protocol: 767, capturedAt: new Date().toISOString(),
    client: `minecraft-protocol ${mc.version ?? 'see package-lock.json'}` };
  let join;
  let joinPacket;
  let position;
  let disconnectReason;
  let resultError;
  let resolveDone;
  let settled = false;
  let closing = false;
  let completionTimer;
  const done = new Promise(resolve => { resolveDone = resolve; });
  const client = mc.createClient({ host: options.host, port: options.port,
    username: options.username, auth: 'offline', version: '1.21', keepAlive: false,
    hideErrors: true, clientSettings: { locale: 'en_us', viewDistance: 2, chatFlags: 0,
      chatColors: true, skinParts: 127, mainHand: 1, enableTextFiltering: false,
      enableServerListing: true } });
  const finish = error => {
    if (settled) return;
    settled = true;
    resultError = error;
    clearTimeout(deadline);
    clearTimeout(completionTimer);
    resolveDone();
  };
  const originalWrite = client.write.bind(client);
  client.write = (name, data) => {
    if (name === 'settings') counts.settingsSent++;
    if (name === 'select_known_packs') counts.knownPacksResponses++;
    return originalWrite(name, data);
  };
  const deadline = setTimeout(() => finish(new Error(`Interop deadline exceeded (${options.timeoutMs} ms)`)), options.timeoutMs);

  function descriptor(state, name, data, raw) {
    let serializer = serializers.get(state);
    if (!serializer) {
      serializer = mc.createSerializer({ state, isServer: true, version: '1.21' });
      serializers.set(state, serializer);
    }
    const bytes = serializer.createPacketBuffer({ name, params: data });
    const id = packetId(bytes);
    return { name, id: id.value, payload: bytes.subarray(id.bytes).toString('base64'),
      packet: bytes.toString('base64'), bytes: bytes.length,
      sha256: createHash('sha256').update(bytes).digest('hex'),
      rawMatchesSerialization: Buffer.isBuffer(raw) ? raw.equals(bytes) : null };
  }

  function maybeComplete() {
    const connected = checks.loginSuccess && checks.configurationComplete && checks.play &&
      checks.join && checks.chunk && checks.position && checks.teleportAckSent;
    const alive = !options.requireKeepAlive || (checks.keepAliveReceived && checks.keepAliveEchoSent);
    if (connected && alive && !completionTimer) {
      // Leave the connection alive briefly to catch immediate disconnects and flush acknowledgements.
      completionTimer = setTimeout(() => finish(null), 300);
    }
  }

  client.on('error', error => finish(error));
  client.on('end', reason => {
    if (!closing && !settled) finish(new Error(`Server ended connection: ${String(reason)}`));
  });
  client.on('disconnect', data => { disconnectReason = data.reason ?? data; finish(new Error('Server sent a disconnect packet')); });
  client.on('kick_disconnect', data => { disconnectReason = data.reason ?? data; finish(new Error('Server kicked the client')); });
  client.on('success', () => { checks.loginSuccess = true; });
  client.on('state', state => { if (state === mc.states.PLAY) { checks.play = true; maybeComplete(); } });
  client.on('packet', (data, metadata, raw) => {
    try {
      if (metadata.state === mc.states.CONFIGURATION) {
        if (['registry_data', 'feature_flags', 'tags'].includes(metadata.name)) {
          const captured = descriptor(metadata.state, metadata.name, data, raw);
          if (captured.rawMatchesSerialization === false) throw new Error(`Raw/serialized mismatch: ${metadata.name}`);
          configuration.push(captured);
          if (metadata.name === 'registry_data') counts.registryData++;
        }
        if (metadata.name === 'finish_configuration') checks.configurationComplete = true;
      }
      if (metadata.state === mc.states.PLAY && metadata.name === 'login') {
        join = data;
        joinPacket = descriptor(metadata.state, metadata.name, data, raw);
        if (joinPacket.rawMatchesSerialization === false) throw new Error('Raw/serialized mismatch: play login');
        checks.join = true;
      }
      if (metadata.state === mc.states.PLAY && metadata.name === 'map_chunk') {
        if (!Buffer.isBuffer(data.chunkData) || data.chunkData.length === 0) throw new Error('Empty chunk section payload');
        counts.chunks++;
        checks.chunk = true;
      }
      maybeComplete();
    } catch (error) { finish(error); }
  });
  client.on('position', data => {
    try {
      if (client.state !== mc.states.PLAY) return;
      position = data;
      checks.position = true;
      client.write('teleport_confirm', { teleportId: data.teleportId });
      checks.teleportAckSent = true;
      client.write('position_look', { x: data.x, y: data.y, z: data.z,
        yaw: data.yaw, pitch: data.pitch, onGround: true });
      maybeComplete();
    } catch (error) { finish(error); }
  });
  client.on('chunk_batch_finished', () => {
    try {
      client.write('chunk_batch_received', { chunksPerTick: 10 });
      counts.chunkBatchResponses++;
    } catch (error) { finish(error); }
  });
  client.on('keep_alive', data => {
    try {
      if (![mc.states.PLAY, mc.states.CONFIGURATION].includes(client.state)) return;
      client.write('keep_alive', { keepAliveId: data.keepAliveId });
      if (client.state === mc.states.PLAY) {
        checks.keepAliveReceived = true;
        checks.keepAliveEchoSent = true;
        counts.keepAlive++;
      }
      maybeComplete();
    } catch (error) { finish(error); }
  });

  await done;
  const report = { ok: !resultError, scope: options.requireKeepAlive ? 'join-chunk-position-keepalive' : 'join-fixture-capture',
    host: options.host, port: options.port, minecraft: '1.21', protocol: 767,
    checks, counts, elapsedMs: +(performance.now() - started).toFixed(1),
    error: resultError?.message, disconnectReason };
  try {
    if (options.capture) {
      const directory = path.resolve(options.capture);
      await mkdir(directory, { recursive: true });
      const complete = checks.configurationComplete && counts.registryData > 0;
      await writeFile(path.join(directory, 'configuration.json'), json({ format: 'mc121-configuration-v1',
        version: '1.21', protocol: 767, source, complete, packets: configuration }));
      if (join) await writeFile(path.join(directory, 'join.json'), json(join));
      if (joinPacket) await writeFile(path.join(directory, 'join-packet.json'), json({ version: '1.21', protocol: 767, source, ...joinPacket }));
      if (position) await writeFile(path.join(directory, 'position.json'), json(position));
      await writeFile(path.join(directory, 'interop-report.json'), json(report));
      report.capture = directory;
    }
    return report;
  } finally {
    closing = true;
    if (!client.ended) client.end('interop-complete');
    client.socket?.destroy();
    clearTimeout(client.closeTimer);
  }
}

try {
  const report = await run(parseOptions(process.argv.slice(2)));
  console.log(JSON.stringify(report));
  if (!report.ok) process.exitCode = 1;
} catch (error) {
  console.log(JSON.stringify({ ok: false, error: error.message }));
  process.exitCode = 1;
}
