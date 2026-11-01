// Independent Prismarine wire encoding for semantic stacks returned by Mojang's CODEC.
import mc from 'minecraft-protocol';import fs from 'node:fs';import readline from 'node:readline';
const root=new URL('../',import.meta.url),catalog=JSON.parse(fs.readFileSync(new URL('fixtures/items-1.21.json',root))),metadata=JSON.parse(fs.readFileSync(new URL('tools/crafting_metadata.json',root)));
const ids=Object.fromEntries(Object.entries(catalog.items).map(([i,v])=>['minecraft:'+v.name,+i]));
const registries=JSON.parse(fs.readFileSync(new URL('../../mc121-oracle/reports/reports/registries.json',import.meta.url)));
// Apply exact-version corrections proved by Mojang's unchanged stream codecs.
const proto=(await import('minecraft-data')).default('1.21').protocol.types;
const fields=proto.SlotComponent[1][1].type[1].fields;
fields.potion_contents[1]=fields.potion_contents[1].filter(f=>f.name!=='customName');
proto.NonEmptySlot=proto.Slot;
fields.food[1].find(f=>f.name==='usingConvertsTo').type=['option','NonEmptySlot'];
for(const name of ['charged_projectiles','bundle_contents'])fields[name][1][0].type[1].type='NonEmptySlot';
fields.food[1].find(f=>f.name==='effects').type[1].type[1].find(f=>f.name==='effect').type='ItemPotionEffect';
const serialize=mc.createSerializer({state:'play',isServer:false,version:'1.21'});
const comp=Object.fromEntries(Object.entries(catalog.component_types).map(([i,v])=>[v.name,+i]));
const strip=n=>n.replace(/^minecraft:/,'');
const text=n=>{if(typeof n==='string'){try{n=JSON.parse(n);}catch{}}return {type:'string',value:typeof n==='string'?n:n.text};};
function explosion(v){return {shape:v.shape??'small_ball',colors:v.colors??[],fadeColors:v.fade_colors??[],hasTrail:v.has_trail??false,hasTwinkle:v.has_twinkle??false};}
function component(name,v){switch(name){
case 'max_stack_size':case 'max_damage':case 'damage':case 'repair_cost':case 'map_id':case 'map_post_processing':return v;
case 'custom_name':case 'item_name':return text(v);
case 'dyed_color':return typeof v==='number'?{color:v,showTooltip:true}:{color:v.rgb,showTooltip:v.show_in_tooltip??true};
case 'enchantments':return {enchantments:Object.entries(v.levels??{}).map(([id,level])=>({id:metadata.registries['minecraft:enchantment'][id],level})),showTooltip:v.show_in_tooltip??true};
case 'potion_contents':return {potionId:v.potion?registries['minecraft:potion'].entries[v.potion].protocol_id:undefined,customColor:v.custom_color,customEffects:[],customName:v.custom_name};
case 'suspicious_stew_effects':return {effects:v.map(e=>({effect:registries['minecraft:mob_effect'].entries[e.id].protocol_id,duration:e.duration??160}))};
case 'firework_explosion':return explosion(v);
case 'fireworks':return {flightDuration:v.flight_duration??0,explosions:(v.explosions??[]).map(explosion)};
case 'banner_patterns':return {layers:v.map(e=>({pattern:{patternId:metadata.registries['minecraft:banner_pattern'][e.pattern]},colorId:metadata.dyes[e.color].id}))};
case 'base_color':return metadata.dyes[v].id;
case 'pot_decorations':return {decorations:v.map(i=>ids[i])};
case 'trim':return {material:{materialId:metadata.registries['minecraft:trim_material'][v.material]},pattern:{patternId:metadata.registries['minecraft:trim_pattern'][v.pattern]},showInTooltip:v.show_in_tooltip??true};
case 'written_book_content':return {rawTitle:typeof v.title==='string'?v.title:v.title.raw,filteredTitle:typeof v.title==='string'?undefined:v.title.filtered,author:v.author,generation:v.generation??0,pages:(v.pages??[]).map(p=>({content:text(typeof p==='string'?p:p.raw),filteredContent:typeof p==='string'?undefined:p.filtered?text(p.filtered):undefined})),resolved:v.resolved??false};
default:throw Error('Unimplemented fixture semantic component '+name);
}}
function slot(v){if(!v||!v.id||v.count<=0)return{itemCount:0};let adds=[],removes=[];
for(let [n,data] of Object.entries(v.components??{})){if(n.startsWith('!')){removes.push({type:strip(n.slice(1))});continue;}n=strip(n);adds.push({type:n,data:component(n,data)});}
return {itemCount:v.count??1,itemId:ids[v.id],addedComponentCount:adds.length,removedComponentCount:removes.length,components:adds,removeComponents:removes};}
for await(const line of readline.createInterface({input:process.stdin,crlfDelay:Infinity})){try{const v=JSON.parse(line);const params={slot:36,item:slot(v)};const packet={name:'set_creative_slot',params};const b=Buffer.alloc(serialize.proto.sizeOf(packet,'packet'));serialize.proto.write(packet,b,0,'packet');console.log(JSON.stringify({slot:b.subarray(3).toString('base64')}));}catch(e){console.log(JSON.stringify({error:e.message}));}}
