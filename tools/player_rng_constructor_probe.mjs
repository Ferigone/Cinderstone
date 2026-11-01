import mc from 'minecraft-protocol';
const port=Number(process.argv[2]);
const names=process.argv.slice(3);
const settings={locale:'en_us',viewDistance:2,chatFlags:0,chatColors:true,skinParts:127,mainHand:1,enableTextFiltering:false,enableServerListing:true};
for(const username of names){
 await new Promise((resolve,reject)=>{
  const client=mc.createClient({host:'127.0.0.1',port,username,auth:'offline',version:'1.21',keepAlive:false,hideErrors:true,clientSettings:settings});let complete=false;
  const timeout=setTimeout(()=>{client.end();reject(new Error('constructor login timeout '+username));},30000);
  client.on('error',reject);client.on('kick_disconnect',data=>reject(new Error(JSON.stringify(data))));
  client.on('keep_alive',data=>client.write('keep_alive',data));client.on('chunk_batch_finished',()=>client.write('chunk_batch_received',{chunksPerTick:16}));
  client.on('position',data=>{if(complete)return;complete=true;client.write('teleport_confirm',{teleportId:data.teleportId});client.end();});
  client.on('end',()=>{clearTimeout(timeout);if(complete){console.log(JSON.stringify({username,joined:true,uuid:client.uuid}));resolve();}else reject(new Error('ended before PLAY position '+username));});
 });
}
