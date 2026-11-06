// SPDX-License-Identifier: GPL-2.0-only
// Executes original ListPlayersCommand, Player display formatting and system-chat codec.
package cinderstone.oracle;
import com.google.gson.*;
import com.mojang.authlib.GameProfile;
import com.mojang.brigadier.*;
import com.mojang.brigadier.tree.*;
import io.netty.buffer.Unpooled;
import java.lang.reflect.*;
import java.nio.file.*;
import java.util.*;
import net.minecraft.SharedConstants;
import net.minecraft.commands.*;
import net.minecraft.network.chat.Component;
import net.minecraft.network.protocol.game.ClientboundCommandsPacket;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.commands.ListPlayersCommand;
import net.minecraft.server.level.*;
import net.minecraft.server.players.PlayerList;
import net.minecraft.world.scores.*;
import net.minecraft.world.phys.*;

public class list_command_oracle {
    // Only external world/team lookup is replaced. Original Player.getDisplayName runs.
    public static class Player extends ServerPlayer {
        GameProfile profile; PlayerTeam team;
        Player() {super(null, null, null, null);}
        public GameProfile getGameProfile() {return profile;}
        public Component getName() {return Component.literal(profile.getName());}
        public PlayerTeam getTeam() {return team;}
    }
    static sun.misc.Unsafe unsafe;
    static void field(Object object, Class<?> owner, String name, Object value) throws Exception {
        Field field = owner.getDeclaredField(name); field.setAccessible(true); field.set(object, value);
    }
    @SuppressWarnings({"unchecked", "rawtypes"})
    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion(); Bootstrap.bootStrap();
        com.mojang.brigadier.exceptions.CommandSyntaxException.BUILT_IN_EXCEPTIONS = new BrigadierExceptions();
        Field f = sun.misc.Unsafe.class.getDeclaredField("theUnsafe"); f.setAccessible(true); unsafe = (sun.misc.Unsafe)f.get(null);
        var dispatcher = new CommandDispatcher<CommandSourceStack>(); ListPlayersCommand.register(dispatcher);
        JsonObject result = new JsonObject(); JsonArray trees = new JsonArray();
        Method fill = Commands.class.getDeclaredMethod("fillUsableCommands", CommandNode.class, CommandNode.class, CommandSourceStack.class, Map.class); fill.setAccessible(true);
        for (int permission : new int[]{0, 4}) {
            var source = new CommandSourceStack(CommandSource.NULL, Vec3.ZERO, Vec2.ZERO, null, permission, "n", Component.literal("n"), null, null);
            var root = new RootCommandNode<CommandSourceStack>(); Map map = new HashMap(); map.put(dispatcher.getRoot(), root);
            fill.invoke(unsafe.allocateInstance(Commands.class), dispatcher.getRoot(), root, source, map);
            JsonObject row = new JsonObject(); row.addProperty("permission", permission);
            row.addProperty("packet_hex", packet(ClientboundCommandsPacket.STREAM_CODEC, new ClientboundCommandsPacket((RootCommandNode)root))); trees.add(row);
        }
        result.add("trees", trees); JsonArray executions = new JsonArray();
        for (int count : new int[]{0,1,2}) for (boolean team : new boolean[]{false,true}) for (String input : new String[]{"list","list uuids"}) {
            if (count == 0 && team) continue;
            executions.add(execute(dispatcher, count, team, input));
        }
        for (String input : new String[]{"list ", "list uuids ", "list bad", "list uuids extra"}) executions.add(execute(dispatcher, 2, false, input));
        result.add("execution", executions);
        Files.writeString(Path.of(args[0]), new GsonBuilder().setPrettyPrinting().create().toJson(result)+"\n");
    }
    @SuppressWarnings({"rawtypes", "unchecked"})
    static String packet(StreamCodec codec, Object packet) {
        var buf = new net.minecraft.network.RegistryFriendlyByteBuf(Unpooled.buffer(), net.minecraft.core.RegistryAccess.EMPTY);
        try {codec.encode(buf, packet); byte[] bytes=new byte[buf.readableBytes()]; buf.readBytes(bytes); return HexFormat.of().formatHex(bytes);} finally {buf.release();}
    }
    static JsonObject execute(CommandDispatcher<CommandSourceStack> dispatcher, int count, boolean decorated, String input) throws Exception {
        var server = (net.minecraft.server.MinecraftServer)unsafe.allocateInstance(net.minecraft.server.dedicated.DedicatedServer.class);
        var list = (PlayerList)unsafe.allocateInstance(net.minecraft.server.dedicated.DedicatedPlayerList.class);
        List<ServerPlayer> players = new ArrayList<>();
        for (int i=0;i<count;i++) {
            Player player = (Player)unsafe.allocateInstance(Player.class);
            player.profile = new GameProfile(new UUID(0,i+1), i==0?"Zulu":"Alpha");
            field(player, net.minecraft.world.entity.Entity.class, "uuid", player.profile.getId());
            field(player, net.minecraft.world.entity.Entity.class, "type", net.minecraft.world.entity.EntityType.PLAYER);
            if (decorated && i==0) {
                player.team = new PlayerTeam(new Scoreboard(), "test");
                player.team.setColor(net.minecraft.ChatFormatting.RED);
                player.team.setPlayerPrefix(Component.literal("[T] ")); player.team.setPlayerSuffix(Component.literal("!"));
            }
            players.add(player);
        }
        field(list, PlayerList.class, "players", players); field(list, PlayerList.class, "maxPlayers", 37);
        field(server, net.minecraft.server.MinecraftServer.class, "playerList", list);
        Component[] feedback = new Component[1];
        var sink = new CommandSource() {
            public void sendSystemMessage(Component message) {feedback[0]=message;}
            public boolean acceptsSuccess() {return true;} public boolean acceptsFailure() {return true;}
            public boolean shouldInformAdmins() {return false;}
        };
        var source = new CommandSourceStack(sink, Vec3.ZERO, Vec2.ZERO, null, 0, "n", Component.literal("n"), server, null);
        JsonObject out = new JsonObject(); out.addProperty("count",count); out.addProperty("team",decorated); out.addProperty("input",input);
        try {out.addProperty("result",dispatcher.execute(input,source));out.addProperty("ok",true);} catch(com.mojang.brigadier.exceptions.CommandSyntaxException error){out.addProperty("ok",false);}
        if(feedback[0]!=null) out.addProperty("feedback_hex",packet(net.minecraft.network.protocol.game.ClientboundSystemChatPacket.STREAM_CODEC,new net.minecraft.network.protocol.game.ClientboundSystemChatPacket(feedback[0],false)));
        return out;
    }
}
