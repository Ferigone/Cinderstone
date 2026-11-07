// SPDX-License-Identifier: GPL-2.0-only
// Original MsgCommand, literal EntityArgument lookup and actual ServerPlayer/OutgoingChatMessage transport.
package cinderstone.oracle;
import com.google.gson.*;
import com.mojang.brigadier.*;
import com.mojang.brigadier.tree.*;
import java.nio.file.*;
import java.security.MessageDigest;
import java.util.*;
import net.minecraft.commands.*;
import net.minecraft.core.*;
import net.minecraft.core.registries.Registries;
import net.minecraft.network.chat.*;
import net.minecraft.network.protocol.game.*;
import net.minecraft.server.players.PlayerList;
import net.minecraft.world.phys.*;
public class msg_oracle extends admin_feedback_oracle{
    public static class Connection extends net.minecraft.server.network.ServerGamePacketListenerImpl{
        List<net.minecraft.network.protocol.Packet<?>> packets;
        Connection(){super(null,null,null,null);}
        public void send(net.minecraft.network.protocol.Packet<?> p){packets.add(p);}
        public void send(net.minecraft.network.protocol.Packet<?> p,net.minecraft.network.PacketSendListener l){packets.add(p);}
    }
    static String bytes(net.minecraft.network.codec.StreamCodec codec,Object value,RegistryAccess access){return say_oracle.bytes(codec,value,access);}
    static void decorate(Player p,String name,boolean enabled){p.team=null;if(enabled){p.team=new net.minecraft.world.scores.PlayerTeam(new net.minecraft.world.scores.Scoreboard(),name);p.team.setColor(name.equals("source")?net.minecraft.ChatFormatting.BLUE:net.minecraft.ChatFormatting.RED);p.team.setPlayerPrefix(Component.literal(name.equals("source")?"[S] ":"[T] ").withStyle(s->s.withFont(net.minecraft.resources.ResourceLocation.withDefaultNamespace("uniform"))));p.team.setPlayerSuffix(Component.keybind("key.jump"));}}
    @SuppressWarnings({"unchecked","rawtypes"})
    public static void main(String[] args)throws Exception{
        byte[] jar=Files.readAllBytes(Path.of(args[0]));String sha=HexFormat.of().formatHex(MessageDigest.getInstance("SHA-1").digest(jar));if(jar.length!=51623779||!sha.equals("450698d1863ab5180c25d7c804ef0fe6369dd1ba"))throw new IllegalArgumentException("Unpinned original jar");
        net.minecraft.SharedConstants.tryDetectVersion();net.minecraft.server.Bootstrap.bootStrap();CommandSyntaxExceptionSetup();
        var f=sun.misc.Unsafe.class.getDeclaredField("theUnsafe");f.setAccessible(true);var u=(sun.misc.Unsafe)f.get(null);
        var server=(say_oracle.Server)u.allocateInstance(say_oracle.Server.class);var list=(net.minecraft.server.dedicated.DedicatedPlayerList)u.allocateInstance(net.minecraft.server.dedicated.DedicatedPlayerList.class);field(server,net.minecraft.server.MinecraftServer.class,"playerList",list);
        var registry=new MappedRegistry<ChatType>(Registries.CHAT_TYPE,com.mojang.serialization.Lifecycle.stable());
        ChatType.bootstrap(new net.minecraft.data.worldgen.BootstrapContext<ChatType>(){
            public Holder.Reference<ChatType> register(net.minecraft.resources.ResourceKey<ChatType> k,ChatType v,com.mojang.serialization.Lifecycle l){return registry.register(k,v,RegistrationInfo.BUILT_IN);}
            public <S>HolderGetter<S> lookup(net.minecraft.resources.ResourceKey<? extends Registry<? extends S>> k){throw new UnsupportedOperationException();}
        });registry.freeze();server.access=new RegistryAccess.ImmutableRegistryAccess(List.of(registry)).freeze();
        var players=new ArrayList<net.minecraft.server.level.ServerPlayer>();for(String name:new String[]{"MsgSource","MsgTarget","MsgOutside"}){
            var p=(Player)u.allocateInstance(Player.class);p.profile=new com.mojang.authlib.GameProfile(UUID.nameUUIDFromBytes(("OfflinePlayer:"+name).getBytes(java.nio.charset.StandardCharsets.UTF_8)),name);p.messages=new ArrayList<>();field(p,net.minecraft.world.entity.Entity.class,"uuid",p.profile.getId());field(p,net.minecraft.world.entity.Entity.class,"type",net.minecraft.world.entity.EntityType.PLAYER);
            var c=(Connection)u.allocateInstance(Connection.class);c.packets=new ArrayList<>();p.connection=c;field(p,net.minecraft.server.level.ServerPlayer.class,"chatVisibility",net.minecraft.world.entity.player.ChatVisiblity.FULL);players.add(p);
        }field(list,PlayerList.class,"players",players);var source=(Player)players.get(0);var target=(Player)players.get(1);
        var d=new CommandDispatcher<CommandSourceStack>();net.minecraft.server.commands.MsgCommand.register(d);var out=new JsonObject();out.addProperty("java_jar_sha1",sha);
        var initial=new CommandSourceStack(source,Vec3.ZERO,Vec2.ZERO,null,0,"MsgSource",source.getDisplayName(),server,source);
        var root=new RootCommandNode<CommandSourceStack>();Map nodes=new HashMap();nodes.put(d.getRoot(),root);var fill=Commands.class.getDeclaredMethod("fillUsableCommands",CommandNode.class,CommandNode.class,CommandSourceStack.class,Map.class);fill.setAccessible(true);fill.invoke(u.allocateInstance(Commands.class),d.getRoot(),root,initial,nodes);
        out.addProperty("tree_hex",bytes(ClientboundCommandsPacket.STREAM_CODEC,new ClientboundCommandsPacket((RootCommandNode)root),server.access));
        var suggestions=new JsonArray();for(String input:new String[]{"","m","msg ","msg MsgT","tell M","w Msg","msg MsgTarget ","msg MsgTarget h"}){var a=d.getCompletionSuggestions(d.parse(input,initial)).join();var row=new JsonObject();row.addProperty("input",input);row.addProperty("start",a.getRange().getStart());row.addProperty("end",a.getRange().getEnd());var texts=new JsonArray();a.getList().forEach(v->texts.add(v.getText()));row.add("texts",texts);suggestions.add(row);}out.add("suggestions",suggestions);
        String[] inputs={"msg MsgTarget hello","tell MsgTarget   cześć  \"world\" 😀  ","w msgtarget alias","msg MsgTarget console","msg MsgSource self","msg Missing unknown","msg MsgTarget","msg MsgTarget "+"😀".repeat(129),"msg MsgTarget target hidden","msg MsgTarget source commands","msg \"MsgTarget\" quoted"};var rows=new JsonArray();
        for(int i=0;i<inputs.length;i++){
            for(var p:players){((Player)p).messages.clear();((Connection)p.connection).packets.clear();field(p,net.minecraft.server.level.ServerPlayer.class,"chatVisibility",net.minecraft.world.entity.player.ChatVisiblity.FULL);}
            decorate(source,"source",i==1);decorate(target,"target",i==1);if(i==8)field(target,net.minecraft.server.level.ServerPlayer.class,"chatVisibility",net.minecraft.world.entity.player.ChatVisiblity.SYSTEM);if(i==9)field(source,net.minecraft.server.level.ServerPlayer.class,"chatVisibility",net.minecraft.world.entity.player.ChatVisiblity.SYSTEM);
            boolean console=i==3;List<Component> consoleMessages=new ArrayList<>();var sink=new CommandSource(){public void sendSystemMessage(Component c){consoleMessages.add(c);}public boolean acceptsSuccess(){return true;}public boolean acceptsFailure(){return true;}public boolean shouldInformAdmins(){return true;}};
            var s=new CommandSourceStack(console?sink:source,Vec3.ZERO,Vec2.ZERO,null,console?4:0,console?"Server":"MsgSource",console?Component.literal("Server"):source.getDisplayName(),server,console?null:source);
            var row=new JsonObject();row.addProperty("input",inputs[i]);row.addProperty("console",console);row.addProperty("team",i==1);row.addProperty("source_visibility",i==9?1:0);row.addProperty("target_visibility",i==8?1:0);
            var parsed=d.parse(inputs[i],s);var context=parsed.getContext();while(context.getChild()!=null)context=context.getChild();var argument=context.getArguments().get("message");if(argument!=null){row.addProperty("start_utf16",argument.getRange().getStart());row.addProperty("end_utf16",argument.getRange().getEnd());row.addProperty("text",argument.getRange().get(inputs[i]));row.addProperty("body_hex",bytes(ComponentSerialization.STREAM_CODEC,Component.literal(argument.getRange().get(inputs[i])),server.access));}
            try{row.addProperty("result",d.execute(parsed));row.addProperty("ok",true);}catch(com.mojang.brigadier.exceptions.CommandSyntaxException e){row.addProperty("ok",false);var c=e.getRawMessage() instanceof Component value?value:Component.literal(e.getRawMessage().getString());if(c.getContents() instanceof net.minecraft.network.chat.contents.TranslatableContents t)row.addProperty("error_key",t.getKey());s.sendFailure(c);}
            var feedback=new JsonArray();for(var c:console?consoleMessages:source.messages)feedback.add(packet(c));row.add("feedback",feedback);
            var packets=new JsonObject();for(var p:players){var a=new JsonArray();for(var packet:((Connection)p.connection).packets)if(packet instanceof ClientboundDisguisedChatPacket chat)a.add(bytes(ClientboundDisguisedChatPacket.STREAM_CODEC,chat,server.access));packets.add(p.getGameProfile().getName(),a);}row.add("packets",packets);rows.add(row);
        }out.add("execution",rows);Files.writeString(Path.of(args[1]),new GsonBuilder().setPrettyPrinting().create().toJson(out)+"\n");
    }
    static void CommandSyntaxExceptionSetup(){com.mojang.brigadier.exceptions.CommandSyntaxException.BUILT_IN_EXCEPTIONS=new BrigadierExceptions();}
}
