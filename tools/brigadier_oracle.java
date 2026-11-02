// Executed Brigadier oracle for WP-E3 stage 1 (docs/reference/commands-brigadier.md).
//
// Runs the UNCHANGED com.mojang.brigadier 1.2.9 classes bundled with the pinned Java 1.21
// server (fixtures/source-lock.json), with the server's own BuiltInExceptionProvider
// (net.minecraft.commands.BrigadierExceptions, obfuscated class `en`) installed exactly as
// the vanilla bootstrap does. Build and run it with tools/brigadier_generate.py; it needs
// a JDK 21 and the oracle directory from tools/bootstrap_oracle.py, which must stay OUTSIDE git.
//
// Usage: java -cp <server-inner-1.21.jar:libraries/**/*.jar:classes> cinderstone.oracle.brigadier_oracle cases.json out.json
// The harness lives in its own package because the server jar is signed and occupies the default package.
//
// The obfuscated member names below come from the official mappings pinned in
// fixtures/source-lock.json: Component.getContents()=b, TranslatableContents.getKey()=b,
// TranslatableContents.getArgs()=d.
package cinderstone.oracle;

import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.mojang.brigadier.ImmutableStringReader;
import com.mojang.brigadier.Message;
import com.mojang.brigadier.StringReader;
import com.mojang.brigadier.arguments.ArgumentType;
import com.mojang.brigadier.arguments.BoolArgumentType;
import com.mojang.brigadier.arguments.DoubleArgumentType;
import com.mojang.brigadier.arguments.FloatArgumentType;
import com.mojang.brigadier.arguments.IntegerArgumentType;
import com.mojang.brigadier.arguments.LongArgumentType;
import com.mojang.brigadier.arguments.StringArgumentType;
import com.mojang.brigadier.exceptions.BuiltInExceptionProvider;
import com.mojang.brigadier.exceptions.CommandSyntaxException;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

public final class brigadier_oracle {
    private static final Gson GSON = new Gson();

    public static void main(String[] args) throws Exception {
        BuiltInExceptionProvider provider =
            (BuiltInExceptionProvider) Class.forName("en").getDeclaredConstructor().newInstance();
        CommandSyntaxException.BUILT_IN_EXCEPTIONS = provider;
        JsonArray cases = JsonParser.parseString(Files.readString(Path.of(args[0]), StandardCharsets.UTF_8)).getAsJsonArray();
        JsonArray out = new JsonArray();
        for (JsonElement element : cases) {
            out.add(run(element.getAsJsonObject()));
        }
        Files.writeString(Path.of(args[1]), asciiOnly(GSON.toJson(out)), StandardCharsets.UTF_8);
    }

    // CommandSyntaxException.getContext() can cut a surrogate pair in half; such a lone surrogate
    // cannot be written as UTF-8, so every non-ASCII UTF-16 unit is emitted as a JSON escape.
    private static String asciiOnly(String json) {
        StringBuilder builder = new StringBuilder(json.length());
        for (int i = 0; i < json.length(); i++) {
            char ch = json.charAt(i);
            if (ch > 126) {
                builder.append(String.format("\\u%04x", (int) ch));
            } else {
                builder.append(ch);
            }
        }
        return builder.toString();
    }

    private static JsonObject run(JsonObject c) {
        String op = c.get("op").getAsString();
        String input = c.get("input").getAsString();
        StringReader reader = new StringReader(input);
        if (c.has("cursor")) {
            reader.setCursor(c.get("cursor").getAsInt());
        }
        JsonObject result = new JsonObject();
        try {
            result.add("ok", apply(op, c, reader));
        } catch (CommandSyntaxException e) {
            result.add("err", describe(e));
        } catch (IndexOutOfBoundsException e) {
            // StringReader.read()/peek() past the end throw an unchecked exception in Brigadier.
            result.addProperty("throws", e.getClass().getSimpleName());
        }
        result.addProperty("cursor", reader.getCursor());
        return result;
    }

    private static JsonElement apply(String op, JsonObject c, StringReader r) throws CommandSyntaxException {
        switch (op) {
            case "readInt": return GSON.toJsonTree(r.readInt());
            case "readLong": return GSON.toJsonTree(Long.toString(r.readLong()));
            case "readFloat": return GSON.toJsonTree(Integer.toHexString(Float.floatToRawIntBits(r.readFloat())));
            case "readDouble": return GSON.toJsonTree(Long.toHexString(Double.doubleToRawLongBits(r.readDouble())));
            case "readBoolean": return GSON.toJsonTree(r.readBoolean());
            case "readUnquotedString": return GSON.toJsonTree(r.readUnquotedString());
            case "readQuotedString": return GSON.toJsonTree(r.readQuotedString());
            case "readString": return GSON.toJsonTree(r.readString());
            case "readStringUntil": return GSON.toJsonTree(r.readStringUntil(c.get("a").getAsString().charAt(0)));
            case "skipWhitespace": r.skipWhitespace(); return GSON.toJsonTree(r.getRemaining());
            case "expect": r.expect(c.get("a").getAsString().charAt(0)); return GSON.toJsonTree(true);
            case "read": return GSON.toJsonTree((int) r.read());
            case "views": {
                JsonObject o = new JsonObject();
                o.addProperty("read", r.getRead());
                o.addProperty("remaining", r.getRemaining());
                o.addProperty("remainingLength", r.getRemainingLength());
                o.addProperty("totalLength", r.getTotalLength());
                o.addProperty("canRead", r.canRead());
                o.addProperty("canRead3", r.canRead(3));
                return o;
            }
            case "arg.bool": return GSON.toJsonTree(BoolArgumentType.bool().parse(r));
            case "arg.int": return GSON.toJsonTree(IntegerArgumentType.integer(c.get("min").getAsInt(), c.get("max").getAsInt()).parse(r));
            case "arg.long": return GSON.toJsonTree(Long.toString(LongArgumentType.longArg(Long.parseLong(c.get("min").getAsString()), Long.parseLong(c.get("max").getAsString())).parse(r)));
            case "arg.float": return GSON.toJsonTree(Integer.toHexString(Float.floatToRawIntBits(FloatArgumentType.floatArg(Float.intBitsToFloat((int) Long.parseLong(c.get("min").getAsString(), 16)), Float.intBitsToFloat((int) Long.parseLong(c.get("max").getAsString(), 16))).parse(r))));
            case "arg.double": return GSON.toJsonTree(Long.toHexString(Double.doubleToRawLongBits(DoubleArgumentType.doubleArg(Double.longBitsToDouble(Long.parseUnsignedLong(c.get("min").getAsString(), 16)), Double.longBitsToDouble(Long.parseUnsignedLong(c.get("max").getAsString(), 16))).parse(r))));
            case "arg.string": return GSON.toJsonTree(stringType(c.get("type").getAsString()).parse(r));
            case "escapeIfRequired": return GSON.toJsonTree(StringArgumentType.escapeIfRequired(c.get("input").getAsString()));
            case "d2s": return GSON.toJsonTree(Double.toString(Double.longBitsToDouble(Long.parseUnsignedLong(c.get("input").getAsString(), 16))));
            case "f2s": return GSON.toJsonTree(Float.toString(Float.intBitsToFloat((int) Long.parseLong(c.get("input").getAsString(), 16))));
            case "isAllowedNumber": return GSON.toJsonTree(StringReader.isAllowedNumber(c.get("input").getAsString().charAt(0)));
            case "isQuotedStringStart": return GSON.toJsonTree(StringReader.isQuotedStringStart(c.get("input").getAsString().charAt(0)));
            case "isAllowedInUnquotedString": return GSON.toJsonTree(StringReader.isAllowedInUnquotedString(c.get("input").getAsString().charAt(0)));
            case "isWhitespace": return GSON.toJsonTree(Character.isWhitespace(c.get("input").getAsString().charAt(0)));
            default: throw new IllegalArgumentException(op);
        }
    }

    private static ArgumentType<String> stringType(String type) {
        switch (type) {
            case "word": return StringArgumentType.word();
            case "string": return StringArgumentType.string();
            case "greedy": return StringArgumentType.greedyString();
            default: throw new IllegalArgumentException(type);
        }
    }

    private static JsonObject describe(CommandSyntaxException e) {
        JsonObject o = new JsonObject();
        Message message = e.getRawMessage();
        o.addProperty("english", message.getString());
        o.addProperty("message", e.getMessage());
        String context = e.getContext();
        o.addProperty("context", context);
        o.addProperty("input", e.getInput());
        o.addProperty("errCursor", e.getCursor());
        try {
            // Resolve through the public interfaces: the runtime classes are not public.
            Object contents = Class.forName("wz").getMethod("b").invoke(message);
            Method getKey = Class.forName("yk").getMethod("b");
            Method getArgs = Class.forName("yk").getMethod("d");
            o.addProperty("key", (String) getKey.invoke(contents));
            JsonArray args = new JsonArray();
            for (Object value : (Object[]) getArgs.invoke(contents)) {
                JsonObject a = new JsonObject();
                a.addProperty("t", value.getClass().getSimpleName());
                a.addProperty("v", String.valueOf(value));
                args.add(a);
            }
            o.add("args", args);
        } catch (ReflectiveOperationException ex) {
            throw new IllegalStateException("message is not a translatable component", ex);
        }
        return o;
    }
}
