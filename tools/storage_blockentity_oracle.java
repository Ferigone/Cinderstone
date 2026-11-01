package oracle;
import java.lang.reflect.*;import java.io.*;import java.nio.file.*;import java.util.*;import com.google.gson.*;
/** Emits actual default and loaded getUpdateTag results from signed Java1.21. */
public final class storage_blockentity_oracle {
 static Class<?> c(String n)throws Exception{return Class.forName(n);}
 static final Gson G=new Gson();
 static byte[] nbt(Object tag)throws Exception{ByteArrayOutputStream b=new ByteArrayOutputStream();c("uo").getMethod("a",c("ub"),DataOutput.class).invoke(null,tag,new DataOutputStream(b));return b.toByteArray();}
 static String b64(Object tag)throws Exception{return Base64.getEncoder().encodeToString(nbt(tag));}
 public static void main(String[] args)throws Exception{
  c("ab").getMethod("a").invoke(null);c("akt").getMethod("a").invoke(null);
  Object registry=c("lt").getField("j").get(null),lookup=c("pd").getMethod("a").invoke(null),pos=c("jd").getConstructor(int.class,int.class,int.class).newInstance(-1,65,-2);
  if(args[0].equals("factories")){
   List<Integer> states=new ArrayList<>();List<JsonObject> templates=new ArrayList<>();Map<String,Integer> keys=new LinkedHashMap<>();int entities=0,nullFactories=0;
   for(int id=0;id<26684;id++){
    Object state=c("dfy").getMethod("a",int.class).invoke(null,id);
    if(!(boolean)c("dtb$a").getMethod("t").invoke(state)){states.add(-1);continue;}
    Object block=c("dtb$a").getMethod("b").invoke(state);Object entity=c("diq").getMethod("a",c("jd"),c("dtc")).invoke(block,pos,state);
    if(entity==null){states.add(-2);nullFactories++;continue;}
    Object type=c("dqh").getMethod("r").invoke(entity);JsonObject t=new JsonObject();t.addProperty("name",c("jz").getMethod("b",Object.class).invoke(registry,type).toString());t.addProperty("type_id",(Integer)c("jz").getMethod("a",Object.class).invoke(registry,type));t.addProperty("full_nbt",b64(c("dqh").getMethod("b",c("jo$a")).invoke(entity,lookup)));
    String key=G.toJson(t);Integer row=keys.get(key);if(row==null){row=templates.size();keys.put(key,row);templates.add(t);}states.add(row);entities++;
   }
   JsonObject result=new JsonObject();result.addProperty("minecraft","1.21");result.addProperty("oracle_inner_sha256","8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01");result.addProperty("description","states[id] = templateindex; -1 statehasnoblockentity, -2 EntityBlock.newBlockEntity returnednull; templatesareactualfreshfactorysaveWithFullMetadata at(-1,65,-2), beforeworldticks");result.add("states",G.toJsonTree(states));result.add("templates",G.toJsonTree(templates));result.addProperty("block_entity_states",entities);result.addProperty("null_factory_states",nullFactories);Files.writeString(Path.of(args[1]),G.toJson(result)+"\n");System.out.println("FACTORIES:"+states.size()+" states, "+entities+" entities, "+nullFactories+" nullfactories, "+templates.size()+" templates");return;
  }
  Field valid=c("dqj").getDeclaredField("U");valid.setAccessible(true);
  Method update=c("dqh").getMethod("a",c("jo$a")),save=c("dqh").getMethod("b",c("jo$a"));
  List<JsonObject> records=new ArrayList<>();
  for(Object type:(Iterable<?>)registry){
   Object block=((Set<?>)valid.get(type)).iterator().next();Object state=c("dfy").getMethod("o").invoke(block);Object entity=c("dqj").getMethod("a",c("jd"),c("dtc")).invoke(type,pos,state);
   if(entity.getClass().getName().equals("dsy")){Object tag=c("ub").getConstructor().newInstance();c("ub").getMethod("a",String.class,int.class).invoke(tag,"facing",0);c("dqh").getMethod("c",c("ub"),c("jo$a")).invoke(entity,tag,lookup);}
   JsonObject r=new JsonObject();r.addProperty("name",c("jz").getMethod("b",Object.class).invoke(registry,type).toString());r.addProperty("id",(Integer)c("jz").getMethod("a",Object.class).invoke(registry,type));r.addProperty("class",entity.getClass().getName());r.addProperty("update_declaring_class",entity.getClass().getMethod("a",c("jo$a")).getDeclaringClass().getName());
   r.addProperty("state_id",(Integer)c("dfy").getMethod("i",c("dtc")).invoke(null,state));
   r.addProperty("full_nbt",b64(save.invoke(entity,lookup)));r.addProperty("update_nbt",b64(update.invoke(entity,lookup)));records.add(r);
  }
  Files.writeString(Path.of(args[0]),G.toJson(records)+"\n");System.out.println("ORACLE:"+records.size());
  if(args.length>1){List<JsonObject> cases=new ArrayList<>();
   for(String line:Files.readAllLines(Path.of(args[1]))){if(line.isBlank())continue;JsonObject q=JsonParser.parseString(line).getAsJsonObject();
    Object key=c("akr").getMethod("a",String.class).invoke(null,q.get("name").getAsString());Object type=c("jz").getMethod("a",c("akr")).invoke(registry,key);int sid=q.get("state_id").getAsInt();Object state=c("dfy").getMethod("a",int.class).invoke(null,sid);
    Object entity=c("dqj").getMethod("a",c("jd"),c("dtc")).invoke(type,pos,state);byte[] bytes=Base64.getDecoder().decode(q.get("input_nbt").getAsString());Object tag=c("uo").getMethod("a",DataInput.class).invoke(null,new DataInputStream(new ByteArrayInputStream(bytes)));c("dqh").getMethod("c",c("ub"),c("jo$a")).invoke(entity,tag,lookup);
    JsonObject r=q.deepCopy();r.addProperty("full_nbt",b64(save.invoke(entity,lookup)));r.addProperty("update_nbt",b64(update.invoke(entity,lookup)));cases.add(r);
   }Files.writeString(Path.of(args[2]),G.toJson(cases)+"\n");System.out.println("ORACLE_CASES:"+cases.size());
  }
 }
}
