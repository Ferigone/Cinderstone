package oracle;
import java.io.*;
import java.lang.reflect.*;
import java.util.*;
import com.google.gson.*;

/** Controlled coordinate-token provider: observes source queries, not biome selection. */
public class BiomeZoomOracle {
 static final PrintStream OUT=System.out;
 static Class<?> MANAGER, SOURCE, POS, HOLDER;
 static final int[][] BLOCKS={
  {0,0,0},{1,1,1},{2,2,2},{3,3,3},{4,4,4},
  {-1,-1,-1},{-2,-2,-2},{-17,319,16},{15,-64,-16},{16,320,31},
  {29999984,2031,-29999984},{-29999984,-2032,29999984},
  {Integer.MIN_VALUE,Integer.MAX_VALUE,Integer.MIN_VALUE+1},
  {Integer.MAX_VALUE,Integer.MIN_VALUE+1,Integer.MAX_VALUE-1},
  {-2147483646,2147483645,-2147483645}
 };
 static Map<String,Object> run(long zoomSeed,Long worldSeed)throws Exception {
  List<List<Integer>> calls=new ArrayList<>();
  Object provider=Proxy.newProxyInstance(SOURCE.getClassLoader(),new Class<?>[]{SOURCE},(p,m,a)->{
   if(!m.getName().equals("getNoiseBiome"))throw new UnsupportedOperationException(m.toString());
   List<Integer> coordinate=List.of((int)a[0],(int)a[1],(int)a[2]);calls.add(coordinate);
   return HOLDER.getConstructor(Object.class).newInstance(coordinate);
  });
  Object manager=MANAGER.getConstructor(SOURCE,long.class).newInstance(provider,zoomSeed);
  List<Object> queries=new ArrayList<>();
  Method get=MANAGER.getMethod("a",POS),value=Class.forName("jm").getMethod("a");
  Constructor<?> position=POS.getConstructor(int.class,int.class,int.class);
  for(int[] block:BLOCKS) {
   calls.clear();Object returned=get.invoke(manager,position.newInstance(block[0],block[1],block[2]));
   Object coordinates=value.invoke(returned);
   if(calls.size()!=1||!calls.get(0).equals(coordinates))throw new AssertionError("wrong resolver query witness");
   queries.add(Map.of("block",block,"selected_quart",coordinates,"provider_calls",calls.size()));
  }
  Map<String,Object> result=new LinkedHashMap<>();if(worldSeed!=null)result.put("world_seed",worldSeed.toString());result.put("zoom_seed",Long.toString(zoomSeed));result.put("queries",queries);return result;
 }
 public static void main(String[] args)throws Exception {
  Class.forName("ab").getMethod("a").invoke(null);Class.forName("akt").getMethod("a").invoke(null);
  MANAGER=Class.forName("ddy");SOURCE=Class.forName("ddy$a");POS=Class.forName("jd");HOLDER=Class.forName("jm$a");
  List<Object> cases=new ArrayList<>();for(long seed:new long[]{0,1,-1,123456789,Long.MIN_VALUE,Long.MAX_VALUE}){
   long zoom=(long)MANAGER.getMethod("a",long.class).invoke(null,seed);cases.add(run(zoom,seed));
  }
  for(long zoom:new long[]{0,1,-1,Long.MIN_VALUE,Long.MAX_VALUE})cases.add(run(zoom,null));
  Method distance=MANAGER.getDeclaredMethod("a",long.class,int.class,int.class,int.class,double.class,double.class,double.class);distance.setAccessible(true);
  Method fiddle=MANAGER.getDeclaredMethod("b",long.class);fiddle.setAccessible(true);
  List<Object> distances=new ArrayList<>();
  for(long zoom:new long[]{0,-1,Long.MIN_VALUE,Long.MAX_VALUE}) for(int[] pos:new int[][]{{0,0,0},{-1,-64,17},{Integer.MIN_VALUE,Integer.MAX_VALUE,-17}}) {
   double[] offset={.25,-.5,.75};double d=(double)distance.invoke(null,zoom,pos[0],pos[1],pos[2],offset[0],offset[1],offset[2]);
   distances.add(Map.of("zoom_seed",Long.toString(zoom),"quart",pos,"offset",offset,"bits",Long.toUnsignedString(Double.doubleToRawLongBits(d))));
  }
  List<Object> fiddles=new ArrayList<>();for(long seed:new long[]{0,1,-1,Long.MIN_VALUE,Long.MAX_VALUE,16777216,-16777216})fiddles.add(Map.of("seed",Long.toString(seed),"bits",Long.toUnsignedString(Double.doubleToRawLongBits((double)fiddle.invoke(null,seed)))));
  Method next=Class.forName("ayj").getMethod("a",long.class,long.class);List<Object> lcg=new ArrayList<>();
  for(long seed:new long[]{0,1,-1,Long.MIN_VALUE,Long.MAX_VALUE})for(long salt:new long[]{-1,0,1,Long.MIN_VALUE,Long.MAX_VALUE})lcg.add(Map.of("seed",Long.toString(seed),"salt",Long.toString(salt),"next",Long.toString((long)next.invoke(null,seed,salt))));
  int[] rejectedCalls={0};Object rejecting=Proxy.newProxyInstance(SOURCE.getClassLoader(),new Class<?>[]{SOURCE},(p,m,a)->{rejectedCalls[0]++;throw new IllegalStateException("actual BIOMES neighbor unavailable");});Object manager=MANAGER.getConstructor(SOURCE,long.class).newInstance(rejecting,-1L);boolean propagated=false;
  try{MANAGER.getMethod("a",POS).invoke(manager,POS.getConstructor(int.class,int.class,int.class).newInstance(-17,319,16));}catch(InvocationTargetException e){propagated=e.getCause()instanceof IllegalStateException&&e.getCause().getMessage().equals("actual BIOMES neighbor unavailable");}
  OUT.println(new Gson().toJson(Map.of("cases",cases,"distance_probes",distances,"fiddle_probes",fiddles,"lcg_probes",lcg,"provider_failure",Map.of("propagated",propagated,"provider_calls",rejectedCalls[0]))));
 }
}
