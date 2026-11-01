package oracle;

import com.google.gson.*;
import com.mojang.serialization.*;
import java.io.*;
import java.lang.reflect.*;
import java.util.*;
import jdk.internal.org.objectweb.asm.*;

/** Calls unchanged, signed vanilla 1.21 classes, using reflection only for obfuscated names. */
public final class crafting_oracle {
    static final Gson GSON = new Gson();
    static Class<?> stack, input, recipeInput, level, provider;
    static Object registry, empty, lookup;
    static DynamicOps<JsonElement> ops = JsonOps.INSTANCE;
    static Codec<Object> stackCodec, recipeCodec;
    static Object mapLevel;
    static Class<?> mapLevelClass;
    static Object levelFor(JsonObject query) throws Exception {
        if (!query.has("map_context")) return null;
        if (mapLevel == null) {
            ClassWriter writer = new ClassWriter(0);
            writer.visit(Opcodes.V21,Opcodes.ACC_PUBLIC,"OracleMapLevel",null,"dcw",null);
            writer.visitField(Opcodes.ACC_PUBLIC|Opcodes.ACC_STATIC,"mapData","Leqt;",null,null).visitEnd();
            MethodVisitor method=writer.visitMethod(Opcodes.ACC_PUBLIC,"a","(Leqr;)Leqt;",null,null);
            method.visitCode();method.visitFieldInsn(Opcodes.GETSTATIC,"OracleMapLevel","mapData","Leqt;");method.visitInsn(Opcodes.ARETURN);method.visitMaxs(1,2);method.visitEnd();writer.visitEnd();
            byte[] bytes=writer.toByteArray();
            mapLevelClass=new ClassLoader(crafting_oracle.class.getClassLoader()) {Class<?> build(){return defineClass("OracleMapLevel",bytes,0,bytes.length);}}.build();
            Field field=Class.forName("sun.misc.Unsafe").getDeclaredField("theUnsafe");field.setAccessible(true);Object unsafe=field.get(null);
            mapLevel=Class.forName("sun.misc.Unsafe").getMethod("allocateInstance",Class.class).invoke(unsafe,mapLevelClass);
        }
        JsonObject info=query.getAsJsonObject("map_context");Object data=null;
        if (!info.has("missing") || !info.get("missing").getAsBoolean()) {
            data=Class.forName("eqt").getMethod("a",byte.class,boolean.class,Class.forName("akq")).invoke(null,(byte)info.get("scale").getAsInt(),false,level.getField("h").get(null));
            if (info.get("exploration").getAsBoolean()) {
                Object holder=Class.forName("eqp").getField("i").get(null);
                Object decoration=Class.forName("eqn").getConstructor(Class.forName("jm"),byte.class,byte.class,byte.class,Optional.class).newInstance(holder,(byte)0,(byte)0,(byte)0,Optional.empty());
                Class.forName("eqt").getMethod("a",List.class).invoke(data,List.of(decoration));
            }
        }
        mapLevelClass.getField("mapData").set(null,data);return mapLevel;
    }
    static Object item(String name) throws Exception {
        Object key = Class.forName("akr").getMethod("a", String.class).invoke(null, name);
        return Class.forName("jz").getMethod("a", Class.forName("akr")).invoke(registry, key);
    }
    static Object parseStack(JsonElement json) {
        if (json.isJsonNull() || json.getAsJsonObject().get("count").getAsInt() <= 0) return empty;
        return stackCodec.parse(ops, json).getOrThrow();
    }
    static JsonElement encode(Object value) {
        try { if ((boolean)stack.getMethod("e").invoke(value)) return JsonNull.INSTANCE; }
        catch (ReflectiveOperationException error) { throw new RuntimeException(error); }
        return stackCodec.encodeStart(ops, value).getOrThrow();
    }
    @SuppressWarnings("unchecked")
    public static void main(String[] args) throws Exception {
        Class.forName("ab").getMethod("a").invoke(null);
        Class.forName("akt").getMethod("a").invoke(null);
        stack = Class.forName("cuq"); input = Class.forName("cyp");
        recipeInput = Class.forName("czc"); level = Class.forName("dcw"); provider = Class.forName("jo$a");
        registry = Class.forName("lt").getField("g").get(null);
        empty = stack.getField("l").get(null);
        lookup = Class.forName("pd").getMethod("a").invoke(null);
        ops = (DynamicOps<JsonElement>)provider.getMethod("a",DynamicOps.class).invoke(lookup,JsonOps.INSTANCE);
        if (args.length > 0) {
            JsonObject tags = JsonParser.parseReader(new FileReader(args[0])).getAsJsonObject().getAsJsonObject("tags");
            Object key = Class.forName("lu").getField("K").get(null);
            Map<Object,List<Object>> bindings = new HashMap<>();
            for (Map.Entry<String,JsonElement> entry : tags.entrySet()) {
                Object name = Class.forName("akr").getMethod("a",String.class).invoke(null,"minecraft:"+entry.getKey());
                Object tag = Class.forName("awu").getMethod("a",Class.forName("akq"),Class.forName("akr")).invoke(null,key,name);
                List<Object> members = new ArrayList<>();
                for (JsonElement id : entry.getValue().getAsJsonArray()) members.add(((Optional<?>)Class.forName("jz").getMethod("c",int.class).invoke(registry,id.getAsInt())).orElseThrow());
                bindings.put(tag,members);
            }
            Class.forName("jz").getMethod("a",Map.class).invoke(registry,bindings);
        }
        // Bind the unchanged official curse tag for RepairItemRecipe's
        // world-registry scan. Other enchantment recipe tags are not consulted.
        Object enchantmentKey=Class.forName("lu").getField("aL").get(null);
        Object enchantments=provider.getMethod("b",Class.forName("akq")).invoke(lookup,enchantmentKey);
        Object curseName=Class.forName("akr").getMethod("a",String.class).invoke(null,"minecraft:curse");
        Object curseTag=Class.forName("awu").getMethod("a",Class.forName("akq"),Class.forName("akr")).invoke(null,enchantmentKey,curseName);
        Method bind=Class.forName("jm$c").getDeclaredMethod("a",Collection.class);bind.setAccessible(true);
        for (String name : List.of("minecraft:binding_curse","minecraft:vanishing_curse")) {
            Object location=Class.forName("akr").getMethod("a",String.class).invoke(null,name);
            Object key=Class.forName("akq").getMethod("a",Class.forName("akq"),Class.forName("akr")).invoke(null,enchantmentKey,location);
            Object holder=((Optional<?>)Class.forName("jn").getMethod("a",Class.forName("akq")).invoke(enchantments,key)).orElseThrow();
            bind.invoke(holder,List.of(curseTag));
        }
        stackCodec = (Codec<Object>)stack.getField("b").get(null);
        recipeCodec = (Codec<Object>)Class.forName("cyz").getField("h").get(null);
        BufferedReader reader = new BufferedReader(new InputStreamReader(System.in));
        for (String line; (line = reader.readLine()) != null;) {
            JsonObject query = JsonParser.parseString(line).getAsJsonObject();
            JsonObject out = new JsonObject();
            try {
                String op = query.get("op").getAsString();
                if (op.equals("slot_decode")) {
                    byte[] data=Base64.getDecoder().decode(query.get("bytes").getAsString());
                    Object access=Class.forName("ka").getMethod("a",Class.forName("jz")).invoke(null,Class.forName("lt").getField("aA").get(null));
                    io.netty.buffer.ByteBuf bytes=io.netty.buffer.Unpooled.wrappedBuffer(data);
                    try {
                        Object buffer=Class.forName("wk").getConstructor(io.netty.buffer.ByteBuf.class,Class.forName("ka")).newInstance(bytes,access);
                        Object codec=stack.getField("h").get(null);Class.forName("yx").getMethod("decode",Object.class).invoke(codec,buffer);out.addProperty("accepted",true);out.addProperty("remaining",bytes.readableBytes());
                    } finally {bytes.release();}
                } else if (op.equals("prototype_wire")) {
                    JsonObject prototypes=new JsonObject();JsonArray errors=new JsonArray();
                    Object access=Class.forName("ka").getMethod("a",Class.forName("jz")).invoke(null,Class.forName("lt").getField("aA").get(null));
                    Object componentRegistry=Class.forName("lt").getField("aq").get(null);
                    for (Object value : (Iterable<?>)registry) {
                        int itemId=(Integer)Class.forName("jz").getMethod("a",Object.class).invoke(registry,value);
                        Object components=Class.forName("cul").getMethod("p").invoke(value);JsonObject encoded=new JsonObject();
                        for (Object component : (Iterable<?>)components) {
                            Object type=Class.forName("ks").getMethod("a").invoke(component);Object contents=Class.forName("ks").getMethod("b").invoke(component);
                            int typeId=(Integer)Class.forName("jz").getMethod("a",Object.class).invoke(componentRegistry,type);
                            io.netty.buffer.ByteBuf bytes=io.netty.buffer.Unpooled.buffer();
                            try {
                                Object buffer=Class.forName("wk").getConstructor(io.netty.buffer.ByteBuf.class,Class.forName("ka")).newInstance(bytes,access);
                                Object codec=Class.forName("kp").getMethod("e").invoke(type);
                                Class.forName("yx").getMethod("encode",Object.class,Object.class).invoke(codec,buffer,contents);
                                byte[] data=new byte[bytes.readableBytes()];bytes.getBytes(0,data);encoded.addProperty(Integer.toString(typeId),Base64.getEncoder().encodeToString(data));
                            } catch (Throwable error) {errors.add(itemId+"/"+typeId+": "+error.getCause());}
                            finally {bytes.release();}
                        }
                        prototypes.add(Integer.toString(itemId),encoded);
                    }
                    out.add("prototypes",prototypes);out.add("errors",errors);
                } else if (op.equals("stew")) {
                    JsonObject stew = new JsonObject();
                    Codec<Object> codec = (Codec<Object>)Class.forName("cxw").getField("b").get(null);
                    for (Object value : (Iterable<?>)registry) {
                        Object holder = Class.forName("dob").getMethod("a",Class.forName("dcv")).invoke(null,value);
                        if (holder != null) {
                            Object effects=Class.forName("dob").getMethod("b").invoke(holder);
                            Object key=Class.forName("jz").getMethod("b",Object.class).invoke(registry,value);
                            stew.add(key.toString(),codec.encodeStart(ops,effects).getOrThrow());
                        }
                    }
                    out.add("stew",stew);
                } else if (op.equals("dyes")) {
                    JsonObject dyes = new JsonObject();
                    for (Object color : (Object[])Class.forName("cti").getMethod("values").invoke(null)) {
                        JsonObject data = new JsonObject();
                        data.addProperty("id",(Integer)Class.forName("cti").getMethod("a").invoke(color));
                        data.addProperty("texture",(Integer)Class.forName("cti").getMethod("d").invoke(color));
                        data.addProperty("firework",(Integer)Class.forName("cti").getMethod("f").invoke(color));
                        dyes.add((String)Class.forName("cti").getMethod("b").invoke(color),data);
                    }
                    out.add("dyes",dyes);
                } else if (op.equals("remainders")) {
                    JsonObject remainders = new JsonObject();
                    for (Object value : (Iterable<?>)registry) {
                        Object remainder = Class.forName("cul").getMethod("u").invoke(value);
                        if (remainder != null) {
                            Object key = Class.forName("jz").getMethod("b", Object.class).invoke(registry, value);
                            Object remainingKey = Class.forName("jz").getMethod("b", Object.class).invoke(registry, remainder);
                            remainders.addProperty(key.toString(), remainingKey.toString());
                        }
                    }
                    out.add("remainders", remainders);
                } else if (op.equals("stack")) {
                    Object value = parseStack(query.get("stack"));
                    if (query.has("count_override")) stack.getMethod("e",int.class).invoke(value,query.get("count_override").getAsInt());
                    out.add("stack", encode(value));
                } else {
                    Object recipe = recipeCodec.parse(ops, query.get("recipe")).getOrThrow();
                    List<Object> items = new ArrayList<>();
                    for (JsonElement entry : query.getAsJsonArray("items")) items.add(parseStack(entry));
                    Object actualInput;
                    String kind = query.get("kind").getAsString();
                    if (kind.equals("crafting")) {
                        // Vanilla trims the occupied rectangle before Recipe.matches is called.
                        actualInput = input.getMethod("a", int.class, int.class, List.class)
                            .invoke(null, query.get("width").getAsInt(), query.get("height").getAsInt(), items);
                    } else if (kind.equals("smithing")) {
                        actualInput = Class.forName("czs").getConstructor(stack, stack, stack).newInstance(items.get(0),items.get(1),items.get(2));
                    } else {
                        actualInput = Class.forName("czp").getConstructor(stack).newInstance(items.get(0));
                    }
                    Method matches = Class.forName("cyz").getMethod("a",recipeInput,level);
                    boolean match = (boolean)matches.invoke(recipe,actualInput,levelFor(query));
                    out.addProperty("matches",match);
                    if (match) {
                        Object result = Class.forName("cyz").getMethod("a", recipeInput,provider).invoke(recipe,actualInput,lookup);
                        out.add("result",encode(result));
                        Object post=stack.getMethod("a",Class.forName("kp")).invoke(result,Class.forName("kq").getField("D").get(null));
                        if (post!=null) out.addProperty("map_post_processing",(Integer)post.getClass().getMethod("a").invoke(post));
                        JsonArray remaining = new JsonArray();
                        for (Object v : (Iterable<?>)Class.forName("cyz").getMethod("a",recipeInput).invoke(recipe,actualInput)) {
                            remaining.add((boolean)stack.getMethod("e").invoke(v) ? JsonNull.INSTANCE : encode(v));
                        }
                        out.add("remaining",remaining);
                    }
                }
            } catch (Throwable error) {
                Throwable cause = error instanceof InvocationTargetException ? error.getCause() : error;
                out.addProperty("error", cause.toString());
            }
            System.out.println("ORACLE:" + GSON.toJson(out));
        }
    }
}
