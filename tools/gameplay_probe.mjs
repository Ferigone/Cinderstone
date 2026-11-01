#!/usr/bin/env node
// A narrow creative-fixture test, not a claim of survival or vanilla parity.
import mc from 'minecraft-protocol';
import {createRequire} from 'node:module';
const require=createRequire(import.meta.url);
const Chunk=require('prismarine-chunk')('1.21');
const args=process.argv.slice(2);
const port=Number(args[args.indexOf('--port')+1]??25565);
const expectAir=args.includes('--expect-air');
const target={x:8,y:63,z:8};
const result={ok:false,scope:expectAir?'persisted-creative-block':'creative-block-break',
  initialBlock:null,position:false,breakSent:false,acknowledged:false,blockUpdate:false};
let finished=false,closing=false,doneTimer;
const client=mc.createClient({host:'127.0.0.1',port,username:'RustBlockProbe',
  auth:'offline',version:'1.21',keepAlive:false,hideErrors:true});
const deadline=setTimeout(()=>finish(new Error('Creative-fixture probe deadline exceeded')),15000);
function finish(error){
  if(finished)return;
  finished=true;closing=true;clearTimeout(deadline);clearTimeout(doneTimer);
  result.ok=!error;
  if(error)result.error=String(error.message??error);
  console.log(JSON.stringify(result));
  client.end();
  process.exitCode=error?1:0;
}
function advance(){
  if(!result.position||result.initialBlock===null)return;
  if(expectAir){
    if(result.initialBlock!==0)return finish(new Error('Removed block did not persist after server restart'));
    if(!doneTimer)doneTimer=setTimeout(()=>finish(),300);
  }else if(!result.breakSent){
    if(result.initialBlock!==9)return finish(new Error('Expected exact 1.21 grass state 9 before creative break'));
    result.breakSent=true;
    client.write('block_dig',{status:0,location:target,face:1,sequence:1});
  }else if(result.acknowledged&&result.blockUpdate&&!doneTimer){
    doneTimer=setTimeout(()=>finish(),300);
  }
}
client.on('error',finish);
client.on('end',()=>{if(!closing)finish(new Error('Server ended creative-fixture test'));});
client.on('kick_disconnect',data=>finish(new Error('Server kick: '+JSON.stringify(data))));
client.on('keep_alive',data=>client.write('keep_alive',data));
client.on('position',data=>{
  try{
    result.position=true;
    client.write('teleport_confirm',{teleportId:data.teleportId});
    client.write('position_look',{x:data.x,y:data.y,z:data.z,yaw:data.yaw,pitch:data.pitch,onGround:true});
    advance();
  }catch(error){finish(error);}
});
client.on('chunk_batch_finished',()=>client.write('chunk_batch_received',{chunksPerTick:16}));
client.on('packet',(data,metadata)=>{
  try{
    if(metadata.state!=='play')return;
    if(metadata.name==='map_chunk'&&data.x===0&&data.z===0){
      const chunk=new Chunk({minY:-64,worldHeight:384});
      chunk.load(data.chunkData);
      result.initialBlock=chunk.getBlockStateId(target);
    }
    if(metadata.name==='acknowledge_player_digging'&&data.sequenceId===1)result.acknowledged=true;
    if(metadata.name==='block_change'&&data.location.x===8&&data.location.y===63&&data.location.z===8&&data.type===0)result.blockUpdate=true;
    advance();
  }catch(error){finish(error);}
});
