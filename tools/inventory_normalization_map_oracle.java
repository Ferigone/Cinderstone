package oracle;

import com.google.gson.*;
import io.netty.buffer.*;
import java.io.*;
import java.nio.file.Path;
import java.util.*;

/** Observes unchanged original STREAM decoding, record/map equality and identity-hash-dependent map order. */
public final class inventory_normalization_map_oracle {
    static final Class<?>[] NONE = new Class<?>[0];
    static Object trimType, armorRegistry;
    static Object call(Object receiver, String method, Class<?>[] types, Object... args) throws Exception {
        return inventory_normalization_oracle.call(receiver, method, types, args);
    }
    static Object decode(String wire) throws Exception {
        ByteBuf bytes = Unpooled.wrappedBuffer(Base64.getDecoder().decode(wire));
        try {
            Object stack = call(inventory_normalization_oracle.slotCodec, "decode", new Class<?>[]{Object.class}, inventory_normalization_oracle.buf(bytes));
            if (bytes.isReadable()) throw new AssertionError("unconsumed Slot bytes");
            return call(stack, "a", new Class<?>[]{inventory_normalization_oracle.TYPE}, trimType);
        } finally { bytes.release(); }
    }
    static Object material(Object trim) throws Exception { return call(call(trim, "b", NONE), "a", NONE); }
    static Map<?,?> overrides(Object material) throws Exception { return (Map<?,?>)call(material, "d", NONE); }
    static JsonArray entries(Map<?,?> map) throws Exception {
        JsonArray rows = new JsonArray();
        for (Map.Entry<?,?> e : map.entrySet()) {
            Object holder = e.getKey(), value = call(holder, "a", NONE);
            JsonObject row = new JsonObject();
            row.addProperty("holder_id", (int)call(armorRegistry, "a", new Class<?>[]{Object.class}, value));
            row.addProperty("suffix", (String)e.getValue());
            row.addProperty("holder_identity_hash", System.identityHashCode(holder));
            rows.add(row);
        }
        return rows;
    }
    static JsonObject roundtrip(String wire) throws Exception {
        JsonObject query = new JsonObject(); query.addProperty("bytes", wire);
        return inventory_normalization_oracle.run(query);
    }
    static JsonObject run(JsonObject query) throws Exception {
        String input = query.get("input").getAsString(), reference = query.get("reference").getAsString();
        // Perform the unmodified original roundtrip before inspecting hash fields.
        JsonObject inputRoundtrip = roundtrip(input), referenceRoundtrip = roundtrip(reference);
        Object left = decode(input), right = decode(reference), lm = material(left), rm = material(right);
        Map<?,?> a = overrides(lm), b = overrides(rm);
        JsonObject result = new JsonObject();
        result.add("input_roundtrip", inputRoundtrip); result.add("reference_roundtrip", referenceRoundtrip);
        result.addProperty("trim_equals", left.equals(right)); result.addProperty("material_equals", lm.equals(rm));
        result.addProperty("map_equals", a.equals(b)); result.addProperty("map_class", a.getClass().getName());
        result.add("input_entries", entries(a)); result.add("reference_entries", entries(b));
        return result;
    }
    public static void main(String[] args) throws Exception {
        inventory_normalization_oracle.init(); inventory_normalization_oracle.fullRegistries(Path.of(args[0]));
        armorRegistry = Class.forName("lt").getField("ap").get(null);
        trimType = call(inventory_normalization_oracle.componentRegistry, "a", new Class<?>[]{int.class}, 35);
        for (int i = 0; i < Integer.parseInt(args[1]); i++) System.identityHashCode(new Object());
        BufferedReader input = new BufferedReader(new InputStreamReader(System.in));
        for (String line; (line = input.readLine()) != null;)
            inventory_normalization_oracle.OUT.println("MAP:" + inventory_normalization_oracle.JSON.toJson(run(JsonParser.parseString(line).getAsJsonObject())));
    }
}
