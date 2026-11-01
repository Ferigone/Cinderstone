package oracle;

import com.google.gson.*;
import io.netty.buffer.*;
import java.io.*;
import java.lang.reflect.*;
import java.util.HexFormat;

/** Executes unmodified original NbtIo/FloatTag/DoubleTag against both real DataOutput implementations. */
public final class nbt_float_oracle {
    private static final Gson JSON = new Gson();
    private static final HexFormat HEX = HexFormat.of();
    private static final Class<?> IO, TAG, ACCOUNTER;
    private static final Method READ, WRITE_ANY, WRITE_NAMED, UNLIMITED;
    static {
        try {
            IO = Class.forName("uo"); TAG = Class.forName("uy"); ACCOUNTER = Class.forName("uk");
            READ = IO.getMethod("b", DataInput.class, ACCOUNTER);
            WRITE_ANY = IO.getMethod("a", TAG, DataOutput.class);
            WRITE_NAMED = IO.getMethod("b", TAG, DataOutput.class);
            UNLIMITED = ACCOUNTER.getMethod("a");
        } catch (ReflectiveOperationException e) { throw new ExceptionInInitializerError(e); }
    }

    private static Object read(byte[] raw) throws Exception {
        ByteBuf bytes = Unpooled.wrappedBuffer(raw);
        try {
            Object tag = READ.invoke(null, new ByteBufInputStream(bytes), UNLIMITED.invoke(null));
            if (bytes.readableBytes() != 0) throw new AssertionError("unconsumed input");
            return tag;
        } finally { bytes.release(); }
    }

    private static String disk(Object tag, boolean named) throws Exception {
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        try (DataOutputStream output = new DataOutputStream(bytes)) {
            (named ? WRITE_NAMED : WRITE_ANY).invoke(null, tag, output);
        }
        return HEX.formatHex(bytes.toByteArray());
    }

    private static String network(Object tag) throws Exception {
        ByteBuf bytes = Unpooled.buffer();
        try {
            WRITE_ANY.invoke(null, tag, new ByteBufOutputStream(bytes));
            byte[] raw = new byte[bytes.readableBytes()];
            bytes.getBytes(bytes.readerIndex(), raw);
            return HEX.formatHex(raw);
        } finally { bytes.release(); }
    }

    private static String bits(Object tag, int kind) throws Exception {
        Field data = tag.getClass().getDeclaredField("w"); data.setAccessible(true);
        return kind == 5
            ? String.format("%08x", Float.floatToRawIntBits(data.getFloat(tag)))
            : String.format("%016x", Double.doubleToRawLongBits(data.getDouble(tag)));
    }

    private static JsonObject run(JsonObject query) throws Exception {
        boolean numeric = query.has("tag_type");
        String raw = numeric ? String.format("%02x", query.get("tag_type").getAsInt()) + query.get("bits").getAsString()
                             : query.get("input_network").getAsString();
        Object tag = read(HEX.parseHex(raw));
        JsonObject result = query.deepCopy();
        result.addProperty("disk_anonymous", disk(tag, false));
        result.addProperty("network_anonymous", network(tag));
        result.addProperty("disk_named", disk(tag, true));
        if (numeric) {
            int kind = query.get("tag_type").getAsInt();
            String decoded = bits(tag, kind), input = query.get("bits").getAsString();
            Object factory;
            if (kind == 5) factory = Class.forName("ue").getMethod("a", float.class).invoke(null,
                Float.intBitsToFloat(Integer.parseUnsignedInt(input, 16)));
            else factory = Class.forName("uc").getMethod("a", double.class).invoke(null,
                Double.longBitsToDouble(Long.parseUnsignedLong(input, 16)));
            if (!decoded.equals(bits(factory, kind))) throw new AssertionError("factory/load disagreement");
            result.addProperty("decoded_bits", decoded);
        }
        return result;
    }

    public static void main(String[] args) throws Exception {
        System.out.println("SOURCE:" + IO.getProtectionDomain().getCodeSource().getLocation());
        BufferedReader input = new BufferedReader(new InputStreamReader(System.in));
        for (String line; (line = input.readLine()) != null;) {
            System.out.println("ORACLE:" + JSON.toJson(run(JsonParser.parseString(line).getAsJsonObject())));
        }
    }
}
