package oracle;

import com.google.gson.*;
import io.netty.buffer.*;
import java.io.*;
import java.lang.reflect.*;
import java.util.*;
import java.util.function.Function;

/** Calls unchanged official packet constructors, STREAM_CODECs and game-protocol visitor/codec. */
public final class item_effects_oracle {
    static final PrintStream OUT = System.out;
    static final Gson JSON = new GsonBuilder().serializeNulls().create();
    static final HexFormat HEX = HexFormat.of();
    static Class<?> REGISTRY, BUFFER, TAG_HOLDER, SOUND_SOURCE, BLOCK_POS;
    static Object access, ambient, soundRegistry, packetCodec;
    static Map<Object, Integer> packetIds = new HashMap<>();
    static Map<String, Object> sounds = new LinkedHashMap<>();
    static Class<?> C(String name) throws Exception { return Class.forName(name); }
    static Class<?>[] T(Class<?>... classes) { return classes; }
    static Object call(Object receiver, String name, Class<?>[] types, Object... args) throws Exception {
        Class<?> type = receiver instanceof Class<?> ? (Class<?>)receiver : receiver.getClass();
        Method method;
        try { method = type.getMethod(name, types); }
        catch (NoSuchMethodException e) { method = type.getDeclaredMethod(name, types); }
        method.setAccessible(true);
        return method.invoke(receiver instanceof Class<?> ? null : receiver, args);
    }
    static Object buf(ByteBuf bytes) throws Exception { return BUFFER.getConstructor(ByteBuf.class, C("ka")).newInstance(bytes, access); }
    static float f32(String raw) { return Float.intBitsToFloat(Integer.parseUnsignedInt(raw, 16)); }
    static double f64(String raw) { return Double.longBitsToDouble(Long.parseUnsignedLong(raw, 16)); }
    static String bits(float value) { return String.format("%08x", Float.floatToRawIntBits(value)); }
    static String bits(double value) { return String.format("%016x", Double.doubleToRawLongBits(value)); }
    static String bytes(ByteBuf bytes) {
        byte[] raw = new byte[bytes.readableBytes()]; bytes.getBytes(bytes.readerIndex(), raw); return HEX.formatHex(raw);
    }
    static int fieldInt(Object object, String name) throws Exception {
        Field f = object.getClass().getDeclaredField(name); f.setAccessible(true); return f.getInt(object);
    }
    static JsonArray vector(Object position) throws Exception {
        JsonArray result = new JsonArray();
        for (String getter : new String[]{"u", "v", "w"}) result.add((int)call(position, getter, T()));
        return result;
    }
    static JsonObject init() throws Exception {
        call(C("ab"), "a", T()); call(C("akt"), "a", T());
        REGISTRY = C("jz"); BUFFER = C("wk"); TAG_HOLDER = C("jm"); SOUND_SOURCE = C("avq"); BLOCK_POS = C("jd");
        access = call(C("ka"), "a", T(REGISTRY), C("lt").getField("aA").get(null));
        ambient = SOUND_SOURCE.getField("i").get(null);
        soundRegistry = C("lt").getField("b").get(null);
        Object template = C("agh").getField("b").get(null);
        Class<?> visitorType = C("wh$a$a");
        Object visitor = Proxy.newProxyInstance(visitorType.getClassLoader(), new Class<?>[]{visitorType}, (proxy, method, args) -> {
            if (method.getName().equals("accept")) packetIds.put(args[0], (Integer)args[1]);
            return null;
        });
        call(template, "a", T(visitorType), visitor);
        Function<ByteBuf, Object> decorate = bytes -> { try { return buf(bytes); } catch (Exception e) { throw new RuntimeException(e); } };
        Object protocol = call(template, "a", T(Function.class), decorate);
        packetCodec = call(protocol, "c", T());
        JsonObject header = new JsonObject(); header.addProperty("source", C("afp").getProtectionDomain().getCodeSource().getLocation().toString());
        header.addProperty("source_ordinal", ((Enum<?>)ambient).ordinal()); header.addProperty("source_name", (String)call(ambient, "a", T()));
        JsonObject ids = new JsonObject();
        for (Map.Entry<Object,Integer> entry : packetIds.entrySet()) {
            String name = call(entry.getKey(), "b", T()).toString();
            if (name.equals("minecraft:sound") || name.equals("minecraft:level_event")) ids.addProperty(name, entry.getValue());
        }
        header.add("packet_ids", ids);
        JsonArray info = new JsonArray();
        for (String name : new String[]{"minecraft:entity.generic.burn", "minecraft:entity.generic.extinguish_fire", "minecraft:entity.generic.splash"}) {
            Object location = call(C("akr"), "a", T(String.class), name);
            Object holder = ((Optional<?>)call(soundRegistry, "c", T(C("akr")), location)).orElseThrow();
            Object value = call(holder, "a", T()); sounds.put(name, holder);
            JsonObject sound = new JsonObject(); sound.addProperty("name", name);
            sound.addProperty("id", (int)call(soundRegistry, "a", T(Object.class), value));
            Optional<?> range = (Optional<?>)call(value, "b", T());
            sound.addProperty("fixed_range_bits", range.isPresent() ? bits((Float)range.get()) : null); info.add(sound);
        }
        header.add("sounds", info); return header;
    }
    static JsonObject encode(Object packet, Object codec) throws Exception {
        ByteBuf payload = Unpooled.buffer(), full = Unpooled.buffer();
        try {
            call(codec, "encode", T(Object.class, Object.class), buf(payload), packet);
            call(packetCodec, "encode", T(Object.class, Object.class), full, packet);
            JsonObject result = new JsonObject(); result.addProperty("packet_id", packetIds.get(call(packet, "a", T())));
            result.addProperty("payload_hex", bytes(payload)); result.addProperty("full_packet_hex", bytes(full));
            Object decoded = call(codec, "decode", T(Object.class), buf(payload));
            if (payload.isReadable()) throw new AssertionError("payload not fully consumed");
            if (packet.getClass() == C("adh")) result.add("decoded_position", vector(call(decoded, "g", T())));
            return result;
        } finally { payload.release(); full.release(); }
    }
    static JsonObject sound(JsonObject query) throws Exception {
        Object holder = sounds.get(query.get("sound_name").getAsString());
        if (holder == null || query.get("source_ordinal").getAsInt() != ((Enum<?>)ambient).ordinal()) throw new AssertionError("outside ItemEntity sound scope");
        JsonArray p = query.getAsJsonArray("position_bits");
        float volume = f32(query.get("volume_bits").getAsString()), pitch = f32(query.get("pitch_bits").getAsString());
        Object packet = C("afp").getConstructor(TAG_HOLDER, SOUND_SOURCE, double.class, double.class, double.class,
                float.class, float.class, long.class).newInstance(holder, ambient, f64(p.get(0).getAsString()),
                f64(p.get(1).getAsString()), f64(p.get(2).getAsString()), volume, pitch, query.get("seed").getAsLong());
        JsonObject result = encode(packet, C("afp").getField("a").get(null));
        JsonArray quantized = new JsonArray(), getters = new JsonArray();
        for (String field : new String[]{"e", "f", "g"}) quantized.add(fieldInt(packet, field));
        for (String method : new String[]{"f", "g", "h"}) getters.add(bits((double)call(packet, method, T())));
        result.add("quantized", quantized); result.add("getter_position_bits", getters);
        result.addProperty("range_bits", bits((float)call(call(holder, "a", T()), "a", T(float.class), volume)));
        return result;
    }
    static JsonObject level(JsonObject query) throws Exception {
        JsonArray p = query.getAsJsonArray("position");
        Object position = BLOCK_POS.getConstructor(int.class, int.class, int.class).newInstance(p.get(0).getAsInt(), p.get(1).getAsInt(), p.get(2).getAsInt());
        Object packet = C("adh").getConstructor(int.class, BLOCK_POS, int.class, boolean.class).newInstance(
                query.get("event").getAsInt(), position, query.get("data").getAsInt(), query.get("global").getAsBoolean());
        JsonObject result = encode(packet, C("adh").getField("a").get(null));
        result.addProperty("packed_position_hex", String.format("%016x", (long)call(position, "a", T())));
        return result;
    }
    public static void main(String[] args) throws Exception {
        OUT.println("HEADER:" + JSON.toJson(init()));
        BufferedReader input = new BufferedReader(new InputStreamReader(System.in));
        for (String line; (line = input.readLine()) != null;) {
            JsonObject query = JsonParser.parseString(line).getAsJsonObject();
            OUT.println("ORACLE:" + JSON.toJson(query.has("sound_name") ? sound(query) : level(query)));
        }
    }
}
