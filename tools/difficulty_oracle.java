// SPDX-License-Identifier: GPL-2.0-only
// Run against the unchanged, obfuscated 1.21 server and its bundled libraries.
package oracle;
import com.google.gson.*;
import com.mojang.serialization.Dynamic;
import com.mojang.serialization.DynamicOps;
import io.netty.buffer.Unpooled;
import java.lang.reflect.Method;
import java.util.HexFormat;

public class difficulty_oracle {
    @SuppressWarnings({"unchecked", "rawtypes"})
    public static void main(String[] args) throws Exception {
        JsonObject result = new JsonObject();
        JsonArray packets = new JsonArray();
        Class<?> packetClass = Class.forName("ach");
        Class<?> bufferClass = Class.forName("vw");
        Class<?> difficultyClass = Class.forName("bqo");
        Method write = packetClass.getDeclaredMethod("a", bufferClass);
        write.setAccessible(true);
        for (int difficulty = 0; difficulty < 4; difficulty++) {
            for (boolean locked : new boolean[] {false, true}) {
                var buffer = Unpooled.buffer();
                try {
                    Object wrapped = bufferClass.getConstructor(io.netty.buffer.ByteBuf.class).newInstance(buffer);
                    Object value = difficultyClass.getMethod("a", int.class).invoke(null, difficulty);
                    Object packet = packetClass.getConstructor(difficultyClass, boolean.class).newInstance(value, locked);
                    write.invoke(packet, wrapped);
                    byte[] bytes = new byte[buffer.readableBytes()];
                    buffer.readBytes(bytes);
                    JsonObject entry = new JsonObject();
                    entry.addProperty("difficulty", difficulty);
                    entry.addProperty("locked", locked);
                    entry.addProperty("payload_hex", HexFormat.of().formatHex(bytes));
                    packets.add(entry);
                } finally { buffer.release(); }
            }
        }
        result.add("packets", packets);
        DynamicOps ops = (DynamicOps) Class.forName("up").getField("a").get(null);
        Object[] tags = {ops.createInt(256), ops.createShort((short)-1),
            ops.createDouble(Double.NaN), ops.createFloat(1.9f), ops.createString("true")};
        JsonArray locks = new JsonArray();
        for (Object tag : tags) locks.add(new Dynamic(ops, tag).asBoolean(false));
        result.add("numeric_lock_results", locks);
        System.out.println(new Gson().toJson(result));
    }
}
