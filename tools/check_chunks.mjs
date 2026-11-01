#!/usr/bin/env node
// Decode native Rust fixtures using the independent PrismarineJS protocol and
// chunk implementations. No Rust encoder logic is reproduced by this check.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const require = createRequire(import.meta.url);
const mcData = require('minecraft-data')('1.21');
const minecraftProtocol = require('minecraft-protocol');
const Chunk = require('prismarine-chunk')('1.21');
const BitArray = require('prismarine-chunk/src/pc/common/BitArrayNoSpan');
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

assert.equal(mcData.version.version, 767);
assert.equal(mcData.version.minecraftVersion, '1.21');
const ids = {
  air: mcData.blocksByName.air.defaultState,
  stone: mcData.blocksByName.stone.defaultState,
  dirt: mcData.blocksByName.dirt.defaultState,
  grass: mcData.blocksByName.grass_block.defaultState,
  biome: mcData.biomesByName.plains.id,
  varied_states: Object.keys(mcData.blocksByStateId)
    .map(Number)
    .filter(state => state !== mcData.blocksByName.air.defaultState)
    .sort((a, b) => a - b)
    .slice(0, 257)
};
assert.equal(new Set(ids.varied_states).size, 257);
for (const state of ids.varied_states) assert.ok(mcData.blocksByStateId[state]);

// A caller may supply a prebuilt probe; otherwise use Cargo from their toolchain.
const binary = process.env.MC121_CHUNK_PROBE_BINARY;
const command = binary || process.env.CARGO || 'cargo';
const args = binary
  ? [JSON.stringify(ids)]
  : ['run', '--quiet', '--bin', 'chunk_probe', '--', JSON.stringify(ids)];
const generated = spawnSync(command, args, {
  cwd: root,
  encoding: 'utf8',
  maxBuffer: 8 * 1024 * 1024,
  timeout: 180000,
  env: process.env
});
if (generated.error) throw generated.error;
assert.equal(generated.status, 0, generated.stderr || 'Rust probe failed');
const fixture = JSON.parse(generated.stdout);
assert.equal(fixture.protocol, 767);
assert.equal(fixture.version, '1.21');
assert.equal(fixture.chunks.length, 3);

const parser = minecraftProtocol.createDeserializer({ state: 'play', isServer: false, version: '1.21' });
const results = [];
for (const sample of fixture.chunks) {
  // The fixture contains a payload, so prepend the independently specified
  // one-byte protocol-767 Level Chunk With Light packet ID, not a frame length.
  const raw = Buffer.concat([Buffer.from([0x27]), Buffer.from(sample.payload_base64, 'base64')]);
  const parsed = parser.parsePacketBuffer(raw);
  assert.equal(parsed.metadata.size, raw.length, `${sample.name}: trailing/unparsed packet bytes`);
  assert.equal(parsed.data.name, 'map_chunk');
  const data = parsed.data.params;
  assert.equal(data.x, sample.x);
  assert.equal(data.z, sample.z);
  assert.equal(data.heightmaps.type, 'compound');
  assert.deepEqual(Object.keys(data.heightmaps.value).sort(), ['MOTION_BLOCKING', 'WORLD_SURFACE']);

  const column = new Chunk({ minY: fixture.min_y, worldHeight: fixture.height });
  column.load(data.chunkData);
  assert.equal(column.sections.length, 24);
  assert.equal(column.biomes.length, 24);
  assert.ok(column.dump().equals(data.chunkData), `${sample.name}: chunk roundtrip differs in independent encoder`);

  for (const check of sample.checks) {
    assert.equal(column.getBlockStateId(check), check.state,
      `${sample.name}: wrong block at local (${check.x},${check.y},${check.z})`);
    assert.equal(column.getBiome(check), fixture.biome, `${sample.name}: wrong biome`);
  }
  assert.equal(column.sections.reduce((sum, section) => sum + section.solidBlockCount, 0), sample.expected_non_air);
  if (sample.expected_direct_section !== null) {
    const section = column.sections[sample.expected_direct_section];
    assert.equal(section.data.constructor.name, 'DirectPaletteContainer');
    assert.equal(section.data.data.bitsPerValue, 15);
  }

  for (const name of ['MOTION_BLOCKING', 'WORLD_SURFACE']) {
    const tag = data.heightmaps.value[name];
    assert.equal(tag.type, 'longArray');
    assert.equal(tag.value.length, 37);
    const heights = BitArray.fromLongArray(tag.value, 9);
    for (const check of sample.height_checks) {
      assert.equal(heights.get(check.z * 16 + check.x), check.height,
        `${sample.name}: ${name} at (${check.x},${check.z})`);
    }
  }

  assert.equal(data.blockEntities.length, 0);
  assert.equal(data.skyLight.length, 26);
  assert.equal(data.blockLight.length, 0);
  assert.ok(data.skyLight.every(section => section.length === 2048));
  const skyMask = BitArray.fromLongArray(data.skyLightMask, 1);
  const darkMask = BitArray.fromLongArray(data.emptyBlockLightMask, 1);
  for (let section = 0; section < 26; section++) {
    assert.equal(skyMask.get(section), 1);
    assert.equal(darkMask.get(section), 1);
  }
  column.loadParsedLight(data.skyLight, data.blockLight,
    data.skyLightMask, data.blockLightMask, data.emptySkyLightMask, data.emptyBlockLightMask);
  for (const check of sample.checks) {
    assert.equal(column.getSkyLight(check), 15);
    assert.equal(column.getBlockLight(check), 0);
  }
  results.push({
    name: sample.name,
    coordinates: [sample.x, sample.z],
    payload_bytes: raw.length - 1,
    decoded_sections: column.sections.length,
    block_checks: sample.checks.length,
    height_checks: sample.height_checks.length * 2,
    direct_palette_checked: sample.expected_direct_section !== null,
    passed: true
  });
}

console.log(JSON.stringify({
  check: 'independent-chunk-decoder',
  minecraft: '1.21',
  protocol: 767,
  decoders: {
    'minecraft-protocol': require('minecraft-protocol/package.json').version,
    'prismarine-chunk': require('prismarine-chunk/package.json').version
  },
  chunks: results,
  passed: true,
  scope: 'Wire encoding and deterministic fixture rendering data; does not establish vanilla generation or lighting parity.'
}, null, 2));
