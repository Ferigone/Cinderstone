// SPDX-License-Identifier: GPL-2.0-only
// Original system-message visibility and admin broadcasting; record only the connection send.
package cinderstone.oracle;

import com.google.gson.*;
import com.mojang.authlib.GameProfile;
import java.lang.reflect.*;
import java.nio.file.*;
import java.security.MessageDigest;
import java.util.*;
import net.minecraft.SharedConstants;
import net.minecraft.commands.*;
import net.minecraft.network.chat.Component;
import net.minecraft.network.protocol.game.ClientboundSystemChatPacket;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.server.players.*;
import net.minecraft.world.entity.player.ChatVisiblity;
import net.minecraft.world.level.GameRules;
import net.minecraft.world.phys.*;

public class admin_visibility_oracle extends admin_feedback_oracle {
    public static class VisiblePlayer extends ServerPlayer {
        GameProfile profile;
        VisiblePlayer(){super(null,null,null,null);}
        public GameProfile getGameProfile(){return profile;}
        public Component getName(){return Component.literal(profile.getName());}
        public net.minecraft.world.scores.PlayerTeam getTeam(){return null;}
        public boolean acceptsSuccess(){return true;}
        public boolean acceptsFailure(){return true;}
        public boolean shouldInformAdmins(){return true;}
        // Original ServerPlayer.sendSystemMessage and its visibility decision are inherited.
    }
    public static class RecordingConnection extends net.minecraft.server.network.ServerGamePacketListenerImpl {
        List<ClientboundSystemChatPacket> packets;
        RecordingConnection(){super(null,null,null,null);}
        public void send(net.minecraft.network.protocol.Packet<?> p,net.minecraft.network.PacketSendListener callback){
            if(p instanceof ClientboundSystemChatPacket message)packets.add(message);else throw new AssertionError(p);
        }
    }
    static JsonArray encoded(RecordingConnection connection){
        var rows=new JsonArray();for(var p:connection.packets){if(p.overlay())throw new AssertionError("overlay");rows.add(packet(p.content()));}return rows;
    }
    @SuppressWarnings({"unchecked","rawtypes"})
    public static void main(String[] args)throws Exception {
        byte[] jar=Files.readAllBytes(Path.of(args[0]));
        String sha=HexFormat.of().formatHex(MessageDigest.getInstance("SHA-1").digest(jar));
        if(jar.length!=51623779||!sha.equals("450698d1863ab5180c25d7c804ef0fe6369dd1ba"))throw new IllegalArgumentException("Unpinned original jar");
        SharedConstants.tryDetectVersion();Bootstrap.bootStrap();
        var f=sun.misc.Unsafe.class.getDeclaredField("theUnsafe");f.setAccessible(true);var u=(sun.misc.Unsafe)f.get(null);
        var server=(net.minecraft.server.MinecraftServer)u.allocateInstance(net.minecraft.server.dedicated.DedicatedServer.class);
        var rules=new GameRules();rules.getRule(GameRules.RULE_LOGADMINCOMMANDS).set(false,null);
        var dataType=net.minecraft.world.level.storage.ServerLevelData.class;
        Object data=Proxy.newProxyInstance(dataType.getClassLoader(),new Class[]{dataType},(p,m,a)->{
            if(m.getName().equals("getGameRules"))return rules;throw new UnsupportedOperationException(m.getName());
        });
        var level=(net.minecraft.server.level.ServerLevel)u.allocateInstance(net.minecraft.server.level.ServerLevel.class);
        field(level,net.minecraft.world.level.Level.class,"levelData",data);
        field(server,net.minecraft.server.MinecraftServer.class,"levels",Map.of(net.minecraft.world.level.Level.OVERWORLD,level));
        var players=new ArrayList<ServerPlayer>();var connections=new ArrayList<RecordingConnection>();
        for(String name:new String[]{"VisibilitySource","VisibilityOther"}){
            var player=(VisiblePlayer)u.allocateInstance(VisiblePlayer.class);
            player.profile=new GameProfile(UUID.nameUUIDFromBytes(("OfflinePlayer:"+name).getBytes(java.nio.charset.StandardCharsets.UTF_8)),name);
            field(player,net.minecraft.world.entity.Entity.class,"uuid",player.profile.getId());
            field(player,net.minecraft.world.entity.Entity.class,"type",net.minecraft.world.entity.EntityType.PLAYER);
            var connection=(RecordingConnection)u.allocateInstance(RecordingConnection.class);connection.packets=new ArrayList<>();player.connection=connection;
            players.add(player);connections.add(connection);
        }
        var list=(PlayerList)u.allocateInstance(net.minecraft.server.dedicated.DedicatedPlayerList.class);
        var ops=new ServerOpList(new java.io.File("unused-ops.json"));var map=StoredUserList.class.getDeclaredField("map");map.setAccessible(true);
        for(var p:players)((Map)map.get(ops)).put(p.getGameProfile().getId().toString(),new ServerOpListEntry(p.getGameProfile(),4,false));
        field(list,PlayerList.class,"players",players);field(list,PlayerList.class,"ops",ops);field(server,net.minecraft.server.MinecraftServer.class,"playerList",list);
        var source=players.get(0);var observer=players.get(1);var rows=new JsonArray();
        for(var visibility:ChatVisiblity.values()){
            for(var p:players)field(p,ServerPlayer.class,"chatVisibility",visibility);
            var stack=new CommandSourceStack(source,Vec3.ZERO,Vec2.ZERO,level,4,"VisibilitySource",source.getDisplayName(),server,source);
            connections.forEach(c->c.packets.clear());stack.sendSuccess(()->Component.translatable("commands.weather.set.clear"),true);
            var row=new JsonObject();row.addProperty("visibility",visibility.name());row.add("private",encoded(connections.get(0)));row.add("admin",encoded(connections.get(1)));
            connections.forEach(c->c.packets.clear());stack.sendFailure(Component.translatable("commands.difficulty.failure","hard"));row.add("failure",encoded(connections.get(0)));
            List<Component> console=new ArrayList<>();
            var sink=new CommandSource(){public void sendSystemMessage(Component c){console.add(c);}public boolean acceptsSuccess(){return true;}public boolean acceptsFailure(){return true;}public boolean shouldInformAdmins(){return true;}};
            connections.forEach(c->c.packets.clear());
            var consoleStack=new CommandSourceStack(sink,Vec3.ZERO,Vec2.ZERO,level,4,"Server",Component.literal("Server"),server,null);
            consoleStack.sendSuccess(()->Component.translatable("commands.weather.set.clear"),true);row.add("console_admin",encoded(connections.get(1)));
            if(console.size()!=1)throw new AssertionError("console source suppressed");rows.add(row);
        }
        var out=new JsonObject();out.addProperty("java_jar_sha1",sha);out.add("rows",rows);
        Files.writeString(Path.of(args[1]),new GsonBuilder().setPrettyPrinting().create().toJson(out)+"\n");
    }
}
