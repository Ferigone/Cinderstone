// Executed command-tree packet oracle for WP-E3 stages 5-9 (docs/reference/commands-brigadier.md).
//
// Builds command trees from JSON descriptions with the UNCHANGED com.mojang.brigadier 1.2.9
// builders, wraps the root in the dedicated server's own ClientboundCommandsPacket and records
// the bytes its StreamCodec writes (packet body, no packet id). The server classes come from the
// pinned 1.21 server jar remapped with the official mappings (tools/loot_oracle_setup.py builds
// <oracle-dir>/server-named.jar and cp.txt); everything stays OUTSIDE git.
//
// Stage 8 also records, for permission levels 0-4, the packet of the tree Commands.fillUsableCommands
// builds for a source of that level ("usable"); a node's "req" is a permission-level requirement.
//
// Usage: java -cp <server-named.jar:libraries/**/*.jar:classes>
//        cinderstone.oracle.brigadier_tree_packet_oracle cases.json out.json
// Tree description: see tools/brigadier_dispatch_generate.py; extra fields used here are "sug"
// ({"name": "<registered provider>"} or {} for an unregistered one) and "cmd" (the same value
// means the very same Command instance, so structurally equal nodes can merge).
package cinderstone.oracle;

import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.mojang.brigadier.Command;
import com.mojang.brigadier.CommandDispatcher;
import com.mojang.brigadier.arguments.ArgumentType;
import com.mojang.brigadier.arguments.BoolArgumentType;
import com.mojang.brigadier.arguments.DoubleArgumentType;
import com.mojang.brigadier.arguments.FloatArgumentType;
import com.mojang.brigadier.arguments.IntegerArgumentType;
import com.mojang.brigadier.arguments.LongArgumentType;
import com.mojang.brigadier.arguments.StringArgumentType;
import com.mojang.brigadier.builder.ArgumentBuilder;
import com.mojang.brigadier.builder.LiteralArgumentBuilder;
import com.mojang.brigadier.builder.RequiredArgumentBuilder;
import com.mojang.brigadier.suggestion.SuggestionProvider;
import com.mojang.brigadier.tree.CommandNode;
import com.mojang.brigadier.tree.RootCommandNode;
import io.netty.buffer.ByteBuf;
import io.netty.buffer.Unpooled;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.HashMap;
import java.util.Map;
import net.minecraft.SharedConstants;
import net.minecraft.commands.CommandSource;
import net.minecraft.commands.CommandSourceStack;
import net.minecraft.commands.Commands;
import net.minecraft.network.chat.Component;
import net.minecraft.world.phys.Vec2;
import net.minecraft.world.phys.Vec3;
import net.minecraft.commands.SharedSuggestionProvider;
import net.minecraft.commands.arguments.AngleArgument;
import net.minecraft.commands.arguments.EntityAnchorArgument;
import net.minecraft.commands.arguments.RangeArgument;
import net.minecraft.commands.arguments.TimeArgument;
import net.minecraft.commands.arguments.UuidArgument;
import net.minecraft.commands.arguments.coordinates.BlockPosArgument;
import net.minecraft.commands.arguments.coordinates.ColumnPosArgument;
import net.minecraft.commands.arguments.coordinates.SwizzleArgument;
import net.minecraft.commands.arguments.coordinates.Vec2Argument;
import net.minecraft.commands.arguments.coordinates.Vec3Argument;
import net.minecraft.commands.synchronization.SuggestionProviders;
import net.minecraft.network.FriendlyByteBuf;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.protocol.game.ClientboundCommandsPacket;
import net.minecraft.server.Bootstrap;

@SuppressWarnings({"unchecked", "rawtypes"})
public final class brigadier_tree_packet_oracle {
    private static final Gson GSON = new Gson();

    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        JsonArray tests = JsonParser.parseString(Files.readString(Path.of(args[0]), StandardCharsets.UTF_8)).getAsJsonArray();
        JsonArray out = new JsonArray();
        for (JsonElement test : tests) {
            out.add(run(test.getAsJsonObject()));
        }
        Files.writeString(Path.of(args[1]), GSON.toJson(out), StandardCharsets.UTF_8);
    }

    private static JsonObject run(JsonObject test) {
        CommandDispatcher<SharedSuggestionProvider> dispatcher = new CommandDispatcher<>();
        Map<String, Command<SharedSuggestionProvider>> commands = new HashMap<>();
        for (JsonElement tree : test.getAsJsonArray("tree")) {
            dispatcher.register((LiteralArgumentBuilder<SharedSuggestionProvider>) build(tree.getAsJsonObject(), dispatcher, commands));
        }
        RootCommandNode<SharedSuggestionProvider> root = dispatcher.getRoot();
        FriendlyByteBuf buffer = new FriendlyByteBuf(Unpooled.buffer());
        ((StreamCodec) ClientboundCommandsPacket.STREAM_CODEC).encode(buffer, new ClientboundCommandsPacket(root));
        JsonObject result = new JsonObject();
        result.addProperty("packet", hex(buffer));
        JsonArray usable = new JsonArray();
        for (int level = 0; level <= 4; level++) {
            usable.add(usablePacket(dispatcher, level));
        }
        result.add("usable", usable);
        return result;
    }

    // The tree the server sends to a player of the given permission level: Commands.fillUsableCommands, a private
    // instance method that does not touch the instance, run on an uninitialised Commands object. The map is the
    // HashMap that sendCommands creates (keyed by CommandNode.equals), seeded with the root.
    private static String usablePacket(CommandDispatcher<SharedSuggestionProvider> dispatcher, int level) {
        try {
            Field unsafeField = sun.misc.Unsafe.class.getDeclaredField("theUnsafe");
            unsafeField.setAccessible(true);
            sun.misc.Unsafe unsafe = (sun.misc.Unsafe) unsafeField.get(null);
            Commands commands = (Commands) unsafe.allocateInstance(Commands.class);
            Method fill = Commands.class.getDeclaredMethod("fillUsableCommands", CommandNode.class, CommandNode.class, CommandSourceStack.class, Map.class);
            fill.setAccessible(true);
            Map map = com.google.common.collect.Maps.newHashMap();
            RootCommandNode<SharedSuggestionProvider> out = new RootCommandNode<>();
            CommandNode root = dispatcher.getRoot();
            map.put(root, out);
            CommandSourceStack source = new CommandSourceStack(CommandSource.NULL, Vec3.ZERO, Vec2.ZERO, null, level, "n", Component.literal("n"), null, null);
            fill.invoke(commands, root, out, source, map);
            FriendlyByteBuf buffer = new FriendlyByteBuf(Unpooled.buffer());
            ((StreamCodec) ClientboundCommandsPacket.STREAM_CODEC).encode(buffer, new ClientboundCommandsPacket(out));
            return hex(buffer);
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException(e);
        }
    }

    private static String hex(ByteBuf buffer) {
        StringBuilder builder = new StringBuilder();
        for (int i = buffer.readerIndex(); i < buffer.writerIndex(); i++) {
            builder.append(String.format("%02x", buffer.getByte(i) & 0xFF));
        }
        return builder.toString();
    }

    private static ArgumentBuilder build(JsonObject spec, CommandDispatcher<SharedSuggestionProvider> dispatcher, Map<String, Command<SharedSuggestionProvider>> commands) {
        String name = spec.get("name").getAsString();
        boolean literal = spec.get("t").getAsString().equals("lit");
        ArgumentBuilder builder = literal ? LiteralArgumentBuilder.literal(name) : RequiredArgumentBuilder.argument(name, type(spec.getAsJsonObject("type")));
        if (!literal && spec.has("sug")) {
            JsonObject sug = spec.getAsJsonObject("sug");
            SuggestionProvider provider = sug.has("name") ? registered(sug.get("name").getAsString()) : (context, suggestions) -> suggestions.buildFuture();
            ((RequiredArgumentBuilder) builder).suggests(provider);
        }
        if (spec.has("req")) {
            int required = spec.get("req").getAsInt();
            builder.requires(source -> ((CommandSourceStack) source).hasPermission(required));
        }
        if (spec.has("cmd")) {
            String key = spec.get("cmd").getAsString();
            builder.executes(commands.computeIfAbsent(key, k -> context -> k.length()));
        }
        if (spec.has("children")) {
            for (JsonElement child : spec.getAsJsonArray("children")) {
                builder.then(build(child.getAsJsonObject(), dispatcher, commands));
            }
        }
        if (spec.has("redirect")) {
            JsonObject redirect = spec.getAsJsonObject("redirect");
            CommandNode<SharedSuggestionProvider> target = redirect.get("to").getAsString().equals("root")
                ? dispatcher.getRoot()
                : dispatcher.getRoot().getChild(redirect.get("to").getAsString());
            builder.redirect(target);
        }
        return builder;
    }

    // The named wrappers vanilla builds its commands with (getProvider(name) returns the bare delegate,
    // which getName() reports as ask_server).
    private static SuggestionProvider registered(String name) {
        switch (name) {
            case "minecraft:ask_server": return SuggestionProviders.ASK_SERVER;
            case "minecraft:all_recipes": return SuggestionProviders.ALL_RECIPES;
            case "minecraft:available_sounds": return SuggestionProviders.AVAILABLE_SOUNDS;
            case "minecraft:summonable_entities": return SuggestionProviders.SUMMONABLE_ENTITIES;
            default: throw new IllegalArgumentException(name);
        }
    }

    private static ArgumentType type(JsonObject t) {
        switch (t.get("k").getAsString()) {
            case "bool": return BoolArgumentType.bool();
            case "int": return IntegerArgumentType.integer(t.get("min").getAsInt(), t.get("max").getAsInt());
            case "long": return LongArgumentType.longArg(Long.parseLong(t.get("min").getAsString()), Long.parseLong(t.get("max").getAsString()));
            case "float": return FloatArgumentType.floatArg(Float.intBitsToFloat((int) Long.parseLong(t.get("min").getAsString(), 16)), Float.intBitsToFloat((int) Long.parseLong(t.get("max").getAsString(), 16)));
            case "double": return DoubleArgumentType.doubleArg(Double.longBitsToDouble(Long.parseUnsignedLong(t.get("min").getAsString(), 16)), Double.longBitsToDouble(Long.parseUnsignedLong(t.get("max").getAsString(), 16)));
            case "word": return StringArgumentType.word();
            case "string": return StringArgumentType.string();
            case "greedy": return StringArgumentType.greedyString();
            // Minecraft position arguments (stage 6): registry ids 8-11, no template
            case "blockpos": return BlockPosArgument.blockPos();
            case "columnpos": return ColumnPosArgument.columnPos();
            case "vec3": return Vec3Argument.vec3();
            case "vec2": return Vec2Argument.vec2();
            // stage 7: simple value arguments
            case "time": return t.has("min") ? TimeArgument.time(t.get("min").getAsInt()) : TimeArgument.time();
            case "uuid": return UuidArgument.uuid();
            case "anchor": return EntityAnchorArgument.anchor();
            case "angle": return AngleArgument.angle();
            case "swizzle": return SwizzleArgument.swizzle();
            // stage 9: range arguments
            case "int_range": return RangeArgument.intRange();
            case "float_range": return RangeArgument.floatRange();
            default: throw new IllegalArgumentException(t.get("k").getAsString());
        }
    }
}
