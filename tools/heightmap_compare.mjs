#!/usr/bin/env node
// Independent Anvil+NBT decoding of all four saved Java heightmaps.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import zlib from 'node:zlib';
import nbt from 'prismarine-nbt';

const root = path.resolve(process.argv[2]);
const names = ['WORLD_SURFACE', 'OCEAN_FLOOR', 'MOTION_BLOCKING', 'MOTION_BLOCKING_NO_LEAVES'];
const coordinates = [[0, 0], [-1, -1], [1, 1], [-1, 0], [0, -1]];
function javaChunk(x, z) {
  const directory = path.join(root, 'java-run/world/region');
  const region = fs.readFileSync(path.join(directory, `r.${Math.floor(x / 32)}.${Math.floor(z / 32)}.mca`));
  const location = region.readUInt32BE((((z & 31) << 5) | (x & 31)) * 4);
  const offset = (location >>> 8) * 4096, capacity = (location & 255) * 4096;
  assert.ok(offset >= 8192 && capacity > 0 && offset + capacity <= region.length);
  const length = region.readUInt32BE(offset);
  assert.ok(length > 1 && length + 4 <= capacity);
  const kind = region[offset + 4];
  assert.ok(!(kind & 128), 'selected oracle chunks must be internal');
  const payload = region.subarray(offset + 5, offset + 4 + length);
  const raw = kind === 1 ? zlib.gunzipSync(payload)
    : kind === 2 ? zlib.inflateSync(payload)
      : kind === 3 ? payload : null;
  assert.ok(raw, 'unsupported compression in selected Java oracle chunks');
  return nbt.simplify(nbt.parseUncompressed(raw));
}
for (const [x, z] of coordinates) {
  const java = javaChunk(x, z);
  const native = nbt.simplify(nbt.parseUncompressed(fs.readFileSync(path.join(root, `chunk.${x}.${z}.nbt`))));
  assert.equal(java.DataVersion, 3953);
  assert.equal(native.DataVersion, 3953);
  assert.equal(java.xPos, x); assert.equal(java.zPos, z);
  for (const name of names) {
    assert.ok(java.Heightmaps[name]); assert.ok(native.Heightmaps[name]);
    assert.deepEqual(native.Heightmaps[name], java.Heightmaps[name], `${x},${z}/${name}`);
  }
}
const result = {
  ok: true, minecraft: '1.21', chunks: coordinates.length,
  saved_heightmaps_compared: coordinates.length * names.length,
  height_entries: coordinates.length * names.length * 256,
  independent_decoder: 'Node Anvil headers/zlib + prismarine-nbt',
  all_four_saved_heightmaps_equal: true,
};
fs.writeFileSync(path.join(root, 'heightmap-nbt-comparison.json'), JSON.stringify(result, null, 2) + '\n');
console.log(JSON.stringify(result));
