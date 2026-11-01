package oracle;
import com.google.gson.*;
import com.google.common.collect.Comparators;
import it.unimi.dsi.fastutil.longs.LongOpenHashSet;
import java.io.*;
import java.lang.reflect.*;
import java.util.*;
import java.util.stream.*;

/** Actual shipped fastutil and Guava; comparator invokes original ChunkPos
 * distanceSquared. No Minecraft/dependency methods or classes are rewritten. */
public final class player_chunk_sender_order_oracle {
 static final Gson JSON=new GsonBuilder().serializeNulls().create();static final PrintStream OUT=System.out;
 static boolean traceFallback;static int fallbackComparisons;
 static Class<?> POS;static Constructor<?> POSITION;static Method DISTANCE;
 static JsonArray array(long[]values){JsonArray out=new JsonArray();for(long value:values)out.add(Long.toString(value));return out;}
 static JsonArray array(Collection<Long>values){JsonArray out=new JsonArray();for(long value:values)out.add(Long.toString(value));return out;}
 static Object field(Object target,String name)throws Exception{Field f=LongOpenHashSet.class.getDeclaredField(name);f.setAccessible(true);return f.get(target);}
 static long key(JsonElement value){return Long.parseLong(value.getAsString());}
 static JsonObject snapshot(LongOpenHashSet set,boolean changed,long key)throws Exception{
  JsonObject out=new JsonObject();out.addProperty("changed",changed);out.addProperty("contains",set.contains(key));out.addProperty("size",set.size());out.addProperty("capacity",(int)field(set,"n"));out.addProperty("min_capacity",(int)field(set,"minN"));out.addProperty("max_fill",(int)field(set,"maxFill"));out.addProperty("contains_zero",(boolean)field(set,"containsNull"));out.add("key_slots",array((long[])field(set,"key")));
  List<Long>iterator=new ArrayList<>();set.iterator().forEachRemaining((long value)->iterator.add(value));out.add("iterator",array(iterator));out.add("stream",array(set.stream().toList()));out.add("long_stream",array(set.longStream().toArray()));return out;
 }
 static Comparator<Long> comparator(Object center){return Comparator.comparingInt(value->{try{if(traceFallback){boolean trim=false,sort=false;for(StackTraceElement frame:Thread.currentThread().getStackTrace()){if(frame.getClassName().equals("com.google.common.collect.TopKSelector")&&frame.getMethodName().equals("trim"))trim=true;if(frame.getClassName().equals("java.util.Arrays")&&frame.getMethodName().equals("sort"))sort=true;}if(trim&&sort)fallbackComparisons++;}return(int)DISTANCE.invoke(center,value.longValue());}catch(Exception e){throw new RuntimeException(e);}});}
 static List<Long> direct(JsonArray input){List<Long>values=new ArrayList<>();for(JsonElement e:input)values.add(key(e));return values;}
 static JsonObject selection(JsonObject q)throws Exception{
  LongOpenHashSet actualSet=null;List<Long>input;traceFallback=q.has("trace_fallback")&&q.get("trace_fallback").getAsBoolean();fallbackComparisons=0;if(q.has("input"))input=direct(q.getAsJsonArray("input"));else{LongOpenHashSet pending=new LongOpenHashSet();for(JsonElement e:q.getAsJsonArray("ops")){JsonObject op=e.getAsJsonObject();if(op.get("op").getAsString().equals("insert"))pending.add(key(op.get("key")));else pending.remove(key(op.get("key")));}input=pending.stream().toList();actualSet=pending;}
  JsonArray c=q.getAsJsonArray("center");Object center=POSITION.newInstance(c.get(0).getAsInt(),c.get(1).getAsInt());Comparator<Long>comparator=comparator(center);int k=q.get("k").getAsInt();
  // A sequential Stream preserves the actual encounter sequence supplied by
  // the real pending-set spliterator, despite its UNORDERED characteristic.
  List<Long>result=(actualSet==null?input.stream():actualSet.stream()).collect(Comparators.least(k,comparator));List<Long>full=(actualSet==null?input.stream():actualSet.stream()).sorted(comparator).toList();
  Class<?>topk=Class.forName("com.google.common.collect.TopKSelector");Method least=topk.getDeclaredMethod("least",int.class,Comparator.class),offer=topk.getDeclaredMethod("offer",Object.class),finish=topk.getDeclaredMethod("topK");for(Method m:new Method[]{least,offer,finish})m.setAccessible(true);Field size=topk.getDeclaredField("bufferSize"),threshold=topk.getDeclaredField("threshold");size.setAccessible(true);threshold.setAccessible(true);Object selector=least.invoke(null,k,comparator);int trims=0;
  for(Long value:input){int before=size.getInt(selector);Object previous=threshold.get(selector);offer.invoke(selector,value);int after=size.getInt(selector);if(k>0&&before==2*k-1&&after==k&&(k!=1||!Objects.equals(previous,threshold.get(selector))))trims++;}
  if(!result.equals(finish.invoke(selector)))throw new AssertionError("actual collector/selector disagreement");
  JsonArray distances=new JsonArray();for(long value:input)distances.add((int)DISTANCE.invoke(center,value));JsonObject out=new JsonObject();out.add("input",array(input));out.add("distances",distances);out.add("least",array(result));out.addProperty("trim_count",trims);out.add("fallback_observed",traceFallback?new JsonPrimitive(fallbackComparisons>0):JsonNull.INSTANCE);out.add("full_sorted",array(full));return out;
 }
 static JsonElement query(JsonObject q)throws Exception{
  if(q.get("kind").getAsString().equals("select"))return selection(q);
  LongOpenHashSet pending=new LongOpenHashSet();JsonArray result=new JsonArray();for(JsonElement e:q.getAsJsonArray("ops")){JsonObject op=e.getAsJsonObject();long key=key(op.get("key"));boolean changed=switch(op.get("op").getAsString()){case"insert"->pending.add(key);case"remove"->pending.remove(key);default->throw new IllegalArgumentException(op.toString());};result.add(snapshot(pending,changed,key));}return result;
 }
 public static void main(String[]args)throws Exception{
  Class.forName("ab").getMethod("a").invoke(null);Class.forName("akt").getMethod("a").invoke(null);POS=Class.forName("dcd");POSITION=POS.getConstructor(int.class,int.class);DISTANCE=POS.getMethod("c",long.class);
  OUT.println("SOURCE:fastutil:"+LongOpenHashSet.class.getProtectionDomain().getCodeSource().getLocation());OUT.println("SOURCE:guava:"+Comparators.class.getProtectionDomain().getCodeSource().getLocation());OUT.println("SOURCE:minecraft:"+POS.getProtectionDomain().getCodeSource().getLocation());
  BufferedReader in=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=in.readLine())!=null;)OUT.println("ORACLE:"+JSON.toJson(query(JsonParser.parseString(line).getAsJsonObject())));
 }
}
