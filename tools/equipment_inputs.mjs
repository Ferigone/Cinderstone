import mc from 'minecraft-protocol';import data from 'minecraft-data';
const d=data('1.21'),serializer=mc.createSerializer({state:'play',isServer:true,version:'1.21'});
const empty={itemCount:0};
const item=(name,count=1,components=[],removeComponents=[])=>({itemCount:count,itemId:d.itemsByName[name].id,addedComponentCount:components.length,removedComponentCount:removeComponents.length,components,removeComponents});
function slot(stack){return serializer.createPacketBuffer({name:'entity_equipment',params:{entityId:17,equipments:[{slot:0,item:stack}]}}).subarray(3).toString('base64');}
const cases=[{op:'new'}];let slots=Array(7).fill(empty);
function step(next=slots,label=''){slots=next;cases.push({op:'step',label,slots:slots.map(slot)});}
step();step([item('stone',3),item('dirt',2),empty,empty,empty,empty,empty],'twohands');
step([item('dirt',2),item('stone',3),empty,empty,empty,empty,empty],'handswap');
step([item('stone',3),item('dirt',2),empty,empty,empty,item('iron_helmet'),empty],'swap-plus-armor');
step([item('stone',4),item('dirt',2),item('iron_boots'),item('iron_leggings'),item('iron_chestplate'),item('iron_helmet'),item('wolf_armor')],'allslots');
step([item('stone',4,[{type:'max_stack_size',data:64}]),...slots.slice(1)],'explicit-default-suppressed');
step([item('iron_sword'),...slots.slice(1)],'sword');step([item('iron_sword',1,[{type:'damage',data:0}]),...slots.slice(1)],'explicit-damage-default-suppressed');
step([item('iron_sword',1,[{type:'damage',data:5}]),...slots.slice(1)],'damage-change');
step([item('stone',1,[{type:'custom_name',data:{type:'string',value:'Named'}},{type:'max_stack_size',data:12}]),...slots.slice(1)],'named');
step([item('stone',1,[{type:'max_stack_size',data:12},{type:'custom_name',data:{type:'string',value:'Named'}}]),...slots.slice(1)],'patch-order-suppressed');
let seed=767121;const random=n=>{seed=(Math.imul(seed,1664525)+1013904223)>>>0;return seed%n;};
const names=['stone','dirt','stick','iron_sword','diamond_helmet','shield','bow','iron_boots','iron_chestplate','elytra'];
for(let i=0;i<600;i++){
 let next=slots.slice();
 if(i%13===0){[next[0],next[1]]=[next[1],next[0]];}
 else{for(let j=0;j<1+random(3);j++){const index=random(7);next[index]=random(5)===0?empty:item(names[random(names.length)],1+random(5));}}
 step(next,'random-'+i);
}
console.log(JSON.stringify(cases));
