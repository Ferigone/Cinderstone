package oracle;
import com.google.gson.*;
import java.io.*;
import java.lang.reflect.*;
import java.nio.*;
import java.nio.file.*;
import java.security.*;
import java.util.*;

/** Actual cachedClimateSampler followed by NOISE on one unchanged NoiseChunk. */
public class WorldgenPreparedClimateOracle extends WorldgenNoiseOracle {
 static String originalArtifactSha256;
 static Object field(Object owner,String name)throws Exception {
  Field f=CHUNK.getDeclaredField(name);f.setAccessible(true);return f.get(owner);
 }
 static List<Object> cacheOwners(Object chunk,String name)throws Exception {
  return new ArrayList<>((List<?>)field(chunk,name));
 }
 static boolean sameOwners(List<Object> before,List<Object> after) {
  if(before.size()!=after.size())return false;
  for(int i=0;i<before.size();i++)if(before.get(i)!=after.get(i))return false;
  return true;
 }
 static Map<String,Object> counters(Object chunk)throws Exception {
  Map<String,Object> out=new LinkedHashMap<>();
  for(String name:new String[]{"G","H","I","y","z"})out.put(name,field(chunk,name));return out;
 }
 static Object runPrepared(JsonObject q)throws Exception {
  String name=q.get("settings").getAsString();long seed=q.get("seed").getAsLong();int cx=q.get("x").getAsInt(),cz=q.get("z").getAsInt();
  Object set=settings(name),key=K.getMethod("a",K,L).invoke(null,registry,L.getMethod("a",String.class).invoke(null,name));
  Object state=S.getMethod("a",Class.forName("jn$a"),K,long.class).invoke(null,getters,key,seed);
  Object noise=SETTINGS.getMethod("f").invoke(set);
  int width=(int)NOISE.getMethod("b").invoke(noise),height=(int)NOISE.getMethod("a").invoke(noise),min=(int)NOISE.getMethod("c").invoke(noise),total=(int)NOISE.getMethod("d").invoke(noise);
  Method picker=Class.forName("dzb").getDeclaredMethod("a",SETTINGS);picker.setAccessible(true);Object fp=picker.invoke(null,set);
  Field bf=Class.forName("dys$b").getField("a");bf.setAccessible(true);Object beard=bf.get(null),blender=BLENDER.getMethod("a").invoke(null);
  Object chunk=CHUNK.getConstructor(int.class,S,int.class,int.class,NOISE,BEARD,SETTINGS,PICKER,BLENDER).newInstance(16/width,state,cx*16,cz*16,noise,beard,set,fp,blender);
  Object aquifer=CHUNK.getMethod("i").invoke(chunk);
  List<Object> originalInterpolators=cacheOwners(chunk,"i"),originalCells=cacheOwners(chunk,"j");
  Map<String,Object> initialCounters=counters(chunk);
  Method cached=CHUNK.getDeclaredMethod("a",Class.forName("dze"),List.class);cached.setAccessible(true);
  Object sampler=cached.invoke(chunk,S.getMethod("a").invoke(state),SETTINGS.getMethod("k").invoke(set));
  Method climate=Class.forName("def$f").getMethod("a",int.class,int.class,int.class),parameters=Class.forName("def$h").getDeclaredMethod("a");parameters.setAccessible(true);
  List<Object> climateValues=new ArrayList<>();
  for(JsonElement point:q.getAsJsonArray("points")) {
   JsonArray p=point.getAsJsonArray();Object target=climate.invoke(sampler,p.get(0).getAsInt(),p.get(1).getAsInt(),p.get(2).getAsInt());long[] array=(long[])parameters.invoke(target);
   List<String> values=new ArrayList<>();for(long value:array)values.add(Long.toString(value));climateValues.add(values);
  }
  boolean cachesRetained=sameOwners(originalInterpolators,cacheOwners(chunk,"i"))&&sameOwners(originalCells,cacheOwners(chunk,"j"));
  boolean countersRetained=initialCounters.equals(counters(chunk));
  Method start=CHUNK.getMethod("f"),advance=CHUNK.getMethod("b",int.class),select=CHUNK.getMethod("b",int.class,int.class),uy=CHUNK.getMethod("a",int.class,double.class),ux=CHUNK.getMethod("b",int.class,double.class),uz=CHUNK.getMethod("c",int.class,double.class),swap=CHUNK.getMethod("h"),stop=CHUNK.getMethod("g"),sample=CHUNK.getDeclaredMethod("e");sample.setAccessible(true);
  Method stateId=Class.forName("dfy").getMethod("i",BLOCKSTATE),schedule=Class.forName("dyk").getMethod("a"),fluidState=BLOCKSTATE.getMethod("u"),emptyFluid=Class.forName("epe").getMethod("c");Object def=SETTINGS.getMethod("g").invoke(set);
  int[] blocks=new int[total*256];int scheduled=0;
  start.invoke(chunk);
  for(int cellX=0;cellX<16/width;cellX++) {
   advance.invoke(chunk,cellX);
   for(int cellZ=0;cellZ<16/width;cellZ++)for(int cellY=total/height-1;cellY>=0;cellY--) {
    select.invoke(chunk,cellY,cellZ);
    for(int dy=height-1;dy>=0;dy--) {
     int y=(Math.floorDiv(min,height)+cellY)*height+dy;uy.invoke(chunk,y,(double)dy/height);
     for(int dx=0;dx<width;dx++) {
      int x=cx*16+cellX*width+dx;ux.invoke(chunk,x,(double)dx/width);
      for(int dz=0;dz<width;dz++) {
       int z=cz*16+cellZ*width+dz;uz.invoke(chunk,z,(double)dz/width);
       Object block=sample.invoke(chunk);if(block==null)block=def;
       blocks[(y-min)*256+(x&15)*16+(z&15)]=(int)stateId.invoke(null,block);
       if((boolean)schedule.invoke(aquifer)&&!(boolean)emptyFluid.invoke(fluidState.invoke(block)))scheduled++;
      }
     }
    }
   }
   swap.invoke(chunk);
  }
  stop.invoke(chunk);
  Map<String,Object> postNoiseCounters=counters(chunk);List<Object> climateAfter=new ArrayList<>();
  for(JsonElement point:q.getAsJsonArray("points")) {
   JsonArray p=point.getAsJsonArray();Object target=climate.invoke(sampler,p.get(0).getAsInt(),p.get(1).getAsInt(),p.get(2).getAsInt());long[] array=(long[])parameters.invoke(target);
   List<String> values=new ArrayList<>();for(long value:array)values.add(Long.toString(value));climateAfter.add(values);
  }
  boolean postCountersRetained=postNoiseCounters.equals(counters(chunk));
  cachesRetained=cachesRetained&&sameOwners(originalInterpolators,cacheOwners(chunk,"i"))&&sameOwners(originalCells,cacheOwners(chunk,"j"));
  ByteBuffer buffer=ByteBuffer.allocate(blocks.length*4).order(ByteOrder.LITTLE_ENDIAN);for(int block:blocks)buffer.putInt(block);byte[] raw=buffer.array();
  Path out=outputDirectory.resolve("java-prepared-"+name.replace(':','-')+"-"+seed+"-"+cx+"-"+cz+".bin");Files.write(out,raw);
  Map<String,Object> result=new LinkedHashMap<>();
  result.put("oracle_inner_sha256",originalArtifactSha256);result.put("settings",name);result.put("seed",seed);result.put("x",cx);result.put("z",cz);result.put("min_y",min);result.put("height",total);result.put("points",q.get("points"));result.put("climate",climateValues);result.put("climate_after",climateAfter);result.put("post_climate_preserves_counters",postCountersRetained);result.put("cache_owners_retained",cachesRetained);result.put("climate_preserves_counters",countersRetained);result.put("aquifer_retained",aquifer==CHUNK.getMethod("i").invoke(chunk));result.put("cache_owners",Map.of("interpolators",originalInterpolators.size(),"cell_caches",originalCells.size()));result.put("initial_counters",initialCounters);result.put("sha256",HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(raw)));result.put("blocks_file",out.toString());result.put("postprocessing_count",scheduled);return result;
 }
 public static void main(String[] args)throws Exception {
  if(args.length!=1)throw new IllegalArgumentException("usage: WorldgenPreparedClimateOracle <output-directory>");
  outputDirectory=Path.of(args[0]).toAbsolutePath().normalize();Files.createDirectories(outputDirectory);
  Class.forName("ab").getMethod("a").invoke(null);Class.forName("akt").getMethod("a").invoke(null);
  K=Class.forName("akq");L=Class.forName("akr");S=Class.forName("dzm");SETTINGS=Class.forName("dzd");CHUNK=Class.forName("dzc");NOISE=Class.forName("dzg");BEARD=Class.forName("dys$c");PICKER=Class.forName("dyk$a");BLENDER=Class.forName("eaa");BLOCKSTATE=Class.forName("dtc");Object lookup=Class.forName("pd").getMethod("a").invoke(null);getters=Class.forName("jo$a").getMethod("b").invoke(lookup);registry=Class.forName("lu").getField("aO").get(null);
  Path originalPath=Path.of(CHUNK.getProtectionDomain().getCodeSource().getLocation().toURI());originalArtifactSha256=HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(Files.readAllBytes(originalPath)));if(!originalArtifactSha256.equals("8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"))throw new AssertionError("unexpected original NoiseChunk class origin");
  Gson gson=new Gson();BufferedReader input=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=input.readLine())!=null;){try{OUT.println(gson.toJson(runPrepared(JsonParser.parseString(line).getAsJsonObject())));}catch(InvocationTargetException e){e.getCause().printStackTrace();System.exit(1);}}
 }
}
