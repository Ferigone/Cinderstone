package oracle;
import com.google.gson.*;
import java.io.*;
import java.lang.reflect.*;
import java.nio.file.*;
import java.util.*;
import jdk.internal.org.objectweb.asm.*;
import static jdk.internal.org.objectweb.asm.Opcodes.*;

/** Original signed Minecraft1.21 BlockCollisions and border methods.
 * ASM creates only oracle callback subclasses; no Minecraft class is rewritten. */
public class runtime_collision_oracle {
 static final Gson JSON=new Gson();static final PrintStream OUT=System.out;
 static Class<?> C(String n)throws Exception{return Class.forName(n);}
 static Class<?>[] T(Class<?>...t){return t;}
 static Object call(Object o,String n,Class<?>[]t,Object...v)throws Exception{Class<?> c=o instanceof Class<?>?(Class<?>)o:o.getClass();try{Method m=c.getMethod(n,t);m.setAccessible(true);return m.invoke(o instanceof Class<?>?null:o,v);}catch(NoSuchMethodException e){return survival_oracle.call(o,n,t,v);}}
 static void set(Object o,String c,String f,Object v)throws Exception{survival_oracle.set(o,c,f,v);}
 static Object state(int id)throws Exception{return call(C("dfy"),"a",T(int.class),id);}
 static Object pos(int x,int y,int z)throws Exception{return C("jd").getConstructor(int.class,int.class,int.class).newInstance(x,y,z);}
 static String bits(double v){return String.format("%016x",Double.doubleToLongBits(v));}
 static Object[] AXES;static Field GRID;static Class<?> WORLD,ACTOR;static Object world,actor,unsafe;static Method allocate;
 static Map<String,Integer> ids=new LinkedHashMap<>();static List<Object> grids=new ArrayList<>();
 static Object alloc(Class<?> c)throws Exception{return allocate.invoke(unsafe,c);}
 static Object field(String c,String f)throws Exception {Field q=C(c).getDeclaredField(f);q.setAccessible(true);return q.get(null);}
 static JsonObject shape(Object s)throws Exception {
  JsonObject out=new JsonObject();JsonArray coords=new JsonArray();int[] dim=new int[3];
  for(int axis=0;axis<3;axis++){List<?> list=(List<?>)call(s,"a",T(C("ji$a")),AXES[axis]);JsonArray a=new JsonArray();for(Object d:list)a.add(bits(((Number)d).doubleValue()));coords.add(a);dim[axis]=list.size()-1;}
  Object grid=GRID.get(s);StringBuilder occupied=new StringBuilder();for(int x=0;x<dim[0];x++)for(int y=0;y<dim[1];y++)for(int z=0;z<dim[2];z++)occupied.append((boolean)call(grid,"b",T(int.class,int.class,int.class),x,y,z)?'1':'0');
  out.add("coordinates",coords);out.addProperty("occupied",occupied.toString());out.addProperty("canonical_block",s==call(C("exs"),"b",T()));return out;
 }
 static int intern(Object s)throws Exception {JsonObject grid=shape(s);String key=JSON.toJson(grid);Integer id=ids.get(key);if(id==null){id=grids.size();ids.put(key,id);grids.add(grid);}return id;}
 static void init()throws Exception {call(C("ab"),"a",T());call(C("akt"),"a",T());AXES=C("ji$a").getEnumConstants();GRID=C("exv").getDeclaredField("a");GRID.setAccessible(true);Class<?> u=C("sun.misc.Unsafe");Field f=u.getDeclaredField("theUnsafe");f.setAccessible(true);unsafe=f.get(null);allocate=u.getMethod("allocateInstance",Class.class);}
 static void extract(Path path)throws Exception {
  Object empty=field("dcl","a"),zero=pos(0,0,0);Field cache=C("dtb$a").getDeclaredField("a");cache.setAccessible(true);List<Object> rows=new ArrayList<>();
  for(int id=0;id<26684;id++){
   Object s=state(id),b=call(s,"b",T()),shape=call(s,"k",T(C("dcc"),C("jd")),empty,zero);boolean dynamic=(boolean)call(b,"p",T()),offset=(boolean)call(s,"q",T()),uncached=cache.get(s)==null;int flags=(dynamic?1:0)|(offset?2:0)|(uncached?4:0);String desc=s.toString(),name=desc.substring(desc.indexOf("minecraft:"),desc.indexOf('}'));int kind=flags==0?0:name.equals("minecraft:bamboo")?2:name.equals("minecraft:pointed_dripstone")?3:name.equals("minecraft:scaffolding")?4:name.equals("minecraft:powder_snow")?5:name.endsWith("shulker_box")?6:name.equals("minecraft:moving_piston")?7:1;
   float horizontal=(float)call(b,"av_",T()),vertical=(float)call(b,"ax_",T());Object off=call(s,"n",T(C("dcc"),C("jd")),empty,zero);double ox=(double)call(off,"a",T()),oy=(double)call(off,"b",T()),oz=(double)call(off,"c",T());
   if(kind==2||kind==3)shape=call(shape,"a",T(double.class,double.class,double.class),-ox,-oy,-oz);
   int detail=0;if(kind==4)detail=(int)call(s,"c",T(C("duf")),field("dmo","c"))*2+((boolean)call(s,"c",T(C("duf")),field("dmo","e"))?1:0);
   rows.add(Arrays.asList(intern(shape),kind,detail,horizontal,vertical,oy!=0.,(boolean)call(s,"i",T())));
  }
  Map<String,Integer> special=new LinkedHashMap<>();for(String[] q:new String[][]{{"scaffolding_stable","dmo","g"},{"scaffolding_bottom","dmo","i"},{"powder_falling","dlt","f"}})special.put(q[0],intern(field(q[1],q[2])));special.put("empty",intern(call(C("exs"),"a",T())));special.put("full",intern(call(C("exs"),"b",T())));
  Map<String,Object> out=new LinkedHashMap<>();out.put("minecraft","1.21");out.put("oracle_inner_sha256","8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01");out.put("states",rows);out.put("shapes",grids);out.put("special",special);Files.writeString(path,JSON.toJson(out)+"\n");OUT.println("EXTRACT:"+rows.size()+" states,"+grids.size()+" fullvoxelshapes");
 }
 public static Object block(Object w,Object p)throws Exception {Map<?,?> map=(Map<?,?>)WORLD.getField("states").get(w);String key=call(p,"u",T())+","+call(p,"v",T())+","+call(p,"w",T());Integer id=(Integer)map.get(key);return state(id==null?0:id);}
 public static List<Object> entities(Object w,Object excluded,Object bounds,Object filter)throws Exception {List<Object> out=new ArrayList<>();for(Object e:(List<?>)WORLD.getField("entities").get(w))if(e!=excluded&&(boolean)call(call(e,"cK",T()),"c",T(C("ewx")),bounds)&&((java.util.function.Predicate<Object>)filter).test(e))out.add(e);return out;}
 static void adapters()throws Exception {
  survival_oracle loader=new survival_oracle();String w="oracle/RuntimeCollisionWorld";ClassWriter cw=survival_oracle.begin(w,"dcw");survival_oracle.field(cw,"states","Ljava/util/Map;");MethodVisitor m=survival_oracle.method(cw,"a_","(Ljd;)Ldtc;");m.visitVarInsn(ALOAD,0);m.visitVarInsn(ALOAD,1);m.visitMethodInsn(INVOKESTATIC,"oracle/runtime_collision_oracle","block","(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",false);m.visitTypeInsn(CHECKCAST,"dtc");m.visitInsn(ARETURN);survival_oracle.end(m);
  m=survival_oracle.method(cw,"c","(II)Ldcc;");m.visitVarInsn(ALOAD,0);m.visitInsn(ARETURN);survival_oracle.end(m);m=survival_oracle.method(cw,"c_","(Ljd;)Ldua;");m.visitInsn(ACONST_NULL);m.visitInsn(ARETURN);survival_oracle.end(m);
  survival_oracle.field(cw,"entities","Ljava/util/List;");m=survival_oracle.method(cw,"a","(Lbsr;Lewx;Ljava/util/function/Predicate;)Ljava/util/List;");for(int i=0;i<4;i++)m.visitVarInsn(ALOAD,i);m.visitMethodInsn(INVOKESTATIC,"oracle/runtime_collision_oracle","entities","(Ljava/lang/Object;Ljava/lang/Object;Ljava/lang/Object;Ljava/lang/Object;)Ljava/util/List;",false);m.visitInsn(ARETURN);survival_oracle.end(m);WORLD=survival_oracle.define(loader,w,cw);
  String a="oracle/RuntimeCollisionActor";cw=survival_oracle.begin(a,"bsr");for(String n:new String[]{"descending","spectator","collidable"})survival_oracle.field(cw,n,"Z");survival_oracle.getter(cw,a,"ca","()Z","descending","Z",IRETURN);survival_oracle.getter(cw,a,"R_","()Z","spectator","Z",IRETURN);survival_oracle.getter(cw,a,"bG","()Z","collidable","Z",IRETURN);ACTOR=survival_oracle.define(loader,a,cw);world=alloc(WORLD);actor=alloc(ACTOR);set(actor,"bsr","n",field("bsx","ag"));
 }
 static JsonArray boxes(Object s)throws Exception {JsonArray out=new JsonArray();for(Object b:(List<?>)call(s,"e",T())){JsonArray a=new JsonArray();for(String f:new String[]{"a","b","c","d","e","f"})a.add(bits(C("ewx").getField(f).getDouble(b)));out.add(a);}return out;}
 static double d(JsonElement e){return Double.longBitsToDouble(Long.parseUnsignedLong(e.getAsString(),16));}
 static Object box(JsonArray a)throws Exception{return C("ewx").getConstructor(double.class,double.class,double.class,double.class,double.class,double.class).newInstance(d(a.get(0)),d(a.get(1)),d(a.get(2)),d(a.get(3)),d(a.get(4)),d(a.get(5)));}
 static JsonElement query(JsonObject q)throws Exception {
  String op=q.get("op").getAsString();JsonArray out=new JsonArray();
  if(op.equals("border")){Object border=C("dut").getConstructor().newInstance();JsonArray b=q.getAsJsonArray("border");call(border,"c",T(double.class,double.class),d(b.get(0)),d(b.get(1)));call(border,"a",T(double.class),d(b.get(2)));JsonArray p=q.getAsJsonArray("position");set(actor,"bsr","s",C("exc").getConstructor(double.class,double.class,double.class).newInstance(d(p.get(0)),d(p.get(1)),d(p.get(2))));JsonObject result=new JsonObject();boolean active=(boolean)call(border,"a",T(C("bsr"),C("ewx")),actor,box(q.getAsJsonArray("bounds")));result.addProperty("active",active);JsonArray bounds=new JsonArray();for(String n:new String[]{"e","f","g","h"})bounds.add(bits((double)call(border,n,T())));result.add("bounds",bounds);result.add("shape",shape(call(border,"c",T())));return result;}
  if(op.equals("entities")){List<Object> entities=new ArrayList<>();for(JsonElement row:q.getAsJsonArray("entities")){JsonObject r=row.getAsJsonObject();Object e=r.get("id").getAsInt()==1?actor:alloc(ACTOR);ACTOR.getField("collidable").setBoolean(e,r.get("collidable").getAsBoolean());ACTOR.getField("spectator").setBoolean(e,r.get("spectator").getAsBoolean());set(e,"bsr","aF",box(r.getAsJsonArray("bounds")));entities.add(e);}WORLD.getField("entities").set(world,entities);List<?> result=(List<?>)call(world,"c",T(C("bsr"),C("ewx")),actor,box(q.getAsJsonArray("bounds")));for(Object s:result)out.add(shape(s));return out;}
  Map<String,Integer> states=new HashMap<>();for(JsonElement row:q.getAsJsonArray("blocks")){JsonArray r=row.getAsJsonArray();states.put(r.get(0).getAsInt()+","+r.get(1).getAsInt()+","+r.get(2).getAsInt(),r.get(3).getAsInt());}WORLD.getField("states").set(world,states);JsonArray p=q.getAsJsonArray("position");set(actor,"bsr","s",C("exc").getConstructor(double.class,double.class,double.class).newInstance(d(p.get(0)),d(p.get(1)),d(p.get(2))));ACTOR.getField("descending").setBoolean(actor,q.get("descending").getAsBoolean());set(actor,"bsr","ac",Float.intBitsToFloat(Integer.parseUnsignedInt(q.get("fall").getAsString(),16)));
  if(op.equals("air")){java.util.stream.Stream<?> statesStream=(java.util.stream.Stream<?>)call(world,"a",T(C("ewx")),box(q.getAsJsonArray("bounds")));for(Object s:statesStream.toList())if(!(boolean)call(s,"i",T()))return new JsonPrimitive(false);return new JsonPrimitive(true);}
  Iterable<?> iterable=(Iterable<?>)call(world,"e",T(C("bsr"),C("ewx")),actor,box(q.getAsJsonArray("bounds")));for(Object s:iterable){JsonObject v=shape(s);v.add("boxes",boxes(s));out.add(v);}return out;
 }
 public static void main(String[]args)throws Exception {init();if(args.length>0&&args[0].equals("extract")){extract(Path.of(args[1]));return;}adapters();BufferedReader in=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=in.readLine())!=null;)try{OUT.println("ORACLE:"+JSON.toJson(query(JsonParser.parseString(line).getAsJsonObject())));}catch(Throwable e){OUT.println("QUERY:"+line);e.printStackTrace(OUT);throw e;}}
}
