import com.google.gson.*;
import com.mojang.serialization.JsonOps;
import it.unimi.dsi.fastutil.objects.ObjectArrayList;
import java.io.PrintStream;
import java.lang.reflect.*;
import java.nio.file.*;
import java.security.MessageDigest;
import java.util.*;

/** Exact original-class component witness; no server or generation status claim. */
public final class EmptyBeardifierProbe {
  static final PrintStream OUT=System.out;
  static final String INNER="8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01";
  static Object shared(String name,Class<?>[] types,Object...args)throws Exception {
    Method m=Class.forName("oracle.WorldgenSurfaceOracle").getDeclaredMethod(name,types);
    m.setAccessible(true);return m.invoke(null,args);
  }
  static Object sharedField(String name)throws Exception {
    Field f=Class.forName("oracle.WorldgenSurfaceOracle").getDeclaredField(name);
    f.setAccessible(true);return f.get(null);
  }
  static String sha(byte[] data)throws Exception {
    return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(data));
  }
  static String bits(double value){return String.format("%016x",Double.doubleToRawLongBits(value));}
  static List<String> bounds(dyr f){return List.of(bits(f.a()),bits(f.b()));}
  static Object compute(dyr f,dyr.b context) {
    try{return Map.of("value_bits",bits(f.a(context)));}
    catch(RuntimeException e){return Map.of("error_class",e.getClass().getName(),"error_message",e.getMessage());}
  }
  static Object fill(dyr f) {
    List<Integer> indices=new ArrayList<>();
    dyr.a provider=new dyr.a(){
      public dyr.b a(int i){indices.add(i);return new dyr.e(i,0,0);}
      public void a(double[] values,dyr function){for(int i=0;i<values.length;i++)values[i]=function.a(a(i));}
    };
    double[] values=new double[3];f.a(values,provider);
    return Map.of("values",Arrays.stream(values).mapToObj(EmptyBeardifierProbe::bits).toList(),"for_index_calls",indices);
  }
  static dyr visited(dyr raw,dzc noise,Method wrap) {
    return raw.a((dyr.f)function->{try{return (dyr)wrap.invoke(noise,function);}catch(ReflectiveOperationException e){throw new RuntimeException(e);}});
  }
  @SuppressWarnings({"unchecked","rawtypes"}) public static void main(String[] args)throws Exception {
    shared("setup",new Class<?>[0]);
    String actual=sha(Files.readAllBytes(Path.of(dyl.class.getProtectionDomain().getCodeSource().getLocation().toURI())));
    if(!INNER.equals(actual))throw new AssertionError("original JAR differs");
    Object registry=sharedField("registry");
    jm<dzd> holder=(jm<dzd>)shared("getHolder",new Class<?>[]{Object.class,String.class},registry,"minecraft:overworld");
    dzd settings=holder.a();
    Object settingsKey=shared("key",new Class<?>[]{Object.class,String.class},registry,"minecraft:overworld");
    dzm state=(dzm)dzm.class.getMethod("a",jn.a.class,akq.class,long.class).invoke(null,sharedField("getters"),settingsKey,0L);
    Method pickerMethod=dzb.class.getDeclaredMethod("a",dzd.class);pickerMethod.setAccessible(true);
    dyk.a picker=(dyk.a)pickerMethod.invoke(null,settings);
    Field markerField=Class.forName("dys$b").getField("a");markerField.setAccessible(true);
    dys.c marker=(dys.c)markerField.get(null);
    dyl empty=new dyl(new ObjectArrayList().iterator(),new ObjectArrayList().iterator());
    jz<ddw> biomeRegistry=(jz<ddw>)sharedField("biomeRegistry");
    dvs markerTarget=new dvs(new dcd(0,0),dvv.a,dcy.e(-64,384),biomeRegistry,null);
    dvs emptyTarget=new dvs(new dcd(0,0),dvv.a,dcy.e(-64,384),biomeRegistry,null);
    dzc markerNoise=dzc.a(markerTarget,state,marker,settings,picker,eaa.a());
    dzc emptyNoise=dzc.a(emptyTarget,state,empty,settings,picker,eaa.a());
    Method wrap=dzc.class.getDeclaredMethod("a",dyr.class);wrap.setAccessible(true);
    String input="{\"type\":\"minecraft:min\",\"argument1\":-1,\"argument2\":{\"type\":\"minecraft:add\",\"argument1\":{\"type\":\"minecraft:beardifier\"},\"argument2\":{\"type\":\"minecraft:interpolated\",\"argument\":0}}}";
    dyr raw=dyr.b.parse(JsonOps.INSTANCE,JsonParser.parseString(input)).getOrThrow();
    dyr wrappedMarker=visited(raw,markerNoise,wrap),wrappedEmpty=visited(raw,emptyNoise,wrap);
    Object markerResult=compute(wrappedMarker,markerNoise),emptyResult=compute(wrappedEmpty,emptyNoise);
    if(!markerResult.equals(Map.of("value_bits",bits(-1.0))))throw new AssertionError("marker witness changed");
    if(!emptyResult.equals(Map.of("error_class","java.lang.IllegalStateException","error_message","Trying to sample interpolator outside the interpolation loop")))throw new AssertionError("empty-owner witness changed: "+emptyResult);
    Map<String,Object> result=new LinkedHashMap<>();
    result.put("minecraft","1.21");result.put("protocol",767);result.put("original_inner_sha256",actual);
    result.put("expression",JsonParser.parseString(input));
    result.put("raw_marker_bounds",bounds(marker));result.put("actual_empty_beardifier_bounds",bounds(empty));
    result.put("raw_expression_bounds",bounds(raw));result.put("marker_visited_bounds",bounds(wrappedMarker));result.put("empty_visited_bounds",bounds(wrappedEmpty));
    result.put("owner_before_interpolation_marker",markerResult);result.put("owner_before_interpolation_empty",emptyResult);
    result.put("external_marker",compute(wrappedMarker,new dyr.e(0,0,0)));result.put("external_empty",compute(wrappedEmpty,new dyr.e(0,0,0)));
    result.put("raw_marker_fill",fill(marker));result.put("actual_empty_fill",fill(empty));
    result.put("marker_replaced_with_actual_owner",wrap.invoke(emptyNoise,marker)==empty);
    result.put("scope","Original constructors, CODEC and actual NoiseChunk visitor; deliberate marker control versus actual zero-list Beardifier. No server, structure selection, full terrain replay or ChunkStatus advancement.");
    OUT.println(new Gson().toJson(result));
  }
}
