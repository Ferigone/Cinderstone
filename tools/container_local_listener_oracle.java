package oracle;

import com.google.gson.*;
import io.netty.buffer.*;
import java.io.*;
import java.lang.reflect.*;
import java.nio.file.*;
import java.util.*;
import jdk.internal.org.objectweb.asm.*;

/** Unchanged cpu local/remote synchronization, aqv$1 packet construction and
 * aqv$2 Inventory identity gate. Only network/criteria callbacks are captured;
 * inherited InventoryMenu recipe recalculation is the existing controlled hook.
 */
public final class container_local_listener_oracle {
 static final PrintStream OUT=System.out;
 static final Gson JSON=new GsonBuilder().serializeNulls().create();
 static Class<?> connectionClass,criterionClass;
 static Object originalCriterion,unsafe,staticBase;static long staticOffset;
 static Object currentMenu,currentInventory,originalLocal;
 static Method originalSlotChanged;
 static int currentSlot=-1,packetCount;
 static JsonArray events=new JsonArray();
 static List<Object> retained=new ArrayList<>();
 static Class<?> C(String name)throws Exception{return Class.forName(name);}
 static Object call(Object o,String n,Class<?>[] t,Object...a)throws Exception{return inventory_normalization_oracle.call(o,n,t,a);}
 static Field field(String owner,String name)throws Exception{Field f=C(owner).getDeclaredField(name);f.setAccessible(true);return f;}
 static Object get(Object o,String owner,String name)throws Exception{return field(owner,name).get(o);}
 static void set(Object o,String owner,String name,Object value)throws Exception{field(owner,name).set(o,value);}
 static void init()throws Exception{
  Path temporary=Files.createTempFile("container-local-bootstrap",".json");
  try{container_oracle.main(new String[]{"equipment",temporary.toString()});}finally{Files.delete(temporary);}
  inventory_normalization_oracle.init();inventory_normalization_oracle.fullRegistries(Path.of("tools/crafting_metadata.json"));
  ClassWriter w=new ClassWriter(0);w.visit(Opcodes.V21,Opcodes.ACC_PUBLIC,"OracleLocalConnection",null,"aru",null);
  MethodVisitor m=w.visitMethod(Opcodes.ACC_PUBLIC,"b","(Lzg;)V",null,new String[]{"java/lang/Exception"});m.visitCode();m.visitVarInsn(Opcodes.ALOAD,1);m.visitMethodInsn(Opcodes.INVOKESTATIC,"oracle/container_local_listener_oracle","packet","(Ljava/lang/Object;)V",false);m.visitInsn(Opcodes.RETURN);m.visitMaxs(1,2);m.visitEnd();w.visitEnd();connectionClass=container_oracle.define("OracleLocalConnection",w.toByteArray());
  w=new ClassWriter(0);w.visit(Opcodes.V21,Opcodes.ACC_PUBLIC,"OracleLocalCriterion",null,"cg",null);
  m=w.visitMethod(Opcodes.ACC_PUBLIC,"<init>","()V",null,null);m.visitCode();m.visitVarInsn(Opcodes.ALOAD,0);m.visitMethodInsn(Opcodes.INVOKESPECIAL,"cg","<init>","()V",false);m.visitInsn(Opcodes.RETURN);m.visitMaxs(1,1);m.visitEnd();
  m=w.visitMethod(Opcodes.ACC_PUBLIC,"a","(Laqv;Lcmw;Lcuq;)V",null,new String[]{"java/lang/Exception"});m.visitCode();m.visitVarInsn(Opcodes.ALOAD,3);m.visitMethodInsn(Opcodes.INVOKESTATIC,"oracle/container_local_listener_oracle","criterion","(Ljava/lang/Object;)V",false);m.visitInsn(Opcodes.RETURN);m.visitMaxs(1,4);m.visitEnd();w.visitEnd();criterionClass=container_oracle.define("OracleLocalCriterion",w.toByteArray());
  Field singleton=C("sun.misc.Unsafe").getDeclaredField("theUnsafe");singleton.setAccessible(true);unsafe=singleton.get(null);Field trigger=field("an","f");originalCriterion=trigger.get(null);staticBase=call(unsafe,"staticFieldBase",new Class[]{Field.class},trigger);staticOffset=(long)call(unsafe,"staticFieldOffset",new Class[]{Field.class},trigger);call(unsafe,"putObject",new Class[]{Object.class,long.class,Object.class},staticBase,staticOffset,criterionClass.getConstructor().newInstance());
  originalSlotChanged=C("aqv$2").getDeclaredMethod("a",C("cpu"),int.class,C("cuq"));originalSlotChanged.setAccessible(true);
  for(String name:new String[]{"cpu","cqw","aqv$1","aqv$2","cuq","cg"})OUT.println("SOURCE:"+name+":"+C(name).getProtectionDomain().getCodeSource().getLocation());
 }
 static String wire(Object stack)throws Exception{
  ByteBuf bytes=Unpooled.buffer();try{call(inventory_normalization_oracle.slotCodec,"encode",new Class[]{Object.class,Object.class},inventory_normalization_oracle.buf(bytes),stack);byte[] raw=new byte[bytes.readableBytes()];bytes.readBytes(raw);return Base64.getEncoder().encodeToString(raw);}finally{bytes.release();}
 }
 public static void packet(Object packet)throws Exception{
  JsonObject event=new JsonObject();event.addProperty("kind","packet");String type=packet.getClass().getName();event.addProperty("id",switch(type){case "acr"->0x15;case "acp"->0x13;default->throw new AssertionError("unexpected packet "+type);});
  Object codec=packet.getClass().getField("a").get(null);ByteBuf bytes=Unpooled.buffer();try{call(codec,"encode",new Class[]{Object.class,Object.class},inventory_normalization_oracle.buf(bytes),packet);byte[] raw=new byte[bytes.readableBytes()];bytes.readBytes(raw);event.addProperty("payload",Base64.getEncoder().encodeToString(raw));}finally{bytes.release();}events.add(event);packetCount++;
 }
 public static void criterion(Object stack)throws Exception{
  JsonObject event=new JsonObject();event.addProperty("kind","criteria");event.addProperty("slot",currentSlot);event.addProperty("stack",wire(stack));event.addProperty("packet_offset",packetCount);events.add(event);
 }
 static Object listener(String id)throws Exception{
  return Proxy.newProxyInstance(C("cqh").getClassLoader(),new Class[]{C("cqh")},(proxy,method,args)->{
   if(method.getName().equals("equals"))return proxy==args[0];
   if(method.getName().equals("hashCode"))return System.identityHashCode(proxy);
   if(method.getName().equals("toString"))return "local-listener-"+id;
   if(args.length==3&&args[2].getClass()==C("cuq")){
    int index=(int)args[1];Object stack=args[2],slot=((List<?>)C("cpu").getField("i").get(currentMenu)).get(index);
    JsonObject event=new JsonObject();event.addProperty("kind","local");event.addProperty("listener",id);event.addProperty("slot",index);event.addProperty("stack",wire(stack));event.addProperty("packet_offset",packetCount);event.addProperty("canonical_empty",stack==container_oracle.empty);event.addProperty("same_as_live",stack==call(slot,"g",new Class[]{}));event.addProperty("same_as_last",stack==((List<?>)get(currentMenu,"cpu","l")).get(index));
    if(C("crq").getField("c").get(slot)==currentInventory)event.addProperty("inventory_slot",(int)call(slot,"i",new Class[]{}));else event.add("inventory_slot",JsonNull.INSTANCE);events.add(event);retained.add(stack);
    if(id.equals("main")){currentSlot=index;try{originalSlotChanged.invoke(originalLocal,currentMenu,index,stack);}finally{currentSlot=-1;}}
   }
   return null;
  });
 }
 static JsonObject snapshot()throws Exception{
  JsonObject out=new JsonObject();List<?> slots=(List<?>)C("cpu").getField("i").get(currentMenu),last=(List<?>)get(currentMenu,"cpu","l"),remote=(List<?>)get(currentMenu,"cpu","o");JsonArray actual=new JsonArray(),local=new JsonArray(),sent=new JsonArray(),emptyLocal=new JsonArray(),emptyRemote=new JsonArray(),same=new JsonArray(),copies=new JsonArray();
  for(int i=0;i<slots.size();i++){actual.add(wire(call(slots.get(i),"g",new Class[]{})));local.add(wire(last.get(i)));sent.add(wire(remote.get(i)));emptyLocal.add(last.get(i)==container_oracle.empty);emptyRemote.add(remote.get(i)==container_oracle.empty);same.add(last.get(i)==remote.get(i));}
  for(Object retainedStack:retained)copies.add(wire(retainedStack));
  out.add("slots",actual);out.add("last_slots",local);out.add("remote_slots",sent);out.addProperty("carried",wire(call(currentMenu,"g",new Class[]{})));out.addProperty("remote_carried",wire(get(currentMenu,"cpu","q")));out.addProperty("state_id",field("cpu","r").getInt(currentMenu));out.addProperty("suppressed",field("cpu","y").getBoolean(currentMenu));out.addProperty("listener_count",((List<?>)get(currentMenu,"cpu","w")).size());out.add("last_canonical_empty",emptyLocal);out.add("remote_canonical_empty",emptyRemote);out.add("last_remote_same_reference",same);out.add("retained_callback_stacks",copies);out.add("events",events.deepCopy());return out;
 }
 static JsonObject run(JsonObject query)throws Exception{
  Object player=container_oracle.newPlayer(false);currentInventory=call(player,"fY",new Class[]{});currentMenu=container_oracle.menuClass.getConstructor(C("cmw"),boolean.class,C("cmx")).newInstance(currentInventory,true,player);
  Object outer=container_oracle.unsafe(C("aqv"));set(outer,"cmx","h",currentInventory);set(outer,"aqv","c",container_oracle.unsafe(connectionClass));
  Constructor<?> syncConstructor=C("aqv$1").getDeclaredConstructor(C("aqv"));syncConstructor.setAccessible(true);Object synchronizer=syncConstructor.newInstance(outer);set(currentMenu,"cpu","x",synchronizer);
  Constructor<?> localConstructor=C("aqv$2").getDeclaredConstructor(C("aqv"));localConstructor.setAccessible(true);originalLocal=localConstructor.newInstance(outer);
  List<?> slots=(List<?>)C("cpu").getField("i").get(currentMenu);for(int i=0;i<46;i++)call(slots.get(i),"f",new Class[]{C("cuq")},container_oracle.parse(query.getAsJsonArray("slots").get(i)));call(currentMenu,"b",new Class[]{C("cuq")},container_oracle.parse(query.get("carried")));field("cpu","r").setInt(currentMenu,query.get("state_id").getAsInt());
  Map<String,Object> listeners=new HashMap<>();listeners.put("main",listener("main"));listeners.put("other",listener("other"));events=new JsonArray();retained=new ArrayList<>();packetCount=0;JsonObject out=new JsonObject();out.add("initial",snapshot());JsonArray states=new JsonArray(),normalized=new JsonArray();
  for(JsonElement value:query.getAsJsonArray("operations")){
   JsonObject op=value.getAsJsonObject();events=new JsonArray();packetCount=0;JsonObject input=op.deepCopy();String name=op.get("op").getAsString();Object stack=op.has("stack")?container_oracle.parse(op.get("stack")):null;if(stack!=null){input.addProperty("source_wire",wire(stack));if(op.has("count"))call(stack,"e",new Class[]{int.class},op.get("count").getAsInt());input.addProperty("stack_wire",wire(stack));}
   switch(name){
    case "add"->call(currentMenu,"a",new Class[]{C("cqh")},listeners.get(op.has("listener")?op.get("listener").getAsString():"main"));
    case "remove"->call(currentMenu,"b",new Class[]{C("cqh")},listeners.get(op.has("listener")?op.get("listener").getAsString():"main"));
    case "set_sync"->call(currentMenu,"a",new Class[]{C("cqi")},synchronizer);
    case "set_slot"->call(slots.get(op.get("slot").getAsInt()),"f",new Class[]{C("cuq")},stack);
    case "count"->call(call(slots.get(op.get("slot").getAsInt()),"g",new Class[]{}),"e",new Class[]{int.class},op.get("count").getAsInt());
    case "carried"->call(currentMenu,"b",new Class[]{C("cuq")},stack);
    case "remote_slot"->call(currentMenu,"a",new Class[]{int.class,C("cuq")},op.get("slot").getAsInt(),stack);
    case "remote_live"->call(currentMenu,"b",new Class[]{int.class,C("cuq")},op.get("slot").getAsInt(),call(slots.get(op.get("slot").getAsInt()),"g",new Class[]{}));
    case "remote_carried"->call(currentMenu,"a",new Class[]{C("cuq")},stack);
    case "broadcast"->call(currentMenu,"d",new Class[]{});
    case "full"->call(currentMenu,"e",new Class[]{});
    case "send_all"->call(currentMenu,"b",new Class[]{});
    case "suppress"->call(currentMenu,"h",new Class[]{});
    case "resume"->call(currentMenu,"i",new Class[]{});
    default->throw new AssertionError(name);
   }
   normalized.add(input);states.add(snapshot());
  }
  out.add("operations",normalized);out.add("states",states);return out;
 }
 public static void main(String[]arguments)throws Exception{
  init();try{BufferedReader reader=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=reader.readLine())!=null;){JsonObject query=JsonParser.parseString(line).getAsJsonObject(),out=new JsonObject();out.add("name",query.get("name"));try{out.add("expected",run(query));}catch(Throwable error){while(error instanceof InvocationTargetException&&error.getCause()!=null)error=error.getCause();out.addProperty("error",error.toString());error.printStackTrace(System.err);}OUT.println("ORACLE:"+JSON.toJson(out));}}finally{call(unsafe,"putObject",new Class[]{Object.class,long.class,Object.class},staticBase,staticOffset,originalCriterion);}
 }
}
