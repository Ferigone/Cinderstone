package oracle;
import com.google.gson.*;import java.io.*;import java.lang.reflect.*;import java.util.*;import java.util.concurrent.atomic.AtomicLong;
import jdk.internal.org.objectweb.asm.*;import static jdk.internal.org.objectweb.asm.Opcodes.*;

/** Original ServerPlayer.awardStat and onItemPickup, original Scoreboard and
 * original criterion triggers. Only world/server/advancement getter boundaries
 * are supplied. The actual empty trigger still constructs its LootContext. */
public final class main_item_gameplay_oracle {
 static final PrintStream OUT=System.out;static final Gson JSON=new GsonBuilder().serializeNulls().create();
 static Class<?> PLAYER,WORLD,SERVER,ADVANCEMENTS,INVENTORY_TRIGGER;static Object world,server,lookup,random,scoreboard,picker,owner,otherAdv;
 static final IdentityHashMap<Object,Object> advancementByPlayer=new IdentityHashMap<>();static final IdentityHashMap<Object,String> labels=new IdentityHashMap<>();static JsonArray events;
 static Class<?> C(String n)throws Exception{return Class.forName(n);}
 static Object call(Object o,String n,Class<?>[]t,Object...a)throws Exception{return player_stats_oracle.call(o,n,t,a);}
 static Object get(Object o,String owner,String field)throws Exception{return player_stats_oracle.get(o,owner,field);}
 static void set(Object o,String owner,String field,Object value)throws Exception {Field f=C(owner).getDeclaredField(field);f.setAccessible(true);f.set(o,value);}
 static JsonObject event(String name,Object receiver){JsonObject e=new JsonObject();e.addProperty("callback",name);if(labels.containsKey(receiver))e.addProperty("player",labels.get(receiver));events.add(e);return e;}
 public static Object environment(Object receiver,String kind)throws Exception {
  JsonObject e=event(kind,receiver);
  return switch(kind){
   case "scoreboard" -> {e.add("counter_values",player_stats_oracle.values((it.unimi.dsi.fastutil.objects.Object2IntMap<?>)get(get(receiver,"aqv","cH"),"awa","a")));yield scoreboard;}
   case "level_context" -> world;case "advancements" -> advancementByPlayer.get(receiver);
   case "world_server" -> server;case "world_random" -> random;case "server_lookup" -> lookup;
   default -> throw new IllegalArgumentException(kind);
  };
 }
 public static boolean awarded(Object adv,Object holder,String criterion){JsonObject e=event("criterion_award",adv);e.addProperty("criterion",criterion);return true;}
 public static void inventoryPredicate(Object predicate)throws Exception {JsonObject e=event("inventory_counts",null);for(int i=3;i<=5;i++){Field f=predicate.getClass().getDeclaredField("arg$"+i);f.setAccessible(true);e.addProperty(new String[]{"full","empty","occupied"}[i-3],f.getInt(predicate));}}
 static Class<?> define(String name,ClassWriter w){w.visitEnd();byte[]raw=w.toByteArray();return new ClassLoader(main_item_gameplay_oracle.class.getClassLoader()){Class<?> build(){return defineClass(name,raw,0,raw.length);}}.build();}
 static void environmentMethod(ClassWriter w,String name,String type,String kind){MethodVisitor m=w.visitMethod(ACC_PUBLIC,name,"()L"+type+";",null,null);m.visitCode();m.visitVarInsn(ALOAD,0);m.visitLdcInsn(kind);m.visitMethodInsn(INVOKESTATIC,"oracle/main_item_gameplay_oracle","environment","(Ljava/lang/Object;Ljava/lang/String;)Ljava/lang/Object;",false);m.visitTypeInsn(CHECKCAST,type);m.visitInsn(ARETURN);m.visitMaxs(2,1);m.visitEnd();}
 static ClassWriter clazz(String name,String parent){ClassWriter w=new ClassWriter(0);w.visit(V21,ACC_PUBLIC,name,null,parent,null);return w;}
 static void init()throws Exception {
  player_stats_oracle.init();ClassWriter w=clazz("OracleMainGameplayPlayer","aqv");environmentMethod(w,"gn","eyg","scoreboard");environmentMethod(w,"A","aqu","level_context");environmentMethod(w,"R","akz","advancements");PLAYER=define("OracleMainGameplayPlayer",w);
  w=clazz("OracleMainGameplayWorld","aqu");environmentMethod(w,"o","net/minecraft/server/MinecraftServer","world_server");environmentMethod(w,"E_","ayw","world_random");WORLD=define("OracleMainGameplayWorld",w);
  w=clazz("OracleMainGameplayServer","net/minecraft/server/MinecraftServer");environmentMethod(w,"be","alb$b","server_lookup");SERVER=define("OracleMainGameplayServer",w);
  w=clazz("OracleMainGameplayAdvancements","akz");MethodVisitor m=w.visitMethod(ACC_PUBLIC,"a","(Lag;Ljava/lang/String;)Z",null,null);m.visitCode();m.visitVarInsn(ALOAD,0);m.visitVarInsn(ALOAD,1);m.visitVarInsn(ALOAD,2);m.visitMethodInsn(INVOKESTATIC,"oracle/main_item_gameplay_oracle","awarded","(Ljava/lang/Object;Ljava/lang/Object;Ljava/lang/String;)Z",false);m.visitInsn(IRETURN);m.visitMaxs(3,3);m.visitEnd();ADVANCEMENTS=define("OracleMainGameplayAdvancements",w);
  w=clazz("OracleMainGameplayInventoryTrigger","cg");m=w.visitMethod(ACC_PUBLIC,"<init>","()V",null,null);m.visitCode();m.visitVarInsn(ALOAD,0);m.visitMethodInsn(INVOKESPECIAL,"cg","<init>","()V",false);m.visitInsn(RETURN);m.visitMaxs(1,1);m.visitEnd();m=w.visitMethod(ACC_PROTECTED,"a","(Laqv;Ljava/util/function/Predicate;)V",null,null);m.visitCode();m.visitVarInsn(ALOAD,2);m.visitMethodInsn(INVOKESTATIC,"oracle/main_item_gameplay_oracle","inventoryPredicate","(Ljava/lang/Object;)V",false);m.visitVarInsn(ALOAD,0);m.visitVarInsn(ALOAD,1);m.visitVarInsn(ALOAD,2);m.visitMethodInsn(INVOKESPECIAL,"cg","a","(Laqv;Ljava/util/function/Predicate;)V",false);m.visitInsn(RETURN);m.visitMaxs(3,3);m.visitEnd();INVENTORY_TRIGGER=define("OracleMainGameplayInventoryTrigger",w);
  lookup=C("alb$b").getConstructor(C("ka$b")).newInstance(player_stats_oracle.access);
  OUT.println("SOURCE:"+C("aqv").getProtectionDomain().getCodeSource().getLocation());OUT.println("SOURCE:"+C("dl").getProtectionDomain().getCodeSource().getLocation());
 }
 static Object player(String label)throws Exception {Object p=player_stats_oracle.alloc(PLAYER);Object adv=player_stats_oracle.alloc(ADVANCEMENTS);advancementByPlayer.put(p,adv);labels.put(p,label);labels.put(adv,label);set(p,"bsr","s",C("exc").getConstructor(double.class,double.class,double.class).newInstance(0.,64.,0.));return p;}
 static Object stack(int id,int count)throws Exception {Object item=C("jz").getMethod("a",int.class).invoke(C("lt").getField("g").get(null),id);return C("cuq").getConstructor(C("dcv"),int.class).newInstance(item,count);}
 static Object trigger(String name)throws Exception{return C("an").getField(name).get(null);}
 @SuppressWarnings("unchecked")static Map<Object,Set<Object>> listeners(Object trigger)throws Exception{return (Map<Object,Set<Object>>)get(trigger,"dv","a");}
 static JsonObject counts()throws Exception {JsonObject out=new JsonObject();for(String k:new String[]{"R","S","f"}){JsonObject v=new JsonObject();v.addProperty("players",listeners(trigger(k)).size());v.addProperty("listeners",listeners(trigger(k)).values().stream().mapToInt(Set::size).sum());out.add(k,v);}return out;}
 static long state48()throws Exception{return ((AtomicLong)get(random,"dyz","h")).get();}
 static JsonObject run(JsonObject q)throws Exception {
  advancementByPlayer.clear();labels.clear();for(String k:new String[]{"R","S","f"})listeners(trigger(k)).clear();events=new JsonArray();world=player_stats_oracle.alloc(WORLD);server=player_stats_oracle.alloc(SERVER);random=C("dyz").getConstructor(long.class).newInstance(123456L);set(world,"dcw","z",random);scoreboard=C("eyg").getConstructor().newInstance();picker=player("picker");Object thrower=player("thrower");otherAdv=player_stats_oracle.alloc(ADVANCEMENTS);labels.put(otherAdv,"other");
  player_stats_oracle.current=C("avv").getConstructor(C("net.minecraft.server.MinecraftServer"),File.class).newInstance(null,new File("/nonexistent-native-stats-oracle-file"));player_stats_oracle.packets.clear();player_stats_oracle.networkObservations=new JsonArray();set(picker,"aqv","cH",player_stats_oracle.current);
  JsonObject key=new JsonObject();key.addProperty("type_id",4);key.addProperty("value_id",q.get("item_id").getAsInt());Object stat=player_stats_oracle.stat(key);call(player_stats_oracle.current,"a",new Class<?>[]{C("cmx"),C("avw"),int.class},picker,stat,q.get("old_value").getAsInt());
  Object item=C("cjh").getConstructor(C("dcw"),double.class,double.class,double.class,C("cuq"),double.class,double.class,double.class).newInstance(world,0.,64.,0.,stack(q.get("item_id").getAsInt(),q.get("count").getAsInt()),0.,0.,0.);
  String ownerKind=q.get("owner").getAsString();owner=switch(ownerKind){case "none"->null;case "player"->thrower;case "entity"->player_stats_oracle.alloc(C("cjh"));default->throw new IllegalArgumentException();};set(item,"cjh","m",owner);
  String mode=q.get("registry").getAsString();if(mode.equals("empty_sets")){for(String k:new String[]{"R","S","f"})for(Object adv:advancementByPlayer.values())listeners(trigger(k)).put(adv,new HashSet<>());}else if(mode.equals("other_player")){Object instance=C("dl$a").getConstructor(Optional.class,Optional.class,Optional.class).newInstance(Optional.empty(),Optional.empty(),Optional.empty());Object listener=C("aq$a").getConstructor(C("ar"),C("ag"),String.class).newInstance(instance,null,"unrelated");for(String k:new String[]{"R","S"})call(trigger(k),"a",new Class<?>[]{C("akz"),C("aq$a")},otherAdv,listener);}else if(!mode.equals("absent"))throw new IllegalArgumentException(mode);
  events=new JsonArray();long before=state48();if(q.has("inventory")){Object inventory=C("cmw").getConstructor(C("cmx")).newInstance(picker);set(picker,"cmx","h",inventory);int i=0;for(JsonElement slot:q.getAsJsonArray("inventory")){JsonObject s=slot.getAsJsonObject();call(inventory,"a",new Class<?>[]{int.class,C("cuq")},i++,stack(s.get("item_id").getAsInt(),s.get("count").getAsInt()));}C("cg").getMethod("a",C("aqv"),C("cmw"),C("cuq")).invoke(INVENTORY_TRIGGER.getConstructor().newInstance(),picker,inventory,stack(1,1));}else{C("aqv").getMethod("a",C("avw"),int.class).invoke(picker,stat,q.get("count").getAsInt());C("aqv").getMethod("a",C("cjh")).invoke(picker,item);}
  // Execute the real nonplayer entityRemoved branch against the owned empty
  // scoreboard as well; no score or team data is manufactured by the oracle.
  C("eyg").getMethod("a",C("bsr")).invoke(scoreboard,item);
  JsonObject out=new JsonObject();out.add("events",events);out.add("counter",player_stats_oracle.snapshot(true));out.add("criteria",counts());out.addProperty("world_rng_before",before);out.addProperty("world_rng_after",state48());out.addProperty("scoreboard_objectives",((Map<?,?>)get(scoreboard,"eyg","c")).size());out.addProperty("scoreboard_player_scores",((Map<?,?>)get(scoreboard,"eyg","e")).size());out.addProperty("scoreboard_teams",((Map<?,?>)get(scoreboard,"eyg","g")).size());out.addProperty("scoreboard_team_members",((Map<?,?>)get(scoreboard,"eyg","h")).size());out.addProperty("original_entity_removed_executed",true);return out;
 }
 public static void main(String[]args)throws Exception {init();BufferedReader input=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=input.readLine())!=null;)OUT.println("ORACLE:"+JSON.toJson(run(JsonParser.parseString(line).getAsJsonObject())));}
}
