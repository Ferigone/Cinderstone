import {writeFile, mkdir} from 'node:fs/promises';
import {resolve, join} from 'node:path';
import {pathToFileURL, fileURLToPath} from 'node:url';

const root=resolve(fileURLToPath(new URL('..',import.meta.url)));
const install=process.env.REA_HOME??'/root/.local/share/rea-cloud';
const target=resolve(process.argv[2]??join(root,'runtime/oracle/server-1.21.jar'));
const {Client}=await import(pathToFileURL(join(install,'node_modules/@modelcontextprotocol/client/dist/index.mjs')));
const {StdioClientTransport}=await import(pathToFileURL(join(install,'node_modules/@modelcontextprotocol/client/dist/stdio.mjs')));
const client=new Client({name:'mc121-port-audit',version:'0.1.0'});
const transport=new StdioClientTransport({command:process.execPath,
  args:[join(install,'node_modules/rea-agents/scripts/rea.mjs'),'mcp'],env:{...process.env},stderr:'pipe'});
const audit={target,operations:[],limitations:['Archive inventory does not decompile JVM bytecode or prove gameplay parity.']};
const unpack=r=>r.structuredContent??JSON.parse(r.content.find(c=>c.type==='text').text);
try {
  await client.connect(transport);
  audit.server=client.getServerVersion();
  const {tools}=await client.listTools();
  for(const name of ['open_binary','inspect_artifact','close_binary'])
    if(!tools.some(t=>t.name===name))throw new Error('Required REA operation missing: '+name);
  for(const [name,args] of [['open_binary',{path:target}],['inspect_artifact',{detail:'complete'}]]){
    const result=await client.callTool({name,arguments:args});
    audit.operations.push({name,is_error:!!result.isError,result:unpack(result)});
    if(result.isError)throw new Error('REA returned an explicit error for '+name);
  }
  await client.callTool({name:'close_binary',arguments:{}});
  console.log(JSON.stringify({server:audit.server,operations:audit.operations.map(o=>({name:o.name,is_error:o.is_error})),output:'evidence/rea-artifact.json'}));
}finally {
  await mkdir(join(root,'evidence'),{recursive:true});
  await writeFile(join(root,'evidence/rea-artifact.json'),JSON.stringify(audit,null,2)+'\n');
  await client.close();
}
