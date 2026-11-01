package oracle;
import com.google.gson.*;
import java.io.*;
import java.lang.reflect.*;
import java.nio.*;
import java.nio.file.*;
import java.security.*;
import java.util.*;
import java.util.function.*;

/** Actual original topMaterial on retained real ProtoChunk/NoiseChunk owners.
 * Reuses WorldgenSurfaceOracle's built-in registry bootstrap, without Unsafe. */
public class WorldgenTopMaterialOracle extends WorldgenSurfaceOracle {
 static Object runTop(JsonObject q)throws Exception {
  String name=q.get("settings").getAsString(),biomeName=q.get("biome").getAsString();long seed=q.get("seed").getAsLong();int cx=q.get("x").getAsInt(),cz=q.get("z").getAsInt();
  Object settingsHolder=getHolder(registry,name),set=value(settingsHolder),state=S.getMethod("a",Class.forName("jn$a"),K,long.class).invoke(null,getters,key(registry,name),seed);
  Object noise=SETTINGS.getMethod("f").invoke(set);int width=(int)NOISE.getMethod("b").invoke(noise),min=(int)NOISE.getMethod("c").invoke(noise),total=(int)NOISE.getMethod("d").invoke(noise);
  Object accessor=ACCESS.getMethod("e",int.class,int.class).invoke(null,min,total),chunkPos=Class.forName("dcd").getConstructor(int.class,int.class).newInstance(cx,cz),upgrade=Class.forName("dvv").getField("a").get(null);
  Object proto=PROTO.getConstructor(Class.forName("dcd"),Class.forName("dvv"),ACCESS,REGISTRY,Class.forName("eab")).newInstance(chunkPos,upgrade,accessor,biomeRegistry,null);PROTO.getMethod("a",Class.forName("dvz")).invoke(proto,Class.forName("dvz").getField("g").get(null));
  Object biomeHolder=getHolder(biomeKey,biomeName),biome=value(biomeHolder),fixed=Class.forName("deh").getConstructor(HOLDER).newInstance(biomeHolder);
  Class.forName("duy").getMethod("a",Class.forName("ddz"),Class.forName("def$f")).invoke(proto,fixed,S.getMethod("b").invoke(state));
  Object biomeManager=Class.forName("ddy").getConstructor(Class.forName("ddy$a"),long.class).newInstance(fixed,Class.forName("ddy").getMethod("a",long.class).invoke(null,seed));
  Object generator=Class.forName("dzb").getConstructor(Class.forName("dea"),HOLDER).newInstance(fixed,settingsHolder);
  Path input=Path.of(q.get("input").getAsString());byte[] before=Files.readAllBytes(input);ByteBuffer blocks=ByteBuffer.wrap(before).order(ByteOrder.LITTLE_ENDIAN);
  Constructor<?> position=POSITION.getConstructor(int.class,int.class,int.class);Method stateById=Class.forName("dfy").getMethod("a",int.class),stateId=Class.forName("dfy").getMethod("i",BLOCKSTATE),setBlock=PROTO.getMethod("a",POSITION,BLOCKSTATE,boolean.class),getBlock=PROTO.getMethod("a_",POSITION);
  for(int y=min;y<min+total;y++)for(int x=0;x<16;x++)for(int z=0;z<16;z++){int id=blocks.getInt();if(id!=0)setBlock.invoke(proto,position.newInstance(cx*16+x,y,cz*16+z),stateById.invoke(null,id),false);}
  Set<Object> types=new HashSet<>();for(String field:new String[]{"a","c"})types.add(Class.forName("dyy$a").getField(field).get(null));Class.forName("dyy").getMethod("a",Class.forName("duy"),Set.class).invoke(null,proto,types);
  Method picker=Class.forName("dzb").getDeclaredMethod("a",SETTINGS);picker.setAccessible(true);Object fluid=picker.invoke(null,set);Field beardField=Class.forName("dys$b").getField("a");beardField.setAccessible(true);Object beard=beardField.get(null),blender=BLENDER.getMethod("a").invoke(null);
  Object chunk=CHUNK.getConstructor(int.class,S,int.class,int.class,NOISE,BEARD,SETTINGS,PICKER,BLENDER).newInstance(16/width,state,cx*16,cz*16,noise,beard,set,fluid,blender);
  Object access=Class.forName("ka$c").getConstructor(List.class).newInstance(List.of(biomeRegistry));Object rule=SETTINGS.getMethod("j").invoke(set);Object context=Class.forName("eaz").getConstructor(Class.forName("dzb"),Class.forName("ka"),ACCESS,CHUNK,S,Class.forName("dzp$o")).newInstance(generator,access,accessor,chunk,state,rule);
  Object surface=S.getMethod("c").invoke(state);Method top=Class.forName("dzq").getMethod("a",Class.forName("dzp$o"),Class.forName("eaz"),Function.class,Class.forName("duy"),CHUNK,POSITION,boolean.class),preliminary=CHUNK.getMethod("a",int.class,int.class),heightAt=Class.forName("duy").getMethod("a",Class.forName("dyy$a"),int.class,int.class),cold=Class.forName("ddw").getMethod("b",POSITION);
  List<Integer> heights=new ArrayList<>();for(int z=0;z<16;z++)for(int x=0;x<16;x++)heights.add((int)heightAt.invoke(proto,Class.forName("dyy$a").getField("a").get(null),x,z));
  List<Object> results=new ArrayList<>();
  for(JsonElement element:q.getAsJsonArray("queries")){JsonObject query=element.getAsJsonObject();JsonArray coords=query.getAsJsonArray("point");int x=coords.get(0).getAsInt(),y=coords.get(1).getAsInt(),z=coords.get(2).getAsInt();Object pos=position.newInstance(x,y,z);boolean wet=query.get("has_fluid").getAsBoolean(),fail=query.has("fail_biome")&&query.get("fail_biome").getAsBoolean();List<List<Integer>> biomeCalls=new ArrayList<>();
   Function<Object,Object> getter=p->{try{int bx=(int)POSITION.getMethod("u").invoke(p),by=(int)POSITION.getMethod("v").invoke(p),bz=(int)POSITION.getMethod("w").invoke(p);biomeCalls.add(List.of(bx,by,bz));if(fail)throw new IllegalStateException("unavailable actual biome owner");return Class.forName("ddy").getMethod("a",POSITION).invoke(biomeManager,p);}catch(ReflectiveOperationException e){throw new RuntimeException(e);}};
   Map<String,Object> row=new LinkedHashMap<>();row.put("point",List.of(x,y,z));row.put("has_fluid",wet);row.put("fail_biome",fail);
   try{Optional<?> result=(Optional<?>)top.invoke(surface,rule,context,getter,proto,chunk,pos,wet);row.put("material",result.isEmpty()?null:stateId.invoke(null,result.get()));row.put("error",null);}catch(InvocationTargetException error){row.put("material",null);row.put("error",error.getCause().getMessage());}
   row.put("biome_queries",biomeCalls);row.put("cold_enough_to_snow",cold.invoke(biome,pos));Map<String,Integer> pre=new LinkedHashMap<>();int px=(x>>4)<<4,pz=(z>>4)<<4;for(int[] p:new int[][]{{px,pz},{px+16,pz},{px,pz+16},{px+16,pz+16}})pre.put(p[0]+","+p[1],(int)preliminary.invoke(chunk,p[0],p[1]));row.put("preliminary",pre);results.add(row);
  }
  ByteBuffer after=ByteBuffer.allocate(before.length).order(ByteOrder.LITTLE_ENDIAN);for(int y=min;y<min+total;y++)for(int x=0;x<16;x++)for(int z=0;z<16;z++)after.putInt((int)stateId.invoke(null,getBlock.invoke(proto,position.newInstance(cx*16+x,y,cz*16+z))));
  Map<String,Object> result=new LinkedHashMap<>();result.put("settings",name);result.put("seed",seed);result.put("biome",biomeName);result.put("chunk",List.of(cx,cz));result.put("min_y",min);result.put("height",total);result.put("world_surface_wg",heights);result.put("queries",results);result.put("blocks_unchanged",Arrays.equals(before,after.array()));result.put("input_sha256",HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(before)));return result;
 }
 public static void main(String[]args)throws Exception {setup();Gson gson=new GsonBuilder().serializeNulls().create();BufferedReader input=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=input.readLine())!=null;)try{OUT.println(gson.toJson(runTop(JsonParser.parseString(line).getAsJsonObject())));}catch(InvocationTargetException e){e.getCause().printStackTrace();System.exit(1);}}
}
