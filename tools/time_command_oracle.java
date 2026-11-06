// SPDX-License-Identifier: GPL-2.0-only
// Executes original TimeCommand registration, Brigadier parse/completion and tree codec.
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
import net.minecraft.server.commands.TimeCommand;
import net.minecraft.world.phys.*;

public class time_command_oracle {
    @SuppressWarnings({"unchecked", "rawtypes"})
    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion(); Bootstrap.bootStrap();
        com.mojang.brigadier.exceptions.CommandSyntaxException.BUILT_IN_EXCEPTIONS = new BrigadierExceptions();
        var dispatcher = new CommandDispatcher<CommandSourceStack>();
        TimeCommand.register(dispatcher);
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
            for (String input : new String[]{"", "t", "time", "time ", "time set", "time set ",
                "time set day", "time set 2d", "time set 1.5s", "time add 0.5s", "time query ",
                "time query day", "time query daytime", "time query gametime", "time set -1",
                "time set 1q", "time set x", "time query foo", "time set 1 ", "time add 12", "time add 12D"}) {
                if (permission != 2 && !(permission == 0 && input.equals("time set day"))) continue;
                var parsed = dispatcher.parse(input, source);
                JsonObject row = new JsonObject(); row.addProperty("input", input);
                row.addProperty("cursor", parsed.getReader().getCursor());
                row.addProperty("remaining", parsed.getReader().canRead());
                row.addProperty("executable", parsed.getContext().getCommand() != null);
                var argument = parsed.getContext().getArguments().get("time");
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
        Files.writeString(Path.of(args[0]), new GsonBuilder().setPrettyPrinting().create().toJson(output)+"\n");
    }
}
