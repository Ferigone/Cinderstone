package oracle;
import com.google.gson.*;
import java.io.*;
import java.lang.reflect.*;
import java.util.*;
import java.util.concurrent.*;
import jdk.internal.org.objectweb.asm.*;
import static jdk.internal.org.objectweb.asm.Opcodes.*;

/** Original DistanceManager source methods, PlayerTicketTracker, loading graph,
 * actual throttler/processor mailbox. Only scheduling storage and Executors are
 * host adapters. Chunk generation and holder futures are not synthesized. */
public final class chunk_loading_tickets_oracle {
 static final Gson JSON=new GsonBuilder().serializeNulls().create();static final PrintStream OUT=System.out;
 static Object unsafe,manager,playerGraph,loadingGraph,ticking;static Method allocate;static Class<?> MANAGER,MAP;static Object chunkMap;static final TreeMap<Long,CompletableFuture<Object>> FUTURES=new TreeMap<>();
 static final TreeMap<Long,Object> holders=new TreeMap<>();static final HashMap<Integer,Object> players=new HashMap<>();
 static JsonArray events=new JsonArray();static ControlledExecutor sorter,main;static int taskSequence;
 static Class<?> C(String n)throws Exception{return Class.forName(n);}static Class<?>[] T(Class<?>...v){return v;}
 static Object call(Object o,String n,Class<?>[]t,Object...a)throws Exception{return survival_oracle.call(o,n,t,a);}
 static void set(Object o,String c,String n,Object v)throws Exception{survival_oracle.set(o,c,n,v);}
 static Object get(Object o,String c,String n)throws Exception{Field f=C(c).getDeclaredField(n);f.setAccessible(true);return f.get(o);}
 static Object alloc(Class<?>c)throws Exception{return allocate.invoke(unsafe,c);}
 static Object construct(String c,Class<?>[]t,Object...a)throws Exception{Constructor<?>ct=C(c).getDeclaredConstructor(t);ct.setAccessible(true);return ct.newInstance(a);}
 static Object chunk(JsonArray a)throws Exception{return construct("dcd",T(int.class,int.class),a.get(0).getAsInt(),a.get(1).getAsInt());}
 static long key(JsonArray a){return Integer.toUnsignedLong(a.get(0).getAsInt())|((long)a.get(1).getAsInt()<<32);}
 static JsonArray point(long key){JsonArray a=new JsonArray();a.add((int)key);a.add((int)(key>>32));return a;}
 static void event(String name,Object...args){JsonObject e=new JsonObject();e.addProperty("event",name);for(int i=0;i<args.length;i+=2){Object v=args[i+1];e.add((String)args[i],JSON.toJsonTree(v));}events.add(e);}
 static final class ControlledExecutor implements Executor {
  final String name;boolean inline;final LinkedHashMap<Integer,Runnable> tasks=new LinkedHashMap<>();
  ControlledExecutor(String name,boolean inline){this.name=name;this.inline=inline;}
  public void execute(Runnable task){if(inline){task.run();return;}int id=++taskSequence;tasks.put(id,task);event("enqueue","executor",name,"task",id);}
  int run(int budget){int used=0;while(budget-->0&&!tasks.isEmpty()){int id=tasks.keySet().iterator().next();Runnable task=tasks.remove(id);event("run","executor",name,"task",id);task.run();used++;}return used;}
 }
 public static Object getHolder(long key){return holders.get(key);}
 public static Object schedule(long key,int level,Object previous,int old)throws Exception {
  event("schedule","chunk",point(key),"old",old,"level",level);
  if(level<=44){Object h=previous;if(h==null){h=alloc(C("apz"));CompletableFuture<Object> future=new CompletableFuture<>();FUTURES.put(key,future);set(h,"apz","i",future);}call(h,"a",T(int.class),level);holders.put(key,h);return h;}
  holders.remove(key);return previous;
 }
 static void init()throws Exception {
  call(C("ab"),"a",T());call(C("akt"),"a",T());Class<?>u=C("sun.misc.Unsafe");Field uf=u.getDeclaredField("theUnsafe");uf.setAccessible(true);unsafe=uf.get(null);allocate=u.getMethod("allocateInstance",Class.class);
  survival_oracle loader=new survival_oracle();String name="oracle/LoadingDistanceManager";ClassWriter c=survival_oracle.begin(name,"aqk");
  MethodVisitor m=survival_oracle.method(c,"<init>","(Ljava/util/concurrent/Executor;Ljava/util/concurrent/Executor;)V");m.visitVarInsn(ALOAD,0);m.visitVarInsn(ALOAD,1);m.visitVarInsn(ALOAD,2);m.visitMethodInsn(INVOKESPECIAL,"aqk","<init>","(Ljava/util/concurrent/Executor;Ljava/util/concurrent/Executor;)V",false);m.visitInsn(RETURN);survival_oracle.end(m);
  survival_oracle.getterConst(c,"a","(J)Z",ICONST_0,IRETURN);
  m=survival_oracle.method(c,"b","(J)Lapz;");m.visitVarInsn(LLOAD,1);m.visitMethodInsn(INVOKESTATIC,"oracle/chunk_loading_tickets_oracle","getHolder","(J)Ljava/lang/Object;",false);m.visitTypeInsn(CHECKCAST,"apz");m.visitInsn(ARETURN);survival_oracle.end(m);
  m=survival_oracle.method(c,"a","(JILapz;I)Lapz;");m.visitVarInsn(LLOAD,1);m.visitVarInsn(ILOAD,3);m.visitVarInsn(ALOAD,4);m.visitVarInsn(ILOAD,5);m.visitMethodInsn(INVOKESTATIC,"oracle/chunk_loading_tickets_oracle","schedule","(JILjava/lang/Object;I)Ljava/lang/Object;",false);m.visitTypeInsn(CHECKCAST,"apz");m.visitInsn(ARETURN);survival_oracle.end(m);
  MANAGER=survival_oracle.define(loader,name,c);
  name="oracle/LoadingChunkMap";c=survival_oracle.begin(name,"aqb");m=survival_oracle.method(c,"a","(J)Lapz;");m.visitVarInsn(LLOAD,1);m.visitMethodInsn(INVOKESTATIC,"oracle/chunk_loading_tickets_oracle","getHolder","(J)Ljava/lang/Object;",false);m.visitTypeInsn(CHECKCAST,"apz");m.visitInsn(ARETURN);survival_oracle.end(m);MAP=survival_oracle.define(loader,name,c);
  OUT.println("HEADER:"+JSON.toJson(Map.of("source",C("aqk").getProtectionDomain().getCodeSource().getLocation().toString(),"max_loaded_level",C("aqa").getField("b").getInt(null),"player_ticket_level",get(null,"aqk","b"))));
 }
 static Object type(String name)throws Exception{return C("aqz").getField(switch(name){case"player"->"c";case"start"->"a";case"forced"->"d";default->throw new IllegalArgumentException(name);}).get(null);}
 static Object ticketKey(String name,Object chunk)throws Exception{return name.equals("start")?C("azs").getField("a").get(null):chunk;}
 static Object player(int id)throws Exception {if(!players.containsKey(id)){Object p=alloc(C("aqv"));set(p,"bsr","o",id);players.put(id,p);}return players.get(id);}
 static Object section(JsonArray a)throws Exception{return call(C("kf"),"a",T(int.class,int.class,int.class),a.get(0).getAsInt(),0,a.get(1).getAsInt());}
 static JsonArray mapRows(Map<?,?>map){TreeMap<Long,Integer>sorted=new TreeMap<>();for(var e:map.entrySet())sorted.put(((Number)e.getKey()).longValue(),((Number)e.getValue()).intValue());JsonArray out=new JsonArray();for(var e:sorted.entrySet()){JsonArray row=point(e.getKey());row.add(e.getValue());out.add(row);}return out;}
 static JsonArray sources(Object source)throws Exception{List<JsonArray>rows=new ArrayList<>();for(var entry:((Map<?,?>)source).entrySet())for(Object ticket:(Iterable<?>)entry.getValue()){JsonArray row=point(((Number)entry.getKey()).longValue());row.add(call(ticket,"a",T()).toString());row.add((int)call(ticket,"b",T()));row.add((long)get(ticket,"aqy","d"));rows.add(row);}rows.sort(Comparator.comparing(JSON::toJson));JsonArray out=new JsonArray();rows.forEach(out::add);return out;}
 static JsonObject graph(Object graph,Map<?,?>levels)throws Exception{JsonObject out=new JsonObject();out.add("levels",mapRows(levels));out.add("computed",mapRows((Map<?,?>)get(graph,"eoq","c")));JsonArray queues=new JsonArray();for(Object queue:(Object[])get(get(graph,"eoq","b"),"eou","b")){JsonArray entries=new JsonArray();for(Object k:(Iterable<?>)queue)entries.add(point(((Number)k).longValue()));queues.add(entries);}out.add("queues",queues);out.addProperty("queue_size",(int)call(graph,"c",T()));out.addProperty("has_work",(boolean)call(graph,"b",T()));return out;}
 static JsonArray keys(Object values){JsonArray out=new JsonArray();for(Object k:(Iterable<?>)values)out.add(point(((Number)k).longValue()));return out;}
 static JsonObject snapshot(JsonArray points)throws Exception {
  JsonObject out=new JsonObject();out.addProperty("view_distance",(int)get(playerGraph,"aqk$c","g"));out.addProperty("ticket_tick",(long)get(manager,"aqk","p"));out.add("primary_tickets",sources(get(manager,"aqk","e")));out.add("ticking_tickets",sources(get(ticking,"ara","d")));
  out.add("player",graph(playerGraph,(Map<?,?>)get(playerGraph,"aqk$b","a")));TreeMap<Long,Integer>levels=new TreeMap<>();for(var e:holders.entrySet())levels.put(e.getKey(),(int)call(e.getValue(),"i",T()));out.add("loading",graph(loadingGraph,levels));out.add("ticking",graph(ticking,(Map<?,?>)get(ticking,"ara","b")));
  out.add("queue_levels",mapRows((Map<?,?>)get(playerGraph,"aqk$c","h")));out.add("to_update",keys(get(playerGraph,"aqk$c","i")));out.add("tickets_to_release",keys(get(manager,"aqk","n")));out.add("sorter_tasks",JSON.toJsonTree(sorter.tasks.keySet()));out.add("main_tasks",JSON.toJsonTree(main.tasks.keySet()));
  Object throttle=get(manager,"aqk","k");Map<?,?> taskQueues=(Map<?,?>)get(throttle,"aqe","b");Object queue=taskQueues.values().iterator().next();out.add("acquired",keys(get(queue,"aqd","e")));out.addProperty("sleeping",((Set<?>)get(throttle,"aqe","c")).size()>0);out.addProperty("mailbox_size",(int)call(get(throttle,"aqe","d"),"b",T()));JsonArray pending=new JsonArray();for(Object levelQueue:(List<?>)get(queue,"aqd","b")){JsonArray level=new JsonArray();for(var e:((Map<?,?>)levelQueue).entrySet()){JsonObject row=new JsonObject();row.add("chunk",point(((Number)e.getKey()).longValue()));JsonArray messages=new JsonArray();for(Object message:(List<?>)e.getValue())messages.add(((Optional<?>)message).isPresent()?"task":"acquire");row.add("messages",messages);level.add(row);}pending.add(level);}out.add("pending",pending);
  JsonArray queries=new JsonArray();for(JsonElement p:points){long key=key(p.getAsJsonArray());JsonObject row=new JsonObject();row.addProperty("player",(int)call(playerGraph,"c",T(long.class),key));row.addProperty("loading",(int)call(loadingGraph,"c",T(long.class),key));row.addProperty("ticking",(int)call(ticking,"c",T(long.class),key));queries.add(row);}out.add("query",queries);return out;
 }
 static JsonArray scene(JsonObject q)throws Exception {
  holders.clear();players.clear();FUTURES.clear();chunkMap=alloc(MAP);taskSequence=0;events=new JsonArray();sorter=new ControlledExecutor("sorter",true);main=new ControlledExecutor("main",false);manager=MANAGER.getConstructor(Executor.class,Executor.class).newInstance(sorter,main);sorter.inline=false;
  playerGraph=get(manager,"aqk","i");loadingGraph=get(manager,"aqk","f");ticking=get(manager,"aqk","h");if(q.has("simulation_distance"))call(manager,"b",T(int.class),q.get("simulation_distance").getAsInt());JsonArray out=new JsonArray();
  for(JsonElement value:q.getAsJsonArray("ops")){JsonObject op=value.getAsJsonObject();events=new JsonArray();JsonElement result=JsonNull.INSTANCE;String name=op.get("op").getAsString();switch(name){
   case"player_add":case"player_remove":call(manager,name.equals("player_add")?"a":"b",T(C("kf"),C("aqv")),section(op.getAsJsonArray("chunk")),player(op.get("player").getAsInt()));break;
   case"view":call(manager,"a",T(int.class),op.get("distance").getAsInt());break;
   case"simulation":call(manager,"b",T(int.class),op.get("distance").getAsInt());break;
   case"raw_add":case"raw_remove":case"region_add":case"region_remove":{String kind=op.get("kind").getAsString();Object chunk=chunk(op.getAsJsonArray("chunk"));call(manager,switch(name){case"raw_add"->"a";case"raw_remove"->"b";case"region_add"->"c";default->"d";},T(C("aqz"),C("dcd"),int.class,Object.class),type(kind),chunk,op.get(name.startsWith("raw")?"level":"distance").getAsInt(),ticketKey(kind,chunk));break;}
   case"forced":call(manager,"a",T(C("dcd"),boolean.class),chunk(op.getAsJsonArray("chunk")),op.get("enabled").getAsBoolean());break;
   case"purge":call(manager,"a",T());break;
   case"run_player":call(playerGraph,"a",T());break;
   case"run_ticking":call(ticking,"a",T());break;
   case"run_loading":result=new JsonPrimitive((int)call(loadingGraph,"a",T(int.class),op.has("budget")?op.get("budget").getAsInt():Integer.MAX_VALUE));break;
   case"run_sorter":result=new JsonPrimitive(sorter.run(op.has("budget")?op.get("budget").getAsInt():Integer.MAX_VALUE));break;
   case"run_main":result=new JsonPrimitive(main.run(op.has("budget")?op.get("budget").getAsInt():Integer.MAX_VALUE));break;
   case"register_releases":{((Set<?>)get(manager,"aqk","j")).clear();result=new JsonPrimitive((boolean)call(manager,"a",T(C("aqb")),chunkMap));break;}
   case"future_complete":{CompletableFuture<Object>future=Objects.requireNonNull(FUTURES.get(key(op.getAsJsonArray("chunk"))));String state=op.get("state").getAsString();boolean accepted=switch(state){case"failure","unloaded"->future.complete(call(C("aqc"),"a",T(String.class),"controlled host result"));case"success"->future.complete(call(C("aqc"),"a",T(Object.class),alloc(C("dvi"))));case"exceptional"->future.completeExceptionally(new IllegalStateException("controlled host future failure"));case"cancelled"->future.cancel(false);default->throw new IllegalArgumentException(state);};result=new JsonPrimitive(accepted);break;}
   case"release":{long key=key(op.getAsJsonArray("chunk"));Class<?> ticket=C("aqc");call(manager,"a",T(long.class,ticket),key,call(ticket,"a",T(String.class),"controlled host future completed normally"));break;}
   default:throw new IllegalArgumentException(op.toString());
  }JsonObject step=new JsonObject();step.add("result",result);step.add("events",events);step.add("state",snapshot(q.getAsJsonArray("points")));out.add(step);}return out;
 }
 public static void main(String[]args)throws Exception{init();BufferedReader in=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=in.readLine())!=null;)try{OUT.println("ORACLE:"+JSON.toJson(scene(JsonParser.parseString(line).getAsJsonObject())));}catch(Throwable e){OUT.println("QUERY:"+line);e.printStackTrace(OUT);throw e;}}
}
