// Executed Minecraft argument-type oracle for WP-E3 stages 6-7 (docs/reference/commands-brigadier.md).
//
// Runs the UNCHANGED argument classes of the pinned 1.21 server (net.minecraft.commands.arguments.*)
// on seeded inputs: the value parsed from a StringReader, the reader cursor afterwards, and for
// coordinates the position / rotation / block position they resolve to for given command sources
// (stage 6), and the values of the simple argument types of stage 7 (time, uuid, entity anchor, angle,
// swizzle).
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
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.EnumSet;
import java.util.UUID;
import net.minecraft.SharedConstants;
import net.minecraft.commands.BrigadierExceptions;
import net.minecraft.commands.CommandSource;
import net.minecraft.commands.CommandSourceStack;
import net.minecraft.commands.arguments.AngleArgument;
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
            result.add("ok", describeValue(value, c.getAsJsonArray("sources")));
        } catch (CommandSyntaxException e) {
            result.add("err", describe(e));
        }
        result.addProperty("cursor", reader.getCursor());
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

    // Stage 7 values; coordinates keep their stage 6 description.
    private static JsonObject describeValue(Object value, JsonArray sources) {
        if (value instanceof Coordinates) {
            return describe((Coordinates) value, sources);
        }
        JsonObject o = new JsonObject();
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
