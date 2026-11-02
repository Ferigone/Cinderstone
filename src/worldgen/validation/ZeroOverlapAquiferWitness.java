package aquiferoracle;
import com.google.gson.Gson;
import java.lang.reflect.*;
import java.nio.file.*;
import java.security.*;
import java.util.*;
import java.util.stream.Stream;

/** Constructor observations only; no target field writes, server or world. */
public final class ZeroOverlapAquiferWitness {
  static final java.io.PrintStream OUT=System.out;
  static final String INNER="8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01";
  static final long SEED=123456789L;
  static Object getters,settingsRegistry,biomeRegistry;
  static Class<?> c(String name)throws Exception{return Class.forName(name);}
  static Object field(Object value,Class<?> owner,String name)throws Exception {
    Field f=owner.getDeclaredField(name);f.setAccessible(true);return f.get(value);
  }
  static Object key(Object registry,String name)throws Exception {
    return c("akq").getMethod("a",c("akq"),c("akr")).invoke(null,registry,c("akr").getMethod("a",String.class).invoke(null,name));
  }
  static Object holder(Object registry,String name)throws Exception {
    Object g=c("jn$a").getMethod("b",c("akq")).invoke(getters,registry);
    return c("jn").getMethod("b",c("akq")).invoke(g,key(registry,name));
  }
  static Object value(Object holder)throws Exception{return c("jm").getMethod("a").invoke(holder);}
  static String sha(Path p)throws Exception{return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(Files.readAllBytes(p)));}
  static Throwable cause(Throwable t){while(t instanceof InvocationTargetException&&t.getCause()!=null)t=t.getCause();return t;}
  static void setup()throws Exception {
    c("ab").getMethod("a").invoke(null);c("akt").getMethod("a").invoke(null);
    Path loaded=Path.of(c("dzc").getProtectionDomain().getCodeSource().getLocation().toURI());
    if(!sha(loaded).equals(INNER))throw new AssertionError("wrong actually loaded NoiseChunk artifact");
    Object lookup=c("pd").getMethod("a").invoke(null);
    getters=c("jo$a").getMethod("b").invoke(lookup);
    settingsRegistry=c("lu").getField("aO").get(null);
    Object biomeKey=c("lu").getField("aF").get(null);
    biomeRegistry=c("ju").getConstructor(c("akq"),c("com.mojang.serialization.Lifecycle"))
      .newInstance(biomeKey,c("com.mojang.serialization.Lifecycle").getMethod("stable").invoke(null));
    Object biomeLookup=c("jo$a").getMethod("b",c("akq")).invoke(lookup,biomeKey);
    Method register=c("ju").getMethod("a",c("akq"),Object.class,c("jy"));
    Object info=c("jy").getField("a").get(null);
    try(Stream<?> entries=(Stream<?>)c("jo").getMethod("b").invoke(biomeLookup)) {
      for(Object h:entries.toList())register.invoke(biomeRegistry,c("jm$c").getMethod("h").invoke(h),value(h),info);
    }
    c("jz").getMethod("m").invoke(biomeRegistry);
  }
  static Object settings(String name)throws Exception{return value(holder(settingsRegistry,name));}
  static Object state(String name)throws Exception {
    return c("dzm").getMethod("a",c("jn$a"),c("akq"),long.class).invoke(null,getters,key(settingsRegistry,name),SEED);
  }
  static Object picker(Object settings)throws Exception {
    Method m=c("dzb").getDeclaredMethod("a",c("dzd"));m.setAccessible(true);return m.invoke(null,settings);
  }
  static Object beard()throws Exception {
    Field f=c("dys$b").getField("a");f.setAccessible(true);return f.get(null);
  }
  static Map<String,Object> aquifer(Object aq)throws Exception {
    Map<String,Object> result=new LinkedHashMap<>();result.put("class",aq.getClass().getName());
    result.put("schedule_fluid_update",c("dyk").getMethod("a").invoke(aq));
    if(c("dyk$c").isInstance(aq)) {
      Map<String,Object> ints=new LinkedHashMap<>();for(String name:List.of("x","y","z","A","B"))ints.put(name,field(aq,c("dyk$c"),name));
      result.put("source_grid_fields",ints);
      result.put("status_cache_length",Array.getLength(field(aq,c("dyk$c"),"r")));
      result.put("location_cache_length",Array.getLength(field(aq,c("dyk$c"),"s")));
    }
    return result;
  }
  static Map<String,Object> factory(String name,int min,int height)throws Exception {
    Map<String,Object> result=new LinkedHashMap<>();result.put("settings",name);result.put("build_min_y",min);result.put("build_height",height);
    Object settings=settings(name),state=state(name),access=c("dcy").getMethod("e",int.class,int.class).invoke(null,min,height);
    Object grid=c("dzg").getMethod("a",c("dcy")).invoke(c("dzd").getMethod("f").invoke(settings),access);
    result.put("clamped_min_y",c("dzg").getMethod("c").invoke(grid));result.put("clamped_height",c("dzg").getMethod("d").invoke(grid));
    result.put("cell_height",c("dzg").getMethod("a").invoke(grid));
    Object pos=c("dcd").getConstructor(int.class,int.class).newInstance(-1,2);
    Object proto=c("dvs").getConstructor(c("dcd"),c("dvv"),c("dcy"),c("jz"),c("eab"))
      .newInstance(pos,c("dvv").getField("a").get(null),access,biomeRegistry,null);
    try {
      Object chunk=c("dzc").getMethod("a",c("duy"),c("dzm"),c("dys$c"),c("dzd"),c("dyk$a"),c("eaa"))
        .invoke(null,proto,state,beard(),settings,picker(settings),c("eaa").getMethod("a").invoke(null));
      result.put("constructed",true);result.put("cell_count_y",field(chunk,c("dzc"),"c"));result.put("min_cell_y",field(chunk,c("dzc"),"d"));
      Object retainedGrid=field(chunk,c("dzc"),"a");result.put("precomputed_clamp_same_object",retainedGrid==grid);
      result.put("retained_grid_height",c("dzg").getMethod("d").invoke(retainedGrid));
      result.put("aquifer",aquifer(c("dzc").getMethod("i").invoke(chunk)));
      List<?> interpolators=(List<?>)field(chunk,c("dzc"),"i");List<Object> shapes=new ArrayList<>();
      for(Object i:interpolators){Object[] slice=(Object[])field(i,c("dzc$i"),"e");shapes.add(List.of(slice.length,slice.length==0?-1:Array.getLength(slice[0])));}
      result.put("interpolator_left_slice_shapes",shapes);
    } catch(Throwable error) {Throwable t=cause(error);result.put("constructed",false);result.put("error_class",t.getClass().getName());result.put("error_message",String.valueOf(t.getMessage()));}
    return result;
  }
  static List<Object> standalone()throws Exception {
    String name="minecraft:overworld";Object settings=settings(name),state=state(name),grid=c("dzd").getMethod("f").invoke(settings),fluid=picker(settings);
    int width=(int)c("dzg").getMethod("b").invoke(grid);
    Object noise=c("dzc").getConstructor(int.class,c("dzm"),int.class,int.class,c("dzg"),c("dys$c"),c("dzd"),c("dyk$a"),c("eaa"))
      .newInstance(16/width,state,-16,32,grid,beard(),settings,fluid,c("eaa").getMethod("a").invoke(null));
    Constructor<?> ctor=c("dyk$c").getDeclaredConstructor(c("dzc"),c("dcd"),c("dze"),c("dzl"),int.class,int.class,c("dyk$a"));ctor.setAccessible(true);
    Object pos=c("dcd").getConstructor(int.class,int.class).newInstance(-1,2),router=c("dzm").getMethod("a").invoke(state),random=c("dzm").getMethod("d").invoke(state);
    List<Object> result=new ArrayList<>();
    for(int height:new int[]{0,-1,-12,-24,-36,-48}) {
      Map<String,Object> row=new LinkedHashMap<>();row.put("min_y",320);row.put("height",height);
      try {Object aq=ctor.newInstance(noise,pos,router,random,320,height,fluid);row.put("constructed",true);row.put("aquifer",aquifer(aq));}
      catch(Throwable error){Throwable t=cause(error);row.put("constructed",false);row.put("error_class",t.getClass().getName());row.put("error_message",String.valueOf(t.getMessage()));}
      result.add(row);
    }
    return result;
  }
  public static void main(String[] args)throws Exception {
    setup();List<Object> rows=new ArrayList<>();
    for(Object[] q:new Object[][]{{"minecraft:overworld",320,64},{"minecraft:nether",-64,64},{"minecraft:overworld",321,16},{"minecraft:overworld",328,16},{"minecraft:overworld",329,16}})
      rows.add(factory((String)q[0],(int)q[1],(int)q[2]));
    OUT.println(new Gson().toJson(Map.of("minecraft","1.21","protocol",767,"inner_sha256",INNER,"seed",SEED,"chunk",List.of(-1,2),"factory",rows,"standalone_aquifer",standalone(),"scope","Actual original constructors only; no interpolation, material samples, BIOMES, SURFACE, scheduler or world execution.")));
  }
}
