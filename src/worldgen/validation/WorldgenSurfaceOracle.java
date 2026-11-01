package oracle;
import com.google.gson.*;
import java.io.*;
import java.lang.reflect.*;
import java.nio.*;
import java.nio.file.*;
import java.security.*;
import java.util.*;
import java.util.stream.*;

/** Controlled single-biome surface execution on actual ProtoChunk owners. No Unsafe. */
public class WorldgenSurfaceOracle {
 static final PrintStream OUT=System.out;
 static Path outputDirectory;
 static Class<?> K,L,S,SETTINGS,CHUNK,NOISE,BEARD,PICKER,BLENDER,BLOCKSTATE,ACCESS,PROTO,REGISTRY,HOLDER,POSITION;
 static Object lookup,getters,registry,biomeKey,biomeRegistry;
 static Object key(Object registry,String name)throws Exception {return K.getMethod("a",K,L).invoke(null,registry,L.getMethod("a",String.class).invoke(null,name));}
 static Object getHolder(Object registry,String name)throws Exception {Object getter=Class.forName("jn$a").getMethod("b",K).invoke(getters,registry);return Class.forName("jn").getMethod("b",K).invoke(getter,key(registry,name));}
 static Object value(Object holder)throws Exception{return HOLDER.getMethod("a").invoke(holder);}
 static void setup()throws Exception {
  Class.forName("ab").getMethod("a").invoke(null);Class.forName("akt").getMethod("a").invoke(null);
  K=Class.forName("akq");L=Class.forName("akr");S=Class.forName("dzm");SETTINGS=Class.forName("dzd");CHUNK=Class.forName("dzc");NOISE=Class.forName("dzg");BEARD=Class.forName("dys$c");PICKER=Class.forName("dyk$a");BLENDER=Class.forName("eaa");BLOCKSTATE=Class.forName("dtc");ACCESS=Class.forName("dcy");PROTO=Class.forName("dvs");REGISTRY=Class.forName("jz");HOLDER=Class.forName("jm");POSITION=Class.forName("jd");
  lookup=Class.forName("pd").getMethod("a").invoke(null);getters=Class.forName("jo$a").getMethod("b").invoke(lookup);registry=Class.forName("lu").getField("aO").get(null);biomeKey=Class.forName("lu").getField("aF").get(null);
  Object lifecycle=Class.forName("com.mojang.serialization.Lifecycle").getMethod("stable").invoke(null);biomeRegistry=Class.forName("ju").getConstructor(K,Class.forName("com.mojang.serialization.Lifecycle")).newInstance(biomeKey,lifecycle);
  Object biomeLookup=Class.forName("jo$a").getMethod("b",K).invoke(lookup,biomeKey);
  Object info=Class.forName("jy").getField("a").get(null);
  Method register=Class.forName("ju").getMethod("a",K,Object.class,Class.forName("jy"));
  try(Stream<?> holders=(Stream<?>)Class.forName("jo").getMethod("b").invoke(biomeLookup)) {for(Object h:holders.toList()){Object k=Class.forName("jm$c").getMethod("h").invoke(h);register.invoke(biomeRegistry,k,value(h),info);}}
  REGISTRY.getMethod("m").invoke(biomeRegistry);
 }
 static Object run(JsonObject q)throws Exception {
  String name=q.get("settings").getAsString(),biomeName=q.get("biome").getAsString();long seed=q.get("seed").getAsLong();int cx=q.get("x").getAsInt(),cz=q.get("z").getAsInt();
  Object settingsHolder=getHolder(registry,name),set=value(settingsHolder),state=S.getMethod("a",Class.forName("jn$a"),K,long.class).invoke(null,getters,key(registry,name),seed);
  Object noise=SETTINGS.getMethod("f").invoke(set);
  int width=(int)NOISE.getMethod("b").invoke(noise),height=(int)NOISE.getMethod("a").invoke(noise),min=(int)NOISE.getMethod("c").invoke(noise),total=(int)NOISE.getMethod("d").invoke(noise);
  Object accessor=ACCESS.getMethod("e",int.class,int.class).invoke(null,min,total);
  Object pos=Class.forName("dcd").getConstructor(int.class,int.class).newInstance(cx,cz);
  Object upgrade=Class.forName("dvv").getField("a").get(null);
  Object proto=PROTO.getConstructor(Class.forName("dcd"),Class.forName("dvv"),ACCESS,REGISTRY,Class.forName("eab")).newInstance(pos,upgrade,accessor,biomeRegistry,null);
  Object noisedStatus=Class.forName("dvz").getField("g").get(null);PROTO.getMethod("a",Class.forName("dvz")).invoke(proto,noisedStatus);
  Object biomeHolder=getHolder(biomeKey,biomeName),fixed=Class.forName("deh").getConstructor(HOLDER).newInstance(biomeHolder);
  Class.forName("duy").getMethod("a",Class.forName("ddz"),Class.forName("def$f")).invoke(proto,fixed,S.getMethod("b").invoke(state));
  // Observe the actual original zoom provider call during SurfaceSystem.
  // The fixed biome makes block images insensitive to Y, so the first selected
  // quart is a separate witness of the caller's legacy-height flag.
  List<List<Integer>> firstBiomeQuartQuery=new ArrayList<>();
  Class<?> noiseBiomeSource=Class.forName("ddy$a");
  Object observedSource=Proxy.newProxyInstance(noiseBiomeSource.getClassLoader(),new Class<?>[]{noiseBiomeSource},(proxy,method,args)->{
   if(method.getName().equals("getNoiseBiome")&&firstBiomeQuartQuery.isEmpty())firstBiomeQuartQuery.add(List.of((int)args[0],(int)args[1],(int)args[2]));
   return method.invoke(fixed,args);
  });
  Object biomeManager=Class.forName("ddy").getConstructor(noiseBiomeSource,long.class).newInstance(observedSource,Class.forName("ddy").getMethod("a",long.class).invoke(null,seed));
  Object generator=Class.forName("dzb").getConstructor(Class.forName("dea"),HOLDER).newInstance(fixed,settingsHolder);
  Object generationContext=Class.forName("dzv").getConstructor(Class.forName("duz"),ACCESS).newInstance(generator,accessor);
  String inputName=q.has("input")?q.get("input").getAsString():"java-"+name.replace(':','-')+"-"+seed+"-"+cx+"-"+cz+".bin";
  Path inputPath=Path.of(inputName);String input=(inputPath.isAbsolute()?inputPath:outputDirectory.resolve(inputPath)).toString();
  byte[] raw=Files.readAllBytes(Path.of(input));ByteBuffer data=ByteBuffer.wrap(raw).order(ByteOrder.LITTLE_ENDIAN);
  Method stateById=Class.forName("dfy").getMethod("a",int.class),stateId=Class.forName("dfy").getMethod("i",BLOCKSTATE),setBlock=PROTO.getMethod("a",POSITION,BLOCKSTATE,boolean.class),getBlock=PROTO.getMethod("a_",POSITION);
  Constructor<?> point=POSITION.getConstructor(int.class,int.class,int.class);int[] before=new int[total*256];int i=0;
  for(int y=min;y<min+total;y++)for(int x=0;x<16;x++)for(int z=0;z<16;z++){int id=data.getInt();before[i++]=id;if(id!=0)setBlock.invoke(proto,point.newInstance(cx*16+x,y,cz*16+z),stateById.invoke(null,id),false);}
  Set<Object> heightmapTypes=new HashSet<>();for(String field:new String[]{"a","c"})heightmapTypes.add(Class.forName("dyy$a").getField(field).get(null));
  Class.forName("dyy").getMethod("a",Class.forName("duy"),Set.class).invoke(null,proto,heightmapTypes);
  Method picker=Class.forName("dzb").getDeclaredMethod("a",SETTINGS);picker.setAccessible(true);Object fp=picker.invoke(null,set);
  Field bf=Class.forName("dys$b").getField("a");bf.setAccessible(true);Object beard=bf.get(null),blender=BLENDER.getMethod("a").invoke(null);
  Object chunk=CHUNK.getConstructor(int.class,S,int.class,int.class,NOISE,BEARD,SETTINGS,PICKER,BLENDER).newInstance(16/width,state,cx*16,cz*16,noise,beard,set,fp,blender);
  Object surface=S.getMethod("c").invoke(state),rule=SETTINGS.getMethod("j").invoke(set);
  boolean useLegacyRandomSource=(boolean)SETTINGS.getMethod("n").invoke(set);
  int firstSurfaceY=(int)Class.forName("duy").getMethod("a",Class.forName("dyy$a"),int.class,int.class).invoke(proto,Class.forName("dyy$a").getField("a").get(null),0,0)+1;
  List<Integer> expectedFirstBiomeBlockQuery=List.of(cx*16,useLegacyRandomSource?0:firstSurfaceY,cz*16);
  Class.forName("dzq").getMethod("a",S,Class.forName("ddy"),REGISTRY,boolean.class,Class.forName("dzv"),Class.forName("duy"),CHUNK,Class.forName("dzp$o")).invoke(surface,state,biomeManager,biomeRegistry,useLegacyRandomSource,generationContext,proto,chunk,rule);
  ByteBuffer result=ByteBuffer.allocate(total*256*4).order(ByteOrder.LITTLE_ENDIAN);Map<Integer,Integer> counts=new TreeMap<>();i=0;int changes=0;
  for(int y=min;y<min+total;y++)for(int x=0;x<16;x++)for(int z=0;z<16;z++){int id=(int)stateId.invoke(null,getBlock.invoke(proto,point.newInstance(cx*16+x,y,cz*16+z)));result.putInt(id);counts.merge(id,1,Integer::sum);if(id!=before[i++])changes++;}
  byte[] bytes=result.array();String out=outputDirectory.resolve("java-surface-"+name.replace(':','-')+"-"+seed+"-"+cx+"-"+cz+"-"+biomeName.replace(':','-')+".bin").toString();Files.write(Path.of(out),bytes);
  int postprocess=0;Object[] posts=(Object[])Class.forName("duy").getMethod("n").invoke(proto);for(Object list:posts)if(list!=null)postprocess+=(int)Class.forName("it.unimi.dsi.fastutil.shorts.ShortList").getMethod("size").invoke(list);
  Map<String,Object> r=new LinkedHashMap<>();r.put("settings",name);r.put("seed",seed);r.put("x",cx);r.put("z",cz);r.put("biome",biomeName);r.put("min_y",min);r.put("height",total);r.put("input",input);r.put("output",out);r.put("sha256",HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes)));r.put("changes",changes);r.put("postprocess",postprocess);r.put("use_legacy_random_source",useLegacyRandomSource);r.put("expected_first_biome_block_query",expectedFirstBiomeBlockQuery);r.put("first_biome_quart_query",firstBiomeQuartQuery.get(0));r.put("counts",counts);List<Integer> heights=new ArrayList<>();Object surfaceType=Class.forName("dyy$a").getField("a").get(null),floorType=Class.forName("dyy$a").getField("c").get(null);Method heightAt=Class.forName("duy").getMethod("a",Class.forName("dyy$a"),int.class,int.class);for(int x=0;x<16;x++)for(int z=0;z<16;z++){heights.add((int)heightAt.invoke(proto,surfaceType,x,z));heights.add((int)heightAt.invoke(proto,floorType,x,z));}r.put("heightmaps_world_surface_wg_ocean_floor_wg",heights);
  // Exact controlled-fixture callbacks from the actual selected Biome owner.
  Object biome=value(biomeHolder);Method cold=Class.forName("ddw").getMethod("b",POSITION),melt=Class.forName("ddw").getMethod("d",POSITION);
  List<String> thermal=new ArrayList<>();List<Boolean> melt63=new ArrayList<>();
  for(int x=0;x<16;x++)for(int z=0;z<16;z++) {StringBuilder row=new StringBuilder();for(int y=min;y<=min+total+1;y++)row.append((boolean)cold.invoke(biome,point.newInstance(cx*16+x,y,cz*16+z))?'1':'0');thermal.add(row.toString());melt63.add((boolean)melt.invoke(biome,point.newInstance(cx*16+x,63,cz*16+z)));}
  Map<String,Object> context=new LinkedHashMap<>();context.put("settings",name);context.put("seed",seed);context.put("x",cx);context.put("z",cz);context.put("biome",biomeName);context.put("min_y",min);context.put("max_y_inclusive",min+total+1);context.put("layout","rows x*16+z, each string indexed by y-min_y");context.put("cold_enough_to_snow",thermal);context.put("should_melt_frozen_ocean_iceberg_slightly_at_63",melt63);
  String contextPath=outputDirectory.resolve("surface-context-"+name.replace(':','-')+"-"+seed+"-"+cx+"-"+cz+"-"+biomeName.replace(':','-')+".json").toString();Files.writeString(Path.of(contextPath),new Gson().toJson(context));r.put("context",contextPath);
  return r;
 }
 public static void main(String[]args)throws Exception {
  if(args.length!=1)throw new IllegalArgumentException("usage: WorldgenSurfaceOracle <output-directory>");
  outputDirectory=Path.of(args[0]).toAbsolutePath().normalize();Files.createDirectories(outputDirectory);
  setup();Gson gson=new Gson();BufferedReader input=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=input.readLine())!=null;)try{OUT.println(gson.toJson(run(JsonParser.parseString(line).getAsJsonObject())));}catch(InvocationTargetException e){e.getCause().printStackTrace();System.exit(1);}}
}
