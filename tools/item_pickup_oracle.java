package oracle;
import com.google.gson.*;
import java.io.*;import java.lang.reflect.*;import java.nio.file.*;import java.util.*;
import io.netty.buffer.*;
import jdk.internal.org.objectweb.asm.*;
import static jdk.internal.org.objectweb.asm.Opcodes.*;

/** Original ItemEntity.playerTouch + original Inventory.add. Only outer player
 * collection/stat/onPickup boundaries and level removal are observed. */
public final class item_pickup_oracle {
 static final PrintStream OUT=System.out;static final Gson JSON=new GsonBuilder().serializeNulls().create();
 static Object current,access;public static Object world;public static boolean creative;static JsonArray events;static Class<?> PLAYER,INVENTORY;
 static Class<?> C(String n)throws Exception{return Class.forName(n);}
 static Object call(Object o,String n,Class<?>[]t,Object...v)throws Exception {try{return o.getClass().getMethod(n,t).invoke(o,v);}catch(NoSuchMethodException e){return survival_oracle.call(o,n,t,v);}}
 static String wire(Object stack)throws Exception {ByteBuf b=Unpooled.buffer();try{Object buf=C("wk").getConstructor(ByteBuf.class,C("ka")).newInstance(b,access);C("yx").getMethod("encode",Object.class,Object.class).invoke(C("cuq").getField("h").get(null),buf,stack);byte[]v=new byte[b.readableBytes()];b.getBytes(b.readerIndex(),v);return Base64.getEncoder().encodeToString(v);}finally{b.release();}}
 static JsonObject state()throws Exception {
  Object s=call(current,"p",new Class<?>[]{});JsonObject r=new JsonObject();r.addProperty("count",(int)call(s,"H",new Class<?>[]{}));r.addProperty("raw_count",(int)item_hurt_oracle.get(s,"cuq","o"));
  r.addProperty("raw_item_id",(int)C("jz").getMethod("a",Object.class).invoke(C("lt").getField("g").get(null),item_hurt_oracle.get(s,"cuq","q")));
  r.addProperty("empty",(boolean)call(s,"e",new Class<?>[]{}));r.addProperty("removed",(boolean)call(current,"dJ",new Class<?>[]{}));r.addProperty("stack_wire",wire(s));return r;
 }
 static JsonObject event(String name)throws Exception {JsonObject r=state();r.addProperty("callback",name);events.add(r);return r;}
 public static void inventory(boolean accepted)throws Exception {event("inventory_add").addProperty("accepted",accepted);}
 public static void collect(int count)throws Exception {event("collect").addProperty("argument_count",count);}
 public static void stat(Object stat,int count)throws Exception {JsonObject r=event("stat");r.addProperty("argument_count",count);Object value=call(stat,"b",new Class<?>[]{});r.addProperty("stat_item_id",(int)C("jz").getMethod("a",Object.class).invoke(C("lt").getField("g").get(null),value));}
 public static void after()throws Exception {event("on_pickup");}
 static Class<?> define(String name,ClassWriter w){w.visitEnd();return container_oracle.define(name,w.toByteArray());}
 static void adapters()throws Exception {
  ClassWriter w=new ClassWriter(0);w.visit(V21,ACC_PUBLIC,"OraclePickupInventory",null,"cmw",null);
  MethodVisitor m=w.visitMethod(ACC_PUBLIC,"<init>","(Lcmx;)V",null,null);m.visitCode();m.visitVarInsn(ALOAD,0);m.visitVarInsn(ALOAD,1);m.visitMethodInsn(INVOKESPECIAL,"cmw","<init>","(Lcmx;)V",false);m.visitInsn(RETURN);m.visitMaxs(2,2);m.visitEnd();
  m=w.visitMethod(ACC_PUBLIC,"f","(Lcuq;)Z",null,null);m.visitCode();m.visitVarInsn(ALOAD,0);m.visitVarInsn(ALOAD,1);m.visitMethodInsn(INVOKESPECIAL,"cmw","f","(Lcuq;)Z",false);m.visitVarInsn(ISTORE,2);m.visitVarInsn(ILOAD,2);m.visitMethodInsn(INVOKESTATIC,"oracle/item_pickup_oracle","inventory","(Z)V",false);m.visitVarInsn(ILOAD,2);m.visitInsn(IRETURN);m.visitMaxs(2,3);m.visitEnd();INVENTORY=define("OraclePickupInventory",w);
  w=new ClassWriter(0);w.visit(V21,ACC_PUBLIC,"OraclePickupPlayer",null,"cmx",null);
  container_oracle.returnStatic(w,"f","()Z","oracle/item_pickup_oracle","creative","Z",IRETURN);
  m=w.visitMethod(ACC_PUBLIC,"R_","()Z",null,null);m.visitCode();m.visitInsn(ICONST_0);m.visitInsn(IRETURN);m.visitMaxs(1,1);m.visitEnd();
  m=w.visitMethod(ACC_PUBLIC,"dO","()Ldcw;",null,null);m.visitCode();m.visitFieldInsn(GETSTATIC,"oracle/item_pickup_oracle","world","Ljava/lang/Object;");m.visitTypeInsn(CHECKCAST,"dcw");m.visitInsn(ARETURN);m.visitMaxs(1,1);m.visitEnd();
  container_oracle.noOp(w,"a","(Lbsy;Lcuq;Lcuq;)V",4);
  m=w.visitMethod(ACC_PUBLIC,"a","(Lbsr;I)V",null,null);m.visitCode();m.visitVarInsn(ILOAD,2);m.visitMethodInsn(INVOKESTATIC,"oracle/item_pickup_oracle","collect","(I)V",false);m.visitInsn(RETURN);m.visitMaxs(1,3);m.visitEnd();
  m=w.visitMethod(ACC_PUBLIC,"a","(Lavw;I)V",null,null);m.visitCode();m.visitVarInsn(ALOAD,1);m.visitVarInsn(ILOAD,2);m.visitMethodInsn(INVOKESTATIC,"oracle/item_pickup_oracle","stat","(Ljava/lang/Object;I)V",false);m.visitInsn(RETURN);m.visitMaxs(2,3);m.visitEnd();
  m=w.visitMethod(ACC_PUBLIC,"a","(Lcjh;)V",null,null);m.visitCode();m.visitMethodInsn(INVOKESTATIC,"oracle/item_pickup_oracle","after","()V",false);m.visitInsn(RETURN);m.visitMaxs(0,2);m.visitEnd();PLAYER=define("OraclePickupPlayer",w);
 }
 static JsonObject run(JsonObject q)throws Exception {
  creative=q.get("creative").getAsBoolean();world=container_oracle.world;container_oracle.set(world,C("dcw"),"B",q.get("client").getAsBoolean());
  Object player=container_oracle.unsafe(PLAYER),inventory=INVENTORY.getConstructor(C("cmx")).newInstance(player);container_oracle.set(player,C("cmx"),"h",inventory);
  Object abilities=C("cmu").getConstructor().newInstance();C("cmu").getField("d").setBoolean(abilities,creative);container_oracle.set(player,C("cmx"),"cA",abilities);
  C("bsr").getMethod("a_",UUID.class).invoke(player,UUID.fromString("00000000-0000-4000-8000-000000000001"));
  Object menu=container_oracle.menuClass.getConstructor(C("cmw"),boolean.class,C("cmx")).newInstance(inventory,true,player);container_oracle.set(player,C("cmx"),"cc",menu);container_oracle.set(player,C("cmx"),"cd",menu);
  C("cmw").getField("k").setInt(inventory,q.get("selected").getAsInt());List<?> slots=(List<?>)C("cpu").getField("i").get(menu);
  for(int i=0;i<46;i++)C("crq").getMethod("f",C("cuq")).invoke(slots.get(i),container_oracle.parse(q.getAsJsonArray("slots").get(i)));
  Object stack=container_oracle.parse(q.get("pickup"));current=C("cjh").getConstructor(C("dcw"),double.class,double.class,double.class,C("cuq"),double.class,double.class,double.class).newInstance(world,8.,64.,8.,stack,0.,0.,0.);
  container_oracle.set(current,C("cjh"),"j",q.get("pickup_delay").getAsInt());String target=q.get("target").getAsString();if(!target.equals("none"))C("cjh").getMethod("b",UUID.class).invoke(current,UUID.fromString(target.equals("match")?"00000000-0000-4000-8000-000000000001":"00000000-0000-4000-8000-000000000002"));
  if(q.get("removed").getAsBoolean())container_oracle.set(current,C("bsr"),"aH",C("bsr$c").getField("b").get(null));
  events=new JsonArray();Object callback=Proxy.newProxyInstance(C("dxh").getClassLoader(),new Class<?>[]{C("dxh")},(proxy,method,args)->{if(method.getName().equals("a")&&args!=null&&args.length==1)event("discard");return null;});C("bsr").getMethod("a",C("dxh")).invoke(current,callback);
  call(call(current,"ar",new Class<?>[]{}),"b",new Class<?>[]{});
  JsonObject out=new JsonObject();out.addProperty("pickup_wire",wire(stack));JsonArray before=new JsonArray();for(JsonElement s:q.getAsJsonArray("slots"))before.add(wire(container_oracle.parse(s)));out.add("slot_wires",before);
  C("cjh").getMethod("b_",C("cmx")).invoke(current,player);
  out.add("events",events);out.add("final",state());Object dirty=call(call(current,"ar",new Class<?>[]{}),"b",new Class<?>[]{});int mask=0;if(dirty!=null)for(Object v:(List<?>)dirty)mask|=1<<(int)call(v,"a",new Class<?>[]{});out.addProperty("metadata_dirty_mask",mask);
  JsonArray after=new JsonArray();for(Object s:slots)after.add(wire(C("crq").getMethod("g").invoke(s)));out.add("inventory_after",after);return out;
 }
 public static void main(String[]args)throws Exception {
  Path file=Files.createTempFile("pickup-bootstrap",".json");try{container_oracle.main(new String[]{"equipment",file.toString()});}finally{Files.delete(file);}adapters();access=C("ka").getMethod("a",C("jz")).invoke(null,C("lt").getField("aA").get(null));
  OUT.println("SOURCE:"+C("cjh").getProtectionDomain().getCodeSource().getLocation());OUT.println("SOURCE:"+C("cmw").getProtectionDomain().getCodeSource().getLocation());
  BufferedReader input=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=input.readLine())!=null;)OUT.println("ORACLE:"+JSON.toJson(run(JsonParser.parseString(line).getAsJsonObject())));
 }
}
