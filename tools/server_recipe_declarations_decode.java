package oracle;

import com.google.gson.*;
import io.netty.buffer.*;
import java.io.*;
import java.security.MessageDigest;
import java.util.*;

/** Decode captured original-server agc bytes with the unchanged shipped codecs. */
public final class server_recipe_declarations_decode {
    static Class<?> type(String name) throws Exception {
        return server_recipe_book_oracle.C(name);
    }

    static Object call(Object target, String name, Class<?>[] types, Object... args) throws Exception {
        return server_recipe_book_oracle.call(target, name, types, args);
    }

    static String sha(byte[] bytes) throws Exception {
        return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
    }

    static byte[] encode(Object codec, Object value) throws Exception {
        ByteBuf raw = Unpooled.buffer();
        try {
            Object buffer = type("wk").getConstructor(ByteBuf.class, type("ka"))
                .newInstance(raw, server_recipe_book_oracle.access);
            call(codec, "encode", new Class<?>[] {Object.class, Object.class}, buffer, value);
            byte[] bytes = new byte[raw.readableBytes()];
            raw.getBytes(raw.readerIndex(), bytes);
            return bytes;
        } finally {
            raw.release();
        }
    }

    static JsonObject decode(String filename) throws Exception {
        JsonObject source = JsonParser.parseReader(new FileReader(filename)).getAsJsonObject();
        byte[] bytes = Base64.getDecoder().decode(source.get("payload").getAsString());
        Object codec = type("agc").getField("a").get(null);
        ByteBuf raw = Unpooled.wrappedBuffer(bytes);
        Object packet;
        int remaining;
        try {
            Object buffer = type("wk").getConstructor(ByteBuf.class, type("ka"))
                .newInstance(raw, server_recipe_book_oracle.access);
            packet = call(codec, "decode", new Class<?>[] {Object.class}, buffer);
            remaining = raw.readableBytes();
        } finally {
            raw.release();
        }
        byte[] again = encode(codec, packet);
        JsonArray recipes = new JsonArray();
        Object holderCodec = type("czb").getField("a").get(null);
        for (Object holder : (Collection<?>)call(packet, "b", new Class<?>[] {})) {
            Object value = call(holder, "b", new Class<?>[] {});
            byte[] payload = encode(holderCodec, holder);
            JsonObject recipe = new JsonObject();
            recipe.addProperty("id", call(holder, "a", new Class<?>[] {}).toString());
            recipe.addProperty("is_special", (boolean)call(value, "as_", new Class<?>[] {}));
            recipe.addProperty("value_class", value.getClass().getName());
            recipe.addProperty("holder_payload_sha256", sha(payload));
            recipe.addProperty("holder_payload_bytes", payload.length);
            recipes.add(recipe);
        }
        JsonObject result = new JsonObject();
        result.addProperty("payload_sha256", sha(bytes));
        result.addProperty("payload_bytes", bytes.length);
        result.addProperty("remaining_bytes", remaining);
        result.addProperty("roundtrip_equal", Arrays.equals(bytes, again));
        result.addProperty("reencoded_sha256", sha(again));
        result.addProperty("decoded_count", recipes.size());
        result.add("recipes", recipes);
        return result;
    }

    public static void main(String[] args) throws Exception {
        server_recipe_book_oracle.init(args[0]);
        JsonArray results = new JsonArray();
        for (int i = 1; i < args.length; i++) results.add(decode(args[i]));
        JsonObject output = new JsonObject();
        output.addProperty("original_inner_sha256", server_recipe_book_oracle.SHA);
        JsonObject sources = new JsonObject();
        for (String name : new String[] {"agc", "czb", "cyz"})
            sources.addProperty(name, type(name).getProtectionDomain().getCodeSource().getLocation().toString());
        output.add("class_sources", sources);
        output.add("results", results);
        server_recipe_book_oracle.OUT.println("ORACLE:" + server_recipe_book_oracle.JSON.toJson(output));
    }
}
