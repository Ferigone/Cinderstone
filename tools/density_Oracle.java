package oracle;
import com.google.gson.*;
import com.mojang.serialization.*;
import java.io.*;
import java.lang.reflect.*;
import java.util.*;

/** Original DensityFunction codecs, RandomState bindings and compute/fillArray. */
class DensityOracle {
 static final PrintStream OUTPUT=System.out,ERROR=System.err;
 static Class<?> FUNCTION,CONTEXT,PROVIDER,POINT,VISITOR,STATE,KEY,LOCATION,ROUTER;
 static Object lookup,ops,state,visitor,chunk,chunkVisitor;
 static Class<?> CHUNK;
 static final Map<String,Object> chunkFunctions=new HashMap<>();
 static Codec<Object> codec;
 static final Map<String,Object> functions=new HashMap<>();
 static JsonObject query;
 static String text(String key){return query.get(key).getAsString();}
 static String bits(double value){return Long.toUnsignedString(Double.doubleToRawLongBits(value));}
 static Object point(JsonArray value)throws Exception{return POINT.getConstructor(int.class,int.class,int.class).newInstance(value.get(0).getAsInt(),value.get(1).getAsInt(),value.get(2).getAsInt());}
 static List<String> bounds(Object value)throws Exception{return List.of(bits((double)FUNCTION.getMethod("a").invoke(value)),bits((double)FUNCTION.getMethod("b").invoke(value)));}
 static void world(long seed,String settings)throws Exception {
  Object registry=Class.forName("lu").getField("aO").get(null);Object location=LOCATION.getMethod("a",String.class).invoke(null,settings);
  Object key=KEY.getMethod("a",KEY,LOCATION).invoke(null,registry,location);
  state=STATE.getMethod("a",Class.forName("jn$a"),KEY,long.class).invoke(null,Class.forName("jo$a").getMethod("b").invoke(lookup),key,seed);
  boolean legacy=!settings.equals("minecraft:overworld")&&!settings.equals("minecraft:amplified")&&!settings.equals("minecraft:large_biomes");
  Constructor<?> constructor=Class.forName("dzm$a").getDeclaredConstructor(STATE,long.class,boolean.class);constructor.setAccessible(true);visitor=constructor.newInstance(state,seed,legacy);
 }
 static void put(Object target,String name,Object value)throws Exception {Field field=CHUNK.getDeclaredField(name);field.setAccessible(true);field.set(target,value);}
 static Object get(Object target,String name)throws Exception {Field field=CHUNK.getDeclaredField(name);field.setAccessible(true);return field.get(target);}
 static Object createProvider(JsonArray points)throws Exception {return Proxy.newProxyInstance(PROVIDER.getClassLoader(),new Class<?>[]{PROVIDER},(proxy,method,args)->{
  if(args.length==1&&args[0]instanceof Integer index)return point(points.get(index).getAsJsonArray());
  if(args.length==2&&args[0]instanceof double[]array){for(int i=0;i<array.length;i++)array[i]=(double)FUNCTION.getMethod("a",CONTEXT).invoke(args[1],point(points.get(i).getAsJsonArray()));return null;}
  throw new UnsupportedOperationException(method.toString());
 });}
 static Object chunkState()throws Exception {Map<String,Object> result=new LinkedHashMap<>();for(String name:new String[]{"y","z","G","H","I","A","B","C","D","E","F"})result.put(name,get(chunk,name));return result;}
 static void newChunk()throws Exception {
  CHUNK=Class.forName("dzc");Class<?> unsafeClass=Class.forName("sun.misc.Unsafe");Field u=unsafeClass.getDeclaredField("theUnsafe");u.setAccessible(true);Object unsafe=u.get(null);chunk=unsafeClass.getMethod("allocateInstance",Class.class).invoke(unsafe,CHUNK);
  JsonObject grid=query.getAsJsonObject("grid");int width=grid.get("cell_width").getAsInt(),height=grid.get("cell_height").getAsInt(),totalHeight=grid.get("height").getAsInt(),minY=grid.get("min_y").getAsInt();int count=query.get("count").getAsInt(),x=query.get("x").getAsInt(),z=query.get("z").getAsInt();
  put(chunk,"w",width);put(chunk,"x",height);put(chunk,"b",count);put(chunk,"c",Math.floorDiv(totalHeight,height));put(chunk,"d",Math.floorDiv(minY,height));put(chunk,"e",Math.floorDiv(x,width));put(chunk,"f",Math.floorDiv(z,width));put(chunk,"g",x>>2);put(chunk,"h",z>>2);put(chunk,"v",(count*width)>>2);
  put(chunk,"i",new ArrayList<>());put(chunk,"j",new ArrayList<>());put(chunk,"k",new HashMap<>());put(chunk,"p",Class.forName("eaa").getMethod("a").invoke(null));
  Field beard=Class.forName("dys$b").getField("a");beard.setAccessible(true);put(chunk,"s",beard.get(null));
  Constructor<?> provider=Class.forName("dzc$1").getDeclaredConstructor(CHUNK);provider.setAccessible(true);put(chunk,"J",provider.newInstance(chunk));
  Method wrap=CHUNK.getDeclaredMethod("a",FUNCTION);wrap.setAccessible(true);
  chunkVisitor=Proxy.newProxyInstance(VISITOR.getClassLoader(),new Class<?>[]{VISITOR},(proxy,method,args)->method.getName().equals("a")?args[0]:wrap.invoke(chunk,args[0]));chunkFunctions.clear();
  if(query.has("router")&&query.get("router").getAsBoolean()){
   Object router=STATE.getMethod("a").invoke(state);String[]names={"barrier","fluid_level_floodedness","fluid_level_spread","lava","temperature","vegetation","continents","erosion","depth","ridges","initial_density_without_jaggedness","final_density","vein_toggle","vein_ridged","vein_gap"};
   for(int i=0;i<names.length;i++){Object function=ROUTER.getMethod(String.valueOf((char)('a'+i))).invoke(router);Object wrapped=FUNCTION.getMethod("a",VISITOR).invoke(function,chunkVisitor);chunkFunctions.put(names[i],wrapped);}
   Object added=Class.forName("dys").getMethod("a",FUNCTION,FUNCTION).invoke(null,chunkFunctions.get("final_density"),beard.get(null));Object cached=Class.forName("dys").getMethod("e",FUNCTION).invoke(null,added);chunkFunctions.put("block_density",FUNCTION.getMethod("a",VISITOR).invoke(cached,chunkVisitor));
  }
 }
 static Object run()throws Exception {
  switch(text("op")){
   case "chunk.new":newChunk();return null;
   case "chunk.wrap":chunkFunctions.put(text("id"),FUNCTION.getMethod("a",VISITOR).invoke(functions.get(text("id")),chunkVisitor));return null;
   case "chunk.state":return chunkState();
   case "chunk.start":CHUNK.getMethod("f").invoke(chunk);return null;
   case "chunk.advance":CHUNK.getMethod("b",int.class).invoke(chunk,query.get("index").getAsInt());return null;
   case "chunk.select":CHUNK.getMethod("b",int.class,int.class).invoke(chunk,query.get("y").getAsInt(),query.get("z").getAsInt());return null;
   case "chunk.y":case "chunk.x":case "chunk.z":CHUNK.getMethod(text("op").equals("chunk.y")?"a":text("op").equals("chunk.x")?"b":"c",int.class,double.class).invoke(chunk,query.get("block").getAsInt(),query.get("fraction").getAsDouble());return null;
   case "chunk.stop":CHUNK.getMethod("g").invoke(chunk);return null;
   case "chunk.swap":CHUNK.getMethod("h").invoke(chunk);return null;
   case "chunk.index":CHUNK.getMethod("c",int.class).invoke(chunk,query.get("index").getAsInt());return null;
   case "chunk.sample":return bits((double)FUNCTION.getMethod("a",CONTEXT).invoke(chunkFunctions.get(text("id")),chunk));
   case "chunk.external":return bits((double)FUNCTION.getMethod("a",CONTEXT).invoke(chunkFunctions.get(text("id")),point(query.getAsJsonArray("point"))));
   case "chunk.fill":case "chunk.column":case "chunk.cell":{
    Object provider;int length;
    if(text("op").equals("chunk.fill")){JsonArray points=query.getAsJsonArray("points");provider=createProvider(points);length=points.size();}
    else if(text("op").equals("chunk.column")){provider=get(chunk,"J");length=(int)get(chunk,"c")+1;}
    else{provider=chunk;int width=(int)get(chunk,"w");length=width*width*(int)get(chunk,"x");}
    double[] values=new double[length];FUNCTION.getMethod("a",double[].class,PROVIDER).invoke(chunkFunctions.get(text("id")),values,provider);List<String> result=new ArrayList<>();for(double value:values)result.add(bits(value));return result;
   }
   case "world":world(query.get("seed").getAsLong(),text("settings"));functions.clear();return null;
   case "load":{
    @SuppressWarnings("unchecked") DynamicOps<JsonElement> dynamic=(DynamicOps<JsonElement>)ops;
    Object value=codec.parse(dynamic,query.get("function")).getOrThrow();value=FUNCTION.getMethod("a",VISITOR).invoke(value,visitor);functions.put(text("id"),value);return bounds(value);
   }
   case "router":{
    Object router=STATE.getMethod("a").invoke(state);String[]names={"barrier","fluid_level_floodedness","fluid_level_spread","lava","temperature","vegetation","continents","erosion","depth","ridges","initial_density_without_jaggedness","final_density","vein_toggle","vein_ridged","vein_gap"};Map<String,Object> result=new LinkedHashMap<>();
    for(int i=0;i<names.length;i++){Object value=ROUTER.getMethod(String.valueOf((char)('a'+i))).invoke(router);functions.put(names[i],value);result.put(names[i],bounds(value));}return result;
   }
   case "climate":{
    Object sampler=STATE.getMethod("b").invoke(state);Class<?> type=Class.forName("def$f"),target=Class.forName("def$h");List<Object> values=new ArrayList<>();
    for(JsonElement p:query.getAsJsonArray("points")){JsonArray coords=p.getAsJsonArray();Object result=type.getMethod("a",int.class,int.class,int.class).invoke(sampler,coords.get(0).getAsInt(),coords.get(1).getAsInt(),coords.get(2).getAsInt());Method array=target.getDeclaredMethod("a");array.setAccessible(true);long[] parameters=(long[])array.invoke(result);List<String> point=new ArrayList<>();for(long parameter:parameters)point.add(Long.toString(parameter));values.add(point);}return values;
   }
   case "eval":{
    Object function=functions.get(text("id"));List<String> values=new ArrayList<>();for(JsonElement p:query.getAsJsonArray("points"))values.add(bits((double)FUNCTION.getMethod("a",CONTEXT).invoke(function,point(p.getAsJsonArray()))));return values;
   }
   case "fill":{
    Object function=functions.get(text("id"));JsonArray points=query.getAsJsonArray("points");
    Object provider=Proxy.newProxyInstance(PROVIDER.getClassLoader(),new Class<?>[]{PROVIDER},(proxy,method,args)->{
     if(args.length==1&&args[0]instanceof Integer index)return point(points.get(index).getAsJsonArray());
     if(args.length==2&&args[0]instanceof double[]array){for(int i=0;i<array.length;i++)array[i]=(double)FUNCTION.getMethod("a",CONTEXT).invoke(args[1],point(points.get(i).getAsJsonArray()));return null;}
     throw new UnsupportedOperationException(method.toString());
    });
    double[] result=new double[points.size()];FUNCTION.getMethod("a",double[].class,PROVIDER).invoke(function,result,provider);List<String> output=new ArrayList<>();for(double value:result)output.add(bits(value));return output;
   }
   default:throw new IllegalArgumentException(text("op"));
  }
 }
 @SuppressWarnings("unchecked") public static void main(String[]args)throws Exception {
  Class.forName("ab").getMethod("a").invoke(null);Class.forName("akt").getMethod("a").invoke(null);
  FUNCTION=Class.forName("dyr");CONTEXT=Class.forName("dyr$b");PROVIDER=Class.forName("dyr$a");POINT=Class.forName("dyr$e");VISITOR=Class.forName("dyr$f");STATE=Class.forName("dzm");KEY=Class.forName("akq");LOCATION=Class.forName("akr");ROUTER=Class.forName("dze");
  lookup=Class.forName("pd").getMethod("a").invoke(null);ops=Class.forName("akp").getMethod("a",DynamicOps.class,Class.forName("jo$a")).invoke(null,JsonOps.INSTANCE,lookup);codec=(Codec<Object>)FUNCTION.getField("d").get(null);
  ERROR.println("DENSITY_ORACLE_CLASS=dyr,dys,dzm,axp;SOURCE="+FUNCTION.getProtectionDomain().getCodeSource().getLocation());
  Gson gson=new GsonBuilder().serializeNulls().create();BufferedReader input=new BufferedReader(new InputStreamReader(System.in));
  for(String line;(line=input.readLine())!=null;){query=JsonParser.parseString(line).getAsJsonObject();Object result;try{result=run();}catch(InvocationTargetException error){result=Map.of("error",text("op").startsWith("chunk.")?error.getCause().getMessage():error.getCause().toString());}OUTPUT.println(gson.toJson(result));}
 }
}
