package oracle;

import com.google.gson.*;
import java.io.*;
import java.lang.invoke.MethodHandles;
import java.lang.reflect.*;
import java.util.*;
import java.util.concurrent.*;
import java.util.function.*;
import jdk.internal.org.objectweb.asm.*;
import static jdk.internal.org.objectweb.asm.Opcodes.*;

/** Unchanged ChunkHolder/updateFutures/CompletableFuture; only ChunkMap provider callbacks and executor are controlled. */
public final class chunk_lifecycle_oracle {
 static final Gson JSON=new GsonBuilder().serializeNulls().create();static final PrintStream OUT=System.out;
 static Object unsafe;static Method allocate;static Class<?> MAP;static Object holder,map,chunk,unloaded;
 static final TreeMap<Integer,CompletableFuture<Object>> FUTURES=new TreeMap<>();static final IdentityHashMap<Object,Integer> FUTURE_IDS=new IdentityHashMap<>();
 static final TreeMap<Integer,Runnable> TASKS=new TreeMap<>();static final LinkedHashSet<Integer> PENDING=new LinkedHashSet<>();
 static final List<IntSupplier> LEVEL_GETTERS=new ArrayList<>();static final List<IntConsumer> LEVEL_SETTERS=new ArrayList<>();static final List<Integer> LEVEL_VALUES=new ArrayList<>();
 static final Deque<JsonObject> PREPARES=new ArrayDeque<>();static JsonArray events=new JsonArray();static int taskSequence,outputStatus=-1;
 static String executorMode="queued";static boolean applyQueueInline=true;
 static Class<?> C(String n)throws Exception{return Class.forName(n);}static Class<?>[] T(Class<?>...t){return t;}
 static Object call(Object o,String n,Class<?>[]t,Object...v)throws Exception{Class<?>c=o instanceof Class<?>?(Class<?>)o:o.getClass();Method m;try{m=c.getMethod(n,t);}catch(NoSuchMethodException e){m=c.getDeclaredMethod(n,t);}m.setAccessible(true);return m.invoke(o instanceof Class<?>?null:o,v);}
 static Object field(Object o,String n)throws Exception{Field f=C("apz").getDeclaredField(n);f.setAccessible(true);return f.get(o);}
 static Object alloc(Class<?>c)throws Exception{return allocate.invoke(unsafe,c);}
 static void event(String kind,Object...pairs){JsonObject e=new JsonObject();e.addProperty("kind",kind);for(int i=0;i<pairs.length;i+=2){Object v=pairs[i+1];if(v instanceof Number)e.addProperty((String)pairs[i],(Number)v);else e.addProperty((String)pairs[i],String.valueOf(v));}events.add(e);}
 public static CompletableFuture<Object> prepare(String stage){JsonObject p=PREPARES.removeFirst();if(!p.get("stage").getAsString().equals(stage))throw new AssertionError("prepare stage order");int id=p.get("future").getAsInt();event("prepare","stage",stage,"future",id);return Objects.requireNonNull(FUTURES.get(id));}
 public static void status(Object s)throws Exception{outputStatus=((Enum<?>)s).ordinal();Object visibility=call(C("dxt"),"a",T(C("aql")),s);event("status","status",outputStatus,"visibility",((Enum<?>)visibility).ordinal());}
 static void adapters()throws Exception{
  ClassWriter c=new ClassWriter(ClassWriter.COMPUTE_FRAMES|ClassWriter.COMPUTE_MAXS);c.visit(V21,ACC_PUBLIC,"ChunkLifecycleOracleMap",null,"aqb",null);
  for(String[]s:new String[][]{{"c","full"},{"b","block"},{"a","entity"}}){MethodVisitor m=c.visitMethod(ACC_PUBLIC,s[0],"(Lapz;)Ljava/util/concurrent/CompletableFuture;",null,null);m.visitCode();m.visitLdcInsn(s[1]);m.visitMethodInsn(INVOKESTATIC,"oracle/chunk_lifecycle_oracle","prepare","(Ljava/lang/String;)Ljava/util/concurrent/CompletableFuture;",false);m.visitInsn(ARETURN);m.visitMaxs(0,0);m.visitEnd();}
  MethodVisitor m=c.visitMethod(ACC_PUBLIC,"a","(Ldcd;Laql;)V",null,null);m.visitCode();m.visitVarInsn(ALOAD,2);m.visitMethodInsn(INVOKESTATIC,"oracle/chunk_lifecycle_oracle","status","(Ljava/lang/Object;)V",false);m.visitInsn(RETURN);m.visitMaxs(0,0);m.visitEnd();c.visitEnd();
  // Same loader/runtime package is essential: ChunkMap.onFullChunkStatusChange is package-private.
  MAP=MethodHandles.privateLookupIn(C("aqb"),MethodHandles.lookup()).defineClass(c.toByteArray());
 }
 static CompletableFuture<Object> future(String state)throws Exception{CompletableFuture<Object>f=new CompletableFuture<>();finish(f,state);return f;}
 static boolean finish(CompletableFuture<Object>f,String state)throws Exception{return switch(state){case"pending"->false;case"success"->f.complete(call(C("aqc"),"a",T(Object.class),chunk));case"failure"->f.complete(call(C("aqc"),"a",T(String.class),"oracle unavailable"));case"unloaded"->f.complete(unloaded);case"exceptional"->f.completeExceptionally(new IllegalStateException("oracle future failure"));case"cancelled"->f.cancel(false);default->throw new IllegalArgumentException(state);};}
 static String state(CompletableFuture<?>f)throws Exception{if(!f.isDone())return"pending";if(f.isCancelled())return"cancelled";if(f.isCompletedExceptionally())return"exceptional";Object v=f.getNow(null);if(v==null)return"completed";if(v==unloaded)return"unloaded";return (boolean)call(v,"a",T())?"success":"failure";}
 static void runTask(int id){Runnable r=TASKS.get(id);if(r==null)throw new IllegalArgumentException("unknown task");PENDING.remove(id);r.run();}
 static Object snapshot()throws Exception{
  JsonObject s=new JsonObject();s.addProperty("old_level",(Integer)field(holder,"j"));s.addProperty("ticket_level",(Integer)field(holder,"k"));s.addProperty("queue_level",(Integer)field(holder,"l"));s.addProperty("was_accessible",(Boolean)field(holder,"t"));
  s.addProperty("confirmation",state((CompletableFuture<?>)field(holder,"u")));s.addProperty("output_status",outputStatus);
  JsonArray slots=new JsonArray();for(String n:new String[]{"g","h","i"})slots.add(FUTURE_IDS.get(field(holder,n)));s.add("slots",slots);
  JsonArray futures=new JsonArray();for(Map.Entry<Integer,CompletableFuture<Object>> e:FUTURES.entrySet()){JsonArray row=new JsonArray();row.add(e.getKey());row.add(state(e.getValue()));futures.add(row);}s.add("futures",futures);s.add("pending_tasks",JSON.toJsonTree(PENDING));
  CompletableFuture<?>save=(CompletableFuture<?>)field(holder,"w");s.addProperty("save_done",save.isDone());s.addProperty("save_exceptional",save.isCompletedExceptionally());
  JsonArray observed=new JsonArray();for(IntSupplier getter:LEVEL_GETTERS)observed.add(getter.getAsInt());s.add("level_getter_values",observed);return s;
 }
 static JsonObject scene(JsonObject q)throws Exception{
  FUTURES.clear();FUTURE_IDS.clear();TASKS.clear();PENDING.clear();LEVEL_GETTERS.clear();LEVEL_SETTERS.clear();LEVEL_VALUES.clear();PREPARES.clear();events=new JsonArray();taskSequence=0;outputStatus=-1;
  executorMode=q.has("executor")?q.get("executor").getAsString():"queued";applyQueueInline=!q.has("apply_queue_inline")||q.get("apply_queue_inline").getAsBoolean();
  Field constant=C("apz").getDeclaredField("e");constant.setAccessible(true);@SuppressWarnings("unchecked")CompletableFuture<Object>sentinel=(CompletableFuture<Object>)constant.get(null);FUTURES.put(0,sentinel);FUTURE_IDS.put(sentinel,0);
  for(JsonElement e:q.getAsJsonArray("futures")){JsonObject f=e.getAsJsonObject();int id=f.get("id").getAsInt();CompletableFuture<Object>cf=future(f.get("state").getAsString());FUTURES.put(id,cf);FUTURE_IDS.put(cf,id);}
  Class<?>listener=C("apz$a"),players=C("apz$b");
  Object callback=Proxy.newProxyInstance(listener.getClassLoader(),new Class<?>[]{listener},(proxy,m,args)->{IntSupplier getter=(IntSupplier)args[1];int level=(Integer)args[2];IntConsumer setter=(IntConsumer)args[3];event("level","queue_level",getter.getAsInt(),"new_level",level);LEVEL_GETTERS.add(getter);LEVEL_SETTERS.add(setter);LEVEL_VALUES.add(level);if(applyQueueInline)setter.accept(level);return null;});
  Object provider=Proxy.newProxyInstance(players.getClassLoader(),new Class<?>[]{players},(proxy,m,args)->List.of());
  Object position=C("dcd").getConstructor(int.class,int.class).newInstance(0,0),height=call(C("dcy"),"e",T(int.class,int.class),-64,384);
  holder=C("apz").getConstructor(C("dcd"),int.class,C("dcy"),C("eot"),listener,players).newInstance(position,q.get("initial_level").getAsInt(),height,null,callback,provider);map=alloc(MAP);
  Executor executor=task->{int id=++taskSequence;TASKS.put(id,task);if(executorMode.equals("reject")){event("reject","task",id);throw new RejectedExecutionException("oracle executor rejected");}event("enqueue","task",id);if(executorMode.equals("inline"))task.run();else PENDING.add(id);};
  JsonObject result=new JsonObject();result.add("initial",JSON.toJsonTree(snapshot()));JsonArray outputs=new JsonArray();
  for(JsonElement e:q.getAsJsonArray("operations")){JsonObject op=e.getAsJsonObject();events=new JsonArray();JsonElement value=JsonNull.INSTANCE;
   switch(op.get("op").getAsString()){
    case"set_level"->call(holder,"a",T(int.class),op.get("level").getAsInt());
    case"update"->{if(op.has("level"))call(holder,"a",T(int.class),op.get("level").getAsInt());for(JsonElement p:op.getAsJsonArray("prepares"))PREPARES.add(p.getAsJsonObject());call(holder,"a",T(C("aqb"),Executor.class),map,executor);if(!PREPARES.isEmpty())throw new AssertionError("unused prepare plans");}
    case"complete"->value=new JsonPrimitive(finish(FUTURES.get(op.get("future").getAsInt()),op.get("state").getAsString()));
    case"run"->runTask(op.get("task").getAsInt());
    case"drain"->{JsonArray ran=new JsonArray();int budget=op.has("budget")?op.get("budget").getAsInt():Integer.MAX_VALUE;while(budget-->0&&!PENDING.isEmpty()){int id=PENDING.iterator().next();ran.add(id);runTask(id);}value=ran;}
    case"executor"->executorMode=op.get("mode").getAsString();
    case"apply_queue"->{int id=op.get("notification").getAsInt();LEVEL_SETTERS.get(id).accept(LEVEL_VALUES.get(id));}
    case"refresh"->call(holder,"l",T());
    default->throw new IllegalArgumentException(op.toString());
   }
   JsonObject output=new JsonObject();output.add("result",value);output.add("events",events);output.add("state",JSON.toJsonTree(snapshot()));outputs.add(output);
  }
  result.add("operations",outputs);return result;
 }
 public static void main(String[]args)throws Exception{
  call(C("ab"),"a",T());call(C("akt"),"a",T());Class<?>u=C("sun.misc.Unsafe");Field f=u.getDeclaredField("theUnsafe");f.setAccessible(true);unsafe=f.get(null);allocate=u.getMethod("allocateInstance",Class.class);chunk=alloc(C("dvi"));unloaded=C("apz").getField("a").get(null);adapters();
  OUT.println("HEADER:"+JSON.toJson(Map.of("source",C("apz").getProtectionDomain().getCodeSource().getLocation().toString(),"max_level",C("aqa").getField("b").getInt(null))));
  BufferedReader input=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=input.readLine())!=null;)OUT.println("ORACLE:"+JSON.toJson(scene(JsonParser.parseString(line).getAsJsonObject())));
 }
}
