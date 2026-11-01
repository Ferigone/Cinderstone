import mc from 'minecraft-protocol';import fs from 'node:fs';
const capture=JSON.parse(fs.readFileSync(new URL('../fixtures/configuration.json',import.meta.url)));
const d=mc.createDeserializer({state:'configuration',isServer:false,version:'1.21'});const registries={};
for(const p of capture.packets.filter(p=>p.name==='registry_data')){
 const {params}=d.parsePacketBuffer(Buffer.concat([Buffer.from([p.id]),Buffer.from(p.payload,'base64')])).data;
 registries[params.id]=Object.fromEntries(params.entries.map((v,i)=>[v.key,i]));
}
console.log(JSON.stringify(registries));
