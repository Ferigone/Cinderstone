#!/usr/bin/env node
import net from 'node:net';
import { pathToFileURL } from 'node:url';
import { performance } from 'node:perf_hooks';

const MAX_PACKET = 2 * 1024 * 1024;

export function encodeVarInt(value) {
  let n = value >>> 0;
  const bytes = [];
  do {
    let byte = n & 0x7f;
    n >>>= 7;
    if (n) byte |= 0x80;
    bytes.push(byte);
  } while (n);
  return Buffer.from(bytes);
}

export function readVarInt(buffer, offset = 0) {
  let value = 0;
  for (let i = 0; i < 5; i++) {
    if (offset + i >= buffer.length) return null;
    const byte = buffer[offset + i];
    if (i === 4 && (byte & 0xf0) !== 0) throw new Error('VarInt overflows 32 bits');
    value |= (byte & 0x7f) << (7 * i);
    if (!(byte & 0x80)) return { value: value | 0, bytes: i + 1 };
  }
  throw new Error('VarInt exceeds five bytes');
}

function string(value) {
  const bytes = Buffer.from(value, 'utf8');
  return Buffer.concat([encodeVarInt(bytes.length), bytes]);
}

function frame(id, payload = Buffer.alloc(0)) {
  const body = Buffer.concat([encodeVarInt(id), payload]);
  return Buffer.concat([encodeVarInt(body.length), body]);
}

export async function probeServer({ host = '127.0.0.1', port = 25565, timeoutMs = 5000,
  expectedProtocol = 767, expectedVersion = '1.21' } = {}) {
  const started = performance.now();
  const nonce = 0x0102030405060708n;
  return await new Promise((resolve, reject) => {
    const socket = net.createConnection({ host, port });
    let pending = Buffer.alloc(0);
    let status;
    let pingStarted;
    let settled = false;
    const finish = (error, result) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      socket.destroy();
      if (error) reject(error);
      else resolve(result);
    };
    const timer = setTimeout(() => finish(new Error(`Status/pong deadline exceeded (${timeoutMs} ms)`)), timeoutMs);
    socket.on('error', error => finish(error));
    socket.on('close', () => {
      if (!settled) finish(new Error('Connection closed before valid status and pong'));
    });
    socket.on('connect', () => {
      const portBytes = Buffer.alloc(2);
      portBytes.writeUInt16BE(port);
      socket.write(frame(0, Buffer.concat([encodeVarInt(expectedProtocol), string(host), portBytes, encodeVarInt(1)])));
      socket.write(frame(0));
    });
    socket.on('data', bytes => {
      try {
        pending = Buffer.concat([pending, bytes]);
        while (pending.length) {
          const length = readVarInt(pending);
          if (!length) return;
          if (length.value < 1 || length.value > MAX_PACKET) throw new Error('Invalid status frame length');
          if (pending.length < length.bytes + length.value) return;
          const body = pending.subarray(length.bytes, length.bytes + length.value);
          pending = pending.subarray(length.bytes + length.value);
          const id = readVarInt(body);
          if (!id) throw new Error('Missing response packet ID');
          const payload = body.subarray(id.bytes);
          if (!status) {
            if (id.value !== 0) throw new Error(`Expected status packet 0, received ${id.value}`);
            const textLength = readVarInt(payload);
            if (!textLength || textLength.value < 0 || payload.length !== textLength.bytes + textLength.value) {
              throw new Error('Invalid status JSON string length');
            }
            status = JSON.parse(payload.subarray(textLength.bytes).toString('utf8'));
            if (status.version?.protocol !== expectedProtocol) throw new Error(`Protocol mismatch: ${status.version?.protocol}`);
            if (expectedVersion && status.version?.name !== expectedVersion) throw new Error(`Version mismatch: ${status.version?.name}`);
            if (!status.players || !Number.isInteger(status.players.online) || !Number.isInteger(status.players.max)) {
              throw new Error('Status lacks valid player counts');
            }
            const ping = Buffer.alloc(8);
            ping.writeBigInt64BE(nonce);
            pingStarted = performance.now();
            socket.write(frame(1, ping));
          } else {
            if (id.value !== 1 || payload.length !== 8) throw new Error('Invalid pong packet');
            if (payload.readBigInt64BE() !== nonce) throw new Error('Pong did not echo the nonce');
            finish(null, { ok: true, host, port, protocol: expectedProtocol, status,
              pongMatched: true, latencyMs: +(performance.now() - pingStarted).toFixed(3),
              elapsedMs: +(performance.now() - started).toFixed(3) });
            return;
          }
        }
      } catch (error) { finish(error); }
    });
  });
}

function options(argv) {
  const result = {};
  const keys = { '--host': 'host', '--port': 'port', '--timeout-ms': 'timeoutMs',
    '--expect-protocol': 'expectedProtocol', '--expect-version': 'expectedVersion' };
  for (let i = 0; i < argv.length; i++) {
    const key = keys[argv[i]];
    if (!key || i + 1 === argv.length) throw new Error(`Unknown or incomplete option: ${argv[i]}`);
    const value = argv[++i];
    result[key] = ['port', 'timeoutMs', 'expectedProtocol'].includes(key) ? Number(value) : value;
  }
  if (result.port !== undefined && (!Number.isInteger(result.port) || result.port < 1 || result.port > 65535)) throw new Error('Invalid port');
  if (result.timeoutMs !== undefined && (!Number.isInteger(result.timeoutMs) || result.timeoutMs < 1)) throw new Error('Invalid timeout');
  return result;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try { console.log(JSON.stringify(await probeServer(options(process.argv.slice(2))))); }
  catch (error) { console.log(JSON.stringify({ ok: false, error: error.message })); process.exitCode = 1; }
}
