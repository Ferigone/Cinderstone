import com.google.gson.*;
import com.mojang.serialization.JsonOps;
import com.mojang.serialization.MapCodec;
import it.unimi.dsi.fastutil.shorts.ShortList;
import it.unimi.dsi.fastutil.objects.ObjectArrayList;
import java.io.*;
import java.lang.reflect.*;
import java.nio.*;
import java.nio.file.*;
import java.security.*;
import java.util.*;
import java.util.concurrent.*;
import java.util.function.*;

/** Controlled existing ProtoChunk observations; not a saved-world or status oracle. */
public final class WorldgenExistingTargetOracle {
 static final PrintStream OUT=System.out;
 static final Gson JSON=new GsonBuilder().disableHtmlEscaping().create();
 static final String INNER="8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01";
 static Path output;
 static final Map<String,dea> sources=new LinkedHashMap<>();
 static jz<ddw> biomeRegistry;
 static Object field(Object owner,Class<?> type,String name)throws Exception {Field f=type.getDeclaredField(name);f.setAccessible(true);return f.get(owner);}
 static Object shared(String name,Class<?>[] types,Object...args)throws Exception {
  Method m=Class.forName("oracle.WorldgenSurfaceOracle").getDeclaredMethod(name,types);m.setAccessible(true);return m.invoke(null,args);
 }
 static Object sharedField(String name)throws Exception {Field f=Class.forName("oracle.WorldgenSurfaceOracle").getDeclaredField(name);f.setAccessible(true);return f.get(null);}
 static String sha(byte[] bytes)throws Exception{return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));}
 static void setup()throws Exception {
  shared("setup",new Class<?>[0]);
  biomeRegistry=(jz<ddw>)sharedField("biomeRegistry");
  String actual=sha(Files.readAllBytes(Path.of(dtc.class.getProtectionDomain().getCodeSource().getLocation().toURI())));
  if(!INNER.equals(actual))throw new AssertionError("wrong original block-state artifact: "+actual);
 }
 static void exportStateFacts(Path catalog,Path destination)throws Exception {
  byte[] catalogBytes=Files.readAllBytes(catalog);JsonObject placement=JsonParser.parseString(new String(catalogBytes,java.nio.charset.StandardCharsets.UTF_8)).getAsJsonObject();
  if(!placement.get("minecraft").getAsString().equals("1.21")||!placement.get("jar_sha256").getAsString().equals(INNER))throw new AssertionError("wrong placement catalog artifact");
  int count=dfy.q.c();boolean[] covered=new boolean[count];
  for(JsonElement entry:placement.getAsJsonArray("blocks")) {
   JsonObject block=entry.getAsJsonObject();dfy original=lt.e.a(akr.a(block.get("name").getAsString()));
   if(original==null||dfy.i(original.o())!=block.get("default").getAsInt())throw new AssertionError("catalog default differs: "+block.get("name"));
   for(int i=block.get("min").getAsInt();i<=block.get("max").getAsInt();i++) {
    if(i<0||i>=count||covered[i]||dfy.a(i).b()!=original||dfy.i(dfy.a(i))!=i)throw new AssertionError("catalog canonical range differs at "+i);
    covered[i]=true;
   }
  }
  List<Object> states=new ArrayList<>(count);
  for(int i=0;i<count;i++) {
   if(!covered[i])throw new AssertionError("catalog missing canonical state "+i);
   dtc state=dfy.q.a(i);if(state==null||dfy.i(state)!=i)throw new AssertionError("noncontiguous state registry");
   Map<String,Object> facts=new LinkedHashMap<>();facts.put("is_air",state.i());facts.put("has_fluid",!state.u().c());facts.put("randomly_ticking_block",state.v());facts.put("randomly_ticking_fluid",state.u().f());facts.put("world_surface_wg_opaque",dyy.a.a.e().test(state));facts.put("ocean_floor_wg_opaque",dyy.a.c.e().test(state));states.add(facts);
  }
  Map<String,Object> defaults=new LinkedHashMap<>();defaults.put("air",dfy.i(dga.a.o()));defaults.put("void_air",dfy.i(dga.nb.o()));defaults.put("bedrock",dfy.i(dga.F.o()));defaults.put("deepslate",dfy.i(dga.sJ.o()));
  Map<String,Object> result=new LinkedHashMap<>();result.put("minecraft","1.21");result.put("protocol",767);result.put("inner_sha256",INNER);result.put("catalog_path","fixtures/placement-1.21.json");result.put("catalog_sha256",sha(catalogBytes));result.put("state_count",count);result.put("defaults",defaults);result.put("layout","states[canonical Block.getId(state)]");result.put("states",states);
  Files.writeString(destination,JSON.toJson(result)+"\n");OUT.println(JSON.toJson(Map.of("state_facts_file",destination.toString(),"sha256",sha(Files.readAllBytes(destination)),"state_count",count,"defaults",defaults,"catalog_sha256",sha(catalogBytes))));
 }
 static Path inputPath(String name){Path p=Path.of(name);return p.isAbsolute()?p:output.resolve(p);}
 static String biomeName(jm<ddw> biome){return biome.e().orElseThrow().a().toString();}
 @SuppressWarnings("unchecked") static jm<ddw> biome(String name)throws Exception {
  jm<ddw> result=(jm<ddw>)shared("getHolder",new Class<?>[]{Object.class,String.class},sharedField("biomeKey"),name);
  if(!biomeName(result).equals(name))throw new IllegalArgumentException("canonical registered biome required: "+name);
  return result;
 }
 static dea source(String name)throws Exception {
  dea source=sources.get(name);if(source!=null)return source;
  Method m=Class.forName("oracle.WorldgenBiomeStageOracle").getDeclaredMethod("source",String.class);m.setAccessible(true);source=(dea)m.invoke(null,name);sources.put(name,source);return source;
 }
 static final class ObservedSource extends dea {
  final dea actual;final List<Object> queries=new ArrayList<>();def.f firstSampler;int differentSamplers;
  ObservedSource(dea actual){this.actual=actual;}
  @Override protected MapCodec<? extends dea> a(){throw new UnsupportedOperationException("observer serialization is outside this controlled query");}
  @Override protected java.util.stream.Stream<jm<ddw>> b(){return actual.c().stream();}
  @Override public jm<ddw> getNoiseBiome(int x,int y,int z,def.f sampler){
   if(firstSampler==null)firstSampler=sampler;else if(firstSampler!=sampler)differentSamplers++;
   jm<ddw> value=actual.getNoiseBiome(x,y,z,sampler);queries.add(List.of(x,y,z,biomeName(value)));return value;
  }
 }
 static final class Owner {
  final int x,z,min,height,physicalMin,physicalHeight;final dvs proto;
  Owner(int x,int z,int min,int height)throws Exception {
   if(height<=0)throw new IllegalArgumentException("positive height required");Math.addExact(min,height);
   this.x=x;this.z=z;this.min=min;this.height=height;dcy bounds=dcy.e(min,height);physicalMin=bounds.ao()*16;physicalHeight=bounds.an()*16;
   proto=new dvs(new dcd(x,z),dvv.a,bounds,biomeRegistry,null);
  }
  void loadBiomes(Path file,def.f sampler)throws Exception {
   JsonArray input=JsonParser.parseString(Files.readString(file)).getAsJsonArray();if(input.size()!=proto.d().length*64)throw new IllegalArgumentException("biome image physical section length mismatch");
   List<jm<ddw>> values=new ArrayList<>();for(JsonElement name:input)values.add(biome(name.getAsString()));
   for(int index=0;index<proto.d().length;index++) {
    final int at=index;proto.d()[index].a((qx,qy,qz,s)->values.get(at*64+(qy&3)*16+(qz&3)*4+(qx&3)),sampler,x*4,(proto.ao()+index)*4,z*4);
   }
  }
  void loadBlocks(Path file)throws Exception {
   byte[] bytes=Files.readAllBytes(file);if(bytes.length!=physicalHeight*256*4)throw new IllegalArgumentException("block image physical section length mismatch");ByteBuffer data=ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN);
   for(int y=physicalMin;y<physicalMin+physicalHeight;y++)for(int x=0;x<16;x++)for(int z=0;z<16;z++) {
    int id=data.getInt();if(id<0||id>=dfy.q.c())throw new IllegalArgumentException("invalid canonical state ID "+Integer.toUnsignedString(id));proto.d()[Math.floorDiv(y,16)-proto.ao()].a(x,y&15,z,dfy.q.a(id),false);
   }
  }
  jm<ddw> rawBiome(int qx,int qy,int qz) {
   int low=min>>2,high=low+(height>>2)-1;int y=Math.min(Math.max(qy,low),high);return proto.d()[Math.floorDiv(y,4)-proto.ao()].c(qx&3,y&3,qz&3);
  }
 }
 static List<String> palette(Owner owner){List<String> result=new ArrayList<>();for(dvj section:owner.proto.d())for(int y=0;y<4;y++)for(int z=0;z<4;z++)for(int x=0;x<4;x++)result.add(biomeName(section.c(x,y,z)));return result;}
 static Map<String,Object> snapshot(Owner owner,String id,String phase)throws Exception {
  Path blocks=output.resolve("java-target-"+id+"-"+phase+".bin"),biomes=output.resolve("java-target-"+id+"-"+phase+"-biomes.json");ByteBuffer bytes=ByteBuffer.allocate(owner.physicalHeight*256*4).order(ByteOrder.LITTLE_ENDIAN);
  for(int y=owner.physicalMin;y<owner.physicalMin+owner.physicalHeight;y++)for(int x=0;x<16;x++)for(int z=0;z<16;z++)bytes.putInt(dfy.i(owner.proto.d()[Math.floorDiv(y,16)-owner.proto.ao()].a(x,y&15,z)));Files.write(blocks,bytes.array());Files.writeString(biomes,JSON.toJson(palette(owner)));
  List<Integer> ws=new ArrayList<>(),of=new ArrayList<>();for(int z=0;z<16;z++)for(int x=0;x<16;x++){ws.add(owner.proto.a(dyy.a.a,x,z)+1);of.add(owner.proto.a(dyy.a.c,x,z)+1);}
  List<Object> posts=new ArrayList<>(),counts=new ArrayList<>();List<Boolean> allocated=new ArrayList<>();int postCount=0;
  for(int section=0;section<owner.proto.d().length;section++) {
   ShortList queue=owner.proto.n()[section];List<Integer> row=new ArrayList<>();if(queue!=null)for(short p:queue){row.add((int)p);postCount++;}posts.add(row);allocated.add(queue!=null);
   dvj s=owner.proto.d()[section];counts.add(Map.of("non_empty_block_count",(short)field(s,dvj.class,"e"),"ticking_block_count",(short)field(s,dvj.class,"f"),"ticking_fluid_count",(short)field(s,dvj.class,"g")));
  }
  Map<String,Object> r=new LinkedHashMap<>();r.put("blocks_file",blocks.toString());r.put("blocks_sha256",sha(bytes.array()));r.put("biomes_file",biomes.toString());r.put("biomes_sha256",sha(Files.readAllBytes(biomes)));r.put("physical_min_y",owner.physicalMin);r.put("physical_height",owner.physicalHeight);r.put("world_surface_wg",ws);r.put("ocean_floor_wg",of);r.put("posts",posts);r.put("post_count",postCount);r.put("post_allocated",allocated);r.put("section_counts",counts);r.put("persisted_status",owner.proto.j().f());r.put("highest_generated_status",owner.proto.k().f());r.put("retrogen_present",owner.proto.x()!=null);r.put("generation_min_y",owner.proto.z().I_());r.put("generation_height",owner.proto.z().J_());r.put("unsaved",owner.proto.i());r.put("debug_void_terrain",ab.a(owner.proto.f()));return r;
 }
 static dym retrogen(JsonObject input)throws Exception {
  JsonObject source=input.deepCopy();if(source.has("missing_bedrock")){JsonArray signed=new JsonArray();for(JsonElement word:source.getAsJsonArray("missing_bedrock")){String text=word.getAsString();signed.add(text.startsWith("-")?Long.parseLong(text):Long.parseUnsignedLong(text));}source.add("missing_bedrock",signed);}
  return dym.a.parse(JsonOps.INSTANCE,source).getOrThrow();
 }
 static List<?> cacheOwners(dzc noise,String name)throws Exception{return new ArrayList<>((List<?>)field(noise,dzc.class,name));}
 static boolean sameOwners(List<?> before,List<?> after){if(before.size()!=after.size())return false;for(int i=0;i<before.size();i++)if(before.get(i)!=after.get(i))return false;return true;}
 static int structureInt(JsonObject input,String key) {
  if(!input.has(key)||!input.get(key).isJsonPrimitive()||!input.getAsJsonPrimitive(key).isNumber())throw new IllegalArgumentException("integer structure field required: "+key);
  return input.get(key).getAsBigDecimal().intValueExact();
 }
 static dyl beardifier(JsonObject query)throws Exception {
  ObjectArrayList<dyl.a> rigid=new ObjectArrayList<>();ObjectArrayList<eku> junctions=new ObjectArrayList<>();
  if(query.has("beardifier")) {
   JsonObject input=query.getAsJsonObject("beardifier");
   if(!input.has("rigids")||!input.has("junctions"))throw new IllegalArgumentException("ordered rigid/junction arrays required");
   for(JsonElement element:input.getAsJsonArray("rigids")) {
    JsonObject part=element.getAsJsonObject();JsonArray box=part.getAsJsonArray("box");if(box.size()!=6)throw new IllegalArgumentException("six bounding coordinates required");
    int[] bounds=new int[6];for(int i=0;i<6;i++)bounds[i]=box.get(i).getAsBigDecimal().intValueExact();
    for(int axis=0;axis<3;axis++)if(bounds[axis]>bounds[axis+3])throw new IllegalArgumentException("ordered bounding coordinates required");
    String name=part.get("terrain_adjustment").getAsString();ekc adjustment=null;for(ekc candidate:ekc.values())if(candidate.c().equals(name)){adjustment=candidate;break;}
    if(adjustment==null)throw new IllegalArgumentException("unsupported actual terrain adjustment: "+name);
    rigid.add(new dyl.a(new ejj(bounds[0],bounds[1],bounds[2],bounds[3],bounds[4],bounds[5]),adjustment,structureInt(part,"ground_level_delta")));
   }
   for(JsonElement element:input.getAsJsonArray("junctions")) {
    JsonObject part=element.getAsJsonObject();
    // The actual Beardifier reads only these three coordinates. Remaining
    // original Junction fields are explicit fixture values, not placement data.
    elb.a projection=(elb.a)Class.forName("elb$a").getField("b").get(null);
    junctions.add(new eku(structureInt(part,"source_x"),structureInt(part,"source_ground_y"),structureInt(part,"source_z"),0,projection));
   }
  }
  return new dyl(rigid.iterator(),junctions.iterator());
 }
 static Map<String,Object> beardifierObservations(JsonObject query,dyl owner)throws Exception {
  String actual=sha(Files.readAllBytes(Path.of(owner.getClass().getProtectionDomain().getCodeSource().getLocation().toURI())));
  if(!INNER.equals(actual))throw new AssertionError("wrong actual Beardifier artifact: "+actual);
  Map<String,Object> result=new LinkedHashMap<>();result.put("beardifier_input_kind",query.has("beardifier")?"actual_ordered_beardifier":"actual_empty_beardifier");result.put("structure_owner_class",owner.getClass().getName());result.put("structure_owner_jar_sha256",actual);
  result.put("beardifier_min_bits",Long.toUnsignedString(Double.doubleToRawLongBits(owner.a())));result.put("beardifier_max_bits",Long.toUnsignedString(Double.doubleToRawLongBits(owner.b())));
  List<Object> samples=new ArrayList<>();
  if(query.has("beardifier_points"))for(JsonElement element:query.getAsJsonArray("beardifier_points")) {
   JsonArray p=element.getAsJsonArray();if(p.size()!=3)throw new IllegalArgumentException("three point coordinates required");
   int x=p.get(0).getAsBigDecimal().intValueExact(),y=p.get(1).getAsBigDecimal().intValueExact(),z=p.get(2).getAsBigDecimal().intValueExact();
   samples.add(List.of(x,y,z,Long.toUnsignedString(Double.doubleToRawLongBits(owner.a(new dyr.e(x,y,z))))));
  }
  result.put("beardifier_samples",samples);return result;
 }
 static void noiseTaskContinuation(Owner owner)throws Exception {Method callback=dwb.class.getDeclaredMethod("b",duy.class);callback.setAccessible(true);if(callback.invoke(null,owner.proto)!=owner.proto)throw new AssertionError("NOISE task callback replaced ProtoChunk");}
 @SuppressWarnings("unchecked") static Object run(JsonObject q)throws Exception {
  String id=q.get("id").getAsString();if(!id.matches("[A-Za-z0-9_-]+"))throw new IllegalArgumentException("safe unique case id required");
  String settingName=q.get("settings").getAsString();long seed=q.get("seed").getAsLong();int cx=q.get("x").getAsInt(),cz=q.get("z").getAsInt();
  jm<dzd> settingsHolder=(jm<dzd>)shared("getHolder",new Class<?>[]{Object.class,String.class},sharedField("registry"),settingName);dzd settings=settingsHolder.a();Object settingsKey=shared("key",new Class<?>[]{Object.class,String.class},sharedField("registry"),settingName);
  boolean skipBiomes=q.has("skip_biomes")&&q.get("skip_biomes").getAsBoolean();boolean sourceAbsent=skipBiomes&&!q.has("biome_source");dzm state=skipBiomes?null:(dzm)dzm.class.getMethod("a",jn.a.class,akq.class,long.class).invoke(null,sharedField("getters"),settingsKey,seed);ObservedSource source=sourceAbsent?null:new ObservedSource(source(q.get("biome_source").getAsString()));dzb generator=new dzb(source,settingsHolder);eaa blender=skipBiomes?null:eaa.a();
  Owner owner=new Owner(cx,cz,q.get("level_min_y").getAsInt(),q.get("level_height").getAsInt());owner.loadBlocks(inputPath(q.get("blocks_file").getAsString()));
  // This fixture installer only returns explicit stored holders. The supplied
  // sampler is never evaluated; do not prewarm the NoiseChunk's climate graph.
  owner.loadBiomes(inputPath(q.get("biomes_file").getAsString()),state==null?null:state.b());
  JsonArray initialPosts=q.getAsJsonArray("posts");if(initialPosts.size()!=owner.proto.n().length)throw new IllegalArgumentException("post queue physical section length mismatch");for(int section=0;section<initialPosts.size();section++)if(!initialPosts.get(section).isJsonNull()){ShortList list=duy.a(owner.proto.n(),section);for(JsonElement value:initialPosts.get(section).getAsJsonArray()){int packed=value.getAsInt();if(packed<Short.MIN_VALUE||packed>0xffff)throw new IllegalArgumentException("invalid packed short");list.add((short)packed);}}
  if(q.has("retrogen"))owner.proto.a(retrogen(q.getAsJsonObject("retrogen")));dyy.a(owner.proto,EnumSet.of(dyy.a.a,dyy.a.c));
  Map<String,Object> result=new LinkedHashMap<>();result.put("query",q);result.put("oracle_inner_sha256",INNER);result.put("source_biome_source_absent",sourceAbsent);result.put("debug_void_terrain",ab.a(owner.proto.f()));result.put("initial",snapshot(owner,id,"initial"));
  dzg fillSettings=settings.f().a(owner.proto.z());int fillCellCount=Math.floorDiv(fillSettings.d(),fillSettings.a());result.put("fill_cell_count_y",fillCellCount);
  if(skipBiomes) {
   if(q.has("surface_neighbors"))throw new IllegalArgumentException("skip_biomes NoCells profile excludes SURFACE");if(fillCellCount>0)throw new IllegalArgumentException("skip_biomes requires actual original NoCells clamp");
   if(field(owner.proto,duy.class,"e")!=null)throw new AssertionError("NoCells initial cache was not absent");
   if(generator.a((eaa)null,(dzm)null,null,owner.proto).join()!=owner.proto)throw new AssertionError("NoCells replaced ProtoChunk");result.put("noise_before_retrogen",snapshot(owner,id,"noise-before-retrogen"));noiseTaskContinuation(owner);result.put("noise",snapshot(owner,id,"noise"));
   // Controlled null-argument early-return probe; not a production generator
   // configuration. The original constructor stores its source lazily.
   result.put("no_cells_entry","controlled_null_argument_early_return_probe");result.put("original_noise_task_callback_executed",true);result.put("biomes_ran",false);result.put("surface_ran",false);result.put("biome_queries",source==null?List.of():source.queries);result.put("noise_chunk_absent",field(owner.proto,duy.class,"e")==null);result.put("persisted_status_unchanged",owner.proto.j()==dvz.c);return result;
  }
  Method pickerMethod=dzb.class.getDeclaredMethod("a",dzd.class);pickerMethod.setAccessible(true);dyk.a picker=(dyk.a)pickerMethod.invoke(null,settings);dyl beard=beardifier(q);result.putAll(beardifierObservations(q,beard));dzc noise=dzc.a(owner.proto,state,beard,settings,picker,blender);if(owner.proto.a((Function<duy,dzc>)unused->noise)!=noise)throw new AssertionError("NC install changed owner");dyk aquifer=noise.i();
  // Execute the exact original body on this controlled worker. Its public
  // async wrapper otherwise chooses an arbitrary ThreadLocal RTree cursor.
  Method createBiomes=dzb.class.getDeclaredMethod("b",eaa.class,dzm.class,ddq.class,duy.class);createBiomes.setAccessible(true);createBiomes.invoke(generator,blender,state,null,owner.proto);result.put("biomes_ran",true);result.put("biomes_entry","original_private_doCreateBiomes_controlled_one_worker");result.put("biomes",snapshot(owner,id,"biomes"));List<?> interpolators=cacheOwners(noise,"i"),cells=cacheOwners(noise,"j");result.put("biome_queries",source.queries);result.put("biomes_same_sampler",source.firstSampler!=null&&source.differentSamplers==0);result.put("same_beardifier_after_biomes",field(noise,dzc.class,"s")==beard);
  if(generator.a(blender,state,null,owner.proto).join()!=owner.proto)throw new AssertionError("NOISE replaced ProtoChunk");result.put("noise_before_retrogen",snapshot(owner,id,"noise-before-retrogen"));
  noiseTaskContinuation(owner);result.put("noise",snapshot(owner,id,"noise"));result.put("original_noise_task_callback_executed",true);result.put("same_beardifier_after_noise",field(noise,dzc.class,"s")==beard);
  List<String> centerPalette=palette(owner);List<Object> surfaceQueries=new ArrayList<>();
  if(q.has("surface_neighbors")) {
   Map<String,Owner> neighbors=new LinkedHashMap<>();neighbors.put(cx+","+cz,owner);List<List<String>> neighborBefore=new ArrayList<>();List<Owner> inputNeighbors=new ArrayList<>();
   for(JsonElement description:q.getAsJsonArray("surface_neighbors")){JsonObject n=description.getAsJsonObject();Owner neighbor=new Owner(n.get("x").getAsInt(),n.get("z").getAsInt(),n.get("level_min_y").getAsInt(),n.get("level_height").getAsInt());if(neighbors.putIfAbsent(neighbor.x+","+neighbor.z,neighbor)!=null)throw new IllegalArgumentException("duplicate/center surface neighbor");neighbor.loadBiomes(inputPath(n.get("biomes_file").getAsString()),state.b());inputNeighbors.add(neighbor);neighborBefore.add(palette(neighbor));}
   ddy manager=new ddy((qx,qy,qz)->{Owner n=neighbors.get(Math.floorDiv(qx,4)+","+Math.floorDiv(qz,4));if(n==null)throw new IllegalStateException("missing supplied SURFACE palette owner for "+qx+","+qy+","+qz);jm<ddw> value=n.rawBiome(qx,qy,qz);surfaceQueries.add(List.of(qx,qy,qz,biomeName(value)));return value;},ddy.a(seed));
   generator.a(owner.proto,new dzv(generator,owner.proto),state,null,manager,biomeRegistry,blender);result.put("surface",snapshot(owner,id,"surface"));for(int i=0;i<inputNeighbors.size();i++)if(!neighborBefore.get(i).equals(palette(inputNeighbors.get(i))))throw new AssertionError("SURFACE mutated supplied foreign palette");
  }
  result.put("surface_biome_queries",surfaceQueries);result.put("surface_ran",q.has("surface_neighbors"));result.put("surface_stored_biomes_unchanged",centerPalette.equals(palette(owner)));result.put("same_beardifier_after_surface",field(noise,dzc.class,"s")==beard);
  result.put("same_noise_chunk",owner.proto.a((Function<duy,dzc>)unused->{throw new AssertionError("lost NC");})==noise);result.put("same_aquifer",noise.i()==aquifer);result.put("same_blender",noise.d()==blender);result.put("same_cache_owners",sameOwners(interpolators,cacheOwners(noise,"i"))&&sameOwners(cells,cacheOwners(noise,"j")));result.put("persisted_status_unchanged",owner.proto.j()==dvz.c);
  dzg grid=(dzg)field(noise,dzc.class,"a");result.put("noise_grid_min_y",grid.c());result.put("noise_grid_height",grid.d());result.put("source_use_legacy_random_source",settings.n());return result;
 }
 public static void main(String[]args)throws Exception {
  if(args.length==3&&args[0].equals("--export-state-facts")){setup();exportStateFacts(Path.of(args[1]),Path.of(args[2]));return;}
  if(args.length!=1)throw new IllegalArgumentException("usage: WorldgenExistingTargetOracle <output-directory> OR --export-state-facts <placement-catalog> <output-json>");output=Path.of(args[0]).toAbsolutePath().normalize();Files.createDirectories(output);setup();BufferedReader input=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=input.readLine())!=null;)try{OUT.println(JSON.toJson(run(JsonParser.parseString(line).getAsJsonObject())));}catch(InvocationTargetException e){e.getCause().printStackTrace();System.exit(1);}
 }
}
