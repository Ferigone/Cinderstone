#!/usr/bin/env node
// Independent Node/OpenSSL reference; no Rust cryptography is mirrored here.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import {spawn,spawnSync} from 'node:child_process';
import {createInterface} from 'node:readline';
import {writeFile,mkdir} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import path from 'node:path';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const binary=process.env.MC121_AUTH_PROBE??path.join(root,'target/debug/auth_probe');
if(!process.env.MC121_AUTH_PROBE){
  const build=spawnSync(process.env.CARGO??'cargo',['build','--locked','--bin','auth_probe'],{cwd:root,env:process.env,stdio:['ignore','ignore','pipe'],encoding:'utf8',timeout:180000});
  if(build.error)throw build.error;
  assert.equal(build.status,0,build.stderr);
}
const processHandle=spawn(binary,[],{cwd:root,stdio:['pipe','pipe','pipe']});
const input=createInterface({input:processHandle.stdout});
const lines=[];const waiting=[];let stderr='';
processHandle.stderr.on('data',data=>stderr+=data);
input.on('line',line=>{if(waiting.length)waiting.shift()(line);else lines.push(line);});
const timeout=setTimeout(()=>{processHandle.kill();throw new Error('Auth crypto probe deadline; '+stderr);},30000);
const next=async()=>JSON.parse(lines.length?lines.shift():await new Promise(resolve=>waiting.push(resolve)));
const ask=async value=>{processHandle.stdin.write(JSON.stringify(value)+'\n');return next();};
function varint(value){const a=[];do{let b=value&127;value>>>=7;if(value)b|=128;a.push(b);}while(value);return Buffer.from(a);}
function digest(serverId,secret,publicKey){
  const hash=crypto.createHash('sha1').update(Buffer.from(serverId,'latin1')).update(secret).update(publicKey).digest();
  let value=BigInt('0x'+hash.toString('hex'));
  if(hash[0]&128)value-=1n<<160n;
  return value.toString(16);
}
let hashes=0,cipherCases=0,negativeResponses=0;
try{
  const challenge=await next();assert.equal(challenge.kind,'public_challenge');
  const spki=Buffer.from(challenge.spki,'base64');
  const token=Buffer.from(challenge.token,'base64');assert.equal(token.length,4);
  const publicKey=crypto.createPublicKey({key:spki,format:'der',type:'spki'});
  assert.equal(publicKey.asymmetricKeyDetails.modulusLength,1024);
  const request=Buffer.from(challenge.request,'base64');assert.equal(request[0],0);assert.equal(request.at(-1),1);
  const secret=crypto.randomBytes(16);
  const rsa=plain=>crypto.publicEncrypt({key:publicKey,padding:crypto.constants.RSA_PKCS1_PADDING},plain);
  const encryptedSecret=rsa(secret),encryptedToken=rsa(token);
  const payload=Buffer.concat([varint(encryptedSecret.length),encryptedSecret,varint(encryptedToken.length),encryptedToken]);
  const accepted=await ask({operation:'accept',payload:payload.toString('base64')});
  assert.equal(accepted.ok,true);assert.equal(accepted.session_hash,digest('',secret,spki));
  for(const bad of [Buffer.concat([payload,Buffer.from([0])]),Buffer.from([0,0]),Buffer.concat([varint(128),encryptedSecret,varint(128),rsa(Buffer.alloc(4,token[0]^255))])]){
    assert.equal((await ask({operation:'accept',payload:bad.toString('base64')})).ok,false);negativeResponses++;
  }
  for(let index=0;index<100;index++){
    const secret=crypto.randomBytes(16),pub=crypto.randomBytes(index*3),id=index%2?'':'fixture';
    const value=await ask({operation:'hash',secret:secret.toString('base64'),public:pub.toString('base64'),server_id:id});
    assert.equal(value.ok,true);assert.equal(value.hash,digest(id,secret,pub));hashes++;
  }
  for(const length of [0,1,2,15,16,17,255,256,257,65535,65536,65537,131073]){
    const key=crypto.randomBytes(16),plain=crypto.randomBytes(length);
    const cipher=crypto.createCipheriv('aes-128-cfb8',key,key);
    const expected=Buffer.concat([cipher.update(plain),cipher.final()]);
    const observed=await ask({operation:'cipher',secret:key.toString('base64'),plain:plain.toString('base64'),strides:[0,1,2,7,15,16,31,8192,65536]});
    assert.equal(observed.ok,true);assert.ok(Buffer.from(observed.cipher,'base64').equals(expected));
    assert.ok(Buffer.from(observed.restored,'base64').equals(plain));cipherCases++;
  }
  const report={ok:true,scope:'RSA1024/SPKI/PKCS1v1.5, AES128CFB8 across asynchronous chunk boundaries, signed SHA1 session hash',
    reference:'Node.js/OpenSSL crypto',rsa_challenge_cases:1,rejected_encryption_responses:negativeResponses,
    session_hash_cases:hashes,aes_stream_cases:cipherCases,real_Microsoft_account_login:false};
  await mkdir(path.join(root,'evidence'),{recursive:true});
  await writeFile(path.join(root,'evidence/auth-crypto-conformance.json'),JSON.stringify(report,null,2)+'\n');
  console.log(JSON.stringify(report,null,2));
}finally{
  clearTimeout(timeout);processHandle.stdin.end();input.close();processHandle.kill();
}
