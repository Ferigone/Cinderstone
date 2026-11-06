// SPDX-License-Identifier: GPL-2.0-only
// Executes original DifficultyCommand registration, Brigadier parse/completion and tree codec.
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
import net.minecraft.server.commands.DifficultyCommand;
import net.minecraft.world.phys.*;

public class difficulty_command_oracle {
    @SuppressWarnings({"unchecked", "rawtypes"})
    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion(); Bootstrap.bootStrap();
        com.mojang.brigadier.exceptions.CommandSyntaxException.BUILT_IN_EXCEPTIONS = new BrigadierExceptions();
        var dispatcher = new CommandDispatcher<CommandSourceStack>();
        DifficultyCommand.register(dispatcher);
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
            for (String input : new String[]{"", "d", "difficulty", "difficulty ", "difficulty h",
                "difficulty peaceful", "difficulty easy", "difficulty normal", "difficulty hard",
                "difficulty 0", "difficulty HARD", "difficulty easy trailing", "difficulty  easy"}) {
                if (permission != 2 && !(permission == 0 && input.equals("difficulty hard"))) continue;
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
        for (int current = 0; current < 4; current++) {
            for (String input : new String[]{"difficulty", "difficulty peaceful", "difficulty easy", "difficulty normal", "difficulty hard", "difficulty easy trailing", "difficulty 0"}) {
                executions.add(execute(dispatcher, unsafe, current, false, input, 2));
            }
        }
        executions.add(execute(dispatcher, unsafe, 3, true, "difficulty peaceful", 2));
        executions.add(execute(dispatcher, unsafe, 3, true, "difficulty hard", 2));
        executions.add(execute(dispatcher, unsafe, 1, false, "difficulty hard", 0));
        result.add("execution", executions);
        Files.writeString(Path.of(args[0]), new GsonBuilder().setPrettyPrinting().create().toJson(result)+"\n");
    }
    static void field(Object object, Class<?> owner, String name, Object value) throws Exception {
        Field field = owner.getDeclaredField(name); field.setAccessible(true); field.set(object, value);
    }
    static String bytes(Component component) {
        var buffer = Unpooled.buffer();
        try {
            net.minecraft.network.chat.ComponentSerialization.TRUSTED_CONTEXT_FREE_STREAM_CODEC.encode(buffer, component);
            byte[] bytes = new byte[buffer.readableBytes()]; buffer.readBytes(bytes);
            return HexFormat.of().formatHex(bytes);
        } finally { buffer.release(); }
    }
    static JsonObject execute(CommandDispatcher<CommandSourceStack> dispatcher, sun.misc.Unsafe unsafe,
                              int current, boolean hardcore, String input, int permission) throws Exception {
        var level = (net.minecraft.server.level.ServerLevel)unsafe.allocateInstance(net.minecraft.server.level.ServerLevel.class);
        Object[] state = new Object[]{net.minecraft.world.Difficulty.byId(current)};
        var dataType = net.minecraft.world.level.storage.ServerLevelData.class;
        Object data = Proxy.newProxyInstance(dataType.getClassLoader(),
            new Class<?>[]{dataType,net.minecraft.world.level.storage.WorldData.class}, (proxy, method, values) -> {
            switch (method.getName()) {
                case "getDifficulty": return state[0];
                case "setDifficulty": state[0] = values[0]; return null;
                case "isDifficultyLocked": return true;
                case "isHardcore": return hardcore;
            }
            throw new UnsupportedOperationException(method.getName());
        });
        field(level, net.minecraft.world.level.Level.class, "levelData", data);
        var server = (net.minecraft.server.MinecraftServer)unsafe.allocateInstance(net.minecraft.server.dedicated.DedicatedServer.class);
        field(server, net.minecraft.server.MinecraftServer.class, "worldData", data);
        field(server, net.minecraft.server.MinecraftServer.class, "levels", Map.of());
        var list = unsafe.allocateInstance(net.minecraft.server.dedicated.DedicatedPlayerList.class);
        field(list,net.minecraft.server.players.PlayerList.class,"players",List.of());
        field(server,net.minecraft.server.MinecraftServer.class,"playerList",list);
        List<Component> messages = new ArrayList<>();
        var sink = new CommandSource() {
            public void sendSystemMessage(Component message) {messages.add(message);}
            public boolean acceptsSuccess() {return true;}
            public boolean acceptsFailure() {return true;}
            public boolean shouldInformAdmins() {return false;}
        };
        var source = new CommandSourceStack(sink, Vec3.ZERO, Vec2.ZERO, level, permission,
            "Difficulty", Component.literal("Difficulty"), server, null);
        JsonObject out = new JsonObject(); out.addProperty("input", input); out.addProperty("permission", permission);
        out.addProperty("current",current); out.addProperty("hardcore",hardcore);
        try {
            out.addProperty("result",dispatcher.execute(input,source)); out.addProperty("ok",true);
            out.addProperty("feedback_hex",bytes(messages.get(0)));
        } catch (com.mojang.brigadier.exceptions.CommandSyntaxException error) {
            out.addProperty("ok",false);
            if (error.getRawMessage() instanceof Component component && component.getContents() instanceof net.minecraft.network.chat.contents.TranslatableContents content) {
                out.addProperty("error_key",content.getKey());
                if (content.getArgs().length>0) out.addProperty("error_argument",String.valueOf(content.getArgs()[0]));
            }
        }
        out.addProperty("effective",((net.minecraft.world.Difficulty)state[0]).getId());
        return out;
    }
}
