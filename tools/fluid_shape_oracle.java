package oracle;

import com.google.gson.*;
import java.io.*;
import java.lang.reflect.*;
import java.util.*;

/** Original FluidState.getShape -> FlowingFluid.getShape, including untouched process caches. */
public final class fluid_shape_oracle {
    static final Gson JSON = new GsonBuilder().serializeNulls().create();
    static final PrintStream OUT = System.out;
    static final List<String> TRACE = new ArrayList<>();
    static final Map<String,Integer> ALIASES = new LinkedHashMap<>();
    static final Map<String,Object> OWNERS = new LinkedHashMap<>();
    static Field CACHE;
    static Object fluidRegistry, stateRegistry, world, aboveFluid;
    static Object aboveBlock;
    static int[] abovePosition;
    static boolean faultAbove;
    static Class<?> C(String name) throws Exception { return Class.forName(name); }
    static Class<?>[] T(Class<?>... values) { return values; }
    static Object call(Object receiver, String method, Class<?>[] types, Object... args) throws Exception {
        Class<?> type = receiver instanceof Class<?> ? (Class<?>)receiver : receiver.getClass();
        Method m = type.getMethod(method, types); m.setAccessible(true);
        return m.invoke(receiver instanceof Class<?> ? null : receiver, args);
    }
    static String bits(double value) { return String.format("%016x", Double.doubleToRawLongBits(value)); }
    static String ownBits(Object fluid) throws Exception { return String.format("%08x", Float.floatToRawIntBits((float)call(fluid,"d",T()))); }
    static Object state(JsonObject key) throws Exception {
        int kind = key.get("kind").getAsInt();
        if (kind == 0) return call(C("epf").getField("a").get(null), "g", T());
        boolean source = key.get("source").getAsBoolean(), falling = key.get("falling").getAsBoolean();
        Object type = C("epf").getField(kind == 1 ? source ? "c" : "b" : source ? "e" : "d").get(null);
        return source ? call(type, "a", T(boolean.class), falling)
                      : call(type, "a", T(int.class, boolean.class), key.get("amount").getAsInt(), falling);
    }
    static JsonObject key(Object fluid) throws Exception {
        Object type = call(fluid, "a", T());
        boolean empty = (boolean)call(fluid, "c", T());
        JsonObject key = new JsonObject();
        key.addProperty("kind", empty ? 0 : C("epk").isInstance(type) ? 1 : 2);
        key.addProperty("source", (boolean)call(fluid,"b",T()));
        key.addProperty("amount", (int)call(fluid,"e",T()));
        key.addProperty("falling", !empty && (boolean)call(fluid,"c",T(C("duf")), C("epc").getField("a").get(null)));
        return key;
    }
    static Object block(int id) throws Exception { return call(C("dfy"), "a", T(int.class), id); }
    static JsonArray boxes(Object shape) throws Exception {
        JsonArray result = new JsonArray();
        for (Object box : (List<?>)call(shape, "e", T())) {
            JsonArray values = new JsonArray();
            for (String field : new String[]{"a","b","c","d","e","f"}) values.add(bits(C("ewx").getField(field).getDouble(box)));
            result.add(values);
        }
        return result;
    }
    static Object vector(JsonArray values) throws Exception {
        double[] v=new double[3];for(int i=0;i<3;i++)v[i]=Double.longBitsToDouble(Long.parseUnsignedLong(values.get(i).getAsString(),16));
        return C("exc").getConstructor(double.class,double.class,double.class).newInstance(v[0],v[1],v[2]);
    }
    static JsonElement clip(Object shape,JsonObject q,Object position) throws Exception {
        Object hit=call(shape,"a",T(C("exc"),C("exc"),C("jd")),vector(q.getAsJsonArray("clip_from_bits")),vector(q.getAsJsonArray("clip_to_bits")),position);
        if(hit==null)return JsonNull.INSTANCE;
        JsonObject out=new JsonObject();JsonArray location=new JsonArray();Object vec=call(hit,"e",T());
        for(String field:new String[]{"c","d","e"})location.add(bits(C("exc").getField(field).getDouble(vec)));
        out.add("position_bits",location);out.addProperty("direction",((Enum<?>)call(hit,"b",T())).ordinal());out.addProperty("inside",(boolean)call(hit,"d",T()));
        out.add("block_position",JSON.toJsonTree(position(call(hit,"a",T()))));return out;
    }
    static JsonArray snapshot() throws Exception {
        JsonArray result = new JsonArray();
        for (Map.Entry<String,Object> owner : OWNERS.entrySet()) {
            JsonObject row = new JsonObject(); row.addProperty("owner", owner.getKey());
            List<Map.Entry<?,?>> entries = new ArrayList<>(((Map<?,?>)CACHE.get(owner.getValue())).entrySet());
            entries.sort(Comparator.comparingInt(e -> { try { return (int)call(stateRegistry,"a",T(Object.class),e.getKey()); } catch(Exception x) { throw new RuntimeException(x); } }));
            JsonArray cached = new JsonArray();
            for (Map.Entry<?,?> entry : entries) {
                JsonObject e = new JsonObject(); e.add("key", key(entry.getKey())); e.add("boxes", boxes(entry.getValue())); cached.add(e);
            }
            row.add("entries", cached); result.add(row);
        }
        return result;
    }
    static int[] position(Object pos) throws Exception { return new int[]{(int)call(pos,"u",T()), (int)call(pos,"v",T()), (int)call(pos,"w",T())}; }
    static String posString(int[] p) { return p[0]+","+p[1]+","+p[2]; }
    static JsonObject init() throws Exception {
        call(C("ab"),"a",T()); call(C("akt"),"a",T());
        fluidRegistry = C("lt").getField("c").get(null); stateRegistry = C("epd").getField("c").get(null);
        CACHE = C("epc").getDeclaredField("g"); CACHE.setAccessible(true);
        for (String field : new String[]{"c","b","e","d"}) {
            Object type = C("epf").getField(field).get(null);
            OWNERS.put(call(fluidRegistry,"b",T(Object.class),type).toString(),type);
        }
        for (int id = 0; id < 26684; id++) {
            Object state = block(id); String description = state.toString();
            if (description.startsWith("Block{minecraft:oak_slab}") && description.contains("type=bottom") && description.contains("waterlogged=true")) ALIASES.putIfAbsent("waterlogged_block_state",id);
            if (description.startsWith("Block{minecraft:water}") && (boolean)call(call(state,"u",T()),"b",T())) ALIASES.putIfAbsent("water_source_block_state",id);
            if (description.startsWith("Block{minecraft:lava}") && (boolean)call(call(state,"u",T()),"b",T())) ALIASES.putIfAbsent("lava_source_block_state",id);
        }
        Class<?> getter = C("dcc");
        world = Proxy.newProxyInstance(getter.getClassLoader(), new Class<?>[]{getter}, (proxy,m,args) -> {
            if (m.getDeclaringClass() == Object.class) return m.getName().equals("toString") ? "FluidShapeMappedWorld" : m.getName().equals("hashCode") ? System.identityHashCode(proxy) : proxy == args[0];
            if (m.getName().equals("b_") || m.getName().equals("a_")) {
                int[] p = position(args[0]); TRACE.add((m.getName().equals("b_") ? "fluid:" : "block:")+posString(p));
                if (!Arrays.equals(p,abovePosition)) throw new IllegalStateException("unexpected shape world query:"+posString(p));
                if (faultAbove) throw new IllegalStateException("oracle above-fluid query fault");
                return m.getName().equals("b_") ? aboveFluid : aboveBlock;
            }
            throw new UnsupportedOperationException("unexpected shape callback:"+m);
        });
        JsonObject header = new JsonObject(); header.addProperty("source", C("epc").getProtectionDomain().getCodeSource().getLocation().toString());
        header.add("aliases",JSON.toJsonTree(ALIASES));
        JsonObject aliasDescriptions=new JsonObject();for(Map.Entry<String,Integer> e:ALIASES.entrySet())aliasDescriptions.addProperty(e.getKey(),block(e.getValue()).toString());
        header.add("alias_descriptions",aliasDescriptions);
        JsonArray states = new JsonArray();
        for (int kind : new int[]{1,2}) for (boolean source : new boolean[]{true,false}) for (int amount = source ? 8 : 1; amount <= 8; amount++) for (boolean falling : new boolean[]{false,true}) {
            JsonObject k = new JsonObject(); k.addProperty("kind",kind); k.addProperty("source",source); k.addProperty("amount",amount); k.addProperty("falling",falling);
            Object fluid = state(k), type = call(fluid,"a",T());
            JsonObject row = new JsonObject(); row.add("key",key(fluid)); row.addProperty("fluid_state_id",(int)call(stateRegistry,"a",T(Object.class),fluid));
            row.addProperty("owner",call(fluidRegistry,"b",T(Object.class),type).toString()); row.addProperty("owner_class",type.getClass().getName());
            String description=fluid.toString();int properties=description.indexOf('[');
            row.addProperty("description",row.get("owner").getAsString()+(properties<0?"":description.substring(properties))); row.addProperty("own_height_bits",ownBits(fluid)); states.add(row);
        }
        header.add("states",states); header.add("initial_cache",snapshot()); return header;
    }
    static JsonObject query(JsonObject raw) throws Exception {
        JsonObject q = raw.deepCopy();
        if (q.has("state_alias")) { q.addProperty("block_state",ALIASES.get(q.remove("state_alias").getAsString())); }
        if (q.has("above_alias")) { q.addProperty("above_block_state",ALIASES.get(q.remove("above_alias").getAsString())); }
        Object fluid = q.has("block_state") ? call(block(q.get("block_state").getAsInt()),"u",T()) : state(q.getAsJsonObject("key"));
        JsonArray p = q.getAsJsonArray("position");
        int x=p.get(0).getAsInt(), y=p.get(1).getAsInt(), z=p.get(2).getAsInt(); abovePosition=new int[]{x,y+1,z};
        aboveBlock = block(q.has("above_block_state") ? q.get("above_block_state").getAsInt() : 0);
        aboveFluid = q.has("above_key") ? state(q.getAsJsonObject("above_key")) : call(aboveBlock,"u",T());
        faultAbove=q.has("fault_above") && q.get("fault_above").getAsBoolean(); TRACE.clear();
        JsonObject expected = new JsonObject(); expected.add("resolved_key",key(fluid));
        try {
            Object position=C("jd").getConstructor(int.class,int.class,int.class).newInstance(x,y,z);
            Object shape = call(fluid,"d",T(C("dcc"),C("jd")),world,position);
            expected.addProperty("accepted",true); expected.add("boxes",boxes(shape));
            if(q.has("clip_from_bits")){JsonElement hit=clip(shape,q,position);expected.addProperty("clip_hit",!hit.isJsonNull());expected.add("clip_result",hit);}
        } catch (Throwable e) {
            while (e instanceof InvocationTargetException || e instanceof UndeclaredThrowableException) e=e.getCause();
            expected.addProperty("accepted",false); expected.addProperty("error_class",e.getClass().getName()); expected.addProperty("error",e.getMessage());
        }
        expected.add("trace",JSON.toJsonTree(TRACE)); expected.add("cache_after",snapshot()); q.add("expected",expected); return q;
    }
    public static void main(String[]args) throws Exception {
        OUT.println("HEADER:"+JSON.toJson(init()));
        BufferedReader input=new BufferedReader(new InputStreamReader(System.in));
        for(String line;(line=input.readLine())!=null;) OUT.println("ORACLE:"+JSON.toJson(query(JsonParser.parseString(line).getAsJsonObject())));
    }
}
