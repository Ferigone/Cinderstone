#!/usr/bin/env node
// Independent protocol-767 serializer fixtures, not an encoder copied from Rust.
import mc from 'minecraft-protocol';
import data from 'minecraft-data';
import {createHash} from 'node:crypto';
import {readFile,writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import assert from 'node:assert/strict';

const md=data('1.21');
assert.equal(md.version.version,767);
assert.equal(md.entitiesByName.player.id,128);
const serialize=mc.createSerializer({state:'play',isServer:true,version:'1.21'});
const destination=fileURLToPath(new URL('entity-fixtures.json',import.meta.url));
function packet(name,params){
  const bytes=serialize.createPacketBuffer({name,params});
  let id=0,offset=0;
  do{id|=(bytes[offset]&127)<<(7*offset);offset++;}while(bytes[offset-1]&128);
  return {name,id,payload:bytes.subarray(offset).toString('base64')};
}
function uuid(name){
  const bytes=createHash('md5').update('OfflinePlayer:'+name).digest();
  bytes[6]=(bytes[6]&15)|48;bytes[8]=(bytes[8]&63)|128;
  return [...bytes];
}
function uuidText(bytes){const v=Buffer.from(bytes).toString('hex');return `${v.slice(0,8)}-${v.slice(8,12)}-${v.slice(12,16)}-${v.slice(16,20)}-${v.slice(20)}`;}
const origin={x:8.5,y:65,z:8.5,yaw:0,pitch:0,on_ground:true};
function player(id,name,position=origin){return {entity_id:id,profile:{uuid:uuid(name),name,properties:[]},position,
 gamemode:1,listed:true,latency:0,display_name:null,chat_session:null,metadata:{flags:0,skin_parts:127,main_hand:1}};}
function angular(degrees){const units=Math.fround(Math.fround(Math.fround(degrees)*256)/360);return (Math.floor(units)<<24)>>24;}
function truncAngular(degrees){const units=Math.fround(Math.fround(Math.fround(degrees)*256)/360);return (Math.trunc(units)<<24)>>24;}
function info(p,actions=63){
 const flags={add_player:!!(actions&1),initialize_chat:!!(actions&2),update_game_mode:!!(actions&4),
 update_listed:!!(actions&8),update_latency:!!(actions&16),update_display_name:!!(actions&32)};
 const entry={uuid:uuidText(p.profile.uuid),player:{name:p.profile.name,properties:p.profile.properties},gamemode:p.gamemode,
 listed:p.listed?1:0,latency:p.latency,displayName:p.display_name?{type:'string',value:'Display'}:null,
 chatSession:p.chat_session?{uuid:uuidText(p.chat_session.uuid),publicKey:{expireTime:BigInt(p.chat_session.expires_at),
 keyBytes:Buffer.from(p.chat_session.public_key),keySignature:Buffer.from(p.chat_session.key_signature)}}:null};
 return packet('player_info',{action:flags,data:[entry]});
}
function head(id,yaw){return packet('entity_head_rotation',{entityId:id,headYaw:angular(yaw)});}
function metadata(id,m){const entries=[];
 if(m.flags)entries.push({key:0,type:'byte',value:(m.flags<<24)>>24});
 if(m.pose)entries.push({key:6,type:'pose',value:m.pose});
 entries.push({key:9,type:'float',value:20});
 if(m.skin_parts)entries.push({key:17,type:'byte',value:(m.skin_parts<<24)>>24});
 if(m.main_hand!==1)entries.push({key:18,type:'byte',value:m.main_hand});
 return packet('entity_metadata',{entityId:id,metadata:entries});}
function spawn(p){return [info(p),packet('spawn_entity',{entityId:p.entity_id,objectUUID:uuidText(p.profile.uuid),type:128,
 x:p.position.x,y:p.position.y,z:p.position.z,pitch:angular(p.position.pitch),yaw:angular(p.position.yaw),
 headPitch:angular(p.position.yaw),objectData:0,velocity:{x:0,y:0,z:0}}),head(p.entity_id,p.position.yaw),metadata(p.entity_id,p.metadata)];}
const cases=[];
const add=(name,kind,input,expected)=>cases.push({name,kind,input,expected});
const a=player(1,'Alice');add('plain-player-spawn','spawn',{player:a},spawn(a));
const b=player(300,'Signed',{...origin,x:-128.0001220703125,z:-0.5,yaw:-1,pitch:91.125,on_ground:false});
b.profile.properties=[{name:'textures',value:'signed-texture-value',signature:'base64-signature'}];
b.chat_session={uuid:Array(16).fill(7),expires_at:1700000000000,public_key:[1,2,3,4],key_signature:[9,8,7]};
b.display_name=[8,0,7,...Buffer.from('Display')];b.metadata={flags:0xa2,skin_parts:63,main_hand:0};b.gamemode=3;b.latency=127;b.listed=false;
add('signed-profile-chat-display-spawn','spawn',{player:b},spawn(b));
for(const actions of [1,2,4,8,16,32,0,63])add('info-mask-'+actions,'info',{player:b,actions},[info(b,actions)]);
const id=300;
const from={...origin,x:0,y:0,z:0};
add('positive-relative-quarter','movement',{id,previous:from,current:{...from,x:.25}},[
 packet('rel_entity_move',{entityId:id,dX:1024,dY:0,dZ:0,onGround:true})]);
add('negative-half-tie-keeps-coordinate','movement',{id,previous:from,current:{...from,x:-.5/4096}},[]);
add('negative-one-and-half-tie','movement',{id,previous:from,current:{...from,x:-1.5/4096}},[
 packet('rel_entity_move',{entityId:id,dX:-1,dY:0,dZ:0,onGround:true})]);
add('negative-small-rotation','movement',{id,previous:from,current:{...from,yaw:-1,pitch:-1}},[
 packet('entity_look',{entityId:id,yaw:-1,pitch:-1,onGround:true}),head(id,-1)]);
add('combined-relative-rotation','movement',{id,previous:from,current:{...from,x:-1.25,y:2.5,z:-3.75,yaw:90,pitch:45}},[
 packet('entity_move_look',{entityId:id,dX:-5120,dY:10240,dZ:-15360,yaw:64,pitch:32,onGround:true}),head(id,90)]);
add('negative-short-boundary','movement',{id,previous:from,current:{...from,x:-8}},[
 packet('rel_entity_move',{entityId:id,dX:-32768,dY:0,dZ:0,onGround:true})]);
add('positive-short-boundary','movement',{id,previous:from,current:{...from,x:32767/4096}},[
 packet('rel_entity_move',{entityId:id,dX:32767,dY:0,dZ:0,onGround:true})]);
add('positive-overflow-teleport','movement',{id,previous:from,current:{...from,x:8,yaw:-1,pitch:-1}},[
 packet('entity_teleport',{entityId:id,x:8,y:0,z:0,yaw:truncAngular(-1),pitch:truncAngular(-1),onGround:true}),head(id,-1)]);
add('ground-state-forces-teleport','movement',{id,previous:from,current:{...from,on_ground:false}},[
 packet('entity_teleport',{entityId:id,x:0,y:0,z:0,yaw:0,pitch:0,onGround:false})]);
for(const animation of [0,3,5])add('animation-'+animation,'animation',{id,animation},[packet('animation',{entityId:id,animation})]);
add('destroy-and-player-list-remove','remove',{id,uuid:b.profile.uuid},[
 packet('entity_destroy',{entityIds:[id]}),packet('player_remove',{players:[uuidText(b.profile.uuid)]})]);
const result={format:'mc121-entity-wire-fixtures-v1',minecraft:'1.21',protocol:767,
 producer:'minecraft-protocol 1.68.0 independent serializer',
 quantization_source:'Minecraft1.21 aig/abv/afw/aqt actual bytecode',cases};
const text=JSON.stringify(result,null,2)+'\n';
if(process.argv.includes('--check')){assert.equal(await readFile(destination,'utf8'),text);console.log(JSON.stringify({passed:true,cases:cases.length}));}
else{await writeFile(destination,text);console.log(JSON.stringify({written:destination,cases:cases.length}));}
