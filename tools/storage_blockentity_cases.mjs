#!/usr/bin/env node
import fs from 'node:fs';import nbt from 'prismarine-nbt';
const defaults=JSON.parse(fs.readFileSync('tools/storage_blockentity_defaults.json'));const cases=[];
const str=value=>({type:'string',value});const int=value=>({type:'int',value});const byte=value=>({type:'byte',value});const long=value=>({type:'long',value:BigInt(value)});const compound=value=>({type:'compound',value});const list=(type,value)=>({type:'list',value:{type,value}});
const item=(id='minecraft:diamond',count=3)=>({id:str(id),count:int(count)});
function emit(record,label,change,state_id=record.state_id){const tag=nbt.parseUncompressed(Buffer.from(record.full_nbt,'base64'));change(tag.value);cases.push({name:record.name,label,state_id,input_nbt:nbt.writeUncompressed(tag).toString('base64')});}
for(const record of defaults){emit(record,'default',()=>{});emit(record,'unknown-and-components',t=>{t.RustPrivateMarker=str('do-not-publish');t.components=compound({'minecraft:custom_data':compound({probe:int(17)})});});
 if(record.update_declaring_class==='dqh')emit(record,'inventory-and-custom-name',t=>{t.Items=list('compound',[{Slot:byte(0),...item()}]);t.CustomName=str('{"text":"Privatecontainer"}');t.LootTable=str('minecraft:chests/simple_dungeon');});
 const id=record.name.substring(10);
 const edit={
  banner:t=>{t.patterns=list('compound',[{pattern:str('minecraft:stripe_top'),color:str('red')}]);t.CustomName=str('{"text":"Banner"}');},
  beacon:t=>{t.Levels=int(3);t.primary_effect=str('minecraft:speed');t.secondary_effect=str('minecraft:regeneration');t.CustomName=str('{"text":"Beacon"}');t.Lock=str('secret');},
  sign:t=>{t.front_text.value.color=str('red');t.front_text.value.has_glowing_text=byte(1);t.front_text.value.messages=list('string',['{"text":"Lineone"}','"Line2"','""','""']);t.is_waxed=byte(1);},
  hanging_sign:t=>{t.back_text.value.color=str('blue');t.is_waxed=byte(1);},
  mob_spawner:t=>{t.Delay={type:'short',value:13};t.SpawnData=compound({entity:compound({id:str('minecraft:pig')})});},
  piston:t=>{t.facing=int(5);t.progress={type:'float',value:0.375};t.extending=byte(1);t.source=byte(1);t.blockState=compound({Name:str('minecraft:stone')});},
  skull:t=>{t.note_block_sound=str('minecraft:entity.pig.ambient');t.profile=compound({name:str('Probe'),id:{type:'intArray',value:[1,2,3,4]}});t.custom_name=str('{"text":"Head"}');},
  structure_block:t=>{t.name=str('minecraft:probe');t.posX=int(-3);t.posY=int(8);t.sizeX=int(4);t.sizeY=int(5);t.sizeZ=int(6);t.mode=str('SAVE');t.seed=long(12345);t.integrity={type:'float',value:0.75};},
  end_gateway:t=>{t.Age=long(12345);t.ExactTeleport=byte(1);t.ExitPortal={type:'intArray',value:[12,34,-56]};},
  conduit:t=>{t.Target={type:'intArray',value:[1,2,3,4]};},
  jigsaw:t=>{t.name=str('minecraft:probe');t.target=str('minecraft:target');t.final_state=str('minecraft:stone');t.selection_priority=int(12);t.placement_priority=int(-3);},
  campfire:t=>{t.Items=list('compound',[{Slot:byte(0),...item('minecraft:beef',1)}]);t.CookingTimes={type:'intArray',value:[1,2,3,4]};},
  brushable_block:t=>{t.item=compound(item());t.hit_direction=int(4);},
  decorated_pot:t=>{t.sherds=list('string',['minecraft:angler_pottery_sherd','minecraft:brick','minecraft:brick','minecraft:archer_pottery_sherd']);t.item=compound(item());},
  trial_spawner:t=>{t.spawn_data=compound({entity:compound({id:str('minecraft:pig')})});t.next_mob_spawns_at=long(2345);},
  vault:t=>{t.shared_data=compound({display_item:compound(item()),connected_particles_range:{type:'double',value:3}});}
 }[id];if(edit)emit(record,'nondefault-fields',edit,id==='trial_spawner'?26646:record.state_id);
}
fs.writeFileSync('tools/storage_blockentity_queries.jsonl',cases.map(c=>JSON.stringify(c)).join('\n')+'\n');console.log(JSON.stringify({cases:cases.length}));
