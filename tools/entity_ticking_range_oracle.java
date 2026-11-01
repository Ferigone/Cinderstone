package oracle;
import com.google.gson.*;
import java.io.*;
import java.lang.reflect.*;
import java.util.*;
import jdk.internal.org.objectweb.asm.*;
import static jdk.internal.org.objectweb.asm.Opcodes.*;

/** Original ara/aqf/eoq/eou, plus actual DistanceManager source methods.
 * The three other DistanceManager trackers are original initialized instances;
 * they are not drained. Chunk scheduling callbacks belong to a separate oracle.
 */
public final class entity_ticking_range_oracle {
 static final Gson JSON=new GsonBuilder().serializeNulls().create();static final PrintStream OUT=System.out;
 static Class<?> C(String n)throws Exception{return Class.forName(n);}static Class<?>[] T(Class<?>...v){return v;}
 static Object call(Object o,String n,Class<?>[]t,Object...a)throws Exception{return survival_oracle.call(o,n,t,a);}
 static void set(Object o,String c,String n,Object v)throws Exception{survival_oracle.set(o,c,n,v);}
 static Object get(Object o,String c,String n)throws Exception{Field f=C(c).getDeclaredField(n);f.setAccessible(true);return f.get(o);}
 static Object unsafe,manager,tracker;static Method allocate;static Class<?> MANAGER;static Map<Integer,Object> players;
 static Object alloc(Class<?>c)throws Exception{return allocate.invoke(unsafe,c);}
 static Object construct(String c,Class<?>[]t,Object...a)throws Exception{Constructor<?> ctor=C(c).getDeclaredConstructor(t);ctor.setAccessible(true);return ctor.newInstance(a);}
 static Object chunk(JsonArray q)throws Exception{return construct("dcd",T(int.class,int.class),q.get(0).getAsInt(),q.get(1).getAsInt());}
 static JsonArray coordinates(long key){JsonArray a=new JsonArray();a.add((int)key);a.add((int)(key>>32));return a;}
 static Object kind(String name)throws Exception{return C("aqz").getField(switch(name){case"player"->"c";case"start"->"a";case"forced"->"d";default->throw new IllegalArgumentException("three-kind core only: "+name);}).get(null);}
 static Object key(String name,Object chunk)throws Exception{return name.equals("start")?C("azs").getField("a").get(null):chunk;}
 static Object player(int id)throws Exception {if(!players.containsKey(id)){Object p=alloc(C("aqv"));set(p,"bsr","o",id);players.put(id,p);}return players.get(id);}
 static Object section(JsonArray p)throws Exception{return call(C("kf"),"a",T(int.class,int.class,int.class),p.get(0).getAsInt(),0,p.get(1).getAsInt());}
 static void init()throws Exception {
  call(C("ab"),"a",T());call(C("akt"),"a",T());Class<?>u=C("sun.misc.Unsafe");Field f=u.getDeclaredField("theUnsafe");f.setAccessible(true);unsafe=f.get(null);allocate=u.getMethod("allocateInstance",Class.class);
  survival_oracle loader=new survival_oracle();String name="oracle/TickingDistanceManager";ClassWriter c=survival_oracle.begin(name,"aqk");survival_oracle.getterConst(c,"a","(J)Z",ICONST_0,IRETURN);survival_oracle.getterConst(c,"b","(J)Lapz;",ACONST_NULL,ARETURN);survival_oracle.getterConst(c,"a","(JILapz;I)Lapz;",ACONST_NULL,ARETURN);MANAGER=survival_oracle.define(loader,name,c);
  OUT.println("SOURCE:"+C("ara").getProtectionDomain().getCodeSource().getLocation());OUT.println("SOURCE_CHUNK:"+C("dcd").getField("a").getLong(null));
 }
 static JsonArray levels(Object source)throws Exception {TreeMap<Long,Integer> rows=new TreeMap<>();for(Object e:((Map<?,?>)source).entrySet()){Map.Entry<?,?> row=(Map.Entry<?,?>)e;rows.put(((Number)row.getKey()).longValue(),((Number)row.getValue()).intValue());}JsonArray out=new JsonArray();for(var row:rows.entrySet()){JsonArray a=coordinates(row.getKey());a.add(row.getValue());out.add(a);}return out;}
 static JsonArray sources(Object source,boolean clock)throws Exception {List<JsonArray> rows=new ArrayList<>();for(Object e:((Map<?,?>)source).entrySet()){Map.Entry<?,?> row=(Map.Entry<?,?>)e;for(Object t:(Iterable<?>)row.getValue()){JsonArray a=coordinates(((Number)row.getKey()).longValue());a.add(call(t,"a",T()).toString());a.add((int)call(t,"b",T()));if(clock)a.add((long)get(t,"aqy","d"));rows.add(a);}}rows.sort(Comparator.comparing(JSON::toJson));JsonArray out=new JsonArray();rows.forEach(out::add);return out;}
 static JsonObject snapshot(JsonArray points)throws Exception {
  JsonObject out=new JsonObject();out.add("levels",levels(get(tracker,"ara","b")));out.add("computed",levels(get(tracker,"eoq","c")));Object priority=get(tracker,"eoq","b");JsonArray queues=new JsonArray();for(Object queue:(Object[])get(priority,"eou","b")){JsonArray entries=new JsonArray();for(Object value:(Iterable<?>)queue)entries.add(coordinates(((Number)value).longValue()));queues.add(entries);}out.add("queues",queues);out.addProperty("queue_size",(int)call(tracker,"c",T()));out.addProperty("has_work",(boolean)call(tracker,"b",T()));out.addProperty("simulation_distance",(int)get(manager,"aqk","q"));out.addProperty("player_ticket_level",(int)call(manager,"g",T()));out.addProperty("ticket_tick",(long)get(manager,"aqk","p"));out.add("tickets",sources(get(tracker,"ara","d"),false));out.add("region_tickets",sources(get(manager,"aqk","e"),true));
  JsonArray queryLevels=new JsonArray();for(JsonElement point:points){Object ch=chunk(point.getAsJsonArray());int value=(int)call(tracker,"a",T(C("dcd")),ch);queryLevels.add(value);}out.add("query_levels",queryLevels);return out;
 }
 static JsonArray query(JsonObject q)throws Exception {
  manager=alloc(MANAGER);tracker=construct("ara",T());players=new HashMap<>();set(manager,"aqk","h",tracker);set(manager,"aqk","q",q.get("simulation_distance").getAsInt());set(manager,"aqk","p",q.has("ticket_tick")?q.get("ticket_tick").getAsLong():0L);set(manager,"aqk","d",construct("it.unimi.dsi.fastutil.longs.Long2ObjectOpenHashMap",T()));set(manager,"aqk","e",construct("it.unimi.dsi.fastutil.longs.Long2ObjectOpenHashMap",T()));set(manager,"aqk","f",construct("aqk$a",T(C("aqk")),manager));set(manager,"aqk","g",construct("aqk$b",T(C("aqk"),int.class),manager,8));set(manager,"aqk","i",construct("aqk$c",T(C("aqk"),int.class),manager,32));
  JsonArray out=new JsonArray();for(JsonElement value:q.getAsJsonArray("ops")){JsonObject op=value.getAsJsonObject();String name=op.get("op").getAsString();Integer remaining=null;boolean accepted=true;
   switch(name){
    case"player_add":call(manager,"a",T(C("kf"),C("aqv")),section(op.getAsJsonArray("chunk")),player(op.get("player").getAsInt()));break;
    case"player_remove":call(manager,"b",T(C("kf"),C("aqv")),section(op.getAsJsonArray("chunk")),player(op.get("player").getAsInt()));break;
    case"simulation":call(manager,"b",T(int.class),op.get("distance").getAsInt());break;
    case"replace_players":call(tracker,"a",T(int.class),op.get("level").getAsInt());break;
    case"raw_add":case"raw_remove":case"region_add":case"region_remove":{String type=op.get("kind").getAsString();Object ch=chunk(op.getAsJsonArray("chunk"));call(name.startsWith("region")?manager:tracker,switch(name){case"raw_add"->"a";case"raw_remove"->"b";case"region_add"->"c";default->"d";},T(C("aqz"),C("dcd"),int.class,Object.class),kind(type),ch,op.get(name.startsWith("region")?"distance":"level").getAsInt(),key(type,ch));break;}
    case"forced":call(manager,"a",T(C("dcd"),boolean.class),chunk(op.getAsJsonArray("chunk")),op.get("enabled").getAsBoolean());break;
    case"purge":for(int n=op.has("count")?op.get("count").getAsInt():1;n>0;n--)call(manager,"a",T());break;
    case"run":if(op.has("budget"))remaining=(int)call(tracker,"b",T(int.class),op.get("budget").getAsInt());else call(tracker,"a",T());break;
    default:throw new IllegalArgumentException(op.toString());
   }
   JsonObject state=snapshot(q.getAsJsonArray("points"));state.add("remaining",remaining==null?JsonNull.INSTANCE:new JsonPrimitive(remaining));out.add(state);
  }return out;
 }
 public static void main(String[]args)throws Exception {init();BufferedReader input=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=input.readLine())!=null;)try{OUT.println("ORACLE:"+JSON.toJson(query(JsonParser.parseString(line).getAsJsonObject())));}catch(Throwable error){OUT.println("QUERY:"+line);error.printStackTrace(OUT);throw error;}}
}
