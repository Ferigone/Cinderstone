#!/usr/bin/env node
// Independentnetwork+NBT decoder and actualJava packetcomparison.
import assert from 'node:assert/strict';import fs from 'node:fs';import path from 'node:path';import mc from 'minecraft-protocol';import chunkFactory from 'prismarine-chunk';import nbt from 'prismarine-nbt';import dataFactory from 'minecraft-data';
const directory=path.resolve(process.argv[2]);const parser=mc.createDeserializer({state:'play',isServer:false,version:'1.21'});const Chunk=chunkFactory('1.21');const mcData=dataFactory('1.21');const records=JSON.parse(fs.readFileSync(path.join(directory,'records.json'))).records;let checkedBlocks=0;let comparedJava=0;
const parse=b=>parser.parsePacketBuffer(Buffer.concat([Buffer.from([0x27]),b]));
for(const record of records){
 const key=`${record.x}.${record.z}`;const rustRaw=fs.readFileSync(path.join(directory,`chunk.${key}.packet`));const rust=parse(rustRaw);assert.equal(rust.metadata.size,rustRaw.length+1);assert.equal(rust.data.name,'map_chunk');const p=rust.data.params;const column=new Chunk({minY:-64,worldHeight:384});column.load(p.chunkData);assert.ok(column.dump().equals(p.chunkData));
 const lightRaw=fs.readFileSync(path.join(directory,`chunk.${key}.light`));const light=parser.parsePacketBuffer(Buffer.concat([Buffer.from([0x2a]),lightRaw]));assert.equal(light.metadata.size,lightRaw.length+1);assert.equal(light.data.name,'update_light');assert.equal(light.data.params.chunkX,record.x);assert.equal(light.data.params.chunkZ,record.z);for(const field of ['skyLightMask','blockLightMask','emptySkyLightMask','emptyBlockLightMask','skyLight','blockLight'])assert.deepEqual(light.data.params[field],p[field]);
 const states=fs.readFileSync(path.join(directory,`chunk.${key}.states`));const disk=nbt.simplify(nbt.parseUncompressed(fs.readFileSync(path.join(directory,`chunk.${key}.nbt`))));assert.equal(disk.DataVersion,3953);assert.equal(disk.xPos,record.x);assert.equal(disk.zPos,record.z);
 for(let y=-64;y<320;y++)for(let z=0;z<16;z++)for(let x=0;x<16;x++){
   const expected=states.readUInt32BE((((y+64)*16+z)*16+x)*4);assert.equal(column.getBlockStateId({x,y,z}),expected,`${key}/${x}/${y}/${z}`);assert.equal(column.getBiome({x,y,z}),mcData.biomesByName.plains.id);checkedBlocks++;
 }
 const javaPath=path.join(directory,`java.${key}.packet`);if(fs.existsSync(javaPath)){
  const java=parse(fs.readFileSync(javaPath)).data.params;
  for(const field of ['x','z','heightmaps','blockEntities','skyLightMask','blockLightMask','emptySkyLightMask','emptyBlockLightMask','skyLight','blockLight'])assert.deepEqual(p[field],java[field],`${key}:Java field ${field}`);
  assert.ok(p.chunkData.equals(java.chunkData),`${key}:Java section bytes differ`);comparedJava++;
 }
}
const result={ok:true,minecraft:'1.21',data_version:3953,independent_decoder:'PrismarineJS',chunks:records.length,block_and_biome_checks:checkedBlocks,actual_java_chunk_comparisons:comparedJava,all_section_bytes_equal:true,all_light_masks_and_arrays_equal:true,all_heightmaps_equal:true,light_update_packets_decoded:true,scope:'existing full overworld chunks without block entities; no terrain generation or world simulation claim'};console.log(JSON.stringify(result));fs.writeFileSync(path.join(directory,'conformance.json'),JSON.stringify(result,null,2)+'\n');
