// SPDX-License-Identifier: GPL-2.0-only
// Executes original SeedCommand, source metadata, results, rich feedback and permission trees.
package cinderstone.oracle;
import com.google.gson.*;
import com.mojang.brigadier.*;
import com.mojang.brigadier.tree.*;
import io.netty.buffer.Unpooled;
import java.lang.reflect.*;
import java.nio.file.*;
import java.util.*;
import net.minecraft.SharedConstants;
import net.minecraft.commands.*;
import net.minecraft.network.FriendlyByteBuf;
import net.minecraft.network.chat.Component;
import net.minecraft.network.protocol.game.ClientboundCommandsPacket;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.commands.SeedCommand;
import net.minecraft.world.phys.*;

public class seed_command_oracle {
    @SuppressWarnings({"unchecked", "rawtypes"})
    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion(); Bootstrap.bootStrap();
        com.mojang.brigadier.exceptions.CommandSyntaxException.BUILT_IN_EXCEPTIONS = new BrigadierExceptions();
        var dispatcher = new CommandDispatcher<CommandSourceStack>();
        SeedCommand.register(dispatcher, true);
        Field f = sun.misc.Unsafe.class.getDeclaredField("theUnsafe"); f.setAccessible(true);
        var unsafe = (sun.misc.Unsafe)f.get(null);
        Commands commands = (Commands)unsafe.allocateInstance(Commands.class);
        Method fill = Commands.class.getDeclaredMethod("fillUsableCommands", CommandNode.class,
            CommandNode.class, CommandSourceStack.class, Map.class); fill.setAccessible(true);
        JsonArray output = new JsonArray();
        for (int permission = 0; permission <= 4; permission++) {
            var source = new CommandSourceStack(CommandSource.NULL, Vec3.ZERO, Vec2.ZERO, null,
                permission, "n", Component.literal("n"), null, null);
            var root = new RootCommandNode<CommandSourceStack>();
            Map map = new HashMap(); map.put(dispatcher.getRoot(), root);
            fill.invoke(commands, dispatcher.getRoot(), root, source, map);
            var visible = new CommandDispatcher<CommandSourceStack>(root);
            JsonObject level = new JsonObject(); level.addProperty("permission", permission);
            var buffer = new net.minecraft.network.RegistryFriendlyByteBuf(Unpooled.buffer(), net.minecraft.core.RegistryAccess.EMPTY);
            try {
                ((StreamCodec)ClientboundCommandsPacket.STREAM_CODEC).encode(buffer, new ClientboundCommandsPacket((RootCommandNode)root));
                byte[] bytes = new byte[buffer.readableBytes()]; buffer.readBytes(bytes);
                level.addProperty("packet_hex", HexFormat.of().formatHex(bytes));
            } finally { buffer.release(); }
            output.add(level);
        }
        JsonObject result = new JsonObject(); result.add("trees", output);
        JsonArray executions = new JsonArray();
        for (long seed : new long[]{0, 1, -1, Long.MIN_VALUE, Long.MAX_VALUE, -1234567890123456789L}) {
            executions.add(execute(dispatcher, unsafe, seed, "seed", 2));
        }
        for (String input : new String[]{"seed extra", "seed ", ""}) executions.add(execute(dispatcher, unsafe, 123, input, 2));
        executions.add(execute(dispatcher, unsafe, 123, "seed", 0));
        executions.add(execute(dispatcher, unsafe, 123, "seed", 1));
        result.add("execution", executions);
        Files.writeString(Path.of(args[0]), new GsonBuilder().setPrettyPrinting().create().toJson(result)+"\n");
    }
    static void field(Object object, Class<?> owner, String name, Object value) throws Exception {
        Field field = owner.getDeclaredField(name); field.setAccessible(true); field.set(object, value);
    }
    static JsonObject execute(CommandDispatcher<CommandSourceStack> dispatcher, sun.misc.Unsafe unsafe,
                              long seed, String input, int permission) throws Exception {
        var level = (net.minecraft.server.level.ServerLevel)unsafe.allocateInstance(net.minecraft.server.level.ServerLevel.class);
        var server = (net.minecraft.server.MinecraftServer)unsafe.allocateInstance(net.minecraft.server.dedicated.DedicatedServer.class);
        var dataType = net.minecraft.world.level.storage.WorldData.class;
        Object data = Proxy.newProxyInstance(dataType.getClassLoader(), new Class<?>[]{dataType}, (proxy, method, values) -> {
            if (method.getName().equals("worldGenOptions")) return new net.minecraft.world.level.levelgen.WorldOptions(seed, true, false);
            throw new UnsupportedOperationException(method.getName());
        });
        field(server, net.minecraft.server.MinecraftServer.class, "worldData", data);
        field(level, net.minecraft.server.level.ServerLevel.class, "server", server);
        Component[] feedback = new Component[1];
        var sink = new CommandSource() {
            public void sendSystemMessage(Component message) {feedback[0] = message;}
            public boolean acceptsSuccess() {return true;}
            public boolean acceptsFailure() {return true;}
            public boolean shouldInformAdmins() {return false;}
        };
        var source = new CommandSourceStack(sink, Vec3.ZERO, Vec2.ZERO, level, permission,
            "Weather", Component.literal("Weather"), server, null);
        JsonObject out = new JsonObject(); out.addProperty("input", input); out.addProperty("permission", permission); out.addProperty("seed", seed);
        try { out.addProperty("result", dispatcher.execute(input, source)); out.addProperty("ok", true); }
        catch (com.mojang.brigadier.exceptions.CommandSyntaxException error) { out.addProperty("ok", false); }
        if (feedback[0] != null) {
            var buffer = new net.minecraft.network.RegistryFriendlyByteBuf(Unpooled.buffer(), net.minecraft.core.RegistryAccess.EMPTY);
            try {
                ((StreamCodec)net.minecraft.network.protocol.game.ClientboundSystemChatPacket.STREAM_CODEC).encode(buffer,
                    new net.minecraft.network.protocol.game.ClientboundSystemChatPacket(feedback[0], false));
                byte[] bytes = new byte[buffer.readableBytes()]; buffer.readBytes(bytes);
                out.addProperty("feedback_hex", HexFormat.of().formatHex(bytes));
            } finally {buffer.release();}
        }
        return out;
    }

}
