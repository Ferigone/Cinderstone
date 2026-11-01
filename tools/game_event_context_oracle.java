package oracle;

import com.google.gson.*;
import java.io.*;
import java.lang.reflect.*;
import java.util.*;
import java.util.concurrent.atomic.AtomicLong;
import jdk.internal.org.objectweb.asm.*;
import static jdk.internal.org.objectweb.asm.Opcodes.*;

/** Runs original dya.post and original dvi.getListenerRegistry/dxy.visit.
 * Only oracle-owned world/chunk-source adapters are generated. No original
 * Minecraft class is patched; missing chunks and empty registries are inputs.
 */
public final class game_event_context_oracle {
    static final Gson JSON=new GsonBuilder().serializeNulls().create();
    static final PrintStream OUT=System.out;
    static Class<?> WORLD,CACHE,CHUNK;
    static Object world,cache;
    static String loadedMode,registryMode;
    static final Map<String,Object> chunks=new LinkedHashMap<>();
    static JsonArray trace;
    static Class<?> C(String name)throws Exception{return Class.forName(name);}
    static Object get(Object target,String owner,String name)throws Exception {
        Field field=C(owner).getDeclaredField(name);field.setAccessible(true);return field.get(target);
    }
    static void set(Object target,String owner,String name,Object value)throws Exception {survival_oracle.set(target,owner,name,value);}
    static Object alloc(Class<?> type)throws Exception{return survival_oracle.alloc(type);}
    static void event(Object...values){JsonArray row=new JsonArray();for(Object value:values){if(value instanceof Number n)row.add(n);else if(value instanceof Boolean b)row.add(b);else row.add(value.toString());}trace.add(row);}
    public static Object source(){event("source");return cache;}
    public static boolean noop(){return registryMode.equals("noop");}
    public static Object chunk(int x,int z)throws Exception {
        boolean present=switch(loadedMode){case "all"->true;case "none"->false;case "checker"->((x*31+z*17)&1)==0;default->throw new IllegalArgumentException(loadedMode);};
        event("chunk",x,z,present);if(!present)return null;
        String key=x+","+z;Object found=chunks.get(key);if(found!=null)return found;
        Object value=alloc(CHUNK);CHUNK.getField("x").setInt(value,x);CHUNK.getField("z").setInt(value,z);
        set(value,"dvi","r",world);set(value,"dvi","u",C("it.unimi.dsi.fastutil.ints.Int2ObjectOpenHashMap").getConstructor().newInstance());chunks.put(key,value);return value;
    }
    public static void registry(Object chunk,int y)throws Exception {
        event("registry",CHUNK.getField("x").getInt(chunk),y,CHUNK.getField("z").getInt(chunk),registryMode);
    }
    static void adapters()throws Exception {
        survival_oracle loader=new survival_oracle();String owner="oracle/game_event_context_oracle";
        ClassWriter c=survival_oracle.begin("oracle/GameEventLevel","aqu");
        MethodVisitor m=survival_oracle.method(c,"l","()Laqs;");m.visitMethodInsn(INVOKESTATIC,owner,"source","()Ljava/lang/Object;",false);m.visitTypeInsn(CHECKCAST,"aqs");m.visitInsn(ARETURN);survival_oracle.end(m);WORLD=survival_oracle.define(loader,"oracle/GameEventLevel",c);
        c=survival_oracle.begin("oracle/GameEventChunkSource","aqs");m=survival_oracle.method(c,"a","(II)Ldvi;");m.visitVarInsn(ILOAD,1);m.visitVarInsn(ILOAD,2);m.visitMethodInsn(INVOKESTATIC,owner,"chunk","(II)Ljava/lang/Object;",false);m.visitTypeInsn(CHECKCAST,"dvi");m.visitInsn(ARETURN);survival_oracle.end(m);CACHE=survival_oracle.define(loader,"oracle/GameEventChunkSource",c);
        c=survival_oracle.begin("oracle/GameEventChunk","dvi");survival_oracle.field(c,"x","I");survival_oracle.field(c,"z","I");
        m=survival_oracle.method(c,"a","(I)Ldyc;");m.visitVarInsn(ALOAD,0);m.visitVarInsn(ILOAD,1);m.visitMethodInsn(INVOKESTATIC,owner,"registry","(Ljava/lang/Object;I)V",false);m.visitMethodInsn(INVOKESTATIC,owner,"noop","()Z",false);Label original=new Label();m.visitJumpInsn(IFEQ,original);m.visitFieldInsn(GETSTATIC,"dyc","a","Ldyc;");m.visitInsn(ARETURN);m.visitLabel(original);m.visitVarInsn(ALOAD,0);m.visitVarInsn(ILOAD,1);m.visitMethodInsn(INVOKESPECIAL,"dvi","a","(I)Ldyc;",false);m.visitInsn(ARETURN);survival_oracle.end(m);CHUNK=survival_oracle.define(loader,"oracle/GameEventChunk",c);
        world=alloc(WORLD);cache=alloc(CACHE);
    }
    static long seed(Object random)throws Exception {return ((AtomicLong)get(random,"dyz","h")).get();}
    static JsonObject query(JsonObject query)throws Exception {
        trace=new JsonArray();chunks.clear();loadedMode=query.get("loaded").getAsString();registryMode=query.get("registry").getAsString();
        Object random=C("dyz").getConstructor(long.class).newInstance(123456789L);set(world,"dcw","z",random);long before=seed(random);
        JsonArray position=query.getAsJsonArray("position_bits");double[] p=new double[3];for(int i=0;i<3;i++)p[i]=Double.longBitsToDouble(Long.parseUnsignedLong(position.get(i).getAsString(),16));
        Object point=C("exc").getConstructor(double.class,double.class,double.class).newInstance(p[0],p[1],p[2]);
        Object holder;
        if(query.has("event")) {
            String fieldName=switch(query.get("event").getAsString()){case "hit_ground"->"A";case "splash"->"O";case "entity_damage"->"o";default->throw new IllegalArgumentException(query.toString());};
            holder=C("dxz").getField(fieldName).get(null);
            Object definition=C("jm").getMethod("a").invoke(holder);
            if((Integer)C("dxz").getMethod("a").invoke(definition)!=query.get("radius").getAsInt())throw new IllegalArgumentException("event radius mismatch");
        } else {
            Object definition=C("dxz").getConstructor(int.class).newInstance(query.get("radius").getAsInt());
            holder=C("jm").getMethod("a",Object.class).invoke(null,definition);
        }
        Object context=C("dxz$a").getConstructor(C("bsr"),C("dtc")).newInstance(null,null);
        Object dispatcher=C("dya").getConstructor(C("aqu")).newInstance(world);
        C("dya").getMethod("a",C("jm"),C("exc"),C("dxz$a")).invoke(dispatcher,holder,point,context);
        JsonArray registryStates=new JsonArray();int registryCount=0;
        for(Object chunk:chunks.values()) {
            Map<?,?> values=(Map<?,?>)get(chunk,"dvi","u");
            for(Map.Entry<?,?> entry:values.entrySet()) {
                Object registry=entry.getValue();JsonObject row=new JsonObject();row.addProperty("x",CHUNK.getField("x").getInt(chunk));row.addProperty("y",((Number)entry.getKey()).intValue());row.addProperty("z",CHUNK.getField("z").getInt(chunk));
                row.addProperty("active",((Collection<?>)get(registry,"dxy","b")).size());row.addProperty("pending_remove",((Collection<?>)get(registry,"dxy","c")).size());row.addProperty("pending_add",((Collection<?>)get(registry,"dxy","d")).size());row.addProperty("processing",(Boolean)get(registry,"dxy","e"));registryStates.add(row);registryCount++;
            }
        }
        JsonObject out=new JsonObject();out.add("trace",trace);out.addProperty("loaded_chunks",chunks.size());out.addProperty("materialized_registries",registryCount);out.add("registry_states",registryStates);out.addProperty("world_rng_before",Long.toUnsignedString(before));out.addProperty("world_rng_after",Long.toUnsignedString(seed(random)));return out;
    }
    public static void main(String[]args)throws Exception {
        C("ab").getMethod("a").invoke(null);C("akt").getMethod("a").invoke(null);Class<?> unsafe=C("sun.misc.Unsafe");Field field=unsafe.getDeclaredField("theUnsafe");field.setAccessible(true);survival_oracle.unsafe=field.get(null);survival_oracle.allocate=unsafe.getMethod("allocateInstance",Class.class);adapters();
        for(String name:new String[]{"dya","dvi","dxy","dyc$1","agf"})OUT.println("SOURCE:"+name+":"+C(name).getProtectionDomain().getCodeSource().getLocation());
        BufferedReader input=new BufferedReader(new InputStreamReader(System.in));for(String line;(line=input.readLine())!=null;)try{OUT.println("ORACLE:"+JSON.toJson(query(JsonParser.parseString(line).getAsJsonObject())));}catch(Throwable error){OUT.println("QUERY:"+line);error.printStackTrace(OUT);throw error;}
    }
}
