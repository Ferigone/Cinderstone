#!/usr/bin/env node
import assert from 'node:assert/strict';
import {readFileSync,readdirSync} from 'node:fs';
import path from 'node:path';
import zlib from 'node:zlib';
import nbt from 'prismarine-nbt';

const directory=path.resolve(process.argv[2]);
const records=JSON.parse(readFileSync(path.join(directory,'records.json'),'utf8'));
let count=0;
for(const record of records){
    for(const chunk of record.chunks){
        const bytes=readFileSync(path.join(directory,`region.${record.region[0]}.${record.region[1]}`,chunk.file));
        const decoded=nbt.parseUncompressed(bytes,'big');
        assert.equal(decoded.type,'compound');
        assert.equal(decoded.value.DataVersion.value,3953);
        assert.equal(decoded.value.xPos.value,chunk.x);
        assert.equal(decoded.value.zPos.value,chunk.z);
        count++;
    }
    const section=path.join(directory,`region.${record.region[0]}.${record.region[1]}`);
    const raw=readFileSync(path.join(section,'sample.raw'));
    assert.ok(zlib.gunzipSync(readFileSync(path.join(section,'sample.gzip'))).equals(raw));
    assert.ok(zlib.inflateSync(readFileSync(path.join(section,'sample.zlib'))).equals(raw));
    assert.ok(readFileSync(path.join(section,'sample.none')).equals(raw));
    const java=nbt.parseUncompressed(readFileSync(path.join(section,'java-read-region.nbt')),'big');
    assert.equal(java.value.rust_port_roundtrip_marker.value,'native Rust / official Java 1.21');
    assert.equal(java.value.DataVersion.value,3953);
    assert.equal(java.value.xPos.value,record.edited_chunk[0]);
    assert.equal(java.value.zPos.value,record.edited_chunk[1]);
}
const external=nbt.parseUncompressed(readFileSync(path.join(directory,'java-read-external.nbt')),'big');
assert.equal(external.value.rust_port_roundtrip_marker.value,'external chunk');
assert.equal(external.value.DataVersion.value,3953);
assert.equal(external.value.xPos.value,255);
assert.equal(external.value.zPos.value,-257);
assert.equal(external.value.huge.value.length,1_100_000);
assert.ok(external.value.huge.value.every(value=>value===7));
console.log(JSON.stringify({ok:true,independent_decoder:'prismarine-nbt',regions:records.length,chunks:count,java_read_rust_external_chunk:true,
  java_read_rust_lz4_regions:records.length,compression_formats:['gzip','zlib','uncompressed','Java LZ4Block'],scope:'NBT/Anvil format interoperability and unknown field preservation; not complete world simulation parity'}));
