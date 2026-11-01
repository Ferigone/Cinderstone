package oracle;
import com.google.gson.*;
import java.io.*;
import java.lang.reflect.*;
import java.util.*;
import jdk.internal.org.objectweb.asm.*;
import static jdk.internal.org.objectweb.asm.Opcodes.*;

/** Actual arq methods, original real LevelChunk handles; only host adapters are generated. */
public final class player_chunk_sender_oracle {
 static final PrintStream OUT=System.out;static final Gson JSON=new Gson();
 static Class<?> MAP,LEVEL,PLAYER,CONNECTION;static Object map,level,player,connection,cache,sender,light;
 static JsonArray trace;static Map<Long,Object> ready;static int cx,cz;static boolean alive;static String fail;
 static Class<?> C(String name)throws Exception{return Class.forName(name);}
 static Object call(Object r,String n,Class<?>[]t,Object...a)throws Exception{return survival_oracle.call(r,n,t,a);}
 static Object get(Object r,String owner,String name)throws Exception{Field f=C(owner).getDeclaredField(name);f.setAccessible(true);return f.get(r);}
 static void set(Object r,String owner,String name,Object value)throws Exception{survival_oracle.set(r,owner,name,value);}
 static Object alloc(Class<?> c)throws Exception{return survival_oracle.alloc(c);}
 static void event(Object...v){JsonArray row=new JsonArray();for(Object x:v){if(x instanceof Number n)row.add(n);else if(x instanceof Boolean b)row.add(b);else row.add(x.toString());}trace.add(row);}
 static long key(int x,int z){return (x&0xffffffffL)|((long)z<<32);}
 static Object pos(int x,int z)throws Exception{return C("dcd").getConstructor(int.class,int.class).newInstance(x,z);}
 static Object chunk(int x,int z)throws Exception{Object value=alloc(C("dvi"));set(value,"duy","d",pos(x,z));set(value,"duy","h",new HashMap<>());set(value,"duy","k",new HashMap<>());set(value,"duy","m",Array.newInstance(C("dvj"),0));return value;}
 public static Object ready(long key){event("ready",(int)key,(int)(key>>>32));return ready.get(key);}
 public static Object center()throws Exception{event("center",cx,cz);return pos(cx,cz);}
 public static boolean alive(){event("alive",alive);return alive;}
 public static Object world(){return level;}
 public static Object source(){return cache;}
 public static Object light(){return light;}
 public static void packet(Object packet)throws Exception{
   if(C("acj").isInstance(packet)){event("begin");if(fail.equals("begin"))throw new IllegalStateException("controlled transport failure");return;}
   if(C("adg").isInstance(packet)){event("chunk",call(packet,"b",new Class<?>[]{}),call(packet,"e",new Class<?>[]{}));if(fail.equals("chunk"))throw new IllegalStateException("controlled transport failure");return;}
   if(C("aci").isInstance(packet)){event("end",call(packet,"b",new Class<?>[]{}));if(fail.equals("end"))throw new IllegalStateException("controlled transport failure");return;}
   if(C("ada").isInstance(packet)){Object p=call(packet,"b",new Class<?>[]{});event("unload",C("dcd").getField("e").getInt(p),C("dcd").getField("f").getInt(p));return;}
   throw new IllegalStateException("unexpected packet "+packet.getClass());
 }
 static void adapters()throws Exception{
  survival_oracle loader=new survival_oracle();String owner="oracle/player_chunk_sender_oracle";
  ClassWriter c=survival_oracle.begin("oracle/ChunkSenderMap","aqb");MethodVisitor m=survival_oracle.method(c,"e","(J)Ldvi;");m.visitVarInsn(LLOAD,1);m.visitMethodInsn(INVOKESTATIC,owner,"ready","(J)Ljava/lang/Object;",false);m.visitTypeInsn(CHECKCAST,"dvi");m.visitInsn(ARETURN);survival_oracle.end(m);MAP=survival_oracle.define(loader,"oracle/ChunkSenderMap",c);
  c=survival_oracle.begin("oracle/ChunkSenderLevel","aqu");m=survival_oracle.method(c,"l","()Laqs;");m.visitMethodInsn(INVOKESTATIC,owner,"source","()Ljava/lang/Object;",false);m.visitTypeInsn(CHECKCAST,"aqs");m.visitInsn(ARETURN);survival_oracle.end(m);m=survival_oracle.method(c,"y_","()Leot;");m.visitMethodInsn(INVOKESTATIC,owner,"light","()Ljava/lang/Object;",false);m.visitTypeInsn(CHECKCAST,"eot");m.visitInsn(ARETURN);survival_oracle.end(m);LEVEL=survival_oracle.define(loader,"oracle/ChunkSenderLevel",c);
  c=survival_oracle.begin("oracle/ChunkSenderPlayer","aqv");
  m=survival_oracle.method(c,"A","()Laqu;");m.visitMethodInsn(INVOKESTATIC,owner,"world","()Ljava/lang/Object;",false);m.visitTypeInsn(CHECKCAST,"aqu");m.visitInsn(ARETURN);survival_oracle.end(m);
  m=survival_oracle.method(c,"dq","()Ldcd;");m.visitMethodInsn(INVOKESTATIC,owner,"center","()Ljava/lang/Object;",false);m.visitTypeInsn(CHECKCAST,"dcd");m.visitInsn(ARETURN);survival_oracle.end(m);
  m=survival_oracle.method(c,"bE","()Z");m.visitMethodInsn(INVOKESTATIC,owner,"alive","()Z",false);m.visitInsn(IRETURN);survival_oracle.end(m);PLAYER=survival_oracle.define(loader,"oracle/ChunkSenderPlayer",c);
  c=survival_oracle.begin("oracle/ChunkSenderConnection","aru");m=survival_oracle.method(c,"b","(Lzg;)V");m.visitVarInsn(ALOAD,1);m.visitMethodInsn(INVOKESTATIC,owner,"packet","(Ljava/lang/Object;)V",false);m.visitInsn(RETURN);survival_oracle.end(m);CONNECTION=survival_oracle.define(loader,"oracle/ChunkSenderConnection",c);
  map=alloc(MAP);level=alloc(LEVEL);cache=alloc(C("aqs"));player=alloc(PLAYER);connection=alloc(CONNECTION);set(cache,"aqs","a",map);set(player,"aqv","c",connection);light=alloc(C("eot"));set(light,"eot","c",C("dcy").getMethod("e",int.class,int.class).invoke(null,0,0));
 }
 static JsonObject state()throws Exception{
  JsonObject result=new JsonObject();for(String n:new String[]{"h","i"})result.addProperty(n,survival_oracle.bits((Float)get(sender,"arq",n)));for(String n:new String[]{"j","k"})result.addProperty(n,(Integer)get(sender,"arq",n));
  Object pending=get(sender,"arq","f");Object stream=C("it.unimi.dsi.fastutil.longs.LongCollection").getMethod("longStream").invoke(pending);long[] keys=(long[])java.util.stream.LongStream.class.getMethod("toArray").invoke(stream);JsonArray list=new JsonArray();for(long key:keys){JsonArray p=new JsonArray();p.add((int)key);p.add((int)(key>>>32));list.add(p);}result.add("pending",list);return result;
 }
 static JsonObject query(JsonObject q)throws Exception{
  sender=C("arq").getConstructor(boolean.class).newInstance(q.get("memory").getAsBoolean());JsonArray outputs=new JsonArray();
  for(JsonElement e:q.getAsJsonArray("steps")){
   JsonObject step=e.getAsJsonObject();trace=new JsonArray();fail=step.has("fail")?step.get("fail").getAsString():"begin";ready=new HashMap<>();if(step.has("ready"))for(JsonElement r:step.getAsJsonArray("ready")){JsonArray a=r.getAsJsonArray();ready.put(key(a.get(0).getAsInt(),a.get(1).getAsInt()),chunk(a.get(2).getAsInt(),a.get(3).getAsInt()));}
   cx=step.has("center")?step.getAsJsonArray("center").get(0).getAsInt():0;cz=step.has("center")?step.getAsJsonArray("center").get(1).getAsInt():0;alive=!step.has("alive")||step.get("alive").getAsBoolean();String op=step.get("op").getAsString();JsonArray selected=new JsonArray();boolean failed=false;
   try{switch(op){
    case "mark":for(JsonElement p:step.getAsJsonArray("positions")){JsonArray a=p.getAsJsonArray();call(sender,"a",new Class<?>[]{C("dvi")},chunk(a.get(0).getAsInt(),a.get(1).getAsInt()));}break;
    case "ack":call(sender,"a",new Class<?>[]{float.class},survival_oracle.f(step.get("desired")));break;
    case "raw":for(String n:new String[]{"h","i"})if(step.has(n))set(sender,"arq",n,survival_oracle.f(step.get(n)));for(String n:new String[]{"j","k"})if(step.has(n))set(sender,"arq",n,step.get(n).getAsInt());break;
    case "drop":JsonArray p=step.getAsJsonArray("position");call(sender,"a",new Class<?>[]{C("aqv"),C("dcd")},player,pos(p.get(0).getAsInt(),p.get(1).getAsInt()));break;
    case "collect":List<?> result=(List<?>)call(sender,"a",new Class<?>[]{C("aqb"),C("dcd")},map,pos(cx,cz));for(Object v:result){Object cp=call(v,"f",new Class<?>[]{});JsonArray a=new JsonArray();a.add(C("dcd").getField("e").getInt(cp));a.add(C("dcd").getField("f").getInt(cp));selected.add(a);}break;
    case "send":call(sender,"a",new Class<?>[]{C("aqv")},player);break;
    default:throw new IllegalArgumentException(op);
   }}catch(InvocationTargetException ex){Throwable cause=ex.getCause();if(cause instanceof IllegalStateException&&cause.getMessage().equals("controlled transport failure"))failed=true;else throw ex;}
   JsonObject out=state();out.add("trace",trace);out.add("selected",selected);out.addProperty("failed_transport",failed);outputs.add(out);
  }
  JsonObject output=new JsonObject();output.add("steps",outputs);return output;
 }
 public static void main(String[]args)throws Exception{
  C("ab").getMethod("a").invoke(null);C("akt").getMethod("a").invoke(null);Class<?> u=C("sun.misc.Unsafe");Field f=u.getDeclaredField("theUnsafe");f.setAccessible(true);survival_oracle.unsafe=f.get(null);survival_oracle.allocate=u.getMethod("allocateInstance",Class.class);adapters();
  Object template=C("agh").getField("a").get(null);Class<?> visitor=C("wh$a$a");Object proxy=Proxy.newProxyInstance(visitor.getClassLoader(),new Class<?>[]{visitor},(p,m,a)->{if(m.getName().equals("accept")){Object type=a[0];Object key=type.getClass().getMethod("b").invoke(type);if(key.toString().equals("minecraft:chunk_batch_received"))OUT.println("ACK_ID:"+a[1]);}return null;});call(template,"a",new Class<?>[]{visitor},proxy);
  for(String n:new String[]{"arq","dvi","dcd","adg","adf","adk","eot","agf"})OUT.println("SOURCE:"+n+":"+C(n).getProtectionDomain().getCodeSource().getLocation());
  BufferedReader input=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=input.readLine())!=null;)try{OUT.println("ORACLE:"+JSON.toJson(query(JsonParser.parseString(line).getAsJsonObject())));}catch(Throwable error){OUT.println("QUERY:"+line);error.printStackTrace(OUT);throw error;}
 }
}
