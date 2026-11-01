package oracle;
import com.google.gson.*;
import java.io.*;
import java.lang.reflect.*;
import java.util.*;
import jdk.internal.org.objectweb.asm.*;
import static jdk.internal.org.objectweb.asm.Opcodes.*;

/** Calls original Entity fluid pushing and original FluidState height/current.
 * Only oracle world/actor callback subclasses are generated; Minecraft classes
 * and methods are unchanged. Vanilla fluid tags are bound from shipped data. */
public final class fluid_physics_oracle {
 static final Gson JSON=new GsonBuilder().serializeNulls().create();static final PrintStream OUT=System.out;
 static Class<?> C(String n)throws Exception{return Class.forName(n);}
 static Class<?>[] T(Class<?>...t){return t;}
 static Object call(Object o,String n,Class<?>[]t,Object...v)throws Exception {Class<?> c=o instanceof Class<?>?(Class<?>)o:o.getClass();try{Method m=c.getMethod(n,t);m.setAccessible(true);return m.invoke(o instanceof Class<?>?null:o,v);}catch(NoSuchMethodException e){return survival_oracle.call(o,n,t,v);}}
 static void set(Object o,String c,String f,Object v)throws Exception{survival_oracle.set(o,c,f,v);}
 static Object field(String c,String f)throws Exception {Field q=C(c).getDeclaredField(f);q.setAccessible(true);return q.get(null);}
 static double d(JsonElement e){return Double.longBitsToDouble(Long.parseUnsignedLong(e.getAsString(),16));}
 static String bits(double v){return String.format("%016x",Double.doubleToLongBits(v));}
 static Object vec(JsonArray a)throws Exception{return C("exc").getConstructor(double.class,double.class,double.class).newInstance(d(a.get(0)),d(a.get(1)),d(a.get(2)));}
 static Object box(JsonArray a)throws Exception{return C("ewx").getConstructor(double.class,double.class,double.class,double.class,double.class,double.class).newInstance(d(a.get(0)),d(a.get(1)),d(a.get(2)),d(a.get(3)),d(a.get(4)),d(a.get(5)));}
 static Object pos(int x,int y,int z)throws Exception{return C("jd").getConstructor(int.class,int.class,int.class).newInstance(x,y,z);}
 static JsonArray vector(Object v)throws Exception {JsonArray r=new JsonArray();for(String n:new String[]{"c","d","e"})r.add(bits(C("exc").getField(n).getDouble(v)));return r;}
 static Class<?> WORLD,ENTITY,PLAYER;static Object unsafe;static Method allocate;static Object world,water,lava;static List<String> events=new ArrayList<>();
 static Object alloc(Class<?> c)throws Exception{return allocate.invoke(unsafe,c);}
 static String key(Object p)throws Exception{return call(p,"u",T())+","+call(p,"v",T())+","+call(p,"w",T());}
 static Object state(Object w,Object p)throws Exception {Map<?,?> map=(Map<?,?>)WORLD.getField("states").get(w);Integer id=(Integer)map.get(key(p));return call(C("dfy"),"a",T(int.class),id==null?0:id);}
 public static Object block(Object w,Object p)throws Exception {events.add("block:"+key(p));return state(w,p);}
 public static Object fluid(Object w,Object p)throws Exception {events.add("fluid:"+key(p));return call(state(w,p),"u",T());}
 public static boolean loaded(Object w,int a,int b,int c,int d)throws Exception {events.add("chunks:"+a+","+b+","+c+","+d);return WORLD.getField("loaded").getBoolean(w);}
 static void adapters()throws Exception {
  survival_oracle loader=new survival_oracle();String w="oracle/FluidPhysicsWorld";ClassWriter c=survival_oracle.begin(w,"dcw");survival_oracle.field(c,"states","Ljava/util/Map;");survival_oracle.field(c,"loaded","Z");
  for(String[] q:new String[][]{{"a_","block","dtc"},{"b_","fluid","epe"}}){MethodVisitor m=survival_oracle.method(c,q[0],"(Ljd;)L"+q[2]+";");m.visitVarInsn(ALOAD,0);m.visitVarInsn(ALOAD,1);m.visitMethodInsn(INVOKESTATIC,"oracle/fluid_physics_oracle",q[1],"(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",false);m.visitTypeInsn(CHECKCAST,q[2]);m.visitInsn(ARETURN);survival_oracle.end(m);}
  MethodVisitor m=survival_oracle.method(c,"b","(IIII)Z");m.visitVarInsn(ALOAD,0);for(int i=1;i<=4;i++)m.visitVarInsn(ILOAD,i);m.visitMethodInsn(INVOKESTATIC,"oracle/fluid_physics_oracle","loaded","(Ljava/lang/Object;IIII)Z",false);m.visitInsn(IRETURN);survival_oracle.end(m);m=survival_oracle.method(c,"c","(II)Ldcc;");m.visitVarInsn(ALOAD,0);m.visitInsn(ARETURN);survival_oracle.end(m);m=survival_oracle.method(c,"c_","(Ljd;)Ldua;");m.visitInsn(ACONST_NULL);m.visitInsn(ARETURN);survival_oracle.end(m);WORLD=survival_oracle.define(loader,w,c);
  for(int i=0;i<2;i++){String a=i==0?"oracle/FluidPhysicsEntity":"oracle/FluidPhysicsPlayer";c=survival_oracle.begin(a,i==0?"bsr":"aqv");survival_oracle.field(c,"pushed","Z");survival_oracle.getter(c,a,"cC","()Z","pushed","Z",IRETURN);Class<?> type=survival_oracle.define(loader,a,c);if(i==0)ENTITY=type;else PLAYER=type;}
  world=alloc(WORLD);
 }
 static void init()throws Exception {
  call(C("ab"),"a",T());call(C("akt"),"a",T());Class<?> u=C("sun.misc.Unsafe");Field f=u.getDeclaredField("theUnsafe");f.setAccessible(true);unsafe=f.get(null);allocate=u.getMethod("allocateInstance",Class.class);water=field("awk","a");lava=field("awk","b");
  // The exact shipped default tag files list water/flowing_water and lava/
  // flowing_lava. Binding uses the original Holder.Reference.bindTags method.
  Object registry=field("lt","c");for(Object fluid:(Iterable<?>)registry){String name=call(registry,"b",T(Object.class),fluid).toString();List<Object> tags=name.endsWith("water")?List.of(water):name.endsWith("lava")?List.of(lava):List.of();call(call(fluid,"k",T()),"a",T(Collection.class),tags);}
  adapters();OUT.println("SOURCE:"+C("bsr").getProtectionDomain().getCodeSource().getLocation());
 }
 static JsonObject query(JsonObject q)throws Exception {
  Map<String,Integer> states=new HashMap<>();for(JsonElement e:q.getAsJsonArray("blocks")){JsonArray b=e.getAsJsonArray();states.put(b.get(0).getAsInt()+","+b.get(1).getAsInt()+","+b.get(2).getAsInt(),b.get(3).getAsInt());}WORLD.getField("states").set(world,states);events.clear();JsonObject out=new JsonObject();String op=q.get("op").getAsString();
  if(op.equals("push")){
   WORLD.getField("loaded").setBoolean(world,q.get("loaded").getAsBoolean());Class<?> kind=q.get("player").getAsBoolean()?PLAYER:ENTITY;Object entity=alloc(kind);kind.getField("pushed").setBoolean(entity,q.get("pushed").getAsBoolean());set(entity,"bsr","r",world);set(entity,"bsr","aF",box(q.getAsJsonArray("bounds")));set(entity,"bsr","v",vec(q.getAsJsonArray("velocity")));Object map=C("it.unimi.dsi.fastutil.objects.Object2DoubleOpenHashMap").getConstructor().newInstance();Object tag=q.get("tag").getAsInt()==1?water:lava;call(map,"put",T(Object.class,double.class),tag,d(q.get("previous_height")));set(entity,"bsr","ak",map);
   boolean touching=(boolean)call(entity,"a",T(C("awu"),double.class),tag,d(q.get("factor")));out.addProperty("touching",touching);out.add("velocity",vector(call(entity,"dr",T())));out.add("height_written",q.get("loaded").getAsBoolean()?new JsonPrimitive(bits((double)call(map,"getDouble",T(Object.class),tag))):JsonNull.INSTANCE);out.addProperty("stored_height",bits((double)call(map,"getDouble",T(Object.class),tag)));
  }else{
   JsonArray p=q.getAsJsonArray("position");Object position=pos(p.get(0).getAsInt(),p.get(1).getAsInt(),p.get(2).getAsInt());Object fluid=call(world,"b_",T(C("jd")),position);
   if(op.equals("flow"))out.add("flow",vector(call(fluid,"c",T(C("dcc"),C("jd")),world,position)));
   else if(op.equals("height"))out.addProperty("height",String.format("%08x",Float.floatToIntBits((float)call(fluid,"a",T(C("dcc"),C("jd")),world,position))));
   else if(op.equals("face")){Object type=q.get("tag").getAsInt()==1?field("epf","c"):field("epf","e");out.addProperty("solid",(boolean)call(type,"a",T(C("dcc"),C("jd"),C("ji")),world,position,C("ji").getEnumConstants()[q.get("direction").getAsInt()]));}
   else throw new IllegalArgumentException(op);
  }
  out.add("events",JSON.toJsonTree(events));return out;
 }
 public static void main(String[]args)throws Exception {init();BufferedReader in=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=in.readLine())!=null;)try{OUT.println("ORACLE:"+JSON.toJson(query(JsonParser.parseString(line).getAsJsonObject())));}catch(Throwable e){OUT.println("QUERY:"+line);e.printStackTrace(OUT);throw e;}}
}
