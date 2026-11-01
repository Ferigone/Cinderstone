#!/usr/bin/env node
import mc from 'minecraft-protocol';import assert from 'node:assert/strict';import {readFile,writeFile} from 'node:fs/promises';
const fixture=JSON.parse(await readFile(new URL('equipment-java.json',import.meta.url),'utf8'));
const serializer=mc.createSerializer({state:'play',isServer:true,version:'1.21'}),decoder=mc.createDeserializer({state:'play',isServer:false,version:'1.21'});
let packets=0,entries=0,swaps=0;
for(const c of fixture.cases)for(const p of c.expected.packets){
 const raw=Buffer.concat([Buffer.from([p.id]),Buffer.from(p.payload,'base64')]);
 const decoded=decoder.parsePacketBuffer(Buffer.from(raw)).data;
 assert.ok(['entity_equipment','entity_status'].includes(decoded.name));
 if(decoded.name==='entity_equipment')entries+=decoded.params.equipments.length;
 else{assert.equal(decoded.params.entityStatus,55);swaps++;}
 assert.deepEqual(serializer.createPacketBuffer(decoded),raw);packets++;
}
const report={ok:true,minecraft:'1.21',protocol:767,source:'actualJava equipment packet corpus decoded/re-encoded by independent minecraft-protocol1.68.0',cases:fixture.cases.length,packets,equipment_entries:entries,hand_swap_status55_packets:swaps};
await writeFile(new URL('../evidence/equipment-interop-conformance.json',import.meta.url),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify(report));
