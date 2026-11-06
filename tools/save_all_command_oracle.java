// SPDX-License-Identifier: GPL-2.0-only
// Executes original SaveAllCommand, source metadata, results, rich feedback and permission trees.
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
import net.minecraft.server.commands.SaveAllCommand;
import net.minecraft.world.phys.*;

public class save_all_command_oracle {
    public static class Server extends net.minecraft.server.dedicated.DedicatedServer {
        boolean success,called,flush,force,quiet;
        Server(){super(null,null,null,null,null,null,null,null);}
        public boolean saveEverything(boolean quiet,boolean flush,boolean force){
            called=true;this.quiet=quiet;this.flush=flush;this.force=force;return success;
        }
    }
    static class Source extends CommandSourceStack {
        List<Component> messages=new ArrayList<>();List<Boolean> broadcasts=new ArrayList<>();
        Source(Server server,int permission){super(CommandSource.NULL,Vec3.ZERO,Vec2.ZERO,null,permission,"n",Component.literal("n"),server,null);}
        public void sendSuccess(java.util.function.Supplier<Component> message,boolean broadcast){messages.add(message.get());broadcasts.add(broadcast);}
    }

    @SuppressWarnings({"unchecked", "rawtypes"})
    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion(); Bootstrap.bootStrap();
        com.mojang.brigadier.exceptions.CommandSyntaxException.BUILT_IN_EXCEPTIONS = new BrigadierExceptions();
        var dispatcher = new CommandDispatcher<CommandSourceStack>();
        SaveAllCommand.register(dispatcher);
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
        JsonArray executions=new JsonArray();
        for(String input:new String[]{"save-all","save-all flush","save-all bad","save-all flush extra","save-all ","save-all flush "})
            executions.add(execute(dispatcher,unsafe,input,4,true));
        executions.add(execute(dispatcher,unsafe,"save-all",3,true));
        executions.add(execute(dispatcher,unsafe,"save-all flush",4,false));
        result.add("execution",executions);
        Files.writeString(Path.of(args[0]),new GsonBuilder().setPrettyPrinting().create().toJson(result)+"\n");
    }
    @SuppressWarnings({"unchecked","rawtypes"})
    static JsonObject execute(CommandDispatcher<CommandSourceStack> dispatcher,sun.misc.Unsafe unsafe,String input,int permission,boolean success) throws Exception {
        Server server=(Server)unsafe.allocateInstance(Server.class);server.success=success;
        Source source=new Source(server,permission);JsonObject out=new JsonObject();
        out.addProperty("input",input);out.addProperty("permission",permission);out.addProperty("save_success",success);
        try{out.addProperty("result",dispatcher.execute(input,source));out.addProperty("ok",true);}
        catch(com.mojang.brigadier.exceptions.CommandSyntaxException error){
            out.addProperty("ok",false);
            if(error.getRawMessage() instanceof Component component && component.getContents() instanceof net.minecraft.network.chat.contents.TranslatableContents contents)
                out.addProperty("error_key",contents.getKey());
        }
        out.addProperty("called",server.called);out.addProperty("quiet",server.quiet);out.addProperty("flush",server.flush);out.addProperty("force",server.force);
        JsonArray feedback=new JsonArray();
        for(int i=0;i<source.messages.size();i++){
            var buffer=new net.minecraft.network.RegistryFriendlyByteBuf(Unpooled.buffer(),net.minecraft.core.RegistryAccess.EMPTY);
            try{
                Component message=source.messages.get(i);
                ((StreamCodec)net.minecraft.network.protocol.game.ClientboundSystemChatPacket.STREAM_CODEC).encode(buffer,new net.minecraft.network.protocol.game.ClientboundSystemChatPacket(message,false));
                byte[] bytes=new byte[buffer.readableBytes()];buffer.readBytes(bytes);JsonObject row=new JsonObject();
                row.addProperty("hex",HexFormat.of().formatHex(bytes));row.addProperty("broadcast",source.broadcasts.get(i));
                row.addProperty("key",((net.minecraft.network.chat.contents.TranslatableContents)message.getContents()).getKey());feedback.add(row);
            }finally{buffer.release();}
        }
        out.add("feedback",feedback);return out;
    }
}
