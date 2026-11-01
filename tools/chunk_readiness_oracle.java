package oracle;

import com.google.gson.*;
import java.io.*;
import java.lang.invoke.MethodHandles;
import java.lang.reflect.*;
import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.*;
import java.util.function.*;
import java.util.stream.*;
import jdk.internal.org.objectweb.asm.*;
import static jdk.internal.org.objectweb.asm.Opcodes.*;

/** Executes original ChunkMap range/prepare methods, genuine postprocessing and tick unpacking. */
public final class chunk_readiness_oracle {
 static final Gson JSON=new GsonBuilder().serializeNulls().create();static final PrintStream OUT=System.out;
 static Object unsafe,map,world,mainExecutor;static Method allocate;static Class<?> MAP,HOLDER,CHUNK,WORLD,EXECUTOR;
 static List<?> STATUSES;static Object full,upgradeEmpty;static long gameTime;static String actionContext="",mainMode="queued",priorityMode="queued";
 static final Map<Long,Object> HOLDERS=new HashMap<>();static final IdentityHashMap<Object,Integer> CHUNK_IDS=new IdentityHashMap<>();static final Map<Integer,Object> CHUNKS=new TreeMap<>();
 static final Map<Integer,CompletableFuture<Object>> SOURCES=new TreeMap<>(),JOBS=new TreeMap<>(),SENDS=new TreeMap<>();static final IdentityHashMap<Object,Integer>SOURCE_IDS=new IdentityHashMap<>();
 static final TreeMap<Integer,Runnable>TASKS=new TreeMap<>();static final LinkedHashMap<Integer,String>PENDING=new LinkedHashMap<>();static JsonArray events=new JsonArray();static int taskCounter,currentTask;static boolean gameTimeFault;
 static Class<?> C(String n)throws Exception{return Class.forName(n);}static Class<?>[] T(Class<?>...t){return t;}
 static Object call(Object o,String n,Class<?>[]t,Object...a)throws Exception{Class<?>c=o instanceof Class<?>?(Class<?>)o:o.getClass();Method m=null;for(Class<?>s=c;s!=null;s=s.getSuperclass()){try{m=s.getDeclaredMethod(n,t);break;}catch(NoSuchMethodException ignored){}}if(m==null)m=c.getMethod(n,t);m.setAccessible(true);return m.invoke(o instanceof Class<?>?null:o,a);}
 static Object get(Object o,String c,String n)throws Exception{Field f=C(c).getDeclaredField(n);f.setAccessible(true);return f.get(o);}static void set(Object o,String c,String n,Object v)throws Exception{Field f=C(c).getDeclaredField(n);f.setAccessible(true);f.set(o,v);}
 static Object alloc(Class<?>c)throws Exception{return allocate.invoke(unsafe,c);}static long packed(int x,int z){return((long)x&0xffffffffL)|((long)z<<32);}
 static void event(String kind,Object...fields){JsonObject e=new JsonObject();e.addProperty("kind",kind);for(int i=0;i<fields.length;i+=2){Object v=fields[i+1];if(v instanceof Number)e.addProperty((String)fields[i],(Number)v);else e.addProperty((String)fields[i],String.valueOf(v));}events.add(e);}
 public static Object lookup(long p){event("lookup","x",(int)p,"z",(int)(p>>32));return HOLDERS.get(p);}
 public static void request(Object holder,Object status)throws Exception{Object p=call(holder,"r",T());event("request","x",C("dcd").getField("e").getInt(p),"z",C("dcd").getField("f").getInt(p),"status",(int)call(status,"b",T()));}
 public static void postprocess(Object chunk,boolean before){if(before){actionContext="postprocess";event("postprocess","chunk",CHUNK_IDS.get(chunk));}else actionContext="";}
 public static void startTicks(Object chunk,boolean before){if(before){actionContext="unpack";event("unpack","chunk",CHUNK_IDS.get(chunk));}else actionContext="";}
 public static void positionRead(Object chunk){if(actionContext.isEmpty())event("send","chunk",CHUNK_IDS.get(chunk));}
 public static void executeMain(Runnable r){enqueue("main",r);}
 static void enqueue(String queue,Runnable r){int id=++taskCounter;TASKS.put(id,r);String mode=queue.equals("main")?mainMode:priorityMode;if(mode.equals("reject")){event("reject","queue",queue,"task",id);throw new RejectedExecutionException(queue+" rejected");}event("enqueue","queue",queue,"task",id);if(mode.equals("inline"))run(id);else PENDING.put(id,queue);}
 static void method(ClassWriter c,int access,String name,String desc,Consumer<MethodVisitor>body){MethodVisitor m=c.visitMethod(access,name,desc,null,null);m.visitCode();body.accept(m);m.visitMaxs(0,0);m.visitEnd();}
 static void observedVoid(ClassWriter c,String name,String desc,String parent,String bridge,boolean argument){method(c,ACC_PUBLIC,name,desc,m->{int chunk=argument?1:0,temporary=argument?2:1;Label start=new Label(),end=new Label(),handler=new Label();m.visitTryCatchBlock(start,end,handler,null);m.visitVarInsn(ALOAD,chunk);m.visitInsn(ICONST_1);m.visitMethodInsn(INVOKESTATIC,"oracle/chunk_readiness_oracle",bridge,"(Ljava/lang/Object;Z)V",false);m.visitLabel(start);m.visitVarInsn(ALOAD,0);if(argument)m.visitVarInsn(ALOAD,1);m.visitMethodInsn(INVOKESPECIAL,parent,name,desc,false);m.visitLabel(end);m.visitVarInsn(ALOAD,chunk);m.visitInsn(ICONST_0);m.visitMethodInsn(INVOKESTATIC,"oracle/chunk_readiness_oracle",bridge,"(Ljava/lang/Object;Z)V",false);m.visitInsn(RETURN);m.visitLabel(handler);m.visitVarInsn(ASTORE,temporary);m.visitVarInsn(ALOAD,chunk);m.visitInsn(ICONST_0);m.visitMethodInsn(INVOKESTATIC,"oracle/chunk_readiness_oracle",bridge,"(Ljava/lang/Object;Z)V",false);m.visitVarInsn(ALOAD,temporary);m.visitInsn(ATHROW);});}
 static Class<?> define(String name,String parent,Consumer<ClassWriter>body)throws Exception{ClassWriter c=new ClassWriter(ClassWriter.COMPUTE_FRAMES|ClassWriter.COMPUTE_MAXS);c.visit(V21,ACC_PUBLIC,name,null,parent,null);body.accept(c);c.visitEnd();return MethodHandles.privateLookupIn(C(parent),MethodHandles.lookup()).defineClass(c.toByteArray());}
 static void adapters()throws Exception{
  MAP=define("ChunkReadinessOracleMap","aqb",c->method(c,ACC_PUBLIC,"a","(J)Lapz;",m->{m.visitVarInsn(LLOAD,1);m.visitMethodInsn(INVOKESTATIC,"oracle/chunk_readiness_oracle","lookup","(J)Ljava/lang/Object;",false);m.visitTypeInsn(CHECKCAST,"apz");m.visitInsn(ARETURN);}));
  HOLDER=define("ChunkReadinessOracleHolder","apz",c->{
   method(c,ACC_PUBLIC,"<init>","(Ldcd;ILdcy;Leot;Lapz$a;Lapz$b;)V",m->{m.visitVarInsn(ALOAD,0);m.visitVarInsn(ALOAD,1);m.visitVarInsn(ILOAD,2);for(int i=3;i<=6;i++)m.visitVarInsn(ALOAD,i);m.visitMethodInsn(INVOKESPECIAL,"apz","<init>","(Ldcd;ILdcy;Leot;Lapz$a;Lapz$b;)V",false);m.visitInsn(RETURN);});
   method(c,ACC_PUBLIC,"a","(Ldvz;Laqb;)Ljava/util/concurrent/CompletableFuture;",m->{m.visitVarInsn(ALOAD,0);m.visitVarInsn(ALOAD,1);m.visitMethodInsn(INVOKESTATIC,"oracle/chunk_readiness_oracle","request","(Ljava/lang/Object;Ljava/lang/Object;)V",false);m.visitVarInsn(ALOAD,0);m.visitVarInsn(ALOAD,1);m.visitVarInsn(ALOAD,2);m.visitMethodInsn(INVOKESPECIAL,"aqn","a","(Ldvz;Laqb;)Ljava/util/concurrent/CompletableFuture;",false);m.visitInsn(ARETURN);});
  });
  CHUNK=define("ChunkReadinessOracleChunk","dvi",c->{
   observedVoid(c,"H","()V","dvi","postprocess",false);
   method(c,ACC_PUBLIC,"f","()Ldcd;",m->{m.visitVarInsn(ALOAD,0);m.visitMethodInsn(INVOKESTATIC,"oracle/chunk_readiness_oracle","positionRead","(Ljava/lang/Object;)V",false);m.visitVarInsn(ALOAD,0);m.visitMethodInsn(INVOKESPECIAL,"duy","f","()Ldcd;",false);m.visitInsn(ARETURN);});
  });
  WORLD=define("ChunkReadinessOracleLevel","aqu",c->observedVoid(c,"b","(Ldvi;)V","aqu","startTicks",true));
  EXECUTOR=define("ChunkReadinessOracleExecutor","bph",c->method(c,ACC_PUBLIC,"execute","(Ljava/lang/Runnable;)V",m->{m.visitVarInsn(ALOAD,1);m.visitMethodInsn(INVOKESTATIC,"oracle/chunk_readiness_oracle","executeMain","(Ljava/lang/Runnable;)V",false);m.visitInsn(RETURN);}));
 }
 static Object position(int x,int y,int z)throws Exception{return C("jd").getConstructor(int.class,int.class,int.class).newInstance(x,y,z);}
 static Object chunk(int id,int x,int z,String kind,boolean saved)throws Exception{
  Object chunk=alloc(kind.equals("full")?CHUNK:C("dvs"));CHUNK_IDS.put(chunk,id);CHUNKS.put(id,chunk);set(chunk,"duy","d",C("dcd").getConstructor(int.class,int.class).newInstance(x,z));
  if(kind.equals("full")){
   set(chunk,"duy","b",Array.newInstance(C("it.unimi.dsi.fastutil.shorts.ShortList"),24));set(chunk,"duy","j",new HashMap<>());set(chunk,"duy","f",upgradeEmpty);set(chunk,"dvi","r",world);
   for(String field:new String[]{"v","w"}){
    Object type=field.equals("v")?call(call(C("dfy"),"a",T(int.class),1),"b",T()):C("epf").getField("c").get(null);
    List<Object> pending=new ArrayList<>();if(saved)for(int i=0;i<2;i++)pending.add(C("eys").getConstructor(Object.class,C("jd"),int.class,C("eyx")).newInstance(type,position((x<<4)+i,64,z<<4),i==0?5:-1,C("eyx").getField("d").get(null)));
    set(chunk,"dvi",field,C("eyo").getConstructor(List.class).newInstance(pending));
   }
  }return chunk;
 }
 static Object holder(int x,int z)throws Exception{
  Object p=C("dcd").getConstructor(int.class,int.class).newInstance(x,z);Class<?>l=C("apz$a"),v=C("apz$b");
  Object listener=Proxy.newProxyInstance(l.getClassLoader(),new Class<?>[]{l},(a,b,c)->null),players=Proxy.newProxyInstance(v.getClassLoader(),new Class<?>[]{v},(a,b,c)->List.of());
  Object h=HOLDER.getConstructor(C("dcd"),int.class,C("dcy"),C("eot"),l,v).newInstance(p,33,call(C("dcy"),"e",T(int.class,int.class),-64,384),null,listener,players);
  set(h,"aqn","f",full);Object task=alloc(C("apy"));set(task,"apy","a",full);((AtomicReference<Object>)get(h,"aqn","i")).set(task);return h;
 }
 static Object outcome(JsonObject spec)throws Exception{
  String state=spec.get("state").getAsString();if(state.equals("success"))return call(C("aqc"),"a",T(Object.class),CHUNKS.get(spec.get("chunk").getAsInt()));
  return call(C("aqc"),"a",T(String.class),spec.has("reason")?spec.get("reason").getAsString():"oracle unavailable");
 }
 static boolean finish(CompletableFuture<Object>f,JsonObject spec)throws Exception{return switch(spec.get("state").getAsString()){case"pending"->false;case"success","failure"->f.complete(outcome(spec));case"exceptional"->f.completeExceptionally(new IllegalStateException("oracle future failure"));case"cancelled"->f.cancel(false);default->throw new IllegalArgumentException(spec.toString());};}
 static JsonObject state(CompletableFuture<?>f)throws Exception{JsonObject out=new JsonObject();if(!f.isDone()){out.addProperty("state","pending");return out;}if(f.isCancelled()){out.addProperty("state","cancelled");return out;}if(f.isCompletedExceptionally()){out.addProperty("state","exceptional");try{f.join();}catch(Throwable e){while(e.getCause()!=null)e=e.getCause();out.addProperty("error_class",e.getClass().getName());}return out;}Object value=f.getNow(null);if(value==null){out.addProperty("state","completed");return out;}boolean success=(boolean)call(value,"a",T());out.addProperty("state",success?"success":"failure");if(success){Object data=call(value,"b",T(Object.class),(Object)null);if(data instanceof List<?>list){JsonArray ids=new JsonArray();for(Object c:list)ids.add(CHUNK_IDS.get(c));out.add("chunks",ids);}else out.addProperty("chunk",CHUNK_IDS.get(data));}else out.addProperty("reason",(String)call(value,"b",T()));return out;}
 static JsonArray ticks(Object chunk,String field)throws Exception{JsonArray out=new JsonArray();Object container=get(chunk,"dvi",field);for(Object tick:((Stream<?>)call(container,"d",T())).toList()){JsonObject t=new JsonObject();t.addProperty("time",String.valueOf(call(tick,"c",T())));t.addProperty("order",String.valueOf(call(tick,"e",T())));out.add(t);}return out;}
 static JsonObject snapshot()throws Exception{JsonObject out=new JsonObject();JsonObject jobs=new JsonObject();for(var e:JOBS.entrySet())jobs.add(e.getKey().toString(),state(e.getValue()));out.add("jobs",jobs);JsonArray pending=new JsonArray();for(var e:PENDING.entrySet())pending.add(JSON.toJsonTree(Map.of("task",e.getKey(),"queue",e.getValue())));out.add("pending",pending);out.addProperty("generated_counter",((AtomicInteger)get(map,"aqb","H")).get());JsonObject cs=new JsonObject();for(var e:CHUNKS.entrySet())if(CHUNK.isInstance(e.getValue())){JsonObject c=new JsonObject();c.add("block_ticks",ticks(e.getValue(),"v"));c.add("fluid_ticks",ticks(e.getValue(),"w"));cs.add(e.getKey().toString(),c);}out.add("chunks",cs);return out;}
 static void run(int task){PENDING.remove(task);int previous=currentTask;currentTask=task;try{TASKS.get(task).run();}finally{currentTask=previous;}}
 static JsonObject scene(JsonObject q)throws Exception{
  HOLDERS.clear();CHUNK_IDS.clear();CHUNKS.clear();SOURCES.clear();SOURCE_IDS.clear();JOBS.clear();SENDS.clear();TASKS.clear();PENDING.clear();events=new JsonArray();taskCounter=0;actionContext="";gameTime=q.has("game_time")?q.get("game_time").getAsLong():123;gameTimeFault=false;mainMode=q.has("main_mode")?q.get("main_mode").getAsString():"queued";priorityMode=q.has("priority_mode")?q.get("priority_mode").getAsString():"queued";
  world=alloc(WORLD);Object levelData=Proxy.newProxyInstance(C("erm").getClassLoader(),new Class<?>[]{C("erm")},(p,m,a)->{if(m.getName().equals("c")){if(gameTimeFault)throw new IllegalStateException("oracle game time unavailable");event("game_time","value",Long.toString(gameTime));return gameTime;}throw new UnsupportedOperationException(m.toString());});set(world,"dcw","A",levelData);
  map=alloc(MAP);mainExecutor=alloc(EXECUTOR);set(map,"aqb","u",mainExecutor);set(map,"aqb","s",world);set(map,"aqb","H",new AtomicInteger());set(map,"aqb","J",C("aqo").getConstructor().newInstance());
  Object sender=Proxy.newProxyInstance(C("bpj").getClassLoader(),new Class<?>[]{C("bpj")},(p,m,a)->{if(m.getName().equals("bz"))return"oracle priority";if(m.getName().equals("a")){Object message=a[0];Field f=message.getClass().getDeclaredField("a");f.setAccessible(true);Object ack=Proxy.newProxyInstance(C("bpj").getClassLoader(),new Class<?>[]{C("bpj")},(r,s,t)->{if(s.getName().equals("a"))event("priority_ack","task",currentTask);return null;});Runnable runnable=(Runnable)((Function<Object,Object>)f.get(message)).apply(ack);enqueue("priority",runnable);return null;}throw new UnsupportedOperationException(m.toString());});set(map,"aqb","D",sender);
  for(JsonElement e:q.getAsJsonArray("chunks")){JsonObject c=e.getAsJsonObject();chunk(c.get("id").getAsInt(),c.get("x").getAsInt(),c.get("z").getAsInt(),c.get("kind").getAsString(),c.has("saved_ticks")&&c.get("saved_ticks").getAsBoolean());}
  for(JsonElement e:q.getAsJsonArray("sources")){JsonObject s=e.getAsJsonObject();CompletableFuture<Object>f=new CompletableFuture<>();finish(f,s);SOURCES.put(s.get("id").getAsInt(),f);SOURCE_IDS.put(f,s.get("id").getAsInt());}
  for(JsonElement e:q.getAsJsonArray("holders")){JsonObject h=e.getAsJsonObject();int x=h.get("x").getAsInt(),z=h.get("z").getAsInt();Object holder=holder(x,z);HOLDERS.put(packed(x,z),holder);if(h.has("allowed"))set(holder,"aqn","f",h.get("allowed").isJsonNull()?null:STATUSES.get(h.get("allowed").getAsInt()));AtomicReferenceArray<Object>array=(AtomicReferenceArray<Object>)get(holder,"aqn","h");for(JsonElement r:h.getAsJsonArray("requests")){JsonArray a=r.getAsJsonArray();array.set(a.get(0).getAsInt(),SOURCES.get(a.get(1).getAsInt()));}if(h.has("send_state")){CompletableFuture<Object>send=new CompletableFuture<>();String state=h.get("send_state").getAsString();if(state.equals("success"))send.complete(null);else if(state.equals("exceptional"))send.completeExceptionally(new IllegalStateException("oracle send failure"));else if(state.equals("cancelled"))send.cancel(false);call(holder,"a",T(CompletableFuture.class),send);SENDS.put(h.get("send_id").getAsInt(),send);}}
  Object actualMap=C("it.unimi.dsi.fastutil.longs.Long2ObjectLinkedOpenHashMap").getConstructor().newInstance();for(var e:HOLDERS.entrySet())call(actualMap,"put",T(long.class,Object.class),e.getKey(),e.getValue());set(map,"aqb","o",actualMap);set(map,"aqb","p",actualMap);
  JsonObject result=new JsonObject();result.add("initial",snapshot());JsonArray results=new JsonArray();
  for(JsonElement e:q.getAsJsonArray("operations")){JsonObject op=e.getAsJsonObject();events=new JsonArray();JsonElement value=JsonNull.INSTANCE;
   switch(op.get("op").getAsString()){
    case"prepare"->{Object h=HOLDERS.get(packed(op.get("x").getAsInt(),op.get("z").getAsInt()));String kind=op.get("kind").getAsString();CompletableFuture<Object>future=(CompletableFuture<Object>)call(map,kind.equals("accessible")?"c":kind.equals("block")?"b":"a",T(C("apz")),h);JOBS.put(op.get("job").getAsInt(),future);}
    case"range"->{Object h=HOLDERS.get(packed(op.get("x").getAsInt(),op.get("z").getAsInt()));JsonArray statuses=op.getAsJsonArray("statuses");IntFunction<Object>fn=distance->STATUSES.get(statuses.get(distance).getAsInt());CompletableFuture<Object>future=(CompletableFuture<Object>)call(map,"a",T(C("apz"),int.class,IntFunction.class),h,op.get("radius").getAsInt(),fn);JOBS.put(op.get("job").getAsInt(),future);}
    case"observe"->{int id=op.get("job").getAsInt(),observer=op.get("observer").getAsInt();JOBS.get(id).whenComplete((v,t)->{try{event("complete","observer",observer,"job",id,"counter",((AtomicInteger)get(map,"aqb","H")).get());}catch(Exception ex){throw new RuntimeException(ex);}});}
    case"source"->value=new JsonPrimitive(finish(SOURCES.get(op.get("id").getAsInt()),op));
    case"job"->value=new JsonPrimitive(finish(JOBS.get(op.get("id").getAsInt()),op));
    case"send"->{CompletableFuture<Object>f=SENDS.get(op.get("id").getAsInt());String state=op.get("state").getAsString();value=new JsonPrimitive(state.equals("success")?f.complete(null):state.equals("cancelled")?f.cancel(false):f.completeExceptionally(new IllegalStateException("oracle send failure")));}
    case"run"->run(op.get("task").getAsInt());
    case"drain"->{String queue=op.has("queue")?op.get("queue").getAsString():"any";int budget=op.has("budget")?op.get("budget").getAsInt():Integer.MAX_VALUE;JsonArray ran=new JsonArray();while(budget-->0){Integer id=PENDING.entrySet().stream().filter(p->queue.equals("any")||p.getValue().equals(queue)).map(Map.Entry::getKey).findFirst().orElse(null);if(id==null)break;ran.add(id);run(id);}value=ran;}
    case"time"->gameTime=op.get("value").getAsLong();
    case"time_fault"->gameTimeFault=op.get("value").getAsBoolean();
    case"executor"->{if(op.get("queue").getAsString().equals("main"))mainMode=op.get("mode").getAsString();else priorityMode=op.get("mode").getAsString();}
    default->throw new IllegalArgumentException(op.toString());
   }
   JsonObject r=new JsonObject();r.add("result",value);r.add("events",events);r.add("state",snapshot());results.add(r);
  }result.add("operations",results);return result;
 }
 public static void main(String[]args)throws Exception{
  call(C("ab"),"a",T());call(C("akt"),"a",T());Class<?>u=C("sun.misc.Unsafe");Field f=u.getDeclaredField("theUnsafe");f.setAccessible(true);unsafe=f.get(null);allocate=u.getMethod("allocateInstance",Class.class);STATUSES=(List<?>)call(C("dvz"),"a",T());full=C("dvz").getField("n").get(null);upgradeEmpty=C("dvv").getField("a").get(null);adapters();
  JsonObject header=new JsonObject();header.addProperty("source",C("aqb").getProtectionDomain().getCodeSource().getLocation().toString());JsonArray statuses=new JsonArray();for(Object s:STATUSES)statuses.add(s.toString());header.add("statuses",statuses);JsonArray around=new JsonArray();for(int d=0;d<12;d++)around.add((int)call(call(C("aqa"),"b",T(int.class),d),"b",T()));header.add("status_around_full",around);header.addProperty("generation_unloaded_error",(String)call(((CompletableFuture<?>)C("aqn").getField("c").get(null)).join(),"b",T()));JsonObject allowed=new JsonObject();for(int level:new int[]{Integer.MIN_VALUE,Integer.MIN_VALUE+33,-1,0,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,Integer.MAX_VALUE}){Object s=call(C("aqa"),"a",T(int.class),level);allowed.add(Integer.toString(level),s==null?JsonNull.INSTANCE:new JsonPrimitive((int)call(s,"b",T())));}header.add("allowed_generation_status",allowed);OUT.println("HEADER:"+JSON.toJson(header));
  BufferedReader input=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=input.readLine())!=null;)OUT.println("ORACLE:"+JSON.toJson(scene(JsonParser.parseString(line).getAsJsonObject())));
 }
}
