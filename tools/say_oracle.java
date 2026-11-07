// SPDX-License-Identifier: GPL-2.0-only
// Executes original SayCommand or EmoteCommands/MessageArgument to a recording PlayerList broadcast sink.
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
import net.minecraft.server.players.PlayerList;
import net.minecraft.world.phys.*;
public class say_oracle extends admin_feedback_oracle {
    public static class Server extends net.minecraft.server.dedicated.DedicatedServer {
        RegistryAccess.Frozen access;
        Server(){super(null,null,null,null,null,null,null,null);}
        public RegistryAccess.Frozen registryAccess(){return access;}
        public ChatDecorator getChatDecorator(){return ChatDecorator.PLAIN;}
    }
    public static class Players extends net.minecraft.server.dedicated.DedicatedPlayerList {
        PlayerChatMessage message;ChatType.Bound bound;
        Players(){super(null,null,null);}
        public void broadcastChatMessage(PlayerChatMessage m,CommandSourceStack s,ChatType.Bound b){message=m;bound=b;}
    }
    static String bytes(net.minecraft.network.codec.StreamCodec codec,Object value,RegistryAccess access){
        var b=new net.minecraft.network.RegistryFriendlyByteBuf(io.netty.buffer.Unpooled.buffer(),access);
        try{codec.encode(b,value);return HexFormat.of().formatHex(io.netty.buffer.ByteBufUtil.getBytes(b));}finally{b.release();}
    }
    @SuppressWarnings({"unchecked","rawtypes"})
    public static void main(String[] args)throws Exception{
        byte[] jar=Files.readAllBytes(Path.of(args[0]));String sha=HexFormat.of().formatHex(MessageDigest.getInstance("SHA-1").digest(jar));if(jar.length!=51623779||!sha.equals("450698d1863ab5180c25d7c804ef0fe6369dd1ba"))throw new IllegalArgumentException("Unpinned original jar");
        net.minecraft.SharedConstants.tryDetectVersion();net.minecraft.server.Bootstrap.bootStrap();com.mojang.brigadier.exceptions.CommandSyntaxException.BUILT_IN_EXCEPTIONS=new BrigadierExceptions();
        var f=sun.misc.Unsafe.class.getDeclaredField("theUnsafe");f.setAccessible(true);var u=(sun.misc.Unsafe)f.get(null);
        var server=(Server)u.allocateInstance(Server.class);var list=(Players)u.allocateInstance(Players.class);field(server,net.minecraft.server.MinecraftServer.class,"playerList",list);
        var registry=new MappedRegistry<ChatType>(Registries.CHAT_TYPE,com.mojang.serialization.Lifecycle.stable());
        ChatType.bootstrap(new net.minecraft.data.worldgen.BootstrapContext<ChatType>(){
            public Holder.Reference<ChatType> register(net.minecraft.resources.ResourceKey<ChatType> k,ChatType v,com.mojang.serialization.Lifecycle l){return registry.register(k,v,RegistrationInfo.BUILT_IN);}
            public <S>HolderGetter<S> lookup(net.minecraft.resources.ResourceKey<? extends Registry<? extends S>> k){throw new UnsupportedOperationException();}
        });registry.freeze();server.access=new RegistryAccess.ImmutableRegistryAccess(List.of(registry)).freeze();
        var player=(Player)u.allocateInstance(Player.class);player.profile=new com.mojang.authlib.GameProfile(UUID.nameUUIDFromBytes("OfflinePlayer:SaySource".getBytes(java.nio.charset.StandardCharsets.UTF_8)),"SaySource");player.messages=new ArrayList<>();field(player,net.minecraft.world.entity.Entity.class,"uuid",player.profile.getId());field(player,net.minecraft.world.entity.Entity.class,"type",net.minecraft.world.entity.EntityType.PLAYER);
        boolean emote=args.length>2&&args[2].equals("emote");
        var d=new CommandDispatcher<CommandSourceStack>();if(emote)net.minecraft.server.commands.EmoteCommands.register(d);else net.minecraft.server.commands.SayCommand.register(d);var out=new JsonObject();out.addProperty("java_jar_sha1",sha);
        var trees=new JsonArray();for(int permission:new int[]{0,2,4}){
            var s=new CommandSourceStack(player,Vec3.ZERO,Vec2.ZERO,null,permission,"SaySource",player.getDisplayName(),server,player);var root=new RootCommandNode<CommandSourceStack>();Map nodes=new HashMap();nodes.put(d.getRoot(),root);var fill=Commands.class.getDeclaredMethod("fillUsableCommands",CommandNode.class,CommandNode.class,CommandSourceStack.class,Map.class);fill.setAccessible(true);fill.invoke(u.allocateInstance(Commands.class),d.getRoot(),root,s,nodes);
            var row=new JsonObject();row.addProperty("permission",permission);row.addProperty("hex",bytes(net.minecraft.network.protocol.game.ClientboundCommandsPacket.STREAM_CODEC,new net.minecraft.network.protocol.game.ClientboundCommandsPacket((RootCommandNode)root),server.access));trees.add(row);
        }out.add("trees",trees);var execution=new JsonArray();
        String[] inputs={"say hello","say   cześć  \"world\" 😀  ","say hello","say hello","say","say ","say "+"😀".repeat(129),"say plain @x"};
        for(int i=0;i<inputs.length;i++){
            if(emote)inputs[i]="me"+inputs[i].substring(3);
            boolean console=i==2,team=i==1;int permission=i==3?0:2;player.team=null;if(team){player.team=new net.minecraft.world.scores.PlayerTeam(new net.minecraft.world.scores.Scoreboard(),"sayteam");player.team.setColor(net.minecraft.ChatFormatting.BLUE);player.team.setPlayerPrefix(Component.literal("[S] ").withStyle(s->s.withFont(net.minecraft.resources.ResourceLocation.withDefaultNamespace("uniform"))));player.team.setPlayerSuffix(Component.keybind("key.jump"));}
            var sink=new CommandSource(){public void sendSystemMessage(Component c){}public boolean acceptsSuccess(){return true;}public boolean acceptsFailure(){return true;}public boolean shouldInformAdmins(){return true;}};
            var s=new CommandSourceStack(console?sink:player,Vec3.ZERO,Vec2.ZERO,null,permission,console?"Server":"SaySource",console?Component.literal("Server"):player.getDisplayName(),server,console?null:player);
            var row=new JsonObject();row.addProperty("input",inputs[i]);row.addProperty("console",console);row.addProperty("team",team);row.addProperty("permission",permission);list.message=null;list.bound=null;
            var parsed=d.parse(inputs[i],s);var argument=parsed.getContext().getArguments().get(emote?"action":"message");if(argument!=null){row.addProperty("start_utf16",argument.getRange().getStart());row.addProperty("end_utf16",argument.getRange().getEnd());}
            try{row.addProperty("result",d.execute(parsed));row.addProperty("ok",true);row.addProperty("message",list.message.signedContent());row.addProperty("type_key",list.bound.chatType().unwrapKey().orElseThrow().location().toString());row.addProperty("body_hex",bytes(ComponentSerialization.STREAM_CODEC,list.message.decoratedContent(),server.access));row.addProperty("name_hex",bytes(ComponentSerialization.STREAM_CODEC,list.bound.name(),server.access));row.addProperty("disguised_hex",bytes(net.minecraft.network.protocol.game.ClientboundDisguisedChatPacket.STREAM_CODEC,new net.minecraft.network.protocol.game.ClientboundDisguisedChatPacket(list.message.decoratedContent(),list.bound),server.access));}
            catch(com.mojang.brigadier.exceptions.CommandSyntaxException e){row.addProperty("ok",false);if(e.getRawMessage() instanceof Component c&&c.getContents() instanceof net.minecraft.network.chat.contents.TranslatableContents t)row.addProperty("error_key",t.getKey());}
            execution.add(row);
        }out.add("execution",execution);Files.writeString(Path.of(args[1]),new GsonBuilder().setPrettyPrinting().create().toJson(out)+"\n");
    }
}
