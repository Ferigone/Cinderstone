package oracle;
import com.google.gson.*;
import com.mojang.serialization.*;
import java.io.*;
import java.lang.reflect.*;
import java.nio.*;
import java.nio.file.*;
import java.security.*;
import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.*;
import java.util.function.*;
import java.util.jar.*;

/** Unchanged original carver/provider execution over controlled real owners.
 * Geometry/material checks never stand in for server ChunkStatus completion. */
public class WorldgenCarverOracle extends WorldgenBiomeSurfaceOracle {
 static final Gson JSON=new GsonBuilder().serializeNulls().create();
 static Object readField(Object owner,Class<?> type,String name)throws Exception {Field f=type.getDeclaredField(name);f.setAccessible(true);return f.get(owner);}
 static Object configured(String name)throws Exception{return value(getHolder(Class.forName("lu").getField("aH").get(null),name));}
 static Map<String,Object> resolvedDebug(Object settings)throws Exception {
  Class<?> type=Class.forName("eay");Map<String,Object> result=new LinkedHashMap<>();result.put("debug_mode",type.getMethod("a").invoke(settings));
  Method id=Class.forName("dfy").getMethod("i",BLOCKSTATE);String[] names={"air_state","water_state","lava_state","barrier_state"};
  for(int i=0;i<4;i++)result.put(names[i],id.invoke(null,type.getMethod(String.valueOf((char)('b'+i))).invoke(settings)));
  result.put("uses_default_owner",settings==type.getField("a").get(null));return result;
 }
 @SuppressWarnings("unchecked") static Object parseProvider(String kind,JsonElement json)throws Exception {
  Class<?> type=Class.forName(kind.equals("float")?"bpu":"ehx");Codec<Object> codec=(Codec<Object>)type.getField("c").get(null);return codec.parse(JsonOps.INSTANCE,json).getOrThrow();
 }
 static final Map<String,String> TAG_DIGESTS=new TreeMap<>();
 @SuppressWarnings("unchecked") static void bindBuiltinTags()throws Exception {
  Path jar=Path.of(CHUNK.getProtectionDomain().getCodeSource().getLocation().toURI());
  Class<?> file=Class.forName("awt"),entry=Class.forName("aws"),loader=Class.forName("awv"),entryWithSource=Class.forName("awv$a"),tag=Class.forName("awu");Codec<Object> codec=(Codec<Object>)file.getField("a").get(null);
  try(JarFile archive=new JarFile(jar.toFile())) {for(String kind:new String[]{"block","fluid"}) {
   Object target=Class.forName("lt").getField(kind.equals("block")?"e":"c").get(null),registryKey=REGISTRY.getMethod("d").invoke(target);String prefix="data/minecraft/tags/"+kind+"/";Map<Object,List<Object>> input=new LinkedHashMap<>();
   for(JarEntry resource:Collections.list(archive.entries()))if(resource.getName().startsWith(prefix)&&resource.getName().endsWith(".json")) {
    byte[] raw=archive.getInputStream(resource).readAllBytes();TAG_DIGESTS.put(resource.getName(),sha(raw));Object decoded=codec.parse(JsonOps.INSTANCE,JsonParser.parseString(new String(raw,java.nio.charset.StandardCharsets.UTF_8))).getOrThrow();List<Object> entries=new ArrayList<>();
    for(Object e:(List<?>)file.getMethod("a").invoke(decoded))entries.add(entryWithSource.getConstructor(entry,String.class).newInstance(e,"pinned minecraft 1.21 server builtin pack"));
    String path=resource.getName().substring(prefix.length(),resource.getName().length()-5);input.put(L.getMethod("a",String.class).invoke(null,"minecraft:"+path),entries);
   }
   Function<Object,Object> lookup=id->{try{return REGISTRY.getMethod("c",L).invoke(target,id);}catch(Exception e){throw new RuntimeException(e);}};
   Object originalLoader=loader.getConstructor(Function.class,String.class).newInstance(lookup,"tags/"+kind);Map<Object,Collection<Object>> built=(Map<Object,Collection<Object>>)loader.getMethod("a",Map.class).invoke(originalLoader,input);
   if(built.size()!=input.size())throw new AssertionError("unresolved original "+kind+" tags "+built.size()+"/"+input.size());Map<Object,List<Object>> bound=new LinkedHashMap<>();
   for(Map.Entry<Object,Collection<Object>> item:built.entrySet())bound.put(tag.getMethod("a",K,L).invoke(null,registryKey,item.getKey()),new ArrayList<>(item.getValue()));REGISTRY.getMethod("a",Map.class).invoke(target,bound);
  }}
 }
 static Map<String,Object> tagWitness()throws Exception {
  Method can=Class.forName("ebe").getDeclaredMethod("a",Class.forName("eax"),BLOCKSTATE);can.setAccessible(true);Method state=Class.forName("dfy").getMethod("a",int.class);Map<String,Object> result=new LinkedHashMap<>();
  for(String name:new String[]{"minecraft:cave","minecraft:nether_cave"}) {Object cc=configured(name),world=Class.forName("ebc").getMethod("a").invoke(cc),config=Class.forName("ebc").getMethod("b").invoke(cc);List<Boolean> values=new ArrayList<>();for(int id:new int[]{1,10,5849})values.add((boolean)can.invoke(world,config,state.invoke(null,id)));result.put(name,values);}
  if(!result.get("minecraft:cave").equals(List.of(true,true,false))||!result.get("minecraft:nether_cave").equals(List.of(true,true,true)))throw new AssertionError("incorrect original replacement membership "+result);return result;
 }
 static Object providers()throws Exception {
  Map<String,Object> debug=new LinkedHashMap<>();for(String name:new String[]{"minecraft:cave","minecraft:cave_extra_underground","minecraft:canyon","minecraft:nether_cave"}) {
   Object config=Class.forName("ebc").getMethod("b").invoke(configured(name));debug.put(name,resolvedDebug(Class.forName("eax").getField("h").get(config)));
  }
  Object settingsHolder=getHolder(registry,"minecraft:overworld"),source=source("minecraft:overworld"),generator=Class.forName("dzb").getConstructor(Class.forName("dea"),HOLDER).newInstance(source,settingsHolder);
  String[][] cases={{"float","0.375"},{"float","{\"type\":\"minecraft:uniform\",\"min_inclusive\":0.1,\"max_exclusive\":0.9}"},{"float","{\"type\":\"minecraft:trapezoid\",\"min\":0.0,\"max\":6.0,\"plateau\":2.0}"},{"height","{\"absolute\":10}"},{"height","{\"type\":\"minecraft:uniform\",\"min_inclusive\":{\"above_bottom\":8},\"max_inclusive\":{\"absolute\":180}}"},{"height","{\"type\":\"minecraft:uniform\",\"min_inclusive\":{\"absolute\":7},\"max_inclusive\":{\"absolute\":7}}"},{"height","{\"type\":\"minecraft:uniform\",\"min_inclusive\":{\"above_bottom\":20},\"max_inclusive\":{\"below_top\":1}}"}};
  List<Object> vectors=new ArrayList<>();Class<?> randomType=Class.forName("ayw"),legacy=Class.forName("dyz"),contextType=Class.forName("dzv");
  for(int i=0;i<cases.length;i++) {
   String kind=cases[i][0];JsonElement json=JsonParser.parseString(cases[i][1]);Object provider=parseProvider(kind,json);long seed=123456789L;
   Object random=legacy.getConstructor(long.class).newInstance(seed),accessor=ACCESS.getMethod("e",int.class,int.class).invoke(null,i==6?0:-64,i==6?16:384),context=contextType.getConstructor(Class.forName("duz"),ACCESS).newInstance(generator,accessor);
   Method sample=kind.equals("float")?Class.forName("bpz").getMethod("a",randomType):Class.forName("ehx").getMethod("a",randomType,contextType);List<Object> samples=new ArrayList<>();
   for(int j=0;j<8;j++)if(kind.equals("float"))samples.add(Integer.toUnsignedLong(Float.floatToRawIntBits((float)sample.invoke(provider,random))));else samples.add(sample.invoke(provider,random,context));
   Map<String,Object> row=new LinkedHashMap<>();row.put("kind",kind);row.put("provider",json);row.put("seed",seed);row.put("min_gen_y",contextType.getMethod("a").invoke(context));row.put("gen_depth",contextType.getMethod("b").invoke(context));row.put("samples",samples);row.put("next_i64",randomType.getMethod("g").invoke(random));vectors.add(row);
  }
  return Map.of("resolved_debug_states",debug,"provider_vectors",vectors);
 }
 static Object freshProto(int x,int z,int min,int height)throws Exception {
  return PROTO.getConstructor(Class.forName("dcd"),Class.forName("dvv"),ACCESS,REGISTRY,Class.forName("eab")).newInstance(Class.forName("dcd").getConstructor(int.class,int.class).newInstance(x,z),Class.forName("dvv").getField("a").get(null),ACCESS.getMethod("e",int.class,int.class).invoke(null,min,height),biomeRegistry,null);
 }
 static Object storedProvider(Map<String,ChunkOwner> region)throws Exception {
  Class<?> provider=Class.forName("ddy$a");return Proxy.newProxyInstance(provider.getClassLoader(),new Class<?>[]{provider},(proxy,method,args)->{
   if(!method.getName().equals("getNoiseBiome"))throw new UnsupportedOperationException(method.toString());int x=(int)args[0],y=(int)args[1],z=(int)args[2];ChunkOwner owner=region.get(chunkKey(Math.floorDiv(x,4),Math.floorDiv(z,4)));if(owner==null)throw new IllegalStateException("unavailable actual stored BIOMES dependency");
   int clamped=Math.max(owner.min>>2,Math.min((owner.min>>2)+(owner.height>>2)-1,y)),section=Math.floorDiv(clamped,4)-Math.floorDiv(owner.min,16);Object[] sections=(Object[])Class.forName("duy").getMethod("d").invoke(owner.proto);return Class.forName("dvj").getMethod("c",int.class,int.class,int.class).invoke(sections[section],x&3,clamped&3,z&3);
  });
 }
 static Object runCarvers(JsonObject q)throws Exception {
  String settings=q.get("settings").getAsString(),sourceName=q.get("biome_source").getAsString();long seed=q.get("seed").getAsLong();int cx=q.get("x").getAsInt(),cz=q.get("z").getAsInt(),min=q.get("level_min_y").getAsInt(),height=q.get("level_height").getAsInt();
  Object settingsHolder=getHolder(registry,settings),set=value(settingsHolder),state=S.getMethod("a",Class.forName("jn$a"),K,long.class).invoke(null,getters,key(registry,settings),seed),source=source(sourceName),generator=Class.forName("dzb").getConstructor(Class.forName("dea"),HOLDER).newInstance(source,settingsHolder);
  Map<String,ChunkOwner> region=new LinkedHashMap<>();List<List<Integer>> chunks=new ArrayList<>();List<List<String>> palettes=new ArrayList<>();List<List<List<Integer>>> resolverQueries=new ArrayList<>();
  for(int dx=-1;dx<=1;dx++)for(int dz=-1;dz<=1;dz++){ChunkOwner owner=prepare(cx+dx,cz+dz,min,height,state,set,source);region.put(chunkKey(owner.x,owner.z),owner);chunks.add(List.of(owner.x,owner.z));resolverQueries.add(fillBiomes(owner,source));palettes.add(owner.palette);}
  ChunkOwner center=region.get(chunkKey(cx,cz));Object future=Class.forName("dzb").getMethod("a",BLENDER,S,Class.forName("ddq"),Class.forName("duy")).invoke(generator,BLENDER.getMethod("a").invoke(null),state,null,center.proto);if(((CompletableFuture<?>)future).join()!=center.proto)throw new AssertionError("NOISE replaced ProtoChunk");
  Class<?> managerType=Class.forName("ddy"),providerType=Class.forName("ddy$a");long zoomSeed=(long)managerType.getMethod("a",long.class).invoke(null,seed);Object manager=managerType.getConstructor(providerType,long.class).newInstance(storedProvider(region),zoomSeed),context=Class.forName("dzv").getConstructor(Class.forName("duz"),ACCESS).newInstance(generator,Class.forName("duy").getMethod("z").invoke(center.proto));
  boolean legacy=(boolean)SETTINGS.getMethod("n").invoke(set);Class.forName("dzq").getMethod("a",S,managerType,REGISTRY,boolean.class,Class.forName("dzv"),Class.forName("duy"),CHUNK,Class.forName("dzp$o")).invoke(S.getMethod("c").invoke(state),state,manager,biomeRegistry,legacy,context,center.proto,center.noise,SETTINGS.getMethod("j").invoke(set));Map<String,Object> before=image(center);
  // All 289 actual ProtoChunk cache owners are available in this controlled
  // source loop. Their EMPTY statuses do not assert STRUCTURE_STARTS completion.
  Map<String,Object> neighborOwners=new LinkedHashMap<>();for(int dx=-8;dx<=8;dx++)for(int dz=-8;dz<=8;dz++){ChunkOwner existing=region.get(chunkKey(cx+dx,cz+dz));neighborOwners.put(chunkKey(cx+dx,cz+dz),existing==null?freshProto(cx+dx,cz+dz,min,height):existing.proto);}
  Object rawSampler=S.getMethod("b").invoke(state);Method resolve=Class.forName("dea").getMethod("getNoiseBiome",int.class,int.class,int.class,Class.forName("def$f"));List<Object> rawQueries=new ArrayList<>();
  Function<int[],Object> raw=coordinates->{try{Object biome=resolve.invoke(source,coordinates[0],coordinates[1],coordinates[2],rawSampler);Map<String,Object> row=new LinkedHashMap<>();row.put("kind",coordinates[3]==0?"neighbor_settings":"top_material");row.put("quart",List.of(coordinates[0],coordinates[1],coordinates[2]));row.put("biome",biomeName(biome));rawQueries.add(row);return biome;}catch(Exception e){throw new RuntimeException(e);}};
  Object rawProvider=Proxy.newProxyInstance(providerType.getClassLoader(),new Class<?>[]{providerType},(proxy,method,args)->raw.apply(new int[]{(int)args[0],(int)args[1],(int)args[2],1}));Object rawManager=managerType.getMethod("a",providerType).invoke(manager,rawProvider);
  AtomicLong unique=(AtomicLong)readField(null,Class.forName("dzn"),"d");long uniqueBefore=unique.get(),uniqueSeed=(long)Class.forName("dzn").getMethod("a").invoke(null),uniqueAfter=unique.get();if(uniqueAfter!=uniqueBefore*1181783497276652981L)throw new AssertionError("unique factory not advanced exactly once");
  Class<?> randomType=Class.forName("ayw"),worldgen=Class.forName("dzx"),legacyType=Class.forName("dyz"),stepType=Class.forName("dyu$a"),carverType=Class.forName("ebc");Object random=worldgen.getConstructor(randomType).newInstance(legacyType.getConstructor(long.class).newInstance(uniqueSeed));
  Object retained=Class.forName("duy").getMethod("a",Function.class).invoke(center.proto,(Function<Object,Object>)unused->{throw new AssertionError("lost retained NoiseChunk");});if(retained!=center.noise)throw new AssertionError("different NoiseChunk");
  Object access=Class.forName("ka$c").getConstructor(List.class).newInstance(List.of(biomeRegistry)),carvingContext=Class.forName("eaz").getConstructor(Class.forName("dzb"),Class.forName("ka"),ACCESS,CHUNK,S,Class.forName("dzp$o")).newInstance(generator,access,Class.forName("duy").getMethod("z").invoke(center.proto),center.noise,state,SETTINGS.getMethod("j").invoke(set)),step=stepType.getField("a").get(null),mask=PROTO.getMethod("b",stepType).invoke(center.proto,step);
  Method cached=Class.forName("duy").getMethod("a",Supplier.class),carvers=Class.forName("ddx").getMethod("a",stepType),seedRandom=worldgen.getMethod("c",long.class,int.class,int.class),count=worldgen.getMethod("l"),start=carverType.getMethod("a",randomType),carve=carverType.getMethod("a",Class.forName("eaz"),Class.forName("duy"),Function.class,randomType,Class.forName("dyk"),Class.forName("dcd"),Class.forName("dux")),getBiome=managerType.getMethod("a",POSITION);
  Function<Object,Object> biomeAt=point->{try{return getBiome.invoke(rawManager,point);}catch(Exception e){throw new RuntimeException(e);}};List<Object> neighborTrace=new ArrayList<>(),attempts=new ArrayList<>();Map<String,Integer> additions=new TreeMap<>();BitSet storedMask=(BitSet)readField(mask,Class.forName("dux"),"b");
  for(int dx=-8;dx<=8;dx++)for(int dz=-8;dz<=8;dz++){
   int x=cx+dx,z=cz+dz;Object owner=neighborOwners.get(chunkKey(x,z));boolean hit=readField(owner,Class.forName("duy"),"r")!=null;String[] resolved={null};
   Supplier<Object> supplier=()->{try{Object biome=raw.apply(new int[]{x<<2,0,z<<2,0});resolved[0]=biomeName(biome);return Class.forName("duz").getMethod("a",HOLDER).invoke(generator,biome);}catch(Exception e){throw new RuntimeException(e);}};
   Object generation=cached.invoke(owner,supplier);List<String> ids=new ArrayList<>();List<Object> configs=new ArrayList<>();for(Object holder:(Iterable<?>)carvers.invoke(generation,step)){ids.add(biomeName(holder));configs.add(value(holder));}Map<String,Object> neighbor=new LinkedHashMap<>();neighbor.put("chunk",List.of(x,z));neighbor.put("cached",hit);neighbor.put("resolved_biome",resolved[0]);neighbor.put("air",ids);neighborTrace.add(neighbor);
   Object origin=Class.forName("dcd").getConstructor(int.class,int.class).newInstance(x,z);
   for(int index=0;index<configs.size();index++){Object configured=configs.get(index);seedRandom.invoke(random,seed+index,x,z);int afterSeed=(int)count.invoke(random);boolean isStart=(boolean)start.invoke(configured,random);int afterStart=(int)count.invoke(random);int cardinalityBefore=storedMask.cardinality();Object carved=isStart?carve.invoke(configured,carvingContext,center.proto,biomeAt,random,center.aquifer,origin,mask):null;if(isStart)additions.merge(ids.get(index),storedMask.cardinality()-cardinalityBefore,Integer::sum);Map<String,Object> attempt=new LinkedHashMap<>();attempt.put("chunk",List.of(x,z));attempt.put("index",index);attempt.put("key",ids.get(index));attempt.put("count_after_seed",afterSeed);attempt.put("start",isStart);attempt.put("count_after_start",afterStart);attempt.put("carved",carved);attempt.put("count_after_carve",count.invoke(random));attempts.add(attempt);}
  }
  // A second access must hit the same source-owned generation-settings object.
  boolean warmed=true;for(Object owner:neighborOwners.values())warmed&=cached.invoke(owner,(Supplier<Object>)()->{throw new AssertionError("cache hit evaluated supplier");})==readField(owner,Class.forName("duy"),"r");
  Map<String,Object> after=image(center);long[] bits=(long[])Class.forName("dux").getMethod("a").invoke(mask);int cardinality=0;for(long word:bits)cardinality+=Long.bitCount(word);Object underlying=readField(random,worldgen,"d");long state48=((AtomicLong)readField(underlying,legacyType,"h")).get();
  boolean sameOwners=true,biomesUnchanged=true,statusesEmpty=true;for(ChunkOwner owner:region.values()){sameOwners&=sameOwners(owner.interpolators,owners(owner.noise,"i"))&&sameOwners(owner.cells,owners(owner.noise,"j"))&&owner.aquifer==CHUNK.getMethod("i").invoke(owner.noise);biomesUnchanged&=owner.palette.equals(biomes(owner.proto));}for(Object owner:neighborOwners.values())statusesEmpty&=Class.forName("dvz").getMethod("f").invoke(PROTO.getMethod("j").invoke(owner)).toString().equals("minecraft:empty");
  String prefix=settings.replace(':','-')+"-"+sourceName.replace(':','-')+"-"+seed+"-"+cx+"-"+cz;Path beforeFile=outputDirectory.resolve("java-before-carvers-"+prefix+".bin"),afterFile=outputDirectory.resolve("java-after-carvers-"+prefix+".bin"),palettesFile=outputDirectory.resolve("java-carver-palettes-"+prefix+".json"),resolverFile=outputDirectory.resolve("java-carver-biome-queries-"+prefix+".json");Files.write(beforeFile,(byte[])before.remove("blocks"));Files.write(afterFile,(byte[])after.remove("blocks"));Files.writeString(palettesFile,JSON.toJson(palettes));Files.writeString(resolverFile,JSON.toJson(resolverQueries));
  Map<String,Object> report=new LinkedHashMap<>();report.put("neighbors",neighborTrace);report.put("attempts",attempts);report.put("biome_queries",rawQueries);report.put("random_count",count.invoke(random));report.put("random_state48",state48);Map<String,Object> result=new LinkedHashMap<>();result.put("settings",settings);result.put("biome_source",sourceName);result.put("seed",seed);result.put("x",cx);result.put("z",cz);result.put("level_min_y",min);result.put("level_height",height);result.put("chunks",chunks);result.put("use_legacy_random_source",legacy);result.put("palettes_file",palettesFile.toString());result.put("resolver_queries_file",resolverFile.toString());result.put("before_file",beforeFile.toString());result.put("after_file",afterFile.toString());result.put("before",before);result.put("after",after);result.put("report",report);result.put("mask",bits);result.put("mask_cardinality",cardinality);result.put("mask_additions_by_carver",additions);result.put("unique_factory_advances_once",true);result.put("neighbor_cache_hits_suppress_supplier",warmed);result.put("same_noise_cache_aquifer_owners",sameOwners);result.put("biomes_unchanged",biomesUnchanged);result.put("all_289_statuses_remain_empty",statusesEmpty);result.put("tag_membership_witness",tagWitness());result.put("tag_source_sha256",TAG_DIGESTS);result.put("oracle_inner_sha256",originalSha);return result;
 }
 public static void main(String[] args)throws Exception {
  if(args.length!=1)throw new IllegalArgumentException("usage: WorldgenCarverOracle <output-directory>");outputDirectory=Path.of(args[0]).toAbsolutePath().normalize();Files.createDirectories(outputDirectory);setup();
  originalSha=sha(Files.readAllBytes(Path.of(CHUNK.getProtectionDomain().getCodeSource().getLocation().toURI())));if(!originalSha.equals(INNER))throw new AssertionError("wrong pinned original NoiseChunk artifact");
  BufferedReader input=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=input.readLine())!=null;)try {
   JsonObject query=JsonParser.parseString(line).getAsJsonObject();String mode=query.has("mode")?query.get("mode").getAsString():"carvers";
   if(mode.equals("tags")){bindBuiltinTags();OUT.println(JSON.toJson(Map.of("witness",tagWitness(),"tag_digests",TAG_DIGESTS)));}else if(mode.equals("providers"))OUT.println(JSON.toJson(providers()));else {if(TAG_DIGESTS.isEmpty())bindBuiltinTags();OUT.println(JSON.toJson(runCarvers(query)));}
  }catch(InvocationTargetException e){e.getCause().printStackTrace();System.exit(1);}
 }
}
