package oracle;

import com.google.gson.*;
import java.io.*;
import java.lang.reflect.*;
import java.util.*;
import java.util.function.Consumer;
import jdk.internal.org.objectweb.asm.*;

/** Calls unchanged ServerEntity.sendChanges and sendPairingData on actual ItemEntity.
 * The existing item oracle supplies only an environmental Level; no Entity,
 * ItemEntity, SynchedEntityData, ServerEntity, or packet method is overridden.
 * Mutation inputs call original ItemStack/ItemEntity methods or set the two
 * externally owned impulse flags. This is a sender proof, not a whole tick proof.
 */
public final class item_replication_oracle {
    public static Object pickupInventory;
    static Object pickupPlayer;
    static int pickupCapacity;
    static JsonArray pickupEvents = new JsonArray();
    static JsonArray emissionStates = new JsonArray();
    static Object currentTracker, currentItem;
    static Class<?> cls(String name) throws Exception { return Class.forName(name); }
    static Object get(Object value, Class<?> owner, String name) throws Exception {
        Field field = owner.getDeclaredField(name); field.setAccessible(true); return field.get(value);
    }
    static void set(Object value, Class<?> owner, String name, Object input) throws Exception {
        Field field = owner.getDeclaredField(name); field.setAccessible(true); field.set(value, input);
    }
    static JsonArray vector(Object value) throws Exception {
        JsonArray out = new JsonArray();
        for (String field : new String[]{"c", "d", "e"})
            out.add(String.format("%016x", Double.doubleToRawLongBits(cls("exc").getField(field).getDouble(value))));
        return out;
    }
    static JsonObject packet(Object packet) throws Exception {
        String name = packet.getClass().getName(), method, bufferType; int id;
        switch (name) {
            case "abv": id=0x01; method="a"; bufferType="wk"; break;
            case "ado$a": id=0x2e; method="b"; bufferType="vw"; break;
            case "ado$b": id=0x2f; method="b"; bufferType="vw"; break;
            case "ado$c": id=0x30; method="b"; bufferType="vw"; break;
            case "aej": id=0x48; method="a"; bufferType="vw"; break;
            case "aez": id=0x58; method="b"; bufferType="wk"; break;
            case "afb": id=0x5a; method="a"; bufferType="vw"; break;
            case "afw": id=0x70; method="a"; bufferType="vw"; break;
            default: throw new IllegalStateException("Unexpected packet: " + name);
        }
        Object bytes = cls("io.netty.buffer.Unpooled").getMethod("buffer").invoke(null);
        Object registry = cls("ka").getMethod("a", cls("jz")).invoke(null, cls("lt").getField("aA").get(null));
        Object buffer = cls("wk").getConstructor(cls("io.netty.buffer.ByteBuf"), cls("ka")).newInstance(bytes, registry);
        Method write = packet.getClass().getDeclaredMethod(method, cls(bufferType)); write.setAccessible(true); write.invoke(packet, buffer);
        byte[] encoded = new byte[(Integer)cls("io.netty.buffer.ByteBuf").getMethod("readableBytes").invoke(bytes)];
        cls("io.netty.buffer.ByteBuf").getMethod("readBytes", byte[].class).invoke(bytes, (Object)encoded);
        cls("io.netty.buffer.ByteBuf").getMethod("release").invoke(bytes);
        JsonObject out = new JsonObject(); out.addProperty("id", id); out.addProperty("payload", Base64.getEncoder().encodeToString(encoded)); return out;
    }
    static JsonArray packets(List<Object> values) throws Exception {
        JsonArray out = new JsonArray(); for (Object packet : values) out.add(packet(packet)); return out;
    }
    static void emit(List<Object> packets, Object value) {
        try {
            packets.add(value); JsonObject state = new JsonObject();
            state.addProperty("packet_class", value.getClass().getName());
            state.addProperty("tick_count", (Integer)get(currentTracker, cls("aqt"), "p"));
            state.addProperty("has_impulse", (Boolean)get(currentItem, cls("bsr"), "av"));
            state.addProperty("hurt_marked", (Boolean)get(currentItem, cls("bsr"), "U"));
            state.addProperty("dirty", (Boolean)cls("aka").getMethod("a").invoke(cls("bsr").getMethod("ar").invoke(currentItem)));
            state.add("sent_velocity_bits", vector(cls("aqt").getMethod("c").invoke(currentTracker)));
            emissionStates.add(state);
        } catch (Exception error) { throw new RuntimeException(error); }
    }
    static String stackWire(Object stack) throws Exception {
        Object bytes = cls("io.netty.buffer.Unpooled").getMethod("buffer").invoke(null);
        Object registry = cls("ka").getMethod("a", cls("jz")).invoke(null, cls("lt").getField("aA").get(null));
        Object buffer = cls("wk").getConstructor(cls("io.netty.buffer.ByteBuf"), cls("ka")).newInstance(bytes, registry);
        cls("yx").getMethod("encode", Object.class, Object.class).invoke(cls("cuq").getField("h").get(null), buffer, stack);
        byte[] encoded = new byte[(Integer)cls("io.netty.buffer.ByteBuf").getMethod("readableBytes").invoke(bytes)];
        cls("io.netty.buffer.ByteBuf").getMethod("readBytes", byte[].class).invoke(bytes, (Object)encoded);
        cls("io.netty.buffer.ByteBuf").getMethod("release").invoke(bytes);
        return Base64.getEncoder().encodeToString(encoded);
    }
    static Class<?> define(String name, byte[] data) {
        return new ClassLoader(item_replication_oracle.class.getClassLoader()) {
            Class<?> build() { return defineClass(name, data, 0, data.length); }
        }.build();
    }
    /** Only external Player/Inventory boundaries are controlled; playerTouch itself is original. */
    static void pickupAdapters() throws Exception {
        ClassWriter writer = new ClassWriter(0);
        writer.visit(Opcodes.V21, Opcodes.ACC_PUBLIC, "OracleSenderInventory", null, "cmw", null);
        MethodVisitor m = writer.visitMethod(Opcodes.ACC_PUBLIC, "f", "(Lcuq;)Z", null, null);
        m.visitCode(); m.visitVarInsn(Opcodes.ALOAD, 1);
        m.visitMethodInsn(Opcodes.INVOKESTATIC, "oracle/item_replication_oracle", "acceptPickup", "(Ljava/lang/Object;)Z", false);
        m.visitInsn(Opcodes.IRETURN); m.visitMaxs(1, 2); m.visitEnd(); writer.visitEnd();
        pickupInventory = item_entity_oracle.unsafe(define("OracleSenderInventory", writer.toByteArray()));
        writer = new ClassWriter(0);
        writer.visit(Opcodes.V21, Opcodes.ACC_PUBLIC, "OracleSenderPlayer", null, "cmx", null);
        m = writer.visitMethod(Opcodes.ACC_PUBLIC, "fY", "()Lcmw;", null, null); m.visitCode();
        m.visitFieldInsn(Opcodes.GETSTATIC, "oracle/item_replication_oracle", "pickupInventory", "Ljava/lang/Object;");
        m.visitTypeInsn(Opcodes.CHECKCAST, "cmw"); m.visitInsn(Opcodes.ARETURN); m.visitMaxs(1, 1); m.visitEnd();
        for (String[] spec : new String[][]{{"(Lbsr;I)V", "collectPickup"}, {"(Lavw;I)V", "statPickup"}}) {
            m = writer.visitMethod(Opcodes.ACC_PUBLIC, "a", spec[0], null, null); m.visitCode();
            m.visitVarInsn(Opcodes.ILOAD, 2); m.visitMethodInsn(Opcodes.INVOKESTATIC, "oracle/item_replication_oracle", spec[1], "(I)V", false);
            m.visitInsn(Opcodes.RETURN); m.visitMaxs(1, 3); m.visitEnd();
        }
        m = writer.visitMethod(Opcodes.ACC_PUBLIC, "a", "(Lcjh;)V", null, null); m.visitCode();
        m.visitMethodInsn(Opcodes.INVOKESTATIC, "oracle/item_replication_oracle", "finishedPickup", "()V", false);
        m.visitInsn(Opcodes.RETURN); m.visitMaxs(0, 2); m.visitEnd(); writer.visitEnd();
        pickupPlayer = item_entity_oracle.unsafe(define("OracleSenderPlayer", writer.toByteArray()));
    }
    public static boolean acceptPickup(Object stack) throws Exception {
        int before = (Integer)cls("cuq").getMethod("H").invoke(stack), take = Math.min(before, Math.max(0, pickupCapacity));
        cls("cuq").getMethod("h", int.class).invoke(stack, take);
        JsonObject event = new JsonObject(); event.addProperty("callback", "inventory_add"); event.addProperty("before", before); event.addProperty("after", before-take); event.addProperty("accepted", take>0); pickupEvents.add(event);
        return take>0;
    }
    public static void collectPickup(int count) { JsonObject event = new JsonObject(); event.addProperty("callback", "collect"); event.addProperty("count", count); pickupEvents.add(event); }
    public static void statPickup(int count) { JsonObject event = new JsonObject(); event.addProperty("callback", "stat"); event.addProperty("count", count); pickupEvents.add(event); }
    public static void finishedPickup() { JsonObject event = new JsonObject(); event.addProperty("callback", "on_pickup"); pickupEvents.add(event); }
    static JsonElement metadata(Object item, Object values) throws Exception {
        return values == null ? JsonNull.INSTANCE : packet(cls("aez").getConstructor(int.class, List.class).newInstance(17, values));
    }
    static void position(Object item, JsonObject value) throws Exception {
        cls("bsr").getMethod("a_", double.class, double.class, double.class).invoke(item, value.get("x").getAsDouble(), value.get("y").getAsDouble(), value.get("z").getAsDouble());
        cls("bsr").getMethod("t", float.class).invoke(item, value.get("yaw").getAsFloat());
        cls("bsr").getMethod("u", float.class).invoke(item, value.get("pitch").getAsFloat());
        set(item, cls("bsr"), "aG", value.get("on_ground").getAsBoolean());
    }
    static void velocity(Object item, JsonArray value) throws Exception {
        cls("bsr").getMethod("n", double.class, double.class, double.class).invoke(item, value.get(0).getAsDouble(), value.get(1).getAsDouble(), value.get(2).getAsDouble());
    }
    static Object stack(JsonElement value) { return item_entity_oracle.codec.parse(item_entity_oracle.ops, value).getOrThrow(); }
    static Object item(JsonObject input, int id) throws Exception {
        JsonObject q = new JsonObject(); q.add("stack", input.get("stack")); q.addProperty("age", 0); q.addProperty("pickup_delay", 0);
        Object item = item_entity_oracle.plainItem(q, id);
        cls("bsr").getMethod("a_", UUID.class).invoke(item, UUID.fromString("00000000-0000-4000-8000-000000000001"));
        if (input.has("position")) position(item, input.getAsJsonObject("position"));
        if (input.has("velocity")) velocity(item, input.getAsJsonArray("velocity"));
        if (input.has("no_gravity")) cls("bsr").getMethod("f", boolean.class).invoke(item, input.get("no_gravity").getAsBoolean());
        return item;
    }
    static void mutate(Object item, JsonObject q) throws Exception {
        pickupEvents = new JsonArray();
        if (q.has("position")) position(item, q.getAsJsonObject("position"));
        if (q.has("velocity")) velocity(item, q.getAsJsonArray("velocity"));
        if (q.has("stack")) cls("cjh").getMethod("a", cls("cuq")).invoke(item, stack(q.get("stack")));
        if (q.has("empty_reference") && q.get("empty_reference").getAsBoolean()) cls("cjh").getMethod("a", cls("cuq")).invoke(item, cls("cuq").getField("l").get(null));
        Object current = cls("cjh").getMethod("p").invoke(item);
        if (q.has("count")) cls("cuq").getMethod("e", int.class).invoke(current, q.get("count").getAsInt());
        if (q.has("shrink")) cls("cuq").getMethod("h", int.class).invoke(current, q.get("shrink").getAsInt());
        if (q.has("set_same_reference") && q.get("set_same_reference").getAsBoolean()) cls("cjh").getMethod("a", cls("cuq")).invoke(item, current);
        if (q.has("pickup")) {
            pickupCapacity = q.get("pickup").getAsInt(); cls("cjh").getMethod("b_", cls("cmx")).invoke(item, pickupPlayer);
        }
        if (q.has("no_gravity")) cls("bsr").getMethod("f", boolean.class).invoke(item, q.get("no_gravity").getAsBoolean());
        if (q.has("silent")) cls("bsr").getMethod("e", boolean.class).invoke(item, q.get("silent").getAsBoolean());
        if (q.has("air_supply")) cls("bsr").getMethod("j", int.class).invoke(item, q.get("air_supply").getAsInt());
        if (q.has("ticks_frozen")) cls("bsr").getMethod("k", int.class).invoke(item, q.get("ticks_frozen").getAsInt());
        if (q.has("shared_flags")) {
            Object data = cls("bsr").getMethod("ar").invoke(item);
            cls("aka").getMethod("a", cls("ajw"), Object.class).invoke(data, get(null, cls("bsr"), "ap"), (byte)q.get("shared_flags").getAsInt());
        }
        if (q.has("impulse")) set(item, cls("bsr"), "av", q.get("impulse").getAsBoolean());
        if (q.has("hurt")) set(item, cls("bsr"), "U", q.get("hurt").getAsBoolean());
        for (String name : new String[]{"merge_destination", "merge_source"}) if (q.has(name)) {
            JsonObject otherQuery = new JsonObject(); otherQuery.add("stack", q.get(name)); Object other = item(otherQuery, 19);
            Object otherStack = cls("cjh").getMethod("p").invoke(other);
            Method merge = cls("cjh").getDeclaredMethod("a", cls("cjh"), cls("cuq"), cls("cjh"), cls("cuq")); merge.setAccessible(true);
            if (name.equals("merge_destination")) merge.invoke(null, item, current, other, otherStack);
            else merge.invoke(null, other, otherStack, item, current);
        }
    }
    static JsonObject snapshot(Object item, Object tracker, List<Object> emitted) throws Exception {
        JsonObject out = new JsonObject(); out.add("packets", packets(emitted));
        out.add("emission_states", emissionStates.deepCopy());
        JsonObject spawn = new JsonObject(); spawn.add("position_bits", vector(cls("aqt").getMethod("b").invoke(tracker)));
        spawn.addProperty("yaw_bits", String.format("%08x", Float.floatToRawIntBits((Float)cls("aqt").getMethod("e").invoke(tracker))));
        spawn.addProperty("pitch_bits", String.format("%08x", Float.floatToRawIntBits((Float)cls("aqt").getMethod("d").invoke(tracker))));
        out.add("spawn", spawn); out.add("sent_velocity_bits", vector(cls("aqt").getMethod("c").invoke(tracker)));
        out.add("cached_metadata", metadata(item, get(tracker, cls("aqt"), "u")));
        Object data = cls("bsr").getMethod("ar").invoke(item);
        out.addProperty("dirty", (Boolean)cls("aka").getMethod("a").invoke(data));
        int dirtyMask = 0; Object entries = get(data, cls("aka"), "e");
        for (int i = 0; i < Array.getLength(entries); i++) if ((Boolean)cls("aka$b").getMethod("c").invoke(Array.get(entries, i))) dirtyMask |= 1 << i;
        out.addProperty("dirty_mask", dirtyMask);
        out.addProperty("has_impulse", (Boolean)get(item, cls("bsr"), "av"));
        out.addProperty("hurt_marked", (Boolean)get(item, cls("bsr"), "U"));
        out.addProperty("tick_count", (Integer)get(tracker, cls("aqt"), "p"));
        out.addProperty("teleport_delay", (Integer)get(tracker, cls("aqt"), "q"));
        out.add("physical_position_bits", vector(cls("bsr").getMethod("dn").invoke(item)));
        out.add("physical_velocity_bits", vector(cls("bsr").getMethod("dr").invoke(item)));
        Object stack = cls("cjh").getMethod("p").invoke(item);
        out.add("physical_stack", (Boolean)cls("cuq").getMethod("e").invoke(stack) ? JsonNull.INSTANCE : item_entity_oracle.codec.encodeStart(item_entity_oracle.ops, stack).getOrThrow());
        out.addProperty("physical_stack_wire", stackWire(stack));
        out.addProperty("physical_stack_reference_is_default", stack == cls("cuq").getField("l").get(null));
        out.addProperty("physical_removed", (Boolean)cls("bsr").getMethod("dJ").invoke(item));
        out.add("pickup_callbacks", pickupEvents.deepCopy());
        return out;
    }
    public static void main(String[] args) throws Exception {
        item_entity_oracle.main(new String[]{"tracking"});
        pickupAdapters();
        for (String name : new String[]{"aqt", "cjh", "aka", "cuq"}) System.out.println("SOURCE:" + name + ":" + cls(name).getProtectionDomain().getCodeSource().getLocation());
        BufferedReader reader = new BufferedReader(new InputStreamReader(System.in));
        for (String line; (line = reader.readLine()) != null;) {
            JsonObject query = JsonParser.parseString(line).getAsJsonObject(), out = query.deepCopy();
            try {
                Object item = item(query.getAsJsonObject("initial"), 17); List<Object> emitted = new ArrayList<>();
                pickupEvents = new JsonArray(); emissionStates = new JsonArray(); currentItem = item;
                out.getAsJsonObject("initial").addProperty("stack_wire", stackWire(cls("cjh").getMethod("p").invoke(item)));
                Object tracker = cls("aqt").getConstructor(cls("aqu"), cls("bsr"), int.class, boolean.class, Consumer.class).newInstance(null, item, 20, true, (Consumer<Object>)(value -> emit(emitted, value)));
                currentTracker = tracker;
                out.add("initial_expected", snapshot(item, tracker, emitted));
                JsonArray results = new JsonArray();
                for (JsonElement entry : query.getAsJsonArray("steps")) {
                    JsonObject step = entry.getAsJsonObject().deepCopy(); mutate(item, step); emitted.clear(); emissionStates = new JsonArray();
                    for (String key : new String[]{"stack", "merge_destination", "merge_source"}) if (step.has(key)) step.addProperty(key + "_wire", stackWire(stack(step.get(key))));
                    boolean pairOnly = step.has("op") && step.get("op").getAsString().equals("pair");
                    if (!pairOnly) cls("aqt").getMethod("a").invoke(tracker);
                    JsonObject result = snapshot(item, tracker, emitted);
                    if (pairOnly || step.has("pair") && step.get("pair").getAsBoolean()) {
                        emitted.clear(); emissionStates = new JsonArray(); cls("aqt").getMethod("a", cls("aqv"), Consumer.class).invoke(tracker, null, (Consumer<Object>)(value -> emit(emitted, value)));
                        result.add("pairing_packets", packets(emitted));
                        result.add("pairing_emission_states", emissionStates.deepCopy());
                    }
                    step.add("expected", result); results.add(step);
                }
                out.add("steps", results);
            } catch (Throwable error) {
                while (error instanceof InvocationTargetException && error.getCause() != null) error = error.getCause();
                out.addProperty("error", error.toString()); error.printStackTrace(System.err);
            }
            System.out.println("ORACLE:" + item_entity_oracle.G.toJson(out));
        }
    }
}
