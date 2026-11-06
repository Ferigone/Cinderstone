// SPDX-License-Identifier: GPL-2.0-only
// Executes original SetPlayerIdleTimeoutCommand, source metadata, results, rich feedback and permission trees.
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
import net.minecraft.server.commands.SetPlayerIdleTimeoutCommand;
import net.minecraft.world.phys.*;

public class idle_command_oracle {
    public static class Server extends net.minecraft.server.dedicated.DedicatedServer {
        int minutes;
        Server(){super(null,null,null,null,null,null,null,null);}
        public void setPlayerIdleTimeout(int value){minutes=value;}
        public int getPlayerIdleTimeout(){return minutes;}
    }
    public static class Player extends net.minecraft.server.level.ServerPlayer {
        Player(){super(null,null,null,null);}
        public void doTick(){}
        public void absMoveTo(double x,double y,double z,float yaw,float pitch){}
        public net.minecraft.world.entity.Entity getRootVehicle(){return this;}
    }
    public static class Listener extends net.minecraft.server.network.ServerGamePacketListenerImpl {
        Component reason;
        Listener(){super(null,null,null,null);}
        public void resetPosition(){}
        protected void keepConnectionAlive(){}
        public void disconnect(Component value){reason=value;}
    }

    @SuppressWarnings({"unchecked", "rawtypes"})
    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion(); Bootstrap.bootStrap();
        com.mojang.brigadier.exceptions.CommandSyntaxException.BUILT_IN_EXCEPTIONS = new BrigadierExceptions();
        var dispatcher = new CommandDispatcher<CommandSourceStack>();
        SetPlayerIdleTimeoutCommand.register(dispatcher);
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
        JsonArray executions = new JsonArray();
        for (String input : new String[]{"setidletimeout 0","setidletimeout 1","setidletimeout 2147483647",
                "setidletimeout -1","setidletimeout 2147483648","setidletimeout 1 extra","setidletimeout 1 ","setidletimeout"})
            executions.add(execute(dispatcher, unsafe, input, 3));
        executions.add(execute(dispatcher,unsafe,"setidletimeout 1",2));
        result.add("execution",executions);
        JsonArray deadlines = new JsonArray();
        var previous = net.minecraft.Util.timeSource;
        try {
            for(long[] values : new long[][]{{99999,40000,1},{100000,40000,1},{100001,40000,1},{100001,0,1},
                {100001,-1,1},{100001,1,0},{100001,1,-1},{100001,1,2147483647L}}) {
                long now=values[0],last=values[1]; int minutes=(int)values[2];
                net.minecraft.Util.timeSource=()->now*1000000L;
                var player=(Player)unsafe.allocateInstance(Player.class);
                field(player,net.minecraft.world.entity.Entity.class,"position",Vec3.ZERO);
                field(player,net.minecraft.server.level.ServerPlayer.class,"lastActionTime",last);
                var server=(Server)unsafe.allocateInstance(Server.class);server.minutes=minutes;
                var listener=(Listener)unsafe.allocateInstance(Listener.class);
                field(listener,net.minecraft.server.network.ServerCommonPacketListenerImpl.class,"server",server);
                field(listener,net.minecraft.server.network.ServerGamePacketListenerImpl.class,"player",player);
                field(listener,net.minecraft.server.network.ServerGamePacketListenerImpl.class,"ackBlockChangesUpTo",-1);
                listener.tick();
                JsonObject row=new JsonObject();row.addProperty("now",now);row.addProperty("last",last);row.addProperty("minutes",minutes);
                row.addProperty("kicked",listener.reason!=null);deadlines.add(row);
            }
        } finally {net.minecraft.Util.timeSource=previous;}
        result.add("deadlines",deadlines);
        Files.writeString(Path.of(args[0]), new GsonBuilder().setPrettyPrinting().create().toJson(result)+"\n");
    }
    static void field(Object object, Class<?> owner, String name, Object value) throws Exception {
        Field field=owner.getDeclaredField(name);field.setAccessible(true);field.set(object,value);
    }
    @SuppressWarnings({"unchecked","rawtypes"})
    static JsonObject execute(CommandDispatcher<CommandSourceStack> dispatcher,sun.misc.Unsafe unsafe,String input,int permission) throws Exception {
        var server=(Server)unsafe.allocateInstance(Server.class);server.minutes=7;
        Component[] feedback=new Component[1];
        var sink=new CommandSource(){
            public void sendSystemMessage(Component message){feedback[0]=message;}
            public boolean acceptsSuccess(){return true;}public boolean acceptsFailure(){return true;}
            public boolean shouldInformAdmins(){return false;}
        };
        var source=new CommandSourceStack(sink,Vec3.ZERO,Vec2.ZERO,null,permission,"n",Component.literal("n"),server,null);
        JsonObject out=new JsonObject();out.addProperty("input",input);out.addProperty("permission",permission);
        try{out.addProperty("result",dispatcher.execute(input,source));out.addProperty("ok",true);}
        catch(com.mojang.brigadier.exceptions.CommandSyntaxException error){out.addProperty("ok",false);}
        out.addProperty("minutes",server.minutes);
        if(feedback[0]!=null){
            var buffer=new net.minecraft.network.RegistryFriendlyByteBuf(Unpooled.buffer(),net.minecraft.core.RegistryAccess.EMPTY);
            try{
                ((StreamCodec)net.minecraft.network.protocol.game.ClientboundSystemChatPacket.STREAM_CODEC).encode(buffer,new net.minecraft.network.protocol.game.ClientboundSystemChatPacket(feedback[0],false));
                byte[] bytes=new byte[buffer.readableBytes()];buffer.readBytes(bytes);out.addProperty("feedback_hex",HexFormat.of().formatHex(bytes));
            }finally{buffer.release();}
        }
        return out;
    }
}
