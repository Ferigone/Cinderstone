package oracle;
import com.google.gson.*;
import java.io.*;
import java.lang.reflect.*;
import java.nio.*;
import java.nio.file.*;
import java.security.*;
import java.util.*;

public class WorldgenNoiseOracle {
 static final PrintStream OUT=System.out;
 static Path outputDirectory;
 static Class<?> K, L, P, S, SETTINGS, CHUNK, NOISE, BEARD, PICKER, BLENDER, BLOCKSTATE;
 static Object getters, registry;
 static Object settings(String name)throws Exception {
  Object location=L.getMethod("a",String.class).invoke(null,name);
  Object key=K.getMethod("a",K,L).invoke(null,registry,location);
  Object getter=Class.forName("jn$a").getMethod("b",K).invoke(getters,registry);
  Object holder=Class.forName("jn").getMethod("b",K).invoke(getter,key);
  return Class.forName("jm").getMethod("a").invoke(holder);
 }
 static Object run(JsonObject q)throws Exception {
  String name=q.get("settings").getAsString();long seed=q.get("seed").getAsLong();int cx=q.get("x").getAsInt(),cz=q.get("z").getAsInt();
  Object set=settings(name);Object key=K.getMethod("a",K,L).invoke(null,registry,L.getMethod("a",String.class).invoke(null,name));
  Object state=S.getMethod("a",Class.forName("jn$a"),K,long.class).invoke(null,getters,key,seed);
  Object noise=SETTINGS.getMethod("f").invoke(set);
  int width=(int)NOISE.getMethod("b").invoke(noise),height=(int)NOISE.getMethod("a").invoke(noise),min=(int)NOISE.getMethod("c").invoke(noise),total=(int)NOISE.getMethod("d").invoke(noise);
  Method picker=Class.forName("dzb").getDeclaredMethod("a",SETTINGS);picker.setAccessible(true);Object fp=picker.invoke(null,set);
  Field bf=Class.forName("dys$b").getField("a");bf.setAccessible(true);Object beard=bf.get(null);
  Object blender=BLENDER.getMethod("a").invoke(null);
  Object chunk=CHUNK.getConstructor(int.class,S,int.class,int.class,NOISE,BEARD,SETTINGS,PICKER,BLENDER).newInstance(16/width,state,cx*16,cz*16,noise,beard,set,fp,blender);
  Method start=CHUNK.getMethod("f"),advance=CHUNK.getMethod("b",int.class),select=CHUNK.getMethod("b",int.class,int.class),uy=CHUNK.getMethod("a",int.class,double.class),ux=CHUNK.getMethod("b",int.class,double.class),uz=CHUNK.getMethod("c",int.class,double.class),swap=CHUNK.getMethod("h"),stop=CHUNK.getMethod("g"),sample=CHUNK.getDeclaredMethod("e");sample.setAccessible(true);
  Method stateId=Class.forName("dfy").getMethod("i",BLOCKSTATE);
  Object def=SETTINGS.getMethod("g").invoke(set),aquifer=CHUNK.getMethod("i").invoke(chunk);
  Method schedule=Class.forName("dyk").getMethod("a");
  Method preliminary=CHUNK.getMethod("a",int.class,int.class);Map<String,Integer> preBefore=new LinkedHashMap<>();
  for(int v:new int[]{-17,-16,-1,0,3,4,15,16,31})preBefore.put(v+","+(-v),(int)preliminary.invoke(chunk,cx*16+v,cz*16-v));
  Map<String,Integer> cacheOwners=new LinkedHashMap<>();for(String f:new String[]{"i","j"}){Field field=CHUNK.getDeclaredField(f);field.setAccessible(true);cacheOwners.put(f,((java.util.List<?>)field.get(chunk)).size());}
  int[] blocks=new int[total*256];boolean[] fluid=new boolean[total*256];Map<Integer,Integer> counts=new TreeMap<>();
  start.invoke(chunk);
  for(int cellX=0;cellX<16/width;cellX++) {
   advance.invoke(chunk,cellX);
   for(int cellZ=0;cellZ<16/width;cellZ++) for(int cellY=total/height-1;cellY>=0;cellY--) {
    select.invoke(chunk,cellY,cellZ);
    for(int dy=height-1;dy>=0;dy--) {
     int y=(Math.floorDiv(min,height)+cellY)*height+dy;uy.invoke(chunk,y,(double)dy/height);
     for(int dx=0;dx<width;dx++) {
      int x=(cx*16+cellX*width+dx);ux.invoke(chunk,x,(double)dx/width);
      for(int dz=0;dz<width;dz++) {
       int z=cz*16+cellZ*width+dz;uz.invoke(chunk,z,(double)dz/width);
       Object block=sample.invoke(chunk);if(block==null)block=def;
       int id=(int)stateId.invoke(null,block);int idx=(y-min)*256+(x&15)*16+(z&15);blocks[idx]=id;counts.merge(id,1,Integer::sum);
       fluid[idx]=(boolean)schedule.invoke(aquifer);
      }
     }
    }
   }
   swap.invoke(chunk);
  }
  stop.invoke(chunk);
  MessageDigest digest=MessageDigest.getInstance("SHA-256");ByteBuffer buffer=ByteBuffer.allocate(blocks.length*4).order(ByteOrder.LITTLE_ENDIAN);for(int id:blocks)buffer.putInt(id);
  byte[] raw=buffer.array();String sha=HexFormat.of().formatHex(digest.digest(raw));
  String file=outputDirectory.resolve("java-"+name.replace(':','-')+"-"+seed+"-"+cx+"-"+cz+".bin").toString();try(FileOutputStream o=new FileOutputStream(file)){o.write(raw);}
  Map<String,Object> result=new LinkedHashMap<>();result.put("settings",name);result.put("seed",seed);result.put("x",cx);result.put("z",cz);result.put("min_y",min);result.put("height",total);result.put("sha256",sha);result.put("counts",counts);result.put("blocks_file",file);result.put("preliminary",preBefore);result.put("cache_owners",cacheOwners);for(int v:new int[]{-17,-16,-1,0,3,4,15,16,31})if((int)preliminary.invoke(chunk,cx*16+v,cz*16-v)!=preBefore.get(v+","+(-v)))throw new AssertionError("preliminary changed");return result;
 }
 public static void main(String[]args)throws Exception {
  if(args.length!=1)throw new IllegalArgumentException("usage: WorldgenNoiseOracle <output-directory>");
  outputDirectory=Path.of(args[0]).toAbsolutePath().normalize();Files.createDirectories(outputDirectory);
  Class.forName("ab").getMethod("a").invoke(null);Class.forName("akt").getMethod("a").invoke(null);
  K=Class.forName("akq");L=Class.forName("akr");S=Class.forName("dzm");SETTINGS=Class.forName("dzd");CHUNK=Class.forName("dzc");NOISE=Class.forName("dzg");BEARD=Class.forName("dys$c");PICKER=Class.forName("dyk$a");BLENDER=Class.forName("eaa");BLOCKSTATE=Class.forName("dtc");
  Object lookup=Class.forName("pd").getMethod("a").invoke(null);getters=Class.forName("jo$a").getMethod("b").invoke(lookup);registry=Class.forName("lu").getField("aO").get(null);
  Gson gson=new Gson();BufferedReader input=new BufferedReader(new InputStreamReader(System.in));
  for(String line;(line=input.readLine())!=null;) {try {OUT.println(gson.toJson(run(JsonParser.parseString(line).getAsJsonObject())));}catch(InvocationTargetException e){e.getCause().printStackTrace();System.exit(1);}}
 }
}
