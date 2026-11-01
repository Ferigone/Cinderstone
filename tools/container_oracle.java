package oracle;

import com.google.gson.*;
import com.mojang.serialization.*;
import jdk.internal.org.objectweb.asm.*;
import java.io.*;
import java.lang.reflect.*;
import java.nio.file.*;
import java.util.*;

/** Calls unmodified Java1.21 InventoryMenu/AbstractContainerMenu click methods.
 * ASM stubs provide environmental hooks only: level features/registries, creative
 * flag, drop collection, and equipment sideeffects. Craft-result recalculation
 * is deliberately disabled; recipe-result consumption is checked separately.
 */
public final class container_oracle {
 static final Gson GSON=new GsonBuilder().serializeNulls().create();
 static Class<?> stack,player,menu,provider; static Object lookup,empty;
 static Codec<Object> stackCodec;static DynamicOps<JsonElement> ops;
 public static Object world;public static boolean creative;
 public static List<Object> dropped=new ArrayList<>();
 public static Map<Object,boolean[]> dropFlags=new IdentityHashMap<>();
 public static void resetDrops(){dropped=new ArrayList<>();dropFlags=new IdentityHashMap<>();}
 static Class<?> cls(String n)throws Exception{return Class.forName(n);}
 public static void recordDrop(Object item)throws Exception{recordDrop(item,false,false);}
 public static void recordDrop(Object item,boolean trace)throws Exception{recordDrop(item,false,trace);}
 public static void recordDrop(Object item,boolean around,boolean trace)throws Exception{if(!(boolean)stack.getMethod("e").invoke(item)){Object copy=stack.getMethod("s").invoke(item);dropped.add(copy);dropFlags.put(copy,new boolean[]{around,trace});}}
 static Object unsafe(Class<?> c)throws Exception{Field f=cls("sun.misc.Unsafe").getDeclaredField("theUnsafe");f.setAccessible(true);Object u=f.get(null);return cls("sun.misc.Unsafe").getMethod("allocateInstance",Class.class).invoke(u,c);}
 static Class<?> define(String name,byte[] data){return new ClassLoader(container_oracle.class.getClassLoader()){Class<?> build(){return defineClass(name,data,0,data.length);}}.build();}
 static void returnStatic(ClassWriter w,String name,String descriptor,String owner,String field,String type,int opcode){MethodVisitor m=w.visitMethod(Opcodes.ACC_PUBLIC,name,descriptor,null,null);m.visitCode();m.visitFieldInsn(Opcodes.GETSTATIC,owner,field,type);m.visitInsn(opcode);m.visitMaxs(1,1);m.visitEnd();}
 static void noOp(ClassWriter w,String name,String desc,int locals){MethodVisitor m=w.visitMethod(Opcodes.ACC_PUBLIC,name,desc,null,null);m.visitCode();m.visitInsn(Opcodes.RETURN);m.visitMaxs(0,locals);m.visitEnd();}
 static Class<?> playerClass,menuClass;
 static void stubs()throws Exception{
  ClassWriter w=new ClassWriter(0);w.visit(Opcodes.V21,Opcodes.ACC_PUBLIC,"OracleContainerLevel",null,"dcw",null);
  returnStatic(w,"J","()Lcpl;","cpn","f","Lcpl;",Opcodes.ARETURN);
  returnStatic(w,"H_","()Lka;","ka","b","Lka$b;",Opcodes.ARETURN);w.visitEnd();
  Class<?> levelClass=define("OracleContainerLevel",w.toByteArray());world=unsafe(levelClass);
  w=new ClassWriter(0);w.visit(Opcodes.V21,Opcodes.ACC_PUBLIC,"OracleContainerPlayer",null,"cmx",null);
  returnStatic(w,"f","()Z","oracle/container_oracle","creative","Z",Opcodes.IRETURN);
  MethodVisitor m=w.visitMethod(Opcodes.ACC_PUBLIC,"R_","()Z",null,null);m.visitCode();m.visitInsn(Opcodes.ICONST_0);m.visitInsn(Opcodes.IRETURN);m.visitMaxs(1,1);m.visitEnd();
  m=w.visitMethod(Opcodes.ACC_PUBLIC,"dO","()Ldcw;",null,null);m.visitCode();m.visitFieldInsn(Opcodes.GETSTATIC,"oracle/container_oracle","world","Ljava/lang/Object;");m.visitTypeInsn(Opcodes.CHECKCAST,"dcw");m.visitInsn(Opcodes.ARETURN);m.visitMaxs(1,1);m.visitEnd();
  noOp(w,"a","(Lbsy;Lcuq;Lcuq;)V",4);
  for(String descriptor:new String[]{"(Lcuq;Z)Lcjh;","(Lcuq;ZZ)Lcjh;"}){m=w.visitMethod(Opcodes.ACC_PUBLIC,"a",descriptor,null,new String[]{"java/lang/Exception"});m.visitCode();m.visitVarInsn(Opcodes.ALOAD,1);m.visitVarInsn(Opcodes.ILOAD,2);if(descriptor.contains("ZZ"))m.visitVarInsn(Opcodes.ILOAD,3);m.visitMethodInsn(Opcodes.INVOKESTATIC,"oracle/container_oracle","recordDrop",descriptor.contains("ZZ")?"(Ljava/lang/Object;ZZ)V":"(Ljava/lang/Object;Z)V",false);m.visitInsn(Opcodes.ACONST_NULL);m.visitInsn(Opcodes.ARETURN);m.visitMaxs(3,4);m.visitEnd();}
  w.visitEnd();playerClass=define("OracleContainerPlayer",w.toByteArray());
  w=new ClassWriter(0);w.visit(Opcodes.V21,Opcodes.ACC_PUBLIC,"OracleContainerMenu",null,"cqw",null);
  m=w.visitMethod(Opcodes.ACC_PUBLIC,"<init>","(Lcmw;ZLcmx;)V",null,null);m.visitCode();m.visitVarInsn(Opcodes.ALOAD,0);m.visitVarInsn(Opcodes.ALOAD,1);m.visitVarInsn(Opcodes.ILOAD,2);m.visitVarInsn(Opcodes.ALOAD,3);m.visitMethodInsn(Opcodes.INVOKESPECIAL,"cqw","<init>","(Lcmw;ZLcmx;)V",false);m.visitInsn(Opcodes.RETURN);m.visitMaxs(4,4);m.visitEnd();
  noOp(w,"a","(Lbqk;)V",2);w.visitEnd();menuClass=define("OracleContainerMenu",w.toByteArray());
 }
 static Object parse(JsonElement json){return json==null||json.isJsonNull()||json.getAsJsonObject().get("id").getAsString().equals("minecraft:air")?empty:stackCodec.parse(ops,json).getOrThrow();}
 static JsonElement encode(Object value)throws Exception{return (boolean)stack.getMethod("e").invoke(value)?JsonNull.INSTANCE:stackCodec.encodeStart(ops,value).getOrThrow();}
 static void set(Object object,Class<?> owner,String field,Object value)throws Exception{Field f=owner.getDeclaredField(field);f.setAccessible(true);f.set(object,value);}
 static Object newPlayer(boolean isCreative)throws Exception{
  creative=isCreative;Object p=unsafe(playerClass);Object inventory=cls("cmw").getConstructor(player).newInstance(p);set(p,player,"h",inventory);Object abilities=cls("cmu").getConstructor().newInstance();cls("cmu").getField("d").setBoolean(abilities,isCreative);set(p,player,"cA",abilities);return p;
 }
 static JsonObject snapshot(Object current)throws Exception{
  JsonObject out=new JsonObject();JsonArray slots=new JsonArray();for(Object slot:(Iterable<?>)menu.getField("i").get(current))slots.add(encode(cls("crq").getMethod("g").invoke(slot)));out.add("slots",slots);out.add("carried",encode(menu.getMethod("g").invoke(current)));JsonArray drops=new JsonArray();for(Object item:dropped)drops.add(encode(item));out.add("drops",drops);JsonArray flags=new JsonArray();for(Object item:dropped){boolean[] bits=dropFlags.get(item);JsonArray pair=new JsonArray();pair.add(bits[0]);pair.add(bits[1]);flags.add(pair);}out.add("drop_flags",flags);return out;
 }
 @SuppressWarnings("unchecked")
 public static void main(String[] args)throws Exception{
  cls("ab").getMethod("a").invoke(null);cls("akt").getMethod("a").invoke(null);stack=cls("cuq");player=cls("cmx");menu=cls("cpu");provider=cls("jo$a");empty=stack.getField("l").get(null);lookup=cls("pd").getMethod("a").invoke(null);ops=(DynamicOps<JsonElement>)provider.getMethod("a",DynamicOps.class).invoke(lookup,JsonOps.INSTANCE);stackCodec=(Codec<Object>)stack.getField("b").get(null);stubs();
  if(args.length>0&&args[0].equals("equipment")){
   Object registry=cls("lt").getField("g").get(null),p=newPlayer(false);JsonObject result=new JsonObject();for(Object item:(Iterable<?>)registry){Object name=cls("jz").getMethod("b",Object.class).invoke(registry,item);JsonObject s=new JsonObject();s.addProperty("id",name.toString());s.addProperty("count",1);Object slot=cls("btn").getMethod("h",stack).invoke(p,parse(s));JsonObject row=new JsonObject();row.addProperty("name",name.toString());row.addProperty("equipment_slot",((Enum<?>)slot).name());result.add(cls("jz").getMethod("a",Object.class).invoke(registry,item).toString(),row);}Files.writeString(Path.of(args[1]),GSON.toJson(result)+"\n");System.out.println("ORACLE_EQUIPMENT:"+result.size());return;
  }
  Method click=menu.getDeclaredMethod("b",int.class,int.class,cls("cqe"),player);click.setAccessible(true);
  BufferedReader input=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=input.readLine())!=null;){JsonObject q=JsonParser.parseString(line).getAsJsonObject();JsonObject result=new JsonObject();result.add("name",q.get("name"));
   try{Object p=newPlayer(q.has("creative")&&q.get("creative").getAsBoolean());Object inventory=player.getMethod("fY").invoke(p);Object current=menuClass.getConstructor(cls("cmw"),boolean.class,player).newInstance(inventory,true,p);set(p,player,"cc",current);set(p,player,"cd",current);resetDrops();
    List<?> slots=(List<?>)menu.getField("i").get(current);JsonArray initial=q.getAsJsonArray("slots");for(int i=0;i<Math.min(46,initial.size());i++)cls("crq").getMethod("f",stack).invoke(slots.get(i),parse(initial.get(i)));menu.getMethod("b",stack).invoke(current,parse(q.get("carried")));
    JsonArray states=new JsonArray();for(JsonElement value:q.getAsJsonArray("clicks")){JsonObject c=value.getAsJsonObject();Object kind=Enum.valueOf((Class)cls("cqe"),c.get("type").getAsString());click.invoke(current,c.get("slot").getAsInt(),c.get("button").getAsInt(),kind,p);states.add(snapshot(current));}result.add("states",states);
   }catch(Throwable error){Throwable cause=error instanceof InvocationTargetException?error.getCause():error;result.addProperty("error",cause.toString());}
   System.out.println("ORACLE:"+GSON.toJson(result));
  }
 }
}
