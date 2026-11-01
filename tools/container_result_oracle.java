package oracle;
import com.google.gson.*;
import com.mojang.serialization.*;
import jdk.internal.org.objectweb.asm.*;
import java.io.*;
import java.lang.reflect.*;
import java.nio.file.*;
import java.util.*;

/** Actual cpu/cqw/crm result-slot actions. Environmental recalculation calls
 * unchanged RecipeManager/Recipe. Awards/unlocks/network side effects omitted. */
public final class container_result_oracle {
 public static Object recipes;
 static Class<?> cls(String n)throws Exception{return Class.forName(n);}
 public static void refresh(Object menu)throws Exception {
  Object grid=cls("cqw").getMethod("r").invoke(menu);if(grid==null)return;
  Object positioned=cls("cql").getMethod("aF_").invoke(grid);Object input=cls("cyp$a").getMethod("a").invoke(positioned);
  Object type=cls("czf").getField("a").get(null);
  Optional<?> recipe=(Optional<?>)cls("czd").getMethod("a",cls("czf"),cls("czc"),cls("dcw")).invoke(recipes,type,input,container_oracle.world);
  Object result=container_oracle.empty;
  if(recipe.isPresent()) {Object value=cls("czb").getMethod("b").invoke(recipe.get());result=cls("cyz").getMethod("a",cls("czc"),cls("jo$a")).invoke(value,input,container_oracle.lookup);}
  Object slot=cls("cpu").getMethod("b",int.class).invoke(menu,0);cls("crq").getMethod("f",cls("cuq")).invoke(slot,result);
 }
 static Class<?> menuClass;
 static void stubs()throws Exception {
  ClassWriter w=new ClassWriter(0);w.visit(Opcodes.V21,Opcodes.ACC_PUBLIC,"OracleResultLevel",null,"dcw",null);
  container_oracle.returnStatic(w,"J","()Lcpl;","cpn","f","Lcpl;",Opcodes.ARETURN);
  container_oracle.returnStatic(w,"H_","()Lka;","ka","b","Lka$b;",Opcodes.ARETURN);
  MethodVisitor m=w.visitMethod(Opcodes.ACC_PUBLIC,"r","()Lczd;",null,null);m.visitCode();m.visitFieldInsn(Opcodes.GETSTATIC,"oracle/container_result_oracle","recipes","Ljava/lang/Object;");m.visitTypeInsn(Opcodes.CHECKCAST,"czd");m.visitInsn(Opcodes.ARETURN);m.visitMaxs(1,1);m.visitEnd();w.visitEnd();
  container_oracle.world=container_oracle.unsafe(container_oracle.define("OracleResultLevel",w.toByteArray()));
  w=new ClassWriter(0);w.visit(Opcodes.V21,Opcodes.ACC_PUBLIC,"OracleResultMenu",null,"cqw",null);
  m=w.visitMethod(Opcodes.ACC_PUBLIC,"<init>","(Lcmw;ZLcmx;)V",null,null);m.visitCode();m.visitVarInsn(Opcodes.ALOAD,0);m.visitVarInsn(Opcodes.ALOAD,1);m.visitVarInsn(Opcodes.ILOAD,2);m.visitVarInsn(Opcodes.ALOAD,3);m.visitMethodInsn(Opcodes.INVOKESPECIAL,"cqw","<init>","(Lcmw;ZLcmx;)V",false);m.visitInsn(Opcodes.RETURN);m.visitMaxs(4,4);m.visitEnd();
  m=w.visitMethod(Opcodes.ACC_PUBLIC,"a","(Lbqk;)V",null,new String[]{"java/lang/Exception"});m.visitCode();m.visitVarInsn(Opcodes.ALOAD,0);m.visitMethodInsn(Opcodes.INVOKESTATIC,"oracle/container_result_oracle","refresh","(Ljava/lang/Object;)V",false);m.visitInsn(Opcodes.RETURN);m.visitMaxs(1,2);m.visitEnd();w.visitEnd();menuClass=container_oracle.define("OracleResultMenu",w.toByteArray());
 }
 @SuppressWarnings("unchecked")
 public static void main(String[] args)throws Exception {
  Path temporary=Files.createTempFile("mc121-equipment-bootstrap",".json");try {container_oracle.main(new String[]{"equipment",temporary.toString()});}finally{Files.delete(temporary);}
  stubs();Codec<Object> recipeCodec=(Codec<Object>)cls("cyz").getField("h").get(null);Method click=cls("cpu").getDeclaredMethod("b",int.class,int.class,cls("cqe"),cls("cmx"));click.setAccessible(true);
  BufferedReader input=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=input.readLine())!=null;){JsonObject q=JsonParser.parseString(line).getAsJsonObject();JsonObject out=new JsonObject();out.add("name",q.get("name"));
   try {
    Object recipe=recipeCodec.parse(container_oracle.ops,q.get("recipe")).getOrThrow();Object name=cls("akr").getMethod("a",String.class).invoke(null,q.get("recipe_id").getAsString());Object holder=cls("czb").getConstructor(cls("akr"),cls("cyz")).newInstance(name,recipe);
    recipes=cls("czd").getConstructor(cls("jo$a")).newInstance(container_oracle.lookup);cls("czd").getMethod("a",Iterable.class).invoke(recipes,List.of(holder));
    Object player=container_oracle.newPlayer(q.get("creative").getAsBoolean());Object inventory=cls("cmx").getMethod("fY").invoke(player);Object current=menuClass.getConstructor(cls("cmw"),boolean.class,cls("cmx")).newInstance(inventory,true,player);container_oracle.set(player,cls("cmx"),"cc",current);container_oracle.set(player,cls("cmx"),"cd",current);container_oracle.resetDrops();
    List<?> slots=(List<?>)cls("cpu").getField("i").get(current);JsonArray initial=q.getAsJsonArray("slots");for(int i=1;i<Math.min(46,initial.size());i++)cls("crq").getMethod("f",cls("cuq")).invoke(slots.get(i),container_oracle.parse(initial.get(i)));cls("cpu").getMethod("b",cls("cuq")).invoke(current,container_oracle.parse(q.get("carried")));refresh(current);
    JsonArray states=new JsonArray();for(JsonElement value:q.getAsJsonArray("clicks")){JsonObject c=value.getAsJsonObject();Object kind=Enum.valueOf((Class)cls("cqe"),c.get("type").getAsString());click.invoke(current,c.get("slot").getAsInt(),c.get("button").getAsInt(),kind,player);states.add(container_oracle.snapshot(current));}out.add("states",states);
   }catch(Throwable error){Throwable cause=error instanceof InvocationTargetException?error.getCause():error;out.addProperty("error",cause.toString());}
   System.out.println("ORACLE:"+container_oracle.GSON.toJson(out));
  }
 }
}
