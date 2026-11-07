// SPDX-License-Identifier: GPL-2.0-only
// Executes original StopCommand registration, Brigadier parse/completion and tree codec.
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
import net.minecraft.server.commands.StopCommand;
import net.minecraft.world.phys.*;

public class stop_command_oracle {
    @SuppressWarnings({"unchecked", "rawtypes"})
    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion(); Bootstrap.bootStrap();
        com.mojang.brigadier.exceptions.CommandSyntaxException.BUILT_IN_EXCEPTIONS = new BrigadierExceptions();
        var dispatcher = new CommandDispatcher<CommandSourceStack>();
        StopCommand.register(dispatcher);
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
            for (String input : new String[]{"", "s", "st", "stop", "stop ", "stop extra", "stop  extra", "STOP"}) {
                if (permission != 4 && !(permission == 3 && input.equals("stop"))) continue;
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
        for (int permission : new int[]{0,3,4}) {
            for(String input:new String[]{"stop","stop ","stop extra","stop  extra","STOP"}) executions.add(execute(dispatcher,unsafe,input,permission));
        }
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
    static JsonObject execute(CommandDispatcher<CommandSourceStack> dispatcher,sun.misc.Unsafe unsafe,String input,int permission)throws Exception {
        var server=(net.minecraft.server.MinecraftServer)unsafe.allocateInstance(net.minecraft.server.dedicated.DedicatedServer.class);
        field(server,net.minecraft.server.MinecraftServer.class,"running",true);
        List<Component> messages=new ArrayList<>();
        var sink=new CommandSource(){public void sendSystemMessage(Component c){messages.add(c);}public boolean acceptsSuccess(){return true;}public boolean acceptsFailure(){return true;}public boolean shouldInformAdmins(){return false;}};
        var source=new CommandSourceStack(sink,Vec3.ZERO,Vec2.ZERO,null,permission,"Stop",Component.literal("Stop"),server,null);
        JsonObject out=new JsonObject();out.addProperty("input",input);out.addProperty("permission",permission);
        try{out.addProperty("result",dispatcher.execute(input,source));out.addProperty("ok",true);}catch(com.mojang.brigadier.exceptions.CommandSyntaxException e){out.addProperty("ok",false);}
        JsonArray encoded=new JsonArray();for(var message:messages)encoded.add(bytes(message));out.add("feedback_hex",encoded);
        var f=net.minecraft.server.MinecraftServer.class.getDeclaredField("running");f.setAccessible(true);out.addProperty("running",f.getBoolean(server));return out;
    }
}
