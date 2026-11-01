// Decode the pinned Mojang dimension registry with an independent wire client.
import mc from 'minecraft-protocol';
import nbt from 'prismarine-nbt';
import fs from 'node:fs';
import crypto from 'node:crypto';
const path = new URL('../fixtures/configuration.json', import.meta.url);
const bytes = fs.readFileSync(path);
const capture = JSON.parse(bytes);
const decoder = mc.createDeserializer({state:'configuration',isServer:false,version:'1.21'});
for (const packet of capture.packets.filter(p=>p.name==='registry_data')) {
  const raw = Buffer.concat([Buffer.from([packet.id]),Buffer.from(packet.payload,'base64')]);
  const decoded = decoder.parsePacketBuffer(raw);
  if (decoded.data.params.id !== 'minecraft:dimension_type') continue;
  if (decoded.metadata.size !== raw.length) throw Error('dimension packet prefix mismatch');
  const dimensions = decoded.data.params.entries.map(entry=>{
    const value=nbt.simplify(entry.value);
    return {key:entry.key, facts:{min_y:value.min_y,height:value.height,
      logical_height:value.logical_height,ultrawarm:value.ultrawarm!==0,
      has_skylight:value.has_skylight!==0,has_ceiling:value.has_ceiling!==0}};
  });
  fs.writeFileSync(new URL('../fixtures/dimension-context-vectors.json',import.meta.url),
    JSON.stringify({minecraft:'1.21',protocol:767,source:'pinned original Mojang registry packet, independently decoded',
      configuration_sha256:crypto.createHash('sha256').update(bytes).digest('hex'),payload:packet.payload,dimensions},null,2)+'\n');
  console.log(JSON.stringify({dimensions:dimensions.length,keys:dimensions.map(d=>d.key)}));
}
