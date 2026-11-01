package oracle;
import com.google.gson.*;
import java.io.*;
import java.lang.reflect.*;
import java.nio.*;
import java.nio.file.*;
import java.util.*;
import java.util.concurrent.*;
import java.util.function.*;

/** Controlled 3x3 real BIOMES palettes -> retained public NOISE -> SurfaceSystem.
 * Palette routing is an explicit fixture adapter, not WorldGenRegion/ChunkStep.
 * Actual persisted ProtoChunk statuses remain EMPTY throughout. No Unsafe. */
public class WorldgenBiomeSurfaceOracle extends WorldgenBiomeStageOracle {
 static final class ChunkOwner {
  int x,z,min,height;Object proto,noise,sampler,aquifer,initialStatus;
  List<Object> interpolators,cells;List<String> palette;
 }
 static List<Object> list(Object... values){return Arrays.asList(values);}
 static String chunkKey(int x,int z){return x+","+z;}
 static ChunkOwner prepare(int x,int z,int min,int height,Object state,Object set,Object biomeSource)throws Exception {
  ChunkOwner owner=new ChunkOwner();owner.x=x;owner.z=z;owner.min=min;owner.height=height;
  Object accessor=ACCESS.getMethod("e",int.class,int.class).invoke(null,min,height),pos=Class.forName("dcd").getConstructor(int.class,int.class).newInstance(x,z),upgrade=Class.forName("dvv").getField("a").get(null);
  owner.proto=PROTO.getConstructor(Class.forName("dcd"),Class.forName("dvv"),ACCESS,REGISTRY,Class.forName("eab")).newInstance(pos,upgrade,accessor,biomeRegistry,null);
  owner.initialStatus=PROTO.getMethod("j").invoke(owner.proto);
  Method picker=Class.forName("dzb").getDeclaredMethod("a",SETTINGS);picker.setAccessible(true);Object fluid=picker.invoke(null,set);Field beard=Class.forName("dys$b").getField("a");beard.setAccessible(true);
  owner.noise=CHUNK.getMethod("a",Class.forName("duy"),S,BEARD,SETTINGS,PICKER,BLENDER).invoke(null,owner.proto,state,beard.get(null),set,fluid,BLENDER.getMethod("a").invoke(null));
  Object installed=Class.forName("duy").getMethod("a",Function.class).invoke(owner.proto,(Function<Object,Object>)unused->owner.noise);if(installed!=owner.noise)throw new AssertionError("different NoiseChunk installed");
  owner.interpolators=owners(owner.noise,"i");owner.cells=owners(owner.noise,"j");owner.aquifer=CHUNK.getMethod("i").invoke(owner.noise);
  Method cached=CHUNK.getDeclaredMethod("a",Class.forName("dze"),List.class);cached.setAccessible(true);owner.sampler=cached.invoke(owner.noise,S.getMethod("a").invoke(state),SETTINGS.getMethod("k").invoke(set));return owner;
 }
 static List<List<Integer>> fillBiomes(ChunkOwner owner,Object source)throws Exception {
  List<List<Integer>> queries=new ArrayList<>();Method resolve=Class.forName("dea").getMethod("getNoiseBiome",int.class,int.class,int.class,Class.forName("def$f"));
  Object observed=Proxy.newProxyInstance(Class.forName("ddz").getClassLoader(),new Class<?>[]{Class.forName("ddz")},(proxy,method,args)->{
   if(!method.getName().equals("getNoiseBiome")||args[3]!=owner.sampler)throw new AssertionError("different biome sampler");
   queries.add(List.of((int)args[0],(int)args[1],(int)args[2]));try{return resolve.invoke(source,args);}catch(InvocationTargetException e){throw e.getCause();}
  });
  Class.forName("duy").getMethod("a",Class.forName("ddz"),Class.forName("def$f")).invoke(owner.proto,observed,owner.sampler);owner.palette=biomes(owner.proto);return queries;
 }
 static Map<String,Object> image(ChunkOwner owner)throws Exception {
  Constructor<?> point=POSITION.getConstructor(int.class,int.class,int.class);Method id=Class.forName("dfy").getMethod("i",BLOCKSTATE),get=PROTO.getMethod("a_",POSITION),height=Class.forName("duy").getMethod("a",Class.forName("dyy$a"),int.class,int.class);
  ByteBuffer blocks=ByteBuffer.allocate(owner.height*256*4).order(ByteOrder.LITTLE_ENDIAN);
  for(int y=owner.min;y<owner.min+owner.height;y++)for(int x=0;x<16;x++)for(int z=0;z<16;z++)blocks.putInt((int)id.invoke(null,get.invoke(owner.proto,point.newInstance(owner.x*16+x,y,owner.z*16+z))));
  List<Integer> surface=new ArrayList<>(),floor=new ArrayList<>();for(int z=0;z<16;z++)for(int x=0;x<16;x++){surface.add((int)height.invoke(owner.proto,Class.forName("dyy$a").getField("a").get(null),x,z)+1);floor.add((int)height.invoke(owner.proto,Class.forName("dyy$a").getField("c").get(null),x,z)+1);}
  Object[] queues=(Object[])Class.forName("duy").getMethod("n").invoke(owner.proto);Method size=Class.forName("it.unimi.dsi.fastutil.shorts.ShortList").getMethod("size"),getShort=Class.forName("it.unimi.dsi.fastutil.shorts.ShortList").getMethod("getShort",int.class),unpack=PROTO.getMethod("a",short.class,int.class,Class.forName("dcd"));Object chunkPos=Class.forName("dcd").getConstructor(int.class,int.class).newInstance(owner.x,owner.z);
  List<List<Short>> posts=new ArrayList<>();List<List<List<Integer>>> positions=new ArrayList<>();List<Boolean> allocated=new ArrayList<>();int count=0,firstSection=Math.floorDiv(owner.min,16);
  for(int section=0;section<queues.length;section++){List<Short> packed=new ArrayList<>();List<List<Integer>> decoded=new ArrayList<>();Object queue=queues[section];allocated.add(queue!=null);if(queue!=null)for(int i=0;i<(int)size.invoke(queue);i++){short value=(short)getShort.invoke(queue,i);Object p=unpack.invoke(null,value,firstSection+section,chunkPos);packed.add(value);decoded.add(List.of((int)POSITION.getMethod("u").invoke(p),(int)POSITION.getMethod("v").invoke(p),(int)POSITION.getMethod("w").invoke(p)));count++;}posts.add(packed);positions.add(decoded);}
  Map<String,Object> result=new LinkedHashMap<>();result.put("blocks",blocks.array());result.put("world_surface_wg",surface);result.put("ocean_floor_wg",floor);result.put("postprocessing_count",count);result.put("postprocessing_sections",posts);result.put("postprocessing_positions_by_section",positions);result.put("source_postprocessing_allocated",allocated);return result;
 }
 static Object runSurface(JsonObject q)throws Exception {
  String settings=q.get("settings").getAsString(),sourceName=q.get("biome_source").getAsString();long seed=q.get("seed").getAsLong();int cx=q.get("x").getAsInt(),cz=q.get("z").getAsInt(),min=q.get("level_min_y").getAsInt(),height=q.get("level_height").getAsInt();
  Object settingsHolder=getHolder(registry,settings),set=value(settingsHolder),state=S.getMethod("a",Class.forName("jn$a"),K,long.class).invoke(null,getters,key(registry,settings),seed),source=source(sourceName),generator=Class.forName("dzb").getConstructor(Class.forName("dea"),HOLDER).newInstance(source,settingsHolder);
  Map<String,ChunkOwner> neighbors=new LinkedHashMap<>();List<List<Integer>> chunks=new ArrayList<>();List<List<String>> palettes=new ArrayList<>();List<List<List<Integer>>> resolverQueries=new ArrayList<>();
  for(int dx=-1;dx<=1;dx++)for(int dz=-1;dz<=1;dz++){ChunkOwner owner=prepare(cx+dx,cz+dz,min,height,state,set,source);neighbors.put(chunkKey(owner.x,owner.z),owner);chunks.add(List.of(owner.x,owner.z));resolverQueries.add(fillBiomes(owner,source));palettes.add(owner.palette);}
  ChunkOwner center=neighbors.get(chunkKey(cx,cz));Object blender=BLENDER.getMethod("a").invoke(null);Object future=Class.forName("dzb").getMethod("a",BLENDER,S,Class.forName("ddq"),Class.forName("duy")).invoke(generator,blender,state,null,center.proto);if(((CompletableFuture<?>)future).join()!=center.proto)throw new AssertionError("NOISE returned another ProtoChunk");Map<String,Object> before=image(center);
  List<List<Integer>> quartQueries=new ArrayList<>(),neighborQueries=new ArrayList<>();
  // Controlled raw SECTION_BIOMES view with the source ChunkAccess Y clamp.
  // Calling ProtoChunk.getNoiseBiome would require a persisted BIOMES status;
  // no fixture is allowed to invent that scheduler/status transition.
  Class<?> providerType=Class.forName("ddy$a");Object provider=Proxy.newProxyInstance(providerType.getClassLoader(),new Class<?>[]{providerType},(proxy,method,args)->{
   if(!method.getName().equals("getNoiseBiome"))throw new UnsupportedOperationException(method.toString());int x=(int)args[0],y=(int)args[1],z=(int)args[2],chunkX=Math.floorDiv(x,4),chunkZ=Math.floorDiv(z,4);List<Integer> call=List.of(x,y,z);quartQueries.add(call);if(chunkX!=cx||chunkZ!=cz)neighborQueries.add(call);
   ChunkOwner owner=neighbors.get(chunkKey(chunkX,chunkZ));if(owner==null)throw new IllegalStateException("unavailable controlled BIOMES snapshot");int minimum=owner.min>>2,maximum=minimum+(owner.height>>2)-1,clamped=Math.max(minimum,Math.min(maximum,y)),section=Math.floorDiv(clamped,4)-Math.floorDiv(owner.min,16);Object[] sections=(Object[])Class.forName("duy").getMethod("d").invoke(owner.proto);return Class.forName("dvj").getMethod("c",int.class,int.class,int.class).invoke(sections[section],x&3,clamped&3,z&3);
  });
  Object manager=Class.forName("ddy").getConstructor(providerType,long.class).newInstance(provider,Class.forName("ddy").getMethod("a",long.class).invoke(null,seed));Object accessor=ACCESS.getMethod("e",int.class,int.class).invoke(null,min,height),context=Class.forName("dzv").getConstructor(Class.forName("duz"),ACCESS).newInstance(generator,accessor);boolean legacy=(boolean)SETTINGS.getMethod("n").invoke(set);
  Class.forName("dzq").getMethod("a",S,Class.forName("ddy"),REGISTRY,boolean.class,Class.forName("dzv"),Class.forName("duy"),CHUNK,Class.forName("dzp$o")).invoke(S.getMethod("c").invoke(state),state,manager,biomeRegistry,legacy,context,center.proto,center.noise,SETTINGS.getMethod("j").invoke(set));Map<String,Object> after=image(center);
  boolean cacheIdentity=true,biomesUnchanged=true,statusesUnchanged=true;for(ChunkOwner owner:neighbors.values()){cacheIdentity&=sameOwners(owner.interpolators,owners(owner.noise,"i"))&&sameOwners(owner.cells,owners(owner.noise,"j"))&&owner.aquifer==CHUNK.getMethod("i").invoke(owner.noise);biomesUnchanged&=owner.palette.equals(biomes(owner.proto));statusesUnchanged&=owner.initialStatus==PROTO.getMethod("j").invoke(owner.proto)&&Class.forName("dvz").getMethod("f").invoke(owner.initialStatus).toString().equals("minecraft:empty");}
  Object retained=Class.forName("duy").getMethod("a",Function.class).invoke(center.proto,(Function<Object,Object>)unused->{throw new AssertionError("lost center NoiseChunk");});if(retained!=center.noise)throw new AssertionError("replaced center NoiseChunk");
  String prefix=settings.replace(':','-')+"-"+sourceName.replace(':','-')+"-"+seed+"-"+cx+"-"+cz;Path paletteFile=outputDirectory.resolve("java-surface-palettes-"+prefix+".json"),resolverFile=outputDirectory.resolve("java-surface-resolver-queries-"+prefix+".json"),neighborFile=outputDirectory.resolve("java-surface-neighbor-queries-"+prefix+".json"),beforeFile=outputDirectory.resolve("java-before-natural-surface-"+prefix+".bin"),afterFile=outputDirectory.resolve("java-after-natural-surface-"+prefix+".bin");Gson json=new Gson();Files.writeString(paletteFile,json.toJson(palettes));Files.writeString(resolverFile,json.toJson(resolverQueries));Files.writeString(neighborFile,json.toJson(neighborQueries));Files.write(beforeFile,(byte[])before.remove("blocks"));Files.write(afterFile,(byte[])after.remove("blocks"));
  Map<String,Integer> counts=new TreeMap<>();for(String biome:center.palette)counts.merge(biome,1,Integer::sum);Map<String,Object> result=new LinkedHashMap<>();result.put("settings",settings);result.put("biome_source",sourceName);result.put("seed",seed);result.put("x",cx);result.put("z",cz);result.put("level_min_y",min);result.put("level_height",height);result.put("chunks",chunks);result.put("center_biome_counts",counts);result.put("palettes_file",paletteFile.toString());result.put("resolver_queries_file",resolverFile.toString());result.put("neighbor_queries_file",neighborFile.toString());result.put("before_file",beforeFile.toString());result.put("after_file",afterFile.toString());result.put("before",before);result.put("after",after);result.put("use_legacy_random_source",legacy);result.put("first_surface_quart_query",quartQueries.get(0));result.put("surface_quart_query_count",quartQueries.size());result.put("same_center_noise_chunk",retained==center.noise);result.put("same_cache_and_aquifer_owners",cacheIdentity);result.put("stored_biomes_unchanged",biomesUnchanged);result.put("persisted_statuses_remain_empty",statusesUnchanged);result.put("noise_future_same_chunk",true);result.put("oracle_inner_sha256",originalSha);return result;
 }
 public static void main(String[] args)throws Exception {if(args.length!=1)throw new IllegalArgumentException("usage: WorldgenBiomeSurfaceOracle <output-directory>");outputDirectory=Path.of(args[0]).toAbsolutePath().normalize();Files.createDirectories(outputDirectory);setup();originalSha=sha(Files.readAllBytes(Path.of(CHUNK.getProtectionDomain().getCodeSource().getLocation().toURI())));if(!originalSha.equals(INNER))throw new AssertionError("wrong pinned original NoiseChunk");Gson gson=new Gson();BufferedReader input=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=input.readLine())!=null;)try{OUT.println(gson.toJson(runSurface(JsonParser.parseString(line).getAsJsonObject())));}catch(InvocationTargetException e){e.getCause().printStackTrace();System.exit(1);}}
}
