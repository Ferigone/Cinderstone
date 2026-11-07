// SPDX-License-Identifier: GPL-2.0-only
// Executes original PardonIpCommand, source metadata, results, rich feedback and permission trees.
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
import net.minecraft.server.commands.PardonIpCommand;
import net.minecraft.world.phys.*;

public class pardon_ip_command_oracle {
    @SuppressWarnings({"unchecked", "rawtypes"})
    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion(); Bootstrap.bootStrap();
        com.mojang.brigadier.exceptions.CommandSyntaxException.BUILT_IN_EXCEPTIONS = new BrigadierExceptions();
        var dispatcher = new CommandDispatcher<CommandSourceStack>();
        PardonIpCommand.register(dispatcher);
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
        for(String input:new String[]{"pardon-ip 192.0.2.7","pardon-ip 198.51.100.9","pardon-ip 203.0.113.8",
            "pardon-ip 999.1.1.1","pardon-ip 01.2.3.4","pardon-ip 127.1","pardon-ip ::1",
            "pardon-ip 192.0.2.7 extra","pardon-ip 192.0.2.7 ","pardon-ip\t192.0.2.7","pardon-ip"})
            executions.add(execute(dispatcher,unsafe,input,3));
        executions.add(execute(dispatcher,unsafe,"pardon-ip 192.0.2.7",2));
        result.add("execution",executions);
        JsonArray completions=new JsonArray();
        for(String input:new String[]{"pardon-ip","pardon-ip ","pardon-ip 19","pardon-ip 2","pardon-ip 192.0.","pardon-ip 198.51.100.9 ","pardon-ip invalid"}){
            SourceData data=source(unsafe,3);var suggestions=dispatcher.getCompletionSuggestions(dispatcher.parse(input,data.source)).join();
            JsonObject row=new JsonObject();row.addProperty("input",input);row.addProperty("start",suggestions.getRange().getStart());row.addProperty("end",suggestions.getRange().getEnd());
            JsonArray texts=new JsonArray();suggestions.getList().forEach(v->texts.add(v.getText()));row.add("texts",texts);completions.add(row);
        }
        result.add("suggestions",completions);
        Files.writeString(Path.of(args[0]),new GsonBuilder().setPrettyPrinting().create().toJson(result)+"\n");
    }
    static void field(Object object,Class<?> owner,String name,Object value)throws Exception{
        Field f=owner.getDeclaredField(name);f.setAccessible(true);f.set(object,value);
    }
    record SourceData(CommandSourceStack source,net.minecraft.server.players.IpBanList ips,List<Component> messages){}
    @SuppressWarnings({"rawtypes","unchecked"})
    static SourceData source(sun.misc.Unsafe unsafe,int permission)throws Exception{
        var server=(net.minecraft.server.MinecraftServer)unsafe.allocateInstance(net.minecraft.server.dedicated.DedicatedServer.class);
        var list=(net.minecraft.server.players.PlayerList)unsafe.allocateInstance(net.minecraft.server.dedicated.DedicatedPlayerList.class);
        var directory=Files.createTempDirectory("pardon-ip-witness-");
        var ips=new net.minecraft.server.players.IpBanList(directory.resolve("banned-ips.json").toFile());
        var map=net.minecraft.server.players.StoredUserList.class.getDeclaredField("map");map.setAccessible(true);
        for(String ip:new String[]{"192.0.2.7","198.51.100.9","2001:db8::1"})
            ((Map)map.get(ips)).put(ip,new net.minecraft.server.players.IpBanListEntry(ip,new Date(0),"Console",ip.equals("198.51.100.9")?new Date(1000):null,"fixture"));
        field(list,net.minecraft.server.players.PlayerList.class,"ipBans",ips);field(server,net.minecraft.server.MinecraftServer.class,"playerList",list);
        List<Component> messages=new ArrayList<>();
        var sink=new CommandSource(){public void sendSystemMessage(Component c){messages.add(c);}public boolean acceptsSuccess(){return true;}public boolean acceptsFailure(){return true;}public boolean shouldInformAdmins(){return false;}};
        return new SourceData(new CommandSourceStack(sink,Vec3.ZERO,Vec2.ZERO,null,permission,"Test",Component.literal("Test"),server,null),ips,messages);
    }
    @SuppressWarnings({"rawtypes","unchecked"})
    static String bytes(Component message){
        var b=new net.minecraft.network.RegistryFriendlyByteBuf(Unpooled.buffer(),net.minecraft.core.RegistryAccess.EMPTY);
        try{((StreamCodec)net.minecraft.network.protocol.game.ClientboundSystemChatPacket.STREAM_CODEC).encode(b,new net.minecraft.network.protocol.game.ClientboundSystemChatPacket(message,false));
            byte[] bytes=new byte[b.readableBytes()];b.readBytes(bytes);return HexFormat.of().formatHex(bytes);}finally{b.release();}
    }
    static JsonObject execute(CommandDispatcher<CommandSourceStack> dispatcher,sun.misc.Unsafe unsafe,String input,int permission)throws Exception{
        SourceData data=source(unsafe,permission);JsonObject out=new JsonObject();out.addProperty("input",input);out.addProperty("permission",permission);boolean parsed=false;
        try{out.addProperty("result",dispatcher.execute(input,data.source));out.addProperty("ok",true);parsed=true;}
        catch(com.mojang.brigadier.exceptions.CommandSyntaxException error){out.addProperty("ok",false);
            if(error.getRawMessage() instanceof Component component&&component.getContents() instanceof net.minecraft.network.chat.contents.TranslatableContents contents){
                out.addProperty("error_key",contents.getKey());parsed=contents.getKey().startsWith("commands.pardonip.");data.source.sendFailure(component);
            }
        }
        out.addProperty("parsed",parsed);
        if(parsed)out.addProperty("target",com.mojang.brigadier.arguments.StringArgumentType.getString(dispatcher.parse(input,data.source).getContext().build(input),"target"));
        JsonArray feedback=new JsonArray();data.messages.forEach(m->feedback.add(bytes(m)));out.add("feedback_hex",feedback);
        JsonArray remaining=new JsonArray();new TreeSet<>(Arrays.asList(data.ips.getUserList())).forEach(remaining::add);out.add("remaining",remaining);return out;
    }
}
