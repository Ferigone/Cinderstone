// SPDX-License-Identifier: GPL-2.0-only
// Original TitleCommand/ComponentArgument with actual typed feedback and recording display packet connections.
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
public class title_oracle extends admin_feedback_oracle{
    public static class Connection extends net.minecraft.server.network.ServerGamePacketListenerImpl{
        List<net.minecraft.network.protocol.Packet<?>> packets;
        Connection(){super(null,null,null,null);}
        public void send(net.minecraft.network.protocol.Packet<?> p){packets.add(p);}
        public void send(net.minecraft.network.protocol.Packet<?> p,net.minecraft.network.PacketSendListener l){packets.add(p);}
    }
    static String bytes(net.minecraft.network.codec.StreamCodec codec,Object value,RegistryAccess access){return say_oracle.bytes(codec,value,access);}

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
        var players=new ArrayList<net.minecraft.server.level.ServerPlayer>();for(String name:new String[]{"TitleSource","TitleTarget","TitleOutside"}){
            var p=(Player)u.allocateInstance(Player.class);p.profile=new com.mojang.authlib.GameProfile(UUID.nameUUIDFromBytes(("OfflinePlayer:"+name).getBytes(java.nio.charset.StandardCharsets.UTF_8)),name);p.messages=new ArrayList<>();field(p,net.minecraft.world.entity.Entity.class,"uuid",p.profile.getId());field(p,net.minecraft.world.entity.Entity.class,"type",net.minecraft.world.entity.EntityType.PLAYER);
            var c=(Connection)u.allocateInstance(Connection.class);c.packets=new ArrayList<>();p.connection=c;field(p,net.minecraft.server.level.ServerPlayer.class,"chatVisibility",net.minecraft.world.entity.player.ChatVisiblity.FULL);players.add(p);
        }field(list,PlayerList.class,"players",players);var source=(Player)players.get(0);var target=(Player)players.get(1);
        field(list,PlayerList.class,"server",server);
        var rules=new net.minecraft.world.level.GameRules();rules.getRule(net.minecraft.world.level.GameRules.RULE_LOGADMINCOMMANDS).set(false,null);var t=net.minecraft.world.level.storage.ServerLevelData.class;var data=java.lang.reflect.Proxy.newProxyInstance(t.getClassLoader(),new Class[]{t},(p,m,a)->{if(m.getName().equals("getGameRules"))return rules;throw new UnsupportedOperationException(m.getName());});var level=(net.minecraft.server.level.ServerLevel)u.allocateInstance(net.minecraft.server.level.ServerLevel.class);field(level,net.minecraft.world.level.Level.class,"levelData",data);field(server,net.minecraft.server.MinecraftServer.class,"levels",Map.of(net.minecraft.world.level.Level.OVERWORLD,level));
        var ops=new net.minecraft.server.players.ServerOpList(new java.io.File("unused-ops.json"));var map=net.minecraft.server.players.StoredUserList.class.getDeclaredField("map");map.setAccessible(true);((Map)map.get(ops)).put(source.profile.getId().toString(),new net.minecraft.server.players.ServerOpListEntry(source.profile,2,false));field(list,PlayerList.class,"ops",ops);
        var d=new CommandDispatcher<CommandSourceStack>();net.minecraft.server.commands.TitleCommand.register(d,CommandBuildContext.simple(server.access,net.minecraft.world.flag.FeatureFlags.VANILLA_SET));var out=new JsonObject();out.addProperty("java_jar_sha1",sha);var trees=new JsonArray();
        for(int permission:new int[]{0,2}){var s=new CommandSourceStack(source,Vec3.ZERO,Vec2.ZERO,level,permission,"TitleSource",source.getDisplayName(),server,source);var root=new RootCommandNode<CommandSourceStack>();Map nodes=new HashMap();nodes.put(d.getRoot(),root);var fill=Commands.class.getDeclaredMethod("fillUsableCommands",CommandNode.class,CommandNode.class,CommandSourceStack.class,Map.class);fill.setAccessible(true);fill.invoke(u.allocateInstance(Commands.class),d.getRoot(),root,s,nodes);var r=new JsonObject();r.addProperty("permission",permission);r.addProperty("hex",bytes(ClientboundCommandsPacket.STREAM_CODEC,new ClientboundCommandsPacket((RootCommandNode)root),server.access));trees.add(r);}out.add("trees",trees);
        var initial=new CommandSourceStack(source,Vec3.ZERO,Vec2.ZERO,level,2,"TitleSource",source.getDisplayName(),server,source);var suggestions=new JsonArray();for(String input:new String[]{"tit","title ","title TitleT","title TitleTarget ","title TitleTarget t","title TitleTarget times 1","title TitleTarget title "}){var a=d.getCompletionSuggestions(d.parse(input,initial)).join();var r=new JsonObject();r.addProperty("input",input);r.addProperty("start",a.getRange().getStart());r.addProperty("end",a.getRange().getEnd());var texts=new JsonArray();a.getList().forEach(v->texts.add(v.getText()));r.add("texts",texts);suggestions.add(r);}out.add("suggestions",suggestions);
        String[] inputs={"title TitleTarget title \"cześć 😀\"","title TitleTarget subtitle {\"text\":\"rich\",\"color\":\"gold\",\"extra\":[{\"keybind\":\"key.jump\"}]}","title TitleTarget actionbar {\"translate\":\"chat.type.text\",\"with\":[\"A\",\"B\"]}","title TitleTarget times 0.5s 2s 10t","title TitleTarget clear","title TitleTarget reset","title TitleSource title [\"self\",\"!\"]","title titletarget clear","title TitleTarget actionbar \"console\"","title TitleTarget title \"hidden\"","title TitleTarget times -1 20 10","title Missing clear","title TitleTarget clear","title TitleTarget title {\"text\":\"ok\"} trailing","title TitleTarget subtitle {\"text\":","title TitleTarget times 1 2"};var rows=new JsonArray();
        for(int i=0;i<inputs.length;i++){
            for(var p:players){((Player)p).messages.clear();((Connection)p.connection).packets.clear();field(p,net.minecraft.server.level.ServerPlayer.class,"chatVisibility",net.minecraft.world.entity.player.ChatVisiblity.FULL);}
            boolean team=i==1,console=i==8;int permission=i==12?0:2,visibility=i==9?2:0;field(target,net.minecraft.server.level.ServerPlayer.class,"chatVisibility",net.minecraft.world.entity.player.ChatVisiblity.values()[visibility]);target.team=null;if(team){target.team=new net.minecraft.world.scores.PlayerTeam(new net.minecraft.world.scores.Scoreboard(),"title");target.team.setColor(net.minecraft.ChatFormatting.GOLD);target.team.setPlayerPrefix(Component.literal("[T] ").withStyle(s->s.withFont(net.minecraft.resources.ResourceLocation.withDefaultNamespace("uniform"))));target.team.setPlayerSuffix(Component.keybind("key.jump"));}
            List<Component> consoleMessages=new ArrayList<>();var sink=new CommandSource(){public void sendSystemMessage(Component c){consoleMessages.add(c);}public boolean acceptsSuccess(){return true;}public boolean acceptsFailure(){return true;}public boolean shouldInformAdmins(){return true;}};var s=new CommandSourceStack(console?sink:source,Vec3.ZERO,Vec2.ZERO,level,permission,console?"Server":"TitleSource",console?Component.literal("Server"):source.getDisplayName(),server,console?null:source);var r=new JsonObject();r.addProperty("input",inputs[i]);r.addProperty("permission",permission);r.addProperty("console",console);r.addProperty("team",team);r.addProperty("visibility",visibility);
            try{r.addProperty("result",d.execute(inputs[i],s));r.addProperty("ok",true);}catch(com.mojang.brigadier.exceptions.CommandSyntaxException e){r.addProperty("ok",false);var c=e.getRawMessage() instanceof Component value?value:Component.literal(e.getRawMessage().getString());if(c.getContents() instanceof net.minecraft.network.chat.contents.TranslatableContents t2)r.addProperty("error_key",t2.getKey());s.sendFailure(c);}
            var feedback=new JsonArray();for(var c:console?consoleMessages:source.messages){feedback.add(packet(c));if(c.getContents() instanceof net.minecraft.network.chat.contents.TranslatableContents t2)r.addProperty("feedback_key",t2.getKey());}r.add("feedback",feedback);var admin=new JsonArray();if(console)source.messages.forEach(c->admin.add(packet(c)));r.add("admin",admin);
            var packets=new JsonObject();for(var p:players){var a=new JsonArray();for(var packet:((Connection)p.connection).packets){var encoded=display(packet,server.access);if(encoded!=null){a.add(encoded);r.add("packet",encoded);}}packets.add(p.getGameProfile().getName(),a);}r.add("packets",packets);rows.add(r);
        }out.add("execution",rows);Files.writeString(Path.of(args[1]),new GsonBuilder().setPrettyPrinting().create().toJson(out)+"\n");
    }
    @SuppressWarnings({"unchecked","rawtypes"})
    static JsonObject display(net.minecraft.network.protocol.Packet<?> p,RegistryAccess access){int id;net.minecraft.network.codec.StreamCodec c;
        if(p instanceof ClientboundSetTitleTextPacket){id=0x65;c=ClientboundSetTitleTextPacket.STREAM_CODEC;}else if(p instanceof ClientboundSetSubtitleTextPacket){id=0x63;c=ClientboundSetSubtitleTextPacket.STREAM_CODEC;}else if(p instanceof ClientboundSetActionBarTextPacket){id=0x4c;c=ClientboundSetActionBarTextPacket.STREAM_CODEC;}else if(p instanceof ClientboundSetTitlesAnimationPacket){id=0x66;c=ClientboundSetTitlesAnimationPacket.STREAM_CODEC;}else if(p instanceof ClientboundClearTitlesPacket){id=0x0f;c=ClientboundClearTitlesPacket.STREAM_CODEC;}else return null;
        var r=new JsonObject();r.addProperty("id",id);r.addProperty("hex",bytes(c,p,access));return r;
    }
}
