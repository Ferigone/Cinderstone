// SPDX-License-Identifier: GPL-2.0-only
// Executes original Player.getDisplayName and CommandSourceStack.sendSuccess/broadcastToAdmins.
package cinderstone.oracle;

import com.google.gson.*;
import com.mojang.authlib.GameProfile;
import io.netty.buffer.Unpooled;
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
import net.minecraft.world.level.GameRules;
import net.minecraft.world.phys.*;
import net.minecraft.world.scores.*;

public class admin_feedback_oracle {
    // Replace external connection/profile/team lookups only; original display and feedback run.
    public static class Player extends ServerPlayer {
        GameProfile profile; PlayerTeam team; List<Component> messages;
        Player() {super(null, null, null, null);}
        public GameProfile getGameProfile() {return profile;}
        public Component getName() {return Component.literal(profile.getName());}
        public PlayerTeam getTeam() {return team;}
        public void sendSystemMessage(Component c) {messages.add(c);}
        public boolean acceptsSuccess() {return true;}
        public boolean acceptsFailure() {return true;}
        public boolean shouldInformAdmins() {return true;}
    }
    static void field(Object o, Class<?> owner, String name, Object value) throws Exception {
        var f=owner.getDeclaredField(name); f.setAccessible(true); f.set(o,value);
    }
    static String packet(Component c) {
        var b=new net.minecraft.network.RegistryFriendlyByteBuf(Unpooled.buffer(),net.minecraft.core.RegistryAccess.EMPTY);
        try {
            ClientboundSystemChatPacket.STREAM_CODEC.encode(b,new ClientboundSystemChatPacket(c,false));
            return HexFormat.of().formatHex(io.netty.buffer.ByteBufUtil.getBytes(b));
        } finally {b.release();}
    }
    @SuppressWarnings({"unchecked","rawtypes"})
    public static void main(String[] args) throws Exception {
        byte[] jar=Files.readAllBytes(Path.of(args[0]));
        String sha=HexFormat.of().formatHex(MessageDigest.getInstance("SHA-1").digest(jar));
        if(jar.length!=51623779 || !sha.equals("450698d1863ab5180c25d7c804ef0fe6369dd1ba")) throw new IllegalArgumentException("Unpinned original jar");
        SharedConstants.tryDetectVersion(); Bootstrap.bootStrap();
        var f=sun.misc.Unsafe.class.getDeclaredField("theUnsafe");f.setAccessible(true);var u=(sun.misc.Unsafe)f.get(null);
        var server=(net.minecraft.server.MinecraftServer)u.allocateInstance(net.minecraft.server.dedicated.DedicatedServer.class);
        var rules=new GameRules(); rules.getRule(GameRules.RULE_LOGADMINCOMMANDS).set(false,null);
        var dataType=net.minecraft.world.level.storage.ServerLevelData.class;
        Object data=Proxy.newProxyInstance(dataType.getClassLoader(),new Class[]{dataType},(p,m,a)->{
            if(m.getName().equals("getGameRules"))return rules;throw new UnsupportedOperationException(m.getName());
        });
        var level=(net.minecraft.server.level.ServerLevel)u.allocateInstance(net.minecraft.server.level.ServerLevel.class);
        field(level,net.minecraft.world.level.Level.class,"levelData",data);
        var border=new net.minecraft.world.level.border.WorldBorder();
        field(level,net.minecraft.world.level.Level.class,"worldBorder",border);
        field(server,net.minecraft.server.MinecraftServer.class,"levels",Map.of(net.minecraft.world.level.Level.OVERWORLD,level));
        var dispatcher=new com.mojang.brigadier.CommandDispatcher<CommandSourceStack>();
        net.minecraft.server.commands.WorldBorderCommand.register(dispatcher);
        var players=new ArrayList<ServerPlayer>();
        for(String name:new String[]{"FeedbackSource","FeedbackOther"}) {
            var p=(Player)u.allocateInstance(Player.class);p.profile=new GameProfile(UUID.nameUUIDFromBytes(("OfflinePlayer:"+name).getBytes(java.nio.charset.StandardCharsets.UTF_8)),name);
            p.messages=new ArrayList<>();field(p,net.minecraft.world.entity.Entity.class,"uuid",p.profile.getId());
            field(p,net.minecraft.world.entity.Entity.class,"type",net.minecraft.world.entity.EntityType.PLAYER);players.add(p);
        }
        var list=(PlayerList)u.allocateInstance(net.minecraft.server.dedicated.DedicatedPlayerList.class);
        var ops=new ServerOpList(new java.io.File("unused-ops.json"));
        var map=StoredUserList.class.getDeclaredField("map");map.setAccessible(true);
        for(var p:players)((Map)map.get(ops)).put(p.getGameProfile().getId().toString(),new ServerOpListEntry(p.getGameProfile(),4,false));
        field(list,PlayerList.class,"players",players);field(list,PlayerList.class,"ops",ops);
        field(server,net.minecraft.server.MinecraftServer.class,"playerList",list);
        var source=(Player)players.get(0);var observer=(Player)players.get(1);
        JsonArray rows=new JsonArray();
        for(String phase:new String[]{"red","blue","none","console"}) {
            source.team=null;
            if(phase.equals("red")||phase.equals("blue")) {
                source.team=new PlayerTeam(new Scoreboard(),"feedback");
                source.team.setColor(phase.equals("red")?net.minecraft.ChatFormatting.RED:net.minecraft.ChatFormatting.BLUE);
                source.team.setPlayerPrefix(phase.equals("red")?Component.literal("[T] "):
                    Component.literal("[U] ").withStyle(s->s.withFont(net.minecraft.resources.ResourceLocation.withDefaultNamespace("uniform"))));
                source.team.setPlayerSuffix(phase.equals("red")?Component.literal("!"):Component.keybind("key.jump"));
            }
            String[] commands={"time set 1000","weather clear 120","difficulty hard","worldborder center 12 -8"};
            Component[] feedback={Component.translatable("commands.time.set",1000),Component.translatable("commands.weather.set.clear"),
                Component.translatable("commands.difficulty.success",Component.translatable("options.difficulty.hard")),
                null};
            for(int i=0;i<commands.length;i++) {
                source.messages.clear();observer.messages.clear();
                List<Component> console=new ArrayList<>();
                var sink=new CommandSource(){public void sendSystemMessage(Component c){console.add(c);}public boolean acceptsSuccess(){return true;}public boolean acceptsFailure(){return true;}public boolean shouldInformAdmins(){return true;}};
                boolean isConsole=phase.equals("console");
                var stack=new CommandSourceStack(isConsole?sink:source,Vec3.ZERO,Vec2.ZERO,level,4,isConsole?"Server":source.profile.getName(),isConsole?Component.literal("Server"):source.getDisplayName(),server,isConsole?null:source);
                if(feedback[i]==null) {
                    border.setCenter(0,0);dispatcher.execute(commands[i],stack);
                } else {
                    final Component message=feedback[i];stack.sendSuccess(()->message,true);
                }
                if(observer.messages.size()!=1 || (isConsole?console:source.messages).size()!=1)throw new AssertionError("feedback audience");
                var row=new JsonObject();row.addProperty("phase",phase);row.addProperty("command",commands[i]);
                row.addProperty("private_hex",packet((isConsole?console:source.messages).get(0)));
                row.addProperty("admin_hex",packet(observer.messages.get(0)));rows.add(row);
            }
        }
        var output=new JsonObject();output.addProperty("java_jar_sha1",sha);output.add("rows",rows);
        Files.writeString(Path.of(args[1]),new GsonBuilder().setPrettyPrinting().create().toJson(output)+"\n");
    }
}
