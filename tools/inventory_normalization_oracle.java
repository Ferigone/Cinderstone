package oracle;
import com.google.gson.*;
import io.netty.buffer.*;
import java.io.*;
import java.lang.reflect.*;
import java.util.*;
import java.nio.file.*;
import java.util.stream.*;
import com.mojang.serialization.Lifecycle;

/** Original ItemStack.OPTIONAL_STREAM_CODEC decode -> encode, never ItemStack.CODEC. */
public final class inventory_normalization_oracle {
 static final Gson JSON=new GsonBuilder().serializeNulls().create();static final PrintStream OUT=System.out;
 static Class<?> STACK,REGISTRY,BUFFER,STREAM,TYPE,MAP,PATCH;static Object access,slotCodec,itemRegistry,componentRegistry;
 static Class<?> C(String n)throws Exception{return Class.forName(n);}static Class<?>[] T(Class<?>...t){return t;}
 static Object call(Object o,String n,Class<?>[]t,Object...v)throws Exception {Class<?> c=o instanceof Class<?>?(Class<?>)o:o.getClass();Method m=null;try{m=c.getMethod(n,t);}catch(NoSuchMethodException e){for(Class<?> q=c;q!=null;q=q.getSuperclass())try{m=q.getDeclaredMethod(n,t);break;}catch(NoSuchMethodException ignored){}}if(m==null)throw new NoSuchMethodException(c+"."+n+Arrays.toString(t));m.setAccessible(true);return m.invoke(o instanceof Class<?>?null:o,v);}
 static Object buf(ByteBuf bytes)throws Exception{return BUFFER.getConstructor(ByteBuf.class,C("ka")).newInstance(bytes,access);}
 static void init()throws Exception {call(C("ab"),"a",T());call(C("akt"),"a",T());STACK=C("cuq");REGISTRY=C("jz");BUFFER=C("wk");STREAM=C("yx");TYPE=C("kp");MAP=C("km");PATCH=C("kn");access=call(C("ka"),"a",T(REGISTRY),C("lt").getField("aA").get(null));slotCodec=STACK.getField("h").get(null);itemRegistry=C("lt").getField("g").get(null);componentRegistry=C("lt").getField("aq").get(null);OUT.println("SOURCE:"+STACK.getProtectionDomain().getCodeSource().getLocation());}
 static void fullRegistries(Path metadata)throws Exception {
  JsonObject ids=JsonParser.parseString(Files.readString(metadata)).getAsJsonObject().getAsJsonObject("registries");Object lookup=call(C("pd"),"a",T());Map<Object,Object> registries=new LinkedHashMap<>();
  for(Object entry:((Stream<?>)call(access,"c",T())).toList())registries.put(call(entry,"a",T()),call(entry,"b",T()));
  for(Map.Entry<String,JsonElement> entry:ids.entrySet()){
   Object location=call(C("akr"),"a",T(String.class),entry.getKey()),registryKey=call(C("akq"),"a",T(C("akr")),location),values=call(lookup,"b",T(C("akq")),registryKey);Object registry=C("ju").getConstructor(C("akq"),Lifecycle.class).newInstance(registryKey,Lifecycle.stable());
   List<Map.Entry<String,JsonElement>> order=new ArrayList<>(entry.getValue().getAsJsonObject().entrySet());order.sort(Comparator.comparingInt(v->v.getValue().getAsInt()));
   for(int id=0;id<order.size();id++){
    Map.Entry<String,JsonElement> value=order.get(id);if(value.getValue().getAsInt()!=id)throw new AssertionError("noncontiguous officialregistry IDs");Object name=call(C("akr"),"a",T(String.class),value.getKey()),key=call(C("akq"),"a",T(C("akq"),C("akr")),registryKey,name);Object holder=((Optional<?>)call(values,"a",T(C("akq")),key)).orElseThrow();Object data=call(holder,"a",T());call(registry,"a",T(C("akq"),Object.class,C("jy")),key,data,C("jy").getField("a").get(null));
   }
   call(registry,"m",T());registries.put(registryKey,registry);
  }
  access=C("ka$c").getConstructor(List.class).newInstance(new ArrayList<>(registries.values()));OUT.println("ACCESS:originalpd bootstrap values; unmodifiedMappedRegistry; exactcaptured11configurationregistry IDs");
 }
 static JsonObject run(JsonObject q)throws Exception {
  byte[] raw=Base64.getDecoder().decode(q.get("bytes").getAsString());ByteBuf input=Unpooled.wrappedBuffer(raw),output=Unpooled.buffer();JsonObject result=new JsonObject();
  try{
   Object stack=call(slotCodec,"decode",T(Object.class),buf(input));result.addProperty("accepted",true);result.addProperty("consumed",input.readerIndex());result.addProperty("remaining",input.readableBytes());
   call(slotCodec,"encode",T(Object.class,Object.class),buf(output),stack);byte[] canonical=new byte[output.readableBytes()];output.getBytes(output.readerIndex(),canonical);result.addProperty("canonical",Base64.getEncoder().encodeToString(canonical));
   Object patch=call(stack,"d",T());result.addProperty("patch_count",(int)call(patch,"c",T()));JsonArray added=new JsonArray(),removed=new JsonArray();for(Object entry:(Set<?>)call(patch,"b",T())){Map.Entry<?,?> e=(Map.Entry<?,?>)entry;int id=(int)call(componentRegistry,"a",T(Object.class),e.getKey());if(((Optional<?>)e.getValue()).isPresent())added.add(id);else removed.add(id);}result.add("added",added);result.add("removed",removed);
   if(q.has("component_id")){
    int id=q.get("component_id").getAsInt();Object type=call(componentRegistry,"a",T(int.class),id),prototype=call(stack,"c",T()),value=call(prototype,"a",T(TYPE),type);ByteBuf payload=Unpooled.wrappedBuffer(Base64.getDecoder().decode(q.get("component_payload").getAsString()));
    try {Object decoded=call(call(type,"e",T()),"decode",T(Object.class),buf(payload));result.addProperty("prototype_equals_decoded",Objects.equals(value,decoded));result.addProperty("prototype_same_reference",value==decoded);result.addProperty("prototype_present",value!=null);result.addProperty("prototype_class",value==null?null:value.getClass().getName());result.addProperty("decoded_class",decoded==null?null:decoded.getClass().getName());result.addProperty("component_consumed",payload.readerIndex());result.addProperty("redundant_removed",call(patch,"a",T(TYPE),type)==null);}finally{payload.release();}
   }
  }catch(Throwable e){while(e instanceof InvocationTargetException&&e.getCause()!=null)e=e.getCause();result=new JsonObject();result.addProperty("accepted",false);result.addProperty("consumed",input.readerIndex());result.addProperty("error_class",e.getClass().getName());result.addProperty("error",e.getMessage());}
  finally{input.release();output.release();}return result;
 }
 public static void main(String[]args)throws Exception {init();if(args.length>0)fullRegistries(Path.of(args[0]));BufferedReader input=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=input.readLine())!=null;)OUT.println("ORACLE:"+JSON.toJson(run(JsonParser.parseString(line).getAsJsonObject())));}
}
