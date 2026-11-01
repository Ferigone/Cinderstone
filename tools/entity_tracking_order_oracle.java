package oracle;

import com.google.gson.*;
import it.unimi.dsi.fastutil.ints.Int2ObjectMap;
import it.unimi.dsi.fastutil.ints.Int2ObjectOpenHashMap;
import java.io.*;
import java.lang.reflect.*;

/** Executes the unchanged bundled fastutil default map and its real iterators.
 * No Minecraft or fastutil methods/classes are replaced or rewritten.
 */
public final class entity_tracking_order_oracle {
    static final Gson JSON=new GsonBuilder().serializeNulls().create();
    static final PrintStream OUT=System.out;
    static final Field CAPACITY,KEYS,ZERO,MINIMUM,MAXFILL;
    static {
        try {
            CAPACITY=field("n");KEYS=field("key");ZERO=field("containsNullKey");
            MINIMUM=field("minN");MAXFILL=field("maxFill");
        } catch(Exception e) { throw new ExceptionInInitializerError(e); }
    }
    static Field field(String name)throws Exception {
        Field field=Int2ObjectOpenHashMap.class.getDeclaredField(name);
        field.setAccessible(true);return field;
    }
    static JsonObject snapshot(Int2ObjectOpenHashMap<Integer> map,Integer old)throws Exception {
        JsonObject out=new JsonObject();JsonArray entries=new JsonArray();
        for(Int2ObjectMap.Entry<Integer> entry:map.int2ObjectEntrySet()) {
            JsonArray pair=new JsonArray();pair.add(entry.getIntKey());pair.add(entry.getValue());entries.add(pair);
        }
        JsonArray values=new JsonArray();for(Integer value:map.values())values.add(value);
        JsonArray slots=new JsonArray();for(int key:(int[])KEYS.get(map))slots.add(key);
        out.add("entries",entries);out.addProperty("size",map.size());
        out.add("old",old==null?JsonNull.INSTANCE:new JsonPrimitive(old));
        out.addProperty("capacity",CAPACITY.getInt(map));out.add("key_slots",slots);
        out.addProperty("contains_zero",ZERO.getBoolean(map));out.addProperty("min_capacity",MINIMUM.getInt(map));
        out.addProperty("max_fill",MAXFILL.getInt(map));out.add("values",values);return out;
    }
    static JsonArray query(JsonObject q)throws Exception {
        Int2ObjectOpenHashMap<Integer> map=new Int2ObjectOpenHashMap<>();JsonArray states=new JsonArray();
        for(JsonElement element:q.getAsJsonArray("ops")) {
            JsonObject op=element.getAsJsonObject();Integer old=null;
            switch(op.get("op").getAsString()) {
                case "put":old=map.put(op.get("key").getAsInt(),Integer.valueOf(op.get("value").getAsInt()));break;
                case "remove":old=map.remove(op.get("key").getAsInt());break;
                case "clear":map.clear();break;
                default:throw new IllegalArgumentException(op.toString());
            }
            states.add(snapshot(map,old));
        }
        return states;
    }
    public static void main(String[]args)throws Exception {
        OUT.println("SOURCE:"+Int2ObjectOpenHashMap.class.getProtectionDomain().getCodeSource().getLocation());
        BufferedReader input=new BufferedReader(new InputStreamReader(System.in));
        for(String line;(line=input.readLine())!=null;)try {
            OUT.println("ORACLE:"+JSON.toJson(query(JsonParser.parseString(line).getAsJsonObject())));
        }catch(Throwable error){OUT.println("QUERY:"+line);error.printStackTrace(OUT);throw error;}
    }
}
