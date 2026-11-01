// Independent protocol serializer supplies player field and particle wire values.
import mc from 'minecraft-protocol';import data from 'minecraft-data';
const d=data('1.21'), serializer=mc.createSerializer({state:'play',isServer:true,version:'1.21'});
const cases=[];const add=(op,extra={})=>cases.push({op,...extra});
function value(id,type,value){const bytes=serializer.createPacketBuffer({name:'entity_metadata',params:{entityId:17,metadata:[{key:id,type,value}]}});return bytes.subarray(4,-1).toString('base64');}
const fields=[['byte',5],['int',123],['optional_component',{type:'string',value:'Name'}],['boolean',true],['boolean',true],['boolean',true],['pose',5],['int',99],['byte',3],['float',20],['particles',[{type:'entity_effect',data:-16711936}]],['boolean',true],['int',3],['int',4],['optional_block_pos',{x:-3,y:70,z:8}],['float',12.5],['int',99],['byte',127],['byte',0],['compound_tag',{type:'compound',value:{id:{type:'string',value:'minecraft:parrot'}}}],['compound_tag',{type:'compound',value:{Variant:{type:'int',value:2}}}]];
add('new');add('drain');
for(let id=0;id<fields.length;id++){const [type,val]=fields[id], payload=value(id,type,val);add('set',{id,payload,force:false});add('drain');add('reuse',{id,force:false});add('drain');add('reuse',{id,force:true});add('drain');}
for(const nbt of [
 {type:'compound',value:{text:{type:'string',value:'Name'}}},
 {type:'list',value:{type:'string',value:['Name']}},
 {type:'compound',value:{text:{type:'string',value:'Name'},unknown:{type:'int',value:99}}},
]){add('set',{id:2,payload:value(2,'optional_component',nbt),force:false});add('drain');}
add('new');
for(const p of d.particlesArray){let val;switch(p.name){
 case 'block':case 'block_marker':case 'falling_dust':case 'dust_pillar':val=1;break;
 case 'dust':val={red:1,green:.5,blue:.25,scale:2};break;
 case 'dust_color_transition':val={fromRed:1,fromGreen:.5,fromBlue:.25,scale:2,toRed:.25,toGreen:.5,toBlue:1};break;
 case 'entity_effect':val=-16711936;break;
 case 'item':val={itemCount:1,itemId:1,addedComponentCount:0,removedComponentCount:0,components:[],removeComponents:[]};break;
 case 'sculk_charge':val=.25;break;
 case 'shriek':val=7;break;
 case 'vibration':val={positionType:'block',position:{x:-3,y:70,z:8},ticks:5};break;
 default:val=undefined;
 }
 const payload=value(10,'particles',[{type:p.name,data:val}]);
 add('set',{id:10,payload,force:false,particle:p.name});add('drain');add('set',{id:10,payload,force:false});add('drain');add('reuse',{id:10,force:false});add('drain');
}
for(const bits of [0,0x80000000,0x7fc00000,0x7fc00001,0xffc00000,0x3f800000,0x41a00000]){const bytes=Buffer.alloc(4);bytes.writeUInt32BE(bits);add('set',{id:9,payload:bytes.toString('base64'),force:false});add('drain');}
console.log(JSON.stringify(cases));
