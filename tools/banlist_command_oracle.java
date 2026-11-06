// SPDX-License-Identifier: GPL-2.0-only
// Executes original BanListCommands registration, Brigadier parse/completion and tree codec.
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
import net.minecraft.server.commands.BanListCommands;
import net.minecraft.world.phys.*;

public class banlist_command_oracle {
    @SuppressWarnings({"unchecked", "rawtypes"})
    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion(); Bootstrap.bootStrap();
        com.mojang.brigadier.exceptions.CommandSyntaxException.BUILT_IN_EXCEPTIONS = new BrigadierExceptions();
        var dispatcher = new CommandDispatcher<CommandSourceStack>();
        BanListCommands.register(dispatcher);
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
            for (String input : new String[]{"", "b", "banlist", "banlist ", "banlist i", "banlist players", "banlist ips", "banlist PLAYERS", "banlist invalid", "banlist players trailing", "banlist  players", "banlist\tplayers"}) {
                if (permission != 3 && !(permission == 2 && input.equals("banlist ips"))) continue;
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
        for (boolean populated : new boolean[]{false,true}) {
            for (String input : new String[]{"banlist", "banlist players", "banlist ips", "banlist players trailing", "banlist invalid"}) executions.add(execute(dispatcher,unsafe,populated,input,3));
        }
        executions.add(execute(dispatcher,unsafe,true,"banlist ips",2));
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
    @SuppressWarnings({"rawtypes","unchecked"})
    static JsonObject execute(CommandDispatcher<CommandSourceStack> dispatcher,sun.misc.Unsafe unsafe,boolean populated,String input,int permission)throws Exception {
        var server=(net.minecraft.server.MinecraftServer)unsafe.allocateInstance(net.minecraft.server.dedicated.DedicatedServer.class);
        var list=unsafe.allocateInstance(net.minecraft.server.dedicated.DedicatedPlayerList.class);
        var bans=new net.minecraft.server.players.UserBanList(new java.io.File("unused-banlist-players.json"));
        var ips=new net.minecraft.server.players.IpBanList(new java.io.File("unused-banlist-ips.json"));
        var map=net.minecraft.server.players.StoredUserList.class.getDeclaredField("map");map.setAccessible(true);
        if(populated){
            for(String[] p:new String[][]{{"00000000-0000-0000-0000-000000000001","BannedOne","Operator","Reason \"one\" ☃","forever"},{"00000000-0000-0000-0000-000000000002","ExpiredOne","Earlier","Expired reason","past"}}){
                var profile=new com.mojang.authlib.GameProfile(UUID.fromString(p[0]),p[1]);
                var entry=new net.minecraft.server.players.UserBanListEntry(profile,new java.util.Date(0),p[2],p[4].equals("past")?new java.util.Date(1):null,p[3]);
                ((Map)map.get(bans)).put(p[0],entry);
            }
            ((Map)map.get(ips)).put("192.0.2.7",new net.minecraft.server.players.IpBanListEntry("192.0.2.7",new java.util.Date(0),"Console",null,"IP reason"));
        }
        field(list,net.minecraft.server.players.PlayerList.class,"bans",bans);field(list,net.minecraft.server.players.PlayerList.class,"ipBans",ips);field(server,net.minecraft.server.MinecraftServer.class,"playerList",list);
        List<Component> messages=new ArrayList<>();
        var sink=new CommandSource(){public void sendSystemMessage(Component c){messages.add(c);}public boolean acceptsSuccess(){return true;}public boolean acceptsFailure(){return true;}public boolean shouldInformAdmins(){return false;}};
        var source=new CommandSourceStack(sink,Vec3.ZERO,Vec2.ZERO,null,permission,"Banlist",Component.literal("Banlist"),server,null);
        JsonObject out=new JsonObject();out.addProperty("input",input);out.addProperty("permission",permission);out.addProperty("populated",populated);
        JsonArray entries=new JsonArray();
        var selected=new ArrayList<net.minecraft.server.players.BanListEntry<?>>();
        if(!input.equals("banlist ips"))selected.addAll(bans.getEntries());if(!input.equals("banlist players"))selected.addAll(ips.getEntries());
        for(var entry:selected){var row=new JsonObject();row.addProperty("display_name",entry.getDisplayName().getString());row.addProperty("source",entry.getSource());row.addProperty("reason",entry.getReason());entries.add(row);}out.add("entries",entries);
        try{out.addProperty("result",dispatcher.execute(input,source));out.addProperty("ok",true);}catch(com.mojang.brigadier.exceptions.CommandSyntaxException e){out.addProperty("ok",false);}
        JsonArray encoded=new JsonArray();for(var message:messages)encoded.add(bytes(message));out.add("feedback_hex",encoded);out.addProperty("retained_player_bans",bans.getEntries().size());out.addProperty("retained_ip_bans",ips.getEntries().size());return out;
    }
}
