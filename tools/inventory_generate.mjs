#!/usr/bin/env node
// Reproducible wire-schema/data extraction from the package-lock-pinned primary
// PrismarineJS Minecraft 1.21 data. No server implementation is copied.
import {createRequire} from 'node:module';
import {writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import path from 'node:path';
const require=createRequire(import.meta.url);
const md=require('minecraft-data')('1.21');
const mc=require('minecraft-protocol');
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
if(md.version.version!==767)throw new Error('Expected exact protocol 767');
const types=md.protocol.types;
const componentNames=types.SlotComponentType[1].mappings;
const fields=types.SlotComponent[1][1].type[1].fields;
// Exact 1.21 bytecode/default-wire corrections to upstream's shared-767
// schema. PotionContents has three fields; FoodProperties has Optional<Slot>
// and PossibleEffect contains the complete MobEffectInstance, not only its ID.
fields.potion_contents[1]=fields.potion_contents[1].filter(f=>f.name!=='customName');
types.NonEmptySlot=types.Slot;
fields.food[1].find(f=>f.name==='usingConvertsTo').type=['option','NonEmptySlot'];
for(const name of ['charged_projectiles','bundle_contents'])fields[name][1][0].type[1].type='NonEmptySlot';
fields.food[1].find(f=>f.name==='effects').type[1].type[1].find(f=>f.name==='effect').type='ItemPotionEffect';

const used=new Set();
function collect(schema){
 if(typeof schema==='string'){
  if(types[schema]&&!used.has(schema)){used.add(schema);collect(types[schema]);}
 }else if(Array.isArray(schema))for(const value of schema)collect(value);
 else if(schema&&typeof schema==='object')for(const value of Object.values(schema))collect(value);
}
collect(types.Slot);
const catalog={version:'1.21',protocol:767,
 source:{package:'minecraft-data',version:require('minecraft-data/package.json').version,
  protocolData:'pc/1.21.1 (shared protocol767; requested identity1.21)',
  exact_1_21_overrides:['PotionContents: no customName field','FoodProperties: Optional<Slot> usingConvertsTo','FoodProperties.PossibleEffect: full MobEffectInstance','Food conversion, charged projectiles and bundle contents use nonempty Slot']},
 types:Object.fromEntries([...used].map(name=>[name,types[name]])),
 component_types:Object.fromEntries(Object.entries(componentNames).map(([id,name])=>[id,{name,schema:fields[name]}])),
 items:Object.fromEntries(md.itemsArray.map(item=>[item.id,{name:item.name,stack_size:item.stackSize,
  max_damage:item.maxDurability??null,block_default_state:md.blocksByName[item.name]?.defaultState??null}])),
 blocks:Object.fromEntries(md.blocksArray.map(block=>[block.name,{id:block.id,min_state:block.minStateId,
  max_state:block.maxStateId,default_state:block.defaultState,properties:block.states}]))};
await writeFile(path.join(root,'fixtures/items-1.21.json'),JSON.stringify(catalog)+'\n');

function sample(schema,variant=0,depth=0){
 if(depth>18)throw new Error('Sample recursion exceeded');
 if(typeof schema==='string'){
  if(schema==='Slot')return {itemCount:0};
  if(schema==='NonEmptySlot')return {itemCount:1,itemId:1,addedComponentCount:0,removedComponentCount:0,components:[],removeComponents:[]};
  if(schema==='anonymousNbt')return {type:'compound',value:{probe:{type:'string',value:'λ'}}};
  if(schema==='anonOptionalNbt')return variant?{type:'compound',value:{}}:undefined;
  if(schema==='UUID')return '00112233-4455-6677-8899-aabbccddeeff';
  if(schema==='position')return {x:-17,y:64,z:32};
  if(schema==='string')return 'minecraft:stone';
  if(schema==='void')return undefined;
  if(schema==='bool')return variant!==0;
  if(['varint','i8','u8','i16','u16','i32','f32','f64'].includes(schema))return 1;
  if(types[schema])return sample(types[schema],variant,depth+1);
  throw new Error('Unknown sample type '+schema);
 }
 const [kind,options]=schema;
 if(kind==='container'){
  const out={};
  for(const f of options){
   if(Array.isArray(f.type)&&f.type[0]==='switch'){
    const choice=f.type[1].fields[String(out[f.type[1].compareTo])]??f.type[1].default;
    const value=sample(choice,variant,depth+1);if(f.name)out[f.name]=value;else Object.assign(out,value);
   }else{const value=sample(f.type,variant,depth+1);if(f.name)out[f.name]=value;else Object.assign(out,value);}
  }return out;
 }
 if(kind==='mapper')return Object.values(options.mappings)[0];
 if(kind==='option')return variant&&depth<9?sample(options,variant,depth+1):undefined;
 if(kind==='array')return depth<9?[sample(options.type,variant,depth+1)]:[];
 if(kind==='registryEntryHolder')return variant?{[options.otherwise.name]:sample(options.otherwise.type,variant,depth+1)}:{[options.baseName]:0};
 if(kind==='registryEntryHolderSet')return variant?{[options.base.name]:sample(options.base.type,variant,depth+1)}:{[options.otherwise.name]:[1,2]};
 if(kind==='bitfield')return Object.fromEntries(options.map(x=>[x.name,0]));
 if(kind==='pstring')return 'minecraft:stone';
 throw new Error('Unknown sample constructor '+kind);
}
const serializer=mc.createSerializer({state:'play',isServer:false,version:'1.21'});
const decoder=mc.createDeserializer({state:'play',isServer:true,version:'1.21'});
const vectors=[];
function vector(name,item){
 const value={name:'set_creative_slot',params:{slot:36,item}};
 // The upstream registryEntryHolder inline writer advances past the zero
 // discriminator without writing it. Its allocUnsafe convenience API can
 // therefore emit uninitialized bytes. Use its same independent writer with
 // an explicitly zero-initialized destination, then verify with its reader.
 const packet=Buffer.alloc(serializer.proto.sizeOf(value,'packet'));
 const written=serializer.proto.write(value,packet,0,'packet');
 if(written!==packet.length)throw new Error('Independent encoder length mismatch: '+name);
 const parsed=decoder.parsePacketBuffer(packet);
 if(parsed.metadata.size!==packet.length)throw new Error('Independent decoder did not consume vector: '+name);
 // Creative slot packet ID=0x32, i16slot follows, then the complete Slot.
 if(packet[0]!==0x32)throw new Error('Creative packet identifier changed');
 vectors.push({name,slot_base64:packet.subarray(3).toString('base64'),item});
}
vector('empty',{itemCount:0});
vector('stone',{itemCount:64,itemId:md.itemsByName.stone.id,addedComponentCount:0,removedComponentCount:0,components:[],removeComponents:[]});
for(const [id,name] of Object.entries(componentNames)){
 for(const variant of [0,1]){
  const data=sample(fields[name],variant);
  vector(name+'-'+variant,{itemCount:1,itemId:md.itemsByName.stone.id,addedComponentCount:1,removedComponentCount:0,
   components:[{type:name,data}],removeComponents:[]});
 }
}
vector('removed-components',{itemCount:1,itemId:md.itemsByName.stone.id,addedComponentCount:0,removedComponentCount:2,
 components:[],removeComponents:[{type:'custom_name'},{type:'max_stack_size'}]});
const clientbound=mc.createSerializer({state:'play',isServer:true,version:'1.21'});
const clientReader=mc.createDeserializer({state:'play',isServer:false,version:'1.21'});
function packetVector(name,params){
 const value={name,params};const buffer=Buffer.alloc(clientbound.proto.sizeOf(value,'packet'));
 const used=clientbound.proto.write(value,buffer,0,'packet');
 if(used!==buffer.length||clientReader.parsePacketBuffer(buffer).metadata.size!==buffer.length)throw new Error('Inventory packet oracle length mismatch');
 return {name,payload_base64:buffer.subarray(1).toString('base64')};
}
const stone={itemCount:64,itemId:1,addedComponentCount:0,removedComponentCount:0,components:[],removeComponents:[]};
const inventory=Array.from({length:46},()=>({itemCount:0}));inventory[36]=stone;inventory[45]=stone;
const packets=[packetVector('window_items',{windowId:0,stateId:2,items:inventory,carriedItem:{itemCount:0}}),
 packetVector('set_slot',{windowId:0,stateId:2,slot:36,item:stone})];
const Block=require('prismarine-block')('1.21');
const states=[];
for(const block of md.blocksArray){
 for(const state of new Set([block.minStateId,block.defaultState,block.maxStateId])){
  const properties=Object.fromEntries(Object.entries(Block.fromStateId(state,0).getProperties()).map(([k,v])=>[k,String(v)]));
  states.push({base:block.defaultState,state,properties});
 }
}
await writeFile(path.join(root,'tools/inventory_vectors.json'),JSON.stringify({version:'1.21',protocol:767,component_count:57,vectors,packets,states},null,2)+'\n');
console.log(JSON.stringify({items:md.itemsArray.length,blocks:md.blocksArray.length,components:57,vectors:vectors.length}));
