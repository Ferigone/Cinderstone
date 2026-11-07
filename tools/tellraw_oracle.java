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
public class tellraw_oracle extends admin_feedback_oracle{
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
        net.minecraft.SharedConstants.tryDetectVersion();net.minecraft.server.Bootstrap.bootStrap();com.mojang.brigadier.exceptions.CommandSyntaxException.BUILT_IN_EXCEPTIONS=new BrigadierExceptions();
        var f=sun.misc.Unsafe.class.getDeclaredField("theUnsafe");f.setAccessible(true);var u=(sun.misc.Unsafe)f.get(null);
        var server=(say_oracle.Server)u.allocateInstance(say_oracle.Server.class);var list=(net.minecraft.server.dedicated.DedicatedPlayerList)u.allocateInstance(net.minecraft.server.dedicated.DedicatedPlayerList.class);field(server,net.minecraft.server.MinecraftServer.class,"playerList",list);
        var registry=new MappedRegistry<ChatType>(Registries.CHAT_TYPE,com.mojang.serialization.Lifecycle.stable());
        ChatType.bootstrap(new net.minecraft.data.worldgen.BootstrapContext<ChatType>(){
            public Holder.Reference<ChatType> register(net.minecraft.resources.ResourceKey<ChatType> k,ChatType v,com.mojang.serialization.Lifecycle l){return registry.register(k,v,RegistrationInfo.BUILT_IN);}
            public <S>HolderGetter<S> lookup(net.minecraft.resources.ResourceKey<? extends Registry<? extends S>> k){throw new UnsupportedOperationException();}
        });registry.freeze();server.access=new RegistryAccess.ImmutableRegistryAccess(List.of(registry)).freeze();
        var players=new ArrayList<net.minecraft.server.level.ServerPlayer>();for(String name:new String[]{"RawSource","RawTarget","RawOutside"}){
            var p=(Player)u.allocateInstance(Player.class);p.profile=new com.mojang.authlib.GameProfile(UUID.nameUUIDFromBytes(("OfflinePlayer:"+name).getBytes(java.nio.charset.StandardCharsets.UTF_8)),name);p.messages=new ArrayList<>();field(p,net.minecraft.world.entity.Entity.class,"uuid",p.profile.getId());field(p,net.minecraft.world.entity.Entity.class,"type",net.minecraft.world.entity.EntityType.PLAYER);
            var c=(Connection)u.allocateInstance(Connection.class);c.packets=new ArrayList<>();p.connection=c;field(p,net.minecraft.server.level.ServerPlayer.class,"chatVisibility",net.minecraft.world.entity.player.ChatVisiblity.FULL);players.add(p);
        }field(list,PlayerList.class,"players",players);var source=(Player)players.get(0);var target=(Player)players.get(1);
        var d=new CommandDispatcher<CommandSourceStack>();net.minecraft.server.commands.TellRawCommand.register(d,CommandBuildContext.simple(server.access,net.minecraft.world.flag.FeatureFlags.VANILLA_SET));var out=new JsonObject();out.addProperty("java_jar_sha1",sha);
        var trees=new JsonArray();for(int permission:new int[]{0,2}){
            var s=new CommandSourceStack(source,Vec3.ZERO,Vec2.ZERO,null,permission,"RawSource",source.getDisplayName(),server,source);var root=new RootCommandNode<CommandSourceStack>();Map nodes=new HashMap();nodes.put(d.getRoot(),root);var fill=Commands.class.getDeclaredMethod("fillUsableCommands",CommandNode.class,CommandNode.class,CommandSourceStack.class,Map.class);fill.setAccessible(true);fill.invoke(u.allocateInstance(Commands.class),d.getRoot(),root,s,nodes);
            var row=new JsonObject();row.addProperty("permission",permission);row.addProperty("hex",bytes(ClientboundCommandsPacket.STREAM_CODEC,new ClientboundCommandsPacket((RootCommandNode)root),server.access));trees.add(row);
        }out.add("trees",trees);
        var initial=new CommandSourceStack(source,Vec3.ZERO,Vec2.ZERO,null,2,"RawSource",source.getDisplayName(),server,source);var suggestions=new JsonArray();
        for(String input:new String[]{"","tel","tellraw ","tellraw RawT","tellraw RawTarget ","tellraw RawTarget {"}){var a=d.getCompletionSuggestions(d.parse(input,initial)).join();var r=new JsonObject();r.addProperty("input",input);r.addProperty("start",a.getRange().getStart());r.addProperty("end",a.getRange().getEnd());var texts=new JsonArray();a.getList().forEach(v->texts.add(v.getText()));r.add("texts",texts);suggestions.add(r);}out.add("suggestions",suggestions);
        String[] inputs={"tellraw RawTarget \"cześć 😀\"","tellraw rawtarget {\"text\":\"rich\",\"color\":\"gold\",\"bold\":true,\"font\":\"minecraft:uniform\",\"extra\":[{\"keybind\":\"key.jump\"}],\"clickEvent\":{\"action\":\"suggest_command\",\"value\":\"/help\"},\"hoverEvent\":{\"action\":\"show_text\",\"contents\":{\"text\":\"hover\",\"italic\":true}}}","tellraw RawTarget {\"translate\":\"chat.type.text\",\"with\":[\"A\",{\"text\":\"B\",\"color\":\"blue\"}]}","tellraw RawSource [\"self\",{\"text\":\"!\",\"bold\":true}]","tellraw RawTarget \"console\"","tellraw RawTarget \"system\"","tellraw RawTarget \"hidden\"","tellraw RawTarget \"denied\"","tellraw Missing \"unknown\"","tellraw RawTarget {\"text\":\"ok\"} trailing","tellraw RawTarget {\"text\":","tellraw RawTarget","tellraw RawTarget \"\"","tellraw RawTarget {\"text\":\"end\"}   "};
        var rows=new JsonArray();for(int i=0;i<inputs.length;i++){
            for(var p:players){((Player)p).messages.clear();((Connection)p.connection).packets.clear();field(p,net.minecraft.server.level.ServerPlayer.class,"chatVisibility",net.minecraft.world.entity.player.ChatVisiblity.FULL);}
            int visibility=i==5?1:i==6?2:0;field(target,net.minecraft.server.level.ServerPlayer.class,"chatVisibility",net.minecraft.world.entity.player.ChatVisiblity.values()[visibility]);boolean console=i==4;int permission=i==7?0:2;List<Component> consoleMessages=new ArrayList<>();
            var sink=new CommandSource(){public void sendSystemMessage(Component c){consoleMessages.add(c);}public boolean acceptsSuccess(){return true;}public boolean acceptsFailure(){return true;}public boolean shouldInformAdmins(){return true;}};
            var s=new CommandSourceStack(console?sink:source,Vec3.ZERO,Vec2.ZERO,null,permission,console?"Server":"RawSource",console?Component.literal("Server"):source.getDisplayName(),server,console?null:source);
            var r=new JsonObject();r.addProperty("input",inputs[i]);r.addProperty("permission",permission);r.addProperty("console",console);r.addProperty("visibility",visibility);var parsed=d.parse(inputs[i],s);var argument=parsed.getContext().getArguments().get("message");if(argument!=null){r.addProperty("start_utf16",argument.getRange().getStart());r.addProperty("end_utf16",argument.getRange().getEnd());}
            try{r.addProperty("result",d.execute(parsed));r.addProperty("ok",true);}catch(com.mojang.brigadier.exceptions.CommandSyntaxException e){r.addProperty("ok",false);if(e.getRawMessage() instanceof Component c&&c.getContents() instanceof net.minecraft.network.chat.contents.TranslatableContents t)r.addProperty("error_key",t.getKey());}
            var packets=new JsonObject();for(var p:players){var a=new JsonArray();for(var packet:((Connection)p.connection).packets)if(packet instanceof ClientboundSystemChatPacket chat)a.add(bytes(ClientboundSystemChatPacket.STREAM_CODEC,chat,server.access));packets.add(p.getGameProfile().getName(),a);}r.add("packets",packets);if(!source.messages.isEmpty()||!target.messages.isEmpty()||!consoleMessages.isEmpty())throw new AssertionError("unexpected feedback");rows.add(r);
        }out.add("execution",rows);Files.writeString(Path.of(args[1]),new GsonBuilder().setPrettyPrinting().create().toJson(out)+"\n");
    }
}
