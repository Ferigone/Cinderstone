package oracle;

import com.google.gson.*;
import com.mojang.serialization.Lifecycle;
import java.io.*;
import java.lang.invoke.MethodHandles;
import java.lang.reflect.*;
import java.util.*;
import java.util.concurrent.*;
import jdk.internal.org.objectweb.asm.*;
import static jdk.internal.org.objectweb.asm.Opcodes.*;

/** Real LevelChunk sections, original heightmap and original skylight engine.
 * World/LightChunkGetter supply owned data, never canned sky brightness. */
public final class rain_fixture_oracle {
 static final PrintStream OUT=System.out;static final Gson JSON=new GsonBuilder().serializeNulls().create();
 static Object unsafe,world,light,plains,plainsHolder,access;static Method allocate;
 static final Map<Long,Object> CHUNKS=new LinkedHashMap<>();static int notifications;
 static Class<?> C(String n)throws Exception{return Class.forName(n);}static Class<?>[] T(Class<?>...t){return t;}
 static Object call(Object o,String n,Class<?>[]types,Object...args)throws Exception{Class<?>c=o instanceof Class<?>?(Class<?>)o:o.getClass();Method m=null;try{m=c.getMethod(n,types);}catch(NoSuchMethodException e){for(Class<?>s=c;s!=null;s=s.getSuperclass())try{m=s.getDeclaredMethod(n,types);break;}catch(NoSuchMethodException ignored){}}if(m==null)throw new NoSuchMethodException(c+"."+n+Arrays.toString(types));m.setAccessible(true);return m.invoke(o instanceof Class<?>?null:o,args);}
 static Object get(Object o,String owner,String n)throws Exception{Field f=C(owner).getDeclaredField(n);f.setAccessible(true);return f.get(o);}static void set(Object o,String owner,String n,Object v)throws Exception{Field f=C(owner).getDeclaredField(n);f.setAccessible(true);f.set(o,v);}
 static long packed(int x,int z){return(x&0xffffffffL)|((long)z<<32);}static Object pos(int x,int y,int z)throws Exception{return C("jd").getConstructor(int.class,int.class,int.class).newInstance(x,y,z);}
 static String fb(float v){return String.format("%08x",Float.floatToRawIntBits(v));}static String db(double v){return String.format("%016x",Double.doubleToRawLongBits(v));}
 public static Object light(){return light;}public static Object biome(Object p){return plainsHolder;}
 public static Object chunk(int x,int z){Object c=CHUNKS.get(packed(x,z));if(c==null)throw new IllegalStateException("unprovided chunk "+x+","+z);return c;}
 public static boolean hasChunk(int x,int z){return CHUNKS.containsKey(packed(x,z));}
 static void method(ClassWriter c,String n,String desc,java.util.function.Consumer<MethodVisitor>b){MethodVisitor m=c.visitMethod(ACC_PUBLIC,n,desc,null,null);m.visitCode();b.accept(m);m.visitMaxs(0,0);m.visitEnd();}
 static void adapters()throws Exception{
  ClassWriter c=new ClassWriter(ClassWriter.COMPUTE_FRAMES|ClassWriter.COMPUTE_MAXS);c.visit(V21,ACC_PUBLIC,"RainFixtureWorld",null,"aqu",null);
  method(c,"I_","()I",m->{m.visitIntInsn(BIPUSH,-64);m.visitInsn(IRETURN);});method(c,"J_","()I",m->{m.visitIntInsn(SIPUSH,384);m.visitInsn(IRETURN);});method(c,"z_","()I",m->{m.visitIntInsn(BIPUSH,63);m.visitInsn(IRETURN);});
  method(c,"y_","()Leot;",m->{m.visitMethodInsn(INVOKESTATIC,"oracle/rain_fixture_oracle","light","()Ljava/lang/Object;",false);m.visitTypeInsn(CHECKCAST,"eot");m.visitInsn(ARETURN);});
  method(c,"t","(Ljd;)Ljm;",m->{m.visitVarInsn(ALOAD,1);m.visitMethodInsn(INVOKESTATIC,"oracle/rain_fixture_oracle","biome","(Ljava/lang/Object;)Ljava/lang/Object;",false);m.visitTypeInsn(CHECKCAST,"jm");m.visitInsn(ARETURN);});
  method(c,"b","(II)Z",m->{m.visitVarInsn(ILOAD,1);m.visitVarInsn(ILOAD,2);m.visitMethodInsn(INVOKESTATIC,"oracle/rain_fixture_oracle","hasChunk","(II)Z",false);m.visitInsn(IRETURN);});
  method(c,"a","(IILdvz;Z)Lduy;",m->{m.visitVarInsn(ILOAD,1);m.visitVarInsn(ILOAD,2);m.visitMethodInsn(INVOKESTATIC,"oracle/rain_fixture_oracle","chunk","(II)Ljava/lang/Object;",false);m.visitTypeInsn(CHECKCAST,"duy");m.visitInsn(ARETURN);});c.visitEnd();
  Class<?> type=MethodHandles.privateLookupIn(C("aqu"),MethodHandles.lookup()).defineClass(c.toByteArray());world=allocate.invoke(unsafe,type);
 }
 static void registries()throws Exception{
  Object lookup=call(C("pd"),"a",T()),key=C("lu").getField("aF").get(null),values=call(lookup,"b",T(C("akq")),key);
  Object plainsKey=C("ded").getField("b").get(null);plainsHolder=((Optional<?>)call(values,"a",T(C("akq")),plainsKey)).orElseThrow();plains=call(plainsHolder,"a",T());
  Object registry=C("ju").getConstructor(C("akq"),Lifecycle.class).newInstance(key,Lifecycle.stable());call(registry,"a",T(C("akq"),Object.class,C("jy")),plainsKey,plains,C("jy").getField("a").get(null));call(registry,"m",T());access=C("ka$c").getConstructor(List.class).newInstance(List.of(registry));
 }
 static int build()throws Exception{
  set(world,"dcw","I",access);set(world,"dcw","B",false);set(world,"dcw","d",false);
  Constructor<?> ctor=C("dvi").getConstructor(C("dcw"),C("dcd"));Method sections=C("duy").getMethod("d"),write=C("dvj").getMethod("a",int.class,int.class,int.class,C("dtc"));Object[]states=new Object[4];for(int i=0;i<4;i++)states[i]=call(C("dfy"),"a",T(int.class),new int[]{0,1,10,9}[i]);
  for(int cz=-1;cz<=1;cz++)for(int cx=-1;cx<=1;cx++){
   Object p=C("dcd").getConstructor(int.class,int.class).newInstance(cx,cz),chunk=ctor.newInstance(world,p);CHUNKS.put(packed(cx,cz),chunk);Object[]s=(Object[])sections.invoke(chunk);
   for(int y=-64;y<=63;y++){Object section=s[(y+64)>>4],state=states[y<=60?1:y<=62?2:3];for(int z=0;z<16;z++)for(int x=0;x<16;x++)write.invoke(section,x,y&15,z,state);}
   call(C("dyy"),"a",T(C("duy"),Set.class),chunk,Set.of(C("dyy$a").getField("e").get(null)));call(chunk,"A",T());
  }
  Object getter=Proxy.newProxyInstance(C("dvl").getClassLoader(),new Class<?>[]{C("dvl")},(p,m,a)->switch(m.getName()){
   case"q"->world;case"c"->CHUNKS.get(packed((Integer)a[0],(Integer)a[1]));case"a"->{notifications++;yield null;}case"toString"->"RainFixtureLightChunkGetter";default->throw new UnsupportedOperationException(m.toString());});
  light=C("eot").getConstructor(C("dvl"),boolean.class,boolean.class).newInstance(getter,false,true);
  for(Object chunk:CHUNKS.values()){
   Object p=call(chunk,"f",T());Object[]s=(Object[])sections.invoke(chunk);int cx=(int)get(p,"dcd","e"),cz=(int)get(p,"dcd","f");
   for(int i=0;i<24;i++)call(light,"a",T(C("kf"),boolean.class),call(C("kf"),"a",T(int.class,int.class,int.class),cx,i-4,cz),(boolean)call(s[i],"c",T()));
   call(light,"a",T(C("dcd"),boolean.class),p,true);call(light,"b",T(C("dcd")),p);
  }
  int passes=0;while((boolean)call(light,"K_",T())){if(++passes>32)throw new AssertionError("light not settled");call(light,"a",T());}return passes;
 }
 static JsonObject run()throws Exception{
  JsonObject out=new JsonObject();out.addProperty("minecraft","1.21");out.addProperty("original_classes_modified",false);JsonObject sources=new JsonObject();for(String n:new String[]{"dcw","dvi","dyy","eoo","eot","eox","ddw"})sources.addProperty(n,C(n).getProtectionDomain().getCodeSource().getLocation().toString());out.add("sources",sources);
  Object cache=((ThreadLocal<?>)get(plains,"ddw","m")).get();out.addProperty("plains_initial_temperature_cache_size",((Map<?,?>)cache).size());if(!((Map<?,?>)cache).isEmpty())throw new AssertionError("unexpected plains prehistory");
  Object climate=get(plains,"ddw","i");JsonObject cf=new JsonObject();cf.addProperty("has_precipitation",(boolean)call(plains,"c",T()));cf.addProperty("base_temperature_bits",fb((float)call(plains,"g",T())));cf.addProperty("modifier",get(climate,"ddw$b","d").toString());cf.addProperty("downfall_bits",fb((float)get(climate,"ddw$b","e")));out.add("plains",cf);
  Object noise=get(null,"ddw","f");JsonObject nf=new JsonObject();nf.addProperty("value_factor_bits",db((double)get(noise,"eoj","b")));nf.addProperty("input_factor_bits",db((double)get(noise,"eoj","c")));nf.addProperty("sqrt3_bits",db((double)get(null,"eok","e")));nf.addProperty("f2_bits",db((double)get(null,"eok","f")));nf.addProperty("g2_bits",db((double)get(null,"eok","g")));Object[]levels=(Object[])get(noise,"eoj","a");JsonArray na=new JsonArray();for(Object level:levels){JsonObject q=new JsonObject();q.add("permutation",JSON.toJsonTree(get(level,"eok","h")));q.addProperty("xo_bits",db((double)get(level,"eok","b")));q.addProperty("yo_bits",db((double)get(level,"eok","c")));na.add(q);}nf.add("levels",na);nf.add("gradients",JSON.toJsonTree(get(null,"eok","a")));out.add("temperature_noise",nf);
  out.addProperty("light_settle_passes",build());out.addProperty("light_pending",(boolean)call(light,"K_",T()));out.addProperty("light_update_notifications",notifications);out.addProperty("chunk_count",CHUNKS.size());out.add("base_state_ids",JSON.toJsonTree(new int[]{0,1,10,9}));
  Object center=CHUNKS.get(packed(0,0)),motion=C("dyy$a").getField("e").get(null),sky=C("ddf").getField("a").get(null);JsonArray heights=new JsonArray(),columns=new JsonArray(),rows=new JsonArray();Map<String,Integer>dedup=new LinkedHashMap<>();
  for(int z=0;z<16;z++)for(int x=0;x<16;x++){
   JsonArray h=new JsonArray();h.add(x);h.add(z);h.add((int)call(center,"a",T(C("dyy$a"),int.class,int.class),motion,x,z));h.add((int)call(world,"a",T(C("dyy$a"),int.class,int.class),motion,x,z));h.add((int)call(call(center,"B",T()),"a",T(int.class,int.class),x,z));heights.add(h);
   JsonArray values=new JsonArray();for(int y=-80;y<=336;y++){Object p=pos(x,y,z);JsonArray v=new JsonArray();v.add(y);v.add((int)call(world,"a",T(C("ddf"),C("jd")),sky,p));v.add((boolean)call(world,"h",T(C("jd")),p));values.add(v);}String key=values.toString();Integer id=dedup.get(key);if(id==null){id=rows.size();rows.add(values);dedup.put(key,id);}JsonArray column=new JsonArray();column.add(x);column.add(z);column.add(id);columns.add(column);
  }out.add("height_columns_x_z_highest_levelheight_lowestskysource",heights);out.add("sky_columns_x_z_row",columns);out.add("sky_rows_y_brightness_canSeeSky",rows);
  JsonArray precipitation=new JsonArray();for(int[]xz:new int[][]{{0,0},{15,15},{-1,-1},{16,-16},{29999999,-29999999},{-29999999,29999999}})for(int y=64;y<=320;y++){Object p=pos(xz[0],y,xz[1]);JsonArray row=new JsonArray();row.add(xz[0]);row.add(y);row.add(xz[1]);row.add(fb((float)call(plains,"f",T(C("jd")),p)));row.add(call(plains,"a",T(C("jd")),p).toString());precipitation.add(row);}out.add("plains_temperature_precipitation_probes",precipitation);out.addProperty("plains_final_temperature_cache_size",((Map<?,?>)cache).size());
  JsonArray rain=new JsonArray();for(int bits:new int[]{0,0x3e4ccccc,0x3e4ccccd,0x3e4cccce,0x3f800000})for(int y:new int[]{-80,-64,0,60,61,62,63,64,65,80,81,319,320,321,336}){float v=Float.intBitsToFloat(bits);set(world,"dcw","v",v);set(world,"dcw","w",v);JsonArray r=new JsonArray();r.add(fb(v));r.add(y);r.add((boolean)call(world,"r",T(C("jd")),pos(8,y,8)));rain.add(r);}out.add("actual_level_rain_cases",rain);out.addProperty("world_rng_after",Long.toUnsignedString(((java.util.concurrent.atomic.AtomicLong)get(get(world,"dcw","z"),"dyz","h")).get()));return out;
 }
 public static void main(String[]args)throws Exception{call(C("ab"),"a",T());call(C("akt"),"a",T());Class<?>u=C("sun.misc.Unsafe");Field f=u.getDeclaredField("theUnsafe");f.setAccessible(true);unsafe=f.get(null);allocate=u.getMethod("allocateInstance",Class.class);registries();adapters();Object random=C("dyz").getConstructor(long.class).newInstance(7121003L);set(world,"dcw","z",random);String before=Long.toUnsignedString(((java.util.concurrent.atomic.AtomicLong)get(random,"dyz","h")).get());JsonObject result=run();result.addProperty("world_rng_before",before);OUT.println("ORACLE:"+JSON.toJson(result));}
}
