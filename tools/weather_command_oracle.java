// SPDX-License-Identifier: GPL-2.0-only
// Executes original WeatherCommand registration, Brigadier parse/completion and tree codec.
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
import net.minecraft.server.commands.WeatherCommand;
import net.minecraft.world.phys.*;

public class weather_command_oracle {
    @SuppressWarnings({"unchecked", "rawtypes"})
    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion(); Bootstrap.bootStrap();
        com.mojang.brigadier.exceptions.CommandSyntaxException.BUILT_IN_EXCEPTIONS = new BrigadierExceptions();
        var dispatcher = new CommandDispatcher<CommandSourceStack>();
        WeatherCommand.register(dispatcher);
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
            var buffer = new FriendlyByteBuf(Unpooled.buffer());
            try {
                ((StreamCodec)ClientboundCommandsPacket.STREAM_CODEC).encode(buffer, new ClientboundCommandsPacket((RootCommandNode)root));
                byte[] bytes = new byte[buffer.readableBytes()]; buffer.readBytes(bytes);
                level.addProperty("packet_hex", HexFormat.of().formatHex(bytes));
            } finally { buffer.release(); }
            JsonArray cases = new JsonArray();
            for (String input : new String[]{"", "w", "weather", "weather ", "weather r", "weather rain",
                "weather rain ", "weather rain 2d", "weather clear 1.5s", "weather thunder 0.5t",
                "weather clear", "weather thunder", "weather rain 0", "weather clear -1", "weather rain 1q",
                "weather rain 0.49t", "weather rain 2147483647t", "weather rain 20 trailing"}) {
                if (permission != 2 && !(permission == 0 && input.equals("weather rain"))) continue;
                var parsed = dispatcher.parse(input, source);
                JsonObject row = new JsonObject(); row.addProperty("input", input);
                row.addProperty("cursor", parsed.getReader().getCursor());
                row.addProperty("remaining", parsed.getReader().canRead());
                row.addProperty("executable", parsed.getContext().getCommand() != null);
                var argument = parsed.getContext().getArguments().get("duration");
                if (argument != null) row.addProperty("ticks", (Integer)argument.getResult());
                var suggestions = visible.getCompletionSuggestions(visible.parse(input, source)).join();
                JsonObject completion = new JsonObject();
                completion.addProperty("start", suggestions.getRange().getStart());
                completion.addProperty("end", suggestions.getRange().getEnd());
                JsonArray texts = new JsonArray(); suggestions.getList().forEach(s -> texts.add(s.getText()));
                completion.add("texts", texts); row.add("suggestions", completion); cases.add(row);
            }
            level.add("cases", cases); output.add(level);
        }
        JsonObject result = new JsonObject(); result.add("grammar", output);
        JsonArray executions = new JsonArray();
        for (long seed : new long[]{0, 1, -1, 123456789}) {
            for (String input : new String[]{"weather clear", "weather rain", "weather thunder",
                "weather clear 2d", "weather rain 1.5s", "weather thunder 0.5t", "weather rain 0",
                "weather clear -1", "weather rain 20 trailing", "weather rain 2147483647t"}) {
                if (seed != 0 && input.split(" ").length != 2) continue;
                executions.add(execute(dispatcher, unsafe, seed, input, 2));
            }
        }
        executions.add(execute(dispatcher, unsafe, 123, "weather thunder 20", 0));
        result.add("execution", executions);
        Files.writeString(Path.of(args[0]), new GsonBuilder().setPrettyPrinting().create().toJson(result)+"\n");
    }
    static void field(Object object, Class<?> owner, String name, Object value) throws Exception {
        Field field = owner.getDeclaredField(name); field.setAccessible(true); field.set(object, value);
    }
    static JsonObject execute(CommandDispatcher<CommandSourceStack> dispatcher, sun.misc.Unsafe unsafe,
                              long seed, String input, int permission) throws Exception {
        var level = (net.minecraft.server.level.ServerLevel)unsafe.allocateInstance(net.minecraft.server.level.ServerLevel.class);
        var random = new net.minecraft.world.level.levelgen.LegacyRandomSource(seed);
        field(level, net.minecraft.world.level.Level.class, "random", random);
        Object[] state = new Object[]{0, 0, 0, false, false};
        var dataType = net.minecraft.world.level.storage.ServerLevelData.class;
        Object data = Proxy.newProxyInstance(dataType.getClassLoader(), new Class<?>[]{dataType}, (proxy, method, values) -> {
            switch (method.getName()) {
                case "setClearWeatherTime": state[0] = values[0]; return null;
                case "setRainTime": state[1] = values[0]; return null;
                case "setThunderTime": state[2] = values[0]; return null;
                case "setRaining": state[3] = values[0]; return null;
                case "setThundering": state[4] = values[0]; return null;
            }
            throw new UnsupportedOperationException(method.getName());
        });
        field(level, net.minecraft.server.level.ServerLevel.class, "serverLevelData", data);
        var server = (net.minecraft.server.MinecraftServer)unsafe.allocateInstance(net.minecraft.server.dedicated.DedicatedServer.class);
        field(server, net.minecraft.server.MinecraftServer.class, "levels", Map.of(net.minecraft.world.level.Level.OVERWORLD, level));
        var sink = new CommandSource() {
            public void sendSystemMessage(Component message) {}
            public boolean acceptsSuccess() {return true;}
            public boolean acceptsFailure() {return true;}
            public boolean shouldInformAdmins() {return false;}
        };
        var source = new CommandSourceStack(sink, Vec3.ZERO, Vec2.ZERO, level, permission,
            "Weather", Component.literal("Weather"), server, null);
        JsonObject out = new JsonObject(); out.addProperty("input", input); out.addProperty("permission", permission); out.addProperty("seed", seed);
        try { out.addProperty("result", dispatcher.execute(input, source)); out.addProperty("ok", true); }
        catch (com.mojang.brigadier.exceptions.CommandSyntaxException error) { out.addProperty("ok", false); }
        out.add("state", new Gson().toJsonTree(state)); out.addProperty("next_int", random.nextInt());
        return out;
    }

}
