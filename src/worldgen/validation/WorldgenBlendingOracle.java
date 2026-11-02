import com.google.gson.*;
import it.unimi.dsi.fastutil.longs.Long2ObjectOpenHashMap;
import it.unimi.dsi.fastutil.shorts.ShortList;
import it.unimi.dsi.fastutil.HashCommon;
import com.mojang.serialization.JsonOps;
import java.io.*;
import java.lang.reflect.*;
import java.nio.*;
import java.nio.file.*;
import java.util.*;
import java.util.function.*;

/** Original calculated old-data snapshots and retained nonempty NoiseChunk.
 * Snapshot owners are controlled ChunkAccess storage, not completed ProtoChunks
 * or WorldGenRegion/ChunkStep. Public Blender.of(region) is not executed. */
public class WorldgenBlendingOracle extends WorldgenOreOracle {
 static dea workerSource;
 static List<Object> ageQueries;
 static int blockReads,biomeReads;
 static String bits(double value){return Long.toUnsignedString(Double.doubleToRawLongBits(value));}
 static List<String> bits(double[] values){List<String> r=new ArrayList<>();for(double v:values)r.add(bits(v));return r;}
 static String biomeName(jm<ddw> holder)throws Exception {return holder.e().orElseThrow().a().toString();}
 static Object inherited(String name)throws Exception{return field(null,Class.forName("oracle.WorldgenSurfaceOracle"),name);}
 static Object source(String name)throws Exception {Method m=Class.forName("oracle.WorldgenBiomeStageOracle").getDeclaredMethod("source",String.class);m.setAccessible(true);return m.invoke(null,name);}
 static eab data(JsonObject q)throws Exception {
  Constructor<eab> c=eab.class.getDeclaredConstructor(int.class,int.class,Optional.class);c.setAccessible(true);Optional<double[]> heights=Optional.empty();
  if(q.has("heights")){double[] h=new double[16];for(int i=0;i<16;i++)h[i]=q.getAsJsonArray("heights").get(i).getAsDouble();heights=Optional.of(h);}
  return c.newInstance(q.get("min_section").getAsInt(),q.get("max_section").getAsInt(),heights);
 }
 static final class Snapshot extends duy {
  final int profile;int localBlockReads,localBiomeReads;
  Snapshot(int x,int z,int profile,eab data)throws Exception {
   super(new dcd(x,z),dvv.a,dcy.e(-64,384),biomeRegistry,0L,null,data);this.profile=profile;
   if(data!=null)for(int y=0;y<256;y++)for(int z0=0;z0<16;z0++)for(int x0=0;x0<16;x0++) {
    dtc value=material(x0,y,z0);if(!value.i())m[e(y)].a(x0,y&15,z0,value,false);
   }
   dyy.a(this,EnumSet.of(dyy.a.a,dyy.a.c));
  }
  dtc material(int x,int y,int z){
   int top=60+profile*17+Math.floorMod(x*3+z*5,7);
   if(y>top)return y<63?state("water"):state("air");
   if(y==top)return state("grass_block");
   if(y>top-4)return state("dirt");
   if(y>=20+profile&&y<=23+profile&&((x+z)&3)==0)return state("air");
   return state("stone");
  }
  @Override public dtc a_(jd p){localBlockReads++;blockReads++;if(p.v()<I_()||p.v()>=am())return state("void_air");return m[e(p.v())].a(p.u()&15,p.v()&15,p.w()&15);}
  @Override public epe b_(jd p){return a_(p).u();}
  @Override public dqh c_(jd p){if(a_(p).t())throw new UnsupportedOperationException("block entity owner unavailable");return null;}
  @Override public jm<ddw> getNoiseBiome(int x,int y,int z){localBiomeReads++;biomeReads++;return super.getNoiseBiome(x,y,z);}
  @Override public dvz j(){return dvz.c;}
  @Override public dtc a(jd p,dtc s,boolean flag){throw new UnsupportedOperationException("snapshot mutation unsupported");}
  @Override public void a(dqh entity){throw new UnsupportedOperationException("snapshot block entity mutation");}
  @Override public void a(bsr entity){throw new UnsupportedOperationException("snapshot entity insertion");}
  @Override public void d(jd p){throw new UnsupportedOperationException("snapshot block entity removal");}
  @Override public ub a(jd p,jo.a registry){throw new UnsupportedOperationException("snapshot persistence");}
  @Override public eyw<dfy> o(){throw new UnsupportedOperationException("snapshot block ticks");}
  @Override public eyw<epd> p(){throw new UnsupportedOperationException("snapshot fluid ticks");}
  @Override public duy.a q(){throw new UnsupportedOperationException("snapshot tick persistence");}
 }
 static String key(int x,int z){return x+","+z;}
 static long packed(int x,int z){return (x&0xffffffffL)|((z&0xffffffffL)<<32);}
 static Map<String,Object> dataImage(Snapshot owner,eab data,Set<jj> sides)throws Exception {
  double[] heights=(double[])field(data,eab.class,"s");double[][] density=(double[][])field(data,eab.class,"u");List<?> biomes=(List<?>)field(data,eab.class,"t");
  List<Object> columns=new ArrayList<>(),bioColumns=new ArrayList<>();for(double[] column:density)columns.add(column==null?null:bits(column));for(Object column:biomes){if(column==null){bioColumns.add(null);continue;}List<String> names=new ArrayList<>();for(Object h:(List<?>)column)names.add(biomeName((jm<ddw>)h));bioColumns.add(names);}
  List<Object> hs=new ArrayList<>(),ds=new ArrayList<>(),bs=new ArrayList<>();
  Method ih=eab.class.getDeclaredMethod("a",int.class,int.class,Class.forName("eab$c")),id=eab.class.getDeclaredMethod("a",int.class,int.class,int.class,int.class,Class.forName("eab$b")),ib=eab.class.getDeclaredMethod("a",int.class,int.class,int.class,Class.forName("eab$a"));ih.setAccessible(true);id.setAccessible(true);ib.setAccessible(true);
  ih.invoke(data,owner.f().e<<2,owner.f().f<<2,(eab.c)(x,z,h)->hs.add(List.of(x,z,bits(h))));
  id.invoke(data,owner.f().e<<2,owner.f().f<<2,-16,40,(eab.b)(x,y,z,d)->ds.add(List.of(x,y,z,bits(d))));
  for(int y:new int[]{-1,0,31,32,63,64}){List<Object> row=new ArrayList<>();ib.invoke(data,owner.f().e<<2,y,owner.f().f<<2,(eab.a)(x,z,b)->{try{row.add(List.of(x,z,biomeName(b)));}catch(Exception e){throw new RuntimeException(e);}});bs.add(Map.of("quart_y",y,"values",row));}
  List<String> sideNames=new ArrayList<>();for(jj side:sides)sideNames.add(side.name());
  Map<String,Object> r=new LinkedHashMap<>();r.put("chunk",List.of(owner.f().e,owner.f().f));r.put("sides",sideNames);r.put("heights",bits(heights));r.put("densities",columns);r.put("biomes",bioColumns);r.put("iterate_heights",hs);r.put("iterate_densities",ds);r.put("iterate_biomes",bs);r.put("calculated",field(data,eab.class,"r"));r.put("source_status",owner.j().f().toString());
  List<Object> reads=new ArrayList<>();Method gh=eab.class.getDeclaredMethod("a",int.class,int.class,int.class),gd=eab.class.getDeclaredMethod("b",int.class,int.class,int.class);gh.setAccessible(true);gd.setAccessible(true);for(int x:new int[]{0,1,3,4})for(int y:new int[]{-1,0,1,7,31,32,33})for(int z:new int[]{0,1,3,4})reads.add(List.of(x,y,z,bits((double)gh.invoke(data,x,y,z)),bits((double)gd.invoke(data,x,y,z))));r.put("reads",reads);return r;
 }
 static Map<String,Object> image(dvs target,Path file)throws Exception {
  ByteBuffer bytes=ByteBuffer.allocate(target.J_()*256*4).order(ByteOrder.LITTLE_ENDIAN);for(int y=target.I_();y<target.am();y++)for(int x=0;x<16;x++)for(int z=0;z<16;z++)bytes.putInt(dfy.i(target.a_(new jd(target.f().e*16+x,y,target.f().f*16+z))));Files.write(file,bytes.array());
  List<Integer> surface=new ArrayList<>(),floor=new ArrayList<>();for(int z=0;z<16;z++)for(int x=0;x<16;x++){surface.add(target.a(dyy.a.a,x,z)+1);floor.add(target.a(dyy.a.c,x,z)+1);}
  List<Object> posts=new ArrayList<>();int count=0;for(ShortList list:target.n()){List<Short> row=new ArrayList<>();if(list!=null)for(short p:list){row.add(p);count++;}posts.add(row);}
  return Map.of("file",file.toString(),"sha256",sha(bytes.array()),"world_surface_wg",surface,"ocean_floor_wg",floor,"posts",posts,"post_count",count,"status",target.j().f().toString());
 }
 static List<Object> keys(Long2ObjectOpenHashMap<eab> data){List<Object> keys=new ArrayList<>();data.forEach((k,v)->keys.add(List.of((int)(long)k,(int)((long)k>>32))));return keys;}
 static Object runBlend(JsonObject q)throws Exception {
  String settings=q.get("settings").getAsString();long seed=q.get("seed").getAsLong();JsonArray chunk=q.getAsJsonArray("chunk");int cx=chunk.get(0).getAsInt(),cz=chunk.get(1).getAsInt();
  Object settingsRegistry=inherited("registry"),settingsHolder=shared("getHolder",new Class<?>[]{Object.class,String.class},settingsRegistry,settings),set=((jm<?>)settingsHolder).a(),getters=inherited("getters"),source=workerSource;
  Object state=Class.forName("dzm").getMethod("a",Class.forName("jn$a"),Class.forName("akq"),long.class).invoke(null,getters,shared("key",new Class<?>[]{Object.class,String.class},settingsRegistry,settings),seed);
  Map<String,Snapshot> owners=new LinkedHashMap<>();List<Snapshot> oldOwners=new ArrayList<>();for(JsonElement entry:q.getAsJsonArray("old")) {JsonObject spec=entry.getAsJsonObject();JsonArray pos=spec.getAsJsonArray("chunk");int x=pos.get(0).getAsInt(),z=pos.get(1).getAsInt();Snapshot owner=new Snapshot(x,z,spec.get("profile").getAsInt(),data(spec));Object oldSource=source;if(spec.has("biome")){Object biomeKey=inherited("biomeKey");oldSource=new deh((jm<ddw>)shared("getHolder",new Class<?>[]{Object.class,String.class},biomeKey,spec.get("biome").getAsString()));}owner.a((ddz)oldSource,((dzm)state).b());owners.put(key(x,z),owner);oldOwners.add(owner);}
  for(Snapshot old:oldOwners)for(jj side:jj.values()){int x=old.f().e+side.b(),z=old.f().f+side.c();if(!owners.containsKey(key(x,z)))owners.put(key(x,z),new Snapshot(x,z,0,null));}
  ageQueries=new ArrayList<>();dds region=(dds)Proxy.newProxyInstance(dds.class.getClassLoader(),new Class<?>[]{dds.class},(proxy,method,args)->{if(method.getName().equals("a")&&Arrays.equals(method.getParameterTypes(),new Class<?>[]{int.class,int.class})){int x=(int)args[0],z=(int)args[1];ageQueries.add(List.of(x,z));Snapshot owner=owners.get(key(x,z));if(owner==null)throw new IllegalStateException("missing age owner");return owner;}throw new UnsupportedOperationException(method.toString());});
  List<Object> dataImages=new ArrayList<>();Method calculate=eab.class.getDeclaredMethod("a",duy.class,Set.class);calculate.setAccessible(true);boolean secondCalculationSkipped=true;
  for(Snapshot old:oldOwners){Set<jj> sides=eab.a(region,old.f().e,old.f().f,false);calculate.invoke(old.t(),old,sides);int blocks=old.localBlockReads,biomes=old.localBiomeReads;calculate.invoke(old.t(),old,sides);secondCalculationSkipped&=blocks==old.localBlockReads&&biomes==old.localBiomeReads;dataImages.add(dataImage(old,old.t(),sides));}
  // Source-reviewed of(region) scan adapter only. The concrete public region
  // constructor/status contract is not substituted by the interface above.
  Long2ObjectOpenHashMap<eab> height=new Long2ObjectOpenHashMap<>(),density=new Long2ObjectOpenHashMap<>();for(int dx=-7;dx<=7;dx++)for(int dz=-7;dz<=7;dz++){if(dx*dx+dz*dz>64)continue;Snapshot old=owners.get(key(cx+dx,cz+dz));if(old==null||old.t()==null)continue;height.put(packed(cx+dx,cz+dz),old.t());if(dx>=-1&&dx<=1&&dz>=-1&&dz<=1)density.put(packed(cx+dx,cz+dz),old.t());}
  Constructor<eaa> blenderCtor=eaa.class.getDeclaredConstructor(Long2ObjectOpenHashMap.class,Long2ObjectOpenHashMap.class);blenderCtor.setAccessible(true);eaa blender=q.has("singleton_empty")&&q.get("singleton_empty").getAsBoolean()?eaa.a():blenderCtor.newInstance(height,density);
  Method heightToOffset=eaa.class.getDeclaredMethod("a",double.class);heightToOffset.setAccessible(true);List<Object> offsets=new ArrayList<>();for(double h:new double[]{-256.5,-128.5,-8.5,-8.0,-0.5,0.0,7.5,8.0,63.0,64.0,128.0,255.0})offsets.add(List.of(bits(h),bits((double)heightToOffset.invoke(null,h))));
  List<Object> scalar=new ArrayList<>();for(JsonElement p:q.getAsJsonArray("points")){JsonArray v=p.getAsJsonArray();int x=v.get(0).getAsInt(),y=v.get(1).getAsInt(),z=v.get(2).getAsInt();eaa.a output=blender.a(x,z);scalar.add(List.of(x,y,z,bits(output.a()),bits(output.b()),bits(blender.a(new dyr.e(x,y,z),0.375))));}
  List<Object> resolverCalls=new ArrayList<>(),resolved=new ArrayList<>();ddz fallback=(x,y,z,sampler)->{resolverCalls.add(List.of(x,y,z,sampler==((dzm)state).b()));return ((dea)source).getNoiseBiome(x,y,z,sampler);};ddz resolver=blender.a(fallback);
  for(JsonElement p:q.getAsJsonArray("biome_points")){JsonArray v=p.getAsJsonArray();int x=v.get(0).getAsInt(),y=v.get(1).getAsInt(),z=v.get(2).getAsInt();resolved.add(List.of(x,y,z,biomeName(resolver.getNoiseBiome(x,y,z,((dzm)state).b()))));}
  dzd settingsValue=(dzd)set;dyk.a picker;Method pick=dzb.class.getDeclaredMethod("a",dzd.class);pick.setAccessible(true);picker=(dyk.a)pick.invoke(null,settingsValue);Field beardField=Class.forName("dys$b").getField("a");beardField.setAccessible(true);dys.c beard=(dys.c)beardField.get(null);
  dvs target=new dvs(new dcd(cx,cz),dvv.a,dcy.e(-64,384),biomeRegistry,null);dzc noise=dzc.a(target,(dzm)state,beard,settingsValue,picker,blender);target.a((Function<duy,dzc>)unused->noise);Object alpha=field(noise,dzc.class,"q"),offset=field(noise,dzc.class,"r");List<Object> prefill=new ArrayList<>();double[][] as=(double[][])field(alpha,alpha.getClass(),"f"),os=(double[][])field(offset,offset.getClass(),"f");for(int x=0;x<as.length;x++)for(int z=0;z<as[x].length;z++)prefill.add(List.of(x,z,bits(as[x][z]),bits(os[x][z])));
  Method wrap=dzc.class.getDeclaredMethod("a",dyr.class);wrap.setAccessible(true);Field af=Class.forName("dys$d").getField("a"),of=Class.forName("dys$f").getField("a");af.setAccessible(true);of.setAccessible(true);dyr visitedAlpha=(dyr)wrap.invoke(noise,af.get(null)),visitedOffset=(dyr)wrap.invoke(noise,of.get(null));
  List<Object> cacheSamples=new ArrayList<>();for(JsonElement p:q.getAsJsonArray("cache_points")){JsonArray v=p.getAsJsonArray();int x=v.get(0).getAsInt(),y=v.get(1).getAsInt(),z=v.get(2).getAsInt();dyr.e point=new dyr.e(x,y,z);cacheSamples.add(List.of(x,y,z,bits(visitedAlpha.a(point)),bits(visitedOffset.a(point))));}
  String beforeLast=Long.toUnsignedString((long)field(noise,dzc.class,"t"));Constructor<?> bc=Class.forName("dys$e").getDeclaredConstructor(dyr.class);bc.setAccessible(true);dyr blendConstant=(dyr)bc.newInstance(dys.a(0.375));dyr wrapped=(dyr)wrap.invoke(noise,blendConstant);List<Object> contexts=new ArrayList<>();contexts.add(List.of("owner",noise.a(),noise.b(),noise.c(),bits(wrapped.a(noise))));contexts.add(List.of("external",noise.a(),noise.b(),noise.c(),bits(wrapped.a(new dyr.e(noise.a(),noise.b(),noise.c())))));
  List<Object> custom=new ArrayList<>();for(JsonElement description:q.getAsJsonArray("custom_roots")) {
   JsonObject def=description.getAsJsonObject();dyr raw=dyr.b.parse(JsonOps.INSTANCE,def.get("function")).getOrThrow();
   dyr visited=raw.a((dyr.f)function->{try{return (dyr)wrap.invoke(noise,function);}catch(Exception e){throw new RuntimeException(e);}});
   List<Object> samples=new ArrayList<>();List<dyr.e> points=new ArrayList<>();for(JsonElement p:q.getAsJsonArray("cache_points")){JsonArray v=p.getAsJsonArray();dyr.e at=new dyr.e(v.get(0).getAsInt(),v.get(1).getAsInt(),v.get(2).getAsInt());points.add(at);samples.add(bits(visited.a(at)));}
   List<Integer> indices=new ArrayList<>();dyr.a provider=new dyr.a(){public dyr.b a(int index){indices.add(index);return points.get(index);}public void a(double[] values,dyr function){for(int i=0;i<values.length;i++)values[i]=function.a(a(i));}};
   double[] values=new double[points.size()];visited.a(values,provider);custom.add(Map.of("name",def.get("name").getAsString(),"raw_bounds",List.of(bits(raw.a()),bits(raw.b())),"wrapped_bounds",List.of(bits(visited.a()),bits(visited.b())),"samples",samples,"fill",bits(values),"for_index_calls",indices));
  }
  // Independent real job owner: component probes above intentionally mutate
  // their last-pair cache and must not alter this production-path witness.
  target=new dvs(new dcd(cx,cz),dvv.a,dcy.e(-64,384),biomeRegistry,null);dzc jobNoise=dzc.a(target,(dzm)state,beard,settingsValue,picker,blender);final dzc installedNoise=jobNoise;target.a((Function<duy,dzc>)unused->installedNoise);Object jobAquifer=jobNoise.i();
  Method cached=dzc.class.getDeclaredMethod("a",dze.class,List.class);cached.setAccessible(true);def.f sampler=(def.f)cached.invoke(jobNoise,((dzm)state).a(),settingsValue.k());List<Object> biomeFallback=new ArrayList<>();
  ddz jobResolver=blender.a((x,y,z,s)->{biomeFallback.add(List.of(x,y,z,s==sampler));return ((dea)source).getNoiseBiome(x,y,z,s);});target.a(jobResolver,sampler);
  List<String> targetBiomes=new ArrayList<>();for(dvj section:target.d())for(int y=0;y<4;y++)for(int z=0;z<4;z++)for(int x=0;x<4;x++)targetBiomes.add(biomeName(section.c(x,y,z)));
  Path biomeFile=output.resolve("java-blending-biomes-"+q.get("id").getAsString()+".json");Files.writeString(biomeFile,JSON.toJson(targetBiomes));
  Object completed=((java.util.concurrent.CompletableFuture<?>)new dzb((dea)source,(jm<dzd>)settingsHolder).a(blender,(dzm)state,null,target)).join();if(completed!=target)throw new AssertionError("NOISE returned a replacement chunk");boolean same=target.a((Function<duy,dzc>)unused->{throw new AssertionError("lost NoiseChunk");})==jobNoise&&jobNoise.i()==jobAquifer&&jobNoise.d()==blender;
  Map<String,Object> noiseImage=image(target,output.resolve("java-blending-"+q.get("id").getAsString()+".bin"));
  Map<String,Object> result=new LinkedHashMap<>();List<Object> shiftValues=new ArrayList<>();eoh shift=(eoh)field(null,eaa.class,"b");for(int[] p:new int[][]{{0,0},{1,-1},{-1,1},{27,-27},{1875066,1875066},{Integer.MAX_VALUE,Integer.MIN_VALUE}})shiftValues.add(List.of(p[0],p[1],bits(shift.a((double)p[0],0.0,(double)p[1]))));
  result.put("shift_noise",shiftValues);result.put("query",q);result.put("biome_file",biomeFile.toString());result.put("biome_fallback",biomeFallback);result.put("data",dataImages);result.put("age_queries",ageQueries);result.put("second_calculation_skipped",secondCalculationSkipped);result.put("height_map_order",keys(height));result.put("density_map_order",keys(density));result.put("height_to_offset",offsets);result.put("scalars",scalar);result.put("resolved_biomes",resolved);result.put("resolver_calls",resolverCalls);result.put("prefill",prefill);result.put("cache_samples",cacheSamples);result.put("last_cache_after_samples",beforeLast);result.put("density_contexts",contexts);result.put("custom",custom);result.put("noise",noiseImage);result.put("same_noise_aquifer_blender",same);result.put("oracle_inner_sha256",INNER);return result;
 }
 static Object mapWitness()throws Exception {
  List<Object> rows=new ArrayList<>();Long2ObjectOpenHashMap<Object> map=new Long2ObjectOpenHashMap<>();Field capacity=map.getClass().getDeclaredField("n");capacity.setAccessible(true);List<int[]> stencil=new ArrayList<>();for(int x=-7;x<=7;x++)for(int z=-7;z<=7;z++)if(x*x+z*z<=64)stencil.add(new int[]{x,z});
  Set<Integer> checkpoints=Set.of(0,1,24,25,48,49,96,97,192,193);Object zeroOwner=null;int inserted=0;
  for(int i=0;i<=stencil.size();i++) {
   if(checkpoints.contains(i)){List<Object> order=new ArrayList<>();map.forEach((k,v)->order.add(List.of((int)(long)k,(int)((long)k>>32),v)));rows.add(Map.of("size",i,"capacity",capacity.getInt(map),"order",order));}
   if(i<stencil.size()){int[] p=stencil.get(i);Object owner=Integer.valueOf(i+1);if(p[0]==0&&p[1]==0)zeroOwner=owner;map.put(packed(p[0],p[1]),owner);inserted++;}
  }
  List<Object> before=new ArrayList<>();map.forEach((k,v)->before.add(k));Object replacement=new String("replacement-owner");Object old=map.put(0L,replacement);List<Object> after=new ArrayList<>();map.forEach((k,v)->after.add(k));
  List<Object> collisions=new ArrayList<>();int bucket=-1;for(int x=-16;x<=16;x++)for(int z=-16;z<=16;z++){long key=packed(x,z);int slot=(int)HashCommon.mix(key)&31;if(bucket<0)bucket=slot;if(slot==bucket&&key!=0&&collisions.size()<8)collisions.add(List.of(x,z,slot));}
  Long2ObjectOpenHashMap<Object> collisionMap=new Long2ObjectOpenHashMap<>();for(int i=0;i<collisions.size();i++){List<?> p=(List<?>)collisions.get(i);collisionMap.put(packed((int)p.get(0),(int)p.get(1)),Integer.valueOf(i+1));}List<Object> collisionOrder=new ArrayList<>();collisionMap.forEach((k,v)->collisionOrder.add(List.of((int)(long)k,(int)((long)k>>32),v)));
  return Map.of("collision_order",collisionOrder,"stencil",rows,"replacement_preserves_order",before.equals(after),"replacement_returns_old_owner",old==zeroOwner,"replacement_retains_new_owner",map.get(0L)==replacement,"collision_keys",collisions,"fastutil_jar_sha256",sha(Files.readAllBytes(Path.of(HashCommon.class.getProtectionDomain().getCodeSource().getLocation().toURI()))),"oracle_inner_sha256",INNER);
 }
 @SuppressWarnings("unchecked") public static void main(String[] args)throws Exception {
  if(args.length!=1)throw new IllegalArgumentException("usage: WorldgenBlendingOracle <output-directory>");output=Path.of(args[0]).toAbsolutePath().normalize();Files.createDirectories(output);shared("setup",new Class<?>[]{});biomeRegistry=(jz<ddw>)inherited("biomeRegistry");workerSource=(dea)source("minecraft:overworld");Method tags=Class.forName("oracle.WorldgenCarverOracle").getDeclaredMethod("bindBuiltinTags");tags.setAccessible(true);tags.invoke(null);if(!sha(Files.readAllBytes(Path.of(eaa.class.getProtectionDomain().getCodeSource().getLocation().toURI()))).equals(INNER))throw new AssertionError("wrong original Blender CodeSource");
  BufferedReader input=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=input.readLine())!=null;){JsonObject q=JsonParser.parseString(line).getAsJsonObject();try{OUT.println(JSON.toJson(q.has("mode")&&q.get("mode").getAsString().equals("maps")?mapWitness():runBlend(q)));}catch(Throwable error){error.printStackTrace(OUT);throw error;}}
 }
}
