package oracle;

import com.google.gson.*;
import io.netty.buffer.*;
import java.io.*;
import java.lang.reflect.*;
import java.nio.file.*;
import java.util.*;
import jdk.internal.org.objectweb.asm.*;

/** Original aqv.drop(boolean), cmw.removeFromSelected, cpu remote prediction,
 * cpu.broadcastChanges and original aqv$1 synchronization. Subclass boundaries
 * supply the world/server, network sink and final item-entity creation only.
 * The same environment can execute the original creative packet handler/tick. */
public final class selected_drop_oracle {
 static final PrintStream OUT=System.out;
 static final Gson JSON=new GsonBuilder().serializeNulls().create();
 public static Object server,world,currentPlayer,marker,features;
 public static boolean creative=true;
 static Class<?> playerClass,listenerClass,gameModeClass;
 static JsonArray packets=new JsonArray(),attempts=new JsonArray();
 static Class<?> C(String n)throws Exception{return Class.forName(n);}
 static Object call(Object o,String n,Class<?>[]t,Object...a)throws Exception{return inventory_normalization_oracle.call(o,n,t,a);}
 static Field field(Class<?> c,String name)throws Exception{Field f=c.getDeclaredField(name);f.setAccessible(true);return f;}
 static void set(Object o,String owner,String name,Object value)throws Exception{field(C(owner),name).set(o,value);}
 static void staticObject(ClassWriter w,String name,String descriptor,String source,String cast){
  MethodVisitor m=w.visitMethod(Opcodes.ACC_PUBLIC,name,descriptor,null,null);m.visitCode();
  m.visitFieldInsn(Opcodes.GETSTATIC,"oracle/selected_drop_oracle",source,"Ljava/lang/Object;");
  m.visitTypeInsn(Opcodes.CHECKCAST,cast);m.visitInsn(Opcodes.ARETURN);m.visitMaxs(1,1);m.visitEnd();
 }
 static void init()throws Exception{
  Path temporary=Files.createTempFile("selected-drop-bootstrap",".json");
  try{container_oracle.main(new String[]{"equipment",temporary.toString()});}finally{Files.delete(temporary);}
  inventory_normalization_oracle.init();inventory_normalization_oracle.fullRegistries(Path.of("tools/crafting_metadata.json"));
  ClassWriter w=new ClassWriter(0);w.visit(Opcodes.V21,Opcodes.ACC_PUBLIC,"OracleSelectedDropServer",null,"net/minecraft/server/MinecraftServer",null);
  MethodVisitor m=w.visitMethod(Opcodes.ACC_PUBLIC,"bx","()Z",null,null);m.visitCode();m.visitInsn(Opcodes.ICONST_1);m.visitInsn(Opcodes.IRETURN);m.visitMaxs(1,1);m.visitEnd();w.visitEnd();server=container_oracle.unsafe(container_oracle.define("OracleSelectedDropServer",w.toByteArray()));
  w=new ClassWriter(0);w.visit(Opcodes.V21,Opcodes.ACC_PUBLIC,"OracleSelectedDropLevel",null,"aqu",null);
  staticObject(w,"o","()Lnet/minecraft/server/MinecraftServer;","server","net/minecraft/server/MinecraftServer");
  staticObject(w,"J","()Lcpl;","features","cpl");w.visitEnd();world=container_oracle.unsafe(container_oracle.define("OracleSelectedDropLevel",w.toByteArray()));
  w=new ClassWriter(0);w.visit(Opcodes.V21,Opcodes.ACC_PUBLIC,"OracleSelectedDropPlayer",null,"aqv",null);
  staticObject(w,"A","()Laqu;","world","aqu");staticObject(w,"dO","()Ldcw;","world","dcw");
  container_oracle.returnStatic(w,"f","()Z","oracle/selected_drop_oracle","creative","Z",Opcodes.IRETURN);
  container_oracle.noOp(w,"a","(Lbsy;Lcuq;Lcuq;)V",4);container_oracle.noOp(w,"m","()V",1);
  container_oracle.noOp(w,"a","(DDDFF)V",9);
  m=w.visitMethod(Opcodes.ACC_PUBLIC,"cZ","()Lbsr;",null,null);m.visitCode();m.visitVarInsn(Opcodes.ALOAD,0);m.visitInsn(Opcodes.ARETURN);m.visitMaxs(1,1);m.visitEnd();
  m=w.visitMethod(Opcodes.ACC_PUBLIC,"a","(Lcuq;ZZ)Lcjh;",null,new String[]{"java/lang/Exception"});m.visitCode();m.visitVarInsn(Opcodes.ALOAD,0);m.visitVarInsn(Opcodes.ALOAD,1);m.visitVarInsn(Opcodes.ILOAD,2);m.visitVarInsn(Opcodes.ILOAD,3);m.visitMethodInsn(Opcodes.INVOKESTATIC,"oracle/selected_drop_oracle","drop","(Ljava/lang/Object;Ljava/lang/Object;ZZ)Ljava/lang/Object;",false);m.visitTypeInsn(Opcodes.CHECKCAST,"cjh");m.visitInsn(Opcodes.ARETURN);m.visitMaxs(4,4);m.visitEnd();w.visitEnd();playerClass=container_oracle.define("OracleSelectedDropPlayer",w.toByteArray());
  w=new ClassWriter(0);w.visit(Opcodes.V21,Opcodes.ACC_PUBLIC,"OracleSelectedDropGameMode",null,"aqw",null);container_oracle.returnStatic(w,"e","()Z","oracle/selected_drop_oracle","creative","Z",Opcodes.IRETURN);w.visitEnd();gameModeClass=container_oracle.define("OracleSelectedDropGameMode",w.toByteArray());
  w=new ClassWriter(0);w.visit(Opcodes.V21,Opcodes.ACC_PUBLIC,"OracleSelectedDropListener",null,"aru",null);container_oracle.noOp(w,"e","()V",1);
  m=w.visitMethod(Opcodes.ACC_PUBLIC,"b","(Lzg;)V",null,new String[]{"java/lang/Exception"});m.visitCode();m.visitVarInsn(Opcodes.ALOAD,1);m.visitMethodInsn(Opcodes.INVOKESTATIC,"oracle/selected_drop_oracle","packet","(Ljava/lang/Object;)V",false);m.visitInsn(Opcodes.RETURN);m.visitMaxs(1,2);m.visitEnd();w.visitEnd();listenerClass=container_oracle.define("OracleSelectedDropListener",w.toByteArray());
  marker=container_oracle.unsafe(C("cjh"));
  for(String name:new String[]{"aqv","cmw","cpu","aru","aqv$1"})OUT.println("SOURCE:"+name+":"+C(name).getProtectionDomain().getCodeSource().getLocation());
 }
 static byte[] wireBytes(Object stack)throws Exception{
  ByteBuf bytes=Unpooled.buffer();try{call(inventory_normalization_oracle.slotCodec,"encode",new Class[]{Object.class,Object.class},inventory_normalization_oracle.buf(bytes),stack);byte[] raw=new byte[bytes.readableBytes()];bytes.readBytes(raw);return raw;}finally{bytes.release();}
 }
 static String wire(Object stack)throws Exception{return Base64.getEncoder().encodeToString(wireBytes(stack));}
 public static void packet(Object packet)throws Exception{
  JsonObject out=new JsonObject();String type=packet.getClass().getName();out.addProperty("id",switch(type){case "acr"->0x15;case "acp"->0x13;default->throw new AssertionError("unhandled packet "+type);});
  Object codec=packet.getClass().getField("a").get(null);ByteBuf bytes=Unpooled.buffer();try{call(codec,"encode",new Class[]{Object.class,Object.class},inventory_normalization_oracle.buf(bytes),packet);byte[] raw=new byte[bytes.readableBytes()];bytes.readBytes(raw);out.addProperty("payload",Base64.getEncoder().encodeToString(raw));}finally{bytes.release();}packets.add(out);
 }
 public static Object drop(Object player,Object stack,boolean around,boolean trace)throws Exception{
  JsonObject out=new JsonObject();out.addProperty("stack",wire(stack));out.addProperty("around",around);out.addProperty("trace_item",trace);
  Object inventory=call(player,"fY",new Class[]{}),menu=field(C("cmx"),"cd").get(player);int selected=C("cmw").getField("k").getInt(inventory);
  out.addProperty("inventory_selected",wire(call(inventory,"f",new Class[]{})));
  out.addProperty("remote_selected",wire(((List<?>)field(C("cpu"),"o").get(menu)).get(36+selected)));
  attempts.add(out);return (boolean)call(stack,"e",new Class[]{})?null:marker;
 }
 static Object[] player(JsonObject query)throws Exception{
  creative=!query.has("creative")||query.get("creative").getAsBoolean();container_oracle.creative=creative;features=query.has("features_enabled")&&!query.get("features_enabled").getAsBoolean()?call(C("cpl"),"a",new Class[]{}):C("cpn").getField("f").get(null);
  Object player=container_oracle.unsafe(playerClass),inventory=C("cmw").getConstructor(C("cmx")).newInstance(player),listener=container_oracle.unsafe(listenerClass);
  set(player,"cmx","h",inventory);set(player,"aqv","e",container_oracle.unsafe(gameModeClass));set(player,"aqv","c",listener);set(player,"bsr","s",C("exc").getConstructor(double.class,double.class,double.class).newInstance(0d,0d,0d));
  Object abilities=C("cmu").getConstructor().newInstance();C("cmu").getField("d").setBoolean(abilities,creative);set(player,"cmx","cA",abilities);
  set(listener,"aru","f",player);set(listener,"arr","d",server);field(C("aru"),"p").setInt(listener,-1);
  Object menu=container_oracle.menuClass.getConstructor(C("cmw"),boolean.class,C("cmx")).newInstance(inventory,true,player);set(player,"cmx","cc",menu);set(player,"cmx","cd",menu);
  Constructor<?> constructor=C("aqv$1").getDeclaredConstructor(C("aqv"));constructor.setAccessible(true);set(menu,"cpu","x",constructor.newInstance(player));
  C("cmw").getField("k").setInt(inventory,query.has("selected")?query.get("selected").getAsInt():0);
  currentPlayer=player;return new Object[]{player,inventory,menu,listener};
 }
 static JsonObject snapshot(Object menu)throws Exception{
  JsonObject out=new JsonObject();JsonArray slots=new JsonArray(),remote=new JsonArray(),drops=new JsonArray();
  for(Object slot:(List<?>)C("cpu").getField("i").get(menu))slots.add(wire(call(slot,"g",new Class[]{})));
  for(Object stack:(List<?>)field(C("cpu"),"o").get(menu))remote.add(wire(stack));
  for(JsonElement entry:attempts)if(!entry.getAsJsonObject().get("stack").getAsString().equals("AA=="))drops.add(entry.getAsJsonObject().get("stack"));
  out.add("slots",slots);out.add("remote_slots",remote);out.addProperty("carried",wire(call(menu,"g",new Class[]{})));out.addProperty("remote_carried",wire(field(C("cpu"),"q").get(menu)));out.addProperty("state_id",field(C("cpu"),"r").getInt(menu));out.add("attempts",attempts.deepCopy());out.add("drops",drops);out.add("packets",packets.deepCopy());return out;
 }
 static JsonObject selected(JsonObject query)throws Exception{
  Object[] host=player(query);Object menu=host[2];List<?> slots=(List<?>)C("cpu").getField("i").get(menu);
  for(int i=0;i<46;i++){call(slots.get(i),"f",new Class[]{C("cuq")},container_oracle.parse(query.getAsJsonArray("slots").get(i)));call(menu,"a",new Class[]{int.class,C("cuq")},i,container_oracle.parse(query.getAsJsonArray("remote_slots").get(i)));}
  call(menu,"b",new Class[]{C("cuq")},container_oracle.parse(query.get("carried")));call(menu,"a",new Class[]{C("cuq")},container_oracle.parse(query.get("remote_carried")));field(C("cpu"),"r").setInt(menu,query.get("initial_state_id").getAsInt());packets=new JsonArray();attempts=new JsonArray();
  JsonObject out=new JsonObject();out.add("initial",snapshot(menu));JsonArray states=new JsonArray();
  for(JsonElement op:query.getAsJsonArray("operations")){
   packets=new JsonArray();attempts=new JsonArray();JsonObject state=new JsonObject();state.addProperty("returned",(boolean)call(host[0],"a",new Class[]{boolean.class},op.getAsJsonObject().get("all").getAsBoolean()));state.add("before_broadcast",snapshot(menu));
   if(op.getAsJsonObject().get("broadcast").getAsBoolean())call(menu,"d",new Class[]{});state.add("after_broadcast",snapshot(menu));states.add(state);
  }
  out.add("states",states);return out;
 }
 static JsonObject creative(JsonObject query)throws Exception{
  Object[] host=player(query);Object listener=host[3];field(C("aru"),"r").setInt(listener,query.get("spam").getAsInt());JsonArray states=new JsonArray(),inputs=new JsonArray();
  for(JsonElement item:query.getAsJsonArray("operations")){
   JsonObject op=item.getAsJsonObject();packets=new JsonArray();attempts=new JsonArray();
   if(op.get("op").getAsString().equals("tick")){call(listener,"d",new Class[]{});inputs.add(JsonNull.INSTANCE);}
   else {Object stack=container_oracle.parse(op.get("stack"));JsonObject decoded=new JsonObject();decoded.addProperty("wire",wire(stack));decoded.addProperty("enabled",(boolean)call(stack,"a",new Class[]{C("cpl")},features));inputs.add(decoded);Object packet=C("ahy").getConstructor(int.class,C("cuq")).newInstance(op.get("slot").getAsInt(),stack);call(listener,"a",new Class[]{C("ahy")},packet);}
   JsonObject state=new JsonObject();state.add("attempts",attempts.deepCopy());state.add("packets",packets.deepCopy());state.addProperty("spam",field(C("aru"),"r").getInt(listener));states.add(state);
  }
  JsonObject out=new JsonObject();out.add("inputs",inputs);out.add("states",states);return out;
 }
 public static void main(String[]arguments)throws Exception{
  init();BufferedReader input=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=input.readLine())!=null;){JsonObject query=JsonParser.parseString(line).getAsJsonObject(),out=new JsonObject();out.add("name",query.get("name"));try{out.add("expected",query.has("mode")&&query.get("mode").getAsString().equals("creative")?creative(query):selected(query));}catch(Throwable error){while(error instanceof InvocationTargetException&&error.getCause()!=null)error=error.getCause();out.addProperty("error",error.toString());error.printStackTrace(System.err);}OUT.println("ORACLE:"+JSON.toJson(out));}
 }
}
