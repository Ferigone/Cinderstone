#!/usr/bin/env node
// Captures exact vanilla light masks/arrays for an already generatedworld.
import mc from 'minecraft-protocol';
import fs from 'node:fs';
import path from 'node:path';
const out=process.argv[3];fs.mkdirSync(out,{recursive:true});
const client=mc.createClient({host:'127.0.0.1',port:Number(process.argv[2]),username:'StorageCapture',auth:'offline',version:'1.21',hideErrors:true,keepAlive:false,clientSettings:{locale:'en_us',viewDistance:2,chatFlags:0,chatColors:true,skinParts:127,mainHand:1,enableTextFiltering:false,enableServerListing:true}});
const serializer=mc.createSerializer({state:'play',isServer:true,version:'1.21'});
const captured=new Set();let closing=false;let timer;
function stop(error){if(closing)return;closing=true;clearTimeout(timer);if(error){console.error(error);process.exitCode=1;}client.end('capture-complete');client.socket?.destroy();clearTimeout(client.closeTimer);}
timer=setTimeout(()=>stop(new Error('capture timeout')),30000);
client.on('error',stop);
client.on('position',data=>{client.write('teleport_confirm',{teleportId:data.teleportId});client.write('position_look',{x:data.x,y:data.y,z:data.z,yaw:data.yaw,pitch:data.pitch,onGround:true});});
client.on('chunk_batch_finished',()=>client.write('chunk_batch_received',{chunksPerTick:10}));
client.on('keep_alive',data=>client.write('keep_alive',{keepAliveId:data.keepAliveId}));
client.on('map_chunk',data=>{const key=`${data.x}.${data.z}`;if(!['0.0','-1.-1','1.1','-1.0','0.-1'].includes(key))return;fs.writeFileSync(path.join(out,`java.${key}.packet`),serializer.createPacketBuffer({name:'map_chunk',params:data}).subarray(1));captured.add(key);console.log(JSON.stringify({chunk:key,skyMask:data.skyLightMask,emptySkyMask:data.emptySkyLightMask,blockMask:data.blockLightMask,emptyBlockMask:data.emptyBlockLightMask,arrays:data.skyLight.length},(_,v)=>typeof v==='bigint'?v.toString():v));if(captured.size===5)stop();});
