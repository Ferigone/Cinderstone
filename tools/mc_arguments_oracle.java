// Executed Minecraft argument-type oracle for WP-E3 stages 6-10 (docs/reference/commands-brigadier.md).
//
// Runs the UNCHANGED argument classes of the pinned 1.21 server (net.minecraft.commands.arguments.*)
// on seeded inputs: the value parsed from a StringReader, the reader cursor afterwards, and for
// coordinates the position / rotation / block position they resolve to for given command sources
// (stage 6), the values of the simple argument types of stage 7 (time, uuid, entity anchor, angle,
// swizzle), the int / float range arguments of stage 9 (MinMaxBounds with their squares) and the
// scoreboard OperationArgument of stage 10 (applied to given score pairs, plus its suggestions).
// The server classes come from the pinned jar remapped with the official mappings
// (tools/loot_oracle_setup.py builds <oracle-dir>/server-named.jar and cp.txt); everything stays
// OUTSIDE git. The exception provider installed is the server's own BrigadierExceptions.
//
// Usage: java -cp <server-named.jar:libraries/**/*.jar:classes>
//        cinderstone.oracle.mc_arguments_oracle cases.json out.json
package cinderstone.oracle;

import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.mojang.brigadier.StringReader;
import com.mojang.brigadier.arguments.ArgumentType;
import com.mojang.brigadier.exceptions.CommandSyntaxException;
import com.mojang.brigadier.suggestion.Suggestion;
import com.mojang.brigadier.suggestion.Suggestions;
import com.mojang.brigadier.suggestion.SuggestionsBuilder;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.lang.reflect.Proxy;
import java.nio.file.Path;
import java.util.EnumSet;
import java.util.Optional;
import java.util.UUID;
import net.minecraft.SharedConstants;
import net.minecraft.commands.BrigadierExceptions;
import net.minecraft.commands.CommandSource;
import net.minecraft.commands.CommandSourceStack;
import net.minecraft.advancements.critereon.MinMaxBounds;
import net.minecraft.commands.arguments.AngleArgument;
import net.minecraft.commands.arguments.OperationArgument;
import net.minecraft.commands.arguments.RangeArgument;
import net.minecraft.commands.arguments.EntityAnchorArgument;
import net.minecraft.commands.arguments.TimeArgument;
import net.minecraft.commands.arguments.UuidArgument;
import net.minecraft.commands.arguments.coordinates.BlockPosArgument;
import net.minecraft.commands.arguments.coordinates.ColumnPosArgument;
import net.minecraft.commands.arguments.coordinates.Coordinates;
import net.minecraft.commands.arguments.coordinates.SwizzleArgument;
import net.minecraft.commands.arguments.coordinates.Vec2Argument;
import net.minecraft.commands.arguments.coordinates.Vec3Argument;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.contents.TranslatableContents;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.scores.ScoreAccess;
import net.minecraft.world.phys.Vec2;
import net.minecraft.world.phys.Vec3;

@SuppressWarnings({"unchecked", "rawtypes"})
public final class mc_arguments_oracle {
    private static final Gson GSON = new Gson();

    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        CommandSyntaxException.BUILT_IN_EXCEPTIONS = new BrigadierExceptions();
        JsonArray cases = JsonParser.parseString(Files.readString(Path.of(args[0]), StandardCharsets.UTF_8)).getAsJsonArray();
        JsonArray out = new JsonArray();
        for (JsonElement element : cases) {
            out.add(run(element.getAsJsonObject()));
        }
        Files.writeString(Path.of(args[1]), asciiOnly(GSON.toJson(out)), StandardCharsets.UTF_8);
    }

    private static String asciiOnly(String json) {
        StringBuilder builder = new StringBuilder(json.length());
        for (int i = 0; i < json.length(); i++) {
            char ch = json.charAt(i);
            builder.append(ch > 126 ? String.format("\\u%04x", (int) ch) : String.valueOf(ch));
        }
        return builder.toString();
    }

    private static ArgumentType type(JsonObject c) {
        String name = c.get("type").getAsString();
        switch (name) {
            case "time": return c.has("min") ? TimeArgument.time(c.get("min").getAsInt()) : TimeArgument.time();
            case "uuid": return UuidArgument.uuid();
            case "anchor": return EntityAnchorArgument.anchor();
            case "angle": return AngleArgument.angle();
            case "swizzle": return SwizzleArgument.swizzle();
            case "int_range": return RangeArgument.intRange();
            case "float_range": return RangeArgument.floatRange();
            case "operation": return OperationArgument.operation();
            case "vec3": return Vec3Argument.vec3();
            case "vec3_raw": return Vec3Argument.vec3(false);
            case "blockpos": return BlockPosArgument.blockPos();
            case "vec2": return Vec2Argument.vec2();
            case "vec2_raw": return Vec2Argument.vec2(false);
            case "columnpos": return ColumnPosArgument.columnPos();
            default: throw new IllegalArgumentException(name);
        }
    }

    private static String bits(double value) {
        return Long.toHexString(Double.doubleToRawLongBits(value));
    }

    private static String bits(float value) {
        return Integer.toHexString(Float.floatToRawIntBits(value));
    }

    private static JsonObject run(JsonObject c) {
        StringReader reader = new StringReader(c.get("input").getAsString());
        if (c.has("cursor")) {
            reader.setCursor(c.get("cursor").getAsInt());
        }
        JsonObject result = new JsonObject();
        try {
            Object value = type(c).parse(reader);
            result.add("ok", describeValue(value, c.getAsJsonArray("sources"), c));
        } catch (CommandSyntaxException e) {
            result.add("err", describe(e));
        }
        result.addProperty("cursor", reader.getCursor());
        if (c.has("suggestAt")) {
            // The type's own listSuggestions (the operation argument does not look at the context).
            try {
                Suggestions suggestions = (Suggestions) ((java.util.concurrent.CompletableFuture) type(c).listSuggestions(null, new SuggestionsBuilder(c.get("input").getAsString(), c.get("suggestAt").getAsInt()))).join();
                JsonObject listed = new JsonObject();
                listed.addProperty("start", suggestions.getRange().getStart());
                listed.addProperty("end", suggestions.getRange().getEnd());
                JsonArray texts = new JsonArray();
                for (Suggestion suggestion : suggestions.getList()) {
                    JsonObject one = new JsonObject();
                    one.addProperty("start", suggestion.getRange().getStart());
                    one.addProperty("end", suggestion.getRange().getEnd());
                    one.addProperty("text", suggestion.getText());
                    texts.add(one);
                }
                listed.add("list", texts);
                result.add("suggestions", listed);
            } catch (Exception e) {
                throw new IllegalStateException(e);
            }
        }
        return result;
    }

    private static CommandSourceStack source(JsonObject spec) {
        JsonArray p = spec.getAsJsonArray("pos");
        JsonArray r = spec.getAsJsonArray("rot");
        Vec3 position = new Vec3(
            Double.longBitsToDouble(Long.parseUnsignedLong(p.get(0).getAsString(), 16)),
            Double.longBitsToDouble(Long.parseUnsignedLong(p.get(1).getAsString(), 16)),
            Double.longBitsToDouble(Long.parseUnsignedLong(p.get(2).getAsString(), 16)));
        Vec2 rotation = new Vec2(
            Float.intBitsToFloat((int) Long.parseLong(r.get(0).getAsString(), 16)),
            Float.intBitsToFloat((int) Long.parseLong(r.get(1).getAsString(), 16)));
        // No level, server or entity: positions only. The anchor then leaves the position unchanged.
        return new CommandSourceStack(CommandSource.NULL, position, rotation, null, 0, "n", Component.literal("n"), null, null);
    }

    private static JsonElement optional(Optional<? extends Number> value) {
        return value.isPresent() ? new com.google.gson.JsonPrimitive(value.get().toString()) : com.google.gson.JsonNull.INSTANCE;
    }

    private static JsonElement optionalBits(Optional<Double> value) {
        return value.isPresent() ? new com.google.gson.JsonPrimitive(bits(value.get().doubleValue())) : com.google.gson.JsonNull.INSTANCE;
    }

    // Stage 7 values; coordinates keep their stage 6 description.
    private static JsonObject describeValue(Object value, JsonArray sources, JsonObject c) {
        if (value instanceof Coordinates) {
            return describe((Coordinates) value, sources);
        }
        JsonObject o = new JsonObject();
        if (value instanceof OperationArgument.Operation) {
            return describeOperation((OperationArgument.Operation) value, c.has("pairs") ? c.getAsJsonArray("pairs") : new JsonArray());
        }
        if (value instanceof Integer) {
            o.addProperty("v", (Integer) value);
        } else if (value instanceof UUID) {
            o.addProperty("msb", Long.toString(((UUID) value).getMostSignificantBits()));
            o.addProperty("lsb", Long.toString(((UUID) value).getLeastSignificantBits()));
        } else if (value instanceof EntityAnchorArgument.Anchor) {
            o.addProperty("v", ((EntityAnchorArgument.Anchor) value).name());
        } else if (value instanceof AngleArgument.SingleAngle) {
            JsonArray at = new JsonArray();
            for (JsonElement element : sources) {
                at.add(bits(((AngleArgument.SingleAngle) value).getAngle(source(element.getAsJsonObject()))));
            }
            o.add("at", at);
        } else if (value instanceof MinMaxBounds.Ints) {
            MinMaxBounds.Ints bounds = (MinMaxBounds.Ints) value;
            o.add("min", optional(bounds.min()));
            o.add("max", optional(bounds.max()));
            o.add("minSq", optional(bounds.minSq()));
            o.add("maxSq", optional(bounds.maxSq()));
        } else if (value instanceof MinMaxBounds.Doubles) {
            MinMaxBounds.Doubles bounds = (MinMaxBounds.Doubles) value;
            o.add("min", optionalBits(bounds.min()));
            o.add("max", optionalBits(bounds.max()));
            o.add("minSq", optionalBits(bounds.minSq()));
            o.add("maxSq", optionalBits(bounds.maxSq()));
        } else if (value instanceof EnumSet) {
            StringBuilder axes = new StringBuilder();
            for (Object axis : (EnumSet) value) {
                axes.append(((Direction.Axis) axis).getName());
            }
            o.addProperty("v", axes.toString());
        } else {
            throw new IllegalArgumentException(value.getClass().getName());
        }
        return o;
    }

    // A ScoreAccess holding one int; the other abstract methods are not used by the operations.
    private static ScoreAccess score(int[] cell) {
        return (ScoreAccess) Proxy.newProxyInstance(ScoreAccess.class.getClassLoader(), new Class<?>[] {ScoreAccess.class}, (proxy, method, args) -> {
            switch (method.getName()) {
                case "get": return cell[0];
                case "set": cell[0] = (Integer) args[0]; return null;
                default: throw new UnsupportedOperationException(method.getName());
            }
        });
    }

    // Applies the parsed operation to each (target, source) pair: the new pair, or the exception.
    private static JsonObject describeOperation(OperationArgument.Operation operation, JsonArray pairs) {
        JsonObject o = new JsonObject();
        JsonArray results = new JsonArray();
        for (JsonElement element : pairs) {
            int[] target = {Integer.parseInt(element.getAsJsonArray().get(0).getAsString())};
            int[] source = {Integer.parseInt(element.getAsJsonArray().get(1).getAsString())};
            JsonObject entry = new JsonObject();
            try {
                operation.apply(score(target), score(source));
                JsonArray now = new JsonArray();
                now.add(Integer.toString(target[0]));
                now.add(Integer.toString(source[0]));
                entry.add("now", now);
            } catch (CommandSyntaxException e) {
                entry.add("err", describe(e));
            }
            results.add(entry);
        }
        o.add("applied", results);
        return o;
    }

    private static JsonObject describe(Coordinates coordinates, JsonArray sources) {
        JsonObject o = new JsonObject();
        JsonArray relative = new JsonArray();
        relative.add(coordinates.isXRelative());
        relative.add(coordinates.isYRelative());
        relative.add(coordinates.isZRelative());
        o.add("rel", relative);
        JsonArray at = new JsonArray();
        for (JsonElement element : sources) {
            CommandSourceStack source = source(element.getAsJsonObject());
            JsonObject entry = new JsonObject();
            Vec3 resolved = coordinates.getPosition(source);
            JsonArray pos = new JsonArray();
            pos.add(bits(resolved.x));
            pos.add(bits(resolved.y));
            pos.add(bits(resolved.z));
            entry.add("pos", pos);
            Vec2 turned = coordinates.getRotation(source);
            JsonArray rot = new JsonArray();
            rot.add(bits(turned.x));
            rot.add(bits(turned.y));
            entry.add("rot", rot);
            BlockPos block = coordinates.getBlockPos(source);
            JsonArray blockArray = new JsonArray();
            blockArray.add(block.getX());
            blockArray.add(block.getY());
            blockArray.add(block.getZ());
            entry.add("block", blockArray);
            at.add(entry);
        }
        o.add("at", at);
        return o;
    }

    private static JsonObject describe(CommandSyntaxException e) {
        JsonObject o = new JsonObject();
        o.addProperty("message", e.getMessage());
        o.addProperty("context", e.getContext());
        o.addProperty("errCursor", e.getCursor());
        Component component = (Component) e.getRawMessage();
        TranslatableContents contents = (TranslatableContents) component.getContents();
        o.addProperty("key", contents.getKey());
        JsonArray args = new JsonArray();
        for (Object value : contents.getArgs()) {
            JsonObject a = new JsonObject();
            a.addProperty("t", value.getClass().getSimpleName());
            a.addProperty("v", String.valueOf(value));
            args.add(a);
        }
        o.add("args", args);
        return o;
    }
}
