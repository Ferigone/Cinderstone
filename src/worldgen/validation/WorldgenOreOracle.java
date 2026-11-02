import com.google.gson.*;
import java.io.*;
import java.lang.reflect.*;
import java.nio.*;
import java.nio.file.*;
import java.security.*;
import java.util.*;

/** Actual configured Ore/ScatteredOre leaves on real ProtoChunk/palette owners.
 * The strict controlled region proxy is not WorldGenRegion or a status proof. */
public class WorldgenOreOracle {
 static final PrintStream OUT=System.out;
 static final Gson JSON=new GsonBuilder().serializeNulls().create();
 static final String INNER="8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01";
 static Path output;
 static jz<ddw> biomeRegistry;
 static boolean recording=false,regionOperation=false;
 static List<Object> events;
 static Object shared(String method,Class<?>[] types,Object...args)throws Exception {
  Method m=Class.forName("oracle.WorldgenSurfaceOracle").getDeclaredMethod(method,types);m.setAccessible(true);return m.invoke(null,args);
 }
 static Object field(Object owner,Class<?> type,String name)throws Exception {Field f=type.getDeclaredField(name);f.setAccessible(true);return f.get(owner);}
 static String sha(byte[] value)throws Exception{return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(value));}
 static List<Integer> point(jd p){return List.of(p.u(),p.v(),p.w());}
 static void log(Object...items){if(recording)events.add(Arrays.asList(items));}
 static dtc state(String name){return lt.e.a(akr.a("minecraft:"+name)).o();}
 static class ObservedSection extends dvj {
  final int x,y,z;int balance=0,acquires=0,releases=0;
  ObservedSection(int x,int y,int z){super(biomeRegistry);this.x=x;this.y=y;this.z=z;}
  @Override public dtc a(int x,int y,int z){dtc s=super.a(x,y,z);if(!regionOperation)log("bulk_get",List.of(this.x*16+x,this.y*16+y,this.z*16+z),dfy.i(s));return s;}
  @Override public dtc a(int x,int y,int z,dtc s,boolean lock){dtc before=super.a(x,y,z,s,lock);if(!regionOperation)log("section_set",List.of(this.x*16+x,this.y*16+y,this.z*16+z),dfy.i(s),lock);return before;}
  @Override public void a(){super.a();if(recording){balance++;acquires++;log("acquire",List.of(x,y,z));}}
  @Override public void b(){super.b();if(recording){balance--;releases++;}}
 }
 static final class Owner {
  final int x,z,min,height;final dvs proto;final ObservedSection[] sections;
  Owner(int x,int z,int min,int height)throws Exception {
   this.x=x;this.z=z;this.min=min;this.height=height;
   dcy access=dcy.e(min,height);sections=new ObservedSection[access.an()];
   for(int i=0;i<sections.length;i++)sections[i]=new ObservedSection(x,access.g(i),z);
   proto=new dvs(new dcd(x,z),dvv.a,sections,new eyr<dfy>(),new eyr<epd>(),access,biomeRegistry,null);
  }
 }
 static final class ControlledRegion implements InvocationHandler {
  final JsonObject query;final int min,height,top,cx,cz,available,writeRadius;final String profile;
  final dcy accessor;final Map<String,Owner> owners=new LinkedHashMap<>();
  ControlledRegion(JsonObject q)throws Exception {
   query=q;min=q.get("min_y").getAsInt();height=q.get("height").getAsInt();top=q.get("top_y").getAsInt();profile=q.get("profile").getAsString();
   JsonArray p=q.getAsJsonArray("origin");cx=Math.floorDiv(p.get(0).getAsInt(),16);cz=Math.floorDiv(p.get(2).getAsInt(),16);available=q.get("available_radius").getAsInt();writeRadius=q.get("write_radius").getAsInt();accessor=dcy.e(min,height);
   for(int dx=-available;dx<=available;dx++)for(int dz=-available;dz<=available;dz++) {
    Owner o=new Owner(cx+dx,cz+dz,min,height);owners.put(key(o.x,o.z),o);
    for(int y=min;y<min+height;y++)for(int z=0;z<16;z++)for(int x=0;x<16;x++) {
     jd pos=new jd(o.x*16+x,y,o.z*16+z);dtc s=initial(pos);if(!s.i())o.proto.a(pos,s,false);
    }
    dyy.a(o.proto,EnumSet.of(dyy.a.a,dyy.a.c));
   }
  }
  String key(int x,int z){return x+","+z;}
  Owner owner(int x,int z){Owner value=owners.get(key(x,z));if(value==null)throw new IllegalStateException("missing_neighbor");return value;}
  dtc initial(jd p) {
   if(p.v()>top)return state("air");
   JsonArray origin=query.getAsJsonArray("origin");
   if(profile.equals("air_plane")&&p.u()==origin.get(0).getAsInt()+1)return state("air");
   if(profile.equals("netherrack"))return state("netherrack");
   if(profile.equals("layered")||profile.equals("air_plane"))return state(p.v()<0?"deepslate":"stone");
   if(profile.equals("stone"))return state("stone");
   throw new IllegalArgumentException("unsupported controlled material profile "+profile);
  }
  boolean ensure(jd p){boolean allowed=Math.abs(Math.floorDiv(p.u(),16)-cx)<=writeRadius&&Math.abs(Math.floorDiv(p.w(),16)-cz)<=writeRadius;log("ensure",point(p),allowed);return allowed;}
  @Override public Object invoke(Object proxy,Method method,Object[] args)throws Throwable {
   String name=method.getName();Class<?>[] params=method.getParameterTypes();
   if(name.equals("f_")&&Arrays.equals(params,new Class<?>[]{jd.class})) {
    jd p=(jd)args[0];return ensure(p);
   }
   if(name.equals("a")&&Arrays.equals(params,new Class<?>[]{dyy.a.class,int.class,int.class})) {
    if(args[0]!=dyy.a.c)throw new UnsupportedOperationException("non-ocean-floor preflight");int x=(int)args[1],z=(int)args[2];int value=owner(Math.floorDiv(x,16),Math.floorDiv(z,16)).proto.a(dyy.a.c,x&15,z&15)+1;log("height",List.of(x,z),value);return value;
   }
   if(name.equals("a")&&Arrays.equals(params,new Class<?>[]{int.class,int.class})) {int x=(int)args[0],z=(int)args[1];log("chunk",List.of(x,z));return owner(x,z).proto;}
   if(name.equals("a_")&&Arrays.equals(params,new Class<?>[]{jd.class})) {
    jd p=(jd)args[0];Owner o=owner(Math.floorDiv(p.u(),16),Math.floorDiv(p.w(),16));regionOperation=true;dtc value;try{value=o.proto.a_(p);}finally{regionOperation=false;}log("world_get",point(p),dfy.i(value));return value;
   }
   if(name.equals("a")&&Arrays.equals(params,new Class<?>[]{jd.class,dtc.class,int.class})) {
    jd p=(jd)args[0];dtc s=(dtc)args[1];int flags=(int)args[2];boolean accepted=ensure(p);
    if(accepted) {Owner o=owner(Math.floorDiv(p.u(),16),Math.floorDiv(p.w(),16));regionOperation=true;try{o.proto.a(p,s,false);}finally{regionOperation=false;}}
    log("region_set",point(p),dfy.i(s),flags,accepted);return accepted;
   }
   if(method.getDeclaringClass()==dcy.class) {
    Object value=method.invoke(accessor,args);if(name.equals("d")&&Arrays.equals(params,new Class<?>[]{int.class}))log("outside_y",args[0],value);if(name.equals("e")&&Arrays.equals(params,new Class<?>[]{int.class})&&((int)value<0||(int)value>=accessor.an()))log("bulk_null_y",args[0]);return value;
   }
   throw new UnsupportedOperationException("unhandled controlled WorldGenLevel call: "+method);
  }
  Map<String,Object> image(String filename)throws Exception {
   ByteBuffer data=ByteBuffer.allocate(owners.size()*height*256*4).order(ByteOrder.LITTLE_ENDIAN);List<Object> maps=new ArrayList<>(),changed=new ArrayList<>();boolean empty=true,locks=true;int acquisitions=0,releases=0;
   for(Owner o:owners.values()) {
    for(int y=min;y<min+height;y++)for(int z=0;z<16;z++)for(int x=0;x<16;x++) {jd p=new jd(o.x*16+x,y,o.z*16+z);dtc s=o.proto.a_(p);data.putInt(dfy.i(s));if(s!=initial(p))changed.add(List.of(p.u(),p.v(),p.w(),dfy.i(s)));}
    List<Integer> surface=new ArrayList<>(),floor=new ArrayList<>();for(int z=0;z<16;z++)for(int x=0;x<16;x++){surface.add(o.proto.a(dyy.a.a,x,z)+1);floor.add(o.proto.a(dyy.a.c,x,z)+1);}
    maps.add(Map.of("chunk",List.of(o.x,o.z),"world_surface_wg",surface,"ocean_floor_wg",floor));empty&=o.proto.j()==dvz.c;
    for(ObservedSection section:o.sections){locks&=section.balance==0;acquisitions+=section.acquires;releases+=section.releases;}
   }
   byte[] bytes=data.array();Path path=output.resolve(filename);Files.write(path,bytes);Map<String,Object> result=new LinkedHashMap<>();result.put("image_file",path.toString());result.put("image_sha256",sha(bytes));result.put("changed",changed);result.put("heightmaps",maps);result.put("statuses_remain_empty",empty);result.put("section_locks_restored",locks);result.put("section_acquisitions",acquisitions);result.put("section_releases",releases);return result;
  }
 }
 static Throwable cause(Throwable error){while(error instanceof InvocationTargetException||error instanceof UndeclaredThrowableException){Throwable next=error.getCause();if(next==null)break;error=next;}return error;}
 static Object configured(String name)throws Exception {
  Object registry=Class.forName("lu").getField("aI").get(null);return ((jm<?>)shared("getHolder",new Class<?>[]{Object.class,String.class},registry,name)).a();
 }
 static Map<String,Object> run(JsonObject q)throws Exception {
  ControlledRegion region=new ControlledRegion(q);events=new ArrayList<>();dds level=(dds)Proxy.newProxyInstance(dds.class.getClassLoader(),new Class<?>[]{dds.class},region);
  ebq<?,?> configured=(ebq<?,?>)configured(q.get("feature").getAsString());eeq config=(eeq)configured.c();long seed=q.get("rng_seed").getAsLong();dzz source=new dzz(seed);dzx random=new dzx(source);JsonArray p=q.getAsJsonArray("origin");jd origin=new jd(p.get(0).getAsInt(),p.get(1).getAsInt(),p.get(2).getAsInt());Boolean placed=null;String error=null;
  recording=true;try{placed=configured.a(level,null,random,origin);}catch(Throwable e){Throwable c=cause(e);if(c instanceof IllegalStateException&&"missing_neighbor".equals(c.getMessage()))error="missing_neighbor";else throw e;}finally{recording=false;}
  Map<String,Object> result=new LinkedHashMap<>();result.put("query",q);result.put("placed",placed);result.put("error",error);result.put("events",events);result.put("random_count",random.l());Object state=field(source,dzz.class,"e");result.put("random_state128",List.of(Long.toUnsignedString((long)field(state,dzy.class,"b")),Long.toUnsignedString((long)field(state,dzy.class,"c"))));result.put("config_size",config.c);result.put("config_discard_bits",Float.floatToRawIntBits(config.d));result.put("feature_type",configured.b().getClass().getName());result.putAll(region.image("java-ore-"+q.get("id").getAsString()+".bin"));result.put("oracle_inner_sha256",INNER);return result;
 }
 static Object configs()throws Exception {
  List<Object> rows=new ArrayList<>();Path jar=Path.of(ecz.class.getProtectionDomain().getCodeSource().getLocation().toURI());
  try(java.util.jar.JarFile archive=new java.util.jar.JarFile(jar.toFile())) {
   List<java.util.jar.JarEntry> files=Collections.list(archive.entries());files.sort(Comparator.comparing(java.util.jar.JarEntry::getName));
   for(java.util.jar.JarEntry file:files)if(file.getName().startsWith("data/minecraft/worldgen/configured_feature/")&&file.getName().endsWith(".json")) {
    byte[] raw=archive.getInputStream(file).readAllBytes();JsonObject decoded=JsonParser.parseString(new String(raw,java.nio.charset.StandardCharsets.UTF_8)).getAsJsonObject();String kind=decoded.get("type").getAsString();
    if(!kind.equals("minecraft:ore")&&!kind.equals("minecraft:scattered_ore"))continue;
    String name="minecraft:"+file.getName().substring("data/minecraft/worldgen/configured_feature/".length(),file.getName().length()-5);
    ebq<?,?> feature=(ebq<?,?>)configured(name);eeq config=(eeq)feature.c();List<Object> targets=new ArrayList<>();
    for(eeq.a target:config.b) {
     Map<String,Boolean> matches=new LinkedHashMap<>();for(String material:new String[]{"air","stone","deepslate","netherrack","dirt","gravel","blackstone","end_stone"})matches.put("minecraft:"+material,target.b.a(state(material),new dzz(123)));
     targets.add(Map.of("state",dfy.i(target.c),"rule_class",target.b.getClass().getName(),"matches",matches));
    }
    rows.add(Map.of("key",name,"feature_type",feature.b().getClass().getName(),"size",config.c,"discard_bits",Float.floatToRawIntBits(config.d),"targets",targets,"source_sha256",sha(raw)));
   }
  }
  return Map.of("configured_ores",rows,"oracle_inner_sha256",INNER);
 }
 @SuppressWarnings("unchecked") public static void main(String[] args)throws Exception {
  if(args.length!=1)throw new IllegalArgumentException("usage: WorldgenOreOracle <output-directory>");output=Path.of(args[0]).toAbsolutePath().normalize();Files.createDirectories(output);
  shared("setup",new Class<?>[]{});biomeRegistry=(jz<ddw>)field(null,Class.forName("oracle.WorldgenSurfaceOracle"),"biomeRegistry");Method tags=Class.forName("oracle.WorldgenCarverOracle").getDeclaredMethod("bindBuiltinTags");tags.setAccessible(true);tags.invoke(null);
  Path loaded=Path.of(ecz.class.getProtectionDomain().getCodeSource().getLocation().toURI());if(!sha(Files.readAllBytes(loaded)).equals(INNER))throw new AssertionError("wrong actually loaded original OreFeature");
  BufferedReader input=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=input.readLine())!=null;){JsonObject q=JsonParser.parseString(line).getAsJsonObject();OUT.println(JSON.toJson(q.has("mode")&&q.get("mode").getAsString().equals("configs")?configs():run(q)));}
 }
}
