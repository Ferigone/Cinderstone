package oracle;
import com.google.gson.*;import jdk.internal.org.objectweb.asm.*;import java.io.*;import java.nio.file.*;import java.util.*;
/** Actual InventoryMenu.removed/clearContainer/placeItemBackInInventory.
 * Environmental Player.isAlive is fixed true; disconnect/death not covered. */
public final class container_close_oracle {
 static Class<?> cls(String n)throws Exception{return Class.forName(n);}
 @SuppressWarnings("unchecked")
 public static void main(String[] args)throws Exception {
  Path tmp=Files.createTempFile("mc121-close-bootstrap",".json");try{container_oracle.main(new String[]{"equipment",tmp.toString()});}finally{Files.delete(tmp);}
  ClassWriter w=new ClassWriter(0);w.visit(Opcodes.V21,Opcodes.ACC_PUBLIC,"OracleAliveContainerPlayer",null,"aqv",null);
  MethodVisitor m=w.visitMethod(Opcodes.ACC_PUBLIC,"bE","()Z",null,null);m.visitCode();m.visitInsn(Opcodes.ICONST_1);m.visitInsn(Opcodes.IRETURN);m.visitMaxs(1,1);m.visitEnd();
  for(String name:new String[]{"w","R_"}){m=w.visitMethod(Opcodes.ACC_PUBLIC,name,"()Z",null,null);m.visitCode();m.visitInsn(Opcodes.ICONST_0);m.visitInsn(Opcodes.IRETURN);m.visitMaxs(1,1);m.visitEnd();}
  container_oracle.returnStatic(w,"f","()Z","oracle/container_oracle","creative","Z",Opcodes.IRETURN);
  m=w.visitMethod(Opcodes.ACC_PUBLIC,"dO","()Ldcw;",null,null);m.visitCode();m.visitFieldInsn(Opcodes.GETSTATIC,"oracle/container_oracle","world","Ljava/lang/Object;");m.visitTypeInsn(Opcodes.CHECKCAST,"dcw");m.visitInsn(Opcodes.ARETURN);m.visitMaxs(1,1);m.visitEnd();
  container_oracle.noOp(w,"a","(Lbsy;Lcuq;Lcuq;)V",4);
  for(String desc:new String[]{"(Lcuq;Z)Lcjh;","(Lcuq;ZZ)Lcjh;"}){m=w.visitMethod(Opcodes.ACC_PUBLIC,"a",desc,null,new String[]{"java/lang/Exception"});m.visitCode();m.visitVarInsn(Opcodes.ALOAD,1);m.visitVarInsn(Opcodes.ILOAD,2);if(desc.contains("ZZ"))m.visitVarInsn(Opcodes.ILOAD,3);m.visitMethodInsn(Opcodes.INVOKESTATIC,"oracle/container_oracle","recordDrop",desc.contains("ZZ")?"(Ljava/lang/Object;ZZ)V":"(Ljava/lang/Object;Z)V",false);m.visitInsn(Opcodes.ACONST_NULL);m.visitInsn(Opcodes.ARETURN);m.visitMaxs(3,4);m.visitEnd();}w.visitEnd();
  Class<?> alive=container_oracle.define("OracleAliveContainerPlayer",w.toByteArray());
  w=new ClassWriter(0);w.visit(Opcodes.V21,Opcodes.ACC_PUBLIC,"OracleCloseInventory",null,"cmw",null);
  m=w.visitMethod(Opcodes.ACC_PUBLIC,"<init>","(Lcmx;)V",null,null);m.visitCode();m.visitVarInsn(Opcodes.ALOAD,0);m.visitVarInsn(Opcodes.ALOAD,1);m.visitMethodInsn(Opcodes.INVOKESPECIAL,"cmw","<init>","(Lcmx;)V",false);m.visitInsn(Opcodes.RETURN);m.visitMaxs(2,2);m.visitEnd();
  m=w.visitMethod(Opcodes.ACC_PUBLIC,"g","(Lcuq;)V",null,null);m.visitCode();m.visitVarInsn(Opcodes.ALOAD,0);m.visitVarInsn(Opcodes.ALOAD,1);m.visitInsn(Opcodes.ICONST_0);m.visitMethodInsn(Opcodes.INVOKESPECIAL,"cmw","a","(Lcuq;Z)V",false);m.visitInsn(Opcodes.RETURN);m.visitMaxs(3,2);m.visitEnd();w.visitEnd();Class<?> inventoryClass=container_oracle.define("OracleCloseInventory",w.toByteArray());
  BufferedReader input=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=input.readLine())!=null;){JsonObject q=JsonParser.parseString(line).getAsJsonObject(),out=new JsonObject();out.add("name",q.get("name"));try{
   container_oracle.creative=q.get("creative").getAsBoolean();Object p=container_oracle.unsafe(alive),inventory=inventoryClass.getConstructor(cls("cmx")).newInstance(p);container_oracle.set(p,cls("cmx"),"h",inventory);Object abilities=cls("cmu").getConstructor().newInstance();cls("cmu").getField("d").setBoolean(abilities,container_oracle.creative);container_oracle.set(p,cls("cmx"),"cA",abilities);cls("cmw").getField("k").setInt(inventory,q.get("selected").getAsInt());
   Object current=container_oracle.menuClass.getConstructor(cls("cmw"),boolean.class,cls("cmx")).newInstance(inventory,true,p);container_oracle.set(p,cls("cmx"),"cc",current);container_oracle.set(p,cls("cmx"),"cd",current);container_oracle.resetDrops();List<?> slots=(List<?>)cls("cpu").getField("i").get(current);JsonArray initial=q.getAsJsonArray("slots");for(int i=0;i<46;i++)cls("crq").getMethod("f",cls("cuq")).invoke(slots.get(i),container_oracle.parse(initial.get(i)));cls("cpu").getMethod("b",cls("cuq")).invoke(current,container_oracle.parse(q.get("carried")));
   cls("cpu").getMethod("a",cls("cmx")).invoke(current,p);JsonArray states=new JsonArray();states.add(container_oracle.snapshot(current));out.add("states",states);
  }catch(Throwable e){Throwable cause=e instanceof java.lang.reflect.InvocationTargetException?e.getCause():e;out.addProperty("error",cause.toString());}System.out.println("ORACLE:"+container_oracle.GSON.toJson(out));}
 }
}
