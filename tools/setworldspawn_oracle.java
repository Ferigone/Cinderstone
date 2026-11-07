// SPDX-License-Identifier: GPL-2.0-only
// Original SetWorldSpawnCommand/Brigadier and notification codec, with a recording world sink.
package cinderstone.oracle;
import com.google.gson.*;
import com.mojang.brigadier.*;
import com.mojang.brigadier.tree.*;
import java.lang.reflect.*;
import java.nio.file.*;
import java.security.MessageDigest;
import java.util.*;
import net.minecraft.commands.*;
import net.minecraft.core.BlockPos;
import net.minecraft.network.chat.Component;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.protocol.game.*;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.phys.*;

public class setworldspawn_oracle extends admin_feedback_oracle {
    static Component sourceDisplay;
    // Only the external level mutation/ticket/player-list sink is recorded.
    public static class Level extends ServerLevel {
        BlockPos spawn;float angle;
        Level(){super(null,null,null,null,null,null,null,false,0,null,false,null);}
        public void setDefaultSpawnPos(BlockPos pos,float value){spawn=pos;angle=value;}
    }
    @SuppressWarnings({"rawtypes","unchecked"})
    static String bytes(StreamCodec codec,Object value){
        var b=new net.minecraft.network.RegistryFriendlyByteBuf(io.netty.buffer.Unpooled.buffer(),net.minecraft.core.RegistryAccess.EMPTY);
        try{codec.encode(b,value);return HexFormat.of().formatHex(io.netty.buffer.ByteBufUtil.getBytes(b));}finally{b.release();}
    }
    @SuppressWarnings({"rawtypes","unchecked"})
    public static void main(String[] args)throws Exception{
        byte[] jar=Files.readAllBytes(Path.of(args[0]));String sha=HexFormat.of().formatHex(MessageDigest.getInstance("SHA-1").digest(jar));
        if(jar.length!=51623779||!sha.equals("450698d1863ab5180c25d7c804ef0fe6369dd1ba"))throw new IllegalArgumentException("Unpinned original jar");
        net.minecraft.SharedConstants.tryDetectVersion();net.minecraft.server.Bootstrap.bootStrap();
        com.mojang.brigadier.exceptions.CommandSyntaxException.BUILT_IN_EXCEPTIONS=new BrigadierExceptions();
        var f=sun.misc.Unsafe.class.getDeclaredField("theUnsafe");f.setAccessible(true);var u=(sun.misc.Unsafe)f.get(null);
        var level=(Level)u.allocateInstance(Level.class);
        field(level,net.minecraft.world.level.Level.class,"dimension",net.minecraft.world.level.Level.OVERWORLD);
        var server=(net.minecraft.server.MinecraftServer)u.allocateInstance(net.minecraft.server.dedicated.DedicatedServer.class);
        var dispatcher=new CommandDispatcher<CommandSourceStack>();net.minecraft.server.commands.SetWorldSpawnCommand.register(dispatcher);
        List<Component> feedback=new ArrayList<>();
        var sink=new CommandSource(){public void sendSystemMessage(Component c){feedback.add(c);}public boolean acceptsSuccess(){return true;}public boolean acceptsFailure(){return true;}public boolean shouldInformAdmins(){return false;}};
        var at=new Vec3(12.5,65,18.5);var rotation=new Vec2(0,-180);
        var player=(admin_feedback_oracle.Player)u.allocateInstance(admin_feedback_oracle.Player.class);
        player.profile=new com.mojang.authlib.GameProfile(UUID.nameUUIDFromBytes("OfflinePlayer:SpawnSource".getBytes(java.nio.charset.StandardCharsets.UTF_8)),"SpawnSource");
        field(player,net.minecraft.world.entity.Entity.class,"uuid",player.profile.getId());field(player,net.minecraft.world.entity.Entity.class,"type",net.minecraft.world.entity.EntityType.PLAYER);
        sourceDisplay=player.getDisplayName();
        var source=new CommandSourceStack(sink,at,rotation,level,2,"Spawn",Component.literal("Spawn"),server,null);
        var output=new JsonObject();output.addProperty("java_jar_sha1",sha);var grammar=new JsonArray();
        var fill=Commands.class.getDeclaredMethod("fillUsableCommands",CommandNode.class,CommandNode.class,CommandSourceStack.class,Map.class);fill.setAccessible(true);
        for(int permission:new int[]{0,2}){
            var s=source.withPermission(permission);var root=new RootCommandNode<CommandSourceStack>();Map map=new HashMap();map.put(dispatcher.getRoot(),root);
            fill.invoke(u.allocateInstance(Commands.class),dispatcher.getRoot(),root,s,map);var visible=new CommandDispatcher<CommandSourceStack>(root);
            var row=new JsonObject();row.addProperty("permission",permission);row.addProperty("tree_hex",bytes(ClientboundCommandsPacket.STREAM_CODEC,new ClientboundCommandsPacket((RootCommandNode)root)));
            var completions=new JsonArray();for(String input:new String[]{"","set","setworldspawn ","setworldspawn ~ ","setworldspawn 1 ","setworldspawn 1 2 ","setworldspawn ^ ","setworldspawn ^ ^ ","setworldspawn 1 2 3 "}){
                var c=new JsonObject();c.addProperty("input",input);var v=visible.getCompletionSuggestions(visible.parse(input,s)).join();
                c.addProperty("start",v.getRange().getStart());c.addProperty("end",v.getRange().getEnd());var texts=new JsonArray();v.getList().forEach(a->texts.add(a.getText()));c.add("texts",texts);completions.add(c);
            }row.add("completions",completions);grammar.add(row);
        }output.add("grammar",grammar);var executions=new JsonArray();
        for(String input:new String[]{"setworldspawn","setworldspawn 12 64 18","setworldspawn ~1 ~-1 ~2 ~30","setworldspawn ^1 ^0 ^2 ~-450",
            "setworldspawn 12 64 18 540","setworldspawn 12 64 18 -0.0","setworldspawn 30000000 64 0","setworldspawn 0 20000000 0",
            "setworldspawn 1.5 64 0","setworldspawn 1 2","setworldspawn 1 2 3 4 extra","setworldspawn 1 2 3 NaN"}){
            executions.add(execute(dispatcher,source,level,feedback,input));
        }executions.add(execute(dispatcher,source.withPermission(0),level,feedback,"setworldspawn 12 64 18"));
        var console=execute(dispatcher,source.withPosition(new Vec3(12,65,18)).withRotation(Vec2.ZERO),level,feedback,"setworldspawn ~1 ~ ~1 ~45");console.addProperty("console_case",true);executions.add(console);
        output.add("execution",executions);Files.writeString(Path.of(args[1]),new GsonBuilder().setPrettyPrinting().create().toJson(output)+"\n");
    }
    static JsonObject execute(CommandDispatcher<CommandSourceStack> dispatcher,CommandSourceStack source,Level level,List<Component> feedback,String input){
        level.spawn=null;feedback.clear();var row=new JsonObject();row.addProperty("input",input);row.addProperty("permission",source.hasPermission(2)?2:0);
        var position=new JsonArray();position.add(source.getPosition().x);position.add(source.getPosition().y);position.add(source.getPosition().z);row.add("source_pos",position);
        var rotation=new JsonArray();rotation.add(source.getRotation().x);rotation.add(source.getRotation().y);row.add("rotation",rotation);
        try{row.addProperty("result",dispatcher.execute(input,source));row.addProperty("ok",true);}
        catch(com.mojang.brigadier.exceptions.CommandSyntaxException e){row.addProperty("ok",false);if(e.getRawMessage() instanceof Component c && c.getContents() instanceof net.minecraft.network.chat.contents.TranslatableContents t)row.addProperty("error_key",t.getKey());}
        row.addProperty("mutated",level.spawn!=null);
        if(level.spawn!=null){var pos=new JsonArray();pos.add(level.spawn.getX());pos.add(level.spawn.getY());pos.add(level.spawn.getZ());row.add("pos",pos);row.addProperty("angle_bits",Float.floatToRawIntBits(level.angle));
            row.addProperty("spawn_packet_hex",bytes(ClientboundSetDefaultSpawnPositionPacket.STREAM_CODEC,new ClientboundSetDefaultSpawnPositionPacket(level.spawn,level.angle)));
            row.addProperty("feedback_hex",packet(feedback.get(0)));
            row.addProperty("admin_hex",packet(Component.translatable("chat.type.admin",sourceDisplay,feedback.get(0)).withStyle(net.minecraft.ChatFormatting.GRAY,net.minecraft.ChatFormatting.ITALIC)));
            row.addProperty("console_admin_hex",packet(Component.translatable("chat.type.admin",Component.literal("Server"),feedback.get(0)).withStyle(net.minecraft.ChatFormatting.GRAY,net.minecraft.ChatFormatting.ITALIC)));}
        return row;
    }
}
