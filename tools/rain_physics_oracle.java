package oracle;

import com.google.gson.*;
import java.io.*;
import java.lang.reflect.*;
import java.util.*;
import jdk.internal.org.objectweb.asm.*;
import static jdk.internal.org.objectweb.asm.Opcodes.*;

/** Unchanged original Level.isRainingAt, BlockAndTintGetter.canSeeSky and
 * actual ItemEntity's inherited private Entity.isInRain. Lower world queries
 * supply controlled brightness/height and actual registered biome holders.
 * No original class bytecode is rewritten and no Entity method is overridden.
 */
public final class rain_physics_oracle {
 static final PrintStream OUT=System.out;
 static final Gson JSON=new GsonBuilder().serializeNulls().create();
 static JsonObject query;static JsonArray trace;static Object world,entity,lookup;static Method allocate;static Object unsafe;static Class<?> WORLD;
 static final Map<String,Object> BIOMES=new LinkedHashMap<>();
 static Class<?> C(String n)throws Exception{return Class.forName(n);}static Class<?>[] T(Class<?>...t){return t;}
 static Object call(Object o,String n,Class<?>[]t,Object...a)throws Exception{Class<?>c=o instanceof Class<?>?(Class<?>)o:o.getClass();try{Method m=c.getMethod(n,t);m.setAccessible(true);return m.invoke(o instanceof Class<?>?null:o,a);}catch(NoSuchMethodException missing){return survival_oracle.call(o,n,t,a);}}
 static void set(Object o,String owner,String f,Object v)throws Exception{survival_oracle.set(o,owner,f,v);}
 static Object alloc(Class<?> c)throws Exception{return allocate.invoke(unsafe,c);}
 static float f(JsonElement e){return Float.intBitsToFloat(Integer.parseUnsignedInt(e.getAsString(),16));}
 static double d(JsonElement e){return Double.longBitsToDouble(Long.parseUnsignedLong(e.getAsString(),16));}
 static int[] xyz(Object p)throws Exception{return new int[]{(int)call(p,"u",T()),(int)call(p,"v",T()),(int)call(p,"w",T())};}
 static String key(int[]p){return p[0]+","+p[1]+","+p[2];}
 static JsonObject row(Object p)throws Exception{
  String key=key(xyz(p));if(query.has("cells")&&query.getAsJsonObject("cells").has(key))return query.getAsJsonObject("cells").getAsJsonObject(key);return query.getAsJsonObject("default");
 }
 static void fault(String phase)throws Exception{if(query.has("fault")&&query.get("fault").getAsString().equals(phase))throw new IllegalStateException("controlled/"+phase);}
 public static void rainLevel()throws Exception{trace.add("rain");fault("rain");}
 public static int sky(Object layer,Object p)throws Exception{trace.add("sky/"+key(xyz(p)));fault("sky");if(((Enum<?>)layer).ordinal()!=0)throw new AssertionError("notSKY");return row(p).get("sky").getAsInt();}
 public static int maximum()throws Exception{trace.add("max");fault("max");return query.has("max_light")?query.get("max_light").getAsInt():15;}
 public static int height(Object type,int x,int z)throws Exception{
  trace.add("height/"+((Enum<?>)type).ordinal()+"/"+x+","+z);fault("height");
  return query.has("heights")&&query.getAsJsonObject("heights").has(x+","+z)?query.getAsJsonObject("heights").get(x+","+z).getAsInt():query.get("height").getAsInt();
 }
 public static Object biome(Object p)throws Exception{trace.add("biome/"+key(xyz(p)));fault("biome");return BIOMES.get(row(p).get("biome").getAsString());}
 static Object position(JsonArray p)throws Exception{return C("jd").getConstructor(int.class,int.class,int.class).newInstance(p.get(0).getAsInt(),p.get(1).getAsInt(),p.get(2).getAsInt());}
 static void init()throws Exception{
  call(C("ab"),"a",T());call(C("akt"),"a",T());Class<?> u=C("sun.misc.Unsafe");Field field=u.getDeclaredField("theUnsafe");field.setAccessible(true);unsafe=field.get(null);allocate=u.getMethod("allocateInstance",Class.class);
  lookup=call(call(C("pd"),"a",T()),"b",T(C("akq")),C("lu").getField("aF").get(null));
  for(String[] entry:new String[][]{{"plains","b"},{"snowy_plains","d"},{"desert","f"}})BIOMES.put(entry[0],((Optional<?>)call(lookup,"a",T(C("akq")),C("ded").getField(entry[1]).get(null))).orElseThrow());
  String n="oracle/RainQueryWorld";ClassWriter c=survival_oracle.begin(n,"dcw");
  MethodVisitor m=survival_oracle.method(c,"d","(F)F");m.visitMethodInsn(INVOKESTATIC,"oracle/rain_physics_oracle","rainLevel","()V",false);m.visitVarInsn(ALOAD,0);m.visitVarInsn(FLOAD,1);m.visitMethodInsn(INVOKESPECIAL,"dcw","d","(F)F",false);m.visitInsn(FRETURN);survival_oracle.end(m);
  m=survival_oracle.method(c,"a","(Lddf;Ljd;)I");m.visitVarInsn(ALOAD,1);m.visitVarInsn(ALOAD,2);m.visitMethodInsn(INVOKESTATIC,"oracle/rain_physics_oracle","sky","(Ljava/lang/Object;Ljava/lang/Object;)I",false);m.visitInsn(IRETURN);survival_oracle.end(m);
  m=survival_oracle.method(c,"Q","()I");m.visitMethodInsn(INVOKESTATIC,"oracle/rain_physics_oracle","maximum","()I",false);m.visitInsn(IRETURN);survival_oracle.end(m);
  m=survival_oracle.method(c,"a","(Ldyy$a;II)I");m.visitVarInsn(ALOAD,1);m.visitVarInsn(ILOAD,2);m.visitVarInsn(ILOAD,3);m.visitMethodInsn(INVOKESTATIC,"oracle/rain_physics_oracle","height","(Ljava/lang/Object;II)I",false);m.visitInsn(IRETURN);survival_oracle.end(m);
  m=survival_oracle.method(c,"t","(Ljd;)Ljm;");m.visitVarInsn(ALOAD,1);m.visitMethodInsn(INVOKESTATIC,"oracle/rain_physics_oracle","biome","(Ljava/lang/Object;)Ljava/lang/Object;",false);m.visitTypeInsn(CHECKCAST,"jm");m.visitInsn(ARETURN);survival_oracle.end(m);
  WORLD=survival_oracle.define(new survival_oracle(),n,c);world=alloc(WORLD);entity=alloc(C("cjh"));set(entity,"bsr","r",world);
  OUT.println("SOURCE:"+C("bsr").getProtectionDomain().getCodeSource().getLocation());
  OUT.println("SOURCE:"+C("dcw").getProtectionDomain().getCodeSource().getLocation());
 }
 static Throwable root(Throwable t){while(t.getCause()!=null)t=t.getCause();return t;}
 static JsonObject run(JsonObject q)throws Exception{
  query=q.deepCopy();query.remove("fault");trace=new JsonArray();set(world,"dcw","v",f(q.get("old_rain")));set(world,"dcw","w",f(q.get("rain")));
  // These are ordinary pure original getters; clear instrumentation before the
  // actual tested call. Precipitation facts are produced by registered Biomes.
  JsonObject actualCells=new JsonObject();List<JsonArray> points=new ArrayList<>();points.add(q.getAsJsonArray("position"));
  if(q.get("op").getAsString().equals("entity")){
   JsonArray top=q.getAsJsonArray("position").deepCopy();Object v=C("jd").getMethod("a",double.class,double.class,double.class).invoke(null,(double)top.get(0).getAsInt(),d(q.get("max_y")),(double)top.get(2).getAsInt());top=JSON.toJsonTree(xyz(v)).getAsJsonArray();points.add(top);
  }
  JsonObject facts=new JsonObject();facts.addProperty("raining",(boolean)call(world,"ad",T()));
  for(JsonArray p:points){Object point=position(p);JsonObject cell=row(point).deepCopy();Object b=call(BIOMES.get(cell.get("biome").getAsString()),"a",T());cell.addProperty("precipitation",((Enum<?>)call(b,"a",T(C("jd")),point)).ordinal());actualCells.add(key(xyz(point)),cell);}facts.add("cells",actualCells);trace=new JsonArray();query=q;
  JsonObject result=new JsonObject();
  try{
   boolean answer;if(q.get("op").getAsString().equals("entity")){
    set(entity,"bsr","t",position(q.getAsJsonArray("position")));double top=d(q.get("max_y"));set(entity,"bsr","aF",C("ewx").getConstructor(double.class,double.class,double.class,double.class,double.class,double.class).newInstance(991.125,top-.25,-995.25,991.375,top,-995.0));answer=(boolean)survival_oracle.original("bsr",entity,"w",T());
   }else answer=(boolean)call(world,"r",T(C("jd")),position(q.getAsJsonArray("position")));
   result.addProperty("accepted",true);result.addProperty("result",answer);
  }catch(Throwable e){Throwable t=root(e);result.addProperty("accepted",false);result.addProperty("error_class",t.getClass().getName());result.addProperty("error_message",t.getMessage());}
  result.add("trace",trace);result.add("world_facts",facts);JsonObject out=new JsonObject();out.add("query",q);out.add("expected",result);return out;
 }
 public static void main(String[]args)throws Exception{init();try(BufferedReader in=new BufferedReader(new InputStreamReader(System.in))){for(String line;(line=in.readLine())!=null;)OUT.println("ORACLE:"+JSON.toJson(run(JsonParser.parseString(line).getAsJsonObject())));}}
}
