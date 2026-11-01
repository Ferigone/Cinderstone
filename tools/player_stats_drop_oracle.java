package oracle;
import com.google.gson.*;import java.io.*;import java.lang.reflect.*;import java.util.*;
import jdk.internal.org.objectweb.asm.*;import static jdk.internal.org.objectweb.asm.Opcodes.*;

/** Executes unchanged aqv.drop and cmx.drop. Only environment Level.addFreshEntity
 * and the outer awardStat boundary are controlled; awards call real counter. */
public final class player_stats_drop_oracle {
 static final PrintStream OUT=System.out;static final Gson JSON=new GsonBuilder().serializeNulls().create();
 public static Object world;public static float yaw,pitch;public static double eyeY;
 static Class<?> PLAYER,WORLD;static JsonObject query;static JsonArray events;static Object argument;
 static Class<?> C(String n)throws Exception{return Class.forName(n);}
 static Object call(Object o,String n,Class<?>[]t,Object...v)throws Exception{return player_stats_oracle.call(o,n,t,v);}
 static void set(Object o,String owner,String name,Object value)throws Exception {Field f=C(owner).getDeclaredField(name);f.setAccessible(true);f.set(o,value);}
 static Object stack(int id,int count)throws Exception {Object value=C("jz").getMethod("a",int.class).invoke(C("lt").getField("g").get(null),id);return C("cuq").getConstructor(C("dcv"),int.class).newInstance(value,count);}
 public static boolean added(Object entity)throws Exception {
  JsonObject event=new JsonObject();event.addProperty("callback","world_add");event.addProperty("returned",query.get("world_accepts").getAsBoolean());events.add(event);
  switch(query.get("after_add").getAsString()) {
   case "none" -> {}
   case "empty" -> call(call(entity,"p",new Class<?>[]{}),"e",new Class<?>[]{int.class},0);
   case "replace" -> call(entity,"a",new Class<?>[]{C("cuq")},stack(query.get("replacement_item_id").getAsInt(),3));
   case "replace_empty" -> call(entity,"a",new Class<?>[]{C("cuq")},C("cuq").getField("l").get(null));
   default -> throw new IllegalArgumentException();
  }
  return query.get("world_accepts").getAsBoolean();
 }
 public static void award(Object stat,int amount)throws Exception {
  JsonObject event=player_stats_oracle.key(stat);event.addProperty("callback","award_stat");event.addProperty("amount",amount);events.add(event);
  call(player_stats_oracle.current,"b",new Class<?>[]{C("cmx"),C("avw"),int.class},null,stat,amount);
 }
 static Class<?> define(String name,ClassWriter w){w.visitEnd();byte[]raw=w.toByteArray();return new ClassLoader(player_stats_drop_oracle.class.getClassLoader()){Class<?> build(){return defineClass(name,raw,0,raw.length);}}.build();}
 static void value(ClassWriter w,String method,String desc,String field,String type,int opcode){MethodVisitor m=w.visitMethod(ACC_PUBLIC,method,desc,null,null);m.visitCode();m.visitFieldInsn(GETSTATIC,"oracle/player_stats_drop_oracle",field,type);if(opcode==ARETURN)m.visitTypeInsn(CHECKCAST,"dcw");m.visitInsn(opcode);m.visitMaxs(2,1);m.visitEnd();}
 static void init()throws Exception {
  player_stats_oracle.init();ClassWriter w=new ClassWriter(0);w.visit(V21,ACC_PUBLIC,"OracleStatsDropWorld",null,"dcw",null);MethodVisitor m=w.visitMethod(ACC_PUBLIC,"b","(Lbsr;)Z",null,null);m.visitCode();m.visitVarInsn(ALOAD,1);m.visitMethodInsn(INVOKESTATIC,"oracle/player_stats_drop_oracle","added","(Ljava/lang/Object;)Z",false);m.visitInsn(IRETURN);m.visitMaxs(1,2);m.visitEnd();WORLD=define("OracleStatsDropWorld",w);
  w=new ClassWriter(0);w.visit(V21,ACC_PUBLIC,"OracleStatsDropPlayer",null,"aqv",null);value(w,"dO","()Ldcw;","world","Ljava/lang/Object;",ARETURN);value(w,"dE","()F","yaw","F",FRETURN);value(w,"dG","()F","pitch","F",FRETURN);value(w,"dx","()D","eyeY","D",DRETURN);
  m=w.visitMethod(ACC_PUBLIC,"a","(Lavw;I)V",null,null);m.visitCode();m.visitVarInsn(ALOAD,1);m.visitVarInsn(ILOAD,2);m.visitMethodInsn(INVOKESTATIC,"oracle/player_stats_drop_oracle","award","(Ljava/lang/Object;I)V",false);m.visitInsn(RETURN);m.visitMaxs(2,3);m.visitEnd();PLAYER=define("OracleStatsDropPlayer",w);
  OUT.println("SOURCE_DROP:"+C("aqv").getProtectionDomain().getCodeSource().getLocation());
 }
 static JsonObject run(JsonObject q)throws Exception {
  query=q;events=new JsonArray();player_stats_oracle.packets.clear();player_stats_oracle.networkObservations=new JsonArray();player_stats_oracle.current=C("avv").getConstructor(C("net.minecraft.server.MinecraftServer"),File.class).newInstance(null,new File("/nonexistent-native-stats-oracle-file"));
  world=player_stats_oracle.alloc(WORLD);set(world,"dcw","z",C("dyz").getConstructor(long.class).newInstance(12345L));
  Object player=player_stats_oracle.alloc(PLAYER);set(player,"bsr","s",C("exc").getConstructor(double.class,double.class,double.class).newInstance(0.,64.,0.));set(player,"bsr","ax",UUID.fromString("00000000-0000-4000-8000-000000000001"));set(player,"bsr","ah",C("dyz").getConstructor(long.class).newInstance(87654L));yaw=0;pitch=0;eyeY=65.62;
  argument=stack(q.get("item_id").getAsInt(),q.get("count").getAsInt());Object dropped=C("aqv").getMethod("a",C("cuq"),boolean.class,boolean.class).invoke(player,argument,q.get("around").getAsBoolean(),q.get("trace_item").getAsBoolean());
  JsonObject event=new JsonObject();event.addProperty("constructed",dropped!=null);event.addProperty("trace_item",q.get("trace_item").getAsBoolean());event.addProperty("original_count",(int)call(argument,"H",new Class<?>[]{}));event.add("returned_item_id",JsonNull.INSTANCE);
  if(dropped!=null){Object returned=call(dropped,"p",new Class<?>[]{});if(!(boolean)call(returned,"e",new Class<?>[]{}))event.addProperty("returned_item_id",player_stats_oracle.id(C("lt").getField("g").get(null),call(returned,"g",new Class<?>[]{})));}
  JsonObject out=new JsonObject();out.add("event",event);out.add("callbacks",events);out.add("counter",player_stats_oracle.snapshot(true));return out;
 }
 public static void main(String[]args)throws Exception {init();BufferedReader input=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=input.readLine())!=null;)OUT.println("ORACLE:"+JSON.toJson(run(JsonParser.parseString(line).getAsJsonObject())));}
}
