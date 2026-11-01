package oracle;
import com.google.gson.*;
import java.lang.reflect.*;
import java.util.*;
import java.util.concurrent.*;

/** Actual fillFromNoise no-cell entry point. All generation dependencies null. */
public class WorldgenNoCellsOracle {
 public static void main(String[]args)throws Exception {
  WorldgenSurfaceOracle.setup();
  Class<?> ACCESS=WorldgenSurfaceOracle.ACCESS,PROTO=WorldgenSurfaceOracle.PROTO,POSITION=WorldgenSurfaceOracle.POSITION;
  Class<?> SETTINGS=WorldgenSurfaceOracle.SETTINGS, NOISE=WorldgenSurfaceOracle.NOISE;
  Object settingsHolder=WorldgenSurfaceOracle.getHolder(WorldgenSurfaceOracle.registry,"minecraft:overworld");
  Object settings=WorldgenSurfaceOracle.value(settingsHolder);
  Object biomeHolder=WorldgenSurfaceOracle.getHolder(WorldgenSurfaceOracle.biomeKey,"minecraft:plains");
  Object fixed=Class.forName("deh").getConstructor(WorldgenSurfaceOracle.HOLDER).newInstance(biomeHolder);
  Object generator=Class.forName("dzb").getConstructor(Class.forName("dea"),WorldgenSurfaceOracle.HOLDER).newInstance(fixed,settingsHolder);
  Object noise=SETTINGS.getMethod("f").invoke(settings);
  Object stone=Class.forName("dga").getField("b").get(null);
  Object stoneState=Class.forName("dfy").getMethod("o").invoke(stone);
  Object pos=Class.forName("dcd").getConstructor(int.class,int.class).newInstance(7,-9);
  Object upgrade=Class.forName("dvv").getField("a").get(null);
  Method fill=Class.forName("dzb").getMethod("a",Class.forName("eaa"),Class.forName("dzm"),Class.forName("ddq"),Class.forName("duy"));
  Field chunkNoise=Class.forName("duy").getDeclaredField("e");chunkNoise.setAccessible(true);
  List<Map<String,Object>> results=new ArrayList<>();
  for(int[] range:new int[][]{{320,16},{384,16},{-160,16},{317,2}}) {
   int min=range[0],height=range[1];
   Object accessor=ACCESS.getMethod("e",int.class,int.class).invoke(null,min,height);
   Object proto=PROTO.getConstructor(Class.forName("dcd"),Class.forName("dvv"),ACCESS,WorldgenSurfaceOracle.REGISTRY,Class.forName("eab")).newInstance(pos,upgrade,accessor,WorldgenSurfaceOracle.biomeRegistry,null);
   Object p=POSITION.getConstructor(int.class,int.class,int.class).newInstance(7*16+2,min,-9*16+5);
   PROTO.getMethod("a",POSITION,WorldgenSurfaceOracle.BLOCKSTATE,boolean.class).invoke(proto,p,stoneState,false);
   Object before=PROTO.getMethod("a_",POSITION).invoke(proto,p),status=PROTO.getMethod("j").invoke(proto);
   Object clamp=NOISE.getMethod("a",ACCESS).invoke(noise,accessor);
   int cmin=(int)NOISE.getMethod("c").invoke(clamp),ch=(int)NOISE.getMethod("d").invoke(clamp),cellHeight=(int)NOISE.getMethod("a").invoke(clamp);
   CompletableFuture<?> future=(CompletableFuture<?>)fill.invoke(generator,null,null,null,proto);
   Map<String,Object> r=new LinkedHashMap<>();
   r.put("level_min_y",min);r.put("level_height",height);r.put("min_y",cmin);r.put("height",ch);r.put("cell_count_y",Math.floorDiv(ch,cellHeight));
   r.put("completed_same_chunk",future.isDone() && future.join()==proto);r.put("sentinel_unchanged",PROTO.getMethod("a_",POSITION).invoke(proto,p)==before);r.put("status_unchanged",PROTO.getMethod("j").invoke(proto)==status);r.put("noise_chunk_absent",chunkNoise.get(proto)==null);
   for(String check:List.of("completed_same_chunk","sentinel_unchanged","status_unchanged","noise_chunk_absent"))if(!Boolean.TRUE.equals(r.get(check)))throw new AssertionError(check);
   results.add(r);
  }
  Map<String,Object> output=new LinkedHashMap<>();output.put("rows",results);
  Object alias=WorldgenSurfaceOracle.getHolder(WorldgenSurfaceOracle.registry,":overworld");output.put("empty_namespace_same_settings_holder",alias==settingsHolder);
  output.put("empty_namespace_canonical",WorldgenSurfaceOracle.L.getMethod("a",String.class).invoke(null,":overworld").toString());
  WorldgenSurfaceOracle.OUT.println(new Gson().toJson(output));
 }
}
