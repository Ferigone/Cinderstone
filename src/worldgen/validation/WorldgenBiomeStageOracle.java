package oracle;
import com.google.gson.*;
import java.io.*;
import java.lang.reflect.*;
import java.nio.*;
import java.nio.file.*;
import java.security.*;
import java.util.*;
import java.util.concurrent.*;
import java.util.function.*;

/** Actual seeded biome sources -> real section palettes -> same-owner public NOISE.
 * Controlled empty blending/beardifier; no ChunkStep/status or FULL publication. */
public class WorldgenBiomeStageOracle extends WorldgenSurfaceOracle {
 static final String INNER="8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01";
 static String originalSha;
 static String sha(byte[] data)throws Exception{return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(data));}
 static String biomeName(Object holder)throws Exception {
  Object key=((Optional<?>)HOLDER.getMethod("e").invoke(holder)).orElseThrow();
  return K.getMethod("a").invoke(key).toString();
 }
 static Object source(String name)throws Exception {
  if(name.equals("minecraft:end")) {
   Object getter=Class.forName("jn$a").getMethod("b",K).invoke(getters,biomeKey);
   return Class.forName("den").getMethod("a",Class.forName("jn")).invoke(null,getter);
  }
  if(!name.equals("minecraft:overworld")&&!name.equals("minecraft:nether"))throw new IllegalArgumentException("explicit built-in biome source required");
  Object presets=Class.forName("lu").getField("aZ").get(null);
  return Class.forName("dej").getMethod("a",HOLDER).invoke(null,getHolder(presets,name));
 }
 static List<Object> owners(Object chunk,String field)throws Exception {
  Field f=CHUNK.getDeclaredField(field);f.setAccessible(true);return new ArrayList<>((List<?>)f.get(chunk));
 }
 static boolean sameOwners(List<Object> before,List<Object> after) {
  if(before.size()!=after.size())return false;for(int i=0;i<before.size();i++)if(before.get(i)!=after.get(i))return false;return true;
 }
 static List<String> biomes(Object proto)throws Exception {
  Object[] sections=(Object[])Class.forName("duy").getMethod("d").invoke(proto);
  Method get=Class.forName("dvj").getMethod("c",int.class,int.class,int.class);List<String> values=new ArrayList<>();
  // Actual SECTION_BIOMES palette order; resolver call order is captured separately.
  for(Object section:sections)for(int y=0;y<4;y++)for(int z=0;z<4;z++)for(int x=0;x<4;x++)values.add(biomeName(get.invoke(section,x,y,z)));
  return values;
 }
 static Object runStage(JsonObject q)throws Exception {
  String name=q.get("settings").getAsString(),sourceName=q.get("biome_source").getAsString();long seed=q.get("seed").getAsLong();int cx=q.get("x").getAsInt(),cz=q.get("z").getAsInt(),min=q.get("level_min_y").getAsInt(),total=q.get("level_height").getAsInt();
  Object settingsHolder=getHolder(registry,name),set=value(settingsHolder),state=S.getMethod("a",Class.forName("jn$a"),K,long.class).invoke(null,getters,key(registry,name),seed);
  Object accessor=ACCESS.getMethod("e",int.class,int.class).invoke(null,min,total),chunkPos=Class.forName("dcd").getConstructor(int.class,int.class).newInstance(cx,cz),upgrade=Class.forName("dvv").getField("a").get(null);
  Object proto=PROTO.getConstructor(Class.forName("dcd"),Class.forName("dvv"),ACCESS,REGISTRY,Class.forName("eab")).newInstance(chunkPos,upgrade,accessor,biomeRegistry,null);
  Object initialStatus=PROTO.getMethod("j").invoke(proto);
  Method picker=Class.forName("dzb").getDeclaredMethod("a",SETTINGS);picker.setAccessible(true);Object fluid=picker.invoke(null,set);
  Field beardField=Class.forName("dys$b").getField("a");beardField.setAccessible(true);Object beard=beardField.get(null),blender=BLENDER.getMethod("a").invoke(null);
  Object chunk=CHUNK.getMethod("a",Class.forName("duy"),S,BEARD,SETTINGS,PICKER,BLENDER).invoke(null,proto,state,beard,set,fluid,blender);
  Method getOrCreate=Class.forName("duy").getMethod("a",Function.class);int[] factoryCalls={0};
  Object installed=getOrCreate.invoke(proto,(Function<Object,Object>)unused->{factoryCalls[0]++;return chunk;});if(installed!=chunk)throw new AssertionError("NoiseChunk installation changed owner");
  List<Object> interpolationOwners=owners(chunk,"i"),cellOwners=owners(chunk,"j");Object aquifer=CHUNK.getMethod("i").invoke(chunk);
  Method cached=CHUNK.getDeclaredMethod("a",Class.forName("dze"),List.class);cached.setAccessible(true);Object sampler=cached.invoke(chunk,S.getMethod("a").invoke(state),SETTINGS.getMethod("k").invoke(set));
  Object biomeSource=source(sourceName);Method resolve=Class.forName("dea").getMethod("getNoiseBiome",int.class,int.class,int.class,Class.forName("def$f"));
  List<List<Integer>> queries=new ArrayList<>();int[] samplerIdentityFaults={0};
  Object observed=Proxy.newProxyInstance(Class.forName("ddz").getClassLoader(),new Class<?>[]{Class.forName("ddz")},(proxy,method,args)->{
   if(!method.getName().equals("getNoiseBiome"))throw new UnsupportedOperationException(method.toString());
   if(args[3]!=sampler)samplerIdentityFaults[0]++;
   queries.add(List.of((int)args[0],(int)args[1],(int)args[2]));
   try{return resolve.invoke(biomeSource,args);}catch(InvocationTargetException e){throw e.getCause();}
  });
  Class.forName("duy").getMethod("a",Class.forName("ddz"),Class.forName("def$f")).invoke(proto,observed,sampler);
  List<String> selected=biomes(proto);if(selected.size()!=total/16*64||queries.size()!=selected.size())throw new AssertionError("BIOMES did not fill every actual section cell");
  Object generator=Class.forName("dzb").getConstructor(Class.forName("dea"),HOLDER).newInstance(biomeSource,settingsHolder);
  // StructureManager is absent but source-proved unused here: the real cached
  // NoiseChunk prevents the createNoiseChunk callback from running. Empty
  // Blender and BeardifierMarker above are explicit controlled inputs.
  Object future=Class.forName("dzb").getMethod("a",BLENDER,S,Class.forName("ddq"),Class.forName("duy")).invoke(generator,blender,state,null,proto);
  if(((CompletableFuture<?>)future).join()!=proto)throw new AssertionError("source NOISE replaced the input ProtoChunk");
  Object retained=getOrCreate.invoke(proto,(Function<Object,Object>)unused->{throw new AssertionError("source NOISE lost the installed NoiseChunk");});
  if(retained!=chunk||factoryCalls[0]!=1||samplerIdentityFaults[0]!=0)throw new AssertionError("source generation owner identity changed");
  boolean caches=sameOwners(interpolationOwners,owners(chunk,"i"))&&sameOwners(cellOwners,owners(chunk,"j"));
  if(!selected.equals(biomes(proto)))throw new AssertionError("NOISE changed stored BIOMES");
  Method stateId=Class.forName("dfy").getMethod("i",BLOCKSTATE),getBlock=PROTO.getMethod("a_",POSITION);Constructor<?> point=POSITION.getConstructor(int.class,int.class,int.class);
  ByteBuffer blocks=ByteBuffer.allocate(total*256*4).order(ByteOrder.LITTLE_ENDIAN);for(int y=min;y<min+total;y++)for(int x=0;x<16;x++)for(int z=0;z<16;z++)blocks.putInt((int)stateId.invoke(null,getBlock.invoke(proto,point.newInstance(cx*16+x,y,cz*16+z))));
  List<Integer> worldSurface=new ArrayList<>(),oceanFloor=new ArrayList<>();Method height=Class.forName("duy").getMethod("a",Class.forName("dyy$a"),int.class,int.class);Object surfaceType=Class.forName("dyy$a").getField("a").get(null),floorType=Class.forName("dyy$a").getField("c").get(null);
  for(int z=0;z<16;z++)for(int x=0;x<16;x++){worldSurface.add((int)height.invoke(proto,surfaceType,x,z)+1);oceanFloor.add((int)height.invoke(proto,floorType,x,z)+1);}
  int posts=0;Object[] lists=(Object[])Class.forName("duy").getMethod("n").invoke(proto);
  int firstSection=Math.floorDiv(min,16),sectionCount=Math.floorDiv(min+total-1,16)-firstSection+1;
  if(lists.length!=sectionCount)throw new AssertionError("unexpected actual postprocessing section span");
  Method shortSize=Class.forName("it.unimi.dsi.fastutil.shorts.ShortList").getMethod("size"),getShort=Class.forName("it.unimi.dsi.fastutil.shorts.ShortList").getMethod("getShort",int.class),pack=PROTO.getMethod("g",POSITION),unpack=PROTO.getMethod("a",short.class,int.class,Class.forName("dcd"));
  List<List<Short>> postSections=new ArrayList<>();List<List<List<Integer>>> postPositions=new ArrayList<>();List<Boolean> postAllocated=new ArrayList<>();
  for(int section=0;section<lists.length;section++) {
   Object list=lists[section];postAllocated.add(list!=null);List<Short> packedValues=new ArrayList<>();List<List<Integer>> positions=new ArrayList<>();
   if(list!=null)for(int index=0;index<(int)shortSize.invoke(list);index++) {
    short packed=(short)getShort.invoke(list,index);Object decoded=unpack.invoke(null,packed,firstSection+section,chunkPos);
    if((short)pack.invoke(null,decoded)!=packed)throw new AssertionError("original packed postprocessing roundtrip failed");
    packedValues.add(packed);positions.add(List.of((int)POSITION.getMethod("u").invoke(decoded),(int)POSITION.getMethod("v").invoke(decoded),(int)POSITION.getMethod("w").invoke(decoded)));posts++;
   }
   // Null and allocated-empty both contain no queued positions; allocation is
   // reported separately rather than invented for native's global call vector.
   postSections.add(packedValues);postPositions.add(positions);
  }
  String prefix=name.replace(':','-')+"-"+sourceName.replace(':','-')+"-"+seed+"-"+cx+"-"+cz;Path biomeFile=outputDirectory.resolve("java-biomes-"+prefix+".json"),queryFile=outputDirectory.resolve("java-biome-queries-"+prefix+".json"),blockFile=outputDirectory.resolve("java-biome-noise-"+prefix+".bin");Gson json=new Gson();Files.writeString(biomeFile,json.toJson(selected));Files.writeString(queryFile,json.toJson(queries));Files.write(blockFile,blocks.array());
  Map<String,Integer> counts=new TreeMap<>();for(String biome:selected)counts.merge(biome,1,Integer::sum);
  Map<String,Object> out=new LinkedHashMap<>();out.put("settings",name);out.put("biome_source",sourceName);out.put("seed",seed);out.put("x",cx);out.put("z",cz);out.put("level_min_y",min);out.put("level_height",total);out.put("biomes_file",biomeFile.toString());out.put("queries_file",queryFile.toString());out.put("blocks_file",blockFile.toString());out.put("biome_counts",counts);out.put("biome_sha256",sha(Files.readAllBytes(biomeFile)));out.put("query_sha256",sha(Files.readAllBytes(queryFile)));out.put("noise_sha256",sha(blocks.array()));out.put("world_surface_wg",worldSurface);out.put("ocean_floor_wg",oceanFloor);out.put("postprocessing_count",posts);out.put("postprocessing_min_section_y",firstSection);out.put("postprocessing_sections",postSections);out.put("postprocessing_positions_by_section",postPositions);out.put("source_postprocessing_allocated",postAllocated);out.put("source_pack_roundtrip",true);out.put("same_noise_chunk",retained==chunk);out.put("same_aquifer",aquifer==CHUNK.getMethod("i").invoke(chunk));out.put("same_cache_owners",caches);out.put("same_sampler",samplerIdentityFaults[0]==0);out.put("persisted_status_unchanged",initialStatus==PROTO.getMethod("j").invoke(proto));out.put("initial_status",Class.forName("dvz").getMethod("f").invoke(initialStatus));out.put("noise_future_same_chunk",true);out.put("oracle_inner_sha256",originalSha);return out;
 }
 public static void main(String[] args)throws Exception {
  if(args.length!=1)throw new IllegalArgumentException("usage: WorldgenBiomeStageOracle <output-directory>");outputDirectory=Path.of(args[0]).toAbsolutePath().normalize();Files.createDirectories(outputDirectory);setup();
  originalSha=sha(Files.readAllBytes(Path.of(CHUNK.getProtectionDomain().getCodeSource().getLocation().toURI())));if(!originalSha.equals(INNER))throw new AssertionError("wrong loaded original NoiseChunk artifact");
  Gson gson=new Gson();BufferedReader input=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=input.readLine())!=null;)try{OUT.println(gson.toJson(runStage(JsonParser.parseString(line).getAsJsonObject())));}catch(InvocationTargetException e){e.getCause().printStackTrace();System.exit(1);}
 }
}
