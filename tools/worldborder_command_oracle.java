// SPDX-License-Identifier: GPL-2.0-only
// Runs unchanged original 1.21 commands and packet codecs with an injected clock.
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
import net.minecraft.network.protocol.game.*;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.commands.WorldBorderCommand;
import net.minecraft.world.level.border.*;
import net.minecraft.world.phys.*;
public class worldborder_command_oracle {
 static long now;
 static void field(Object o,Class<?> c,String n,Object v)throws Exception{var f=c.getDeclaredField(n);f.setAccessible(true);f.set(o,v);}
 static String bytes(Component c){var b=Unpooled.buffer();try{net.minecraft.network.chat.ComponentSerialization.TRUSTED_CONTEXT_FREE_STREAM_CODEC.encode(b,c);return HexFormat.of().formatHex(io.netty.buffer.ByteBufUtil.getBytes(b));}finally{b.release();}}
 @SuppressWarnings({"unchecked","rawtypes"})
 public static void main(String[] args)throws Exception {
  SharedConstants.tryDetectVersion();Bootstrap.bootStrap();
  com.mojang.brigadier.exceptions.CommandSyntaxException.BUILT_IN_EXCEPTIONS=new BrigadierExceptions();
  var clock=Class.forName("net.minecraft.util.TimeSource$NanoTimeSource");
  net.minecraft.Util.timeSource=(net.minecraft.util.TimeSource.NanoTimeSource)Proxy.newProxyInstance(clock.getClassLoader(),new Class[]{clock},(p,m,a)->{if(m.getName().equals("getAsLong"))return now*1000000L;throw new UnsupportedOperationException(m.getName());});
  var f=sun.misc.Unsafe.class.getDeclaredField("theUnsafe");f.setAccessible(true);var u=(sun.misc.Unsafe)f.get(null);
  var dispatcher=new CommandDispatcher<CommandSourceStack>();WorldBorderCommand.register(dispatcher);
  var level=(net.minecraft.server.level.ServerLevel)u.allocateInstance(net.minecraft.server.level.ServerLevel.class);
  var server=(net.minecraft.server.MinecraftServer)u.allocateInstance(net.minecraft.server.dedicated.DedicatedServer.class);
  field(server,net.minecraft.server.MinecraftServer.class,"levels",Map.of(net.minecraft.world.level.Level.OVERWORLD,level));
  var border=new WorldBorder();field(level,net.minecraft.world.level.Level.class,"worldBorder",border);
  List<Component> messages=new ArrayList<>();JsonArray[] packets={new JsonArray()};
  border.addListener((BorderChangeListener)Proxy.newProxyInstance(BorderChangeListener.class.getClassLoader(),new Class[]{BorderChangeListener.class},(p,m,a)->{
   Object packet=null;int id=0;
   switch(m.getName()) {
    case "onBorderSizeSet":packet=new ClientboundSetBorderSizePacket(border);id=0x4f;break;
    case "onBorderSizeLerping":packet=new ClientboundSetBorderLerpSizePacket(border);id=0x4e;break;
    case "onBorderCenterSet":packet=new ClientboundSetBorderCenterPacket(border);id=0x4d;break;
    case "onBorderSetWarningTime":packet=new ClientboundSetBorderWarningDelayPacket(border);id=0x50;break;
    case "onBorderSetWarningBlocks":packet=new ClientboundSetBorderWarningDistancePacket(border);id=0x51;break;
   }
   if(packet!=null){var b=new FriendlyByteBuf(Unpooled.buffer());try{var codec=(StreamCodec)packet.getClass().getField("STREAM_CODEC").get(null);codec.encode(b,packet);var row=new JsonObject();row.addProperty("id",id);row.addProperty("payload",HexFormat.of().formatHex(io.netty.buffer.ByteBufUtil.getBytes(b)));packets[0].add(row);}finally{b.release();}}
   return null;
  }));
  var sink=new CommandSource(){public void sendSystemMessage(Component c){messages.add(c);}public boolean acceptsSuccess(){return true;}public boolean acceptsFailure(){return true;}public boolean shouldInformAdmins(){return false;}};
  var source=new CommandSourceStack(sink,new Vec3(10.25,65,-3.5),Vec2.ZERO,level,2,"Border",Component.literal("Border"),server,null);
  JsonObject output=new JsonObject();JsonArray grammar=new JsonArray();
  var commands=(Commands)u.allocateInstance(Commands.class);
  var fill=Commands.class.getDeclaredMethod("fillUsableCommands",CommandNode.class,CommandNode.class,CommandSourceStack.class,Map.class);fill.setAccessible(true);
  for(int permission:new int[]{0,2,4}){
   var s=source.withPermission(permission);var root=new RootCommandNode<CommandSourceStack>();Map map=new HashMap();map.put(dispatcher.getRoot(),root);fill.invoke(commands,dispatcher.getRoot(),root,s,map);
   var visible=new CommandDispatcher<CommandSourceStack>(root);var g=new JsonObject();g.addProperty("permission",permission);var b=new FriendlyByteBuf(Unpooled.buffer());
   try{((StreamCodec)ClientboundCommandsPacket.STREAM_CODEC).encode(b,new ClientboundCommandsPacket((RootCommandNode)root));g.addProperty("packet_hex",HexFormat.of().formatHex(io.netty.buffer.ByteBufUtil.getBytes(b)));}finally{b.release();}
   JsonArray cases=new JsonArray();
   for(String input:new String[]{"","w","worldborder ","worldborder c","worldborder center ","worldborder center ~ ","worldborder center 12 ","worldborder center ^","worldborder damage ","worldborder warning ","worldborder set 64 "}){
    var row=new JsonObject();row.addProperty("input",input);var suggestions=visible.getCompletionSuggestions(visible.parse(input,s)).join();var completion=new JsonObject();completion.addProperty("start",suggestions.getRange().getStart());completion.addProperty("end",suggestions.getRange().getEnd());var texts=new JsonArray();suggestions.getList().forEach(x->texts.add(x.getText()));completion.add("texts",texts);row.add("suggestions",completion);cases.add(row);
   }g.add("cases",cases);grammar.add(g);
  }output.add("grammar",grammar);
  JsonArray rows=new JsonArray();
  String[] inputs={"get","set 64","center 12 -8","center ~ ~","warning time 7","warning distance 9","damage amount 0.125","damage buffer 2.675","set 80 4","add 16 3","get","get","set 0","set -1","add 59999968","center 30000000 0","center ~ ~","warning time 7","damage amount 0.2","damage amount 0.2","damage buffer 5","set 2.25","get","center -0.0 1.0","set 64 trailing","set 64 -1","get"};
  long[] times={0,0,0,0,0,0,0,0,1000,2000,3000,9000,9000,9000,9000,9000,9000,9000,9000,9000,9000,9000,9000,9000,9000,9000,9000};
  for(int i=0;i<inputs.length;i++){
   now=times[i];border.tick();messages.clear();packets[0]=new JsonArray();
   var row=new JsonObject();row.addProperty("input","worldborder "+inputs[i]);row.addProperty("now",now);row.addProperty("permission",i==inputs.length-1?0:2);
   try{row.addProperty("result",dispatcher.execute("worldborder "+inputs[i],source.withPermission(i==inputs.length-1?0:2)));row.addProperty("ok",true);row.addProperty("feedback_hex",bytes(messages.get(0)));}
   catch(com.mojang.brigadier.exceptions.CommandSyntaxException e){row.addProperty("ok",false);if(e.getRawMessage() instanceof Component c && c.getContents() instanceof net.minecraft.network.chat.contents.TranslatableContents t){row.addProperty("error_key",t.getKey());row.addProperty("error_hex",bytes(c));}}
   row.add("packets",packets[0]);var state=new JsonArray();for(double v:new double[]{border.getCenterX(),border.getCenterZ(),border.getDamagePerBlock(),border.getDamageSafeZone(),border.getWarningBlocks(),border.getWarningTime(),border.getSize(),border.getLerpRemainingTime(),border.getLerpTarget()})state.add(v);row.add("state",state);rows.add(row);
  }output.add("execution",rows);Files.writeString(Path.of(args[0]),new GsonBuilder().setPrettyPrinting().create().toJson(output)+"\n");
 }
}
