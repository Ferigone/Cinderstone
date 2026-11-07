// SPDX-License-Identifier: GPL-2.0-only
// Executes original TriggerCommand with real ServerScoreboard/ScoreAccess and recording transport.
package cinderstone.oracle;
import com.google.gson.*;
import com.mojang.brigadier.*;
import com.mojang.brigadier.tree.*;
import java.lang.reflect.*;
import java.nio.file.*;
import java.security.MessageDigest;
import java.util.*;
import net.minecraft.commands.*;
import net.minecraft.network.chat.Component;
import net.minecraft.network.protocol.game.*;
import net.minecraft.server.ServerScoreboard;
import net.minecraft.server.players.*;
import net.minecraft.world.level.GameRules;
import net.minecraft.world.phys.*;
import net.minecraft.world.scores.*;
import net.minecraft.world.scores.criteria.ObjectiveCriteria;

public class trigger_oracle extends admin_feedback_oracle {
    public static class Player extends admin_feedback_oracle.Player {
        ServerScoreboard board;
        Player(){super();}
        public Scoreboard getScoreboard(){return board;}
    }
    public static class Connection extends net.minecraft.server.network.ServerGamePacketListenerImpl {
        List<net.minecraft.network.protocol.Packet<?>> packets;
        Connection(){super(null,null,null,null);}
        public void send(net.minecraft.network.protocol.Packet<?> p){packets.add(p);}
        public void send(net.minecraft.network.protocol.Packet<?> p,net.minecraft.network.PacketSendListener callback){packets.add(p);}
    }
    @SuppressWarnings({"rawtypes","unchecked"})
    static String bytes(net.minecraft.network.codec.StreamCodec codec,Object value){var b=new net.minecraft.network.RegistryFriendlyByteBuf(io.netty.buffer.Unpooled.buffer(),net.minecraft.core.RegistryAccess.EMPTY);try{codec.encode(b,value);return HexFormat.of().formatHex(io.netty.buffer.ByteBufUtil.getBytes(b));}finally{b.release();}}
    static sun.misc.Unsafe unsafe;static net.minecraft.server.MinecraftServer server;static Player player,observer;
    static ServerScoreboard setup(int initial,boolean enabled)throws Exception{
        var board=new ServerScoreboard(server);field(server,net.minecraft.server.MinecraftServer.class,"scoreboard",board);player.board=board;observer.board=board;
        for(String name:new String[]{"click","deep_click","deep.click","locked","absent","dummy"}){
            var display=name.equals("click")?Component.literal("Press").withStyle(net.minecraft.ChatFormatting.GOLD,net.minecraft.ChatFormatting.BOLD):Component.literal(name);
            var objective=board.addObjective(name,name.equals("dummy")?ObjectiveCriteria.DUMMY:ObjectiveCriteria.TRIGGER,display,ObjectiveCriteria.RenderType.INTEGER,false,null);
            if(name.equals("click")||name.startsWith("deep")||name.equals("dummy")){
                var score=board.getOrCreatePlayerScore(player,objective);score.set(name.equals("click")?initial:0);
                if(!name.equals("click")||enabled)score.unlock();
            }
            if(name.equals("click"))board.setDisplayObjective(DisplaySlot.SIDEBAR,objective);
        }
        player.messages.clear();observer.messages.clear();((Connection)player.connection).packets.clear();((Connection)observer.connection).packets.clear();return board;
    }
    static JsonElement state(ServerScoreboard board,String name){var o=board.getObjective(name);var s=o==null?null:board.getPlayerScoreInfo(player,o);if(s==null)return JsonNull.INSTANCE;var out=new JsonObject();out.addProperty("value",s.value());out.addProperty("locked",s.isLocked());return out;}
    @SuppressWarnings({"rawtypes","unchecked"})
    public static void main(String[] args)throws Exception{
        byte[] jar=Files.readAllBytes(Path.of(args[0]));String sha=HexFormat.of().formatHex(MessageDigest.getInstance("SHA-1").digest(jar));if(jar.length!=51623779||!sha.equals("450698d1863ab5180c25d7c804ef0fe6369dd1ba"))throw new IllegalArgumentException("Unpinned original jar");
        net.minecraft.SharedConstants.tryDetectVersion();net.minecraft.server.Bootstrap.bootStrap();com.mojang.brigadier.exceptions.CommandSyntaxException.BUILT_IN_EXCEPTIONS=new BrigadierExceptions();
        var f=sun.misc.Unsafe.class.getDeclaredField("theUnsafe");f.setAccessible(true);unsafe=(sun.misc.Unsafe)f.get(null);
        server=(net.minecraft.server.MinecraftServer)unsafe.allocateInstance(net.minecraft.server.dedicated.DedicatedServer.class);
        var rules=new GameRules();rules.getRule(GameRules.RULE_LOGADMINCOMMANDS).set(false,null);
        var t=net.minecraft.world.level.storage.ServerLevelData.class;var data=Proxy.newProxyInstance(t.getClassLoader(),new Class[]{t},(p,m,a)->{if(m.getName().equals("getGameRules"))return rules;throw new UnsupportedOperationException(m.getName());});
        var level=(net.minecraft.server.level.ServerLevel)unsafe.allocateInstance(net.minecraft.server.level.ServerLevel.class);field(level,net.minecraft.world.level.Level.class,"levelData",data);field(server,net.minecraft.server.MinecraftServer.class,"levels",Map.of(net.minecraft.world.level.Level.OVERWORLD,level));
        var players=new ArrayList<net.minecraft.server.level.ServerPlayer>();
        for(String name:new String[]{"TriggerUser","TriggerOp"}){
            var p=(Player)unsafe.allocateInstance(Player.class);p.profile=new com.mojang.authlib.GameProfile(UUID.nameUUIDFromBytes(("OfflinePlayer:"+name).getBytes(java.nio.charset.StandardCharsets.UTF_8)),name);p.messages=new ArrayList<>();
            field(p,net.minecraft.world.entity.Entity.class,"uuid",p.profile.getId());field(p,net.minecraft.world.entity.Entity.class,"type",net.minecraft.world.entity.EntityType.PLAYER);
            var c=(Connection)unsafe.allocateInstance(Connection.class);c.packets=new ArrayList<>();p.connection=c;players.add(p);
        }player=(Player)players.get(0);observer=(Player)players.get(1);
        var list=(PlayerList)unsafe.allocateInstance(net.minecraft.server.dedicated.DedicatedPlayerList.class);var ops=new ServerOpList(new java.io.File("unused-ops.json"));var map=StoredUserList.class.getDeclaredField("map");map.setAccessible(true);
        ((Map)map.get(ops)).put(observer.profile.getId().toString(),new ServerOpListEntry(observer.profile,4,false));field(list,PlayerList.class,"players",players);field(list,PlayerList.class,"ops",ops);field(server,net.minecraft.server.MinecraftServer.class,"playerList",list);
        var dispatcher=new CommandDispatcher<CommandSourceStack>();net.minecraft.server.commands.TriggerCommand.register(dispatcher);
        var source=new CommandSourceStack(player,Vec3.ZERO,Vec2.ZERO,level,0,"TriggerUser",player.getDisplayName(),server,player);
        setup(0,true);var output=new JsonObject();output.addProperty("java_jar_sha1",sha);
        var root=new RootCommandNode<CommandSourceStack>();Map nodes=new HashMap();nodes.put(dispatcher.getRoot(),root);var fill=Commands.class.getDeclaredMethod("fillUsableCommands",CommandNode.class,CommandNode.class,CommandSourceStack.class,Map.class);fill.setAccessible(true);fill.invoke(unsafe.allocateInstance(Commands.class),dispatcher.getRoot(),root,source,nodes);
        output.addProperty("tree_hex",bytes(ClientboundCommandsPacket.STREAM_CODEC,new ClientboundCommandsPacket((RootCommandNode)root)));
        var completion=new JsonArray();for(String input:new String[]{"","tr","trigger ","trigger c","trigger deep","trigger click ","trigger click a","trigger click add "}){
            var s=dispatcher.getCompletionSuggestions(dispatcher.parse(input,source)).join();var row=new JsonObject();row.addProperty("input",input);row.addProperty("start",s.getRange().getStart());row.addProperty("end",s.getRange().getEnd());var texts=new JsonArray();s.getList().forEach(v->texts.add(v.getText()));row.add("texts",texts);completion.add(row);
        }output.add("suggestions",completion);var execution=new JsonArray();
        String[] inputs={"trigger click","trigger click add 3","trigger click set -7","trigger click add 1","trigger click","trigger click set 0","trigger click add 5 trailing","trigger click set bad","trigger click add 2147483648","trigger missing","trigger dummy","trigger absent","trigger click","trigger click"};
        int[] initial={0,5,5,Integer.MAX_VALUE,Integer.MAX_VALUE,0,9,9,9,0,0,0,5,5};
        for(int i=0;i<inputs.length;i++){
            boolean enabled=i!=12,console=i==13;var board=setup(initial[i],enabled);var row=new JsonObject();row.addProperty("input",inputs[i]);row.addProperty("initial",initial[i]);row.addProperty("enabled",enabled);row.addProperty("console",console);
            String objective=inputs[i].split(" ")[1];row.add("before",state(board,objective));List<Component> consoleMessages=new ArrayList<>();
            var sink=new CommandSource(){public void sendSystemMessage(Component c){consoleMessages.add(c);}public boolean acceptsSuccess(){return true;}public boolean acceptsFailure(){return true;}public boolean shouldInformAdmins(){return true;}};
            var s=console?new CommandSourceStack(sink,Vec3.ZERO,Vec2.ZERO,level,4,"Server",Component.literal("Server"),server,null):source;
            try{row.addProperty("result",dispatcher.execute(inputs[i],s));row.addProperty("ok",true);}catch(com.mojang.brigadier.exceptions.CommandSyntaxException e){row.addProperty("ok",false);Component c=e.getRawMessage() instanceof Component value?value:Component.literal(e.getRawMessage().getString());if(c.getContents() instanceof net.minecraft.network.chat.contents.TranslatableContents value)row.addProperty("error_key",value.getKey());s.sendFailure(c);}
            row.add("after",state(board,objective));var messages=new JsonArray();for(var c:console?consoleMessages:player.messages)messages.add(packet(c));row.add("feedback",messages);var admin=new JsonArray();observer.messages.forEach(c->admin.add(packet(c)));row.add("admin",admin);
            var scores=new JsonArray();for(var p:((Connection)observer.connection).packets)if(p instanceof ClientboundSetScorePacket value)scores.add(bytes(ClientboundSetScorePacket.STREAM_CODEC,value));row.add("score_packets",scores);execution.add(row);
        }output.add("execution",execution);Files.writeString(Path.of(args[1]),new GsonBuilder().setPrettyPrinting().create().toJson(output)+"\n");
    }
}
