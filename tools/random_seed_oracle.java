package oracle;
import com.google.gson.*;
import java.io.*;import java.lang.instrument.*;import java.lang.reflect.*;import java.security.ProtectionDomain;import java.util.*;import java.util.concurrent.atomic.AtomicLong;
import jdk.internal.org.objectweb.asm.*;
import static jdk.internal.org.objectweb.asm.Opcodes.*;

/** Original RandomSupport bytecode with only its nanoTime boundary replaced
 * and returned seeds observed. Original arithmetic/factories/RNGs execute. */
public final class random_seed_oracle {
 static final PrintStream OUT=System.out;static final Gson JSON=new Gson();
 static AtomicLong counter;static long[] times;static int cursor;static JsonArray clocks,seeds;
 static int clockReplacements,returnObservers,transformed;
 public static void premain(String ignored,Instrumentation instrumentation) {
  instrumentation.addTransformer(new ClassFileTransformer(){
   @Override public byte[] transform(ClassLoader loader,String name,Class<?> redef,ProtectionDomain domain,byte[] input) {
    if(!name.equals("dzn"))return null;
    ClassReader reader=new ClassReader(input);ClassWriter writer=new ClassWriter(reader,ClassWriter.COMPUTE_MAXS);
    reader.accept(new ClassVisitor(ASM8,writer){
     @Override public MethodVisitor visitMethod(int access,String method,String desc,String signature,String[] exceptions){
      MethodVisitor target=super.visitMethod(access,method,desc,signature,exceptions);
      if(!method.equals("a")||!desc.equals("()J"))return target;
      return new MethodVisitor(ASM8,target){
       @Override public void visitMethodInsn(int op,String owner,String method,String desc,boolean itf){
        if(op==INVOKESTATIC&&owner.equals("java/lang/System")&&method.equals("nanoTime")&&desc.equals("()J")){clockReplacements++;super.visitMethodInsn(op,"oracle/random_seed_oracle","nanoTime",desc,false);}else super.visitMethodInsn(op,owner,method,desc,itf);
       }
       @Override public void visitInsn(int opcode){
        if(opcode==LRETURN){returnObservers++;super.visitInsn(DUP2);super.visitMethodInsn(INVOKESTATIC,"oracle/random_seed_oracle","generated","(J)V",false);}
        super.visitInsn(opcode);
       }
      };
     }
    },0);transformed++;return writer.toByteArray();
   }
  });
 }
 public static long nanoTime(){
  if(cursor>=times.length)throw new AssertionError("unexpected unique seed request");long sample=times[cursor++];JsonObject event=new JsonObject();event.addProperty("updated_uniquifier_bits",String.format("%016x",counter.get()));event.addProperty("nano_time",sample);clocks.add(event);return sample;
 }
 public static void generated(long value){seeds.add(value);}
 static Class<?> C(String name)throws Exception{return Class.forName(name);}
 static Object call(Object object,String method,Class<?>[]types,Object...args)throws Exception {return C("ayw").getMethod(method,types).invoke(object,args);}
 static JsonObject snapshot(Object random)throws Exception {
  JsonObject out=new JsonObject();out.addProperty("class",random.getClass().getName());out.addProperty("int",(int)call(random,"f",new Class<?>[]{}));out.addProperty("long",(long)call(random,"g",new Class<?>[]{}));out.addProperty("float_bits",String.format("%08x",Float.floatToRawIntBits((float)call(random,"i",new Class<?>[]{}))));out.addProperty("double_bits",String.format("%016x",Double.doubleToRawLongBits((double)call(random,"j",new Class<?>[]{}))));Field state=random.getClass().getDeclaredField("h");state.setAccessible(true);out.addProperty("state48_bits",String.format("%016x",((AtomicLong)state.get(random)).get()));return out;
 }
 static JsonObject run(JsonObject q)throws Exception {
  counter.set(Long.parseUnsignedLong(q.get("initial_uniquifier_bits").getAsString(),16));times=new long[q.getAsJsonArray("nano_times").size()];for(int i=0;i<times.length;i++)times[i]=q.getAsJsonArray("nano_times").get(i).getAsLong();cursor=0;clocks=new JsonArray();seeds=new JsonArray();JsonArray operations=new JsonArray();
  for(JsonElement element:q.getAsJsonArray("operations")){
   String operation=element.getAsString();JsonObject out=new JsonObject();out.addProperty("operation",operation);
   switch(operation){
    case "unique" -> out.addProperty("seed",(long)C("dzn").getMethod("a").invoke(null));
    case "create" -> out.add("random",snapshot(C("ayw").getMethod("a").invoke(null)));
    case "thread_safe" -> out.add("random",snapshot(C("ayw").getMethod("b").invoke(null)));
    case "level" -> {
     int start=seeds.size();Object temporary=C("ayw").getMethod("a").invoke(null);out.addProperty("rand_value",(int)call(temporary,"f",new Class<?>[]{}));Object random=C("ayw").getMethod("a").invoke(null),threadSafe=C("ayw").getMethod("b").invoke(null);out.add("random",snapshot(random));out.add("thread_safe_random",snapshot(threadSafe));JsonArray initial=new JsonArray();for(int i=start;i<seeds.size();i++)initial.add(seeds.get(i));out.add("initial_seeds",initial);
    }
    default -> throw new IllegalArgumentException(operation);
   }operations.add(out);
  }
  if(cursor!=times.length)throw new AssertionError("unconsumed clock samples");JsonObject out=new JsonObject();out.add("operations",operations);out.add("clock_callbacks",clocks);out.add("observed_seeds",seeds);out.addProperty("final_uniquifier_bits",String.format("%016x",counter.get()));return out;
 }
 public static void main(String[]args)throws Exception {
  Field field=C("dzn").getDeclaredField("d");field.setAccessible(true);counter=(AtomicLong)field.get(null);
  if(clockReplacements!=1||returnObservers!=1||transformed!=1)throw new AssertionError("unexpected instrumentation boundary counts");
  OUT.println("SOURCE:"+C("dzn").getProtectionDomain().getCodeSource().getLocation());OUT.println("LIVE_NANOTIME:"+System.nanoTime());
  OUT.println("INSTRUMENTATION:"+clockReplacements+","+returnObservers+","+transformed);
  BufferedReader input=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=input.readLine())!=null;)OUT.println("ORACLE:"+JSON.toJson(run(JsonParser.parseString(line).getAsJsonObject())));
 }
}
