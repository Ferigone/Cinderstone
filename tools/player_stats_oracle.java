package oracle;
import com.google.gson.*;
import java.io.*;import java.lang.reflect.*;import java.util.*;import java.util.function.*;
import io.netty.buffer.*;import it.unimi.dsi.fastutil.objects.*;
import jdk.internal.org.objectweb.asm.*;
import static jdk.internal.org.objectweb.asm.Opcodes.*;

/** Original StatsCounter/ServerStatsCounter arithmetic, dirty drain and actual
 * AwardStats codec. Only the outer connection.send boundary is observed. */
public final class player_stats_oracle {
 static final PrintStream OUT=System.out;static final Gson JSON=new GsonBuilder().serializeNulls().create();
 static Object types,access,current,player;static List<Object> packets=new ArrayList<>();static JsonArray networkObservations=new JsonArray();static int packetId;
 static Class<?> C(String n)throws Exception{return Class.forName(n);}
 static Object call(Object o,String n,Class<?>[]t,Object...args)throws Exception {Class<?> owner=o instanceof Class<?>?(Class<?>)o:o.getClass();Method m=null;try{m=owner.getMethod(n,t);}catch(NoSuchMethodException ignored){for(Class<?> c=owner;c!=null;c=c.getSuperclass())try{m=c.getDeclaredMethod(n,t);break;}catch(NoSuchMethodException e){}}if(m==null)throw new NoSuchMethodException(owner+"."+n);m.setAccessible(true);return m.invoke(o instanceof Class<?>?null:o,args);}
 static Object get(Object o,String owner,String name)throws Exception {Field f=C(owner).getDeclaredField(name);f.setAccessible(true);return f.get(o);}
 static Object alloc(Class<?> c)throws Exception {Class<?> u=C("sun.misc.Unsafe");Field f=u.getDeclaredField("theUnsafe");f.setAccessible(true);return u.getMethod("allocateInstance",Class.class).invoke(f.get(null),c);}
 static int id(Object registry,Object value)throws Exception{return (int)C("jz").getMethod("a",Object.class).invoke(registry,value);}
 static String name(Object registry,Object value)throws Exception{return C("jz").getMethod("b",Object.class).invoke(registry,value).toString();}
 static JsonObject key(Object stat)throws Exception {Object type=call(stat,"a",new Class<?>[]{}),registry=call(type,"b",new Class<?>[]{}),value=call(stat,"b",new Class<?>[]{});JsonObject out=new JsonObject();out.addProperty("type_id",id(types,type));out.addProperty("value_id",id(registry,value));return out;}
 static Object stat(JsonObject key)throws Exception {Object type=C("jz").getMethod("a",int.class).invoke(types,key.get("type_id").getAsInt());Object registry=call(type,"b",new Class<?>[]{}),value=C("jz").getMethod("a",int.class).invoke(registry,key.get("value_id").getAsInt());return call(type,"b",new Class<?>[]{Object.class},value);}
 static JsonArray sorted(Collection<JsonObject> values){List<JsonObject> rows=new ArrayList<>(values);rows.sort(Comparator.comparingInt((JsonObject v)->v.get("type_id").getAsInt()).thenComparingInt(v->v.get("value_id").getAsInt()));JsonArray out=new JsonArray();rows.forEach(out::add);return out;}
 static JsonArray dirty(Object counter)throws Exception {List<JsonObject> rows=new ArrayList<>();for(Object value:(Set<?>)get(counter,"avv","e"))rows.add(key(value));return sorted(rows);}
 @SuppressWarnings("unchecked")static JsonArray values(Object2IntMap<?> map)throws Exception {List<JsonObject> rows=new ArrayList<>();for(Object2IntMap.Entry<?> entry:map.object2IntEntrySet()){JsonObject r=key(entry.getKey());r.addProperty("value",entry.getIntValue());rows.add(r);}return sorted(rows);}
 public static void emitted(Object packet)throws Exception {JsonObject observation=new JsonObject();observation.add("dirty_at_send",dirty(current));networkObservations.add(observation);packets.add(packet);}
 static String packetWire(Object packet)throws Exception {ByteBuf bytes=Unpooled.buffer();try{Object buf=C("wk").getConstructor(ByteBuf.class,C("ka")).newInstance(bytes,access);C("yx").getMethod("encode",Object.class,Object.class).invoke(C("aby").getField("a").get(null),buf,packet);byte[]raw=new byte[bytes.readableBytes()];bytes.getBytes(bytes.readerIndex(),raw);return Base64.getEncoder().encodeToString(raw);}finally{bytes.release();}}
 static JsonObject packet(Object value)throws Exception {JsonObject out=new JsonObject();out.addProperty("packet_id",packetId);out.add("values",values((Object2IntMap<?>)call(value,"b",new Class<?>[]{})));out.addProperty("wire",packetWire(value));return out;}
 static void init()throws Exception {
  call(C("ab"),"a",new Class<?>[]{});call(C("akt"),"a",new Class<?>[]{});types=C("lt").getField("v").get(null);access=call(C("ka"),"a",new Class<?>[]{C("jz")},C("lt").getField("aA").get(null));
  ClassWriter w=new ClassWriter(0);w.visit(V21,ACC_PUBLIC,"OracleStatsConnection",null,"aru",null);MethodVisitor m=w.visitMethod(ACC_PUBLIC,"b","(Lzg;)V",null,null);m.visitCode();m.visitVarInsn(ALOAD,1);m.visitMethodInsn(INVOKESTATIC,"oracle/player_stats_oracle","emitted","(Ljava/lang/Object;)V",false);m.visitInsn(RETURN);m.visitMaxs(1,2);m.visitEnd();w.visitEnd();
  Class<?> connection=new ClassLoader(player_stats_oracle.class.getClassLoader()){Class<?> build(){return defineClass("OracleStatsConnection",w.toByteArray(),0,w.toByteArray().length);}}.build();
  player=alloc(C("aqv"));Field f=C("aqv").getDeclaredField("c");f.setAccessible(true);f.set(player,alloc(connection));
  Object packetType=C("agg").getField("f").get(null),template=C("agh").getField("b").get(null);Class<?> visitor=C("wh$a$a");Object proxy=Proxy.newProxyInstance(visitor.getClassLoader(),new Class<?>[]{visitor},(p,method,args)->{if(method.getName().equals("accept")&&args[0].equals(packetType))packetId=(int)args[1];return null;});C("wh$a").getMethod("a",visitor).invoke(template,proxy);
  OUT.println("SOURCE:"+C("awa").getProtectionDomain().getCodeSource().getLocation());OUT.println("SOURCE:"+C("avv").getProtectionDomain().getCodeSource().getLocation());OUT.println("SOURCE:"+C("aby").getProtectionDomain().getCodeSource().getLocation());
 }
 static JsonObject facts()throws Exception {
  JsonObject out=new JsonObject();out.addProperty("minecraft","1.21");out.addProperty("oracle_inner_sha256","8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01");out.addProperty("award_stats_packet_id",packetId);JsonArray all=new JsonArray();
  for(Object type:(Iterable<?>)types){Object registry=call(type,"b",new Class<?>[]{});JsonObject row=new JsonObject();row.addProperty("type_id",id(types,type));row.addProperty("type_name",name(types,type));JsonArray values=new JsonArray();for(Object value:(Iterable<?>)registry){JsonObject v=new JsonObject();v.addProperty("value_id",id(registry,value));v.addProperty("value_name",name(registry,value));values.add(v);}row.add("values",values);all.add(row);}out.add("types",all);
  Object custom=C("avz").getField("i").get(null),drop=C("avz").getField("F").get(null);out.add("drop_key",key(call(custom,"b",new Class<?>[]{Object.class},drop)));return out;
 }
 static JsonObject snapshot(boolean server)throws Exception {
  JsonObject out=new JsonObject();out.add("values",values((Object2IntMap<?>)get(current,"awa","a")));
  if(server){out.add("dirty",dirty(current));out.add("file",JsonParser.parseString((String)call(current,"b",new Class<?>[]{})));}
  JsonArray sent=new JsonArray();for(Object p:packets)sent.add(packet(p));out.add("packets",sent);out.add("network",networkObservations.deepCopy());return out;
 }
 static JsonObject run(JsonObject q)throws Exception {
  boolean server=q.get("server").getAsBoolean();current=server?C("avv").getConstructor(C("net.minecraft.server.MinecraftServer"),File.class).newInstance(null,new File("/nonexistent-native-stats-oracle-file")):C("awa").getConstructor().newInstance();JsonArray states=new JsonArray();
  for(JsonElement step:q.getAsJsonArray("operations")){JsonObject op=step.getAsJsonObject();packets.clear();networkObservations=new JsonArray();String name=op.get("op").getAsString();JsonElement result=JsonNull.INSTANCE;
   switch(name){
    case "get" -> result=JSON.toJsonTree(call(current,"a",new Class<?>[]{C("avw")},stat(op.getAsJsonObject("key"))));
    case "set" -> call(current,"a",new Class<?>[]{C("cmx"),C("avw"),int.class},null,stat(op.getAsJsonObject("key")),op.get("value").getAsInt());
    case "increment" -> call(current,"b",new Class<?>[]{C("cmx"),C("avw"),int.class},null,stat(op.getAsJsonObject("key")),op.get("amount").getAsInt());
    case "pickup_award" -> {Object type=C("avz").getField("e").get(null),item=C("jz").getMethod("a",int.class).invoke(C("lt").getField("g").get(null),op.get("item_id").getAsInt());call(current,"b",new Class<?>[]{C("cmx"),C("avw"),int.class},null,call(type,"b",new Class<?>[]{Object.class},item),op.get("original_count").getAsInt());}
    case "mark_all_dirty" -> call(current,"c",new Class<?>[]{});
    case "send" -> call(current,"a",new Class<?>[]{C("aqv")},player);
    case "load_current" -> call(current,"a",new Class<?>[]{C("com.mojang.datafixers.DataFixer"),String.class},call(C("azx"),"a",new Class<?>[]{}),op.get("json").getAsString());
    case "ordered_packet" -> {Object2IntLinkedOpenHashMap<Object> map=new Object2IntLinkedOpenHashMap<>();for(JsonElement entry:op.getAsJsonArray("entries")){JsonObject v=entry.getAsJsonObject();map.put(stat(v.getAsJsonObject("key")),v.get("value").getAsInt());}packets.add(C("aby").getConstructor(Object2IntMap.class).newInstance(map));}
    default -> throw new IllegalArgumentException(name);
   }
   JsonObject state=snapshot(server);state.add("result",result);states.add(state);
  }
  JsonObject out=new JsonObject();out.add("states",states);return out;
 }
 public static void main(String[]args)throws Exception {init();if(args.length>0&&args[0].equals("facts")){OUT.println("ORACLE:"+JSON.toJson(facts()));return;}BufferedReader input=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=input.readLine())!=null;)OUT.println("ORACLE:"+JSON.toJson(run(JsonParser.parseString(line).getAsJsonObject())));}
}
