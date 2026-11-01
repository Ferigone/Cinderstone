package oracle;

import com.google.gson.*;
import java.io.*;
import java.util.*;
import java.util.function.BiFunction;

/** Calls unchanged original CollisionGetter.findSupportingBlock and the original
 * BlockCollisions candidate iterator. World/actor adapters are shared with the
 * independent runtime collision oracle; no Minecraft class is rewritten.
 */
public final class entity_support_oracle {
    static final PrintStream OUT=System.out;static final Gson JSON=new GsonBuilder().serializeNulls().create();
    static Class<?> C(String n)throws Exception{return runtime_collision_oracle.C(n);}
    static Class<?>[] T(Class<?>...t){return t;}
    static Object call(Object o,String n,Class<?>[]t,Object...a)throws Exception{return runtime_collision_oracle.call(o,n,t,a);}
    static JsonArray pos(Object p)throws Exception {JsonArray a=new JsonArray();for(String n:new String[]{"u","v","w"})a.add((int)call(p,n,T()));return a;}
    static JsonObject query(JsonObject q)throws Exception {
        Map<String,Integer> states=new HashMap<>();for(JsonElement row:q.getAsJsonArray("blocks")){JsonArray a=row.getAsJsonArray();states.put(a.get(0).getAsInt()+","+a.get(1).getAsInt()+","+a.get(2).getAsInt(),a.get(3).getAsInt());}
        Object world=runtime_collision_oracle.world,actor=runtime_collision_oracle.actor;runtime_collision_oracle.WORLD.getField("states").set(world,states);JsonArray p=q.getAsJsonArray("position");runtime_collision_oracle.set(actor,"bsr","s",C("exc").getConstructor(double.class,double.class,double.class).newInstance(runtime_collision_oracle.d(p.get(0)),runtime_collision_oracle.d(p.get(1)),runtime_collision_oracle.d(p.get(2))));runtime_collision_oracle.ACTOR.getField("descending").setBoolean(actor,q.get("descending").getAsBoolean());runtime_collision_oracle.set(actor,"bsr","ac",Float.intBitsToFloat(Integer.parseUnsignedInt(q.get("fall").getAsString(),16)));Object bounds=runtime_collision_oracle.box(q.getAsJsonArray("bounds"));
        BiFunction<Object,Object,Object> copy=(position,shape)->{try{return call(position,"j",T());}catch(Exception e){throw new RuntimeException(e);}};
        Iterator<?> iterator=(Iterator<?>)C("dca").getConstructor(C("dcg"),C("bsr"),C("ewx"),boolean.class,BiFunction.class).newInstance(world,actor,bounds,false,copy);JsonArray positions=new JsonArray();while(iterator.hasNext())positions.add(pos(iterator.next()));
        Optional<?> support=(Optional<?>)C("dcg").getMethod("g",C("bsr"),C("ewx")).invoke(world,actor,bounds);JsonObject out=new JsonObject();out.add("positions",positions);out.add("support",support.isPresent()?pos(support.get()):JsonNull.INSTANCE);return out;
    }
    public static void main(String[]args)throws Exception {runtime_collision_oracle.init();placement_tags.bind(System.getProperty("mc121.blocktags","fixtures/placement-block-tags-1.21.json"));runtime_collision_oracle.adapters();OUT.println("SOURCE:"+C("dcg").getProtectionDomain().getCodeSource().getLocation());BufferedReader in=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=in.readLine())!=null;)try{OUT.println("ORACLE:"+JSON.toJson(query(JsonParser.parseString(line).getAsJsonObject())));}catch(Throwable error){OUT.println("QUERY:"+line);error.printStackTrace(OUT);throw error;}}
}
